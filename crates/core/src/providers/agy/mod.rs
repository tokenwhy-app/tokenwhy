// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Field numbers follow codeburn v0.9.27 `src/providers/antigravity.ts` (MIT, AgentSeal), checked on real databases on 23/10/2026.

pub(crate) mod pb;

use super::common::{self, Builder, Call};
use crate::model::{
    Capabilities, Category, CostSource, FileRecords, FileRole, Level, ParseResume, ProviderId,
    SkipCount, SourceFile, TimestampMs, Usage, Vendor,
};
use crate::provider::{Change, Fingerprint, Provider};
use crate::sqlite;
use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};

pub const ID: &str = "agy";

pub struct Antigravity;

fn conv_dir(root: &Path) -> PathBuf {
    let c = root.join("conversations");
    if c.is_dir() {
        c
    } else {
        root.to_path_buf()
    }
}

fn app_dir(root: &Path) -> PathBuf {
    let c = conv_dir(root);
    if c.file_name().is_some_and(|n| n == "conversations") {
        c.parent().map_or(c.clone(), Path::to_path_buf)
    } else {
        c
    }
}

fn is_db(p: &Path) -> bool {
    p.extension().is_some_and(|e| e == "db")
}

#[derive(Default, Clone)]
struct Summary {
    project: String,
    parent: Option<String>,
    agent: Option<String>,
}

fn summaries(app: &Path) -> HashMap<String, Summary> {
    let db = app.join("conversation_summaries.db");
    let mut out = HashMap::new();
    if !db.is_file() {
        return out;
    }
    let Ok(copy) = sqlite::open_copy(&db) else {
        return out;
    };
    let c = copy.conn();
    if !sqlite::has_table(c, "conversation_summaries") {
        return out;
    }
    let cols: HashSet<String> = c
        .prepare("PRAGMA table_info(conversation_summaries)")
        .and_then(|mut s| {
            s.query_map([], |r| r.get::<_, String>(1))
                .map(|rows| rows.flatten().collect())
        })
        .unwrap_or_default();
    let col = |n: &str| {
        if cols.contains(n) {
            format!("`{n}`")
        } else {
            "''".to_string()
        }
    };
    let q = format!(
        "SELECT conversation_id, {}, {}, {} FROM conversation_summaries",
        col("workspace_uris"),
        col("parent_conversation_id"),
        col("agent_name")
    );
    let Ok(mut st) = c.prepare(&q) else {
        return out;
    };
    let rows = st.query_map([], |r| {
        Ok((
            r.get::<_, Option<String>>(0)?.unwrap_or_default(),
            r.get::<_, Option<String>>(1)?.unwrap_or_default(),
            r.get::<_, Option<String>>(2)?.unwrap_or_default(),
            r.get::<_, Option<String>>(3)?.unwrap_or_default(),
        ))
    });
    if let Ok(rows) = rows {
        for (id, ws, parent, agent) in rows.flatten() {
            let first = serde_json::from_str::<Vec<String>>(&ws)
                .ok()
                .and_then(|v| v.into_iter().next())
                .unwrap_or(ws);
            out.insert(
                id,
                Summary {
                    project: common::base_name(&first),
                    parent: Some(parent).filter(|p| !p.is_empty()),
                    agent: Some(agent).filter(|a| !a.is_empty()),
                },
            );
        }
    }
    out
}

fn source(path: &Path, s: Option<&Summary>) -> Option<SourceFile> {
    let id = path.file_stem()?.to_string_lossy().into_owned();
    let s = s.cloned().unwrap_or_default();
    let project = if s.project.is_empty() {
        "antigravity".to_string()
    } else {
        s.project
    };
    Some(SourceFile {
        provider: ProviderId(ID.to_string()),
        path: path.to_path_buf(),
        role: if s.parent.is_some() {
            FileRole::Agent
        } else {
            FileRole::Main
        },
        project_key: project,
        session_id: s.parent.unwrap_or(id),
        agent_type: s.agent,
        agent_group: None,
    })
}

impl Provider for Antigravity {
    fn id(&self) -> ProviderId {
        ProviderId(ID.to_string())
    }

