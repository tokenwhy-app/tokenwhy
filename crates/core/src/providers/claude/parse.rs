// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use crate::classify::{self, is_log_path};
use crate::model::*;
use memchr::memmem::Finder;
use serde::de::{Deserializer, SeqAccess, Visitor};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};

const CHUNK: u64 = 4 << 20;
const PERSISTED: &str = "<persisted-output>";

pub(super) fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[derive(Default)]
struct TextInfo {
    chars: u64,
    prefix: String,
}

fn prefix_of(s: &str) -> String {
    let mut n = s.len().min(32);
    while !s.is_char_boundary(n) {
        n -= 1;
    }
    s[..n].to_string()
}

impl TextInfo {
    fn of(s: &str) -> Self {
        TextInfo {
            chars: s.chars().count() as u64,
            prefix: prefix_of(s),
        }
    }
}

impl<'de> Deserialize<'de> for TextInfo {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = TextInfo;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a string")
            }
            fn visit_str<E>(self, s: &str) -> Result<TextInfo, E> {
                Ok(TextInfo::of(s))
            }
        }
        d.deserialize_str(V)
    }
}

#[derive(Default)]
struct ResultContent {
    chars: u64,
    images: u32,
    prefix: String,
}

#[derive(Deserialize)]
struct ResBlock {
    #[serde(rename = "type")]
    ty: Option<String>,
    text: Option<TextInfo>,
}

impl<'de> Deserialize<'de> for ResultContent {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = ResultContent;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("string or array of blocks")
            }
            fn visit_str<E>(self, s: &str) -> Result<ResultContent, E> {
                let t = TextInfo::of(s);
                Ok(ResultContent {
                    chars: t.chars,
                    images: 0,
                    prefix: t.prefix,
                })
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<ResultContent, A::Error> {
                let mut rc = ResultContent::default();
                while let Some(b) = seq.next_element::<ResBlock>()? {
                    if b.ty.as_deref() == Some("image") {
                        rc.images += 1;
                    } else if let Some(t) = b.text {
                        rc.chars += t.chars;
                        if rc.prefix.is_empty() {
                            rc.prefix = t.prefix;
                        }
                    }
                }
                Ok(rc)
            }
        }
        d.deserialize_any(V)
    }
}

#[derive(Default, Deserialize)]
struct Input {
    command: Option<Value>,
    file_path: Option<Value>,
    notebook_path: Option<Value>,
    path: Option<Value>,
    skill: Option<Value>,
    subagent_type: Option<Value>,
    limit: Option<Value>,
}

fn vs(v: &Option<Value>) -> Option<&str> {
    v.as_ref().and_then(|x| x.as_str())
}

#[derive(Deserialize)]
struct Block {
    #[serde(rename = "type")]
    ty: Option<String>,
    name: Option<String>,
    id: Option<String>,
    tool_use_id: Option<String>,
    is_error: Option<bool>,
    input: Option<Input>,
    text: Option<TextInfo>,
    content: Option<ResultContent>,
}

enum Content {
    Text(TextInfo),
    Blocks(Vec<Block>),
}

impl<'de> Deserialize<'de> for Content {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Content;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("string or array of blocks")
            }
            fn visit_str<E>(self, s: &str) -> Result<Content, E> {
                Ok(Content::Text(TextInfo::of(s)))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Content, A::Error> {
                let mut v = Vec::new();
                while let Some(b) = seq.next_element::<Block>()? {
                    v.push(b);
                }
                Ok(Content::Blocks(v))
            }
        }
        d.deserialize_any(V)
    }
}

#[derive(Deserialize)]
struct RawUsage {
    input_tokens: Option<f64>,
    cache_creation_input_tokens: Option<f64>,
    cache_read_input_tokens: Option<f64>,
    output_tokens: Option<f64>,
    server_tool_use: Option<ServerToolUse>,
    cache_creation: Option<CacheCreation>,
}

