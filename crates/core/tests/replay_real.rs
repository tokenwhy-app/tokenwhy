// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Diagnostic run with `--ignored`. TOKENWHY_REPLAY_ROOT (default ~/.claude/projects), TOKENWHY_REPLAY_OUT (required), TOKENWHY_REPLAY_ROUNDS (default 6), TOKENWHY_REPLAY_GAP_S (default 30).

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokenwhy_core::pipeline::{self, Pipeline, RootSpec};
use tokenwhy_core::*;

struct Gated {
    inner: Box<dyn Provider>,
    gate: Option<Arc<RwLock<HashSet<PathBuf>>>>,
}

impl Gated {
    fn admits(&self, p: &Path) -> bool {
        self.gate
            .as_ref()
            .is_none_or(|g| g.read().unwrap().contains(p))
    }
}

impl Provider for Gated {
    fn id(&self) -> ProviderId {
        self.inner.id()
    }
    fn label(&self) -> &'static str {
        self.inner.label()
    }
    fn caps(&self) -> Capabilities {
        self.inner.caps()
    }
    fn default_roots(&self) -> Vec<PathBuf> {
        vec![]
    }
    fn discover(&self, root: &Path) -> io::Result<Vec<SourceFile>> {
        Ok(self
            .inner
            .discover(root)?
            .into_iter()
            .filter(|s| self.admits(&s.path))
            .collect())
    }
    fn accepts(&self, root: &Path, p: &Path) -> bool {
        self.inner.accepts(root, p)
    }
    fn parse_file(&self, f: &SourceFile, r: Option<ParseResume>) -> io::Result<FileRecords> {
        self.inner.parse_file(f, r)
    }
    fn sources_of(&self, root: &Path, p: &Path) -> Vec<SourceFile> {
        if self.admits(p) {
            self.inner.sources_of(root, p)
        } else {
            vec![]
        }
    }
    fn drop_copies(&self, f: &mut ParsedFile, owned: &dyn Fn(IdHash) -> bool) {
        self.inner.drop_copies(f, owned)
    }
    fn vendors(&self) -> &'static [Vendor] {
        self.inner.vendors()
    }
    fn prices(&self) -> PriceTable {
        self.inner.prices()
    }
}

fn claude() -> Box<dyn Provider> {
    provider::all()
        .into_iter()
        .find(|p| p.id().0 == "claude-code")
        .expect("claude provider")
}

fn pipe(
    root: &Path,
    gate: Option<Arc<RwLock<HashSet<PathBuf>>>>,
    store: Option<PathBuf>,
) -> Pipeline {
    let mut p = Pipeline::with_providers(
        vec![Box::new(Gated {
            inner: claude(),
            gate,
        })],
        store,
    );
    p.scan(
        &[RootSpec {
            provider: 0,
            path: root.to_path_buf(),
        }],
        &|_| {},
    )
    .unwrap();
    p
}

fn stat(p: &Path) -> Option<(u64, SystemTime)> {
    fs::metadata(p)
        .ok()
        .map(|m| (m.len(), m.modified().unwrap()))
}

fn all_stats(root: &Path) -> BTreeMap<PathBuf, (u64, SystemTime)> {
    claude()
        .discover(root)
        .unwrap()
        .into_iter()
        .filter_map(|s| stat(&s.path).map(|st| (s.path, st)))
        .collect()
}

fn records(p: &Pipeline) -> BTreeMap<String, Vec<String>> {
    let mut m = BTreeMap::new();
    for f in p.files() {
        let mut v: Vec<String> = f.records.turns.iter().map(|t| format!("{t:?}")).collect();
        v.extend(f.records.tool_calls.iter().map(|c| format!("{c:?}")));
        v.sort();
        m.insert(f.source.path.display().to_string(), v);
    }
    m
}

fn snapshot(p: &Pipeline, now: i64) -> serde_json::Value {
    serde_json::to_value(
        p.report(
            &Settings::default(),
            &pipeline::query(None, now, None, AnonLevel::Off),
            now,
        )
        .0,
    )
    .unwrap()
}

fn json_diff(a: &serde_json::Value, b: &serde_json::Value, path: &str, out: &mut Vec<String>) {
    use serde_json::Value::*;
    match (a, b) {
        (Object(x), Object(y)) => {
            for k in x.keys().chain(y.keys().filter(|k| !x.contains_key(*k))) {
                json_diff(
                    x.get(k).unwrap_or(&Null),
                    y.get(k).unwrap_or(&Null),
                    &format!("{path}.{k}"),
                    out,
                );
            }
        }
        (Array(x), Array(y)) if x.len() == y.len() => {
            for (i, (u, v)) in x.iter().zip(y).enumerate() {
                json_diff(u, v, &format!("{path}[{i}]"), out);
            }
        }
        _ if a != b => out.push(format!("{path}: live {} | full {}", short(a), short(b))),
        _ => {}
    }
}

fn short(v: &serde_json::Value) -> String {
    let s = v.to_string();
    if s.len() > 160 {
        format!("{}... ({} chars)", &s[..160], s.len())
    } else {
        s
    }
}

fn compare(live: &Pipeline, full: &Pipeline, now: i64) -> Vec<String> {
    let mut out = Vec::new();
    let (a, b) = (records(live), records(full));
    for k in a.keys().chain(b.keys().filter(|k| !a.contains_key(*k))) {
        let (x, y) = (a.get(k), b.get(k));
        if x != y {
            let (x, y) = (
                x.cloned().unwrap_or_default(),
                y.cloned().unwrap_or_default(),
            );
            let only_live = x.iter().filter(|r| !y.contains(r)).count();
            let only_full = y.iter().filter(|r| !x.contains(r)).count();
            out.push(format!(
                "records {k}: live {} full {} (only live {only_live}, only full {only_full})",
                x.len(),
                y.len()
            ));
        }
    }
    json_diff(
        &snapshot(live, now),
        &snapshot(full, now),
        "snapshot",
        &mut out,
    );
    out
}

