// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use super::*;
use serde_json::json;

const ID: &str = "9f2c4b7e1a0d4c3b8e6f5a2d1c0b9e8f";

fn amt(v: f64) -> Value {
    json!({ "value": v })
}

fn snapshot(w: Week) -> Snapshot {
    let secret = "secret-acme-payroll";
    serde_json::from_value(json!({
        "schema": 2, "generated_ms": w.to_ms(), "window": { "from_ms": w.from_ms(), "to_ms": w.to_ms() },
        "anonymised": false, "progress": 1.0,
        "live": { "today": { "fresh_tokens": 0, "cache_read_tokens": 0, "cost_usd": amt(0.0), "sessions": 0, "agents": 0 },
                  "sessions": [] },
        "findings": [
            { "rule": "R1", "project": secret, "line": format!("C:/Users/jane/{secret}/CLAUDE.md is big"), "tokens_saved_per_week": amt(9e4) },
            { "rule": "R7", "project": null, "line": secret, "tokens_saved_per_week": amt(1.0) },
            { "rule": "R99", "project": null, "line": secret, "tokens_saved_per_week": amt(1.0) }
        ],
        "startup": [{ "provider": "claude-code", "project": secret, "sessions": 12, "median": 31_000, "p90": 52_000,
            "min": 20_000, "max": 60_000, "ratio": null, "unattributed": amt(1_000.0), "parts": [
                { "kind": "system-prompt", "name": "system", "tokens": amt(3_000.0) },
                { "kind": "builtin-tools", "name": "tools", "tokens": amt(9_000.0) },
                { "kind": "mcp-tools", "name": format!("{secret}-mcp"), "tokens": amt(8_000.0) },
                { "kind": "skill-listing", "name": format!("{secret}-skill"), "tokens": amt(4_000.0) },
                { "kind": "project-instructions", "name": format!("D:/{secret}/CLAUDE.md"), "tokens": amt(5_000.0) },
                { "kind": "memory", "name": format!("D:/{secret}/MEMORY.md"), "tokens": amt(1_000.0) }
            ] }],
        "skills": [
            { "skill": format!("{secret}-skill"), "listing_tokens_median": amt(300.0), "activations": 4,
              "body_tokens_median": amt(2_000.0), "carried_cost_usd": amt(0.1), "sessions_loaded": 3, "sessions_listed": 12,
              "attributed_turns": 9, "attributed_cost_usd": amt(0.2) },
            { "skill": "pdf", "listing_tokens_median": amt(100.0), "activations": 0, "body_tokens_median": amt(0.0),
              "carried_cost_usd": amt(0.0), "sessions_loaded": 0, "sessions_listed": 12, "attributed_turns": 0,
              "attributed_cost_usd": amt(0.0) }
        ],
        "memory": [{ "kind": "memory", "path": format!("D:/{secret}/MEMORY.md"), "label": secret, "bytes": 4_000,
            "tokens": amt(1_000.0), "carried_cost_usd": amt(0.0), "on_demand_reads": 0, "duplicate_lines": 12 }],
        "subagents": [],
        "compactions": [{ "provider": "claude-code", "project": secret, "session": secret, "ts_ms": w.from_ms() + 1,
            "pre_tokens": 160_000, "post_tokens": 20_000, "first_after_ctx": 30_000, "cost_per_turn_before": amt(0.2),
            "cost_per_turn_after": amt(0.05), "rebuild_tokens": amt(10_000.0), "carry_ratio": 0.3,
            "fresh_restart_cost_usd": amt(0.1) }],
        "tools": [
            { "category": "read-file", "detail": format!("{secret}.rs"), "project": secret, "calls": 40,
              "first_tokens": amt(1.0), "carried_tokens": amt(1.0), "share": 0.6, "median_chars": 1, "p95_chars": 1,
              "over_25k": 3, "persisted": 0 },
            { "category": "project-tool", "detail": secret, "project": secret, "calls": 4, "first_tokens": amt(1.0),
              "carried_tokens": amt(1.0), "share": 0.4, "median_chars": 1, "p95_chars": 1, "over_25k": 0, "persisted": 0 }
        ],
        "unpriced_models": [],
        "sessions": [{ "provider": "claude-code", "project": secret, "session": secret, "start_ms": w.from_ms() + 1,
            "turns": 120, "mean_ctx": 60_000.0, "peak_ctx": 140_000, "cost_usd": amt(4.0), "compactions": 1 }]
    }))
    .expect("test snapshot")
}

