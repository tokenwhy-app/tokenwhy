// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use crate::Wrapped;
use serde::Serialize;

pub struct Archetype {
    pub id: &'static str,
    pub name: &'static str,
    pub line: &'static str,
    pub tip: Option<&'static str>,
    score: fn(&Wrapped) -> Option<f64>,
}

/// Thresholds measured on a real year of history, then frozen.
pub const NIGHT: f64 = 0.35;
pub const EARLY: f64 = 0.30;
pub const DELEGATOR: f64 = 0.50;
pub const CACHE: f64 = 0.93;
pub const MARATHON: f64 = 0.8;
pub const SPRINT_SESSIONS: f64 = 8.0;
pub const SPRINT_MAX_COMPACTIONS: f64 = 0.1;
pub const WEEKEND: f64 = 0.40;
pub const POLYGLOT_LANGS: usize = 5;
pub const POLYGLOT_SHARE: f64 = 0.05;
pub const BIG_MODEL: f64 = 0.80;
pub const IMPROVER: f64 = 0.30;

pub const STEADY: Archetype = Archetype {
    id: "steady-hand",
    name: "Steady Hand",
    line: "No extremes. Regular hours, regular sessions.",
    tip: None,
    score: |_| None,
};

pub const ALL: &[Archetype] = &[
    Archetype {
        id: "night-shift",
        name: "Night Shift",
        line: "You and Claude do your best work when the house is quiet.",
        tip: Some("After a break, start fresh."),
        score: |w| (w.peak_hour.is_some()).then(|| w.night_share / NIGHT),
    },
    Archetype {
        id: "early-bird",
        name: "Early Bird",
        line: "First commit before breakfast.",
        tip: Some("One morning session keeps the cache warm."),
        score: |w| (w.peak_hour.is_some()).then(|| w.early_share / EARLY),
    },
    Archetype {
        id: "delegator",
        name: "Delegator",
        line: "You manage a team of subagents. They do most of the reading.",
        tip: Some("Keep subagent briefs narrow."),
        score: |w| (w.cost_usd > 0.0).then(|| w.agent_share / DELEGATOR),
    },
    Archetype {
        id: "cache-whisperer",
        name: "Cache Whisperer",
        line: "Almost everything you send was already there.",
        tip: Some("Keep CLAUDE.md stable."),
        score: |w| (w.tokens_total > 0).then(|| w.cache_share / CACHE),
    },
    Archetype {
        id: "marathoner",
        name: "Marathoner",
        line: "Your sessions go the distance, compactions and all.",
        tip: Some("New task, new session."),
        score: |w| (w.sessions > 0).then(|| w.compactions_per_session / MARATHON),
    },
    Archetype {
        id: "sprinter",
        name: "Sprinter",
        line: "Short sessions, many of them, in and out.",
        tip: Some("Trim the startup load."),
        score: |w| {
            (w.sessions > 0 && w.compactions_per_session < SPRINT_MAX_COMPACTIONS)
                .then(|| w.sessions_per_day / SPRINT_SESSIONS)
        },
    },
    Archetype {
        id: "weekend-builder",
        name: "Weekend Builder",
        line: "Weekdays are for work. Weekends are for the real projects.",
        tip: None,
        score: |w| (w.active_days > 0).then(|| w.weekend_share / WEEKEND),
    },
    Archetype {
        id: "polyglot",
        name: "Polyglot",
        line: "{n} languages this year. Your file tree has range.",
        tip: None,
        score: |w| {
            let n = w.langs.iter().filter(|l| l.share >= POLYGLOT_SHARE).count();
            (w.lang_edits > 0).then(|| n as f64 / POLYGLOT_LANGS as f64)
        },
    },
    Archetype {
        id: "big-model-energy",
        name: "Big Model Energy",
        line: "Only the largest model will do.",
        tip: Some("Small models read for less."),
        score: |w| (w.cost_usd > 0.0).then(|| w.top_tier_share / BIG_MODEL),
    },
    Archetype {
        id: "improver",
        name: "Improver",
        line: "You got cheaper as the year went on. Same work, smaller bill.",
        tip: None,
        score: |w| w.cost_per_edit_drop.map(|d| d / IMPROVER),
    },
];

pub fn by_id(id: &str) -> &'static Archetype {
    ALL.iter().find(|a| a.id == id).unwrap_or(&STEADY)
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Result {
    pub primary: String,
    pub traits: Vec<String>,
    pub scores: Vec<(String, f64)>,
}

pub fn classify(w: &Wrapped) -> Result {
    let scores: Vec<(String, f64)> = ALL
        .iter()
        .filter_map(|a| (a.score)(w).map(|s| (a.id.to_string(), s)))
        .collect();
    let mut hits: Vec<&(String, f64)> = scores.iter().filter(|(_, s)| *s >= 1.0).collect();
    hits.sort_by(|a, b| b.1.total_cmp(&a.1));
    Result {
        primary: hits.first().map_or(STEADY.id, |h| h.0.as_str()).to_string(),
        traits: hits.iter().skip(1).take(2).map(|h| h.0.clone()).collect(),
        scores,
    }
}
