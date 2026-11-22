// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use super::summary::FileSummary;
use super::util::median;
use super::{amount, CostSum, FileCtx};
use crate::model::{Category, IdHash, MemoryRow, StartupKind};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
pub struct MemoryAgg {
    pub kind: StartupKind,
    pub path: String,
    pub projects: Vec<String>,
    pub bytes: u64,
    pub tokens: f64,
    pub est: bool,
    pub files: u32,
    pub duplicate_lines: u32,
    pub duplicate_tokens: f64,
}

#[derive(Default)]
struct Acc {
    path: Option<String>,
    kind: Option<StartupKind>,
    projects: BTreeSet<String>,
    bytes: u64,
    tokens: Vec<f64>,
    est: bool,
    carried: CostSum,
    files: u32,
    reads: u32,
    dups: u32,
    dup_tokens: f64,
}

fn is_instructions(k: StartupKind) -> bool {
    matches!(
        k,
        StartupKind::UserInstructions | StartupKind::ProjectInstructions | StartupKind::Memory
    )
}
fn norm(p: &str) -> String {
    let s = p.replace('\\', "/").to_lowercase();
    let b = s.as_bytes();
    if b.len() >= 3 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b'/' {
        return format!("{}:{}", b[1] as char, &s[2..]);
    }
    s
}

pub(crate) fn label(path: &str) -> String {
    let parts: Vec<&str> = path.split(['/', '\\']).filter(|s| !s.is_empty()).collect();
    let n = parts.len();
    let Some(name) = parts.last() else {
        return path.to_string();
    };
    let dir =
        if n >= 3 && (parts[n - 2].eq_ignore_ascii_case("memory") || parts[n - 2] == ".claude") {
            parts[n - 3]
        } else if n >= 2 {
            parts[n - 2]
        } else {
            return name.to_string();
        };
    format!("{dir} · {name}")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Loaded {
    path: String,
    kind: StartupKind,
    chars: u64,
    tokens: f64,
    est: bool,
    carried: Option<f64>,
    dup: Option<(u32, f64)>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct Part {
    loaded: Vec<Loaded>,
    reads: Vec<String>,
}

pub(crate) fn part(f: &FileCtx) -> Part {
    let mut out = Part::default();
    if let Some(sp) = &f.pf.records.startup {
        let n_turns = f.turns.len() as f64;
        let rate = f.first_rate();
        let lines_of = |name: u32| -> BTreeSet<IdHash> {
            sp.instruction_lines
                .iter()
                .filter(|(n, _)| *n == name)
                .flat_map(|(_, l)| l.iter().copied())
                .collect()
        };
        let user_lines: BTreeSet<IdHash> = sp
            .parts
            .iter()
            .filter(|p| p.kind == StartupKind::UserInstructions)
            .flat_map(|p| lines_of(p.name))
            .collect();
        for p in sp.parts.iter().filter(|p| is_instructions(p.kind)) {
            let (tok, est) = f.tok(p.chars);
            let dup = (p.kind != StartupKind::UserInstructions).then(|| {
                let lines = lines_of(p.name);
                let d = lines.intersection(&user_lines).count() as u32;
                (
                    d,
                    if d > 0 {
                        tok * d as f64 / lines.len() as f64
                    } else {
                        0.0
                    },
                )
            });
            out.loaded.push(Loaded {
                path: f.s(p.name).to_string(),
                kind: p.kind,
                chars: p.chars,
                tokens: tok,
                est,
                carried: rate.map(|r| tok * (f.write_rate(r) + r.cache_read * n_turns) / 1e6),
                dup,
            });
        }
    }
    let loaded: BTreeSet<String> = out.loaded.iter().map(|l| norm(&l.path)).collect();
    for c in
        f.pf.records
            .tool_calls
            .iter()
            .filter(|c| c.category == Category::ReadFile)
    {
        let Some((_, pi)) = c.path else { continue };
        let path = f.s(pi);
        if loaded.contains(&norm(path)) {
            out.reads.push(path.to_string());
        }
    }
    out
}

pub(crate) fn fold(files: &[&FileSummary]) -> (Vec<MemoryRow>, Vec<MemoryAgg>) {
    let mut by: BTreeMap<String, Acc> = BTreeMap::new();
    for f in files {
        for l in &f.memory.loaded {
            let a = by.entry(norm(&l.path)).or_default();
            a.path.get_or_insert_with(|| l.path.clone());
            a.kind = Some(l.kind);
            a.projects.insert(f.project.clone());
            a.bytes = a.bytes.max(l.chars);
            if l.chars > 0 {
                a.tokens.push(l.tokens / l.chars as f64);
            }
            a.est |= l.est;
            a.files += 1;
            a.carried.add(l.carried);
            if let Some((d, dup_tokens)) = l.dup {
                if d > a.dups {
                    a.dups = d;
                    a.dup_tokens = dup_tokens;
                }
            }
        }
        for path in &f.memory.reads {
            if let Some(a) = by.get_mut(&norm(path)) {
                a.reads += 1;
            }
        }
    }
    let mut rows = vec![];
    let mut aggs = vec![];
    for (key, a) in by {
        let path = a.path.clone().unwrap_or(key);
        let kind = a.kind.unwrap_or(StartupKind::Memory);
        // Token figure for the largest version seen, so it matches `bytes` (earlier, smaller
        // versions of the same file otherwise drag a median of raw token counts down).
        let tokens = median(&a.tokens) * a.bytes as f64;
        let mut carried = a.carried.amount();
        carried.est = a.est;
        rows.push(MemoryRow {
            provider: None,
            kind,
            label: label(&path),
            path: path.clone(),
            bytes: a.bytes,
            tokens: amount(tokens.round(), a.est),
            carried_cost_usd: carried,
            on_demand_reads: a.reads,
            duplicate_lines: a.dups,
        });
        aggs.push(MemoryAgg {
            kind,
            path,
            projects: a.projects.into_iter().collect(),
            bytes: a.bytes,
            tokens,
            est: a.est,
            files: a.files,
            duplicate_lines: a.dups,
            duplicate_tokens: a.dup_tokens,
        });
    }
    (rows, aggs)
}
