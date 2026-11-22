// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Expected turn lists worked by hand from the lines written.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokenwhy_core::pipeline::{self, Pipeline, RootSpec};
use tokenwhy_core::*;

#[derive(Default, Clone)]
struct Toy {
    full: Arc<AtomicUsize>,
    resumed: Arc<AtomicUsize>,
}

fn source(p: &Path) -> SourceFile {
    SourceFile {
        provider: ProviderId("toy".into()),
        session_id: p.file_stem().unwrap().to_string_lossy().into(),
        path: p.to_path_buf(),
        role: FileRole::Main,
        project_key: "p".into(),
        agent_type: None,
        agent_group: None,
    }
}

impl Provider for Toy {
    fn id(&self) -> ProviderId {
        ProviderId("toy".into())
    }
    fn label(&self) -> &'static str {
        "Toy"
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
        p.extension().is_some_and(|x| x == "jsonl")
    }
    fn discover(&self, root: &Path) -> io::Result<Vec<SourceFile>> {
        let mut v = Vec::new();
        for e in fs::read_dir(root)? {
            let p = e?.path();
            if self.accepts(root, &p) {
                v.push(source(&p));
            }
        }
        Ok(v)
    }
    fn sources_of(&self, root: &Path, path: &Path) -> Vec<SourceFile> {
        (path.parent() == Some(root) && self.accepts(root, path) && path.is_file())
            .then(|| source(path))
            .into_iter()
            .collect()
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
        if end > start {
            for line in String::from_utf8_lossy(&bytes[start..end]).lines() {
                let f: Vec<&str> = line.split(',').collect();
                let id = f[0].as_bytes()[0] as u64;
                let (ts, out) = (f[1].parse().unwrap(), f[2].parse().unwrap());
                match rec.turns.iter_mut().find(|t| t.msg_id == id) {
                    Some(t) => t.usage.output = out,
                    None => {
                        let order = rec.turns.len() as u32;
                        let usage = Usage {
                            output: out,
                            ..Usage::default()
                        };
                        rec.turns.push(Turn {
                            msg_id: id,
                            first_ts_ms: ts,
                            model: 0,
                            usage,
                            attribution_skill: None,
                            segment: 0,
                            order,
                            stored_cost: None,
                        });
                    }
                }
            }
            rec.offset_done = end as u64;
        }
        Ok(rec)
    }
    fn prices(&self) -> PriceTable {
        PriceTable {
            version: 1,
            provider: self.id(),
            rates: vec![],
        }
    }
}

fn kept(pipe: &Pipeline) -> Vec<(String, char, i64, u64)> {
    let mut v: Vec<_> = pipe
        .files()
        .iter()
        .flat_map(|f| {
            let stem = f.source.session_id.clone();
            f.records.turns.iter().map(move |t| {
                (
                    stem.clone(),
                    t.msg_id as u8 as char,
                    t.first_ts_ms,
                    t.usage.output,
                )
            })
        })
        .collect();
    v.sort();
    v
}

fn pipe(p: &Toy, root: &Path, store: Option<PathBuf>) -> Pipeline {
    let mut pipe = Pipeline::with_providers(vec![Box::new(p.clone())], store);
    pipe.scan(
        &[RootSpec {
            provider: 0,
            path: root.to_path_buf(),
        }],
        &|_| {},
    )
    .unwrap();
    pipe
}

fn snapshot(pipe: &Pipeline) -> serde_json::Value {
    let now = 1_000_000;
    let mut v = serde_json::to_value(
        pipe.report(
            &Settings::default(),
            &pipeline::query(None, now, None, AnonLevel::Off),
            now,
        )
        .0,
    )
    .unwrap();
    let o = v.as_object_mut().unwrap();
    o.remove("daily");
    o.remove("week");
    v
}

fn assert_matches_full(live: &Pipeline, root: &Path) {
    let full = pipe(&Toy::default(), root, None);
    assert_eq!(kept(live), kept(&full));
    assert_eq!(snapshot(live), snapshot(&full));
}

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(d.join("root")).unwrap();
    d
}

fn s(stem: &str, id: char, ts: i64, out: u64) -> (String, char, i64, u64) {
    (stem.into(), id, ts, out)
}

#[test]
fn incremental_equals_full() {
    let dir = tmp("incr");
    let root = dir.join("root");
    let f = root.join("s1.jsonl");
    fs::write(&f, "a,100,10\nb,200,5\nc,300,7").unwrap();
    let p = Toy::default();
    let mut live = pipe(&p, &root, Some(dir.join("store")));
    assert_eq!(
        kept(&live),
        vec![s("s1", 'a', 100, 10), s("s1", 'b', 200, 5)]
    );

    fs::write(&f, "a,100,10\nb,200,5\nc,300,7\na,150,40\nd,400,9\n").unwrap();
    live.update(std::slice::from_ref(&f)).unwrap();
    assert_eq!(
        p.resumed.load(Ordering::SeqCst),
        1,
        "grown file must resume, not reparse"
    );
    assert_eq!(
        kept(&live),
        vec![
            s("s1", 'a', 100, 40),
            s("s1", 'b', 200, 5),
            s("s1", 'c', 300, 7),
            s("s1", 'd', 400, 9)
        ]
    );
    assert_matches_full(&live, &root);
}

