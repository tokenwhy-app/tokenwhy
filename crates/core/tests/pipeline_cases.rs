// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use std::path::{Path, PathBuf};
use tokenwhy_core::pipeline::{self, Pipeline};
use tokenwhy_core::render::{self, Format};
use tokenwhy_core::*;

fn fx(p: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/claude")
        .join(p)
}

fn report(root: &str, level: AnonLevel) -> Snapshot {
    let mut pipe = Pipeline::with_providers(provider::all(), None);
    let roots = pipe
        .roots(&[(Some("claude-code".into()), fx(root))])
        .unwrap();
    pipe.scan(&roots, &|_| {}).unwrap();
    let now = pipeline::now_ms();
    pipe.report(
        &Settings::default(),
        &pipeline::query(None, now, None, level),
        now,
    )
    .0
}

#[test]
fn compaction_after_copied_prefix() {
    let s = report("resume_compaction", AnonLevel::Off);
    let c = s
        .compactions
        .iter()
        .find(|c| c.pre_tokens == 200_000)
        .expect("the kept compaction of b");
    assert_eq!(c.first_after_ctx, 50_000);
    let (before, after) = (
        c.cost_per_turn_before.value.unwrap(),
        c.cost_per_turn_after.value.unwrap(),
    );
    assert!(
        after > 2.0 * before * 0.999,
        "before {before} after {after}"
    );
    assert_eq!(
        s.compactions
            .iter()
            .filter(|c| c.pre_tokens == 100_000)
            .count(),
        1
    );
}

#[test]
fn anon_hides_command_names() {
    let s = report("anon_command", AnonLevel::Strict);
    let md = render::render(&s, Format::Markdown, None);
    assert!(!md.contains("acme"), "{md}");
    let other: Vec<_> = s
        .tools
        .iter()
        .filter(|t| t.category == Category::Other)
        .collect();
    assert_eq!(other.len(), 1);
    assert!(
        other[0]
            .detail
            .as_deref()
            .is_some_and(|d| !d.contains("acme")),
        "{:?}",
        other[0].detail
    );
}
