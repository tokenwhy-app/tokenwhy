// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use super::summary::FileSummary;
use super::util::median;
use super::{amount, fresh, CostSum, FileCtx};
use crate::model::{Amount, ProviderId, SubagentRow};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct ProjectCost {
    pub project: String,
    pub main_usd: f64,
    pub sub_usd: f64,
    pub main_fresh: u64,
    pub sub_fresh: u64,
    pub agents: u32,
    pub agent_startup_median: f64,
    pub agent_startup_total: u64,
}

impl ProjectCost {
    pub fn cost_share(&self) -> f64 {
        let t = self.main_usd + self.sub_usd;
        if t > 0.0 {
            self.sub_usd / t
        } else {
            0.0
        }
    }
}

#[derive(Debug, Clone)]
pub struct AgentModelAgg {
    pub agent_type: String,
    pub model: String,
    pub runs: u32,
    pub usd: f64,
}

#[derive(Default)]
struct Group {
    costs: Vec<f64>,
    cost: CostSum,
    fresh: u64,
    startup_usd: f64,
}

type GroupKey = (ProviderId, String, Option<String>, Option<String>);

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct Part {
    usd: f64,
    cost: CostSum,
    fresh: u64,
    model: Option<String>,
    startup_usd: Option<f64>,
}

pub(crate) fn part(f: &FileCtx) -> Part {
    let mut p = Part {
        usd: f.costs.iter().flatten().sum(),
        fresh: f.turns.iter().map(|t| fresh(&t.usage)).sum(),
        ..Default::default()
    };
    for c in &f.costs {
        p.cost.add(*c);
    }
    if !f.is_main() {
        if let Some(first) = f.turns.first() {
            let mut counts: BTreeMap<&str, u32> = BTreeMap::new();
            for t in &f.turns {
                *counts.entry(f.model_of(t)).or_default() += 1;
            }
            p.model = counts
                .iter()
                .max_by_key(|(_, n)| **n)
                .map(|(m, _)| m.to_string());
            p.startup_usd = f
                .first_rate()
                .map(|r| super::ctx(&first.usage) as f64 * r.cache_read / 1e6);
        }
    }
    p
}

pub(crate) fn fold(
    files: &[&FileSummary],
) -> (Vec<SubagentRow>, Vec<ProjectCost>, Vec<AgentModelAgg>) {
    let mut projects: BTreeMap<(ProviderId, String), (ProjectCost, Vec<f64>)> = BTreeMap::new();
    let mut groups: BTreeMap<GroupKey, Group> = BTreeMap::new();
    let mut models: BTreeMap<(String, String), (u32, f64)> = BTreeMap::new();
    for f in files {
        let sp = &f.sub;
        let (usd, fr) = (sp.usd, sp.fresh);
        let (p, a_list) = projects
            .entry((f.provider.clone(), f.project.clone()))
            .or_default();
        p.project = f.project.clone();
        if f.main {
            p.main_usd += usd;
            p.main_fresh += fr;
            continue;
        }
        let Some(a) = f.first_ctx else { continue };
        p.sub_usd += usd;
        p.sub_fresh += fr;
        p.agents += 1;
        a_list.push(a as f64);
        p.agent_startup_total += a;
        let model = sp.model.clone();
        let g = groups
            .entry((
                f.provider.clone(),
                f.project.clone(),
                f.agent_type.clone(),
                model.clone(),
            ))
            .or_default();
        g.costs.push(usd);
        if sp.cost.known {
            g.cost.add(Some(sp.cost.usd));
        }
        g.fresh += fr;
        if let Some(s) = sp.startup_usd {
            g.startup_usd += s;
        }
        let m = models
            .entry((
                f.agent_type.clone().unwrap_or_default(),
                model.unwrap_or_default(),
            ))
            .or_default();
        m.0 += 1;
        m.1 += usd;
    }
    let rows = groups
        .into_iter()
        .map(|((provider, project, agent_type, model), g)| {
            let p = &projects[&(provider.clone(), project.clone())].0;
            let total = p.main_usd + p.sub_usd;
            let total_fresh = (p.main_fresh + p.sub_fresh) as f64;
            SubagentRow {
                provider,
                project,
                agent_type,
                model,
                agents: g.costs.len() as u32,
                cost_usd: g.cost.amount(),
                median_cost_usd: if g.cost.known {
                    amount(median(&g.costs), false)
                } else {
                    Amount {
                        value: None,
                        est: false,
                    }
                },
                cost_share: if total > 0.0 { g.cost.usd / total } else { 0.0 },
                fresh_share: if total_fresh > 0.0 {
                    g.fresh as f64 / total_fresh
                } else {
                    0.0
                },
                startup_share: if g.cost.usd > 0.0 {
                    g.startup_usd / g.cost.usd
                } else {
                    0.0
                },
            }
        })
        .collect();
    let projects = projects
        .into_values()
        .map(|(mut p, a)| {
            p.agent_startup_median = median(&a);
            p
        })
        .collect();
    let models = models
        .into_iter()
        .map(|((agent_type, model), (runs, usd))| AgentModelAgg {
            agent_type,
            model,
            runs,
            usd,
        })
        .collect();
    (rows, projects, models)
}
