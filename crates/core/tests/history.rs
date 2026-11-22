// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use chrono::{Local, NaiveDate, TimeZone};
use std::fs;
use std::path::{Path, PathBuf};
use tokenwhy_core::history::By;
use tokenwhy_core::pipeline::Pipeline;
use tokenwhy_core::provider;

fn line(id: &str, ts: &str, input: u64, write: u64, read: u64, output: u64) -> String {
    format!(
        r#"{{"type": "assistant", "timestamp": "{ts}", "uuid": "u-{id}", "cwd": "D:\\work\\demo", "message": {{"id": "{id}", "model": "claude-opus-5", "role": "assistant", "content": [{{"type": "text", "text": "ok"}}], "usage": {{"input_tokens": {input}, "cache_creation_input_tokens": {write}, "cache_read_input_tokens": {read}, "output_tokens": {output}}}}}}}"#
    ) + "\n"
}

fn local_day(y: i32, m: u32, d: u32) -> i64 {
    let n = NaiveDate::from_ymd_opt(y, m, d)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap();
    Local
        .from_local_datetime(&n)
        .earliest()
        .unwrap()
        .timestamp_millis()
}

fn scan(store: &Path, root: &Path) -> Pipeline {
    let mut pipe = Pipeline::with_providers(provider::all(), Some(store.to_path_buf()));
    let roots = pipe
        .roots(&[(Some("claude-code".into()), root.to_path_buf())])
        .unwrap();
    pipe.scan(&roots, &|_| {}).unwrap();
    pipe.save_cache().unwrap();
    pipe
}

#[test]
fn deleted_transcript_keeps_its_day() {
    let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("history_deleted");
    let _ = fs::remove_dir_all(&base);
    let (root, store) = (base.join("root"), base.join("store"));
    let proj = root.join("demo");
    fs::create_dir_all(&proj).unwrap();
    // Noon UTC keeps each turn on its date in any time zone from UTC-11 to UTC+11.
    let old = proj.join("old.jsonl");
    fs::write(
        &old,
        line("m1", "2026-10-01T12:00:00.000Z", 10, 200, 3000, 40)
            + &line("m2", "2026-10-01T12:05:00.000Z", 5, 0, 3200, 60),
    )
    .unwrap();
    fs::write(
        proj.join("new.jsonl"),
        line("m3", "2026-10-02T12:00:00.000Z", 1, 100, 500, 9),
    )
    .unwrap();

    let (d1, d2) = (local_day(2026, 10, 1), local_day(2026, 10, 2));
    let before = scan(&store, &root)
        .history()
        .daily(0, i64::MAX, By::Project, None, None);
    fs::remove_file(&old).unwrap();
    let after = scan(&store, &root)
        .history()
        .daily(0, i64::MAX, By::Project, None, None);
    assert_eq!(before, after);

    let r1 = after
        .iter()
        .find(|r| r.day_ms == d1)
        .expect("day of the deleted transcript");
    assert_eq!(
        (r1.fresh_tokens, r1.cache_read_tokens, r1.cache_write_tokens),
        (315, 6200, 200)
    );
    assert_eq!((r1.sessions, r1.agents), (1, 0));
    assert!(r1.cost_usd.value.is_some_and(|c| c > 0.0));
    let r2 = after.iter().find(|r| r.day_ms == d2).unwrap();
    assert_eq!(
        (r2.fresh_tokens, r2.cache_read_tokens, r2.cache_write_tokens),
        (110, 500, 100)
    );
    assert_eq!(after.len(), 2);
}

