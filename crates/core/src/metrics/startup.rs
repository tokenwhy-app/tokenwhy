// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use super::summary::FileSummary;
use super::util::{median, pct};
use super::{amount, ctx, FileCtx};
use crate::model::{Amount, ProviderId, StartupKind, StartupPartRow, StartupRow};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SessionParts {
    pub raw_ratio: Option<f64>,
    pub in_range: bool,
    pub parts: Vec<(StartupKind, String, f64, bool)>,
    pub unattributed: (f64, bool),
}

const RATIO_MIN: f64 = 0.2;
const RATIO_MAX: f64 = 0.4;

pub(crate) fn file_ratio(f: &FileCtx) -> Option<f64> {
    let p = session_parts(f)?;
    p.in_range.then_some(p.raw_ratio).flatten()
}

pub(crate) fn session_parts(f: &FileCtx) -> Option<SessionParts> {
    let sp = f.pf.records.startup.as_ref()?;
    let s = ctx(&f.turns.first()?.usage);
    let known: Vec<_> = sp
        .parts
        .iter()
        .filter(|p| p.kind != StartupKind::Base)
        .collect();
    let sum: u64 = known.iter().map(|p| p.chars).sum();
    let raw_ratio = (sp.has_full_shape && sum > 0).then(|| s as f64 / sum as f64);
    let in_range = raw_ratio.is_some_and(|r| (RATIO_MIN..=RATIO_MAX).contains(&r));
    let r = if in_range { raw_ratio } else { None };
    let mut parts: Vec<_> = known
        .iter()
        .map(|p| {
            let (t, est) = match r {
                Some(r) => (p.chars as f64 * r, false),
                None => (p.chars as f64 / 4.0, true),
            };
            (p.kind, f.s(p.name).to_string(), t, est)
        })
        .collect();
    let attributed: f64 = parts.iter().map(|p| p.2).sum();
    let gap = (s as f64 - attributed).max(0.0);
    let unattributed = if sp.has_full_shape {
        (gap, !in_range)
    } else {
        parts.push((StartupKind::Base, String::new(), gap, true));
        (0.0, false)
    };
    Some(SessionParts {
        raw_ratio,
        in_range,
        parts,
        unattributed,
    })
}

#[derive(Debug, Clone)]
pub struct ProjectStartup {
    pub provider: ProviderId,
    pub project: String,
    pub sessions: u32,
    pub median: f64,
    pub agent_median: f64,
    pub agent_total: u64,
    pub agents: u32,
}

#[derive(Default)]
struct Acc {
    s: Vec<f64>,
    ratios: Vec<f64>,
    all_in_range: bool,
    with_parts: u32,
    parts: BTreeMap<(StartupKind, String), (Vec<f64>, bool)>,
    unattributed: (Vec<f64>, bool),
    agent_a: Vec<f64>,
}

pub(crate) fn fold(files: &[&FileSummary]) -> (Vec<StartupRow>, Vec<ProjectStartup>) {
    let mut by: BTreeMap<(ProviderId, String), Acc> = BTreeMap::new();
    for f in files {
        let Some(first_ctx) = f.first_ctx else {
            continue;
        };
        if f.head_copied {
            continue;
        }
        let acc = by
            .entry((f.provider.clone(), f.project.clone()))
            .or_insert_with(|| Acc {
                all_in_range: true,
                ..Default::default()
            });
        if !f.main {
            acc.agent_a.push(first_ctx as f64);
            continue;
        }
        acc.s.push(first_ctx as f64);
        if let Some(p) = &f.parts {
            acc.with_parts += 1;
            match (p.in_range, p.raw_ratio) {
                (true, Some(r)) => acc.ratios.push(r),
                _ => acc.all_in_range = false,
            }
            for (k, n, t, est) in &p.parts {
                let e = acc.parts.entry((*k, n.clone())).or_default();
                e.0.push(*t);
                e.1 |= est;
            }
            acc.unattributed.0.push(p.unattributed.0);
            acc.unattributed.1 |= p.unattributed.1;
        }
    }
    let mut rows = vec![];
    let mut projects = vec![];
    for ((provider, project), a) in by {
        projects.push(ProjectStartup {
            provider: provider.clone(),
            project: project.clone(),
            sessions: a.s.len() as u32,
            median: median(&a.s),
            agent_median: median(&a.agent_a),
            agent_total: a.agent_a.iter().sum::<f64>() as u64,
            agents: a.agent_a.len() as u32,
        });
        if a.s.is_empty() {
            continue;
        }
        let mut parts: Vec<StartupPartRow> = a
            .parts
            .into_iter()
            .map(|((kind, name), (v, est))| StartupPartRow {
                kind,
                name,
                tokens: amount(median(&v).round(), est),
            })
            .collect();
        parts.sort_by(|x, y| {
            y.tokens
                .value
                .unwrap_or(0.0)
                .total_cmp(&x.tokens.value.unwrap_or(0.0))
        });
        let ratio = (a.with_parts > 0 && a.all_in_range).then(|| median(&a.ratios));
        rows.push(StartupRow {
            provider,
            project,
            sessions: a.s.len() as u32,
            median: median(&a.s).round() as u64,
            p90: pct(&a.s, 0.9) as u64,
            min: a.s.iter().cloned().fold(f64::MAX, f64::min) as u64,
            max: a.s.iter().cloned().fold(0.0, f64::max) as u64,
            ratio,
            parts,
            unattributed: if a.with_parts > 0 {
                amount(median(&a.unattributed.0).round(), a.unattributed.1)
            } else {
                Amount {
                    value: None,
                    est: false,
                }
            },
        });
    }
    (rows, projects)
}
