// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use super::{fresh, live::local_midnight, CostSum, FileCtx};
use crate::model::{Category, TimestampMs};
use chrono::{Local, TimeZone, Timelike};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DayPart {
    pub day_ms: TimestampMs,
    pub model: String,
    pub fresh: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub cost_usd: f64,
    pub cost_known: bool,
    pub compactions: u32,
    pub output: Option<u64>,
    pub edits: Option<u32>,
    pub hours: Option<u32>,
    pub langs: Option<Vec<(u8, u32, u32)>>,
}

pub(crate) fn part(f: &FileCtx) -> Vec<DayPart> {
    let mut out: Vec<DayPart> = vec![];
    let slot = |out: &mut Vec<DayPart>, day: TimestampMs, model: &str| -> usize {
        match out.iter().position(|d| d.day_ms == day && d.model == model) {
            Some(i) => i,
            None => {
                out.push(DayPart {
                    day_ms: day,
                    model: model.to_string(),
                    fresh: 0,
                    cache_read: 0,
                    cache_write: 0,
                    cost_usd: 0.0,
                    cost_known: false,
                    compactions: 0,
                    output: Some(0),
                    edits: Some(0),
                    hours: Some(0),
                    langs: Some(vec![]),
                });
                out.len() - 1
            }
        }
    };
    for (i, t) in f.turns.iter().enumerate() {
        let k = slot(&mut out, local_midnight(t.first_ts_ms), f.model_of(t));
        let d = &mut out[k];
        d.fresh += fresh(&t.usage);
        d.cache_read += t.usage.cache_read;
        d.cache_write += t.usage.cache_write_5m + t.usage.cache_write_1h;
        let mut c = CostSum {
            usd: d.cost_usd,
            known: d.cost_known,
        };
        c.add(f.costs[i]);
        (d.cost_usd, d.cost_known) = (c.usd, c.known);
        *d.output.get_or_insert(0) += t.usage.output;
        if f.costs[i].is_some_and(|c| c > 0.0) {
            if let Some(lt) = Local.timestamp_millis_opt(t.first_ts_ms).single() {
                *d.hours.get_or_insert(0) |= 1 << lt.hour();
            }
        }
    }
    let strings = &f.pf.records.strings;
    for c in
        f.pf.records
            .tool_calls
            .iter()
            .filter(|c| !c.is_error && matches!(c.category, Category::Edit | Category::ReadFile))
    {
        let Some(t) = f.turns.iter().find(|t| t.order == c.turn_order) else {
            continue;
        };
        let k = slot(&mut out, local_midnight(t.first_ts_ms), f.model_of(t));
        let edit = c.category == Category::Edit;
        if edit {
            *out[k].edits.get_or_insert(0) += 1;
        }
        let lang = c
            .path
            .and_then(|(_, i)| strings.get(i as usize))
            .and_then(|p| crate::lang::of_path(p));
        if let Some(id) = lang {
            let langs = out[k].langs.get_or_insert_with(Vec::new);
            let i = match langs.binary_search_by_key(&id, |l| l.0) {
                Ok(i) => i,
                Err(i) => {
                    langs.insert(i, (id, 0, 0));
                    i
                }
            };
            if edit {
                langs[i].1 += 1;
            } else {
                langs[i].2 += 1;
            }
        }
    }
    for c in &f.pf.records.compactions {
        let Some(t) = f
            .turns
            .iter()
            .find(|t| t.order > c.order)
            .or(f.turns.last())
        else {
            continue;
        };
        let k = slot(&mut out, local_midnight(t.first_ts_ms), f.model_of(t));
        out[k].compactions += 1;
    }
    out.sort_by(|a, b| (a.day_ms, &a.model).cmp(&(b.day_ms, &b.model)));
    out
}
