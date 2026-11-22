// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Ported from codeburn v0.9.27 `src/providers/codex.ts` (commit 7ecc87f, MIT, AgentSeal) and ccusage `rust/adapters/codex` (MIT, ryoppippi, read 22/10/2026).

use super::common::{self, Builder, Call};
use crate::model::{
    Capabilities, Category, Compaction, CostSource, FileRecords, FileRole, Level, ParseResume,
    ProviderId, SkipCount, SourceFile, Usage, Vendor,
};
use crate::provider::Provider;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};

pub const ID: &str = "codex";

pub struct Codex;

fn is_rollout(p: &Path) -> bool {
    p.file_name()
        .map(|n| n.to_string_lossy())
        .is_some_and(|n| n.starts_with("rollout-") && n.ends_with(".jsonl"))
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        match e.file_type() {
            Ok(t) if t.is_dir() => walk(&p, out),
            Ok(t) if t.is_file() && is_rollout(&p) => out.push(p),
            _ => {}
        }
    }
}

struct Meta {
    id: String,
    cwd: String,
    parent: Option<String>,
}

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k)
        .and_then(Value::as_str)
        .filter(|x| !x.is_empty())
        .map(str::to_string)
}

fn read_meta(path: &Path) -> Option<Meta> {
    let f = std::fs::File::open(path).ok()?;
    let mut line = String::new();
    BufReader::new(f).take_line(&mut line)?;
    let v: Value = serde_json::from_str(&line).ok()?;
    if v.get("type").and_then(Value::as_str) != Some("session_meta") {
        return None;
    }
    let p = v.get("payload").filter(|p| p.is_object())?;
    let stem = path.file_stem()?.to_string_lossy().into_owned();
    Some(Meta {
        id: s(p, "id").or_else(|| s(p, "session_id")).unwrap_or(stem),
        cwd: s(p, "cwd").unwrap_or_default(),
        parent: p
            .pointer("/source/subagent/thread_spawn/parent_thread_id")
            .and_then(Value::as_str)
            .filter(|x| !x.is_empty())
            .map(str::to_string),
    })
}

trait TakeLine {
    fn take_line(self, buf: &mut String) -> Option<()>;
}

impl<R: io::Read> TakeLine for BufReader<R> {
    /// Capped at 4 MiB: Codex 0.128+ embeds the system prompt in `session_meta`.
    fn take_line(self, buf: &mut String) -> Option<()> {
        let mut r = io::Read::take(self, 4 << 20);
        let mut b = BufReader::new(&mut r);
        b.read_line(buf).ok().filter(|n| *n > 0)?;
        Some(())
    }
}

fn source(path: &Path) -> Option<SourceFile> {
    let m = read_meta(path)?;
    let project = common::base_name(&m.cwd);
    Some(SourceFile {
        provider: ProviderId(ID.to_string()),
        path: path.to_path_buf(),
        role: if m.parent.is_some() {
            FileRole::Agent
        } else {
            FileRole::Main
        },
        project_key: if project.is_empty() {
            "codex".into()
        } else {
            project
        },
        session_id: m.parent.unwrap_or(m.id),
        agent_type: None,
        agent_group: None,
    })
}

impl Provider for Codex {
    fn id(&self) -> ProviderId {
        ProviderId(ID.to_string())
    }

