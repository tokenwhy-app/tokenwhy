// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Expected values worked by hand from tests/fixtures/claude and Anthropic's price page (checked 2026-10-21).

use std::fs;
use std::path::{Path, PathBuf};
use tokenwhy_core::price::{cost, rate_for};
use tokenwhy_core::providers::claude::ClaudeCode;
use tokenwhy_core::*;

fn fx(p: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/claude")
        .join(p)
}

fn source(path: PathBuf, role: FileRole) -> SourceFile {
    SourceFile {
        provider: ClaudeCode.id(),
        path,
        role,
        project_key: "D--work-demo".into(),
        session_id: "s".into(),
        agent_type: None,
        agent_group: None,
    }
}

fn parse(path: PathBuf) -> FileRecords {
    ClaudeCode
        .parse_file(&source(path, FileRole::Main), None)
        .unwrap()
}

fn turn_cost(table: &PriceTable, r: &FileRecords) -> f64 {
    r.turns
        .iter()
        .map(|t| {
            cost(
                rate_for(table, &r.strings[t.model as usize]).unwrap(),
                &t.usage,
            )
        })
        .sum()
}

#[test]
fn stream_dedup() {
    let r = parse(fx("stream_dedup.jsonl"));
    assert_eq!(r.turns.len(), 1);
    assert_eq!(r.turns[0].usage.output, 120);
    assert_eq!(r.turns[0].first_ts_ms, 1_792_576_800_000);
    let c = turn_cost(&ClaudeCode.prices(), &r);
    assert!((c - 0.008).abs() < 1e-12, "{c}");
}

#[test]
fn resume_copy() {
    let p = ClaudeCode;
    let files = p.discover(&fx("resume_copy")).unwrap();
    assert_eq!(files.len(), 2);
    let mut parsed: Vec<ParsedFile> = files
        .into_iter()
        .map(|s| {
            let records = p.parse_file(&s, None).unwrap();
            ParsedFile { source: s, records }
        })
        .collect();
    assert_eq!(
        parsed.iter().map(|f| f.records.turns.len()).sum::<usize>(),
        5
    );
    p.dedup(&mut parsed);
    let by = |n: &str| {
        parsed
            .iter()
            .find(|f| f.source.path.ends_with(n))
            .unwrap()
            .records
            .turns
            .len()
    };
    assert_eq!((by("a.jsonl"), by("b.jsonl")), (2, 1));
}

#[test]
fn workflow_agents() {
    let p = ClaudeCode;
    let files = p.discover(&fx("workflow_agents")).unwrap();
    let mains: Vec<_> = files.iter().filter(|f| f.role == FileRole::Main).collect();
    let agents: Vec<_> = files.iter().filter(|f| f.role == FileRole::Agent).collect();
    assert_eq!(
        (mains.len(), agents.len()),
        (1, 2),
        "journal.jsonl files are not transcripts"
    );
    assert!(agents.iter().all(|a| a.session_id == "S1"));
    let a = agents
        .iter()
        .find(|a| a.path.ends_with("agent-a.jsonl"))
        .unwrap();
    let b = agents
        .iter()
        .find(|a| a.path.ends_with("agent-b.jsonl"))
        .unwrap();
    assert_eq!(a.agent_type.as_deref(), Some("Explore"));
    assert_eq!(
        (b.agent_type.as_deref(), b.agent_group.as_deref()),
        (Some("chore"), Some("Lots"))
    );

    let table = p.prices();
    let main = p.parse_file(mains[0], None).unwrap();
    assert_eq!(main.tool_calls.len(), 1);
    assert_eq!(main.tool_calls[0].result_chars, 25);
    assert_eq!(main.tool_calls[0].category, Category::ReadFile);
    let agent_cost: f64 = agents
        .iter()
        .map(|s| turn_cost(&table, &p.parse_file(s, None).unwrap()))
        .sum();
    let share = agent_cost / (agent_cost + turn_cost(&table, &main));
    assert!((share - 0.6).abs() < 1e-9, "{share}");
}

