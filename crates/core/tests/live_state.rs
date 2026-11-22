// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use std::path::PathBuf;
use tokenwhy_core::live::live;
use tokenwhy_core::*;

fn file(role: FileRole, name: &str, session: &str, turns: Vec<Turn>) -> ParsedFile {
    ParsedFile {
        source: SourceFile {
            provider: ProviderId("toy".into()),
            path: PathBuf::from(name),
            role,
            project_key: "proj".into(),
            session_id: session.into(),
            agent_type: None,
            agent_group: None,
        },
        records: FileRecords {
            strings: vec!["m".into()],
            turns,
            ..FileRecords::default()
        },
    }
}

fn turn(order: u32, ts: i64, segment: u32, input: u64, cache_read: u64, output: u64) -> Turn {
    let usage = Usage {
        input,
        cache_read,
        output,
        ..Usage::default()
    };
    Turn {
        msg_id: order as u64 + ts as u64,
        first_ts_ms: ts,
        model: 0,
        usage,
        attribution_skill: None,
        segment,
        order,
        stored_cost: None,
    }
}

#[test]
fn active_session_and_today() {
    use chrono::TimeZone;
    let now: i64 = chrono::Local
        .with_ymd_and_hms(2026, 3, 10, 12, 0, 0)
        .unwrap()
        .timestamp_millis();
    let min = 60_000;
    let main = file(
        FileRole::Main,
        "main",
        "s1",
        vec![
            turn(0, now - 5 * min, 0, 100, 900, 10),
            turn(1, now - 4 * min, 0, 100, 1400, 20),
            turn(2, now - 3 * min, 1, 100, 300, 5),
            turn(3, now - 2 * min, 1, 100, 800, 7),
        ],
    );
    let running = file(
        FileRole::Agent,
        "a1",
        "s1",
        vec![turn(0, now - min, 0, 50, 0, 3)],
    );
    let stale = file(
        FileRole::Agent,
        "a2",
        "s1",
        vec![turn(0, now - 30 * min, 0, 50, 0, 3)],
    );
    let old_main = file(
        FileRole::Main,
        "old",
        "s0",
        vec![turn(0, now - 3 * 3_600_000, 0, 1, 1, 1)],
    );
    let files = vec![main, running, stale, old_main];
    let mtime = |p: &std::path::Path| match p.to_str().unwrap() {
        "main" => Some(now - 2 * min),
        "a1" => Some(now - min),
        "a2" => Some(now - 30 * min),
        _ => Some(now - 3 * 3_600_000),
    };
    let l = live(&files, &[], now, &mtime);

    assert_eq!(l.sessions.len(), 1, "the 3 h old session is not active");
    let s = &l.sessions[0];
    assert_eq!((s.session.as_str(), s.turns, s.compactions), ("s1", 4, 0));
    assert_eq!(s.ctx_tokens, 900); // last turn: 100 + 800
    assert_eq!(s.ctx_growth_since_boundary, 500); // 900 - 400 (first turn of segment 1)
    assert_eq!(s.agents_running, 1); // a1 written 1 min ago, a2 30 min ago

    let t = &l.today;
    {
        assert_eq!(t.fresh_tokens, 110 + 120 + 105 + 107 + 53 + 53 + 2);
        assert_eq!(t.cache_read_tokens, 900 + 1400 + 300 + 800 + 1);
        assert_eq!(t.agents, 2);
    }
}
