// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use crate::metrics::spend::Kind;
use crate::metrics::util::{fmt_tok, median};
use crate::metrics::Aggregates;
use crate::model::{
    Amount, Capabilities, Category, Finding, Level, ProviderId, StartupKind, Thresholds,
};
use chrono::{Local, TimeZone};

const MAX_LINE: usize = 110;
const MAX_NAME: usize = 28;

fn short(name: &str) -> String {
    let n: String = name.chars().take(MAX_NAME).collect();
    if n.len() < name.len() {
        format!("{}..", n.trim_end())
    } else {
        n
    }
}

fn base(path: &str) -> String {
    short(
        path.rsplit(['/', '\\'])
            .find(|s| !s.is_empty())
            .unwrap_or(path),
    )
}

fn sid(session: &str) -> String {
    session.chars().take(8).collect()
}

fn day(ms: i64) -> String {
    Local
        .timestamp_millis_opt(ms)
        .single()
        .map(|t| t.format("%m-%d").to_string())
        .unwrap_or_default()
}

fn pct(v: f64) -> String {
    format!("{:.0} %", v * 100.0)
}

fn kind_label(k: StartupKind) -> &'static str {
    match k {
        StartupKind::SystemPrompt => "system prompt",
        StartupKind::BuiltinTools => "built-in tools",
        StartupKind::McpTools => "MCP tools",
        StartupKind::DeferredTools => "deferred tools",
        StartupKind::SkillListing => "skill listing",
        StartupKind::AgentListing => "agent listing",
        StartupKind::McpInstructions => "MCP instructions",
        StartupKind::UserInstructions | StartupKind::ProjectInstructions | StartupKind::Memory => {
            "instructions"
        }
        StartupKind::Base => "base prompt and tools",
    }
}

fn fit(line: String) -> String {
    if line.chars().count() <= MAX_LINE {
        return line;
    }
    line.chars().take(MAX_LINE).collect()
}

struct Out<'a> {
    v: Vec<Finding>,
    agg: &'a Aggregates,
}

impl Out<'_> {
    fn push(
        &mut self,
        rule: &str,
        project: Option<&str>,
        line: String,
        saved: Option<f64>,
        kind: Kind,
    ) {
        let tokens = saved.map(|s| s.max(0.0).round());
        let rate = project
            .and_then(|p| self.agg.spend_by_project.get(p))
            .and_then(|k| k.per_token(kind))
            .or_else(|| self.agg.spend_all.per_token(kind));
        let usd = tokens
            .zip(rate)
            .map(|(t, r)| (t * r * 100.0).round() / 100.0);
        let co2_rate = project
            .and_then(|p| self.agg.spend_by_project.get(p))
            .and_then(|k| k.co2_per_token(kind))
            .or_else(|| self.agg.spend_all.co2_per_token(kind));
        let co2 = tokens.zip(co2_rate).map(|(t, r)| t * r);
        self.v.push(Finding {
            provider: None,
            rule: rule.into(),
            project: project.map(str::to_string),
            line: fit(line),
            tokens_saved_per_week: Amount {
                value: tokens,
                est: saved.is_some(),
            },
            usd_saved_per_week: Amount {
                value: usd,
                est: usd.is_some(),
            },
            co2_g_saved_per_week: Amount {
                value: co2,
                est: co2.is_some(),
            },
            usd_saved_last_7d: Amount::none(),
        });
    }
}

pub const RECENT_MS: i64 = 7 * 86_400_000;

pub fn mark_recent(window: &mut [Finding], recent: &[Finding]) {
    let key = |f: &Finding| {
        (
            f.rule.clone(),
            f.project.clone(),
            f.provider.as_ref().map(|p| p.0.clone()),
        )
    };
    let mut sum = std::collections::HashMap::new();
    for f in recent {
        *sum.entry(key(f)).or_insert(0.0) += f.usd_saved_per_week.value.unwrap_or(0.0);
    }
    let mut base = std::collections::HashMap::new();
    for f in window.iter() {
        *base.entry(key(f)).or_insert(0.0) += f.usd_saved_per_week.value.unwrap_or(0.0);
    }
    for f in window.iter_mut() {
        let Some(u) = f.usd_saved_per_week.value else {
            continue;
        };
        let k = key(f);
        let total = base[&k];
        let r = sum.get(&k).copied().unwrap_or(0.0);
        let share = if total > 0.0 { u / total } else { 0.0 };
        f.usd_saved_last_7d = Amount {
            value: Some((r * share * 100.0).round() / 100.0),
            est: true,
        };
    }
}