#[derive(Deserialize)]
struct ServerToolUse {
    web_search_requests: Option<f64>,
}

#[derive(Deserialize)]
struct CacheCreation {
    ephemeral_5m_input_tokens: Option<f64>,
    ephemeral_1h_input_tokens: Option<f64>,
}

fn clamp(v: Option<f64>) -> u64 {
    match v {
        Some(x) if x.is_finite() && x > 0.0 => x as u64,
        _ => 0,
    }
}

impl RawUsage {
    /// codeburn models.ts:1516 guard.
    fn to_usage(&self) -> Usage {
        let cc = self.cache_creation.as_ref();
        let h1 = clamp(cc.and_then(|c| c.ephemeral_1h_input_tokens));
        let m5 = clamp(cc.and_then(|c| c.ephemeral_5m_input_tokens));
        let total = match self.cache_creation_input_tokens {
            Some(_) => clamp(self.cache_creation_input_tokens),
            None => m5.saturating_add(h1),
        }
        .max(h1);
        Usage {
            input: clamp(self.input_tokens),
            cache_write_5m: total - h1,
            cache_write_1h: h1,
            cache_read: clamp(self.cache_read_input_tokens),
            output: clamp(self.output_tokens),
            web_search_requests: clamp(
                self.server_tool_use
                    .as_ref()
                    .and_then(|s| s.web_search_requests),
            ) as u32,
        }
    }
}

#[derive(Deserialize)]
struct Message {
    id: Option<String>,
    model: Option<String>,
    usage: Option<RawUsage>,
    content: Option<Content>,
}

#[derive(Deserialize)]
struct CompactMeta {
    #[serde(rename = "preTokens")]
    pre_tokens: Option<f64>,
    #[serde(rename = "postTokens")]
    post_tokens: Option<f64>,
    trigger: Option<String>,
}

#[derive(Deserialize)]
struct Origin {
    kind: Option<String>,
}

#[derive(Deserialize)]
struct InstrFile {
    path: Option<String>,
    #[serde(rename = "type")]
    ty: Option<String>,
    content: Option<String>,
}

