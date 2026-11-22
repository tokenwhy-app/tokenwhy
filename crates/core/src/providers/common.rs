// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use crate::classify::{self, is_log_path};
use crate::model::{Category, FileRecords, IdHash, SkillLoad, TimestampMs, ToolCall, Turn, Usage};
use std::collections::HashMap;

pub fn fnv(bytes: &[u8]) -> IdHash {
    crate::corpus::fnv1a(bytes)
}

pub fn path_key(p: &str) -> IdHash {
    fnv(p.replace('\\', "/").to_lowercase().as_bytes())
}

#[derive(Default)]
pub struct Builder {
    pub rec: FileRecords,
    idx: HashMap<String, u32>,
    order: u32,
}

#[derive(Default)]
pub struct Call<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub category: Option<Category>,
    pub command: Option<&'a str>,
    pub path: Option<&'a str>,
    pub writes: bool,
    pub detail: Option<String>,
    pub result_chars: u64,
    pub is_error: bool,
}

impl Builder {
    pub fn intern(&mut self, s: &str) -> u32 {
        if let Some(i) = self.idx.get(s) {
            return *i;
        }
        let i = self.rec.strings.len() as u32;
        self.rec.strings.push(s.to_string());
        self.idx.insert(s.to_string(), i);
        i
    }

    pub fn next_order(&mut self) -> u32 {
        self.order += 1;
        self.order
    }

    pub fn turn(&mut self, msg_id: IdHash, ts: TimestampMs, model: &str, usage: Usage) -> u32 {
        let order = self.next_order();
        let model = self.intern(model);
        let segment = self.rec.compactions.len() as u32;
        self.rec.turns.push(Turn {
            msg_id,
            first_ts_ms: ts,
            model,
            usage,
            attribution_skill: None,
            segment,
            order,
            stored_cost: None,
        });
        order
    }

    pub fn call(&mut self, turn_order: u32, c: Call<'_>) -> usize {
        let mut bounded = false;
        let mut path = c.path.map(str::to_string);
        let mut detail = c.detail;
        let category = match (c.category, c.command) {
            (Some(Category::ReadFile), _) if path.as_deref().is_some_and(is_log_path) => {
                Category::Logs
            }
            (Some(cat), _) => cat,
            (None, Some(cmd)) => {
                let k = classify::classify_command(cmd, &[]);
                bounded = k.bounded;
                path = path.or(k.path);
                detail = detail.or(k.detail);
                k.category
            }
            (None, None) => {
                detail = detail.or_else(|| Some(c.name.chars().take(20).collect()));
                Category::Other
            }
        };
        let tool = self.intern(c.name);
        let detail = detail.map(|d| self.intern(&d));
        let path = path.map(|p| (path_key(&p), self.intern(&p)));
        self.rec.tool_calls.push(ToolCall {
            call_id: fnv(c.id.as_bytes()),
            turn_order,
            tool,
            category,
            detail,
            bounded,
            path,
            writes_path: c.writes,
            result_chars: c.result_chars,
            is_error: c.is_error,
            persisted: false,
            images: 0,
        });
        self.rec.tool_calls.len() - 1
    }

    pub fn skill(&mut self, name: &str, body_chars: u64) {
        let order = self.next_order();
        let skill = self.intern(name);
        self.rec.skill_loads.push(SkillLoad {
            skill,
            body_chars,
            order,
        });
    }

    pub fn finish(mut self, project: &str) -> FileRecords {
        if self.rec.project_name.is_none() && !project.is_empty() {
            let i = self.intern(project);
            self.rec.project_name = Some(i);
        }
        self.rec
    }
}

pub fn skill_of(path: &str) -> Option<&str> {
    let mut parts = path.rsplit(['/', '\\']);
    let file = parts.next()?;
    if !file.eq_ignore_ascii_case("SKILL.md") {
        return None;
    }
    parts.next().filter(|s| !s.is_empty())
}

pub fn base_name(p: &str) -> String {
    let p = p.strip_prefix("file://").unwrap_or(p);
    let p = p.trim_end_matches(['/', '\\']);
    let b = p.rsplit(['/', '\\']).next().unwrap_or("");
    percent_decode(b)
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Some(v) = std::str::from_utf8(&b[i + 1..i + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
            {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn home() -> Option<std::path::PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .filter(|h| !h.is_empty())
        .map(std::path::PathBuf::from)
}

pub fn env_dirs(var: &str) -> Option<Vec<std::path::PathBuf>> {
    let v = std::env::var_os(var)?;
    let dirs: Vec<_> = v
        .to_string_lossy()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(std::path::PathBuf::from)
        .collect();
    (!dirs.is_empty()).then_some(dirs)
}

#[cfg(feature = "sqlite")]
pub fn mtime_ms(p: &std::path::Path) -> TimestampMs {
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as i64)
}
