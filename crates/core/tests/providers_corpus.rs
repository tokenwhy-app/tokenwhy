// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Expected values are the lines the tests write.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokenwhy_core::pipeline::{self, Pipeline, RootSpec};
use tokenwhy_core::provider::{Change, Fingerprint};
use tokenwhy_core::*;
use tokenwhy_core::{history, rules};

#[derive(Clone)]
struct Toy {
    id: &'static str,
    whole: bool,
    rate: f64,
    full: Arc<AtomicUsize>,
    resumed: Arc<AtomicUsize>,
}

impl Toy {
    fn new(id: &'static str, whole: bool) -> Toy {
        Toy {
            id,
            whole,
            rate: 0.0,
            full: Arc::default(),
            resumed: Arc::default(),
        }
    }
    fn source(&self, p: &Path) -> SourceFile {
        SourceFile {
            provider: self.id(),
            session_id: p.file_stem().unwrap().to_string_lossy().into(),
            path: p.to_path_buf(),
            role: FileRole::Main,
            project_key: "p".into(),
            agent_type: None,
            agent_group: None,
        }
    }
}

impl Provider for Toy {
    fn id(&self) -> ProviderId {
        ProviderId(self.id.into())
    }
    fn label(&self) -> &'static str {
        self.id
    }
    fn caps(&self) -> Capabilities {
        Capabilities {
            turns: Level::Full,
            cache_read: Level::None,
            cache_write: Level::None,
            tool_outputs: Level::None,
            skills: Level::None,
            startup: Level::None,
            memory: Level::None,
            subagents: Level::None,
            compactions: Level::None,
            mcp: Level::None,
            cost: CostSource::Table,
        }
    }
    fn default_roots(&self) -> Vec<PathBuf> {
        vec![]
    }
    fn accepts(&self, _root: &Path, p: &Path) -> bool {
        p.extension().is_some_and(|x| x == "log")
    }
    fn discover(&self, root: &Path) -> io::Result<Vec<SourceFile>> {
        let mut v = Vec::new();
        for e in fs::read_dir(root)? {
            let p = e?.path();
            if self.accepts(root, &p) {
                v.push(self.source(&p));
            }
        }
        Ok(v)
    }
    fn sources_of(&self, root: &Path, path: &Path) -> Vec<SourceFile> {
        (self.accepts(root, path) && path.is_file())
            .then(|| self.source(path))
            .into_iter()
            .collect()
    }
    fn fingerprint(&self, src: &SourceFile) -> io::Result<Fingerprint> {
        if !self.whole {
            return tokenwhy_core::corpus::append_fingerprint(&src.path);
        }
        let bytes = fs::read(&src.path)?;
        let m = fs::metadata(&src.path)?;
        let mtime_ns = m
            .modified()?
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64);
        Ok(Fingerprint {
            size: bytes.len() as u64,
            mtime_ns,
            head_len: 0,
            head_hash: bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
                (h ^ *b as u64).wrapping_mul(0x100_0000_01b3)
            }),
            change: Change::Whole,
        })
    }
    fn vendors(&self) -> &'static [Vendor] {
        &[Vendor::Anthropic]
    }
    fn parse_file(
        &self,
        file: &SourceFile,
        resume: Option<ParseResume>,
    ) -> io::Result<FileRecords> {
        let bytes = fs::read(&file.path)?;
        let (mut rec, start) = match resume {
            Some(r) => {
                self.resumed.fetch_add(1, Ordering::SeqCst);
                (r.previous, r.offset as usize)
            }
            None => {
                self.full.fetch_add(1, Ordering::SeqCst);
                (FileRecords::default(), 0)
            }
        };
        let end = bytes.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
        for line in String::from_utf8_lossy(&bytes[start.min(end)..end]).lines() {
            let f: Vec<&str> = line.split(',').collect();
            let order = rec.turns.len() as u32;
            rec.turns.push(Turn {
                msg_id: f[0].as_bytes()[0] as u64,
                first_ts_ms: f[1].parse().unwrap(),
                model: 0,
                usage: Usage {
                    output: f[2].parse().unwrap(),
                    ..Usage::default()
                },
                attribution_skill: None,
                segment: 0,
                order,
                stored_cost: None,
            });
        }
        rec.offset_done = end as u64;
        rec.strings = vec!["m".into()];
        Ok(rec)
    }
    fn prices(&self) -> PriceTable {
        PriceTable {
            version: 1,
            provider: self.id(),
            rates: vec![ModelRate {
                prefix: "m".into(),
                input: 0.0,
                cache_write_5m: 0.0,
                cache_write_1h: 0.0,
                cache_read: 0.0,
                output: self.rate,
                web_search: 0.0,
                long_context_threshold: None,
                long_context: None,
                checked: String::new(),
            }],
        }
    }
}

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn kept(pipe: &Pipeline) -> Vec<(String, char, u64)> {
    let mut v: Vec<_> = pipe
        .files()
        .iter()
        .flat_map(|f| {
            let p = f.source.provider.0.clone();
            f.records
                .turns
                .iter()
                .map(move |t| (p.clone(), t.msg_id as u8 as char, t.usage.output))
        })
        .collect();
    v.sort();
    v
}

