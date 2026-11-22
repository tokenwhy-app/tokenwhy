// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Dual-era per MCP 2026-07-28 (basic/versioning): requests carrying `_meta["io.modelcontextprotocol/protocolVersion"]` are served statelessly; an `initialize` selects the 2025-11-25 handshake.

use serde_json::{json, Map, Value};
use std::io::{self, BufRead, Write};
use tokenwhy_core::agent::{self, Filter};
use tokenwhy_core::Snapshot;

pub const MODERN_VERSIONS: [&str; 1] = ["2026-07-28"];
pub const LEGACY_VERSIONS: [&str; 3] = ["2025-11-25", "2025-06-18", "2025-03-26"];
const VERSION_KEY: &str = "io.modelcontextprotocol/protocolVersion";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Request {
    pub window_days: Option<u32>,
    pub project: Option<String>,
    pub provider: Option<String>,
}

pub trait Source {
    fn snapshot(&mut self, req: &Request) -> Result<Snapshot, String>;
}

type RpcError = (i64, String, Option<Value>);

fn invalid(msg: impl Into<String>) -> RpcError {
    (-32602, msg.into(), None)
}

pub struct Server<'a> {
    src: &'a mut dyn Source,
    legacy: bool,
}

impl<'a> Server<'a> {
    pub fn new(src: &'a mut dyn Source) -> Self {
        Server { src, legacy: false }
    }

    pub fn serve(&mut self, input: impl BufRead, mut output: impl Write) -> io::Result<()> {
        for line in input.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Some(reply) = self.handle(&line) {
                writeln!(output, "{reply}")?;
                output.flush()?;
            }
        }
        Ok(())
    }

    pub fn handle(&mut self, line: &str) -> Option<Value> {
        let msg: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => return Some(error(Value::Null, (-32700, "Parse error".into(), None))),
        };
        let id = msg.get("id").cloned();
        let Some(method) = msg.get("method").and_then(Value::as_str) else {
            return match id {
                Some(_) if msg.get("result").is_some() || msg.get("error").is_some() => None,
                _ => Some(error(
                    id.unwrap_or(Value::Null),
                    (-32600, "Invalid Request".into(), None),
                )),
            };
        };
        let id = id.filter(|v| !v.is_null())?;
        let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.dispatch(method, &params)
        }))
        .unwrap_or_else(|_| Err((-32603, "Internal error".into(), None)));
        Some(match res {
            Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
            Err(e) => error(id, e),
        })
    }

    fn dispatch(&mut self, method: &str, params: &Value) -> Result<Value, RpcError> {
        if method == "initialize" {
            self.legacy = true;
            let asked = params.get("protocolVersion").and_then(Value::as_str);
            let version = asked
                .filter(|v| LEGACY_VERSIONS.contains(v))
                .unwrap_or(LEGACY_VERSIONS[0]);
            return Ok(json!({
                "protocolVersion": version,
                "capabilities": {"tools": {}},
                "serverInfo": server_info(),
                "instructions": INSTRUCTIONS,
            }));
        }
        let asked = params
            .get("_meta")
            .and_then(|m| m.get(VERSION_KEY))
            .and_then(Value::as_str);
        match asked {
            Some(v) if !MODERN_VERSIONS.contains(&v) => {
                let supported: Vec<&str> = MODERN_VERSIONS
                    .iter()
                    .chain(LEGACY_VERSIONS.iter())
                    .copied()
                    .collect();
                return Err((
                    -32022,
                    "Unsupported protocol version".into(),
                    Some(json!({"supported": supported, "requested": v})),
                ));
            }
            None if !self.legacy => return Err(invalid(format!("missing _meta.{VERSION_KEY}"))),
            _ => {}
        }
        let mut result = match method {
            "server/discover" => json!({
                "supportedVersions": MODERN_VERSIONS.iter().chain(LEGACY_VERSIONS.iter()).collect::<Vec<_>>(),
                "capabilities": {"tools": {}},
                "instructions": INSTRUCTIONS,
            }),
            "ping" => json!({}),
            "tools/list" => json!({"tools": tools()}),
            "tools/call" => self.call(params)?,
            _ => return Err((-32601, format!("Method not found: {method}"), None)),
        };
        if let Some(o) = result.as_object_mut() {
            o.insert("resultType".into(), "complete".into());
            o.insert(
                "_meta".into(),
                json!({"io.modelcontextprotocol/serverInfo": server_info()}),
            );
        }
        Ok(result)
    }

    fn call(&mut self, params: &Value) -> Result<Value, RpcError> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("missing tool name"))?;
        let empty = Map::new();
        let args = params
            .get("arguments")
            .and_then(Value::as_object)
            .unwrap_or(&empty);
        let s = |k: &str| args.get(k).and_then(Value::as_str).map(str::to_string);
        let n = |k: &str| -> Result<Option<u64>, RpcError> {
            match args.get(k) {
                None | Some(Value::Null) => Ok(None),
                Some(v) => v
                    .as_u64()
                    .filter(|n| *n >= 1)
                    .map(Some)
                    .ok_or_else(|| invalid(format!("{k} must be a positive integer"))),
            }
        };
        let out = match name {
            "get_findings" => {
                let req = Request {
                    window_days: n("window_days")?.map(|d| d.min(u32::MAX as u64) as u32),
                    project: s("project"),
                    provider: s("provider"),
                };
                self.src
                    .snapshot(&req)
                    .map(|snap| text_json(get_findings(&snap)))
            }
            "explain_spend" => {
                let (project, session) = (s("project"), s("session"));
                let req = Request {
                    project: project.clone(),
                    ..Default::default()
                };
                self.src
                    .snapshot(&req)
                    .map(|snap| text_json(explain_spend(&snap, project, session)))
            }
            "suggest_fixes" => {
                let limit = n("limit")?.map_or(agent::DEFAULT_LIMIT, |l| l as usize);
                self.src.snapshot(&Request::default()).map(|snap| {
                    let b = agent::brief(
                        &snap,
                        &Filter {
                            limit,
                            ..Default::default()
                        },
                    );
                    (
                        agent::markdown(&b),
                        serde_json::to_value(&b).unwrap_or(Value::Null),
                    )
                })
            }
            "get_summary" => self
                .src
                .snapshot(&Request::default())
                .map(|snap| text_json(get_summary(&snap))),
            _ => return Err(invalid(format!("unknown tool: {name}"))),
        };
        Ok(match out {
            Ok((text, structured)) => json!({
                "content": [{"type": "text", "text": text}],
                "structuredContent": structured,
                "isError": false,
            }),
            Err(e) => json!({
                "content": [{"type": "text", "text": e}],
                "isError": true,
            }),
        })
    }
}