#[derive(Deserialize)]
struct Attachment {
    #[serde(rename = "type")]
    ty: Option<String>,
    files: Option<Vec<InstrFile>>,
    content: Option<String>,
    #[serde(rename = "addedNames")]
    added_names: Option<Vec<String>>,
    #[serde(rename = "addedLines")]
    added_lines: Option<Vec<String>>,
    #[serde(rename = "addedBlocks")]
    added_blocks: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct Shape {
    system: Option<Value>,
    tools: Option<Vec<Value>>,
}

#[derive(Deserialize)]
struct Line {
    #[serde(rename = "type")]
    ty: Option<String>,
    subtype: Option<String>,
    timestamp: Option<String>,
    cwd: Option<String>,
    uuid: Option<String>,
    #[serde(rename = "isMeta")]
    is_meta: Option<bool>,
    #[serde(rename = "isCompactSummary")]
    is_compact_summary: Option<bool>,
    #[serde(rename = "sourceToolUseID")]
    source_tool_use_id: Option<String>,
    #[serde(rename = "attributionSkill")]
    attribution_skill: Option<String>,
    #[serde(rename = "compactMetadata")]
    compact_metadata: Option<CompactMeta>,
    origin: Option<Origin>,
    message: Option<Message>,
    attachment: Option<Attachment>,
    shape: Option<Shape>,
}

const SKIP_TYPES: [&str; 9] = [
    "api-request-blob",
    "api-request",
    "file-history-snapshot",
    "file-history-delta",
    "queue-operation",
    "ai-title",
    "last-prompt",
    "mode",
    "permission-mode",
];

const STARTUP_ATTACHMENTS: [&str; 5] = [
    "instructions",
    "skill_listing",
    "deferred_tools_delta",
    "agent_listing_delta",
    "mcp_instructions_delta",
];

struct State {
    rec: FileRecords,
    strs: HashMap<String, u32>,
    turn_idx: HashMap<IdHash, usize>,
    call_idx: HashMap<IdHash, usize>,
    skill_calls: HashMap<IdHash, u32>,
    mcp_set: HashSet<(u32, u32)>,
    startup: StartupParts,
    seen_kinds: HashSet<StartupKind>,
    next_order: u32,
    skip: Vec<Finder<'static>>,
    attach_marker: Finder<'static>,
    shape_marker: Finder<'static>,
    project_tools: Vec<String>,
}

impl State {
    fn new(previous: FileRecords) -> Self {
        let mut s = State {
            strs: HashMap::new(),
            turn_idx: HashMap::new(),
            call_idx: HashMap::new(),
            skill_calls: HashMap::new(),
            mcp_set: HashSet::new(),
            startup: previous.startup.clone().unwrap_or_default(),
            seen_kinds: HashSet::new(),
            next_order: 0,
            skip: SKIP_TYPES
                .iter()
                .map(|t| Finder::new(format!("\"type\":\"{t}\"").as_bytes()).into_owned())
                .collect(),
            attach_marker: Finder::new(b"\"attachment\":{\"type\":\"").into_owned(),
            shape_marker: Finder::new(b"\"type\":\"api-request-shape\"").into_owned(),
            project_tools: Settings::default().project_tools,
            rec: previous,
        };
        for (i, st) in s.rec.strings.iter().enumerate() {
            s.strs.insert(st.clone(), i as u32);
        }
        for (i, t) in s.rec.turns.iter().enumerate() {
            s.turn_idx.insert(t.msg_id, i);
            s.next_order = s.next_order.max(t.order + 1);
        }
        for (i, c) in s.rec.tool_calls.iter().enumerate() {
            s.call_idx.insert(c.call_id, i);
            if s.rec.strings.get(c.tool as usize).map(String::as_str) == Some("Skill") {
                if let Some(d) = c.detail {
                    s.skill_calls.insert(c.call_id, d);
                }
            }
        }
        for o in s
            .rec
            .skill_loads
            .iter()
            .map(|x| x.order)
            .chain(s.rec.compactions.iter().map(|x| x.order))
            .chain(s.rec.human_prompts.iter().copied())
        {
            s.next_order = s.next_order.max(o + 1);
        }
        s.mcp_set = s.rec.mcp.loaded.iter().copied().collect();
        s.seen_kinds = s.startup.parts.iter().map(|p| p.kind).collect();
        s
    }

    fn intern(&mut self, st: &str) -> u32 {
        if let Some(&i) = self.strs.get(st) {
            return i;
        }
        let i = self.rec.strings.len() as u32;
        self.rec.strings.push(st.to_string());
        self.strs.insert(st.to_string(), i);
        i
    }

    fn order(&mut self) -> u32 {
        let o = self.next_order;
        self.next_order += 1;
        o
    }

    fn line(&mut self, raw: &[u8]) {
        let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
        if raw.is_empty() || self.skip.iter().any(|f| f.find(raw).is_some()) {
            return;
        }
        if self.shape_marker.find(raw).is_some() && self.startup.has_full_shape {
            return;
        }
        if let Some(p) = self.attach_marker.find(raw) {
            let rest = &raw[p + self.attach_marker.needle().len()..];
            let end = rest.iter().position(|&b| b == b'"').unwrap_or(0);
            let ty = std::str::from_utf8(&rest[..end]).unwrap_or("");
            if !STARTUP_ATTACHMENTS.contains(&ty) {
                return;
            }
        }
        let Ok(l) = serde_json::from_slice::<Line>(raw) else {
            return;
        };
        if self.rec.project_name.is_none() {
            if let Some(cwd) = &l.cwd {
                let base = cwd
                    .trim_end_matches(['/', '\\'])
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or("");
                if !base.is_empty() {
                    let i = self.intern(base);
                    self.rec.project_name = Some(i);
                }
            }
        }
        let ts = l
            .timestamp
            .as_deref()
            .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
            .map(|d| d.timestamp_millis());
        match l.ty.as_deref() {
            Some("assistant") => self.assistant(&l, ts.unwrap_or(0)),
            Some("user") => self.user(&l),
            Some("system") => self.system(&l),
            Some("attachment") => self.attachment(&l),
            Some("api-request-shape") => self.shape(&l),
            _ => {}
        }
    }

