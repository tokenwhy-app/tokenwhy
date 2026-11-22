// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Opening the agent's file, even read-only, can create `-shm` next to it, so every read goes through a private copy.

use rusqlite::{Connection, OpenFlags};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

pub struct DbCopy {
    conn: Option<Connection>,
    dir: PathBuf,
}

impl DbCopy {
    pub fn conn(&self) -> &Connection {
        self.conn.as_ref().expect("open until drop")
    }
}

impl Drop for DbCopy {
    fn drop(&mut self) {
        drop(self.conn.take());
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

pub fn stat_with_wal(db: &Path) -> io::Result<(u64, u64)> {
    let mut size = 0;
    let mut mtime = 0;
    for (i, p) in [db.to_path_buf(), wal_of(db)].iter().enumerate() {
        match std::fs::metadata(p) {
            Ok(m) => {
                size += m.len();
                let t = m
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_nanos() as u64);
                mtime = mtime.max(t);
            }
            Err(e) if i == 0 => return Err(e),
            Err(_) => {}
        }
    }
    Ok((size, mtime))
}

fn wal_of(db: &Path) -> PathBuf {
    let mut s = db.as_os_str().to_owned();
    s.push("-wal");
    PathBuf::from(s)
}

fn tmp_root() -> PathBuf {
    std::env::var_os("TOKENWHY_SQLITE_TMP")
        .map(PathBuf::from)
        .or_else(|| crate::cache::cache_dir().map(|d| d.join("tmp")))
        .unwrap_or_else(|| std::env::temp_dir().join("tokenwhy"))
}

/// The WAL is replayed in the copy, so rows the agent has not checkpointed are read too.
pub fn open_copy(db: &Path) -> io::Result<DbCopy> {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    static SWEEP: std::sync::Once = std::sync::Once::new();
    SWEEP.call_once(sweep_stale);
    let dir = tmp_root().join(format!(
        "{:016x}-{}-{}",
        crate::corpus::fnv1a(db.as_os_str().as_encoded_bytes()),
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir)?;
    let mut copy = DbCopy { conn: None, dir };
    let target = copy.dir.join("c.db");
    std::fs::copy(db, &target)?;
    let wal = wal_of(db);
    if wal.is_file() {
        std::fs::copy(&wal, wal_of(&target))?;
    }
    let conn = Connection::open_with_flags(
        &target,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(io::Error::other)?;
    copy.conn = Some(conn);
    Ok(copy)
}

fn sweep_stale() {
    let me = format!("-{}-", std::process::id());
    let Ok(rd) = std::fs::read_dir(tmp_root()) else {
        return;
    };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let old = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|d| d.as_secs() > 3600);
        if old && !name.contains(&me) && e.path().is_dir() {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

pub fn has_table(c: &Connection, table: &str) -> bool {
    c.query_row(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
        [table],
        |_| Ok(()),
    )
    .is_ok()
}

struct Cached {
    db: PathBuf,
    stat: (u64, u64),
    copy: DbCopy,
}

static CACHE: Mutex<Vec<Cached>> = Mutex::new(Vec::new());

pub fn with_cached<T>(db: &Path, f: impl FnOnce(&Connection) -> T) -> io::Result<T> {
    let stat = stat_with_wal(db)?;
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(i) = cache.iter().position(|c| c.db == db) {
        if cache[i].stat == stat {
            return Ok(f(cache[i].copy.conn()));
        }
        cache.remove(i);
    }
    let copy = open_copy(db)?;
    let out = f(copy.conn());
    cache.push(Cached {
        db: db.to_path_buf(),
        stat,
        copy,
    });
    Ok(out)
}

pub fn release_cached() {
    CACHE.lock().unwrap_or_else(|e| e.into_inner()).clear();
}
