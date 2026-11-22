// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Roots and layouts from ccusage `rust/adapters/opencode` (MIT, ryoppippi, read 22/10/2026) and codeburn v0.9.27 (commit 7ecc87f, MIT, AgentSeal).

#[cfg(feature = "sqlite")]
mod db;
mod parse;

use super::common;
use crate::model::{
    Capabilities, CostSource, FileRecords, FileRole, Level, ParseResume, ProviderId, SourceFile,
    Vendor,
};
use crate::provider::{Change, Fingerprint, Provider};
use serde_json::Value;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

pub const ID: &str = "opencode";

pub struct Opencode;

#[derive(Debug, Clone, Default)]
pub struct SessionInfo {
    pub id: String,
    pub parent: Option<String>,
    pub directory: String,
    pub agent: Option<String>,
}

pub(crate) fn root_of(id: &str, parents: &HashMap<String, Option<String>>) -> String {
    let mut cur = id.to_string();
    for _ in 0..64 {
        match parents.get(&cur).cloned().flatten() {
            Some(p) if parents.contains_key(&p) => cur = p,
            Some(p) => return p,
            None => return cur,
        }
    }
    cur
}

fn source(path: PathBuf, s: &SessionInfo, root: String) -> SourceFile {
    let project = common::base_name(&s.directory);
    SourceFile {
        provider: ProviderId(ID.to_string()),
        path,
        role: if root == s.id {
            FileRole::Main
        } else {
            FileRole::Agent
        },
        project_key: if project.is_empty() {
            "opencode".into()
        } else {
            project
        },
        session_id: root,
        agent_type: if s.parent.is_some() {
            s.agent.clone()
        } else {
            None
        },
        agent_group: None,
    }
}

fn read_json(p: &Path) -> Option<Value> {
    serde_json::from_slice(&std::fs::read(p).ok()?).ok()
}

fn sorted_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "json") && p.is_file())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

fn json_sessions(root: &Path) -> Vec<SessionInfo> {
    let mut out = vec![];
    let Ok(rd) = std::fs::read_dir(root.join("storage").join("session")) else {
        return out;
    };
    for proj in rd.flatten().filter(|e| e.path().is_dir()) {
        for f in sorted_files(&proj.path()) {
            let Some(v) = read_json(&f) else { continue };
            let id = v
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| f.file_stem().map(|s| s.to_string_lossy().into_owned()));
            let Some(id) = id else { continue };
            let s = |k: &str| {
                v.get(k)
                    .and_then(Value::as_str)
                    .filter(|x| !x.is_empty())
                    .map(str::to_string)
            };
            out.push(SessionInfo {
                id,
                parent: s("parentID").or_else(|| s("parent_id")),
                directory: s("directory").unwrap_or_default(),
                agent: s("agent"),
            });
        }
    }
    out
}

fn json_sources(root: &Path) -> Vec<SourceFile> {
    let all = json_sessions(root);
    let parents: HashMap<String, Option<String>> = all
        .iter()
        .map(|s| (s.id.clone(), s.parent.clone()))
        .collect();
    let msgs = root.join("storage").join("message");
    all.iter()
        .filter_map(|s| {
            let dir = msgs.join(&s.id);
            dir.is_dir()
                .then(|| source(dir, s, root_of(&s.id, &parents)))
        })
        .collect()
}

#[cfg(feature = "sqlite")]
fn db_files(root: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(root)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| is_db_name(p) && p.is_file())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

#[cfg(feature = "sqlite")]
fn is_db_name(p: &Path) -> bool {
    p.file_name()
        .map(|n| n.to_string_lossy())
        .is_some_and(|n| n.starts_with("opencode") && n.ends_with(".db"))
}

#[cfg(feature = "sqlite")]
fn db_sources(db: &Path) -> Vec<SourceFile> {
    db::sessions(db)
        .unwrap_or_default()
        .into_iter()
        .map(|(s, root)| {
            let virt = PathBuf::from(format!("{}#{}", db.display(), s.id));
            source(virt, &s, root)
        })
        .collect()
}

#[cfg(not(feature = "sqlite"))]
fn db_sources(_db: &Path) -> Vec<SourceFile> {
    vec![]
}

impl Provider for Opencode {
    fn id(&self) -> ProviderId {
        ProviderId(ID.to_string())
    }

