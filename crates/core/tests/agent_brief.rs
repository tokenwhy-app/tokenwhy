#![cfg(feature = "agent")]
// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.
//! Expected ranks, sums and shares worked out on paper from hand-set findings.

use tokenwhy_core::agent::{brief, markdown, Filter};
use tokenwhy_core::{Amount, Finding, ProviderId, Snapshot, Window};

const DAY: i64 = 86_400_000;

fn amount(v: Option<f64>) -> Amount {
    Amount {
        value: v,
        est: v.is_some(),
    }
}

fn finding(rule: &str, project: Option<&str>, usd: Option<f64>, tokens: Option<f64>) -> Finding {
    Finding {
        provider: Some(ProviderId("claude-code".into())),
        rule: rule.into(),
        project: project.map(str::to_string),
        line: format!("{rule} evidence line"),
        tokens_saved_per_week: amount(tokens),
        usd_saved_per_week: amount(usd),
        co2_g_saved_per_week: Amount::none(),
        usd_saved_last_7d: Amount::none(),
    }
}

fn fixture() -> Snapshot {
    let mut s: Snapshot =
        serde_json::from_str(include_str!("../../../app/web/public/sample-snapshot.json")).unwrap();
    s.window = Window {
        from_ms: 0,
        to_ms: 14 * DAY,
    };
    s.progress = 1.0;
    s.daily = [30.0, 10.0]
        .iter()
        .enumerate()
        .map(|(i, c)| {
            serde_json::from_value(serde_json::json!({
                "day_ms": i as i64 * DAY, "project": "alpha", "fresh_tokens": 0, "cache_read_tokens": 0,
                "cache_write_tokens": 0, "cost_usd": {"value": c}, "sessions": 1, "agents": 0, "compactions": 0
            }))
            .unwrap()
        })
        .collect();
    s.findings = vec![
        finding("R11", None, Some(3.0), Some(6_000_000.0)),
        finding("R17", Some("alpha"), Some(1.5), Some(3_000_000.0)),
        finding("R17", Some("alpha"), Some(0.5), Some(1_000_000.0)),
        finding("R3", Some("alpha"), Some(5.0), Some(1_200_000.0)),
        finding("R9", None, None, None),
    ];
    s
}

#[test]
fn ranked_by_dollars_with_share_of_spend() {
    let b = brief(&fixture(), &Filter::default());
    assert_eq!(b.spend_usd, Some(40.0));
    assert_eq!(b.spend_usd_per_week, Some(20.0));
    assert_eq!(b.saved_usd_per_week, 10.0);
    let got: Vec<_> = b
        .items
        .iter()
        .map(|g| {
            (
                g.rule.as_str(),
                g.usd_saved_per_week,
                g.share_of_spend,
                g.findings,
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            ("R3", Some(5.0), Some(0.25), 1),
            ("R11", Some(3.0), Some(0.15), 1),
            ("R17", Some(2.0), Some(0.1), 2),
            ("R9", None, None, 1),
        ]
    );
    assert_eq!(b.items[2].tokens_saved_per_week, Some(4_000_000.0));
    assert!(b.items[0].fix.contains("enabledPlugins"));
}

#[test]
fn limit_and_project_filter() {
    let b = brief(
        &fixture(),
        &Filter {
            project: Some("alpha".into()),
            limit: 1,
            ..Default::default()
        },
    );
    assert_eq!(b.items.len(), 1);
    assert_eq!(b.more, 1);
    assert_eq!(b.saved_usd_per_week, 7.0);
}

#[test]
fn markdown_stays_short() {
    let md = markdown(&brief(&fixture(), &Filter::default()));
    assert!(md.contains("$20.00/week"));
    assert!(md.contains("## 1. R3"));
    assert!(md.lines().count() <= 60, "{md}");
    assert!(!md.contains('\u{2014}'));
}
