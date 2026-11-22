// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Field numbers from codeburn v0.9.27 docs/providers/antigravity.md; expected values summed by hand from the encoded rows.

use super::pb::enc;
use super::*;
use rusqlite::Connection;

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/tmp/agy-unit")
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("conversations")).unwrap();
    std::env::set_var(
        "TOKENWHY_SQLITE_TMP",
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp/sqlite-copies"),
    );
    d
}

struct Gen<'a> {
    input: u64,
    output: Option<u64>,
    thinking: u64,
    response: u64,
    cache_write: u64,
    cache_read: u64,
    rid: Option<&'a str>,
    model: &'a str,
    model_enum: Option<&'a str>,
    created_s: Option<u64>,
    steps: &'a [u64],
}

fn gen(g: &Gen<'_>) -> Vec<u8> {
    let mut u = vec![];
    enc::uint(2, g.input, &mut u);
    if let Some(o) = g.output {
        enc::uint(3, o, &mut u);
    }
    enc::uint(4, g.cache_write, &mut u);
    enc::uint(5, g.cache_read, &mut u);
    enc::uint(9, g.thinking, &mut u);
    enc::uint(10, g.response, &mut u);
    if let Some(r) = g.rid {
        enc::bytes(11, r.as_bytes(), &mut u);
    }
    let mut chat = vec![];
    enc::bytes(4, &u, &mut chat);
    if let Some(s) = g.created_s {
        let mut ts = vec![];
        enc::uint(1, s, &mut ts);
        let mut start = vec![];
        enc::bytes(4, &ts, &mut start);
        enc::bytes(9, &start, &mut chat);
    }
    enc::bytes(19, g.model.as_bytes(), &mut chat);
    if let Some(e) = g.model_enum {
        let mut kv = vec![];
        enc::bytes(1, b"model_enum", &mut kv);
        enc::bytes(2, e.as_bytes(), &mut kv);
        enc::bytes(20, &kv, &mut chat);
    }
    let mut root = vec![];
    enc::bytes(1, &chat, &mut root);
    let mut packed = vec![];
    for s in g.steps {
        enc::varint(*s, &mut packed);
    }
    enc::bytes(2, &packed, &mut root);
    root
}

fn step(secs: u64, tools: &[(&str, &str)]) -> Vec<u8> {
    let mut m = vec![];
    let mut ts = vec![];
    enc::uint(1, secs, &mut ts);
    enc::bytes(1, &ts, &mut m);
    for (i, (name, args)) in tools.iter().enumerate() {
        let mut t = vec![];
        enc::bytes(1, format!("call-{secs}-{i}").as_bytes(), &mut t);
        enc::bytes(2, name.as_bytes(), &mut t);
        enc::bytes(3, args.as_bytes(), &mut t);
        enc::bytes(4, &t, &mut m);
    }
    m
}