    fn label(&self) -> &'static str {
        "Antigravity"
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
            compactions: Level::None,
            mcp: Level::None,
            cost: CostSource::Estimate,
        }
    }

    fn default_roots(&self) -> Vec<PathBuf> {
        let cands = common::env_dirs("ANTIGRAVITY_DATA_DIR").unwrap_or_else(|| {
            let Some(h) = common::home() else {
                return vec![];
            };
            let g = h.join(".gemini");
            let mut v: Vec<PathBuf> = [
                "antigravity",
                "antigravity-cli",
                "antigravity-ide",
                "antigravity-backup",
                "antigravity-acp",
            ]
            .iter()
            .map(|n| g.join(n))
            .collect();
            v.push(h.join(".config").join("antigravity"));
            v
        });
        cands
            .into_iter()
            .filter(|r| {
                r.join("conversations").is_dir()
                    || r.file_name().is_some_and(|n| n == "conversations") && r.is_dir()
            })
            .collect()
    }

    fn discover(&self, root: &Path) -> io::Result<Vec<SourceFile>> {
        let dir = conv_dir(root);
        let mut dbs: Vec<PathBuf> = std::fs::read_dir(&dir)?
            .flatten()
            .map(|e| e.path())
            .filter(|p| is_db(p) && p.is_file())
            .collect();
        dbs.sort();
        if dbs.is_empty() {
            return Ok(vec![]);
        }
        let sums = summaries(&app_dir(root));
        Ok(dbs
            .iter()
            .filter_map(|p| {
                source(
                    p,
                    p.file_stem().and_then(|s| sums.get(&*s.to_string_lossy())),
                )
            })
            .collect())
    }

    fn watch_dirs(&self, root: &Path) -> Vec<(PathBuf, bool)> {
        vec![(conv_dir(root), false)]
    }

    fn accepts(&self, root: &Path, path: &Path) -> bool {
        let n = path
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default();
        path.parent() == Some(conv_dir(root).as_path())
            && (n.ends_with(".db") || n.ends_with(".db-wal"))
    }

    fn sources_of(&self, root: &Path, path: &Path) -> Vec<SourceFile> {
        if !self.accepts(root, path) {
            return vec![];
        }
        let s = path.to_string_lossy();
        let db = PathBuf::from(s.strip_suffix("-wal").unwrap_or(&s));
        if !db.is_file() {
            return vec![];
        }
        let sums = summaries(&app_dir(root));
        source(
            &db,
            db.file_stem().and_then(|s| sums.get(&*s.to_string_lossy())),
        )
        .into_iter()
        .collect()
    }

    fn fingerprint(&self, src: &SourceFile) -> io::Result<Fingerprint> {
        let (size, mtime_ns) = sqlite::stat_with_wal(&src.path)?;
        Ok(Fingerprint {
            size,
            mtime_ns,
            head_len: 0,
            head_hash: 0,
            change: Change::Whole,
        })
    }

    fn parse_file(
        &self,
        file: &SourceFile,
        _resume: Option<ParseResume>,
    ) -> io::Result<FileRecords> {
        parse(file)
    }

    fn vendors(&self) -> &'static [Vendor] {
        &[Vendor::Anthropic, Vendor::Google]
    }

    fn public_names(&self) -> &'static [&'static str] {
        &[
            "view_file",
            "run_command",
            "replace_file_content",
            "multi_replace_file_content",
            "write_to_file",
            "manage_task",
            "list_dir",
            "grep_search",
            "find_by_name",
            "schedule",
            "search_web",
            "manage_subagents",
            "invoke_subagent",
            "read_url_content",
            "define_subagent",
            "call_mcp_tool",
        ]
    }

    fn skipped(&self, root: &Path) -> Vec<SkipCount> {
        let n = std::fs::read_dir(conv_dir(root))
            .map(|rd| {
                rd.flatten()
                    .filter(|e| e.path().extension().is_some_and(|x| x == "pb"))
                    .count()
            })
            .unwrap_or(0);
        if n == 0 {
            vec![]
        } else {
            vec![SkipCount {
                reason: "encrypted .pb conversations (Antigravity IDE)".into(),
                count: n as u32,
            }]
        }
    }
}

