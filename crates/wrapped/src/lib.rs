// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Computed from the long-term history only, so two runs on the same history give the same cards.

pub mod archetype;
pub mod cards;
pub mod equiv;
pub mod svg;

use chrono::{DateTime, Datelike, Local, NaiveDate, TimeZone, Weekday};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use tokenwhy_core::history::{By, History};
use tokenwhy_core::model::{TimestampMs, Vendor};

pub const FONT_REGULAR: &[u8] = include_bytes!("../assets/twcard-Regular.ttf");
pub const FONT_HEAVY: &[u8] = include_bytes!("../assets/twcard-Heavy.ttf");
pub const FONTS: [&[u8]; 2] = [FONT_REGULAR, FONT_HEAVY];

pub const MIN_DAYS: i64 = 7;
const LAST_THIRD_MIN_EDITS: u32 = 30;

pub fn date_of(ms: TimestampMs) -> NaiveDate {
    DateTime::from_timestamp_millis(ms)
        .unwrap_or_default()
        .with_timezone(&Local)
        .date_naive()
}

pub fn midnight(d: NaiveDate) -> TimestampMs {
    Local
        .from_local_datetime(&d.and_hms_opt(0, 0, 0).unwrap_or_default())
        .earliest()
        .map_or(0, |t| t.timestamp_millis())
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Period {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub year: Option<i32>,
}

impl Period {
    pub fn year(y: i32) -> Result<Period, String> {
        let d = |y| NaiveDate::from_ymd_opt(y, 1, 1).ok_or(format!("bad year {y}"));
        Ok(Period {
            from: d(y)?,
            to: d(y + 1)?,
            year: Some(y),
        })
    }

    pub fn range(from: NaiveDate, last: NaiveDate) -> Result<Period, String> {
        let to = last.succ_opt().ok_or("bad end date")?;
        if (to - from).num_days() < MIN_DAYS {
            return Err(format!("a Wrapped needs at least {MIN_DAYS} days"));
        }
        Ok(Period {
            from,
            to,
            year: None,
        })
    }

    pub fn from_ms(&self) -> TimestampMs {
        midnight(self.from)
    }
    pub fn to_ms(&self) -> TimestampMs {
        midnight(self.to)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Share {
    pub name: String,
    pub share: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Marathon {
    pub minutes: u32,
    pub turns: Option<u32>,
    pub day: NaiveDate,
    pub compactions: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Wrapped {
    pub period: Period,
    pub open: bool,
    pub provider: Option<String>,
    pub tokens_total: u64,
    pub cache_share: f64,
    pub output_tokens: u64,
    pub cost_usd: f64,
    pub cost_partial: bool,
    pub cost_peak: Option<(NaiveDate, f64)>,
    pub active_days: u32,
    pub calendar_days: u32,
    pub longest_streak: Option<(u32, NaiveDate)>,
    pub current_streak: Option<u32>,
    pub hours: [u32; 24],
    pub peak_hour: Option<u8>,
    pub night_share: f64,
    pub early_share: f64,
    pub weekend_share: f64,
    pub weekday_cost: [f64; 7],
    pub busiest_weekday: Option<u8>,
    pub sessions: u32,
    pub sessions_per_day: f64,
    pub projects: Vec<Share>,
    pub project_count: u32,
    pub models: Vec<Share>,
    pub agent_share: f64,
    pub compactions: u32,
    pub compactions_per_session: f64,
    pub edits: u32,
    pub cost_per_edit: Option<(f64, f64)>,
    pub cost_per_edit_drop: Option<f64>,
    pub cache_saved_usd: f64,
    pub habit_saved_usd: Option<f64>,
    pub co2_g: Option<f64>,
    pub co2_g_range: Option<(f64, f64)>,
    pub co2_method: Option<&'static str>,
    pub water_l_range: Option<(f64, f64)>,
    pub co2_coverage: f64,
    pub langs: Vec<Share>,
    pub lang_edits: u32,
    pub langs_since: Option<NaiveDate>,
    pub marathon: Option<Marathon>,
    pub words: u64,
    pub first_day: Option<NaiveDate>,
    pub first_day_ever: Option<NaiveDate>,
    pub top_tier_share: f64,
    pub archetype: archetype::Result,
}

pub fn model_family(model: &str) -> String {
    let parts: Vec<&str> = model.split('-').collect();
    let dated = |p: &str| p.len() == 8 && p.bytes().all(|b| b.is_ascii_digit());
    let parts: Vec<&str> = match parts.split_last() {
        Some((last, rest)) if dated(last) => rest.to_vec(),
        _ => parts,
    };
    if parts.first() != Some(&"claude") {
        return parts.join("-");
    }
    let num = |p: &&str| p.bytes().all(|b| b.is_ascii_digit());
    let names: Vec<&str> = parts[1..].iter().filter(|p| !num(p)).copied().collect();
    let version: Vec<&str> = parts[1..].iter().filter(|p| num(p)).copied().collect();
    let mut s = names.join(" ");
    if !version.is_empty() {
        s.push(' ');
        s.push_str(&version.join("."));
    }
    s
}

fn rate_input(model: &str) -> Option<(f64, f64)> {
    [Vendor::Anthropic, Vendor::OpenAi, Vendor::Google]
        .into_iter()
        .find_map(|v| tokenwhy_core::price::rate_for(tokenwhy_core::price::vendor_table(v), model))
        .map(|r| (r.input, r.cache_read))
}

fn shares(m: BTreeMap<String, f64>) -> Vec<Share> {
    let total: f64 = m.values().sum();
    let mut v: Vec<Share> = m
        .into_iter()
        .filter(|(_, x)| *x > 0.0)
        .map(|(name, x)| Share {
            name,
            share: if total > 0.0 { x / total } else { 0.0 },
        })
        .collect();
    v.sort_by(|a, b| {
        b.share
            .total_cmp(&a.share)
            .then_with(|| a.name.cmp(&b.name))
    });
    v
}

fn ratio(a: f64, b: f64) -> f64 {
    if b > 0.0 {
        a / b
    } else {
        0.0
    }
}

pub fn compute(
    h: &History,
    period: &Period,
    provider: Option<&str>,
    account: Option<&str>,
    names: bool,
    now_ms: TimestampMs,
) -> Wrapped {
    let (from, to) = (period.from_ms(), period.to_ms());
    let today = date_of(now_ms);
    let by_model = h.daily_for(from, to, By::Model, None, provider, account);
    let by_day = h.daily_for(from, to, By::None, None, provider, account);
    let by_project = h.daily_for(from, to, By::Project, None, provider, account);
    let entries = h.entries_for(from, to, provider, account);

    let active: BTreeSet<NaiveDate> = by_day
        .iter()
        .filter(|r| r.fresh_tokens > 0)
        .map(|r| date_of(r.day_ms))
        .collect();
    let n_active = active.len() as u32;
    let cost_of = |r: &tokenwhy_core::model::DailyRow| r.cost_usd.value.unwrap_or(0.0);

    let output_tokens: u64 = by_day.iter().filter_map(|r| r.output_tokens).sum();
    let fresh: u64 = by_day.iter().map(|r| r.fresh_tokens).sum();
    let read: u64 = by_day.iter().map(|r| r.cache_read_tokens).sum();
    let prompt = (fresh - output_tokens.min(fresh)) + read;
    let cost_usd: f64 = by_day.iter().map(cost_of).sum();
    let cost_peak = by_day
        .iter()
        .filter(|r| r.cost_usd.value.is_some())
        .max_by(|a, b| cost_of(a).total_cmp(&cost_of(b)))
        .map(|r| (date_of(r.day_ms), cost_of(r)));

    let mut longest: Option<(u32, NaiveDate)> = None;
    let mut run: Option<(u32, NaiveDate, NaiveDate)> = None;
    for d in &active {
        run = match run {
            Some((n, start, last)) if last.succ_opt() == Some(*d) => Some((n + 1, start, *d)),
            _ => Some((1, *d, *d)),
        };
        let (n, start, _) = run.unwrap_or((0, *d, *d));
        if longest.is_none_or(|(m, _)| n > m) {
            longest = Some((n, start));
        }
    }
    let current_streak = run
        .filter(|(_, _, last)| {
            period.to > today && (*last == today || last.succ_opt() == Some(today))
        })
        .map(|(n, _, _)| n)
        .filter(|n| *n >= 2);

    let mut masks: BTreeMap<NaiveDate, u32> = BTreeMap::new();
    for e in entries.iter().filter(|e| e.main) {
        for d in &e.days {
            if let Some(m) = d.hours {
                *masks.entry(date_of(d.day_ms)).or_default() |= m;
            }
        }
    }
    let mut hours = [0u32; 24];
    for m in masks.values() {
        for (h, slot) in hours.iter_mut().enumerate() {
            *slot += (m >> h) & 1;
        }
    }
    let total_h: u32 = hours.iter().sum();
    let sum_h = |hs: &[usize]| hs.iter().map(|&h| hours[h]).sum::<u32>() as f64;
    let peak_hour = (total_h > 0).then(|| {
        (0..24u8)
            .max_by(|a, b| hours[*a as usize].cmp(&hours[*b as usize]).then(b.cmp(a)))
            .unwrap_or(0)
    });

    let mut weekday_cost = [0f64; 7];
    for r in &by_day {
        weekday_cost[date_of(r.day_ms).weekday().num_days_from_monday() as usize] += cost_of(r);
    }
    let busiest_weekday = (cost_usd > 0.0).then(|| {
        (0..7u8)
            .max_by(|a, b| {
                weekday_cost[*a as usize]
                    .total_cmp(&weekday_cost[*b as usize])
                    .then(b.cmp(a))
            })
            .unwrap_or(0)
    });
    let weekend = active
        .iter()
        .filter(|d| matches!(d.weekday(), Weekday::Sat | Weekday::Sun))
        .count();

    let sessions = entries.iter().filter(|e| e.main).count() as u32;
    let compactions: u32 = by_day.iter().map(|r| r.compactions).sum();

    let mut per_project: BTreeMap<String, f64> = BTreeMap::new();
    for r in &by_project {
        *per_project
            .entry(r.project.clone().unwrap_or_default())
            .or_default() += cost_of(r);
    }
    // Rows without a real project, like "(stats-cache)" history or an import, are not projects to name or count.
    per_project.retain(|name, _| !name.is_empty() && !name.starts_with('('));
    let project_count = per_project.len() as u32;
    let mut projects = shares(per_project);
    projects.truncate(3);
    if !names {
        for (i, p) in projects.iter_mut().enumerate() {
            p.name = format!("project{}", i + 1);
        }
    }

    let mut per_family: BTreeMap<String, f64> = BTreeMap::new();
    let mut cache_saved_usd = 0.0;
    let mut tiers: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    for r in &by_model {
        let model = r.model.clone().unwrap_or_default();
        *per_family.entry(model_family(&model)).or_default() += cost_of(r);
        if let Some((input, cache)) = rate_input(&model) {
            cache_saved_usd += r.cache_read_tokens as f64 * (input - cache) / 1e6;
            tiers.entry(model).or_insert((input, 0.0)).1 += cost_of(r);
        }
    }
    let top_rate = tiers
        .values()
        .filter(|t| t.1 > 0.0)
        .map(|t| t.0)
        .fold(0.0, f64::max);
    let top_tier_share = ratio(
        tiers
            .values()
            .filter(|t| t.0 == top_rate)
            .map(|t| t.1)
            .sum(),
        cost_usd,
    );

    let agent_cost: f64 = by_day.iter().map(|r| r.agent_cost_usd).sum();
    let edits: u32 = by_day.iter().filter_map(|r| r.edits).sum();

    let days: Vec<NaiveDate> = active.iter().copied().collect();
    let third = days.len() / 3;
    let span = |ds: &[NaiveDate]| {
        let set: BTreeSet<&NaiveDate> = ds.iter().collect();
        by_day
            .iter()
            .filter(|r| set.contains(&date_of(r.day_ms)))
            .fold((0.0, 0u32), |(c, e), r| {
                (c + cost_of(r), e + r.edits.unwrap_or(0))
            })
    };
    let (first, last) = (span(&days[..third]), span(&days[days.len() - third..]));
    let cost_per_edit =
        (third > 0 && first.1 >= LAST_THIRD_MIN_EDITS && last.1 >= LAST_THIRD_MIN_EDITS)
            .then(|| (first.0 / f64::from(first.1), last.0 / f64::from(last.1)));
    let cost_per_edit_drop = cost_per_edit.map(|(c1, c3)| ratio(c1 - c3, c1));
    let habit_saved_usd = cost_per_edit
        .zip(cost_per_edit_drop)
        .filter(|(_, d)| *d > 0.10)
        .map(|((c1, c3), _)| (c1 - c3) * f64::from(last.1));

    let co2: Vec<(f64, f64)> = by_day
        .iter()
        .filter(|r| r.fresh_tokens > 0)
        .filter_map(|r| r.co2_g.map(|lo| (lo, r.co2_g_max.unwrap_or(lo))))
        .collect();
    let co2_g_range = (!co2.is_empty()).then(|| {
        co2.iter()
            .fold((0.0, 0.0), |(a, b), (lo, hi)| (a + lo, b + hi))
    });
    let co2_g = co2_g_range.map(|(lo, hi)| (lo + hi) / 2.0);
    let water: Vec<(f64, f64)> = by_day
        .iter()
        .filter(|r| r.fresh_tokens > 0)
        .filter_map(|r| r.water_l.map(|lo| (lo, r.water_l_max.unwrap_or(lo))))
        .collect();
    let water_l_range = (!water.is_empty())
        .then(|| {
            water
                .iter()
                .fold((0.0, 0.0), |(a, b), (lo, hi)| (a + lo, b + hi))
        })
        .filter(|(_, hi)| *hi > 0.0);

    let mut lang_counts: BTreeMap<u8, u32> = BTreeMap::new();
    let mut langs_since: Option<NaiveDate> = None;
    for e in &entries {
        for d in &e.days {
            let Some(ls) = &d.langs else { continue };
            let day = date_of(d.day_ms);
            langs_since = Some(langs_since.map_or(day, |s| s.min(day)));
            for &(id, n, _) in ls.iter().filter(|l| l.0 != 0) {
                *lang_counts.entry(id).or_default() += n;
            }
        }
    }
    let lang_edits: u32 = lang_counts.values().sum();
    let langs = shares(
        lang_counts
            .into_iter()
            .map(|(id, n)| (tokenwhy_core::lang::name(id).to_string(), f64::from(n)))
            .collect(),
    );

    const MAX_SESSION_MS: i64 = 24 * 3_600_000;
    let marathon = entries
        .iter()
        .filter(|e| e.main)
        .filter_map(|e| Some((e, e.last_ms? - e.floor_ms)))
        .filter(|(_, ms)| (1..=MAX_SESSION_MS).contains(ms))
        .max_by_key(|(_, ms)| *ms)
        .map(|(e, ms)| Marathon {
            minutes: (ms / 60_000) as u32,
            turns: e.turns,
            day: date_of(e.floor_ms),
            compactions: e.days.iter().map(|d| d.compactions).sum(),
        });

    let first_day_ever = h
        .daily_for(0, TimestampMs::MAX, By::None, None, provider, account)
        .iter()
        .find(|r| r.fresh_tokens > 0)
        .map(|r| date_of(r.day_ms));

    let mut w = Wrapped {
        period: period.clone(),
        open: period.to > today,
        provider: provider.map(str::to_string),
        tokens_total: fresh + read,
        cache_share: ratio(read as f64, prompt as f64),
        output_tokens,
        cost_usd,
        cost_partial: by_day
            .iter()
            .any(|r| r.cost_usd.value.is_none() && r.fresh_tokens > 0),
        cost_peak,
        active_days: n_active,
        calendar_days: (period.to.min(today.succ_opt().unwrap_or(today)) - period.from)
            .num_days()
            .max(0) as u32,
        longest_streak: longest,
        current_streak,
        hours,
        peak_hour,
        night_share: ratio(sum_h(&[22, 23, 0, 1, 2, 3]), f64::from(total_h)),
        early_share: ratio(sum_h(&[5, 6, 7, 8]), f64::from(total_h)),
        weekend_share: ratio(weekend as f64, f64::from(n_active)),
        weekday_cost,
        busiest_weekday,
        sessions,
        sessions_per_day: ratio(f64::from(sessions), f64::from(n_active)),
        projects,
        project_count,
        models: shares(per_family),
        agent_share: ratio(agent_cost, cost_usd),
        compactions,
        compactions_per_session: ratio(f64::from(compactions), f64::from(sessions)),
        edits,
        cost_per_edit,
        cost_per_edit_drop,
        cache_saved_usd,
        habit_saved_usd,
        co2_g,
        co2_g_range,
        co2_method: h.co2_method(),
        water_l_range,
        co2_coverage: ratio(co2.len() as f64, f64::from(n_active)),
        langs,
        lang_edits,
        langs_since,
        marathon,
        words: output_tokens * 3 / 4,
        first_day: active.first().copied(),
        first_day_ever,
        top_tier_share,
        archetype: archetype::Result::default(),
    };
    w.archetype = archetype::classify(&w);
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_families_drop_vendor_and_date() {
        assert_eq!(model_family("claude-opus-4-5-20251101"), "opus 4.5");
        assert_eq!(model_family("claude-3-5-sonnet-20241022"), "sonnet 3.5");
        assert_eq!(model_family("claude-fable-5"), "fable 5");
        assert_eq!(model_family("gpt-5-codex"), "gpt-5-codex");
    }

    #[test]
    fn ranges_under_a_week_are_refused() {
        let d = |m, d| NaiveDate::from_ymd_opt(2026, m, d).unwrap();
        assert!(Period::range(d(3, 1), d(3, 6)).is_err());
        assert!(Period::range(d(3, 1), d(3, 7)).is_ok());
    }
}
