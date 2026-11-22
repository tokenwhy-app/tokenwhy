// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

#![cfg(feature = "claude-stats")]

use std::path::{Path, PathBuf};
use tokenwhy_core::history::By;
use tokenwhy_core::pipeline::Pipeline;
use tokenwhy_core::*;

fn day(date: &str) -> i64 {
    use chrono::{Local, NaiveDate, TimeZone};
    let d = NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap();
    Local
        .from_local_datetime(&d.and_hms_opt(0, 0, 0).unwrap())
        .earliest()
        .unwrap()
        .timestamp_millis()
}

// Rates from prices/anthropic.json; totals worked out by hand.
#[test]
fn fills_missing_days_estimated_and_own_day_wins() {
    let root: PathBuf =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude-stats/config/projects");
    let mut pipe = Pipeline::with_providers(provider::all(), None);
    let roots = pipe.roots(&[(Some("claude-code".into()), root)]).unwrap();
    pipe.scan(&roots, &|_| {}).unwrap();
    let rows = pipe.history().daily(0, i64::MAX, By::Provider, None, None);

    let d20 = rows
        .iter()
        .find(|r| r.day_ms == day("2026-10-20"))
        .expect("20/10 filled");
    assert_eq!(d20.provider.as_ref().unwrap().0, "claude-code");
    assert_eq!(d20.fresh_tokens, 500_000);
    assert_eq!(d20.cache_read_tokens, 1_000_000);
    assert!((d20.cost_usd.value.unwrap() - 6.17).abs() < 1e-9);
    assert!(d20.cost_usd.est, "stats-cache days are estimates");
    assert_eq!(d20.sessions, 0);

    let d21 = rows
        .iter()
        .find(|r| r.day_ms == day("2026-10-21"))
        .expect("own day");
    assert!(
        d21.fresh_tokens < 1_000,
        "own transcript day, not the file's 2M tokens"
    );
    assert!(!d21.cost_usd.est);
}
