// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use crate::model::{
    ActiveSession, Amount, FileRole, Live, ParsedFile, PriceTable, TimestampMs, Today, Turn,
};
use crate::price;
use chrono::{Local, TimeZone};
use std::collections::HashSet;

pub const ACTIVE_MAIN_MS: i64 = 10 * 60_000;
pub const ACTIVE_AGENT_MS: i64 = 2 * 60_000;

fn ctx(t: &Turn) -> u64 {
    t.usage.input + t.usage.cache_write_5m + t.usage.cache_write_1h + t.usage.cache_read
}

fn fresh(t: &Turn) -> u64 {
    t.usage.input + t.usage.cache_write_5m + t.usage.cache_write_1h + t.usage.output
}

fn cost_of<'a>(
    f: &ParsedFile,
    turns: impl Iterator<Item = &'a Turn>,
    prices: &[PriceTable],
) -> (f64, bool) {
    let table = prices.iter().find(|p| p.provider == f.source.provider);
    let (mut sum, mut missing) = (0.0, false);
    for t in turns {
        let model = f
            .records
            .strings
            .get(t.model as usize)
            .map(String::as_str)
            .unwrap_or("");
        if let Some(c) = t.stored_cost {
            sum += c;
            continue;
        }
        match table.and_then(|tb| price::rate_for(tb, model)) {
            Some(rate) => sum += price::cost(rate, &t.usage),
            None => missing = true,
        }
    }
    (sum, missing)
}

fn amount(sum: f64, missing: bool) -> Amount {
    Amount {
        value: Some(sum),
        est: missing,
    }
}

fn local_midnight_ms(now_ms: TimestampMs) -> TimestampMs {
    let Some(now) = Local.timestamp_millis_opt(now_ms).single() else {
        return now_ms - now_ms.rem_euclid(86_400_000);
    };
    let day = now.date_naive();
    day.and_hms_opt(0, 0, 0)
        .and_then(|d| Local.from_local_datetime(&d).earliest())
        .map_or(now_ms - now_ms.rem_euclid(86_400_000), |d| {
            d.timestamp_millis()
        })
}

fn last_ts(f: &ParsedFile) -> Option<TimestampMs> {
    f.records.turns.iter().map(|t| t.first_ts_ms).max()
}

pub fn live(
    files: &[ParsedFile],
    prices: &[PriceTable],
    now_ms: TimestampMs,
    mtime_ms: &dyn Fn(&std::path::Path) -> Option<TimestampMs>,
) -> Live {
    let written = |f: &ParsedFile| mtime_ms(&f.source.path).or_else(|| last_ts(f)).unwrap_or(0);

    let midnight = local_midnight_ms(now_ms);
    let mut today = Today {
        fresh_tokens: 0,
        cache_read_tokens: 0,
        cost_usd: amount(0.0, false),
        sessions: 0,
        agents: 0,
    };
    let (mut tsum, mut tmiss) = (0.0, false);
    let mut sessions = HashSet::new();
    for f in files {
        let turns: Vec<&Turn> = f
            .records
            .turns
            .iter()
            .filter(|t| t.first_ts_ms >= midnight && t.first_ts_ms <= now_ms)
            .collect();
        if turns.is_empty() {
            continue;
        }
        match f.source.role {
            FileRole::Main => {
                sessions.insert((f.source.provider.clone(), f.source.session_id.clone()));
            }
            FileRole::Agent => today.agents += 1,
        }
        for t in &turns {
            today.fresh_tokens += fresh(t);
            today.cache_read_tokens += t.usage.cache_read;
        }
        let (s, m) = cost_of(f, turns.into_iter(), prices);
        tsum += s;
        tmiss |= m;
    }
    for f in files.iter().filter(|f| f.source.role == FileRole::Agent) {
        if f.records
            .turns
            .iter()
            .any(|t| t.first_ts_ms >= midnight && t.first_ts_ms <= now_ms)
        {
            sessions.insert((f.source.provider.clone(), f.source.session_id.clone()));
        }
    }
    today.sessions = sessions.len() as u32;
    today.cost_usd = amount(tsum, tmiss);

    let mut active: Vec<(i64, ActiveSession)> = Vec::new();
    for main in files.iter().filter(|f| f.source.role == FileRole::Main) {
        let w = written(main);
        if now_ms - w > ACTIVE_MAIN_MS || main.records.turns.is_empty() {
            continue;
        }
        let same = |f: &&ParsedFile| {
            f.source.provider == main.source.provider
                && f.source.project_key == main.source.project_key
                && f.source.session_id == main.source.session_id
        };
        let agents: Vec<&ParsedFile> = files
            .iter()
            .filter(|f| f.source.role == FileRole::Agent)
            .filter(same)
            .collect();
        let mut turns: Vec<&Turn> = main.records.turns.iter().collect();
        turns.sort_by_key(|t| t.order);
        let last = turns[turns.len() - 1];
        let boundary_first = turns
            .iter()
            .find(|t| t.segment == last.segment)
            .copied()
            .unwrap_or(last);
        let (mut s, mut m) = cost_of(main, turns.iter().copied(), prices);
        for a in &agents {
            let (s2, m2) = cost_of(a, a.records.turns.iter(), prices);
            s += s2;
            m |= m2;
        }
        let project = main
            .records
            .project_name
            .and_then(|i| main.records.strings.get(i as usize).cloned())
            .unwrap_or_else(|| main.source.project_key.clone());
        active.push((
            w,
            ActiveSession {
                provider: main.source.provider.clone(),
                project,
                session: main.source.session_id.clone(),
                ctx_tokens: ctx(last),
                turns: turns.len() as u32,
                cost_usd: amount(s, m),
                agents_running: agents
                    .iter()
                    .filter(|a| now_ms - written(a) <= ACTIVE_AGENT_MS)
                    .count() as u32,
                compactions: main.records.compactions.len() as u32,
                ctx_growth_since_boundary: ctx(last).saturating_sub(ctx(boundary_first)),
            },
        ));
    }
    active.sort_by(|a, b| b.0.cmp(&a.0));
    Live {
        today,
        sessions: active.into_iter().map(|(_, s)| s).collect(),
    }
}