    fn assistant(&mut self, l: &Line, ts: TimestampMs) {
        let Some(m) = &l.message else { return };
        let model = m.model.as_deref().unwrap_or("");
        if model.starts_with('<') {
            return;
        }
        let key = m.id.as_deref().or(l.uuid.as_deref()).unwrap_or("");
        if key.is_empty() {
            return;
        }
        let id = fnv1a64(key.as_bytes());
        let usage = m.usage.as_ref().map(RawUsage::to_usage);
        let turn_order = match self.turn_idx.get(&id) {
            Some(&i) => {
                let t = &mut self.rec.turns[i];
                if let Some(u) = usage {
                    t.usage = u;
                }
                t.order
            }
            None => {
                let order = self.order();
                let model_i = self.intern(model);
                self.rec.turns.push(Turn {
                    msg_id: id,
                    first_ts_ms: ts,
                    model: model_i,
                    usage: usage.unwrap_or_default(),
                    attribution_skill: None,
                    segment: self.rec.compactions.len() as u32,
                    order,
                    stored_cost: None,
                });
                self.turn_idx.insert(id, self.rec.turns.len() - 1);
                order
            }
        };
        if let Some(sk) = &l.attribution_skill {
            let si = self.intern(sk);
            let i = self.turn_idx[&id];
            self.rec.turns[i].attribution_skill = Some(si);
        }
        if let Some(Content::Blocks(blocks)) = &m.content {
            for b in blocks {
                if b.ty.as_deref() == Some("tool_use") {
                    self.tool_use(b, turn_order);
                }
            }
        }
    }

    fn tool_use(&mut self, b: &Block, turn_order: u32) {
        let (Some(id), Some(name)) = (b.id.as_deref(), b.name.as_deref()) else {
            return;
        };
        let call_id = fnv1a64(id.as_bytes());
        if self.call_idx.contains_key(&call_id) {
            return;
        }
        let empty = Input::default();
        let inp = b.input.as_ref().unwrap_or(&empty);
        let mut bounded = false;
        let mut writes = false;
        let mut path: Option<String> = None;
        let mut detail: Option<String> = None;
        let category = match name {
            "Bash" | "PowerShell" => {
                let c =
                    classify::classify_command(vs(&inp.command).unwrap_or(""), &self.project_tools);
                bounded = c.bounded;
                path = c.path;
                detail = c.detail;
                c.category
            }
            "Read" => {
                path = vs(&inp.file_path).or(vs(&inp.path)).map(str::to_string);
                bounded = inp.limit.is_some();
                if path.as_deref().is_some_and(is_log_path) {
                    Category::Logs
                } else {
                    Category::ReadFile
                }
            }
            "Grep" => {
                path = vs(&inp.path).map(str::to_string);
                Category::Search
            }
            "Glob" => Category::List,
            "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => {
                path = vs(&inp.file_path)
                    .or(vs(&inp.notebook_path))
                    .map(str::to_string);
                writes = true;
                Category::Edit
            }
            "WebFetch" | "WebSearch" => Category::Web,
            "Agent" | "Task" => {
                detail = vs(&inp.subagent_type).map(str::to_string);
                Category::Agent
            }
            "Skill" => {
                detail = vs(&inp.skill).map(str::to_string);
                Category::Other
            }
            n if n.starts_with("mcp__") => {
                detail = n["mcp__".len()..].split("__").next().map(str::to_string);
                Category::Mcp
            }
            n => {
                detail = Some(n.chars().take(20).collect());
                Category::Other
            }
        };
        let tool = self.intern(name);
        let detail_i = detail.as_deref().map(|d| self.intern(d));
        let path_pair = path.as_deref().map(|p| {
            let norm = p.replace('\\', "/").to_lowercase();
            (fnv1a64(norm.as_bytes()), self.intern(p))
        });
        if name == "Skill" {
            if let Some(d) = detail_i {
                self.skill_calls.insert(call_id, d);
            }
        }
        self.rec.tool_calls.push(ToolCall {
            call_id,
            turn_order,
            tool,
            category,
            detail: detail_i,
            bounded,
            path: path_pair,
            writes_path: writes,
            result_chars: 0,
            is_error: false,
            persisted: false,
            images: 0,
        });
        self.call_idx.insert(call_id, self.rec.tool_calls.len() - 1);
        self.rec.open_calls.push(call_id);
    }

