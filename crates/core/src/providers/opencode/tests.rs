// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Layouts from codeburn v0.9.27 opencode-file-parser.ts and docs/providers/opencode.md; expected values summed by hand.

use super::*;
#[cfg(feature = "sqlite")]
use crate::pipeline::Pipeline;
use serde_json::json;

fn root(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/tmp/opencode-unit")
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    #[cfg(feature = "sqlite")]
    std::env::set_var(
        "TOKENWHY_SQLITE_TMP",
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp/sqlite-copies"),
    );
    d
}

fn put(p: PathBuf, v: Value) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, v.to_string()).unwrap();
}

fn tokens(i: u64, o: u64, r: u64, cr: u64, cw: u64) -> Value {
    json!({"input": i, "output": o, "reasoning": r, "cache": {"read": cr, "write": cw}})
}

fn json_storage(r: &Path) {
    let st = r.join("storage");
    put(
        st.join("session/p1/ses_a.json"),
        json!({"id": "ses_a", "directory": "/home/u/webapp"}),
    );
    put(
        st.join("session/p1/ses_b.json"),
        json!({"id": "ses_b", "parentID": "ses_a", "directory": "/home/u/webapp"}),
    );
    put(
        st.join("message/ses_a/msg_1.json"),
        json!({"id": "msg_1", "role": "user", "time": {"created": 1000}}),
    );
    put(
        st.join("message/ses_a/msg_2.json"),
        json!({"id": "msg_2", "role": "assistant", "modelID": "claude-sonnet-4-5", "providerID": "anthropic",
               "time": {"created": 2000}, "cost": 0.01, "tokens": tokens(100, 50, 25, 1000, 200)}),
    );
    put(
        st.join("part/msg_2/prt_1.json"),
        json!({"type": "tool", "tool": "bash", "callID": "call_1",
               "state": {"status": "completed", "input": {"command": "cargo test"}, "output": "ok 3 passed"}}),
    );
    put(
        st.join("message/ses_a/msg_3.json"),
        json!({"id": "msg_3", "role": "assistant", "modelID": "gpt-5.4", "providerID": "openai",
               "time": {"created": 3000}, "cost": 0, "tokens": tokens(10, 5, 0, 2000, 0)}),
    );
    put(
        st.join("message/ses_b/msg_4.json"),
        json!({"id": "msg_4", "role": "assistant", "modelID": "gpt-5.4", "providerID": "openai",
               "time": {"created": 2500}, "tokens": tokens(7, 3, 0, 0, 0)}),
    );
}

#[cfg(feature = "sqlite")]
fn sum(files: &[crate::model::ParsedFile]) -> (usize, u64, u64, u64, u64) {
    files
        .iter()
        .flat_map(|f| &f.records.turns)
        .fold((0, 0, 0, 0, 0), |a, t| {
            let u = t.usage;
            (
                a.0 + 1,
                a.1 + u.input,
                a.2 + u.output,
                a.3 + u.cache_read,
                a.4 + u.cache_write_5m,
            )
        })
}

#[test]
fn json_storage_sessions_and_child() {
    let r = root("json");
    json_storage(&r);
    let found = Opencode.discover(&r).unwrap();
    let b = found.iter().find(|s| s.path.ends_with("ses_b")).unwrap();
    assert_eq!((b.role, b.session_id.as_str()), (FileRole::Agent, "ses_a"));
    let a = found.iter().find(|s| s.path.ends_with("ses_a")).unwrap();
    let rec = Opencode.parse_file(a, None).unwrap();
    let u: Vec<(u64, u64, u64, u64)> = rec
        .turns
        .iter()
        .map(|t| {
            (
                t.usage.input,
                t.usage.output,
                t.usage.cache_read,
                t.usage.cache_write_5m,
            )
        })
        .collect();
    assert_eq!(u, [(100, 75, 1000, 200), (10, 5, 2000, 0)]);
    let stored: Vec<Option<f64>> = rec.turns.iter().map(|t| t.stored_cost).collect();
    assert_eq!(stored, [Some(0.01), None]);
    assert_eq!(rec.human_prompts.len(), 1);
    assert_eq!(rec.tool_calls.len(), 1);
    assert_eq!(rec.tool_calls[0].category, crate::model::Category::Test);
    assert_eq!(rec.tool_calls[0].result_chars, 11);
    assert_eq!(rec.strings[rec.project_name.unwrap() as usize], "webapp");
    assert_eq!(Opencode.fingerprint(a).unwrap().change, Change::Whole);
}