const INSTRUCTIONS: &str = "tokenwhy reads local agent transcripts (read-only, offline) and says where tokens and \
dollars go. Call suggest_fixes for ranked instructions to apply, explain_spend for one project or session.";

fn server_info() -> Value {
    json!({"name": "tokenwhy", "version": env!("CARGO_PKG_VERSION")})
}

fn error(id: Value, (code, message, data): RpcError) -> Value {
    let mut e = json!({"code": code, "message": message});
    if let Some(d) = data {
        e["data"] = d;
    }
    json!({"jsonrpc": "2.0", "id": id, "error": e})
}

fn text_json(v: Value) -> (String, Value) {
    (v.to_string(), v)
}

fn tools() -> Value {
    let read_only = json!({"readOnlyHint": true, "openWorldHint": false});
    json!([
        {
            "name": "get_findings",
            "description": "Every finding in the window with its rule, evidence line, tokens and dollars saved per week, and the fix to apply.",
            "inputSchema": {"type": "object", "properties": {
                "window_days": {"type": "integer", "minimum": 1, "description": "Days to analyse (default 14)."},
                "project": {"type": "string"},
                "provider": {"type": "string", "description": "Agent id, e.g. claude-code."}
            }},
            "annotations": read_only,
        },
        {
            "name": "explain_spend",
            "description": "Where the money went: per project without arguments, or for one project or one session (id or prefix), with its costliest sessions and fixes.",
            "inputSchema": {"type": "object", "properties": {
                "project": {"type": "string"},
                "session": {"type": "string"}
            }},
            "annotations": read_only,
        },
        {
            "name": "suggest_fixes",
            "description": "Fixes ranked by dollars a week saved, each with evidence and the exact instruction (config line, CLAUDE.md rule, plugin to disable).",
            "inputSchema": {"type": "object", "properties": {
                "limit": {"type": "integer", "minimum": 1, "description": "Fixes to return (default 8)."}
            }},
            "annotations": read_only,
        },
        {
            "name": "get_summary",
            "description": "Spend in the window and today, session counts and the top three fixes.",
            "inputSchema": {"type": "object", "properties": {}},
            "annotations": read_only,
        },
    ])
}