    fn label(&self) -> &'static str {
        "Codex"
    }

    fn caps(&self) -> Capabilities {
        Capabilities {
            turns: Level::Full,
            cache_read: Level::Full,
            cache_write: Level::None,
            tool_outputs: Level::Partial,
            skills: Level::Partial,
            startup: Level::None,
            memory: Level::None,
            subagents: Level::Partial,
            compactions: Level::Partial,
            mcp: Level::None,
            cost: CostSource::Table,
        }
    }

    fn default_roots(&self) -> Vec<PathBuf> {
        common::env_dirs("CODEX_HOME")
            .or_else(|| common::home().map(|h| vec![h.join(".codex")]))
            .unwrap_or_default()
            .into_iter()
            .filter(|r| r.join("sessions").is_dir() || r.join("archived_sessions").is_dir())
            .collect()
    }

    fn discover(&self, root: &Path) -> io::Result<Vec<SourceFile>> {
        let mut active = vec![];
        walk(&root.join("sessions"), &mut active);
        active.sort();
        let names: HashSet<_> = active
            .iter()
            .filter_map(|p| p.file_name().map(|n| n.to_owned()))
            .collect();
        let mut archived = vec![];
        if let Ok(rd) = std::fs::read_dir(root.join("archived_sessions")) {
            for e in rd.flatten() {
                let p = e.path();
                if is_rollout(&p)
                    && p.is_file()
                    && !p.file_name().is_some_and(|n| names.contains(n))
                {
                    archived.push(p);
                }
            }
        }
        archived.sort();
        Ok(active
            .iter()
            .chain(&archived)
            .filter_map(|p| source(p))
            .collect())
    }

    fn watch_dirs(&self, root: &Path) -> Vec<(PathBuf, bool)> {
        ["sessions", "archived_sessions"]
            .iter()
            .map(|d| root.join(d))
            .filter(|d| d.is_dir())
            .map(|d| (d, true))
            .collect()
    }

    fn accepts(&self, root: &Path, path: &Path) -> bool {
        is_rollout(path)
            && (path.starts_with(root.join("sessions"))
                || path.starts_with(root.join("archived_sessions")))
    }

    fn sources_of(&self, root: &Path, path: &Path) -> Vec<SourceFile> {
        if !self.accepts(root, path) || !path.is_file() {
            return vec![];
        }
        if path.starts_with(root.join("archived_sessions")) {
            let mut active = vec![];
            walk(&root.join("sessions"), &mut active);
            if active.iter().any(|a| a.file_name() == path.file_name()) {
                return vec![];
            }
        }
        source(path).into_iter().collect()
    }

    fn parse_file(
        &self,
        file: &SourceFile,
        _resume: Option<ParseResume>,
    ) -> io::Result<FileRecords> {
        parse(file)
    }

    fn vendors(&self) -> &'static [Vendor] {
        &[Vendor::OpenAi]
    }

    fn public_names(&self) -> &'static [&'static str] {
        &[
            "exec_command",
            "exec",
            "shell",
            "apply_patch",
            "read_file",
            "write_file",
            "apply_diff",
            "read_dir",
            "spawn_agent",
            "wait_agent",
            "close_agent",
            "update_plan",
            "web_search",
            "view_image",
        ]
    }

    fn skipped(&self, root: &Path) -> Vec<SkipCount> {
        fn count(dir: &Path) -> u32 {
            let Ok(rd) = std::fs::read_dir(dir) else {
                return 0;
            };
            rd.flatten()
                .map(|e| {
                    let p = e.path();
                    if p.is_dir() {
                        count(&p)
                    } else {
                        u32::from(p.to_string_lossy().ends_with(".jsonl.zst"))
                    }
                })
                .sum()
        }
        let n = count(&root.join("sessions")) + count(&root.join("archived_sessions"));
        if n == 0 {
            vec![]
        } else {
            vec![SkipCount {
                reason: "compressed .jsonl.zst rollouts".into(),
                count: n,
            }]
        }
    }
}

#[derive(Default, Clone, Copy, PartialEq)]
struct Tok {
    input: u64,
    cached: u64,
    output: u64,
    reasoning: u64,
    total: Option<u64>,
}

fn tok(v: Option<&Value>) -> Option<Tok> {
    let v = v.filter(|v| v.is_object())?;
    let n = |k: &str| v.get(k).and_then(Value::as_u64).unwrap_or(0);
    Some(Tok {
        input: n("input_tokens"),
        cached: n("cached_input_tokens"),
        output: n("output_tokens"),
        reasoning: n("reasoning_output_tokens"),
        total: v.get("total_tokens").and_then(Value::as_u64),
    })
}

fn command_of(args: &Value) -> Option<String> {
    match args.get("cmd").or_else(|| args.get("command"))? {
        Value::String(s) => Some(s.clone()),
        Value::Array(a) => {
            let parts: Vec<&str> = a.iter().filter_map(Value::as_str).collect();
            match parts.as_slice() {
                [sh, flag, script] if flag.starts_with('-') && sh.contains("sh") => {
                    Some(script.to_string())
                }
                _ => Some(parts.join(" ")),
            }
        }
        _ => None,
    }
}

fn patch_path(input: &str) -> Option<String> {
    input.lines().find_map(|l| {
        ["*** Update File: ", "*** Add File: ", "*** Delete File: "]
            .iter()
            .find_map(|p| l.strip_prefix(p))
            .map(|p| p.trim().to_string())
    })
}

struct Pending {
    id: String,
    name: String,
    category: Option<Category>,
    command: Option<String>,
    path: Option<String>,
    writes: bool,
    detail: Option<String>,
}

#[derive(Default)]
struct State {
    b: Builder,
    model: Option<String>,
    seen: HashSet<String>,
    pending: Vec<Pending>,
    calls: HashMap<String, usize>,
    skill_of_call: HashMap<String, usize>,
    early_output: HashMap<String, u64>,
    last_turn: Option<u32>,
    last_ctx: u64,
    open_compaction: Option<usize>,
}