fn fill(c: &Connection) {
    c.execute_batch(
        "CREATE TABLE gen_metadata (idx INTEGER PRIMARY KEY, data BLOB);
         CREATE TABLE steps (idx INTEGER PRIMARY KEY, step_type INTEGER, status INTEGER, metadata BLOB);",
    )
    .unwrap();
    let rows = [
        gen(&Gen {
            input: 1000,
            output: Some(300),
            thinking: 100,
            response: 200,
            cache_write: 50,
            cache_read: 4000,
            rid: Some("r1"),
            model: "gemini-3.8-flash",
            model_enum: Some("MODEL_PLACEHOLDER_M999"),
            created_s: Some(1_790_000_000),
            steps: &[1, 2],
        }),
        gen(&Gen {
            input: 1000,
            output: Some(300),
            thinking: 100,
            response: 200,
            cache_write: 50,
            cache_read: 4000,
            rid: Some("r1"),
            model: "gemini-3.8-flash",
            model_enum: None,
            created_s: Some(1_790_000_000),
            steps: &[1, 2],
        }),
        gen(&Gen {
            input: 0,
            output: None,
            thinking: 0,
            response: 0,
            cache_write: 0,
            cache_read: 9,
            rid: Some("r0"),
            model: "gemini-3.8-flash",
            model_enum: None,
            created_s: None,
            steps: &[],
        }),
        gen(&Gen {
            input: 200,
            output: None,
            thinking: 10,
            response: 40,
            cache_write: 0,
            cache_read: 0,
            rid: None,
            model: "gemini-pro-default",
            model_enum: Some("MODEL_PLACEHOLDER_M35"),
            created_s: None,
            steps: &[3],
        }),
    ];
    for (i, r) in rows.iter().enumerate() {
        c.execute(
            "INSERT INTO gen_metadata VALUES (?1, ?2)",
            rusqlite::params![i as i64, r],
        )
        .unwrap();
    }
    let steps = [
        (
            1,
            step(
                1_790_000_001,
                &[(
                    "view_file",
                    r#"{"AbsolutePath":"C:\\p\\skills\\foo\\SKILL.md"}"#,
                )],
            ),
        ),
        (
            2,
            step(
                1_790_000_002,
                &[("run_command", r#"{"CommandLine":"git status"}"#)],
            ),
        ),
        (
            3,
            step(
                1_790_000_100,
                &[
                    ("send_message", "{}"),
                    ("call_mcp_tool", r#"{"ServerName":"gh","ToolName":"x"}"#),
                ],
            ),
        ),
    ];
    for (i, m) in steps {
        c.execute(
            "INSERT INTO steps VALUES (?1, 0, 3, ?2)",
            rusqlite::params![i, m],
        )
        .unwrap();
    }
}

fn src(path: PathBuf) -> SourceFile {
    source(&path, None).unwrap()
}

fn check(r: &FileRecords) {
    assert_eq!(r.turns.len(), 2);
    let sum = |f: fn(&Usage) -> u64| r.turns.iter().map(|t| f(&t.usage)).sum::<u64>();
    assert_eq!(sum(|u| u.input), 1200);
    assert_eq!(sum(|u| u.output), 350);
    assert_eq!(sum(|u| u.cache_read), 4000);
    assert_eq!(sum(|u| u.cache_write_5m), 50);
    let ts: Vec<i64> = r.turns.iter().map(|t| t.first_ts_ms).collect();
    assert_eq!(ts, [1_790_000_000_000, 1_790_000_100_000]);
    let models: Vec<&str> = r
        .turns
        .iter()
        .map(|t| r.strings[t.model as usize].as_str())
        .collect();
    assert_eq!(models, ["gemini-3.8-flash", "claude-sonnet-4-6"]);
    let cats: Vec<Category> = r.tool_calls.iter().map(|c| c.category).collect();
    assert_eq!(cats, [Category::ReadFile, Category::Vcs, Category::Mcp]);
    assert_eq!(r.tool_calls[2].turn_order, r.turns[1].order);
    assert_eq!(r.skill_loads.len(), 1);
    assert_eq!(r.strings[r.skill_loads[0].skill as usize], "foo");
}

#[test]
fn usage_tools_and_times_from_encoded_rows() {
    let d = tmp("plain");
    let db = d.join("conversations/c1.db");
    fill(&Connection::open(&db).unwrap());
    let before: Vec<_> = std::fs::read_dir(d.join("conversations"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    let r = parse(&src(db)).unwrap();
    check(&r);
    let after: Vec<_> = std::fs::read_dir(d.join("conversations"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert_eq!(
        before, after,
        "parse must not create files next to the agent's database"
    );
}

#[test]
fn rows_still_in_the_wal_are_read() {
    let d = tmp("wal");
    let db = d.join("conversations/c2.db");
    let c = Connection::open(&db).unwrap();
    c.pragma_update(None, "journal_mode", "WAL").unwrap();
    c.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
    fill(&c);
    assert!(d.join("conversations/c2.db-wal").metadata().unwrap().len() > 0);
    let r = parse(&src(db.clone())).unwrap();
    check(&r);
    let fp = Antigravity.fingerprint(&src(db)).unwrap();
    assert_eq!(fp.change, Change::Whole);
    drop(c);
}

#[test]
fn watcher_and_skips_stay_in_the_conversations_folder() {
    let d = tmp("paths");
    std::fs::write(d.join("conversations/a.pb"), b"x").unwrap();
    std::fs::write(d.join("conversations/b.pb"), b"x").unwrap();
    let p = Antigravity;
    assert_eq!(p.watch_dirs(&d), vec![(d.join("conversations"), false)]);
    assert!(p.accepts(&d, &d.join("conversations/x.db-wal")));
    assert!(!p.accepts(&d, &d.join("brain/x/y.db")));
    assert_eq!(p.skipped(&d)[0].count, 2);
}