    fn user(&mut self, l: &Line) {
        let Some(m) = &l.message else { return };
        let Some(content) = &m.content else { return };
        let is_meta = l.is_meta.unwrap_or(false);

        if l.is_compact_summary.unwrap_or(false) {
            let chars = match content {
                Content::Text(t) => t.chars,
                Content::Blocks(bs) => bs
                    .iter()
                    .filter_map(|b| b.text.as_ref())
                    .map(|t| t.chars)
                    .sum(),
            };
            if let Some(c) = self.rec.compactions.last_mut() {
                if c.summary_chars == 0 {
                    c.summary_chars = chars;
                }
            }
            return;
        }

        if is_meta {
            if let Some(src) = &l.source_tool_use_id {
                if let Some(&skill) = self.skill_calls.get(&fnv1a64(src.as_bytes())) {
                    let chars = match content {
                        Content::Text(t) => t.chars,
                        Content::Blocks(bs) => bs
                            .iter()
                            .filter_map(|b| b.text.as_ref())
                            .map(|t| t.chars)
                            .sum(),
                    };
                    let order = self.order();
                    self.rec.skill_loads.push(SkillLoad {
                        skill,
                        body_chars: chars,
                        order,
                    });
                }
            }
            return;
        }

        match content {
            Content::Text(t) => {
                if self.is_human(l, &t.prefix) {
                    let o = self.order();
                    self.rec.human_prompts.push(o);
                }
            }
            Content::Blocks(bs) => {
                let mut any_result = false;
                for b in bs {
                    if b.ty.as_deref() == Some("tool_result") {
                        any_result = true;
                        self.tool_result(b);
                    }
                }
                if !any_result {
                    if let Some(first) = bs.iter().find_map(|b| b.text.as_ref()) {
                        if self.is_human(l, &first.prefix) {
                            let o = self.order();
                            self.rec.human_prompts.push(o);
                        }
                    }
                }
            }
        }
    }

    fn is_human(&self, l: &Line, prefix: &str) -> bool {
        let origin_ok = l
            .origin
            .as_ref()
            .is_none_or(|o| o.kind.as_deref().is_none_or(|k| k == "human"));
        origin_ok && !prefix.starts_with("<system-reminder") && !prefix.starts_with("<ide_")
    }

    fn tool_result(&mut self, b: &Block) {
        let Some(id) = b.tool_use_id.as_deref() else {
            return;
        };
        let h = fnv1a64(id.as_bytes());
        let Some(&i) = self.call_idx.get(&h) else {
            return;
        };
        let call = &mut self.rec.tool_calls[i];
        if let Some(rc) = &b.content {
            call.result_chars = rc.chars;
            call.images = rc.images;
            call.persisted = rc.prefix.starts_with(PERSISTED);
        }
        call.is_error = b.is_error.unwrap_or(false);
        if let Some(p) = self.rec.open_calls.iter().rposition(|&c| c == h) {
            self.rec.open_calls.remove(p);
        }
    }

