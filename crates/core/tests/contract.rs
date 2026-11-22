// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use serde_json::Value;
use tokenwhy_core::{CompactView, CostSource, Level, Settings, Snapshot};

#[test]
fn sample_snapshot_matches_model() {
    let json = include_str!("../../../app/web/public/sample-snapshot.json");
    let snap: Snapshot =
        serde_json::from_str(json).expect("sample-snapshot.json fits model::Snapshot");
    assert_eq!(snap.schema, tokenwhy_core::SNAPSHOT_SCHEMA_VERSION);
}

#[test]
fn sample_providers_keep_labels() {
    let snap: Snapshot =
        serde_json::from_str(include_str!("../../../app/web/public/sample-snapshot.json")).unwrap();
    let ids: Vec<&str> = snap.providers.iter().map(|p| p.id.0.as_str()).collect();
    assert_eq!(ids, ["claude-code", "codex", "agy", "import:amp"]);
    assert!(snap.providers[3].imported && !snap.providers[2].imported);
    assert_eq!(snap.providers[2].caps.cost, CostSource::Estimate);
    assert_eq!(snap.providers[1].caps.startup, Level::None);
    assert_eq!(snap.providers[1].skipped[0].count, 4);
    let by_rule = |r: &str| snap.findings.iter().find(|f| f.rule == r).unwrap();
    assert_eq!(by_rule("R11").provider.as_ref().unwrap().0, "codex");
    assert!(by_rule("R2").provider.is_none());
    assert_eq!(snap.tools[0].provider.as_ref().unwrap().0, "codex");
}

#[test]
fn schema_declares_every_written_key() {
    let schema: Value =
        serde_json::from_str(include_str!("../../../app/web/snapshot.schema.json")).unwrap();
    let snap: Snapshot =
        serde_json::from_str(include_str!("../../../app/web/public/sample-snapshot.json")).unwrap();
    let out = serde_json::to_value(&snap).unwrap();
    assert_eq!(schema["properties"]["schema"]["const"], out["schema"]);
    let declared = |props: &Value, v: &Value, at: &str| {
        for k in v.as_object().unwrap().keys() {
            assert!(props.get(k).is_some(), "{at}.{k} missing from the schema");
        }
    };
    declared(&schema["properties"], &out, "snapshot");
    let defs = &schema["$defs"];
    declared(
        &defs["providerInfo"]["properties"],
        &out["providers"][1],
        "providerInfo",
    );
    for (field, def) in [
        ("findings", "finding"),
        ("skills", "skillRow"),
        ("memory", "memoryRow"),
        ("tools", "toolRow"),
    ] {
        for row in out[field].as_array().unwrap() {
            declared(&defs[def]["properties"], row, def);
        }
    }
}

// An empty or older settings file gives view C and no always-on-top.
#[test]
fn empty_settings_take_owner_defaults() {
    let s: Settings = serde_json::from_str("{}").unwrap();
    assert_eq!(s.compact_view, CompactView::C);
    assert!(!s.always_on_top);
}
