// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use super::summary::FileSummary;
use super::util::median;
use super::{amount, CostSum, FileCtx};
use crate::model::{Category, SkillRow, StartupKind};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
struct Acc {
    listing: Vec<f64>,
    listing_est: bool,
    sessions_listed: BTreeSet<String>,
    bodies: Vec<f64>,
    body_est: bool,
    activations: u32,
    carried: CostSum,
    carried_est: bool,
    sessions_loaded: BTreeSet<String>,
    attributed_turns: u32,
    attributed: CostSum,
}

#[derive(Debug, Clone)]
pub struct PluginAgg {
    pub project: String,
    pub plugin: String,
    pub sessions_listed: u32,
    pub tokens_per_session: f64,
    pub est: bool,
    pub activations: u32,
    pub tool_calls: u32,
}

pub(crate) fn plugin_of(skill: &str) -> Option<&str> {
    skill.split_once(':').map(|(p, _)| p)
}

pub type ListingChars = Vec<(String, u64, u32)>;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct Part {
    listing: Vec<(String, u64)>,
    loads: Vec<(String, f64, bool, Option<f64>)>,
    attributed: Vec<(String, Option<f64>)>,
    calls: Vec<(Vec<String>, u32)>,
}

pub(crate) fn part(f: &FileCtx) -> Part {
    let mut p = Part::default();
    if f.is_main() {
        for sp in
            f.pf.records
                .startup
                .iter()
                .flat_map(|sp| &sp.parts)
                .filter(|p| p.kind == StartupKind::SkillListing)
        {
            p.listing.push((f.s(sp.name).to_string(), sp.chars));
        }
    }
    for l in &f.pf.records.skill_loads {
        let (b, est) = f.tok(l.body_chars);
        let k = f.turns_after_in_segment(l.order) as f64;
        let carried = f
            .rate_near(l.order)
            .map(|r| (b * f.write_rate(r) + b * r.cache_read * k) / 1e6);
        p.loads.push((f.s(l.skill).to_string(), b, est, carried));
    }
    for (i, t) in f.turns.iter().enumerate() {
        if let Some(s) = t.attribution_skill {
            p.attributed.push((f.s(s).to_string(), f.costs[i]));
        }
    }
    for c in &f.pf.records.tool_calls {
        if matches!(
            c.category,
            Category::Mcp | Category::Agent | Category::Other
        ) {
            let names: Vec<String> = [Some(c.tool), c.detail]
                .iter()
                .flatten()
                .map(|n| f.s(*n).to_string())
                .collect();
            match p.calls.iter_mut().find(|(n, _)| *n == names) {
                Some(e) => e.1 += 1,
                None => p.calls.push((names, 1)),
            }
        }
    }
    p
}

type PluginAcc = (Vec<f64>, bool, u32, u32);

pub(crate) fn fold(files: &[&FileSummary]) -> (Vec<SkillRow>, Vec<PluginAgg>, ListingChars) {
    let mut listing_chars: BTreeMap<String, (u64, u32)> = BTreeMap::new();
    let mut by: BTreeMap<String, Acc> = BTreeMap::new();
    let mut plugins: BTreeMap<(String, String), PluginAcc> = BTreeMap::new();
    let mut calls: Vec<(&str, &[String], u32)> = vec![];
    for f in files {
        let session = &f.session;
        let sk = &f.skills;
        if f.main {
            for (name, chars) in &sk.listing {
                let e = listing_chars.entry(name.clone()).or_default();
                e.0 = e.0.max(*chars);
                e.1 += 1;
            }
            if let Some(p) = &f.parts {
                let mut per_plugin: BTreeMap<String, f64> = BTreeMap::new();
                for (_, name, tok, est) in
                    p.parts.iter().filter(|p| p.0 == StartupKind::SkillListing)
                {
                    let a = by.entry(name.clone()).or_default();
                    a.listing.push(*tok);
                    a.listing_est |= *est;
                    a.sessions_listed.insert(session.clone());
                    if let Some(pl) = plugin_of(name) {
                        *per_plugin.entry(pl.to_string()).or_default() += tok;
                        plugins
                            .entry((f.project.clone(), pl.to_string()))
                            .or_default()
                            .1 |= *est;
                    }
                }
                for (pl, tok) in per_plugin {
                    plugins
                        .entry((f.project.clone(), pl))
                        .or_default()
                        .0
                        .push(tok);
                }
            }
        }
        for (name, b, est, carried) in &sk.loads {
            let a = by.entry(name.clone()).or_default();
            a.activations += 1;
            a.bodies.push(*b);
            a.body_est |= est;
            a.carried_est |= est;
            a.sessions_loaded.insert(session.clone());
            a.carried.add(*carried);
            if let Some(pl) = plugin_of(name) {
                plugins
                    .entry((f.project.clone(), pl.to_string()))
                    .or_default()
                    .2 += 1;
            }
        }
        for (name, cost) in &sk.attributed {
            let a = by.entry(name.clone()).or_default();
            a.attributed_turns += 1;
            a.attributed.add(*cost);
        }
        for (names, n) in &sk.calls {
            calls.push((&f.project, names, *n));
        }
    }
    for ((project, pl), v) in plugins.iter_mut() {
        v.3 = calls
            .iter()
            .filter(|(p, n, _)| *p == project && n.iter().any(|n| n.contains(pl.as_str())))
            .map(|c| c.2)
            .sum();
    }
    let rows = by
        .into_iter()
        .map(|(skill, a)| {
            let mut carried = a.carried.amount();
            carried.est = a.carried_est;
            let mut attributed = a.attributed.amount();
            attributed.est = false;
            SkillRow {
                provider: None,
                skill,
                listing_tokens_median: amount(median(&a.listing).round(), a.listing_est),
                activations: a.activations,
                body_tokens_median: amount(median(&a.bodies).round(), a.body_est),
                carried_cost_usd: carried,
                sessions_loaded: a.sessions_loaded.len() as u32,
                sessions_listed: a.sessions_listed.len() as u32,
                attributed_turns: a.attributed_turns,
                attributed_cost_usd: attributed,
            }
        })
        .collect();
    let plugins = plugins
        .into_iter()
        .map(|((project, plugin), (v, est, act, calls))| PluginAgg {
            project,
            plugin,
            sessions_listed: v.len() as u32,
            tokens_per_session: median(&v),
            est,
            activations: act,
            tool_calls: calls,
        })
        .collect();
    let listing_chars = listing_chars
        .into_iter()
        .map(|(k, (c, n))| (k, c, n))
        .collect();
    (rows, plugins, listing_chars)
}
