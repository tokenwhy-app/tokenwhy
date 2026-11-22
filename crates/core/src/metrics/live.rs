// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use super::summary::FileSummary;
use super::{ctx, fresh, CostSum, FileCtx};
use crate::model::{ActiveSession, Live, TimestampMs, Today};
use chrono::{Local, TimeZone};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const ACTIVE_MS: i64 = 10 * 60_000;
const AGENT_RUNNING_MS: i64 = 2 * 60_000;

pub(crate) fn local_midnight(now_ms: TimestampMs) -> TimestampMs {
    Local
        .timestamp_millis_opt(now_ms)
        .single()
        .and_then(|t| t.date_naive().and_hms_opt(0, 0, 0))
        .and_then(|m| Local.from_local_datetime(&m).earliest())
        .map(|m| m.timestamp_millis())
        .unwrap_or(now_ms - now_ms.rem_euclid(86_400_000))
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct Part {
    pub midnight: TimestampMs,
    today_fresh: u64,
    today_cache_read: u64,
    today_cost: CostSum,
    today_any: bool,
    cost: CostSum,
    last_ctx: u64,
    seg_start_ctx: u64,
}

pub(crate) fn part(f: &FileCtx, now_ms: TimestampMs) -> Part {
    let midnight = local_midnight(now_ms);
    let mut p = Part {
        midnight,
        ..Default::default()
    };
    for (i, t) in f
        .turns
        .iter()
        .enumerate()
        .filter(|(_, t)| t.first_ts_ms >= midnight && t.first_ts_ms <= now_ms)
    {
        p.today_fresh += fresh(&t.usage);
        p.today_cache_read += t.usage.cache_read;
        p.today_cost.add(f.costs[i]);
        p.today_any = true;
    }
    for x in &f.costs {
        p.cost.add(*x);
    }
    if let Some(last) = f.turns.last() {
        p.last_ctx = ctx(&last.usage);
        p.seg_start_ctx = f
            .turns
            .iter()
            .find(|t| t.segment == last.segment)
            .map(|t| ctx(&t.usage))
            .unwrap_or(0);
    }
    p
}

impl Part {
    pub(crate) fn roll(&mut self, midnight: TimestampMs) {
        if midnight > self.midnight {
            *self = Part {
                midnight,
                cost: self.cost,
                last_ctx: self.last_ctx,
                seg_start_ctx: self.seg_start_ctx,
                ..Default::default()
            };
        }
    }
}

pub(crate) fn fold(files: &[&FileSummary], now_ms: TimestampMs) -> Live {
    let (mut fresh_t, mut cr) = (0u64, 0u64);
    let mut cost = CostSum::default();
    let mut sessions = BTreeSet::new();
    let mut agents = 0u32;
    let midnight = local_midnight(now_ms);
    for f in files {
        let p = &f.live;
        if p.midnight != midnight || !p.today_any {
            continue;
        }
        fresh_t += p.today_fresh;
        cr += p.today_cache_read;
        if p.today_cost.known {
            cost.add(Some(p.today_cost.usd));
        }
        if f.main {
            sessions.insert((f.provider.clone(), f.session.clone()));
        } else {
            agents += 1;
        }
    }
    let mut active = vec![];
    for f in files
        .iter()
        .filter(|f| f.main && f.last_ts.is_some_and(|t| now_ms - t <= ACTIVE_MS))
    {
        let mut c = CostSum::default();
        let mut running = 0;
        for g in files
            .iter()
            .filter(|g| g.provider == f.provider && g.session == f.session)
        {
            if g.live.cost.known {
                c.add(Some(g.live.cost.usd));
            }
            if !g.main && g.last_ts.is_some_and(|t| now_ms - t <= AGENT_RUNNING_MS) {
                running += 1;
            }
        }
        active.push(ActiveSession {
            provider: f.provider.clone(),
            project: f.project.clone(),
            session: f.session.clone(),
            ctx_tokens: f.live.last_ctx,
            turns: f.turns,
            cost_usd: c.amount(),
            agents_running: running,
            compactions: f.n_compactions,
            ctx_growth_since_boundary: f.live.last_ctx.saturating_sub(f.live.seg_start_ctx),
        });
    }
    active.sort_by_key(|s| std::cmp::Reverse(s.ctx_tokens));
    Live {
        today: Today {
            fresh_tokens: fresh_t,
            cache_read_tokens: cr,
            cost_usd: cost.amount(),
            sessions: sessions.len() as u32,
            agents,
        },
        sessions: active,
    }
}

pub(crate) fn next_expiry(files: &[&FileSummary], now_ms: TimestampMs) -> Option<TimestampMs> {
    files
        .iter()
        .filter_map(|f| {
            let t = f.last_ts?;
            let ttl = if f.main { ACTIVE_MS } else { AGENT_RUNNING_MS };
            (now_ms - t <= ttl).then_some(t + ttl + 1)
        })
        .min()
}