fn env_or(k: &str, d: u64) -> u64 {
    std::env::var(k)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(d)
}

#[test]
#[ignore]
fn replay_real_root() {
    let root = std::env::var("TOKENWHY_REPLAY_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| claude().default_roots().remove(0));
    let out = PathBuf::from(std::env::var("TOKENWHY_REPLAY_OUT").expect("TOKENWHY_REPLAY_OUT"));
    let _ = fs::remove_dir_all(out.join("store-a"));
    let _ = fs::remove_dir_all(out.join("store-b"));
    fs::create_dir_all(&out).unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let mut log = Vec::<String>::new();
    let mut diffs = 0usize;
    let note = |log: &mut Vec<String>, line: String| {
        println!("{line}");
        log.push(line);
    };

    let quiet_before = SystemTime::now() - Duration::from_secs(1800);
    let start = all_stats(&root);
    let mut order: Vec<(SystemTime, PathBuf)> = start
        .iter()
        .filter(|(_, s)| s.1 < quiet_before)
        .map(|(p, s)| (s.1, p.clone()))
        .collect();
    order.sort();
    let n = order.len();
    note(
        &mut log,
        format!("A: {} files under root, {n} quiet", start.len()),
    );
    let gate = Arc::new(RwLock::new(HashSet::new()));
    let half = n / 2;
    gate.write()
        .unwrap()
        .extend(order[..half].iter().map(|(_, p)| p.clone()));
    let mut live = pipe(&root, Some(gate.clone()), Some(out.join("store-a")));
    let checkpoints = [half, n * 3 / 4, n];
    let mut admitted = half;
    for &cp in &checkpoints {
        while admitted < cp {
            let batch: Vec<PathBuf> = order[admitted..(admitted + 100).min(cp)]
                .iter()
                .map(|(_, p)| p.clone())
                .collect();
            gate.write().unwrap().extend(batch.iter().cloned());
            live.update(&batch).unwrap();
            admitted += batch.len();
        }
        let d = compare(&live, &pipe(&root, Some(gate.clone()), None), now);
        diffs += d.len();
        note(
            &mut log,
            format!("A checkpoint {cp}/{n} files: {} differences", d.len()),
        );
        log.extend(d.into_iter().take(40));
    }
    live.save_cache().unwrap();
    drop(live);
    let reopened = pipe(&root, Some(gate.clone()), Some(out.join("store-a")));
    let d = compare(&reopened, &pipe(&root, Some(gate.clone()), None), now);
    diffs += d.len();
    note(
        &mut log,
        format!("A store round trip: {} differences", d.len()),
    );
    log.extend(d.into_iter().take(40));
    let moved = order
        .iter()
        .filter(|(_, p)| stat(p) != start.get(p).copied())
        .count();
    note(
        &mut log,
        format!("A: {moved} quiet files changed during the run (noise if > 0)"),
    );
    drop(reopened);

    let rounds = env_or("TOKENWHY_REPLAY_ROUNDS", 6);
    let gap = Duration::from_secs(env_or("TOKENWHY_REPLAY_GAP_S", 30));
    let mut live = pipe(&root, None, Some(out.join("store-b")));
    let mut seen = all_stats(&root);
    let (mut compared, mut raced, mut grown_total, mut new_total) = (0, 0, 0, 0);
    for r in 0..rounds {
        std::thread::sleep(gap);
        let cur = all_stats(&root);
        let dirty: Vec<PathBuf> = cur
            .iter()
            .filter(|(p, s)| seen.get(*p) != Some(s))
            .map(|(p, _)| p.clone())
            .collect();
        let new = dirty.iter().filter(|p| !seen.contains_key(*p)).count();
        grown_total += dirty.len() - new;
        new_total += new;
        live.update(&dirty).unwrap();
        let full = pipe(&root, None, None);
        let after = all_stats(&root);
        seen = cur;
        if after != seen {
            raced += 1;
            let moved: HashSet<String> = after
                .iter()
                .filter(|(p, s)| seen.get(*p) != Some(s))
                .map(|(p, _)| p.display().to_string())
                .collect();
            let (a, b) = (records(&live), records(&full));
            let d: Vec<String> = a
                .keys()
                .chain(b.keys())
                .filter(|k| !moved.contains(*k) && a.get(*k) != b.get(*k))
                .map(|k| format!("records {k}"))
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();
            diffs += d.len();
            note(&mut log, format!("B round {r}: {} dirty ({new} new), {} moved during the full pass: records of the rest {} differences, snapshot not compared", dirty.len(), moved.len(), d.len()));
            log.extend(d.into_iter().take(40));
            continue;
        }
        compared += 1;
        let d = compare(&live, &full, now);
        diffs += d.len();
        note(
            &mut log,
            format!(
                "B round {r}: {} dirty ({new} new): {} differences",
                dirty.len(),
                d.len()
            ),
        );
        log.extend(d.into_iter().take(40));
    }
    note(&mut log, format!("B: {compared} rounds compared, {raced} skipped, {grown_total} grown and {new_total} new files fed"));
    fs::write(out.join("replay-log.txt"), log.join("\n")).unwrap();
    assert_eq!(diffs, 0, "see {}", out.join("replay-log.txt").display());
}