type Cap = fn(&Capabilities) -> Level;

fn able(agg: &Aggregates, provider: &ProviderId, cap: Cap) -> bool {
    agg.providers
        .iter()
        .find(|i| &i.id == provider)
        .is_none_or(|i| cap(&i.caps) != Level::None)
}

fn any_able(agg: &Aggregates, cap: Cap) -> bool {
    agg.providers.is_empty() || agg.providers.iter().any(|i| cap(&i.caps) != Level::None)
}

pub fn findings(agg: &Aggregates, th: &Thresholds) -> Vec<Finding> {
    let mut o = Out { v: vec![], agg };
    let wk = agg.weeks.max(1.0 / 7.0);
    let days = ((agg.window.to_ms - agg.window.from_ms) as f64 / 86_400_000.0).round().max(1.0);
    let on = |cap: Cap| any_able(agg, cap);

    for p in agg.project_startup.iter().filter(|p| {
        able(agg, &p.provider, |c| c.startup)
            && p.sessions >= th.startup_min_sessions
            && p.median > th.startup_median_tokens as f64
    }) {
        let largest = agg
            .startup
            .iter()
            .find(|r| r.project == p.project && r.provider == p.provider)
            .and_then(|r| {
                r.parts
                    .iter()
                    .find(|x| x.kind != StartupKind::Base || r.parts.len() == 1)
            })
            .map(|x| {
                let n = if x.name.is_empty() {
                    kind_label(x.kind).to_string()
                } else {
                    base(&x.name)
                };
                format!(
                    " Largest part: {} {}.",
                    n,
                    fmt_tok(x.tokens.value.unwrap_or(0.0))
                )
            })
            .unwrap_or_default();
        let line = format!(
            "{}: startup {} tokens.{}",
            short(&p.project),
            fmt_tok(p.median),
            largest
        );
        o.push(
            "R1",
            Some(&p.project),
            line,
            Some((p.median - th.startup_median_tokens as f64) * p.sessions as f64 / wk),
            Kind::Write,
        );
        if let Some(f) = o.v.last_mut() {
            f.provider = Some(p.provider.clone());
        }
    }
    for m in agg
        .memory_files
        .iter()
        .filter(|m| on(|c| c.memory) && m.kind == StartupKind::Memory && m.bytes > th.memory_bytes)
    {
        let line = format!(
            "{} {:.1} KB ({} tok every session and agent). Keep one short line per file.",
            base(&m.path),
            m.bytes as f64 / 1000.0,
            fmt_tok(m.tokens)
        );
        let excess = m.tokens * (m.bytes - th.memory_bytes) as f64 / m.bytes as f64;
        o.push(
            "R2",
            single(&m.projects),
            line,
            Some(excess * m.files as f64 / wk),
            Kind::Write,
        );
    }
    for p in agg.plugins.iter().filter(|p| {
        on(|c| c.startup)
            && p.sessions_listed >= th.plugin_min_sessions
            && p.activations == 0
            && p.tool_calls == 0
    }) {
        let line = format!(
            "{}: {} tok per session here, never used in {} days. Disable it for this project.",
            short(&p.plugin),
            fmt_tok(p.tokens_per_session),
            days
        );
        o.push(
            "R3",
            Some(&p.project),
            line,
            Some(p.tokens_per_session * p.sessions_listed as f64 / wk),
            Kind::Write,
        );
    }
    for m in agg.mcp.iter().filter(|m| {
        on(|c| c.mcp)
            && m.tools_loaded > th.mcp_min_tools
            && m.sessions_loaded >= th.mcp_min_sessions
            && (m.tools_called as f64 / m.tools_loaded as f64) < th.mcp_called_share
    }) {
        let line = format!(
            "{}: {} of {} tools called in {} sessions. Remove the server or scope it.",
            short(&m.server),
            m.tools_called,
            m.tools_loaded,
            m.sessions_loaded
        );
        o.push(
            "R4",
            None,
            line,
            Some(m.listing_tokens_per_session * m.sessions_loaded as f64 / wk),
            Kind::Write,
        );
    }
    for s in agg
        .skills
        .iter()
        .filter(|s| on(|c| c.skills) && s.activations > 0)
    {
        let b = s.body_tokens_median.value.unwrap_or(0.0);
        if b > th.skill_body_tokens as f64 {
            let line = format!(
                "{}: {} tok per activation. Split the body; load sections on demand.",
                short(&s.skill),
                fmt_tok(b)
            );
            o.push(
                "R5",
                None,
                line,
                Some((b - th.skill_body_tokens as f64) * s.activations as f64 / wk),
                Kind::Write,
            );
        }
    }
    // A `plugin:skill` description belongs to a third-party plugin the user cannot edit.
    for (skill, chars, sessions) in agg
        .skill_listing_chars
        .iter()
        .filter(|s| on(|c| c.startup) && s.1 > th.skill_listing_chars && !s.0.contains(':'))
    {
        let line = format!(
            "{} description {} chars. Cut it to one sentence.",
            short(skill),
            chars
        );
        o.push(
            "R6",
            None,
            line,
            Some((chars - th.skill_listing_chars) as f64 / 4.0 * *sessions as f64 / wk),
            Kind::Write,
        );
    }
    for m in agg.memory_files.iter().filter(|m| {
        on(|c| c.startup)
            && m.kind != StartupKind::UserInstructions
            && m.duplicate_lines >= th.instruction_duplicates
    }) {
        let line = format!(
            "{} repeats {} user-level lines ({} tok). Remove the copies.",
            base(&m.path),
            m.duplicate_lines,
            fmt_tok(m.duplicate_tokens)
        );
        o.push(
            "R7",
            single(&m.projects),
            line,
            Some(m.duplicate_tokens * m.files as f64 / wk),
            Kind::Write,
        );
    }
    for p in agg
        .project_costs
        .iter()
        .filter(|p| on(|c| c.subagents) && p.cost_share() > th.subagent_cost_share)
    {
        let share = p.cost_share();
        let line = format!(
            "{}: subagents {} of cost; {} agents, median startup {}. Give them file paths, not file contents.",
            short(&p.project),
            pct(share),
            p.agents,
            fmt_tok(p.agent_startup_median)
        );
        let saved = p.agent_startup_total as f64 * (share - th.subagent_cost_share) / share;
        o.push("R8", Some(&p.project), line, Some(saved / wk), Kind::Write);
    }
    for t in agg
        .reader_agent_types
        .iter()
        .filter(|_| on(|c| c.subagents))
    {
        let runs: Vec<_> = agg
            .agent_models
            .iter()
            .filter(|m| &m.agent_type == t)
            .collect();
        let total: u32 = runs.iter().map(|m| m.runs).sum();
        let opus: Vec<_> = runs.iter().filter(|m| m.model.contains("opus")).collect();
        let n: u32 = opus.iter().map(|m| m.runs).sum();
        if total > 0 && n as f64 / total as f64 > th.reader_agent_opus_share {
            let usd: f64 = opus.iter().map(|m| m.usd).sum();
            let line = format!(
                "{} agents ran on opus {} times (${:.0}). Set their model to haiku.",
                short(t),
                n,
                usd
            );
            o.push("R9", None, line, None, Kind::Write);
        }
    }
    for s in agg.sessions.iter().filter(|s| {
        on(|c| c.compactions)
            && (s.compactions >= th.compactions_per_session
                || s.max_carry_ratio.is_some_and(|r| r > th.carry_ratio))
    }) {
        let line = format!(
            "{} {}: {} compaction{}, ctx {} peak. End topics with a handoff and start fresh.",
            short(&s.project),
            day(s.start_ms),
            s.compactions,
            if s.compactions == 1 { "" } else { "s" },
            fmt_tok(s.max_ctx as f64)
        );
        let rebuild: f64 = agg
            .compaction_aggs
            .iter()
            .filter(|c| c.session == s.session && c.project == s.project)
            .map(|c| c.rebuild_tokens)
            .sum();
        o.push(
            "R10",
            Some(&s.project),
            line,
            Some(rebuild / wk),
            Kind::Write,
        );
    }
    if on(|c| c.tool_outputs) {
        tool_rules(agg, th, wk, &mut o);
    }
    for r in agg
        .rereads
        .iter()
        .filter(|r| on(|c| c.tool_outputs) && r.count >= th.reread_count)
    {
        let line = format!(
            "{} read {} times in session {} ({} tok). Keep the part you need in the brief.",
            base(&r.path),
            r.count,
            sid(&r.session),
            fmt_tok(r.tokens)
        );
        o.push(
            "R16",
            Some(&r.project),
            line,
            Some(r.tokens * (r.count - 1) as f64 / r.count as f64 / wk),
            Kind::Write,
        );
    }
    for s in agg.segments.iter().filter(|s| {
        on(|c| c.turns) && s.mean_ctx > th.segment_mean_ctx as f64 && s.turns > th.segment_turns
    }) {
        let line = format!(
            "{} {}: {} turns at {} mean ctx. Split the topic.",
            short(&s.project),
            day(s.start_ms),
            s.turns,
            fmt_tok(s.mean_ctx)
        );
        o.push(
            "R17",
            Some(&s.project),
            line,
            Some((s.mean_ctx - th.segment_mean_ctx as f64) * s.turns as f64 / wk),
            Kind::Read,
        );
    }
    for s in agg.sessions.iter().filter(|s| {
        on(|c| c.cache_read) && s.turns > th.cache_hit_min_turns && s.cache_hit < th.cache_hit
    }) {
        let line = format!(
            "Session {}: cache hit {}. Check for edits to instruction files mid-session.",
            sid(&s.session),
            pct(s.cache_hit)
        );
        o.push("R18", Some(&s.project), line, None, Kind::Read);
    }

    if let [only] = agg.providers.as_slice() {
        for f in &mut o.v {
            f.provider.get_or_insert_with(|| only.id.clone());
        }
    }
    let min = th.min_saved_per_week as f64;
    o.v.retain(|f| f.tokens_saved_per_week.value.is_none_or(|v| v >= min));
    o.v.sort_by(|a, b| {
        let (x, y) = (a.tokens_saved_per_week.value, b.tokens_saved_per_week.value);
        y.unwrap_or(-1.0)
            .total_cmp(&x.unwrap_or(-1.0))
            .then_with(|| a.rule.cmp(&b.rule))
    });
    o.v
}