    fn system(&mut self, l: &Line) {
        if l.subtype.as_deref() != Some("compact_boundary") {
            return;
        }
        let meta = l.compact_metadata.as_ref();
        let trigger = self.intern(meta.and_then(|m| m.trigger.as_deref()).unwrap_or("auto"));
        let order = self.order();
        self.rec.compactions.push(Compaction {
            order,
            trigger,
            pre_tokens: clamp(meta.and_then(|m| m.pre_tokens)),
            post_tokens: clamp(meta.and_then(|m| m.post_tokens)),
            summary_chars: 0,
        });
    }

    fn part(&mut self, kind: StartupKind, name: &str, chars: u64, content_hash: IdHash) {
        let name = self.intern(name);
        self.startup.parts.push(StartupPart {
            kind,
            name,
            chars,
            content_hash,
        });
    }

    fn mcp_loaded(&mut self, full_tool_name: &str) {
        if let Some((server, tool)) = split_mcp(full_tool_name) {
            let pair = (self.intern(server), self.intern(tool));
            if self.mcp_set.insert(pair) {
                self.rec.mcp.loaded.push(pair);
            }
        }
    }

    fn attachment(&mut self, l: &Line) {
        let Some(a) = &l.attachment else { return };
        match a.ty.as_deref() {
            Some("instructions") if self.seen_kinds.insert(StartupKind::UserInstructions) => {
                for f in a.files.iter().flatten() {
                    let kind = match f.ty.as_deref() {
                        Some("User") => StartupKind::UserInstructions,
                        Some("AutoMem") => StartupKind::Memory,
                        _ => StartupKind::ProjectInstructions,
                    };
                    let content = f.content.as_deref().unwrap_or("");
                    let path = f.path.as_deref().unwrap_or("");
                    self.part(
                        kind,
                        path,
                        content.chars().count() as u64,
                        fnv1a64(content.as_bytes()),
                    );
                    let mut hashes: Vec<IdHash> = Vec::new();
                    for line in content.lines() {
                        let norm = line
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" ")
                            .to_lowercase();
                        if norm.chars().count() >= 30 {
                            let h = fnv1a64(norm.as_bytes());
                            if !hashes.contains(&h) {
                                hashes.push(h);
                            }
                        }
                    }
                    let pi = self.intern(path);
                    self.startup.instruction_lines.push((pi, hashes));
                }
            }
            Some("skill_listing") if self.seen_kinds.insert(StartupKind::SkillListing) => {
                for (name, chars) in listing_entries(a.content.as_deref().unwrap_or("")) {
                    self.part(StartupKind::SkillListing, &name, chars, 0);
                }
            }
            Some("agent_listing_delta") if self.seen_kinds.insert(StartupKind::AgentListing) => {
                let lines: Vec<String> = a.added_lines.clone().unwrap_or_default();
                for (name, chars) in listing_entries(&lines.join("\n")) {
                    self.part(StartupKind::AgentListing, &name, chars, 0);
                }
            }
            Some("mcp_instructions_delta")
                if self.seen_kinds.insert(StartupKind::McpInstructions) =>
            {
                let names = a.added_names.clone().unwrap_or_default();
                let blocks = a.added_blocks.clone().unwrap_or_default();
                for (n, b) in names.iter().zip(blocks.iter()) {
                    self.part(StartupKind::McpInstructions, n, b.chars().count() as u64, 0);
                }
            }
            Some("deferred_tools_delta") => {
                let names = a.added_names.clone().unwrap_or_default();
                for n in &names {
                    self.mcp_loaded(n);
                }
                if self.seen_kinds.insert(StartupKind::DeferredTools) {
                    let mut per: Vec<(String, u64)> = Vec::new();
                    for n in &names {
                        let server = split_mcp(n).map_or("(builtin)", |(s, _)| s);
                        match per.iter_mut().find(|(s, _)| s == server) {
                            Some(e) => e.1 += n.chars().count() as u64 + 1,
                            None => per.push((server.to_string(), n.chars().count() as u64 + 1)),
                        }
                    }
                    for (s, c) in per {
                        self.part(StartupKind::DeferredTools, &s, c, 0);
                    }
                }
            }
            _ => {}
        }
    }

    fn shape(&mut self, l: &Line) {
        if self.startup.has_full_shape {
            return;
        }
        let Some(s) = &l.shape else { return };
        self.startup.has_full_shape = true;
        let mut sys = 0u64;
        match &s.system {
            Some(Value::Array(a)) => {
                for b in a {
                    sys += b
                        .get("text")
                        .and_then(|t| t.as_str())
                        .map_or(0, |t| t.chars().count() as u64);
                }
            }
            Some(Value::String(t)) => sys += t.chars().count() as u64,
            _ => {}
        }
        self.part(StartupKind::SystemPrompt, "system", sys, 0);
        let mut mcp_chars: Vec<(String, u64)> = Vec::new();
        for t in s.tools.iter().flatten() {
            let chars = serde_json::to_string(t).map_or(0, |j| j.chars().count() as u64);
            let name = t.get("name").and_then(|n| n.as_str()).unwrap_or("");
            match split_mcp(name) {
                Some((server, _)) => {
                    self.mcp_loaded(name);
                    match mcp_chars.iter_mut().find(|(s, _)| s == server) {
                        Some(e) => e.1 += chars,
                        None => mcp_chars.push((server.to_string(), chars)),
                    }
                }
                None => self.part(StartupKind::BuiltinTools, name, chars, 0),
            }
        }
        for (s, c) in mcp_chars {
            self.part(StartupKind::McpTools, &s, c, 0);
        }
    }
}

