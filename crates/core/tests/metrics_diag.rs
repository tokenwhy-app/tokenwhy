// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Expected values worked out by hand from the formulas of design section 5 at the test price table.

use std::path::PathBuf;
use tokenwhy_core::anon::{Level, Options};
use tokenwhy_core::metrics::aggregate;
use tokenwhy_core::render::{render, snapshot_with, Format};
use tokenwhy_core::*;

const T0: i64 = 1_790_000_000_000;
const PROVIDER: &str = "test";

fn prices() -> Vec<PriceTable> {
    vec![PriceTable {
        version: 1,
        provider: ProviderId(PROVIDER.into()),
        rates: vec![ModelRate {
            prefix: "m-5-25".into(),
            input: 5.0,
            cache_write_5m: 6.25,
            cache_write_1h: 10.0,
            cache_read: 0.5,
            output: 25.0,
            web_search: 0.01,
            long_context_threshold: None,
            long_context: None,
            checked: "2026-10-21".into(),
        }],
    }]
}

struct B {
    r: FileRecords,
}

impl B {
    fn new() -> Self {
        let mut b = B {
            r: FileRecords::default(),
        };
        b.s("m-5-25");
        b
    }
    fn s(&mut self, v: &str) -> u32 {
        if let Some(i) = self.r.strings.iter().position(|x| x == v) {
            return i as u32;
        }
        self.r.strings.push(v.into());
        (self.r.strings.len() - 1) as u32
    }
    fn turn(&mut self, order: u32, segment: u32, usage: Usage) {
        self.r.turns.push(Turn {
            msg_id: order as u64,
            first_ts_ms: T0 + order as i64 * 1000,
            model: 0,
            usage,
            attribution_skill: None,
            segment,
            order,
            stored_cost: None,
        });
    }
    fn read(&mut self, turn_order: u32, path: &str, chars: u64) {
        let p = self.s(path);
        let tool = self.s("Read");
        self.r.tool_calls.push(ToolCall {
            call_id: (turn_order as u64) << 8 | self.r.tool_calls.len() as u64,
            turn_order,
            tool,
            category: Category::ReadFile,
            detail: None,
            bounded: false,
            path: Some((fnv(path), p)),
            writes_path: false,
            result_chars: chars,
            is_error: false,
            persisted: false,
            images: 0,
        });
    }
    fn compaction(&mut self, order: u32, pre: u64, post: u64) {
        let trigger = self.s("auto");
        self.r.compactions.push(Compaction {
            order,
            trigger,
            pre_tokens: pre,
            post_tokens: post,
            summary_chars: 0,
        });
    }
    fn file(self, project: &str, session: &str) -> ParsedFile {
        ParsedFile {
            source: SourceFile {
                provider: ProviderId(PROVIDER.into()),
                path: PathBuf::from(format!("{project}/{session}.jsonl")),
                role: FileRole::Main,
                project_key: project.into(),
                session_id: session.into(),
                agent_type: None,
                agent_group: None,
            },
            records: self.r,
        }
    }
}

fn fnv(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ b as u64).wrapping_mul(0x0100_0000_01b3)
    })
}

fn query(anon: bool) -> Query {
    let anonymise = if anon { Level::Strict } else { Level::Off };
    Query {
        window: Window {
            from_ms: T0 - 86_400_000,
            to_ms: T0 + 86_400_000,
        },
        project: None,
        anonymise,
        provider: None,
        account: None,
    }
}

