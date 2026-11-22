// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use super::startup::ProjectStartup;
use super::summary::FileSummary;
use super::{amount, ctx, CostSum, FileCtx};
use crate::model::{Amount, Category, CompactionRow, IdHash};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone)]
pub struct CompactionAgg {
    pub project: String,
    pub session: String,
    pub carry_ratio: Option<f64>,
    pub rebuild_tokens: f64,
}

fn mean(c: CostSum, n: usize) -> Amount {
    if n == 0 || !c.known {
        return Amount {
            value: None,
            est: false,
        };
    }
    amount(c.usd / n as f64, false)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Part {
    pre: u64,
    post: u64,
    first_after_ctx: u64,
    has_first_after: bool,
    before: CostSum,
    nb: usize,
    after: CostSum,
    na: usize,
    rebuild: f64,
    rate: Option<(f64, f64)>,
    ts: i64,
}

pub(crate) fn parts(f: &FileCtx) -> Vec<Part> {
    let mut out = vec![];
    let mut comps: Vec<_> = f.pf.records.compactions.iter().collect();
    comps.sort_by_key(|c| c.order);
    for (i, c) in comps.iter().enumerate() {
        let (b, a) = (i as u32, i as u32 + 1);
        let (mut before, mut after) = (CostSum::default(), CostSum::default());
        let (mut nb, mut na) = (0usize, 0usize);
        let mut first_after = None;
        for (j, t) in f.turns.iter().enumerate() {
            if t.segment == b {
                before.add(f.costs[j]);
                nb += 1;
            } else if t.segment == a {
                after.add(f.costs[j]);
                na += 1;
                first_after.get_or_insert(j);
            }
        }
        let reads = |seg: &dyn Fn(u32) -> bool| -> Vec<(IdHash, u64)> {
            f.pf.records
                .tool_calls
                .iter()
                .filter(|c| matches!(c.category, Category::ReadFile | Category::Search))
                .filter(|c| seg(f.segment_at(c.turn_order)))
                .filter_map(|c| c.path.map(|(h, _)| (h, c.result_chars)))
                .collect()
        };
        let earlier: BTreeSet<IdHash> = reads(&|s| s < a).into_iter().map(|(h, _)| h).collect();
        let rebuild_chars: u64 = reads(&|s| s == a)
            .into_iter()
            .filter(|(h, _)| earlier.contains(h))
            .map(|(_, n)| n)
            .sum();
        let rate = first_after
            .and_then(|j| f.rates[j])
            .or_else(|| f.rate_near(c.order));
        out.push(Part {
            pre: c.pre_tokens,
            post: c.post_tokens,
            first_after_ctx: first_after.map(|j| ctx(&f.turns[j].usage)).unwrap_or(0),
            has_first_after: first_after.is_some(),
            before,
            nb,
            after,
            na,
            rebuild: rebuild_chars as f64 / 4.0,
            rate: rate.map(|r| (r.cache_read, r.cache_write_5m)),
            ts: first_after
                .map(|j| f.turns[j].first_ts_ms)
                .or_else(|| {
                    f.turns
                        .iter()
                        .rev()
                        .find(|t| t.order < c.order)
                        .map(|t| t.first_ts_ms)
                })
                .unwrap_or(0),
        });
    }
    out
}

pub(crate) fn fold(
    files: &[&FileSummary],
    startup: &[ProjectStartup],
) -> (Vec<CompactionRow>, Vec<CompactionAgg>) {
    let mut rows = vec![];
    let mut aggs = vec![];
    for f in files.iter().filter(|f| f.main) {
        let med_startup = startup
            .iter()
            .find(|p| p.provider == f.provider && p.project == f.project)
            .map(|p| p.median)
            .filter(|m| *m > 0.0);
        for c in &f.compactions {
            let fa_ctx = c.first_after_ctx;
            let carry_ratio = match (c.has_first_after, med_startup) {
                (true, Some(m)) => Some(fa_ctx as f64 / m),
                _ => None,
            };
            let fresh_restart = match (c.rate, med_startup) {
                (Some((cache_read, cache_write_5m)), Some(m)) => amount(
                    (c.na as f64 * m * cache_read + m * cache_write_5m) / 1e6,
                    false,
                ),
                _ => Amount {
                    value: None,
                    est: false,
                },
            };
            rows.push(CompactionRow {
                provider: f.provider.clone(),
                project: f.project.clone(),
                session: f.session.clone(),
                ts_ms: c.ts,
                pre_tokens: c.pre,
                post_tokens: c.post,
                first_after_ctx: fa_ctx,
                cost_per_turn_before: mean(c.before, c.nb),
                cost_per_turn_after: mean(c.after, c.na),
                rebuild_tokens: amount(c.rebuild, true),
                carry_ratio,
                fresh_restart_cost_usd: fresh_restart,
            });
            aggs.push(CompactionAgg {
                project: f.project.clone(),
                session: f.session.clone(),
                carry_ratio,
                rebuild_tokens: c.rebuild,
            });
        }
    }
    (rows, aggs)
}
