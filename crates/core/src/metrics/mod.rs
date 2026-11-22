// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

mod compaction;
pub mod days;
mod live;
mod memory;
mod segments;
mod skills;
pub mod spend;
mod startup;
mod subagents;
pub mod summary;
mod tools;
pub(crate) mod util;

use crate::model::{
    Amount, CompactionRow, FileRole, Live, MemoryRow, ModelRate, ParsedFile, PriceTable,
    ProviderId, Query, Settings, SkillRow, StartupRow, SubagentRow, TimestampMs, Today, ToolRow,
    Turn, Usage, Window,
};
use std::collections::{BTreeSet, HashMap};

pub use compaction::CompactionAgg;
pub use memory::MemoryAgg;
pub use segments::{SegmentAgg, SessionAgg};
pub use skills::PluginAgg;
pub use startup::ProjectStartup;
pub use subagents::{AgentModelAgg, ProjectCost};
pub use summary::{summarize, FileSummary, SessionSummary};
pub use tools::{McpAgg, Reread, ToolTotals};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum NameKind {
    Project,
    Session,
    Path,
    Skill,
    AgentType,
    McpServer,
    Command,
}

#[derive(Debug)]
pub struct Aggregates {
    pub window: Window,
    pub now_ms: TimestampMs,
    pub weeks: f64,
    pub startup: Vec<StartupRow>,
    pub skills: Vec<SkillRow>,
    pub memory: Vec<MemoryRow>,
    pub subagents: Vec<SubagentRow>,
    pub compactions: Vec<CompactionRow>,
    pub tools: Vec<ToolRow>,
    pub live: Live,
    pub unpriced_models: Vec<String>,
    pub project_startup: Vec<ProjectStartup>,
    pub memory_files: Vec<MemoryAgg>,
    pub plugins: Vec<PluginAgg>,
    pub skill_listing_chars: Vec<(String, u64, u32)>,
    pub project_costs: Vec<ProjectCost>,
    pub agent_models: Vec<AgentModelAgg>,
    pub compaction_aggs: Vec<CompactionAgg>,
    pub sessions: Vec<SessionAgg>,
    pub segments: Vec<SegmentAgg>,
    pub tool_totals: ToolTotals,
    pub mcp: Vec<McpAgg>,
    pub rereads: Vec<Reread>,
    pub names: Vec<(NameKind, String)>,
    pub reader_agent_types: Vec<String>,
    pub daily: Vec<crate::model::DailyRow>,
    pub week: Option<crate::model::WeekDeltas>,
    pub session_rows: Vec<crate::model::SessionRow>,
    pub hourly: Vec<crate::model::HourRow>,
    pub providers: Vec<crate::model::ProviderInfo>,
    pub accounts: Vec<crate::model::AccountInfo>,
    pub spend_all: spend::KindSpend,
    pub spend_by_project: HashMap<String, spend::KindSpend>,
}

impl Default for Aggregates {
    fn default() -> Self {
        Self {
            window: Window {
                from_ms: 0,
                to_ms: 0,
            },
            now_ms: 0,
            weeks: 1.0,
            startup: vec![],
            skills: vec![],
            memory: vec![],
            subagents: vec![],
            compactions: vec![],
            tools: vec![],
            live: Live {
                today: Today {
                    fresh_tokens: 0,
                    cache_read_tokens: 0,
                    cost_usd: Amount {
                        value: Some(0.0),
                        est: false,
                    },
                    sessions: 0,
                    agents: 0,
                },
                sessions: vec![],
            },
            unpriced_models: vec![],
            project_startup: vec![],
            memory_files: vec![],
            plugins: vec![],
            skill_listing_chars: vec![],
            project_costs: vec![],
            agent_models: vec![],
            compaction_aggs: vec![],
            sessions: vec![],
            segments: vec![],
            tool_totals: ToolTotals::default(),
            mcp: vec![],
            rereads: vec![],
            names: vec![],
            reader_agent_types: vec![],
            daily: vec![],
            week: None,
            session_rows: vec![],
            hourly: vec![],
            providers: vec![],
            accounts: vec![],
            spend_all: Default::default(),
            spend_by_project: HashMap::new(),
        }
    }
}