#[test]
fn records_of_one_session() {
    let r = parse(fx("records.jsonl"));
    let s = |i: u32| r.strings[i as usize].as_str();
    assert_eq!(s(r.project_name.unwrap()), "demo");
    assert_eq!(r.turns.len(), 3);
    assert_eq!(
        (
            r.turns[0].usage.cache_write_5m,
            r.turns[0].usage.cache_write_1h
        ),
        (1000, 0)
    );
    assert_eq!(s(r.turns[1].attribution_skill.unwrap()), "alpha");
    assert_eq!(r.skill_loads.len(), 1);
    assert_eq!(
        (s(r.skill_loads[0].skill), r.skill_loads[0].body_chars),
        ("alpha", 120)
    );
    assert_eq!(r.compactions.len(), 1);
    let c = &r.compactions[0];
    assert_eq!(
        (c.pre_tokens, c.post_tokens, c.summary_chars, s(c.trigger)),
        (200_000, 9_000, 50, "auto")
    );
    assert_eq!((r.turns[1].segment, r.turns[2].segment), (0, 1));
    let read = r.tool_calls.iter().find(|c| s(c.tool) == "Read").unwrap();
    assert_eq!(read.category, Category::Logs);
    assert!(read.persisted);
    assert_eq!(
        read.result_chars,
        "<persisted-output>\nOutput too large. Preview:\nline1\n"
            .chars()
            .count() as u64
    );
    assert!(r.open_calls.len() == 1, "the Skill call has no result line");
    assert_eq!(r.human_prompts.len(), 1);
    let st = r.startup.as_ref().unwrap();
    let kinds = |k: StartupKind| {
        st.parts
            .iter()
            .filter(|p| p.kind == k)
            .map(|p| (s(p.name), p.chars))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        kinds(StartupKind::UserInstructions),
        vec![("C:\\h\\CLAUDE.md", 38)]
    );
    assert_eq!(
        kinds(StartupKind::SkillListing),
        vec![("alpha", 22), ("plug:beta", 19)]
    );
    assert_eq!(st.instruction_lines[0].1.len(), 1);
}

#[test]
fn resume_equals_full() {
    let full_text = fs::read(fx("records.jsonl")).unwrap();
    let mut cut = full_text.len() * 6 / 10;
    while full_text[cut - 1] == b'\n' {
        cut += 1;
    }
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("l1_resume");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("live.jsonl");
    fs::write(&path, &full_text[..cut]).unwrap();
    let src = source(path.clone(), FileRole::Main);
    let first = ClaudeCode.parse_file(&src, None).unwrap();
    assert!(
        first.offset_done < cut as u64,
        "half-written line left for the next scan"
    );
    fs::write(&path, &full_text).unwrap();
    let resumed = ClaudeCode
        .parse_file(
            &src,
            Some(ParseResume {
                offset: first.offset_done,
                previous: first,
            }),
        )
        .unwrap();
    let full = parse(fx("records.jsonl"));
    assert_eq!(resumed, full);
}

#[test]
fn prices_by_hand() {
    let t = ClaudeCode.prices();
    let r = |m: &str| rate_for(&t, m).unwrap();
    assert_eq!(
        (
            r("claude-opus-4-1-20250805").input,
            r("claude-opus-4-1-20250805").output
        ),
        (15.0, 75.0)
    );
    assert_eq!(
        (
            r("claude-opus-4-20250514").input,
            r("claude-opus-4-5-20251101").input
        ),
        (15.0, 5.0)
    );
    assert_eq!(
        (r("claude-opus-5-5").input, r("claude-opus-5-5").output),
        (4.0, 20.0)
    );
    assert!(rate_for(&t, "claude-unknown-9").is_none());
    let u = Usage {
        input: 1_000_000,
        cache_write_5m: 1_000_000,
        cache_write_1h: 1_000_000,
        cache_read: 1_000_000,
        output: 1_000_000,
        web_search_requests: 3,
    };
    let c = cost(r("claude-sonnet-5-5"), &u);
    assert!((c - 18.73).abs() < 1e-9, "{c}");
    let big = Usage {
        input: 150_000,
        output: 1_000,
        ..Default::default()
    };
    assert!((cost(r("claude-haiku-5-5"), &big) - 0.0775).abs() < 1e-12);
    let edge = Usage {
        input: 100_000,
        output: 1_000,
        ..Default::default()
    };
    assert!((cost(r("claude-haiku-5-5"), &edge) - 0.0105).abs() < 1e-12);
}

/// Rates from Anthropic's claude-api model table (read 2026-10-22), USD per million tokens.
#[test]
fn price_table_matches_anthropic_list_prices() {
    let t = ClaudeCode.prices();
    for (model, input, output, cache_read) in [
        ("claude-fable-5-1", 10.0, 50.0, 0.25),
        ("claude-fable-5", 10.0, 50.0, 1.0),
        ("claude-opus-5-5", 4.0, 20.0, 0.2),
        ("claude-sonnet-5-5", 2.0, 10.0, 0.2),
        ("claude-sonnet-5", 2.0, 10.0, 0.2),
    ] {
        let r = rate_for(&t, model).unwrap();
        assert_eq!(
            (r.input, r.output, r.cache_read),
            (input, output, cache_read),
            "{model}"
        );
    }
}