    fn label(&self) -> &'static str {
        "opencode"
    }

    fn caps(&self) -> Capabilities {
        Capabilities {
            turns: Level::Full,
            cache_read: Level::Full,
            cache_write: Level::Full,
            tool_outputs: Level::Partial,
            skills: Level::Partial,
            startup: Level::None,
            memory: Level::None,
            subagents: Level::Partial,
            compactions: Level::Partial,
            mcp: Level::None,
            cost: CostSource::Stored,
        }
    }

    fn default_roots(&self) -> Vec<PathBuf> {
        common::env_dirs("OPENCODE_DATA_DIR")
            .or_else(|| {
                std::env::var_os("XDG_DATA_HOME")
                    .filter(|v| !v.is_empty())
                    .map(|x| vec![PathBuf::from(x).join("opencode")])
            })
            .or_else(|| {
                common::home().map(|h| vec![h.join(".local").join("share").join("opencode")])
            })
            .unwrap_or_default()
            .into_iter()
            .filter(|r| r.is_dir())
            .collect()
    }

    fn discover(&self, root: &Path) -> io::Result<Vec<SourceFile>> {
        #[cfg_attr(not(feature = "sqlite"), allow(unused_mut))]
        let mut out = json_sources(root);
        #[cfg(feature = "sqlite")]
        for db in db_files(root) {
            out.extend(db_sources(&db));
        }
        Ok(out)
    }

    fn watch_dirs(&self, root: &Path) -> Vec<(PathBuf, bool)> {
        let mut v = vec![(root.to_path_buf(), false)];
        let m = root.join("storage").join("message");
        if m.is_dir() {
            v.push((m, true));
        }
        v
    }

    fn accepts(&self, root: &Path, path: &Path) -> bool {
        let n = path
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default();
        if path.parent() == Some(root) {
            return n.starts_with("opencode") && (n.ends_with(".db") || n.ends_with(".db-wal"));
        }
        path.starts_with(root.join("storage").join("message")) && n.ends_with(".json")
    }

    fn sources_of(&self, root: &Path, path: &Path) -> Vec<SourceFile> {
        if !self.accepts(root, path) {
            return vec![];
        }
        if path.parent() == Some(root) {
            let s = path.to_string_lossy();
            let db = PathBuf::from(s.strip_suffix("-wal").unwrap_or(&s));
            return if db.is_file() {
                db_sources(&db)
            } else {
                vec![]
            };
        }
        let Some(dir) = path.parent() else {
            return vec![];
        };
        json_sources(root)
            .into_iter()
            .filter(|s| s.path == dir)
            .collect()
    }

    fn fingerprint(&self, src: &SourceFile) -> io::Result<Fingerprint> {
        if src.path.is_dir() {
            let (mut size, mut mtime, mut count) = (0u64, 0u64, 0u64);
            for f in sorted_files(&src.path) {
                let m = std::fs::metadata(&f)?;
                size += m.len();
                count += 1;
                let t = m
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_nanos() as u64);
                mtime = mtime.max(t);
            }
            return Ok(Fingerprint {
                size,
                mtime_ns: mtime,
                head_len: 0,
                head_hash: count,
                change: Change::Whole,
            });
        }
        #[cfg(feature = "sqlite")]
        {
            db::fingerprint(&src.path)
        }
        #[cfg(not(feature = "sqlite"))]
        Err(io::ErrorKind::NotFound.into())
    }

    fn parse_file(
        &self,
        file: &SourceFile,
        _resume: Option<ParseResume>,
    ) -> io::Result<FileRecords> {
        if file.path.is_dir() {
            return Ok(parse_json(&file.path, &file.project_key));
        }
        #[cfg(feature = "sqlite")]
        {
            db::parse(&file.path, &file.project_key)
        }
        #[cfg(not(feature = "sqlite"))]
        Err(io::ErrorKind::NotFound.into())
    }

    fn vendors(&self) -> &'static [Vendor] {
        &[Vendor::Anthropic, Vendor::OpenAi, Vendor::Google]
    }

    fn public_names(&self) -> &'static [&'static str] {
        &[
            "bash",
            "read",
            "edit",
            "write",
            "patch",
            "multiedit",
            "grep",
            "glob",
            "list",
            "webfetch",
            "websearch",
            "task",
            "skill",
            "todowrite",
            "todoread",
        ]
    }
}

fn parse_json(dir: &Path, project: &str) -> FileRecords {
    let part_root = dir
        .parent()
        .and_then(Path::parent)
        .map(|s| s.join("part"))
        .unwrap_or_default();
    let mut msgs: Vec<(i64, String, Value)> = sorted_files(dir)
        .iter()
        .filter_map(|f| {
            let v = read_json(f)?;
            let id = v
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| f.file_stem().map(|s| s.to_string_lossy().into_owned()))?;
            let t = v
                .pointer("/time/created")
                .and_then(Value::as_i64)
                .unwrap_or(0);
            Some((t, id, v))
        })
        .collect();
    msgs.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    let mut s = parse::Session::default();
    for (_, id, m) in &msgs {
        let parts: Vec<Value> = sorted_files(&part_root.join(id))
            .iter()
            .filter_map(|p| read_json(p))
            .collect();
        let role = m.get("role").and_then(Value::as_str).unwrap_or("");
        s.message(id, role, m, &parts);
    }
    s.b.finish(project)
}

#[cfg(test)]
mod tests;
