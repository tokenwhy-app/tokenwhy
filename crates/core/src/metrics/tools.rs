// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use super::startup::session_parts;
use super::summary::FileSummary;
use super::util::{median, pct};
use super::{amount, FileCtx};
use crate::model::{Category, IdHash, Query, Settings, StartupKind, ToolRow};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default)]
pub struct ToolTotals {
    pub carried: f64,
    pub by_category: BTreeMap<Category, f64>,
    pub by_detail: Vec<(Category, String, f64)>,
    pub search_chars: Vec<u64>,
    pub big_results: u32,
    pub biggest: Option<(Category, String, u64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reread {
    pub project: String,
    pub session: String,
    pub path: String,
    pub count: u32,
    pub tokens: f64,
}

#[derive(Debug, Clone)]
pub struct McpAgg {
    pub server: String,
    pub tools_loaded: u32,
    pub tools_called: u32,
    pub sessions_loaded: u32,
    pub listing_tokens_per_session: f64,
    pub est: bool,
}

pub(crate) struct ToolsOut {
    pub rows: Vec<ToolRow>,
    pub totals: ToolTotals,
    pub rereads: Vec<Reread>,
    pub mcp: Vec<McpAgg>,
}

#[derive(Default)]
struct Acc {
    chars: Vec<f64>,
    first: f64,
    carried: f64,
    over: u32,
    persisted: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Group {
    category: Category,
    detail: String,
    chars: Vec<u64>,
    first: f64,
    carried: f64,
    persisted: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct Part {
    groups: Vec<Group>,
    by_category: Vec<(Category, f64)>,
    carried: f64,
    search_chars: Vec<u64>,
    biggest: Option<(Category, String, u64)>,
    rereads: Vec<Reread>,
    mcp_loaded: Vec<(String, String)>,
    mcp_called: Vec<(String, String)>,
    mcp_listing: Vec<(String, f64, bool)>,
}

impl Part {
    pub(crate) fn retain_loaded(&mut self, keep: impl FnMut(&(String, String)) -> bool) {
        self.mcp_loaded.retain(keep);
    }
}

pub(crate) fn part(f: &FileCtx) -> Part {
    let mut p = Part::default();
    let mut idx: BTreeMap<(Category, String), usize> = BTreeMap::new();
    for c in &f.pf.records.tool_calls {
        let detail = c.detail.map(|d| f.s(d).to_string()).unwrap_or_default();
        let first = c.result_chars as f64 / 4.0;
        let carried = first * f.turns_after_in_segment(c.turn_order) as f64;
        let i = *idx.entry((c.category, detail.clone())).or_insert_with(|| {
            p.groups.push(Group {
                category: c.category,
                detail: detail.clone(),
                chars: vec![],
                first: 0.0,
                carried: 0.0,
                persisted: 0,
            });
            p.groups.len() - 1
        });
        let g = &mut p.groups[i];
        g.chars.push(c.result_chars);
        g.first += first;
        g.carried += carried;
        g.persisted += c.persisted as u32;
        p.carried += carried;
        match p.by_category.iter_mut().find(|(k, _)| *k == c.category) {
            Some(e) => e.1 += carried,
            None => p.by_category.push((c.category, carried)),
        }
        if c.category == Category::Search {
            p.search_chars.push(c.result_chars);
        }
        if p.biggest.as_ref().is_none_or(|b| c.result_chars > b.2) && c.result_chars > 0 {
            p.biggest = Some((c.category, detail, c.result_chars));
        }
    }
    if f.is_main() {
        p.rereads = file_rereads(f);
    }
    for (s, t) in &f.pf.records.mcp.loaded {
        p.mcp_loaded
            .push((f.s(*s).to_string(), f.s(*t).to_string()));
    }
    for c in
        f.pf.records
            .tool_calls
            .iter()
            .filter(|c| c.category == Category::Mcp)
    {
        if let Some(d) = c.detail {
            p.mcp_called
                .push((f.s(d).to_string(), f.s(c.tool).to_string()));
        }
    }
    if f.is_main() {
        if let Some(sp) = session_parts(f) {
            let mut per: BTreeMap<String, (f64, bool)> = BTreeMap::new();
            for (kind, name, tok, est) in &sp.parts {
                if matches!(
                    kind,
                    StartupKind::McpTools
                        | StartupKind::DeferredTools
                        | StartupKind::McpInstructions
                ) {
                    let e = per.entry(name.clone()).or_default();
                    e.0 += tok;
                    e.1 |= *est;
                }
            }
            p.mcp_listing = per.into_iter().map(|(s, (t, e))| (s, t, e)).collect();
        }
    }
    p
}

pub(crate) fn fold(files: &[&FileSummary], query: &Query, settings: &Settings) -> ToolsOut {
    let big = settings.thresholds.big_result_chars;
    let mut by: BTreeMap<(Category, String), Acc> = BTreeMap::new();
    let mut totals = ToolTotals::default();
    let mut rereads = vec![];
    for f in files {
        let p = &f.tools;
        for g in &p.groups {
            let a = by.entry((g.category, g.detail.clone())).or_default();
            a.chars.extend(g.chars.iter().map(|c| *c as f64));
            a.first += g.first;
            a.carried += g.carried;
            a.over += g.chars.iter().filter(|c| **c > big).count() as u32;
            a.persisted += g.persisted;
            totals.big_results += g.chars.iter().filter(|c| **c > big).count() as u32;
        }
        totals.carried += p.carried;
        for (c, v) in &p.by_category {
            *totals.by_category.entry(*c).or_default() += v;
        }
        totals.search_chars.extend(&p.search_chars);
        if let Some(b) = &p.biggest {
            if totals.biggest.as_ref().is_none_or(|t| b.2 > t.2) {
                totals.biggest = Some(b.clone());
            }
        }
        rereads.extend(p.rereads.iter().cloned());
    }
    let project = query.project.clone();
    let rows = by
        .iter()
        .map(|((category, detail), a)| ToolRow {
            provider: None,
            category: *category,
            detail: (!detail.is_empty()).then(|| detail.clone()),
            project: project.clone(),
            calls: a.chars.len() as u32,
            first_tokens: amount(a.first.round(), true),
            carried_tokens: amount(a.carried.round(), true),
            share: if totals.carried > 0.0 {
                a.carried / totals.carried
            } else {
                0.0
            },
            median_chars: median(&a.chars).round() as u64,
            p95_chars: pct(&a.chars, 0.95) as u64,
            over_25k: a.over,
            persisted: a.persisted,
        })
        .collect::<Vec<_>>();
    totals.by_detail = by
        .into_iter()
        .map(|((c, d), a)| (c, d, a.carried))
        .collect();
    let mut rows = rows;
    rows.sort_by(|x, y| y.share.total_cmp(&x.share));
    ToolsOut {
        rows,
        totals,
        rereads,
        mcp: mcp(files),
    }
}

/// Rewritten from codeburn optimize.ts:1473.
fn file_rereads(f: &FileCtx) -> Vec<Reread> {
    let mut calls: Vec<_> =
        f.pf.records
            .tool_calls
            .iter()
            .filter(|c| c.path.is_some())
            .collect();
    calls.sort_by_key(|c| c.turn_order);
    let mut run: BTreeMap<(u32, IdHash), (u32, f64)> = BTreeMap::new();
    let mut best: BTreeMap<(u32, IdHash), (u32, f64, u32)> = BTreeMap::new();
    for c in calls {
        let (h, name) = c.path.unwrap();
        let key = (f.segment_at(c.turn_order), h);
        if c.writes_path {
            run.remove(&key);
            continue;
        }
        if c.category != Category::ReadFile {
            continue;
        }
        let r = run.entry(key).or_default();
        r.0 += 1;
        r.1 += c.result_chars as f64 / 4.0;
        let b = best.entry(key).or_insert((0, 0.0, name));
        if r.0 > b.0 {
            *b = (r.0, r.1, name);
        }
    }
    best.into_values()
        .filter(|b| b.0 >= 2)
        .map(|(count, tokens, name)| Reread {
            project: f.project.clone(),
            session: f.session().to_string(),
            path: f.s(name).to_string(),
            count,
            tokens,
        })
        .collect()
}

/// Rewritten from codeburn optimize.ts:1842.
fn mcp(files: &[&FileSummary]) -> Vec<McpAgg> {
    let mut loaded: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut sessions: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut called: Vec<(&str, &str)> = vec![];
    let mut listing: BTreeMap<String, (Vec<f64>, bool)> = BTreeMap::new();
    for f in files {
        let p = &f.tools;
        for (server, tool) in &p.mcp_loaded {
            loaded
                .entry(server.clone())
                .or_default()
                .insert(tool.clone());
            sessions
                .entry(server.clone())
                .or_default()
                .insert(f.session.clone());
        }
        called.extend(p.mcp_called.iter().map(|(s, t)| (s.as_str(), t.as_str())));
        for (s, t, est) in &p.mcp_listing {
            let e = listing.entry(s.clone()).or_default();
            e.0.push(*t);
            e.1 |= *est;
        }
    }
    loaded
        .into_iter()
        .map(|(server, tools)| {
            let tools_called = tools
                .iter()
                .filter(|t| {
                    called
                        .iter()
                        .any(|(s, c)| *s == server && (*c == t.as_str() || c.ends_with(t.as_str())))
                })
                .count();
            let (l, est) = listing.get(&server).cloned().unwrap_or_default();
            McpAgg {
                tools_loaded: tools.len() as u32,
                tools_called: tools_called as u32,
                sessions_loaded: sessions.get(&server).map_or(0, |s| s.len() as u32),
                listing_tokens_per_session: median(&l),
                est,
                server,
            }
        })
        .collect()
}