fn day(w: Week, d: i64, provider: Option<&str>, model: Option<&str>, cost: f64) -> DailyRow {
    serde_json::from_value(json!({
        "provider": provider, "day_ms": w.from_ms() + d * 86_400_000, "project": null, "model": model,
        "fresh_tokens": 400_000, "cache_read_tokens": 9_000_000, "cache_write_tokens": 300_000,
        "cost_usd": amt(cost), "sessions": 3, "agents": 2, "compactions": 1, "output_tokens": 50_000,
        "agent_cost_usd": cost / 4.0
    }))
    .expect("test day")
}

fn inputs(snap: &Snapshot, w: Week) -> Inputs<'_> {
    Inputs {
        snap,
        days: (0..5).map(|d| day(w, d, None, None, 6.0)).collect(),
        by_model: vec![
            day(
                w,
                0,
                None,
                Some("claude-opus-4-1-secret-acme-payroll"),
                24.0,
            ),
            day(w, 1, None, Some("claude-sonnet-4-5"), 6.0),
        ],
        by_provider: vec![
            day(w, 0, Some("claude-code"), None, 30.0),
            day(w, 1, Some("secret-acme-payroll"), None, 1.0),
        ],
        langs: vec![(1, 40, 90), (2, 10, 30), (22, 0, 50), (200, 99, 99)],
    }
}

fn strings_are_closed(v: &Value) {
    let fixed: Vec<&str> = FIELDS
        .iter()
        .map(|f| f.0)
        .chain(APP_VERSIONS.iter().copied())
        .chain(OS.iter().copied())
        .chain(PROVIDERS.iter().copied())
        .chain(FAMILIES.iter().copied())
        .collect();
    let rules: Vec<String> = RULES.map(|n| format!("R{n}")).collect();
    let ok = |s: &str| {
        fixed.contains(&s)
            || rules.iter().any(|r| r == s)
            || is_hex_id(s)
            || s == NOT_SHARED
            || Week::parse(s).is_some()
    };
    match v {
        Value::String(s) => assert!(ok(s), "free string in payload: {s}"),
        Value::Array(a) => a.iter().for_each(strings_are_closed),
        Value::Object(m) => m.iter().for_each(|(k, x)| {
            assert!(ok(k), "unknown key {k}");
            strings_are_closed(x);
        }),
        _ => {}
    }
}

#[test]
fn built_payload_passes_the_worker_checks_and_carries_no_name() {
    let w = Week::last_complete(tokenwhy_core::pipeline::now_ms());
    let snap = snapshot(w);
    let p = assemble(&inputs(&snap, w), w, Some(ID));
    let body: Value = serde_json::from_str(&p.to_json()).unwrap();
    assert_eq!(validate(&body, w), Ok(()));
    strings_are_closed(&body);
    assert!(!p.to_json().contains("secret"), "{}", p.to_json());
    assert!(!p.to_json().contains("pdf"));
    assert_eq!(body["rules"], json!(["R1", "R7"]));
    assert_eq!(body["providers"], json!(["claude-code"]));
    assert_eq!(
        (body["lang_1"].clone(), body["lang_3"].clone()),
        (json!(1), json!(22))
    );
    assert_eq!(body["model_1"], json!("claude-opus"));
    assert_eq!(
        (body["sessions"].clone(), body["cost_cents"].clone()),
        (json!(10), json!(2500))
    );
    let pretty: Value = serde_json::from_str(&p.to_pretty()).unwrap();
    assert_eq!(pretty, body);
}

#[test]
fn empty_week_is_valid_and_tampering_is_caught() {
    let w = Week::last_complete(tokenwhy_core::pipeline::now_ms());
    let snap = snapshot(w);
    let mut i = inputs(&snap, w);
    i.days.clear();
    let body: Value = serde_json::to_value(assemble(&i, w, Some(ID))).unwrap();
    assert_eq!(body["active_days"], json!(0));
    assert_eq!(validate(&body, w), Ok(()));

    let full: Value = serde_json::to_value(assemble(&inputs(&snap, w), w, Some(ID))).unwrap();
    let tamper = |k: &str, v: Value| {
        let mut b = full.clone();
        b[k] = v;
        validate(&b, w)
    };
    assert_eq!(tamper("project", json!("acme")), Err("project".into()));
    assert_eq!(tamper("sessions", json!(3)), Err("sessions".into()));
    assert_eq!(tamper("install", json!(NOT_SHARED)), Err("install".into()));
    assert_eq!(tamper("week", json!(w.back(6).label())), Err("week".into()));
    assert_eq!(tamper("rules", json!(["R1", "R1"])), Err("rules".into()));
}

#[test]
fn this_build_version_is_one_the_worker_accepts() {
    assert!(APP_VERSIONS.contains(&app_version().as_str()));
}