#[cfg(feature = "sqlite")]
fn sqlite_db(r: &Path) {
    let c = rusqlite::Connection::open(r.join("opencode.db")).unwrap();
    c.execute_batch(
        "CREATE TABLE session (id TEXT PRIMARY KEY, project_id TEXT, parent_id TEXT, directory TEXT, agent TEXT);
         CREATE TABLE message (id TEXT PRIMARY KEY, session_id TEXT, time_created INTEGER, time_updated INTEGER, data TEXT);
         CREATE TABLE part (id TEXT PRIMARY KEY, message_id TEXT, session_id TEXT, time_created INTEGER, time_updated INTEGER, data TEXT);
         CREATE TABLE session_v2 (id TEXT PRIMARY KEY, parent_id TEXT, directory TEXT);
         CREATE TABLE session_message (id TEXT PRIMARY KEY, session_id TEXT, type TEXT, seq INTEGER, time_created INTEGER, time_updated INTEGER, data TEXT);
         PRAGMA journal_mode=WAL;",
    )
    .unwrap();
    c.execute(
        "INSERT INTO session VALUES ('ses_x', 'p', NULL, '/home/u/api', 'build')",
        [],
    )
    .unwrap();
    c.execute(
        "INSERT INTO session_v2 VALUES ('ses_x', NULL, '/home/u/api')",
        [],
    )
    .unwrap();
    let m10 = json!({"id": "msg_10", "role": "assistant", "modelID": "claude-sonnet-4-5", "time": {"created": 5000},
                     "tokens": tokens(300, 30, 0, 0, 0)});
    c.execute(
        "INSERT INTO message VALUES ('msg_10', 'ses_x', 5000, 5000, ?1)",
        [m10.to_string()],
    )
    .unwrap();
    let v10 = json!({"model": {"id": "claude-sonnet-4-5", "providerID": "anthropic"}, "time": {"created": 5000},
                     "tokens": tokens(300, 30, 0, 0, 0)});
    let v11 = json!({"model": {"id": "claude-opus-4-6", "providerID": "anthropic"}, "time": {"created": 6000},
                     "tokens": tokens(40, 4, 1, 400, 0),
                     "content": [{"type": "tool", "name": "read",
                                  "state": {"input": {"filePath": "/x/skills/s1/SKILL.md"}}}]});
    for (id, seq, v) in [("msg_10", 1, v10), ("msg_11", 2, v11)] {
        c.execute(
            "INSERT INTO session_message VALUES (?1, 'ses_x', 'assistant', ?2, 0, 0, ?3)",
            rusqlite::params![id, seq, v.to_string()],
        )
        .unwrap();
    }
}

#[cfg(feature = "sqlite")]
#[test]
fn sqlite_legacy_and_v2_copy_count_once_with_json_storage() {
    let r = root("both");
    json_storage(&r);
    sqlite_db(&r);
    let listing = |d: &Path| -> Vec<_> {
        let mut v: Vec<_> = std::fs::read_dir(d)
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        v.sort();
        v
    };
    let before = listing(&r);
    let mut pipe = Pipeline::with_providers(vec![Box::new(Opencode)], None);
    let roots = pipe.roots(&[(Some(ID.into()), r.clone())]).unwrap();
    pipe.scan(&roots, &|_| {}).unwrap();
    let files = pipe.files();
    assert_eq!(sum(&files), (5, 457, 118, 3400, 200));
    let x = files
        .iter()
        .find(|f| f.source.session_id == "ses_x")
        .unwrap();
    assert_eq!(x.records.skill_loads.len(), 1);
    assert_eq!(listing(&r), before, "nothing written next to opencode.db");
}