impl State {
    fn emit(&mut self, order: u32, c: Pending) {
        let skill = c
            .path
            .as_deref()
            .and_then(common::skill_of)
            .map(str::to_string);
        let i = self.b.call(
            order,
            Call {
                id: &c.id,
                name: &c.name,
                category: c.category,
                command: c.command.as_deref(),
                path: c.path.as_deref(),
                writes: c.writes,
                detail: c.detail,
                ..Default::default()
            },
        );
        let out = self.early_output.remove(&c.id);
        if let Some(chars) = out {
            self.b.rec.tool_calls[i].result_chars = chars;
        }
        if let Some(sk) = skill {
            self.b.skill(&sk, out.unwrap_or(0));
            self.skill_of_call
                .insert(c.id.clone(), self.b.rec.skill_loads.len() - 1);
        }
        self.calls.insert(c.id, i);
    }

    fn account(&mut self, u: Tok, key: String, ts: i64) {
        if (u.input | u.cached | u.output | u.reasoning) == 0 || !self.seen.insert(key.clone()) {
            return;
        }
        let cached = u.cached.min(u.input);
        let usage = Usage {
            input: u.input - cached,
            cache_read: cached,
            output: u.output,
            ..Default::default()
        };
        let m = self.model.clone().unwrap_or_else(|| "gpt-5".into());
        let order = self.b.turn(common::fnv(key.as_bytes()), ts, &m, usage);
        self.last_ctx = u.input;
        if let Some(c) = self.open_compaction.take() {
            self.b.rec.compactions[c].post_tokens = u.input;
        }
        for c in std::mem::take(&mut self.pending) {
            self.emit(order, c);
        }
        self.last_turn = Some(order);
    }
}

pub(crate) fn parse(file: &SourceFile) -> io::Result<FileRecords> {
    let f = std::fs::File::open(&file.path)?;
    let mut st = State::default();
    let mut session = file.session_id.clone();
    let mut root_id = session.clone();
    let mut prev_total = Tok::default();
    let mut prev_identity: Option<String> = None;
    let mut prev_cumulative: Option<u64> = None;
    let mut has_record = false;
    let mut done: u64 = 0;

    let mut reader = BufReader::new(f);
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 || !line.ends_with('\n') {
            break;
        }
        done += n as u64;
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let ty = v.get("type").and_then(Value::as_str).unwrap_or("");
        let p = v.get("payload").cloned().unwrap_or(Value::Null);
        let pty = p.get("type").and_then(Value::as_str).unwrap_or("");
        let ts = v
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
            .map_or(0, |d| d.timestamp_millis());

        match (ty, pty) {
            ("session_meta", _) => {
                session = s(&p, "id")
                    .or_else(|| s(&p, "session_id"))
                    .unwrap_or(session);
                root_id = s(&p, "forked_from_id")
                    .or_else(|| s(&p, "parent_thread_id"))
                    .or_else(|| {
                        p.pointer("/source/subagent/thread_spawn/parent_thread_id")
                            .and_then(Value::as_str)
                            .filter(|x| !x.is_empty())
                            .map(str::to_string)
                    })
                    .unwrap_or_else(|| session.clone());
                if let Some(m) = s(&p, "model") {
                    st.model = Some(m);
                }
                prev_identity = None;
            }
            ("turn_context", _) => {
                if let Some(m) = s(&p, "model") {
                    st.model = Some(m);
                }
            }
            ("token_usage_record", _) => {
                let Some(u) = tok(p.get("usage")) else {
                    continue;
                };
                has_record = true;
                let key = match s(&p, "response_id") {
                    Some(r) => format!("codex:{root_id}:response:{r}"),
                    None => format!("codex:usage-record:{}:{done}", file.path.display()),
                };
                st.account(u, key, ts);
            }
            ("event_msg", "token_count") if !has_record => {
                let Some(info) = p.get("info").filter(|i| i.is_object()) else {
                    continue;
                };
                let identity = info.to_string();
                if prev_identity.as_deref() == Some(identity.as_str()) {
                    continue;
                }
                prev_identity = Some(identity);
                let total = tok(info.get("total_token_usage"));
                let cumulative = total.and_then(|t| t.total);
                if cumulative.is_some() && cumulative == prev_cumulative {
                    continue;
                }
                prev_cumulative = cumulative;
                let u = match (tok(info.get("last_token_usage")), total) {
                    (Some(l), _) => Some(l),
                    (None, Some(t)) if cumulative.unwrap_or(0) > 0 => Some(Tok {
                        input: t.input.saturating_sub(prev_total.input),
                        cached: t.cached.saturating_sub(prev_total.cached),
                        output: t.output.saturating_sub(prev_total.output),
                        reasoning: t.reasoning.saturating_sub(prev_total.reasoning),
                        total: None,
                    }),
                    _ => None,
                };
                if let Some(t) = total {
                    prev_total = t;
                }
                let Some(u) = u else { continue };
                let key = match (cumulative, total) {
                    (Some(c), Some(t)) => format!(
                        "codex:{root_id}:{c}:{}:{}:{}:{}",
                        t.input, t.cached, t.output, t.reasoning
                    ),
                    _ => format!("codex:record:{}:{done}", file.path.display()),
                };
                st.account(u, key, ts);
            }
            ("event_msg", "user_message") => {
                let o = st.b.next_order();
                st.b.rec.human_prompts.push(o);
            }
            ("compacted", _) | ("event_msg", "context_compacted") => {
                let order = st.b.next_order();
                let trigger = st.b.intern("auto");
                st.b.rec.compactions.push(Compaction {
                    order,
                    trigger,
                    pre_tokens: st.last_ctx,
                    post_tokens: 0,
                    summary_chars: s(&p, "message").map_or(0, |m| m.chars().count() as u64),
                });
                st.open_compaction = Some(st.b.rec.compactions.len() - 1);
            }
            ("response_item", "function_call" | "custom_tool_call" | "local_shell_call") => {
                st.pending.push(pending_call(&p, pty, &session, done));
            }
            ("response_item", "function_call_output" | "custom_tool_call_output") => {
                let Some(id) = s(&p, "call_id") else { continue };
                let chars = match p.get("output") {
                    Some(Value::String(o)) => o.chars().count() as u64,
                    Some(o @ Value::Object(_)) => o
                        .get("output")
                        .or_else(|| o.get("content"))
                        .and_then(Value::as_str)
                        .map_or(0, |x| x.chars().count() as u64),
                    _ => 0,
                };
                match st.calls.get(&id) {
                    Some(&i) => {
                        st.b.rec.tool_calls[i].result_chars = chars;
                        if let Some(&k) = st.skill_of_call.get(&id) {
                            st.b.rec.skill_loads[k].body_chars = chars;
                        }
                    }
                    None => {
                        st.early_output.insert(id, chars);
                    }
                }
            }
            _ => {}
        }
    }
    if let Some(order) = st.last_turn {
        for c in std::mem::take(&mut st.pending) {
            st.emit(order, c);
        }
    }
    let mut rec = st.b.finish(&file.project_key);
    rec.offset_done = done;
    Ok(rec)
}

