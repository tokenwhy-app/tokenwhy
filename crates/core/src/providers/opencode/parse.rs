// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Fields from codeburn v0.9.27 (commit 7ecc87f) sqlite-session-parser.ts and opencode-file-parser.ts (MIT, AgentSeal).

use super::super::common::{self, Builder, Call};
use crate::model::{Category, Compaction, Usage};
use serde_json::Value;

#[derive(Default)]
pub struct Session {
    pub b: Builder,
    last_ctx: u64,
    open_compaction: Option<usize>,
    seen: std::collections::HashSet<String>,
}

fn n(v: &Value, ptr: &str) -> u64 {
    v.pointer(ptr).and_then(Value::as_u64).unwrap_or(0)
}

fn st<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(Value::as_str).filter(|s| !s.is_empty())
}

/// Any other `<server>_<tool>` name is an MCP tool (codeburn docs, Quirks).
const BUILTIN: &[&str] = &[
    "bash",
    "read",
    "edit",
    "write",
    "patch",
    "multiedit",
    "grep",
    "glob",
    "list",
    "ls",
    "webfetch",
    "websearch",
    "task",
    "skill",
    "todowrite",
    "todoread",
    "lsp_diagnostics",
    "lsp_hover",
    "invalid",
    "codesearch",
    "batch",
];

impl Session {
    pub fn message(&mut self, id: &str, role: &str, m: &Value, parts: &[Value]) -> bool {
        if !self.seen.insert(id.to_string()) {
            return false;
        }
        for p in parts {
            if p.get("type").and_then(Value::as_str) == Some("compaction") {
                self.compaction();
            }
        }
        if role == "user" {
            let o = self.b.next_order();
            self.b.rec.human_prompts.push(o);
            return true;
        }
        if role == "compaction" {
            self.compaction();
        }
        let t = m.get("tokens").cloned().unwrap_or(Value::Null);
        let usage = Usage {
            input: n(&t, "/input"),
            cache_write_5m: n(&t, "/cache/write"),
            cache_write_1h: 0,
            cache_read: n(&t, "/cache/read"),
            // opencode keeps reasoning apart from output; it is billed at the output rate.
            output: n(&t, "/output") + n(&t, "/reasoning"),
            web_search_requests: 0,
        };
        let tools: Vec<&Value> = parts
            .iter()
            .filter(|p| p.get("type").and_then(Value::as_str) == Some("tool"))
            .collect();
        let empty = usage.input + usage.output + usage.cache_read + usage.cache_write_5m == 0;
        if empty && tools.is_empty() {
            return true;
        }
        let model = st(m, "modelID")
            .or_else(|| m.pointer("/model/id").and_then(Value::as_str))
            .or_else(|| m.pointer("/model/modelID").and_then(Value::as_str))
            .unwrap_or("unknown")
            .to_string();
        let ts = m
            .pointer("/time/created")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let order = self.b.turn(common::fnv(id.as_bytes()), ts, &model, usage);
        // opencode writes 0 for subscription and unknown-price models; our tables price those.
        if let Some(c) = m.get("cost").and_then(Value::as_f64).filter(|c| *c > 0.0) {
            if let Some(t) = self.b.rec.turns.last_mut() {
                t.stored_cost = Some(c);
            }
        }
        if !empty {
            let ctx = usage.input + usage.cache_read + usage.cache_write_5m;
            if let Some(c) = self.open_compaction.take() {
                self.b.rec.compactions[c].post_tokens = ctx;
            }
            self.last_ctx = ctx;
        }
        for p in tools {
            self.tool(order, id, p);
        }
        true
    }

    fn compaction(&mut self) {
        let order = self.b.next_order();
        let trigger = self.b.intern("auto");
        self.b.rec.compactions.push(Compaction {
            order,
            trigger,
            pre_tokens: self.last_ctx,
            post_tokens: 0,
            summary_chars: 0,
        });
        self.open_compaction = Some(self.b.rec.compactions.len() - 1);
    }

    fn tool(&mut self, order: u32, msg: &str, p: &Value) {
        let name = st(p, "tool").or_else(|| st(p, "name")).unwrap_or("unknown");
        let state = p.get("state").cloned().unwrap_or(Value::Null);
        let input = state.get("input").cloned().unwrap_or(Value::Null);
        let output = state.get("output").and_then(Value::as_str);
        let chars = output.map_or(0, |o| o.chars().count() as u64);
        let fallback = format!("{msg}:{name}");
        let id = st(p, "callID").or_else(|| st(p, "id")).unwrap_or(&fallback);
        let path_of = |k: &[&str]| k.iter().find_map(|k| st(&input, k)).map(str::to_string);
        let mut c = Call {
            id,
            name,
            result_chars: chars,
            is_error: st(&state, "status") == Some("error"),
            ..Default::default()
        };
        let path;
        let mut bounded = false;
        let mut skill = None;
        match name {
            "bash" => {
                path = None;
                c.command = st(&input, "command");
            }
            "read" => {
                path = path_of(&["filePath", "path"]);
                bounded = input.get("limit").is_some() || input.get("offset").is_some();
                c.category = Some(Category::ReadFile);
                if let Some(s) = path.as_deref().and_then(common::skill_of) {
                    skill = Some(s.to_string());
                }
            }
            "edit" | "write" | "patch" | "multiedit" => {
                path = path_of(&["filePath", "path"]);
                c.writes = path.is_some();
                c.category = Some(Category::Edit);
            }
            "grep" | "codesearch" => {
                path = path_of(&["path"]);
                c.category = Some(Category::Search);
            }
            "glob" | "list" | "ls" => {
                path = None;
                c.category = Some(Category::List);
            }
            "webfetch" | "websearch" => {
                path = None;
                c.category = Some(Category::Web);
            }
            "task" => {
                path = None;
                c.category = Some(Category::Agent);
                c.detail = st(&input, "subagent_type").map(str::to_string);
            }
            "skill" => {
                path = None;
                c.category = Some(Category::Other);
                c.detail = st(&input, "name").map(str::to_string);
                skill = c.detail.clone();
            }
            n if !BUILTIN.contains(&n) && n.contains('_') => {
                path = None;
                c.category = Some(Category::Mcp);
                c.detail = n.split('_').next().map(str::to_string);
            }
            _ => path = None,
        }
        c.path = path.as_deref();
        let i = self.b.call(order, c);
        self.b.rec.tool_calls[i].bounded |= bounded;
        if let Some(s) = skill {
            self.b.skill(&s, chars);
        }
    }
}