fn single(projects: &[String]) -> Option<&str> {
    match projects {
        [p] => Some(p.as_str()),
        _ => None,
    }
}

fn tool_rules(agg: &Aggregates, th: &Thresholds, wk: f64, o: &mut Out<'_>) {
    let t = &agg.tool_totals;
    if t.carried <= 0.0 {
        return;
    }
    let share = |cats: &[Category]| {
        cats.iter()
            .map(|c| t.by_category.get(c).copied().unwrap_or(0.0))
            .sum::<f64>()
            / t.carried
    };
    let top = |cats: &[Category]| {
        t.by_detail
            .iter()
            .filter(|(c, d, _)| cats.contains(c) && !d.is_empty())
            .max_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(_, d, v)| (d.clone(), v / t.carried))
    };
    let excess = |s: f64, limit: f64| t.carried * (s - limit) / wk;

    let s = share(&[Category::ReadFile]);
    if s > th.read_file_share {
        let top = top(&[Category::ReadFile])
            .map(|(d, v)| format!(" {} {}.", short(&d), pct(v)))
            .unwrap_or_default();
        let line = format!(
            "File reads: {} of tool output.{} Read by symbol or by line range.",
            pct(s),
            top
        );
        o.push(
            "R11",
            None,
            line,
            Some(excess(s, th.read_file_share)),
            Kind::Read,
        );
    }
    let s = share(&[Category::Search]);
    let med = median(&t.search_chars.iter().map(|v| *v as f64).collect::<Vec<_>>());
    if s > th.search_share && med > th.search_median_chars as f64 {
        let name = top(&[Category::Search])
            .map(|(d, _)| short(&d))
            .unwrap_or_else(|| "Search".into());
        let line = format!(
            "{}: {} of tool output, median {} chars per call. Scope the path and cap the matches.",
            name,
            pct(s),
            fmt_tok(med)
        );
        o.push(
            "R12",
            None,
            line,
            Some(excess(s, th.search_share)),
            Kind::Read,
        );
    }
    if t.big_results >= th.big_result_count {
        let top = t
            .biggest
            .as_ref()
            .map(|(c, d, _)| {
                format!(" (top: {} {})", crate::render::category_label(*c), short(d))
                    .replace(" )", ")")
            })
            .unwrap_or_default();
        let line = format!(
            "{} tool results over {} chars{}. Cap or tail the output.",
            t.big_results,
            fmt_tok(th.big_result_chars as f64),
            top
        );
        o.push(
            "R13",
            None,
            line,
            Some(t.big_results as f64 * th.big_result_chars as f64 / 4.0 / wk),
            Kind::Write,
        );
    }
    let s = share(&[Category::List]);
    if s > th.list_share {
        let name = top(&[Category::List])
            .map(|(d, _)| short(&d))
            .unwrap_or_else(|| "Listings".into());
        let line = format!("{}: {} of tool output. Use a scoped glob.", name, pct(s));
        o.push(
            "R14",
            None,
            line,
            Some(excess(s, th.list_share)),
            Kind::Read,
        );
    }
    let s = share(&[Category::Build, Category::Test]);
    if s > th.build_test_share {
        let name = top(&[Category::Build, Category::Test])
            .map(|(d, _)| short(&d))
            .unwrap_or_else(|| "Build and test".into());
        let line = format!(
            "{} output {}. Use quiet flags (--console=plain -q).",
            name,
            pct(s)
        );
        o.push(
            "R15",
            None,
            line,
            Some(excess(s, th.build_test_share)),
            Kind::Read,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::spend::KindSpend;
    use crate::metrics::SegmentAgg;
    use crate::model::Usage;

    /// Published opus-4-5 cache-read price, $0.50 per million; expected saving worked out by hand.
    #[test]
    fn cache_read_saving_priced_at_cache_read_rate() {
        let table = crate::price::parse_table(include_str!("prices/anthropic.json")).unwrap();
        let rate = crate::price::rate_for(&table, "claude-opus-4-5-20251101").unwrap();
        let u = Usage {
            input: 20_000,
            cache_write_5m: 100_000,
            cache_write_1h: 0,
            cache_read: 1_000_000,
            output: 30_000,
            web_search_requests: 0,
        };
        let spend = KindSpend {
            tok: [u.input, u.cache_write_5m, u.cache_read, u.output],
            usd: crate::price::cost_by_kind(rate, &u),
            co2: [0.0; 4],
        };
        let mut agg = Aggregates {
            weeks: 1.0,
            spend_all: spend,
            ..Default::default()
        };
        agg.segments.push(SegmentAgg {
            project: "p".into(),
            session: "s".into(),
            start_ms: 0,
            segment: 0,
            turns: 200,
            mean_ctx: 550_000.0,
            max_ctx: 600_000,
            output: 0,
            usd: None,
        });
        let f = findings(&agg, &Thresholds::default());
        let r17 = f.iter().find(|f| f.rule == "R17").expect("R17 fires");
        assert_eq!(r17.tokens_saved_per_week.value, Some(20_000_000.0));
        assert_eq!(r17.usd_saved_per_week.value, Some(10.0));
    }

    /// Opus-4-5 cache reads at $0.50 per million; expected shares worked out by hand.
    #[test]
    fn last_week_double_the_window_average() {
        let table = crate::price::parse_table(include_str!("prices/anthropic.json")).unwrap();
        let rate = crate::price::rate_for(&table, "claude-opus-4-5-20251101").unwrap();
        let u = Usage {
            input: 0,
            cache_write_5m: 0,
            cache_write_1h: 0,
            cache_read: 1_000_000,
            output: 0,
            web_search_requests: 0,
        };
        let spend = KindSpend {
            tok: [u.input, u.cache_write_5m, u.cache_read, u.output],
            usd: crate::price::cost_by_kind(rate, &u),
            co2: [0.0; 4],
        };
        let seg = |project: &str, session: &str, start_ms| SegmentAgg {
            project: project.into(),
            session: session.into(),
            start_ms,
            segment: 0,
            turns: 200,
            mean_ctx: 550_000.0,
            max_ctx: 600_000,
            output: 0,
            usd: None,
        };
        let day = 86_400_000;
        let mut window = Aggregates {
            weeks: 4.0,
            spend_all: spend,
            ..Default::default()
        };
        window.segments = vec![
            seg("p", "old", 2 * day),
            seg("p", "new", 25 * day),
            seg("q", "old", 3 * day),
        ];
        let mut recent = Aggregates {
            weeks: 1.0,
            spend_all: spend,
            ..Default::default()
        };
        recent.segments = vec![seg("p", "new", 25 * day)];
        let mut f = findings(&window, &Thresholds::default());
        mark_recent(&mut f, &findings(&recent, &Thresholds::default()));
        let sum = |p: &str, get: fn(&Finding) -> Option<f64>| -> f64 {
            f.iter()
                .filter(|x| x.rule == "R17" && x.project.as_deref() == Some(p))
                .map(|x| get(x).unwrap())
                .sum()
        };
        assert_eq!(sum("p", |x| x.usd_saved_per_week.value), 5.0);
        assert_eq!(sum("p", |x| x.usd_saved_last_7d.value), 10.0);
        assert_eq!(sum("q", |x| x.usd_saved_last_7d.value), 0.0);
    }
}