#[test]
fn week_cost_per_output_counts_agent_cost_but_main_output_only() {
    let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("history_week_unit");
    let _ = fs::remove_dir_all(&base);
    let (root, store) = (base.join("root"), base.join("store"));
    let proj = root.join("demo");
    fs::create_dir_all(&proj).unwrap();
    // Opus 5 list price: $5 per M input, $25 per M output; costs worked out by hand.
    fs::write(
        proj.join("last.jsonl"),
        line("w1", "2026-10-03T12:00:00.000Z", 2000, 0, 0, 400),
    )
    .unwrap();
    fs::write(
        proj.join("this.jsonl"),
        line("w2", "2026-10-10T12:00:00.000Z", 0, 0, 0, 800),
    )
    .unwrap();
    fs::write(
        proj.join("agent-a1.jsonl"),
        line("w3", "2026-10-10T12:30:00.000Z", 0, 0, 0, 400),
    )
    .unwrap();

    let w =
        scan(&store, &root)
            .history()
            .week(local_day(2026, 10, 15) + 12 * 3_600_000, None, None);
    let out = w.output_tokens.expect("output recorded");
    assert_eq!((out.this, out.last), (800.0, 400.0));
    let per_k = |cost: f64, out: f64| cost / (out / 1000.0);
    assert!(
        (per_k(w.cost_usd.this, out.this) - 0.0375).abs() < 1e-9,
        "{}",
        w.cost_usd.this
    );
    assert!(
        (per_k(w.cost_usd.last, out.last) - 0.05).abs() < 1e-9,
        "{}",
        w.cost_usd.last
    );
    assert!((w.agent_cost_usd.this - 0.01).abs() < 1e-9);
    let h = w.active_hours.unwrap();
    assert_eq!((h.this, h.last), (1.0, 1.0));
}

#[test]
fn v2_store_migrates_without_losing_or_doubling_days() {
    use std::collections::BTreeMap;
    use tokenwhy_core::metrics::days::DayPart;
    #[derive(serde::Deserialize)]
    struct E3 {
        provider: String,
        session: u64,
        project: String,
        main: bool,
        floor_ms: i64,
        days: Vec<DayPart>,
        #[allow(dead_code)]
        account: Option<String>,
        #[allow(dead_code)]
        last_ms: Option<i64>,
        #[allow(dead_code)]
        turns: Option<u32>,
    }
    #[derive(serde::Serialize)]
    struct P2 {
        day_ms: i64,
        model: String,
        fresh: u64,
        cache_read: u64,
        cache_write: u64,
        cost_usd: f64,
        cost_known: bool,
        compactions: u32,
        output: Option<u64>,
        edits: Option<u32>,
        hours: Option<u32>,
    }
    #[derive(serde::Deserialize)]
    struct D3 {
        magic: [u8; 8],
        version: u32,
        files: BTreeMap<u64, E3>,
    }
    #[derive(serde::Serialize)]
    struct E2 {
        session: u64,
        project: String,
        main: bool,
        floor_ms: i64,
        days: Vec<P2>,
    }
    #[derive(serde::Serialize)]
    struct D2 {
        magic: [u8; 8],
        version: u32,
        files: BTreeMap<u64, E2>,
    }
    let fnv = |s: &[u8]| {
        s.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
            (h ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3)
        })
    };

    let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("history_v2");
    let _ = fs::remove_dir_all(&base);
    let (root, a, b) = (base.join("root"), base.join("a"), base.join("b"));
    let proj = root.join("demo");
    fs::create_dir_all(&proj).unwrap();
    let t = proj.join("t.jsonl");
    fs::write(
        &t,
        line("m1", "2026-10-01T12:00:00.000Z", 10, 200, 3000, 40),
    )
    .unwrap();
    let want = scan(&a, &root)
        .history()
        .daily(0, i64::MAX, By::Project, None, None);
    assert_eq!(want.len(), 1);

    let v3: D3 = postcard::from_bytes(&fs::read(a.join("history.bin")).unwrap()).unwrap();
    assert_eq!((v3.magic, v3.version), (*b"TWHIST01", 6));
    let files = v3
        .files
        .into_values()
        .map(|e| {
            assert_eq!(e.provider, "claude-code");
            let key = fnv(t.to_string_lossy().as_bytes());
            let e2 = E2 {
                session: e.session,
                project: e.project,
                main: e.main,
                floor_ms: e.floor_ms,
                days: e
                    .days
                    .into_iter()
                    .map(|d| P2 {
                        day_ms: d.day_ms,
                        model: d.model,
                        fresh: d.fresh,
                        cache_read: d.cache_read,
                        cache_write: d.cache_write,
                        cost_usd: d.cost_usd,
                        cost_known: d.cost_known,
                        compactions: d.compactions,
                        output: d.output,
                        edits: d.edits,
                        hours: d.hours,
                    })
                    .collect(),
            };
            (key, e2)
        })
        .collect();
    let v2 = postcard::to_allocvec(&D2 {
        magic: *b"TWHIST01",
        version: 2,
        files,
    })
    .unwrap();
    fs::create_dir_all(&b).unwrap();
    fs::write(b.join("history.bin"), &v2).unwrap();

    let got = scan(&b, &root)
        .history()
        .daily(0, i64::MAX, By::Project, None, None);
    assert_eq!(got, want);
    assert!(asides(&b, 2).contains(&v2));

    fs::write(b.join("history.bin"), &v2).unwrap();
    fs::remove_file(&t).unwrap();
    let kept = scan(&b, &root)
        .history()
        .daily(0, i64::MAX, By::Project, None, None);
    assert_eq!(kept, want);
}