fn pending_call(p: &Value, pty: &str, session: &str, at: u64) -> Pending {
    let name = s(p, "name").unwrap_or_else(|| {
        if pty == "local_shell_call" {
            "shell"
        } else {
            "unknown"
        }
        .to_string()
    });
    let id = s(p, "call_id")
        .or_else(|| s(p, "id"))
        .unwrap_or_else(|| format!("{session}:{at}"));
    let args = match p.get("arguments") {
        Some(Value::String(a)) => serde_json::from_str(a).unwrap_or(Value::Null),
        Some(o) => o.clone(),
        None => p.get("action").cloned().unwrap_or(Value::Null),
    };
    let input = s(p, "input");
    let mut c = Pending {
        id,
        name: name.clone(),
        category: None,
        command: None,
        path: None,
        writes: false,
        detail: None,
    };
    match name.as_str() {
        "exec_command" | "exec" | "shell" | "container.exec" => {
            c.command = command_of(&args);
            if c.command.is_none() {
                c.category = Some(Category::Script);
            }
        }
        "apply_patch" | "apply_diff" | "write_file" => {
            c.category = Some(Category::Edit);
            c.path = s(&args, "file_path")
                .or_else(|| s(&args, "path"))
                .or_else(|| input.as_deref().and_then(patch_path))
                .or_else(|| s(&args, "input").as_deref().and_then(patch_path));
            c.writes = c.path.is_some();
        }
        "read_file" => {
            c.category = Some(Category::ReadFile);
            c.path = s(&args, "file_path").or_else(|| s(&args, "path"));
        }
        "read_dir" | "list_dir" => c.category = Some(Category::List),
        "spawn_agent" | "wait_agent" | "close_agent" => c.category = Some(Category::Agent),
        "web_search" => c.category = Some(Category::Web),
        n if n.starts_with("mcp__") => {
            c.category = Some(Category::Mcp);
            c.detail = n["mcp__".len()..].split("__").next().map(str::to_string);
        }
        _ => {}
    }
    c
}

#[cfg(test)]
mod tests;