fn get_findings(snap: &Snapshot) -> Value {
    let findings: Vec<Value> = snap
        .findings
        .iter()
        .map(|f| {
            let mut v = serde_json::to_value(f).unwrap_or(Value::Null);
            v["fix"] = agent::fix(&f.rule).map_or("", |x| x.apply).into();
            v
        })
        .collect();
    let spend = agent::spend_usd(snap);
    json!({
        "schema": agent::BRIEF_SCHEMA_VERSION,
        "window": snap.window,
        "anonymised": snap.anonymised,
        "spend_usd": spend,
        "spend_usd_per_week": spend.map(|s| (s / agent::weeks(&snap) * 100.0).round() / 100.0),
        "findings": findings,
    })
}

fn explain_spend(snap: &Snapshot, project: Option<String>, session: Option<String>) -> Value {
    let base = json!({
        "schema": agent::BRIEF_SCHEMA_VERSION,
        "window": snap.window,
        "anonymised": snap.anonymised,
    });
    let mut out = base.as_object().cloned().unwrap_or_default();
    if let Some(id) = session {
        let Some(s) = snap.sessions.iter().find(|s| s.session.starts_with(&id)) else {
            out.insert(
                "error".into(),
                format!("no session {id} in the window").into(),
            );
            return Value::Object(out);
        };
        let b = agent::brief(
            snap,
            &Filter {
                project: Some(s.project.clone()),
                limit: 3,
                ..Default::default()
            },
        );
        out.insert(
            "session".into(),
            serde_json::to_value(s).unwrap_or(Value::Null),
        );
        out.insert(
            "fixes".into(),
            serde_json::to_value(&b.items).unwrap_or(Value::Null),
        );
        return Value::Object(out);
    }
    let mut by_project: Vec<(String, f64, u32)> = Vec::new();
    for d in &snap.daily {
        let p = d.project.clone().unwrap_or_default();
        let c = d.cost_usd.value.unwrap_or(0.0);
        match by_project.iter_mut().find(|r| r.0 == p) {
            Some(r) => {
                r.1 += c;
                r.2 += d.sessions;
            }
            None => by_project.push((p, c, d.sessions)),
        }
    }
    by_project.sort_by(|a, b| b.1.total_cmp(&a.1));
    let rows: Vec<Value> = by_project
        .iter()
        .filter(|r| project.as_ref().is_none_or(|p| &r.0 == p))
        .take(10)
        .map(|(p, c, n)| json!({"project": p, "cost_usd": (c * 100.0).round() / 100.0, "sessions": n}))
        .collect();
    let top: Vec<Value> = snap
        .sessions
        .iter()
        .filter(|s| project.as_ref().is_none_or(|p| &s.project == p))
        .take(5)
        .filter_map(|s| serde_json::to_value(s).ok())
        .collect();
    let b = agent::brief(
        snap,
        &Filter {
            project,
            limit: 5,
            ..Default::default()
        },
    );
    out.insert("spend_usd".into(), b.spend_usd.into());
    out.insert("spend_usd_per_week".into(), b.spend_usd_per_week.into());
    out.insert("projects".into(), rows.into());
    out.insert("top_sessions".into(), top.into());
    out.insert(
        "fixes".into(),
        serde_json::to_value(&b.items).unwrap_or(Value::Null),
    );
    Value::Object(out)
}

fn get_summary(snap: &Snapshot) -> Value {
    let b = agent::brief(
        snap,
        &Filter {
            limit: 3,
            ..Default::default()
        },
    );
    let top: Vec<Value> = b
        .items
        .iter()
        .map(|g| json!({"rule": g.rule, "project": g.project, "usd_saved_per_week": g.usd_saved_per_week, "fix": g.fix}))
        .collect();
    json!({
        "schema": agent::BRIEF_SCHEMA_VERSION,
        "window": snap.window,
        "anonymised": snap.anonymised,
        "partial": b.partial,
        "spend_usd": b.spend_usd,
        "spend_usd_per_week": b.spend_usd_per_week,
        "today_usd": snap.live.today.cost_usd.value,
        "sessions": snap.sessions.len(),
        "active_sessions": snap.live.sessions.len(),
        "saved_usd_per_week": b.saved_usd_per_week,
        "top_fixes": top,
    })
}