#[test]
fn same_raw_id_in_two_providers_keeps_both() {
    let d = tmp("two_providers_same_id");
    for (dir, line) in [("a", "x,100,7\n"), ("b", "x,100,9\n")] {
        fs::create_dir_all(d.join(dir)).unwrap();
        fs::write(d.join(dir).join("s.log"), line).unwrap();
    }
    let mut pipe = Pipeline::with_providers(
        vec![
            Box::new(Toy::new("toy-a", false)),
            Box::new(Toy::new("toy-b", false)),
        ],
        Some(d.join("store")),
    );
    let roots = [
        RootSpec {
            provider: 0,
            path: d.join("a"),
        },
        RootSpec {
            provider: 1,
            path: d.join("b"),
        },
    ];
    pipe.scan(&roots, &|_| {}).unwrap();
    let both = vec![("toy-a".into(), 'x', 7), ("toy-b".into(), 'x', 9)];
    assert_eq!(kept(&pipe), both);
    fs::write(d.join("a").join("t.log"), "x,200,7\n").unwrap();
    pipe.update(&[d.join("a").join("t.log")]).unwrap();
    assert_eq!(kept(&pipe), both);
}

#[test]
fn whole_source_reparses_on_change_and_reuses_otherwise() {
    let d = tmp("whole_source");
    fs::create_dir_all(d.join("r")).unwrap();
    let f = d.join("r").join("db.log");
    fs::write(&f, "a,100,1\nb,200,2\n").unwrap();
    let toy = Toy::new("toy-w", true);
    let mut pipe = Pipeline::with_providers(vec![Box::new(toy.clone())], Some(d.join("store")));
    let roots = [RootSpec {
        provider: 0,
        path: d.join("r"),
    }];
    pipe.scan(&roots, &|_| {}).unwrap();
    assert_eq!(toy.full.load(Ordering::SeqCst), 1);

    pipe.scan(&roots, &|_| {}).unwrap();
    pipe.update(std::slice::from_ref(&f)).unwrap();
    assert_eq!(
        toy.full.load(Ordering::SeqCst),
        1,
        "unchanged source parsed again"
    );

    fs::write(&f, "a,100,1\nb,200,5\nc,300,3\n").unwrap();
    pipe.update(std::slice::from_ref(&f)).unwrap();
    assert_eq!(toy.full.load(Ordering::SeqCst), 2);
    assert_eq!(toy.resumed.load(Ordering::SeqCst), 0);
    let w = |id, out| ("toy-w".to_string(), id, out);
    assert_eq!(kept(&pipe), vec![w('a', 1), w('b', 5), w('c', 3)]);

    fs::write(&f, "a,100,1\nb,200,6\nc,300,3\n").unwrap();
    pipe.scan(&roots, &|_| {}).unwrap();
    assert_eq!(toy.full.load(Ordering::SeqCst), 3);
    assert_eq!(kept(&pipe), vec![w('a', 1), w('b', 6), w('c', 3)]);

    pipe.save_cache().unwrap();
    let again = Toy::new("toy-w", true);
    let mut p2 = Pipeline::with_providers(vec![Box::new(again.clone())], Some(d.join("store")));
    p2.scan(&roots, &|_| {}).unwrap();
    assert_eq!(again.full.load(Ordering::SeqCst), 0);
    assert_eq!(kept(&p2), vec![w('a', 1), w('b', 6), w('c', 3)]);
}

#[test]
fn provider_filter_splits_totals_without_loss() {
    let d = tmp("provider_filter");
    // 1970-01-02 and 1970-01-03 noon UTC: two separate local days in any zone from UTC-11 to UTC+11.
    let (day1, day2) = (129_600_000, 216_000_000);
    for (dir, file, text) in [
        ("a", "s1.log", format!("a,{day1},7\nb,{day2},3\n")),
        ("a", "s2.log", format!("c,{day2},5\n")),
        ("b", "s3.log", format!("x,{day1},11\n")),
    ] {
        fs::create_dir_all(d.join(dir)).unwrap();
        fs::write(d.join(dir).join(file), text).unwrap();
    }
    let mut pipe = Pipeline::with_providers(
        vec![
            Box::new(Toy::new("toy-a", false)),
            Box::new(Toy::new("toy-b", false)),
        ],
        None,
    );
    let roots = [
        RootSpec {
            provider: 0,
            path: d.join("a"),
        },
        RootSpec {
            provider: 1,
            path: d.join("b"),
        },
    ];
    pipe.scan(&roots, &|_| {}).unwrap();
    let h = pipe.history();
    let fresh = |p: Option<&str>| -> u64 {
        h.daily(0, i64::MAX, history::By::None, None, p)
            .iter()
            .map(|r| r.fresh_tokens)
            .sum()
    };
    assert_eq!((fresh(Some("toy-a")), fresh(Some("toy-b"))), (15, 11));
    assert_eq!(fresh(None), 26);
    let by: Vec<(String, u64)> = h
        .daily(0, i64::MAX, history::By::Provider, None, None)
        .iter()
        .map(|r| (r.provider.clone().unwrap().0, r.fresh_tokens))
        .collect();
    let sum = |id: &str| -> u64 { by.iter().filter(|r| r.0 == id).map(|r| r.1).sum() };
    assert_eq!((sum("toy-a"), sum("toy-b")), (15, 11));

    let sessions = |p: Option<&str>| {
        let mut q = pipeline::query(None, 1_000_000_000, None, AnonLevel::Off);
        q.provider = p.map(|p| ProviderId(p.into()));
        pipe.report(&Settings::default(), &q, 1_000_000_000)
            .0
            .sessions
            .len()
    };
    assert_eq!(
        (
            sessions(None),
            sessions(Some("toy-a")),
            sessions(Some("toy-b"))
        ),
        (3, 2, 1)
    );
}