pub(crate) struct FileCtx<'a> {
    pub pf: &'a ParsedFile,
    pub project: String,
    pub turns: Vec<&'a Turn>,
    pub costs: Vec<Option<f64>>,
    pub ratio: Option<f64>,
    rates: Vec<Option<&'a ModelRate>>,
    seg_orders: HashMap<u32, Vec<u32>>,
}

impl<'a> FileCtx<'a> {
    pub fn s(&self, i: u32) -> &'a str {
        self.pf
            .records
            .strings
            .get(i as usize)
            .map(String::as_str)
            .unwrap_or("")
    }
    pub fn provider(&self) -> &'a ProviderId {
        &self.pf.source.provider
    }
    pub fn session(&self) -> &'a str {
        &self.pf.source.session_id
    }
    pub fn is_main(&self) -> bool {
        self.pf.source.role == FileRole::Main
    }
    pub fn first_ts(&self) -> Option<TimestampMs> {
        self.turns.first().map(|t| t.first_ts_ms)
    }
    pub fn last_ts(&self) -> Option<TimestampMs> {
        self.turns.last().map(|t| t.first_ts_ms)
    }
    pub fn segment_at(&self, order: u32) -> u32 {
        self.pf.records.segment_at(order)
    }
    pub fn turns_after_in_segment(&self, order: u32) -> u32 {
        let Some(v) = self.seg_orders.get(&self.segment_at(order)) else {
            return 0;
        };
        (v.len() - v.partition_point(|o| *o <= order)) as u32
    }
    pub fn rate_near(&self, order: u32) -> Option<&'a ModelRate> {
        let i = self
            .turns
            .iter()
            .position(|t| t.order > order)
            .or_else(|| self.turns.len().checked_sub(1))?;
        self.rates[i]
    }
    pub fn first_rate(&self) -> Option<&'a ModelRate> {
        self.rates.first().copied().flatten()
    }
    pub fn write_rate(&self, rate: &ModelRate) -> f64 {
        let (w5, w1) = self.turns.iter().fold((0u64, 0u64), |a, t| {
            (a.0 + t.usage.cache_write_5m, a.1 + t.usage.cache_write_1h)
        });
        if w1 > w5 {
            rate.cache_write_1h
        } else {
            rate.cache_write_5m
        }
    }
    pub fn tok(&self, chars: u64) -> (f64, bool) {
        match self.ratio {
            Some(r) => (chars as f64 * r, false),
            None => (chars as f64 / 4.0, true),
        }
    }
    pub fn model_of(&self, t: &Turn) -> &'a str {
        self.s(t.model)
    }
}

pub fn ctx(u: &Usage) -> u64 {
    u.input + u.cache_write_5m + u.cache_write_1h + u.cache_read
}

pub fn fresh(u: &Usage) -> u64 {
    u.input + u.cache_write_5m + u.cache_write_1h + u.output
}

#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct CostSum {
    pub usd: f64,
    pub known: bool,
}

impl CostSum {
    pub fn add(&mut self, c: Option<f64>) {
        if let Some(v) = c {
            self.usd += v;
            self.known = true;
        }
    }
    pub fn amount(self) -> Amount {
        Amount {
            value: self.known.then_some(self.usd),
            est: false,
        }
    }
}

pub(crate) fn amount(v: f64, est: bool) -> Amount {
    Amount {
        value: Some(v),
        est,
    }
}

fn rate_for<'a>(
    prices: &'a [PriceTable],
    provider: &ProviderId,
    model: &str,
) -> Option<&'a ModelRate> {
    prices
        .iter()
        .filter(|t| &t.provider == provider)
        .find_map(|t| crate::price::rate_for(t, model))
}

pub(crate) fn project_name(pf: &ParsedFile) -> String {
    pf.records
        .project_name
        .and_then(|i| pf.records.strings.get(i as usize).cloned())
        .unwrap_or_else(|| pf.source.project_key.clone())
}

