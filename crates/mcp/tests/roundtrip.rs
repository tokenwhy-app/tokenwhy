// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use serde_json::{json, Value};
use tokenwhy_core::{Amount, Finding, ProviderId, Snapshot, Window};
use tokenwhy_mcp::{Request, Server, Source};

const DAY: i64 = 86_400_000;

struct Fixture(Vec<Request>);

fn finding(rule: &str, usd: f64) -> Finding {
    let a = |v: f64| Amount {
        value: Some(v),
        est: true,
    };
    Finding {
        provider: Some(ProviderId("claude-code".into())),
        rule: rule.into(),
        project: Some("alpha".into()),
        line: format!("{rule} evidence"),
        tokens_saved_per_week: a(1e6),
        usd_saved_per_week: a(usd),
        co2_g_saved_per_week: Amount::none(),
        usd_saved_last_7d: Amount::none(),
    }
}

impl Source for Fixture {
    fn snapshot(&mut self, req: &Request) -> Result<Snapshot, String> {
        self.0.push(req.clone());
        let mut s: Snapshot =
            serde_json::from_str(include_str!("../../../app/web/public/sample-snapshot.json"))
                .unwrap();
        s.window = Window {
            from_ms: 0,
            to_ms: 14 * DAY,
        };
        s.progress = 1.0;
        s.daily = [30.0, 10.0]
            .iter()
            .map(|c| {
                serde_json::from_value(json!({
                    "day_ms": 0, "project": "alpha", "fresh_tokens": 0, "cache_read_tokens": 0,
                    "cache_write_tokens": 0, "cost_usd": {"value": c}, "sessions": 1, "agents": 0, "compactions": 0
                }))
                .unwrap()
            })
            .collect();
        s.findings = vec![finding("R11", 3.0), finding("R3", 5.0)];
        Ok(s)
    }
}

fn exchange(lines: &[Value]) -> (Vec<Value>, Vec<Request>) {
    let input: String = lines.iter().map(|l| format!("{l}\n")).collect();
    let mut src = Fixture(vec![]);
    let mut out = Vec::new();
    Server::new(&mut src)
        .serve(input.as_bytes(), &mut out)
        .unwrap();
    let replies = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    (replies, src.0)
}

#[test]
fn legacy_handshake_then_tools() {
    let (r, asked) = exchange(&[
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {
            "name": "suggest_fixes", "arguments": {"limit": 1}}}),
        json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {
            "name": "get_findings", "arguments": {"window_days": 7, "project": "alpha"}}}),
    ]);
    assert_eq!(r.len(), 4);
    assert_eq!(r[0]["result"]["protocolVersion"], "2025-11-25");
    let names: Vec<&str> = r[1]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "get_findings",
            "explain_spend",
            "suggest_fixes",
            "get_summary"
        ]
    );
    let fixes = &r[2]["result"]["structuredContent"];
    assert_eq!(fixes["items"][0]["rule"], "R3");
    assert_eq!(fixes["items"][0]["share_of_spend"], 0.25);
    assert_eq!(fixes["more"], 1);
    assert_eq!(fixes["saved_usd_per_week"], 8.0);
    assert!(r[2]["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("## 1. R3"));
    assert_eq!(r[3]["id"], 4);
    assert_eq!(
        r[3]["result"]["structuredContent"]["spend_usd_per_week"],
        20.0
    );
    assert_eq!(
        asked[1],
        Request {
            window_days: Some(7),
            project: Some("alpha".into()),
            provider: None
        }
    );
}

#[test]
fn modern_requests_carry_their_version() {
    let meta = |v: &str| json!({"io.modelcontextprotocol/protocolVersion": v, "io.modelcontextprotocol/clientCapabilities": {}});
    let (r, _) = exchange(&[
        json!({"jsonrpc": "2.0", "id": "a", "method": "server/discover", "params": {"_meta": meta("2026-07-28")}}),
        json!({"jsonrpc": "2.0", "id": "b", "method": "tools/list", "params": {"_meta": meta("1900-01-01")}}),
        json!({"jsonrpc": "2.0", "id": "c", "method": "tools/list"}),
        json!({"jsonrpc": "2.0", "id": "d", "method": "tools/call", "params": {
            "_meta": meta("2026-07-28"), "name": "get_summary"}}),
    ]);
    assert_eq!(r[0]["result"]["resultType"], "complete");
    assert_eq!(r[0]["result"]["supportedVersions"][0], "2026-07-28");
    assert_eq!(r[1]["error"]["code"], -32022);
    assert_eq!(r[1]["error"]["data"]["requested"], "1900-01-01");
    assert_eq!(r[2]["error"]["code"], -32602);
    let s = &r[3]["result"]["structuredContent"];
    assert_eq!(s["spend_usd"], 40.0);
    assert_eq!(s["top_fixes"][0]["rule"], "R3");
}