fn split_mcp(name: &str) -> Option<(&str, &str)> {
    name.strip_prefix("mcp__")?.split_once("__")
}

fn listing_entries(text: &str) -> Vec<(String, u64)> {
    let mut out: Vec<(String, u64)> = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("- ") {
            let name = rest.split_once(": ").map_or(rest, |(n, _)| n);
            out.push((name.to_string(), line.chars().count() as u64 + 1));
        } else if let Some(last) = out.last_mut() {
            last.1 += line.chars().count() as u64 + 1;
        }
    }
    out
}

pub(super) fn parse_file(
    file: &SourceFile,
    resume: Option<ParseResume>,
) -> io::Result<FileRecords> {
    let mut f = File::open(&file.path)?;
    let len = f.metadata()?.len();
    let (start, previous) = match resume {
        Some(r) if r.offset <= len => (r.offset, r.previous),
        _ => (0, FileRecords::default()),
    };
    let mut st = State::new(previous);
    f.seek(SeekFrom::Start(start))?;
    let mut buf: Vec<u8> = Vec::new();
    let mut pos = start;
    loop {
        let n = (&mut f).take(CHUNK).read_to_end(&mut buf)?;
        if n == 0 {
            break;
        }
        let mut from = 0;
        while let Some(i) = memchr::memchr(b'\n', &buf[from..]) {
            st.line(&buf[from..from + i]);
            from += i + 1;
        }
        pos += from as u64;
        buf.drain(..from);
    }
    let mut rec = st.rec;
    rec.offset_done = pos;
    if rec.project_name.is_none() {
        let key = file.project_key.as_str();
        let stripped = match key.split_once("--") {
            Some((d, rest)) if d.len() == 1 => rest,
            _ => key,
        };
        let mut strings = rec.strings;
        strings.push(stripped.to_string());
        rec.project_name = Some(strings.len() as u32 - 1);
        rec.strings = strings;
    }
    if !st.startup.parts.is_empty() || st.startup.has_full_shape {
        rec.startup = Some(st.startup);
    }
    Ok(rec)
}