fn agg(files: &[ParsedFile]) -> metrics::Aggregates {
    aggregate(
        files,
        &prices(),
        &query(false),
        &Settings::default(),
        T0 + 3_600_000,
    )
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn u(input: u64, cw5: u64, cr: u64, output: u64) -> Usage {
    Usage {
        input,
        cache_write_5m: cw5,
        cache_write_1h: 0,
        cache_read: cr,
        output,
        web_search_requests: 0,
    }
}

#[test]
fn compaction() {
    let mut b = B::new();
    for o in 1..=4 {
        b.turn(o, 0, u(1000, 0, 0, 100));
    }
    b.read(2, "src/X.java", 4000);
    b.compaction(5, 200_000, 9_000);
    for o in 7..=9 {
        b.turn(o, 1, u(0, 0, 20_000, 200));
    }
    b.read(7, "src/X.java", 2000); // re-read after the boundary: counts
    b.read(8, "src/Y.java", 1000); // first read: does not
    let a = agg(&[b.file("p", "s1")]);
    assert_eq!(a.compactions.len(), 1);
    let c = &a.compactions[0];
    assert_eq!((c.pre_tokens, c.post_tokens), (200_000, 9_000));
    assert!(close(c.cost_per_turn_before.value.unwrap(), 0.0075));
    assert!(close(c.cost_per_turn_after.value.unwrap(), 0.015));
    assert_eq!(c.rebuild_tokens.value, Some(500.0));
    assert_eq!(c.first_after_ctx, 20_000);
    assert!(close(c.carry_ratio.unwrap(), 20.0));
    assert!(close(c.fresh_restart_cost_usd.value.unwrap(), 0.00775));
}

#[test]
fn skill_carry() {
    let mut b = B::new();
    let skill = b.s("mc-modding");
    b.r.skill_loads.push(SkillLoad {
        skill,
        body_chars: 8000,
        order: 1,
    });
    for o in 2..=11 {
        b.turn(o, 0, u(100, 0, 0, 10));
    }
    b.compaction(12, 50_000, 5_000);
    for o in 13..=17 {
        b.turn(o, 1, u(100, 0, 0, 10));
    }
    let a = agg(&[b.file("p", "s1")]);
    let s = a.skills.iter().find(|s| s.skill == "mc-modding").unwrap();
    assert_eq!(s.activations, 1);
    assert_eq!(s.body_tokens_median.value, Some(2000.0));
    assert!(s.body_tokens_median.est);
    assert!(close(s.carried_cost_usd.value.unwrap(), 0.0225));
}

fn startup_file(project: &str, first_ctx: u64) -> ParsedFile {
    let mut b = B::new();
    let sys = b.s("");
    let mem = b.s("C:/m/MEMORY.md");
    b.r.startup = Some(StartupParts {
        parts: vec![
            StartupPart {
                kind: StartupKind::SystemPrompt,
                name: sys,
                chars: 40_000,
                content_hash: 0,
            },
            StartupPart {
                kind: StartupKind::BuiltinTools,
                name: sys,
                chars: 20_000,
                content_hash: 0,
            },
            StartupPart {
                kind: StartupKind::Memory,
                name: mem,
                chars: 20_000,
                content_hash: 0,
            },
        ],
        has_full_shape: true,
        instruction_lines: vec![],
    });
    b.turn(1, 0, u(0, first_ctx, 0, 50));
    b.file(project, &format!("s-{project}"))
}

#[test]
fn startup_parts() {
    let a = agg(&[
        startup_file("inrange", 24_000),
        startup_file("outrange", 40_000),
    ]);
    let part = |row: &StartupRow, k: StartupKind| {
        row.parts
            .iter()
            .find(|p| p.kind == k)
            .unwrap()
            .tokens
            .clone()
    };
    let r = a.startup.iter().find(|r| r.project == "inrange").unwrap();
    assert!(close(r.ratio.unwrap(), 0.3));
    assert_eq!(
        part(r, StartupKind::SystemPrompt),
        Amount {
            value: Some(12_000.0),
            est: false
        }
    );
    assert_eq!(
        part(r, StartupKind::Memory),
        Amount {
            value: Some(6_000.0),
            est: false
        }
    );
    assert_eq!(r.unattributed.value, Some(0.0));
    let r = a.startup.iter().find(|r| r.project == "outrange").unwrap();
    assert_eq!(r.ratio, None);
    assert_eq!(
        part(r, StartupKind::SystemPrompt),
        Amount {
            value: Some(10_000.0),
            est: true
        }
    );
    assert_eq!(
        part(r, StartupKind::BuiltinTools),
        Amount {
            value: Some(5_000.0),
            est: true
        }
    );
    assert_eq!(
        r.unattributed,
        Amount {
            value: Some(20_000.0),
            est: true
        }
    );
}

#[test]
fn anon_leak() {
    let mut b = B::new();
    let mem = b.s(r"C:\Users\alice\.claude\projects\secretproj\memory\MEMORY.md");
    b.r.startup = Some(StartupParts {
        parts: vec![StartupPart {
            kind: StartupKind::Memory,
            name: mem,
            chars: 9_000,
            content_hash: 0,
        }],
        has_full_shape: false,
        instruction_lines: vec![],
    });
    let skill = b.s("mysecretskill");
    b.r.skill_loads.push(SkillLoad {
        skill,
        body_chars: 40_000,
        order: 1,
    });
    let name = b.s("secretproj");
    b.r.project_name = Some(name);
    for o in 2..=6 {
        b.turn(o, 0, u(1000, 0, 0, 100));
    }
    for o in 2..=6 {
        b.read(o, r"C:\Users\alice\secretproj\notes.txt", 4000);
    }
    let files = [b.file("secretproj", "secretsess-0001")];
    let q = query(true);
    let settings = Settings::default();
    let a = aggregate(&files, &prices(), &q, &settings, T0 + 3_600_000);
    let f = rules::findings(&a, &settings.thresholds);
    let fired: Vec<&str> = f.iter().map(|f| f.rule.as_str()).collect();
    // The names must reach finding lines before anonymisation for the test to mean anything.
    for r in ["R2", "R5", "R16"] {
        assert!(fired.contains(&r), "{r} did not fire: {fired:?}");
    }
    let (snap, map) = snapshot_with(
        &a,
        f,
        &q,
        1.0,
        &Options {
            level: Level::Strict,
            salt: 42,
        },
    );
    assert!(snap.anonymised);
    assert!(map.aliases.values().any(|v| v == "secretproj"));
    for fmt in [Format::Text, Format::Markdown, Format::Json] {
        let out = render(&snap, fmt, None);
        for secret in [
            "secretproj",
            "alice",
            "mysecretskill",
            "secretsess",
            "notes",
        ] {
            assert!(!out.contains(secret), "{secret} leaked in {fmt:?}:\n{out}");
        }
    }
}

#[test]
fn memory_rows_are_startup_files_only() {
    let mut f = startup_file("p", 24_000);
    let mut b = B { r: f.records };
    b.read(
        1,
        "D:/x/src/main/java/com/polly/memory/MemoryStorage.java",
        900,
    );
    b.read(1, "C:/Users/u/.claude/projects/p/memory/bob-combat.md", 400);
    b.read(1, "/c/m/MEMORY.md", 20_000);
    f.records = b.r;
    let a = agg(&[f]);
    assert_eq!(a.memory.len(), 1);
    let m = &a.memory[0];
    assert_eq!(m.path, "C:/m/MEMORY.md");
    assert_eq!(m.label, "m · MEMORY.md");
    assert_eq!(m.on_demand_reads, 1);
    assert_eq!(
        m.tokens,
        Amount {
            value: Some(6_000.0),
            est: false
        }
    );
}