#[test]
fn ownership_moves_with_files() {
    let dir = tmp("own");
    let root = dir.join("root");
    let (s1, s2, s0) = (
        root.join("s1.jsonl"),
        root.join("s2.jsonl"),
        root.join("s0.jsonl"),
    );
    fs::write(&s1, "a,100,1\nb,200,2\n").unwrap();
    let p = Toy::default();
    let mut live = pipe(&p, &root, Some(dir.join("store")));

    fs::write(&s2, "a,100,1\nb,200,2\ne,500,5\n").unwrap();
    live.update(std::slice::from_ref(&s2)).unwrap();
    assert_eq!(
        kept(&live),
        vec![
            s("s1", 'a', 100, 1),
            s("s1", 'b', 200, 2),
            s("s2", 'e', 500, 5)
        ]
    );
    assert_matches_full(&live, &root);

    fs::write(&s0, "z,50,9\nb,200,2\n").unwrap();
    live.update(std::slice::from_ref(&s0)).unwrap();
    assert_eq!(
        kept(&live),
        vec![
            s("s0", 'b', 200, 2),
            s("s0", 'z', 50, 9),
            s("s1", 'a', 100, 1),
            s("s2", 'e', 500, 5)
        ]
    );
    assert_matches_full(&live, &root);

    live.save_cache().unwrap();
    let before = p.full.load(Ordering::SeqCst) + p.resumed.load(Ordering::SeqCst);
    let reopened = pipe(&p, &root, Some(dir.join("store")));
    assert_eq!(
        p.full.load(Ordering::SeqCst) + p.resumed.load(Ordering::SeqCst),
        before,
        "unchanged files are not parsed"
    );
    assert_matches_full(&reopened, &root);

    fs::remove_file(&s1).unwrap();
    live.update(std::slice::from_ref(&s1)).unwrap();
    assert_eq!(
        kept(&live),
        vec![
            s("s0", 'b', 200, 2),
            s("s0", 'z', 50, 9),
            s("s2", 'a', 100, 1),
            s("s2", 'e', 500, 5)
        ]
    );
    assert_matches_full(&live, &root);

    let full_before = p.full.load(Ordering::SeqCst);
    fs::write(&s0, "y,60,3\n").unwrap();
    live.update(std::slice::from_ref(&s0)).unwrap();
    assert_eq!(p.full.load(Ordering::SeqCst), full_before + 1);
    assert_eq!(
        kept(&live),
        vec![
            s("s0", 'y', 60, 3),
            s("s2", 'a', 100, 1),
            s("s2", 'b', 200, 2),
            s("s2", 'e', 500, 5)
        ]
    );
    assert_matches_full(&live, &root);
}

#[test]
fn price_change_keeps_records() {
    let dir = tmp("price");
    let root = dir.join("root");
    fs::write(root.join("s1.jsonl"), "a,100,10\n").unwrap();
    let p = Toy::default();
    let mut live = pipe(&p, &root, Some(dir.join("store")));
    live.save_cache().unwrap();
    assert_eq!(p.full.load(Ordering::SeqCst), 1);

    #[derive(Clone)]
    struct Newer(Toy);
    impl Provider for Newer {
        fn id(&self) -> ProviderId {
            self.0.id()
        }
        fn label(&self) -> &'static str {
            self.0.label()
        }
        fn caps(&self) -> Capabilities {
            self.0.caps()
        }
        fn default_roots(&self) -> Vec<PathBuf> {
            vec![]
        }
        fn accepts(&self, root: &Path, p: &Path) -> bool {
            self.0.accepts(root, p)
        }
        fn discover(&self, root: &Path) -> io::Result<Vec<SourceFile>> {
            self.0.discover(root)
        }
        fn sources_of(&self, root: &Path, path: &Path) -> Vec<SourceFile> {
            self.0.sources_of(root, path)
        }
        fn vendors(&self) -> &'static [Vendor] {
            self.0.vendors()
        }
        fn parse_file(
            &self,
            file: &SourceFile,
            resume: Option<ParseResume>,
        ) -> io::Result<FileRecords> {
            self.0.parse_file(file, resume)
        }
        fn prices(&self) -> PriceTable {
            PriceTable {
                version: 2,
                ..self.0.prices()
            }
        }
    }
    let mut again =
        Pipeline::with_providers(vec![Box::new(Newer(p.clone()))], Some(dir.join("store")));
    again
        .scan(
            &[RootSpec {
                provider: 0,
                path: root.clone(),
            }],
            &|_| {},
        )
        .unwrap();
    assert_eq!(
        p.full.load(Ordering::SeqCst),
        1,
        "a new price table version must not reparse"
    );
}

// Regression: a cold session whose file is rewritten from the head or deleted once panicked ("live file number") or lost its records.
#[test]
fn cold_session_reparse_and_remove() {
    let dir = tmp("cold");
    let root = dir.join("root");
    let (s1, s2) = (root.join("s1.jsonl"), root.join("s2.jsonl"));
    fs::write(&s1, "a,100,1\nb,200,2\n").unwrap();
    fs::write(&s2, "a,100,1\nb,200,2\ne,500,5\n").unwrap();
    let p = Toy::default();
    pipe(&p, &root, Some(dir.join("store")))
        .save_cache()
        .unwrap();

    let mut live = pipe(&p, &root, Some(dir.join("store")));
    fs::write(&s1, "y,60,3\n").unwrap();
    live.update(std::slice::from_ref(&s1)).unwrap();
    assert_eq!(
        kept(&live),
        vec![
            s("s1", 'y', 60, 3),
            s("s2", 'a', 100, 1),
            s("s2", 'b', 200, 2),
            s("s2", 'e', 500, 5)
        ]
    );
    assert_matches_full(&live, &root);

    live.save_cache().unwrap();
    let mut live = pipe(&p, &root, Some(dir.join("store")));
    fs::remove_file(&s2).unwrap();
    live.update(std::slice::from_ref(&s2)).unwrap();
    assert_eq!(kept(&live), vec![s("s1", 'y', 60, 3)]);
    assert_matches_full(&live, &root);
}
