// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use super::summary::FileSummary;
use super::{ctx, CostSum, FileCtx};
use crate::model::{Amount, HourRow, ProviderId, SessionRow, TimestampMs};
use chrono::{Local, TimeZone, Timelike};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Input = 0,
    Write = 1,
    Read = 2,
    Output = 3,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct KindSpend {
    pub tok: [u64; 4],
    pub usd: [f64; 4],
    pub co2: [f64; 4],
}

impl KindSpend {
    fn add(&mut self, o: &KindSpend) {
        for i in 0..4 {
            self.tok[i] += o.tok[i];
            self.usd[i] += o.usd[i];
            self.co2[i] += o.co2[i];
        }
    }
    pub fn per_token(&self, kind: Kind) -> Option<f64> {
        let i = kind as usize;
        (self.tok[i] > 0).then(|| self.usd[i] / self.tok[i] as f64)
    }
    pub fn co2_per_token(&self, kind: Kind) -> Option<f64> {
        let i = kind as usize;
        (self.tok[i] > 0 && self.co2[i] > 0.0).then(|| self.co2[i] / self.tok[i] as f64)
    }
    pub fn co2_total(&self) -> Option<f64> {
        let g: f64 = self.co2.iter().sum();
        (g > 0.0).then_some(g)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct Part {
    kinds: KindSpend,
    cost: CostSum,
    ctx_sum: u64,
    ctx_max: u64,
    hours: Vec<(TimestampMs, u8, f64)>,
    models: Vec<(String, [u64; 4])>,
}

pub(crate) fn part(f: &FileCtx) -> Part {
    let mut p = Part::default();
    let mut hours: BTreeMap<(TimestampMs, u8), f64> = BTreeMap::new();
    for (i, t) in f.turns.iter().enumerate() {
        let c = ctx(&t.usage);
        p.ctx_sum += c;
        p.ctx_max = p.ctx_max.max(c);
        p.cost.add(f.costs[i]);
        let Some(rate) = f.rates[i] else { continue };
        let mut usd = crate::price::cost_by_kind(rate, &t.usage);
        if let Some(stored) = t.stored_cost {
            let table: f64 = usd.iter().sum();
            if table > 0.0 {
                usd.iter_mut().for_each(|u| *u *= stored / table);
            }
        }
        let u = &t.usage;
        let tok = [
            u.input,
            u.cache_write_5m + u.cache_write_1h,
            u.cache_read,
            u.output,
        ];
        p.kinds.add(&KindSpend {
            tok,
            usd,
            co2: [0.0; 4],
        });
        let m = f.s(t.model);
        match p.models.iter_mut().find(|(n, _)| n == m) {
            Some((_, k)) => k.iter_mut().zip(tok).for_each(|(a, b)| *a += b),
            None => p.models.push((m.to_string(), tok)),
        }
        if let Some(lt) = Local.timestamp_millis_opt(t.first_ts_ms).single() {
            *hours
                .entry((super::live::local_midnight(t.first_ts_ms), lt.hour() as u8))
                .or_default() += f.costs[i].unwrap_or(0.0);
        }
    }
    p.hours = hours.into_iter().map(|((d, h), v)| (d, h, v)).collect();
    p
}

pub struct Spend {
    pub sessions: Vec<SessionRow>,
    pub hourly: Vec<HourRow>,
    pub all: KindSpend,
    pub by_project: HashMap<String, KindSpend>,
}

const MAX_SESSIONS: usize = 300;

#[cfg(feature = "co2")]
fn co2_of(p: &Part, cfg: &crate::co2::Co2Settings) -> [f64; 4] {
    let mut g = [0.0; 4];
    for (m, t) in &p.models {
        if let Some(b) = crate::co2::band(cfg, m) {
            let k = crate::co2::grams_of(&b.mid(), t[0], t[1], t[2], t[3]);
            g.iter_mut().zip(k).for_each(|(a, b)| *a += b);
        }
    }
    g
}

pub(crate) fn fold(files: &[&FileSummary], settings: &crate::model::Settings) -> Spend {
    #[cfg(not(feature = "co2"))]
    let _ = settings;
    let mut all = KindSpend::default();
    let mut by_project: HashMap<String, KindSpend> = HashMap::new();
    let mut hours: BTreeMap<(TimestampMs, u8), f64> = BTreeMap::new();
    let mut sess: HashMap<(ProviderId, String), SessionRow> = HashMap::new();
    let mut cost: HashMap<(ProviderId, String), CostSum> = HashMap::new();
    let mut co2: HashMap<(ProviderId, String), KindSpend> = HashMap::new();
    for f in files {
        let p = &f.spend;
        #[allow(unused_mut)]
        let mut kinds = p.kinds;
        #[cfg(feature = "co2")]
        {
            kinds.co2 = co2_of(p, &settings.co2);
        }
        all.add(&kinds);
        by_project.entry(f.project.clone()).or_default().add(&kinds);
        for (d, h, v) in &p.hours {
            *hours.entry((*d, *h)).or_default() += v;
        }
        let key = (f.provider.clone(), f.session.clone());
        co2.entry(key.clone()).or_default().add(&kinds);
        let c = cost.entry(key.clone()).or_default();
        if p.cost.known {
            c.add(Some(p.cost.usd));
        }
        if f.main {
            let Some(start) = f.first_ts else { continue };
            sess.insert(
                key,
                SessionRow {
                    provider: f.provider.clone(),
                    project: f.project.clone(),
                    session: f.session.clone(),
                    start_ms: start,
                    turns: f.turns,
                    mean_ctx: if f.turns > 0 {
                        (p.ctx_sum as f64 / f.turns as f64).round()
                    } else {
                        0.0
                    },
                    peak_ctx: p.ctx_max,
                    cost_usd: Amount {
                        value: None,
                        est: false,
                    },
                    co2_g: Amount::none(),
                    compactions: f.n_compactions,
                },
            );
        }
    }
    let mut sessions: Vec<SessionRow> = sess
        .into_iter()
        .map(|(k, mut r)| {
            r.cost_usd = cost.get(&k).copied().unwrap_or_default().amount();
            let g = co2.get(&k).and_then(KindSpend::co2_total);
            r.co2_g = Amount {
                value: g,
                est: g.is_some(),
            };
            r
        })
        .collect();
    sessions.sort_by(|a, b| {
        b.cost_usd
            .value
            .unwrap_or(-1.0)
            .total_cmp(&a.cost_usd.value.unwrap_or(-1.0))
            .then_with(|| a.start_ms.cmp(&b.start_ms))
    });
    sessions.truncate(MAX_SESSIONS);
    let hourly = hours
        .into_iter()
        .map(|((day_ms, hour), cost_usd)| HourRow {
            provider: None,
            day_ms,
            hour,
            cost_usd,
        })
        .collect();
    Spend {
        sessions,
        hourly,
        all,
        by_project,
    }
}