/// Model ids the app writes only as placeholders, from codeburn's PLACEHOLDER_MODELS (taken there from the app's
/// GetAvailableModels catalog, 6 Oct 2026). A later app build can repoint them.
const PLACEHOLDERS: &[(&str, &str)] = &[
    ("MODEL_PLACEHOLDER_M16", "gemini-3.1-pro-high"),
    ("MODEL_PLACEHOLDER_M37", "gemini-3.1-pro-high"),
    ("MODEL_PLACEHOLDER_M36", "gemini-3.1-pro-low"),
    ("MODEL_PLACEHOLDER_M84", "gemini-3.5-flash-high"),
    ("MODEL_PLACEHOLDER_M18", "gemini-3-flash"),
    ("MODEL_PLACEHOLDER_M318", "gemini-3.8-flash-high"),
    ("MODEL_PLACEHOLDER_M319", "gemini-3.8-flash-medium"),
    ("MODEL_PLACEHOLDER_M320", "gemini-3.8-flash-low"),
    ("MODEL_PLACEHOLDER_M35", "claude-sonnet-4-6"),
    ("MODEL_PLACEHOLDER_M26", "claude-opus-4-6-thinking"),
];

fn model_of(chat: &[(u32, pb::Val<'_>)]) -> String {
    let mut model_enum = None;
    for (n, v) in chat {
        if let (20, pb::Val::Bytes(b)) = (n, v) {
            let kv = pb::fields(b);
            if pb::text(&kv, 1) == Some("model_enum") {
                model_enum = pb::text(&kv, 2);
            }
        }
    }
    let display = pb::text(chat, 21);
    if display.is_none() {
        if let Some((_, m)) = PLACEHOLDERS.iter().find(|(k, _)| Some(*k) == model_enum) {
            return m.to_string();
        }
    }
    pb::text(chat, 19)
        .or(model_enum)
        .or(display)
        .unwrap_or("unknown")
        .to_string()
}

fn str_arg<'a>(a: &'a serde_json::Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|k| a.get(*k).and_then(|v| v.as_str()))
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn step_tools(b: &mut Builder, cascade: &str, step: i64, md: &[u8], turn: u32) {
    for (n, v) in pb::fields(md) {
        let (4, pb::Val::Bytes(tb)) = (n, v) else {
            continue;
        };
        let tf = pb::fields(tb);
        let Some(name) = pb::text(&tf, 2) else {
            continue;
        };
        if name == "send_message" {
            continue;
        }
        let fallback_id = format!("{cascade}:{step}:{name}");
        let id = pb::text(&tf, 1).unwrap_or(&fallback_id);
        let args = pb::text(&tf, 3)
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
            .unwrap_or(serde_json::Value::Null);
        let path_arg = |keys: &[&str]| str_arg(&args, keys).map(str::to_string);
        let mut c = Call {
            id,
            name,
            ..Default::default()
        };
        let path;
        match name {
            "view_file" => {
                path = path_arg(&["AbsolutePath", "file_path"]);
                c.category = Some(Category::ReadFile);
                if let Some(s) = path.as_deref().and_then(common::skill_of) {
                    let s = s.to_string();
                    b.skill(&s, 0);
                }
            }
            "grep_search" => {
                path = path_arg(&["SearchPath", "SearchDirectory"]);
                c.category = Some(Category::Search);
            }
            "list_dir" | "find_by_name" => {
                path = None;
                c.category = Some(Category::List);
            }
            "run_command" => {
                path = None;
                c.command = str_arg(&args, &["CommandLine"]);
            }
            "replace_file_content" | "multi_replace_file_content" | "write_to_file" => {
                path = path_arg(&["TargetFile", "AbsolutePath", "file_path"]);
                c.writes = path.is_some();
                c.category = Some(Category::Edit);
            }
            "search_web" | "read_url_content" => {
                path = None;
                c.category = Some(Category::Web);
            }
            "invoke_subagent" => {
                path = None;
                c.category = Some(Category::Agent);
                c.detail = args
                    .get("Subagents")
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.first())
                    .and_then(|s| str_arg(s, &["TypeName", "Role"]))
                    .map(str::to_string);
            }
            "call_mcp_tool" => {
                path = None;
                c.category = Some(Category::Mcp);
                c.detail = str_arg(
                    &args,
                    &["ServerName", "server_name", "serverName", "server"],
                )
                .map(str::to_string);
            }
            n if n.starts_with("mcp_") => {
                path = None;
                c.category = Some(Category::Mcp);
                c.detail = n
                    .trim_start_matches("mcp__")
                    .trim_start_matches("mcp_")
                    .split('_')
                    .next()
                    .map(str::to_string);
            }
            _ => path = None,
        }
        c.path = path.as_deref();
        b.call(turn, c);
    }
}

