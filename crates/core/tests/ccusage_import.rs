// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Expected values from the hand-written exports in tests/fixtures/ccusage.

use std::fs;
use std::path::{Path, PathBuf};
use tokenwhy_core::history::By;
use tokenwhy_core::import;
use tokenwhy_core::pipeline::{self, Pipeline};
use tokenwhy_core::*;

fn fx(p: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(p)
}

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn day(date: &str) -> i64 {
    use chrono::{Local, NaiveDate, TimeZone};
    let d = NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap();
    Local
        .from_local_datetime(&d.and_hms_opt(0, 0, 0).unwrap())
        .earliest()
        .unwrap()
        .timestamp_millis()
}

fn rows(pipe: &Pipeline) -> Vec<(String, i64, u64, u64, f64)> {
    pipe.history()
        .daily(0, i64::MAX, By::Provider, None, None)
        .into_iter()
        .map(|r| {
            let c = (r.cost_usd.value.unwrap_or(0.0) * 1000.0).round() / 1000.0;
            (
                r.provider.unwrap().0,
                r.day_ms,
                r.fresh_tokens,
                r.cache_read_tokens,
                c,
            )
        })
        .collect()
}

fn imported(pipe: &Pipeline) -> Vec<(String, i64, u64, u64, f64)> {
    rows(pipe)
        .into_iter()
        .filter(|r| r.0.starts_with(import::PREFIX))
        .collect()
}

#[test]
fn native_day_wins_imported_fills_gaps_and_reimport_replaces() {
    let store = tmp("ccusage");
    let mut pipe = Pipeline::with_providers(provider::all(), Some(store.clone()));
    let roots = pipe
        .roots(&[(Some("claude-code".into()), fx("claude/resume_copy"))])
        .unwrap();
    pipe.scan(&roots, &|_| {}).unwrap();
    let native_21 = rows(&pipe)
        .into_iter()
        .any(|r| r.0 == "claude-code" && r.1 == day("2026-10-21"));
    assert!(native_21, "the fixture gives claude-code a day on 21/10");

    let n = pipe.import_ccusage(&fx("ccusage/by-agent.json")).unwrap();
    assert_eq!(
        n, 4,
        "two claude models on 20/10, one claude row on 21/10, one amp row"
    );
    let want = vec![
        ("import:amp".to_string(), day("2026-10-21"), 44, 400, 0.5),
        (
            "import:claude".to_string(),
            day("2026-10-20"),
            185,
            1200,
            1.75,
        ),
    ];
    let mut got = imported(&pipe);
    got.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(got, want);

    pipe.import_ccusage(&fx("ccusage/by-agent.json")).unwrap();
    let mut again = imported(&pipe);
    again.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(again, want);

    pipe.import_ccusage(&fx("ccusage/newer.json")).unwrap();
    let mut newer = imported(&pipe);
    newer.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        newer,
        vec![
            ("import:amp".to_string(), day("2026-10-21"), 44, 400, 0.5),
            ("import:claude".to_string(), day("2026-10-20"), 600, 0, 3.0),
        ]
    );

    pipe.save_cache().unwrap();
    let mut reopened = Pipeline::with_providers(provider::all(), Some(store));
    assert_eq!(imported(&reopened).len(), 2);
    reopened.use_imports(false);
    assert!(imported(&reopened).is_empty());

    let now = pipeline::now_ms();
    let s = pipe
        .report(
            &Settings::default(),
            &pipeline::query(None, now, None, AnonLevel::Off),
            now,
        )
        .0;
    let imp: Vec<&str> = s
        .providers
        .iter()
        .filter(|p| p.imported)
        .map(|p| p.label.as_str())
        .collect();
    assert_eq!(imp, ["Amp (ccusage import)", "Claude (ccusage import)"]);
}

#[test]
fn rows_without_agent_are_refused() {
    let text = fs::read_to_string(fx("ccusage/no-agent.json")).unwrap();
    let e = import::parse(&text, 1).unwrap_err();
    assert!(e.contains("--by-agent"), "{e}");
}

#[test]
fn session_rows_count_on_their_last_day() {
    let text = fs::read_to_string(fx("ccusage/sections.json")).unwrap();
    let r = import::parse(&text, 7).unwrap();
    assert_eq!(r.len(), 1);
    let r = &r[0];
    assert_eq!(
        (r.agent.as_str(), r.day_ms, r.model.as_str()),
        ("codex", day("2026-10-19"), "gpt-5.4")
    );
    assert_eq!(
        (r.input + r.cache_write + r.output, r.cache_read, r.cost_usd),
        (990, 9000, Some(0.9))
    );
}
