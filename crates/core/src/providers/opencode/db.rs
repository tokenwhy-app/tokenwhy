// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Tables from codeburn v0.9.27 docs/providers/opencode.md and sqlite-session-parser.ts.

use super::parse::Session;
use super::{root_of, SessionInfo};
use crate::model::FileRecords;
use crate::provider::{Change, Fingerprint};
use crate::sqlite;
use rusqlite::Connection;
use serde_json::Value;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

fn with_db<T>(db: &Path, f: impl FnOnce(&Connection) -> T) -> io::Result<T> {
    sqlite::with_cached(db, f)
}

pub fn split(virt: &Path) -> Option<(PathBuf, String)> {
    let s = virt.to_string_lossy();
    let (db, id) = s.rsplit_once('#')?;
    Some((PathBuf::from(db), id.to_string()))
}

fn cols(c: &Connection, table: &str) -> Vec<String> {
    c.prepare(&format!("PRAGMA table_info({table})"))
        .and_then(|mut s| {
            s.query_map([], |r| r.get::<_, String>(1))
                .map(|r| r.flatten().collect())
        })
        .unwrap_or_default()
}

fn sessions_in(c: &Connection) -> Vec<SessionInfo> {
    let mut out: Vec<SessionInfo> = vec![];
    for table in ["session", "session_v2"] {
        if !sqlite::has_table(c, table) {
            continue;
        }
        let have = cols(c, table);
        let col = |n: &str| {
            if have.iter().any(|h| h == n) {
                format!("`{n}`")
            } else {
                "NULL".into()
            }
        };
        let q = format!(
            "SELECT id, {}, {}, {} FROM {table}",
            col("parent_id"),
            col("directory"),
            col("agent")
        );
        let Ok(mut st) = c.prepare(&q) else { continue };
        let rows = st.query_map([], |r| {
            Ok(SessionInfo {
                id: r.get(0)?,
                parent: r.get::<_, Option<String>>(1)?.filter(|p| !p.is_empty()),
                directory: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                agent: r.get::<_, Option<String>>(3)?.filter(|a| !a.is_empty()),
            })
        });
        if let Ok(rows) = rows {
            for s in rows.flatten() {
                if !out.iter().any(|o| o.id == s.id) {
                    out.push(s);
                }
            }
        }
    }
    out
}

pub fn sessions(db: &Path) -> io::Result<Vec<(SessionInfo, String)>> {
    let all = with_db(db, sessions_in)?;
    let parents: HashMap<String, Option<String>> = all
        .iter()
        .map(|s| (s.id.clone(), s.parent.clone()))
        .collect();
    Ok(all
        .into_iter()
        .map(|s| {
            let r = root_of(&s.id, &parents);
            (s, r)
        })
        .collect())
}

fn one(c: &Connection, q: &str, id: &str) -> (u64, i64) {
    c.query_row(q, [id], |r| {
        Ok((
            r.get::<_, Option<i64>>(0)?.unwrap_or(0) as u64,
            r.get::<_, Option<i64>>(1)?.unwrap_or(0),
        ))
    })
    .unwrap_or((0, 0))
}

pub fn fingerprint(virt: &Path) -> io::Result<Fingerprint> {
    let (db, id) = split(virt).ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
    let (count, latest, v2) = with_db(&db, |c| {
        let mut count = 0;
        let mut latest = 0i64;
        for t in ["message", "part"] {
            if sqlite::has_table(c, t) {
                let (n, l) = one(
                    c,
                    &format!("SELECT count(*), max(time_updated) FROM {t} WHERE session_id=?1"),
                    &id,
                );
                count += n;
                latest = latest.max(l);
            }
        }
        let mut v2 = (0, 0);
        if sqlite::has_table(c, "session_message") {
            v2 = one(
                c,
                "SELECT count(*), max(seq) FROM session_message WHERE session_id=?1",
                &id,
            );
            let (_, l) = one(
                c,
                "SELECT count(*), max(time_updated) FROM session_message WHERE session_id=?1",
                &id,
            );
            latest = latest.max(l);
        }
        (count, latest, v2)
    })?;
    if count == 0 && v2.0 == 0 {
        return Err(io::ErrorKind::NotFound.into());
    }
    Ok(Fingerprint {
        size: count + v2.0,
        mtime_ns: latest.max(0) as u64 * 1_000_000,
        head_len: 0,
        head_hash: super::super::common::fnv(format!("{}:{}", v2.0, v2.1).as_bytes()),
        change: Change::Whole,
    })
}

fn rows(c: &Connection, q: &str, id: &str) -> Vec<(String, String, String)> {
    let Ok(mut st) = c.prepare(q) else {
        return vec![];
    };
    st.query_map([id], |r| {
        let text = |i: usize| -> rusqlite::Result<String> {
            Ok(r.get_ref(i)?
                .as_bytes_or_null()?
                .map(|b| String::from_utf8_lossy(b).into_owned())
                .unwrap_or_default())
        };
        Ok((text(0)?, text(1)?, text(2)?))
    })
    .map(|r| r.flatten().collect())
    .unwrap_or_default()
}

pub fn parse(virt: &Path, project: &str) -> io::Result<FileRecords> {
    let (db, id) = split(virt).ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
    let (legacy, parts, v2) = with_db(&db, |c| {
        let legacy = if sqlite::has_table(c, "message") {
            rows(
                c,
                "SELECT id, '', data FROM message WHERE session_id=?1 ORDER BY time_created, id",
                &id,
            )
        } else {
            vec![]
        };
        let parts = if sqlite::has_table(c, "part") {
            rows(
                c,
                "SELECT message_id, id, data FROM part WHERE session_id=?1 ORDER BY id",
                &id,
            )
        } else {
            vec![]
        };
        let v2 = if sqlite::has_table(c, "session_message") {
            rows(
                c,
                "SELECT id, type, data FROM session_message WHERE session_id=?1 ORDER BY seq",
                &id,
            )
        } else {
            vec![]
        };
        (legacy, parts, v2)
    })?;
    let mut by_msg: HashMap<String, Vec<Value>> = HashMap::new();
    for (m, _, data) in parts {
        if let Ok(v) = serde_json::from_str::<Value>(&data) {
            by_msg.entry(m).or_default().push(v);
        }
    }
    let mut s = Session::default();
    for (mid, _, data) in legacy {
        let Ok(m) = serde_json::from_str::<Value>(&data) else {
            continue;
        };
        let role = m
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        s.message(
            &mid,
            &role,
            &m,
            by_msg.get(&mid).map_or(&[][..], Vec::as_slice),
        );
    }
    for (mid, ty, data) in v2 {
        let Ok(m) = serde_json::from_str::<Value>(&data) else {
            continue;
        };
        let content: Vec<Value> = m
            .get("content")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if matches!(ty.as_str(), "user" | "assistant" | "compaction") {
            s.message(&mid, &ty, &m, &content);
        }
    }
    Ok(s.b.finish(project))
}