#[test]
fn languages_last_turn_and_turns_reach_the_history() {
    let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("history_langs");
    let _ = fs::remove_dir_all(&base);
    let (root, store) = (base.join("root"), base.join("store"));
    let proj = root.join("demo");
    fs::create_dir_all(&proj).unwrap();
    let call = |name: &str, path: &str| {
        format!(
            r#"{{"type": "tool_use", "id": "t-{path}", "name": "{name}", "input": {{"file_path": "D:/work/demo/{path}"}}}}"#
        )
    };
    let calls = [
        call("Edit", "a.rs"),
        call("Edit", "b.rs"),
        call("Edit", "c.ts"),
        call("Write", "Cargo.toml"),
        call("Read", "x.unknownext"),
    ]
    .join(", ");
    let tools = format!(
        r#"{{"type": "assistant", "timestamp": "2026-10-01T12:10:00.000Z", "uuid": "u-m2", "cwd": "D:/work/demo", "message": {{"id": "m2", "model": "claude-opus-5", "role": "assistant", "content": [{calls}], "usage": {{"input_tokens": 5, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 100, "output_tokens": 20}}}}}}"#
    ) + "\n";
    fs::write(
        proj.join("s.jsonl"),
        line("m1", "2026-10-01T12:00:00.000Z", 10, 200, 3000, 40) + &tools,
    )
    .unwrap();
    let pipe = scan(&store, &root);
    let views = pipe.history().entries_for(0, i64::MAX, None, None);
    assert_eq!(views.len(), 1);
    let v = &views[0];
    let name = |id: u8| tokenwhy_core::lang::name(id);
    let langs: Vec<(&str, u32, u32)> = v
        .days
        .iter()
        .flat_map(|d| d.langs.clone().unwrap_or_default())
        .map(|(id, e, r)| (name(id), e, r))
        .collect();
    assert_eq!(
        langs,
        [("Other", 0, 1), ("Rust", 3, 0), ("TypeScript", 1, 0)]
    );
    assert_eq!(v.turns, Some(2));
    assert_eq!(v.last_ms.map(|l| l - v.floor_ms), Some(10 * 60_000));
    let bin = fs::read(store.join("history.bin")).unwrap();
    for needle in [&b"unknownext"[..], b"Cargo", b".rs", b"demo/"] {
        assert!(!bin.windows(needle.len()).any(|w| w == needle));
    }
}