pub(crate) fn prepare<'a>(
    files: &[&'a ParsedFile],
    prices: &'a [PriceTable],
    unpriced: &mut BTreeSet<String>,
) -> Vec<FileCtx<'a>> {
    let mut out: Vec<FileCtx<'a>> = files
        .iter()
        .map(|&pf| {
            let mut turns: Vec<&Turn> = pf.records.turns.iter().collect();
            turns.sort_by_key(|t| t.order);
            let mut seg_orders: HashMap<u32, Vec<u32>> = HashMap::new();
            for t in &turns {
                seg_orders.entry(t.segment).or_default().push(t.order);
            }
            let mut fc = FileCtx {
                pf,
                project: project_name(pf),
                turns,
                costs: vec![],
                ratio: None,
                rates: vec![],
                seg_orders,
            };
            for t in &fc.turns {
                let model = fc.model_of(t);
                let rate = rate_for(prices, &pf.source.provider, model);
                if rate.is_none() && t.stored_cost.is_none() && !model.is_empty() {
                    unpriced.insert(model.to_string());
                }
                fc.rates.push(rate);
                fc.costs.push(
                    t.stored_cost
                        .or_else(|| rate.map(|r| crate::price::cost(r, &t.usage))),
                );
            }
            fc.ratio = startup::file_ratio(&fc);
            fc
        })
        .collect();
    let session_ratio: Vec<((ProviderId, String), f64)> = out
        .iter()
        .filter(|f| f.is_main())
        .filter_map(|f| {
            f.ratio
                .map(|r| ((f.provider().clone(), f.session().to_string()), r))
        })
        .collect();
    for f in out.iter_mut().filter(|f| !f.is_main() && f.ratio.is_none()) {
        let key = (f.provider().clone(), f.session().to_string());
        f.ratio = session_ratio
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, r)| *r);
    }
    let session_project: HashMap<(ProviderId, String), String> = out
        .iter()
        .filter(|f| f.is_main())
        .map(|f| {
            (
                (f.provider().clone(), f.session().to_string()),
                f.project.clone(),
            )
        })
        .collect();
    for f in out.iter_mut().filter(|f| !f.is_main()) {
        if let Some(p) = session_project.get(&(f.provider().clone(), f.session().to_string())) {
            f.project = p.clone();
        }
    }
    out
}

pub fn aggregate(
    files: &[ParsedFile],
    prices: &[PriceTable],
    query: &Query,
    settings: &Settings,
    now_ms: TimestampMs,
) -> Aggregates {
    let refs: Vec<&ParsedFile> = files.iter().collect();
    let sessions = summarize(&refs, prices, now_ms);
    let mut order: Vec<&FileSummary> = sessions.iter().flat_map(|s| &s.files).collect();
    let pos: HashMap<&std::path::Path, usize> = files
        .iter()
        .enumerate()
        .map(|(i, f)| (f.source.path.as_path(), i))
        .collect();
    order.sort_by_key(|f| pos.get(f.path.as_path()).copied().unwrap_or(usize::MAX));
    fold(&order, query, settings, now_ms)
}

