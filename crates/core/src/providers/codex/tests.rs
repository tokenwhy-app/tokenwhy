// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Rollouts hand-built in the format of codeburn v0.9.27 docs/providers/codex.md; expected values summed by hand.

use super::*;
use crate::pipeline::Pipeline;
use serde_json::json;

fn line(ty: &str, payload: Value) -> String {
    json!({"timestamp": "2026-10-01T10:00:00.000Z", "type": ty, "payload": payload}).to_string()
        + "\n"
}

fn tc(last: Option<[u64; 4]>, total: Option<[u64; 4]>) -> String {
    let u = |a: [u64; 4]| {
        json!({"input_tokens": a[0], "cached_input_tokens": a[1], "output_tokens": a[2],
               "reasoning_output_tokens": a[3], "total_tokens": a[0] + a[2]})
    };
    let mut info = serde_json::Map::new();
    if let Some(l) = last {
        info.insert("last_token_usage".into(), u(l));
    }
    if let Some(t) = total {
        info.insert("total_token_usage".into(), u(t));
    }
    line("event_msg", json!({"type": "token_count", "info": info}))
}

fn meta(id: &str, extra: Value) -> String {
    let mut p = json!({"id": id, "cwd": "/home/u/proj"});
    p.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    line("session_meta", p)
}

fn parent() -> String {
    [
        meta("aaa", json!({})),
        line("turn_context", json!({"model": "gpt-5.4"})),
        line("event_msg", json!({"type": "user_message", "message": "x"})),
        line(
            "response_item",
            json!({"type": "function_call", "name": "exec_command",
            "arguments": "{\"cmd\":\"git status\"}", "call_id": "c1"}),
        ),
        line(
            "response_item",
            json!({"type": "function_call_output", "call_id": "c1", "output": "On branch main"}),
        ),
        tc(Some([1000, 200, 100, 40]), Some([1000, 200, 100, 40])),
        tc(Some([1000, 200, 100, 40]), Some([1000, 200, 100, 40])),
        line(
            "response_item",
            json!({"type": "custom_tool_call", "name": "apply_patch", "call_id": "c2",
            "input": "*** Begin Patch\n*** Update File: src/a.rs\n@@\n-x\n+y\n*** End Patch"}),
        ),
        tc(None, Some([3000, 1200, 300, 90])),
        tc(Some([0, 0, 0, 0]), None),
    ]
    .concat()
}

fn fork() -> String {
    [
        meta("bbb", json!({"forked_from_id": "aaa"})),
        line("turn_context", json!({"model": "gpt-5.4"})),
        tc(Some([1000, 200, 100, 40]), Some([1000, 200, 100, 40])),
        tc(None, Some([3000, 1200, 300, 90])),
        tc(Some([2000, 1800, 100, 10]), Some([5000, 3000, 400, 100])),
    ]
    .concat()
}

fn child() -> String {
    [
        meta(
            "ccc",
            json!({"source": {"subagent": {"thread_spawn": {"parent_thread_id": "aaa"}}}}),
        ),
        line("turn_context", json!({"model": "gpt-5.4"})),
        tc(Some([500, 0, 50, 0]), Some([500, 0, 50, 0])),
    ]
    .concat()
}

fn home(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/tmp/codex-unit")
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    let day = d.join("sessions/2026/10/01");
    std::fs::create_dir_all(&day).unwrap();
    std::fs::create_dir_all(d.join("archived_sessions")).unwrap();
    std::fs::write(day.join("rollout-2026-10-01T10-00-00-aaa.jsonl"), parent()).unwrap();
    std::fs::write(day.join("rollout-2026-10-01T11-00-00-bbb.jsonl"), fork()).unwrap();
    std::fs::write(day.join("rollout-2026-10-01T12-00-00-ccc.jsonl"), child()).unwrap();
    std::fs::write(
        d.join("archived_sessions/rollout-2026-10-01T10-00-00-aaa.jsonl"),
        parent(),
    )
    .unwrap();
    std::fs::write(
        d.join("archived_sessions/rollout-old.jsonl.zst"),
        b"\x28\xb5\x2f\xfd",
    )
    .unwrap();
    d
}

fn sum(files: &[crate::model::ParsedFile]) -> (usize, u64, u64, u64) {
    let t = files.iter().flat_map(|f| &f.records.turns);
    t.fold((0, 0, 0, 0), |a, t| {
        (
            a.0 + 1,
            a.1 + t.usage.input,
            a.2 + t.usage.cache_read,
            a.3 + t.usage.output,
        )
    })
}

#[test]
fn parent_calls_from_last_and_from_total_delta() {
    let d = home("parent");
    let src = source(&d.join("sessions/2026/10/01/rollout-2026-10-01T10-00-00-aaa.jsonl")).unwrap();
    let r = parse(&src).unwrap();
    let u: Vec<(u64, u64, u64)> = r
        .turns
        .iter()
        .map(|t| (t.usage.input, t.usage.cache_read, t.usage.output))
        .collect();
    assert_eq!(u, [(800, 200, 100), (1000, 1000, 200)]);
    assert_eq!(r.strings[r.turns[0].model as usize], "gpt-5.4");
    assert_eq!(r.tool_calls.len(), 2);
    assert_eq!(r.tool_calls[0].category, Category::Vcs);
    assert_eq!(r.tool_calls[0].result_chars, 14);
    assert_eq!(r.tool_calls[0].turn_order, r.turns[0].order);
    assert_eq!(r.tool_calls[1].category, Category::Edit);
    assert_eq!(r.tool_calls[1].turn_order, r.turns[1].order);
    assert_eq!(
        r.strings[r.tool_calls[1].path.unwrap().1 as usize],
        "src/a.rs"
    );
    assert_eq!(r.human_prompts.len(), 1);
    assert_eq!(r.strings[r.project_name.unwrap() as usize], "proj");
}

#[test]
fn fork_replay_adds_nothing_and_archived_copy_is_skipped() {
    let d = home("corpus");
    let found = Codex.discover(&d).unwrap();
    assert_eq!(
        found.len(),
        3,
        "active aaa, bbb, ccc; archived aaa is a copy"
    );
    let child = found
        .iter()
        .find(|s| s.path.to_string_lossy().ends_with("ccc.jsonl"))
        .unwrap();
    assert_eq!(
        (child.role, child.session_id.as_str()),
        (FileRole::Agent, "aaa")
    );

    let mut pipe = Pipeline::with_providers(vec![Box::new(Codex)], None);
    let roots = pipe.roots(&[(Some(ID.into()), d.clone())]).unwrap();
    pipe.scan(&roots, &|_| {}).unwrap();
    assert_eq!(sum(&pipe.files()), (4, 2500, 3000, 450));
    let info = pipe.provider_infos();
    assert_eq!(info[0].skipped[0].count, 1);
}