pub(crate) fn parse(file: &SourceFile) -> io::Result<FileRecords> {
    let cascade = file
        .path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let copy = sqlite::open_copy(&file.path)?;
    let c = copy.conn();
    let fallback_ts: TimestampMs = common::mtime_ms(&file.path);
    let blob = |r: &rusqlite::Row<'_>, i: usize| -> rusqlite::Result<Vec<u8>> {
        Ok(r.get_ref(i)?
            .as_bytes_or_null()?
            .unwrap_or_default()
            .to_vec())
    };
    let mut steps: HashMap<i64, Vec<u8>> = HashMap::new();
    if sqlite::has_table(c, "steps") {
        let mut st = c
            .prepare("SELECT idx, metadata FROM steps")
            .map_err(io::Error::other)?;
        let rows = st
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, blob(r, 1)?)))
            .map_err(io::Error::other)?;
        steps.extend(rows.flatten());
    }
    let mut gens: Vec<(i64, Vec<u8>)> = vec![];
    if sqlite::has_table(c, "gen_metadata") {
        let mut st = c
            .prepare("SELECT idx, data FROM gen_metadata ORDER BY idx")
            .map_err(io::Error::other)?;
        let rows = st
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, blob(r, 1)?)))
            .map_err(io::Error::other)?;
        gens.extend(rows.flatten());
    }
    drop(copy);

    let mut b = Builder::default();
    let mut seen: HashSet<String> = HashSet::new();
    let mut used_steps: HashSet<i64> = HashSet::new();
    for (idx, data) in &gens {
        let root = pb::fields(data);
        let chat = pb::fields(pb::bytes(&root, 1).unwrap_or_default());
        let Some(ub) = pb::bytes(&chat, 4) else {
            continue;
        };
        let u = pb::fields(ub);
        let input = pb::uint(&u, 2);
        let thinking = pb::uint(&u, 9);
        let response = pb::uint(&u, 10);
        // Field 3 is thinking + response; Google bills thinking at the output rate.
        let output = match pb::uint(&u, 3) {
            0 => thinking.saturating_add(response),
            o => o,
        };
        if input == 0 && output == 0 {
            continue;
        }
        let rid = pb::text(&u, 11)
            .map(str::to_string)
            .unwrap_or_else(|| format!("idx{idx}"));
        if !seen.insert(rid.clone()) {
            continue;
        }
        let step_idx: Vec<i64> = pb::packed(&root, 2).into_iter().map(|x| x as i64).collect();
        let ts = pb::bytes(&chat, 9)
            .and_then(|m| pb::time_ms(&pb::fields(m), 4))
            .or_else(|| {
                step_idx
                    .first()
                    .and_then(|s| steps.get(s))
                    .and_then(|md| pb::time_ms(&pb::fields(md), 1))
            })
            .unwrap_or(fallback_ts);
        let usage = Usage {
            input,
            cache_write_5m: pb::uint(&u, 4),
            cache_write_1h: 0,
            cache_read: pb::uint(&u, 5),
            output,
            web_search_requests: 0,
        };
        let msg_id = common::fnv(format!("{cascade}:{rid}").as_bytes());
        let turn = b.turn(msg_id, ts, &model_of(&chat), usage);
        for s in step_idx {
            if !used_steps.insert(s) {
                continue;
            }
            if let Some(md) = steps.get(&s) {
                step_tools(&mut b, &cascade, s, md, turn);
            }
        }
    }
    Ok(b.finish(&file.project_key))
}

#[cfg(test)]
mod tests;