pub fn fold(
    all: &[&FileSummary],
    query: &Query,
    settings: &Settings,
    now_ms: TimestampMs,
) -> Aggregates {
    let project_ok = |f: &FileSummary| {
        query.project.as_deref().is_none_or(|p| f.project == p)
            && query.provider.as_ref().is_none_or(|p| &f.provider == p)
    };
    let sessions_in: BTreeSet<(&ProviderId, &str)> = all
        .iter()
        .filter(|f| f.main && project_ok(f))
        .filter(|f| {
            f.first_ts
                .is_some_and(|ts| ts >= query.window.from_ms && ts < query.window.to_ms)
        })
        .map(|f| (&f.provider, f.session.as_str()))
        .collect();
    let in_window: Vec<&FileSummary> = all
        .iter()
        .copied()
        .filter(|f| project_ok(f) && sessions_in.contains(&(&f.provider, f.session.as_str())))
        .collect();

    // All time (from 0) starts at the first turn in the data: dividing by the days since 1970 turned weekly
    // figures into cents.
    let mut window = query.window.clone();
    if window.from_ms <= 0 {
        if let Some(first) = in_window.iter().filter_map(|f| f.first_ts).min() {
            window.from_ms = first;
        }
    }
    let mut agg = Aggregates {
        window,
        now_ms,
        reader_agent_types: settings.reader_agent_types.clone(),
        ..Default::default()
    };

    let (startup_rows, project_startup) = startup::fold(&in_window);
    agg.startup = startup_rows;
    agg.project_startup = project_startup;
    let (skills, plugins, listing_chars) = skills::fold(&in_window);
    agg.skill_listing_chars = listing_chars;
    agg.skills = skills;
    agg.plugins = plugins;
    let (memory, memory_files) = memory::fold(&in_window);
    agg.memory = memory;
    agg.memory_files = memory_files;
    let (subagents, project_costs, agent_models) = subagents::fold(&in_window);
    agg.subagents = subagents;
    agg.project_costs = project_costs;
    agg.agent_models = agent_models;
    let (rows, caggs) = compaction::fold(&in_window, &agg.project_startup);
    agg.compactions = rows;
    agg.compaction_aggs = caggs;
    let (sessions, segs) = segments::fold(&in_window, &agg.compaction_aggs);
    agg.sessions = sessions;
    agg.segments = segs;
    let t = tools::fold(&in_window, query, settings);
    agg.tools = t.rows;
    agg.tool_totals = t.totals;
    agg.mcp = t.mcp;
    agg.rereads = t.rereads;
    let sp = spend::fold(&in_window, settings);
    agg.session_rows = sp.sessions;
    agg.hourly = sp.hourly;
    agg.weeks = active_weeks(&agg.window, agg.hourly.iter().map(|h| h.day_ms));
    agg.spend_all = sp.all;
    agg.spend_by_project = sp.by_project;
    let live_files: Vec<&FileSummary> = all.iter().copied().filter(|f| project_ok(f)).collect();
    agg.live = live::fold(&live_files, now_ms);
    agg.unpriced_models = live_files
        .iter()
        .flat_map(|f| f.unpriced.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    agg.names = collect_names(all, &agg);
    agg
}

pub fn next_live_expiry(all: &[&FileSummary], now_ms: TimestampMs) -> Option<TimestampMs> {
    live::next_expiry(all, now_ms)
}

pub fn local_midnight(now_ms: TimestampMs) -> TimestampMs {
    live::local_midnight(now_ms)
}

fn collect_names(all: &[&FileSummary], agg: &Aggregates) -> Vec<(NameKind, String)> {
    let mut set: BTreeSet<(NameKind, String)> = BTreeSet::new();
    for f in all {
        set.extend(f.names.iter().cloned());
    }
    for m in &agg.mcp {
        set.insert((NameKind::McpServer, m.server.clone()));
    }
    let cost = |p: &str| {
        agg.project_costs
            .iter()
            .filter(|c| c.project == p)
            .map(|c| c.main_usd + c.sub_usd)
            .sum::<f64>()
    };
    let (mut projects, rest): (Vec<_>, Vec<_>) = set
        .into_iter()
        .filter(|(_, n)| !n.is_empty())
        .partition(|(k, _)| *k == NameKind::Project);
    projects.sort_by(|a, b| {
        cost(&b.1)
            .total_cmp(&cost(&a.1))
            .then_with(|| a.1.cmp(&b.1))
    });
    projects.extend(rest);
    projects
}

/// Counts only the 7-day blocks that hold any use, so a holiday does not lower the average.
pub fn active_weeks(
    window: &crate::model::Window,
    use_ms: impl Iterator<Item = TimestampMs>,
) -> f64 {
    const WEEK_MS: i64 = 7 * 86_400_000;
    let span = window.to_ms - window.from_ms;
    let calendar = (span as f64 / 86_400_000.0).max(1.0) / 7.0;
    if span < WEEK_MS {
        return calendar;
    }
    let mut blocks: Vec<i64> = use_ms
        .filter(|&t| t >= window.from_ms && t < window.to_ms)
        .map(|t| (window.to_ms - 1 - t) / WEEK_MS)
        .collect();
    blocks.sort_unstable();
    blocks.dedup();
    if blocks.is_empty() {
        calendar
    } else {
        blocks.len() as f64
    }
}

#[cfg(test)]
mod active_weeks_tests {
    use super::active_weeks;
    use crate::model::Window;

    #[test]
    fn empty_week_is_left_out() {
        let day = 86_400_000i64;
        let w = Window { from_ms: 0, to_ms: 28 * day };
        let use_days = [1i64, 9, 25].map(|d| d * day + 3_600_000);
        let weeks = active_weeks(&w, use_days.into_iter());
        let total = 90.0;
        assert_eq!(total / weeks, 30.0);
    }
}