/// A real v2 or v4 store, on a copy only: `TOKENWHY_V4_COPY=<file> cargo test --test history -- --ignored`.
#[test]
#[ignore]
fn real_store_migrates_with_equal_daily_totals() {
    use std::collections::BTreeMap;
    #[derive(serde::Deserialize)]
    struct P4 {
        day_ms: i64,
        _model: String,
        fresh: u64,
        cache_read: u64,
        cache_write: u64,
        cost_usd: f64,
        _cost_known: bool,
        compactions: u32,
        _output: Option<u64>,
        _edits: Option<u32>,
        _hours: Option<u32>,
    }
    #[derive(serde::Deserialize)]
    struct E4 {
        _provider: String,
        _session: u64,
        _project: String,
        _main: bool,
        _floor_ms: i64,
        days: Vec<P4>,
        _account: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct E2 {
        _session: u64,
        _project: String,
        _main: bool,
        _floor_ms: i64,
        days: Vec<P4>,
    }
    #[derive(serde::Deserialize)]
    struct D<E> {
        _magic: [u8; 8],
        _version: u32,
        files: BTreeMap<u64, E>,
    }
    let src = PathBuf::from(std::env::var("TOKENWHY_V4_COPY").expect("TOKENWHY_V4_COPY"));
    let v4 = fs::read(&src).unwrap();
    let (_, version): ([u8; 8], u32) = postcard::take_from_bytes(&v4).unwrap().0;
    let days: Vec<P4> = match version {
        4 => postcard::from_bytes::<D<E4>>(&v4)
            .unwrap()
            .files
            .into_values()
            .flat_map(|e| e.days)
            .collect(),
        2 => postcard::from_bytes::<D<E2>>(&v4)
            .unwrap()
            .files
            .into_values()
            .flat_map(|e| e.days)
            .collect(),
        v => panic!("version {v}"),
    };
    let mut want: BTreeMap<i64, (u64, u64, u64, i64, u32)> = BTreeMap::new();
    for d in &days {
        let w = want.entry(d.day_ms).or_default();
        w.0 += d.fresh;
        w.1 += d.cache_read;
        w.2 += d.cache_write;
        w.3 += (d.cost_usd * 1e6).round() as i64;
        w.4 += d.compactions;
    }
    let dir = src.parent().unwrap().join("migrated");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("history.bin"), &v4).unwrap();
    let h = tokenwhy_core::history::History::open(Some(&dir));
    let got: BTreeMap<i64, (u64, u64, u64, i64, u32)> = h
        .daily(0, i64::MAX, By::None, None, None)
        .iter()
        .map(|r| {
            let c = (r.cost_usd.value.unwrap_or(0.0) * 1e6).round() as i64;
            (
                r.day_ms,
                (
                    r.fresh_tokens,
                    r.cache_read_tokens,
                    r.cache_write_tokens,
                    c,
                    r.compactions,
                ),
            )
        })
        .collect();
    assert_eq!(got.len(), want.len());
    for (day, w) in &want {
        let g = got[day];
        assert_eq!((g.0, g.1, g.2, g.4), (w.0, w.1, w.2, w.4), "day {day}");
        assert!(
            (g.3 - w.3).abs() <= 2 + w.3.abs() / 10_000_000,
            "cost on day {day}: {} vs {}",
            g.3,
            w.3
        );
    }
    assert!(asides(&dir, version).contains(&v4));
    let v5 = fs::metadata(dir.join("history.bin")).unwrap().len();
    eprintln!(
        "v{version} {} bytes, v5 {} bytes, {} days",
        v4.len(),
        v5,
        want.len()
    );
}

fn asides(dir: &Path, v: u32) -> Vec<Vec<u8>> {
    let prefix = format!("history.bin.v{v}.");
    fs::read_dir(dir)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with(&prefix))
        .map(|e| fs::read(e.path()).unwrap())
        .collect()
}

fn downgrade_dir(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../_temp/history-tests")
        .join(name);
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn newer_history_survives_two_older_runs() {
    let base = downgrade_dir("newer");
    let (root, store) = (base.join("root"), base.join("store"));
    fs::create_dir_all(root.join("demo")).unwrap();
    fs::write(
        root.join("demo").join("s.jsonl"),
        line("m1", "2026-03-01T12:00:00Z", 10, 0, 0, 5),
    )
    .unwrap();
    fs::create_dir_all(&store).unwrap();
    let mut newer = b"TWHIST01".to_vec();
    newer.push(7);
    newer.extend_from_slice(b"days the older build cannot read");
    fs::write(store.join("history.bin"), &newer).unwrap();
    for _ in 0..2 {
        let p = scan(&store, &root);
        assert_eq!(p.history().newer(), Some(7));
    }
    assert_eq!(fs::read(store.join("history.bin")).unwrap(), newer);
}

#[test]
fn two_corrupt_histories_keep_both_copies() {
    let store = downgrade_dir("corrupt");
    let mut kept = vec![];
    for tag in [b"first torn write".as_slice(), b"second torn write"] {
        let mut bad = b"TWHIST01".to_vec();
        bad.push(5);
        bad.extend_from_slice(&[0xff; 4]);
        bad.extend_from_slice(tag);
        fs::write(store.join("history.bin"), &bad).unwrap();
        let _ = tokenwhy_core::history::History::open(Some(&store));
        kept.push(bad);
    }
    let mut aside = asides(&store, 5);
    aside.sort();
    kept.sort();
    assert_eq!(aside, kept);
}
