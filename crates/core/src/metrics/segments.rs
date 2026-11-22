// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use super::compaction::CompactionAgg;
use super::summary::FileSummary;
use super::{ctx, CostSum, FileCtx};
use crate::model::TimestampMs;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct SegmentAgg {
    pub project: String,
    pub session: String,
    pub start_ms: TimestampMs,
    pub segment: u32,
    pub turns: u32,
    pub mean_ctx: f64,
    pub max_ctx: u64,
    pub output: u64,
    pub usd: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct SessionAgg {
    pub project: String,
    pub session: String,
    pub start_ms: TimestampMs,
    pub turns: u32,
    pub compactions: u32,
    pub max_ctx: u64,
    pub max_carry_ratio: Option<f64>,
    pub cache_hit: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct Part {
    segs: Vec<(u32, u32, u64, u64, u64, CostSum)>,
    cache_read: u64,
    ctx_all: u64,
}

pub(crate) fn part(f: &FileCtx) -> Part {
    let mut by: BTreeMap<u32, (u32, u64, u64, u64, CostSum)> = BTreeMap::new();
    let (mut cr, mut all) = (0u64, 0u64);
    for (i, t) in f.turns.iter().enumerate() {
        let c = ctx(&t.usage);
        let e = by.entry(t.segment).or_default();
        e.0 += 1;
        e.1 += c;
        e.2 = e.2.max(c);
        e.3 += t.usage.output;
        e.4.add(f.costs[i]);
        cr += t.usage.cache_read;
        all += c;
    }
    Part {
        segs: by
            .into_iter()
            .map(|(k, v)| (k, v.0, v.1, v.2, v.3, v.4))
            .collect(),
        cache_read: cr,
        ctx_all: all,
    }
}

pub(crate) fn fold(
    files: &[&FileSummary],
    comps: &[CompactionAgg],
) -> (Vec<SessionAgg>, Vec<SegmentAgg>) {
    let mut sessions = vec![];
    let mut segs = vec![];
    for f in files.iter().filter(|f| f.main) {
        let Some(start) = f.first_ts else { continue };
        let p = &f.segments;
        for (segment, n, sum, max, out, cost) in &p.segs {
            segs.push(SegmentAgg {
                project: f.project.clone(),
                session: f.session.clone(),
                start_ms: start,
                segment: *segment,
                turns: *n,
                mean_ctx: *sum as f64 / *n as f64,
                max_ctx: *max,
                output: *out,
                usd: cost.known.then_some(cost.usd),
            });
        }
        let mine: Vec<_> = comps
            .iter()
            .filter(|c| c.session == f.session && c.project == f.project)
            .collect();
        sessions.push(SessionAgg {
            project: f.project.clone(),
            session: f.session.clone(),
            start_ms: start,
            turns: f.turns,
            compactions: f.n_compactions,
            max_ctx: p.segs.iter().map(|v| v.3).max().unwrap_or(0),
            max_carry_ratio: mine.iter().filter_map(|c| c.carry_ratio).reduce(f64::max),
            cache_hit: if p.ctx_all > 0 {
                p.cache_read as f64 / p.ctx_all as f64
            } else {
                1.0
            },
        });
    }
    (sessions, segs)
}