#[test]
fn startup_rule_needs_startup_capability() {
    use tokenwhy_core::metrics::{Aggregates, ProjectStartup};
    let toy = Toy::new("toy-s", false);
    let agg = |caps: Capabilities| Aggregates {
        project_startup: vec![ProjectStartup {
            provider: toy.id(),
            project: "demo".into(),
            sessions: 10,
            median: 120_000.0,
            agent_median: 0.0,
            agent_total: 0,
            agents: 0,
        }],
        providers: vec![ProviderInfo {
            id: toy.id(),
            label: "Toy".into(),
            roots: 1,
            files: 10,
            caps,
            imported: false,
            skipped: vec![],
        }],
        ..Aggregates::default()
    };
    let r1 = |a: &Aggregates| {
        rules::findings(a, &Thresholds::default())
            .iter()
            .filter(|f| f.rule == "R1")
            .count()
    };
    assert_eq!(r1(&agg(toy.caps())), 0);
    let mut full = toy.caps();
    full.startup = Level::Full;
    assert_eq!(r1(&agg(full)), 1);
}

#[test]
fn price_change_resummarises_only_that_provider() {
    let d = tmp("price_per_provider");
    for (dir, line) in [
        (
            "a",
            "a,100,1000000
",
        ),
        (
            "b",
            "b,100,2000000
",
        ),
    ] {
        fs::create_dir_all(d.join(dir)).unwrap();
        fs::write(d.join(dir).join("s.log"), line).unwrap();
    }
    let roots = [
        RootSpec {
            provider: 0,
            path: d.join("a"),
        },
        RootSpec {
            provider: 1,
            path: d.join("b"),
        },
    ];
    let open = |rate_b: f64| {
        let (a, mut b) = (Toy::new("toy-a", false), Toy::new("toy-b", false));
        let mut a = a;
        a.rate = 1.0;
        b.rate = rate_b;
        let mut p = Pipeline::with_providers(
            vec![Box::new(a.clone()), Box::new(b.clone())],
            Some(d.join("store")),
        );
        p.scan(&roots, &|_| {}).unwrap();
        p.save_cache().unwrap();
        let q = pipeline::query(None, 1_000_000, None, AnonLevel::Off);
        let mut cost: Vec<(String, f64)> = p
            .report(&Settings::default(), &q, 1_000_000)
            .0
            .sessions
            .iter()
            .map(|r| (r.provider.0.clone(), r.cost_usd.value.unwrap()))
            .collect();
        cost.sort_by(|x, y| x.0.cmp(&y.0));
        (
            cost,
            a.full.load(Ordering::SeqCst) + b.full.load(Ordering::SeqCst),
        )
    };
    assert_eq!(
        open(3.0),
        (vec![("toy-a".into(), 1.0), ("toy-b".into(), 6.0)], 2)
    );
    assert_eq!(
        open(5.0),
        (vec![("toy-a".into(), 1.0), ("toy-b".into(), 10.0)], 0)
    );
}

/// Audit L1: a crash between the index and the summaries writes must not bring back the stale summaries' cost.
#[test]
fn summaries_older_than_index_are_rebuilt() {
    let d = tmp("l1_generation");
    fs::create_dir_all(d.join("a")).unwrap();
    let log = d.join("a").join("s.log");
    fs::write(&log, "a,100,1000000\n").unwrap();
    let roots = [RootSpec {
        provider: 0,
        path: d.join("a"),
    }];
    let cost = || {
        let mut t = Toy::new("toy-a", false);
        t.rate = 1.0;
        let mut p = Pipeline::with_providers(vec![Box::new(t)], Some(d.join("store")));
        p.scan(&roots, &|_| {}).unwrap();
        p.save_cache().unwrap();
        let q = pipeline::query(None, 1_000_000, None, AnonLevel::Off);
        p.report(&Settings::default(), &q, 1_000_000).0.sessions[0]
            .cost_usd
            .value
            .unwrap()
    };
    assert_eq!(cost(), 1.0);
    let sums = d.join("store").join("summaries.bin");
    let stale = fs::read(&sums).unwrap();
    fs::write(&log, "a,100,1000000\nc,200,1000000\n").unwrap();
    assert_eq!(cost(), 2.0);
    fs::write(&sums, stale).unwrap();
    assert_eq!(cost(), 2.0);
}
