// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Copy rules: plain words, no em dash.

use crate::archetype;
use crate::Wrapped;
use chrono::NaiveDate;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Viz {
    None,
    Clock([u32; 24], [f64; 7]),
    Bars(Vec<(String, f64)>),
    Poster(Vec<(String, String)>),
    Figure(String, String, String),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Card {
    pub id: &'static str,
    pub kicker: String,
    pub big: String,
    pub unit: String,
    pub line: String,
    pub foot: String,
    pub viz: Viz,
}

impl Card {
    pub fn text(&self) -> String {
        let mut s = format!("{}\n  {}", self.kicker, self.big);
        if !self.unit.is_empty() {
            s.push(' ');
            s.push_str(&self.unit);
        }
        for t in [&self.line, &self.foot] {
            if !t.is_empty() {
                s.push_str("\n  ");
                s.push_str(t);
            }
        }
        if let Viz::Figure(big, unit, line) = &self.viz {
            s.push_str(&format!(
                "
  {big} {unit}
  {line}"
            ));
        }
        if let Viz::Poster(tiles) = &self.viz {
            for (k, v) in tiles {
                s.push_str(&format!("\n  {k}: {v}"));
            }
        }
        s
    }
}

pub fn short(n: f64) -> String {
    let (v, suffix) = match n {
        n if n >= 1e12 => (n / 1e12, "T"),
        n if n >= 1e9 => (n / 1e9, "B"),
        n if n >= 1e6 => (n / 1e6, "M"),
        n if n >= 1e3 => (n / 1e3, "K"),
        n => return format!("{}", n.round()),
    };
    if v >= 100.0 {
        format!("{v:.0}{suffix}")
    } else {
        format!("{v:.1}{suffix}")
    }
}

fn grouped(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

pub fn money(usd: f64) -> String {
    if usd <= 0.0 {
        return "$0".into();
    }
    let digits = usd.log10().floor() as i32 + 1;
    if digits >= 3 {
        let unit = 10f64.powi(digits - 3);
        format!("${}", grouped(((usd / unit).round() * unit) as u64))
    } else {
        let dec = (3 - digits).clamp(0, 2) as usize;
        format!("${usd:.dec$}")
    }
}

pub fn co2(g: f64) -> String {
    let (v, unit) = match g {
        g if g >= 1e6 => (g / 1e6, "t"),
        g if g >= 1e3 => (g / 1e3, "kg"),
        g => (g, "g"),
    };
    let p = 10f64.powi(v.log10().floor() as i32);
    let r = (v / p).round() * p;
    if r >= 1.0 {
        format!("{r:.0} {unit}")
    } else {
        format!("{r} {unit}")
    }
}

fn pct(x: f64) -> String {
    format!("{:.0}%", x * 100.0)
}

pub fn date(d: NaiveDate) -> String {
    d.format("%b %-d").to_string()
}

fn date_y(d: NaiveDate) -> String {
    d.format("%b %-d, %Y").to_string()
}

const WEEKDAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

fn hour(h: u8) -> String {
    format!("{h}:00")
}

fn agent_name(w: &Wrapped) -> &'static str {
    match w.provider.as_deref() {
        Some("claude-code") => "Claude Code",
        _ => "your coding agents",
    }
}

/// Markdown and Config never win the headline.
fn top_lang(w: &Wrapped) -> Option<&crate::Share> {
    w.langs
        .iter()
        .find(|l| l.name != "Markdown" && l.name != "Config")
}

const BIG_TOKENS: u64 = 1_000_000_000;

fn co2_equiv(w: &Wrapped) -> Option<String> {
    w.co2_g
        .filter(|g| *g > 0.0)
        .and_then(|g| crate::equiv::pick(g, crate::equiv::CO2))
        .map(|(e, c)| crate::equiv::phrase(e, c))
}

pub fn water(lo: f64, hi: f64) -> String {
    let m3 = hi >= 1000.0;
    let f = |l: f64| {
        let v = if m3 { l / 1000.0 } else { l };
        if v <= 0.0 {
            return "0".to_string();
        }
        let p = 10f64.powi(v.log10().floor() as i32 - 1);
        let r = (v / p).round() * p;
        if r >= 10.0 {
            format!("{r:.0}")
        } else {
            format!("{}", (r * 10.0).round() / 10.0)
        }
    };
    let unit = if m3 { "m³" } else { "L" };
    if (hi - lo).abs() <= 1e-9 * hi {
        format!("~{} {unit}", f(lo))
    } else {
        format!("~{} to {} {unit}", f(lo), f(hi))
    }
}

#[cfg(feature = "co2")]
fn water_figure(w: &Wrapped) -> Option<Viz> {
    let (lo, hi) = w.water_l_range?;
    let mid = (lo + hi) / 2.0;
    let eq = crate::equiv::pick(mid, &crate::equiv::WATER)
        .map(|(e, c)| crate::equiv::phrase(e, c))
        .unwrap_or_default();
    Some(Viz::Figure(water(mid, mid), "water".into(), eq))
}

#[cfg(not(feature = "co2"))]
fn water_figure(_: &Wrapped) -> Option<Viz> {
    None
}

pub fn about(w: &Wrapped) -> Vec<String> {
    let mut v = vec![
        "Everything is counted from the session logs on this machine; nothing is sent anywhere."
            .to_string(),
        "Costs and savings are at API list prices. Your plan may cost less.".to_string(),
        "Tokens become words at 3/4 of a word per token (OpenAI's rule of thumb).".to_string(),
        "Languages are counted from the file extensions of edited files.".to_string(),
    ];
    if w.co2_g.is_some_and(|g| g > 0.0) {
        let (lo, hi) = w.co2_g_range.unwrap_or_default();
        let how = match w.co2_method {
            Some("jegham") => "per-token factors from Jegham et al. 2025, via claude-carbon; cache reads count at a share of input".to_string(),
            _ => format!(
                "EcoLogits, {} to {}; the card shows the middle. It counts output tokens only, not input or cache",
                co2(lo),
                co2(hi)
            ),
        };
        v.push(format!(
            "CO2e is an order of magnitude, not a measure: {how}. It covers {} of your active days.",
            pct(w.co2_coverage)
        ));
        if let Some((lo, hi)) = w.water_l_range {
            v.push(format!(
                "Water {}: EcoLogits' cooling and power-plant water for the same output tokens. Volumes: C.I.EAU (143 L a person a day), metric cup and drop, Olympic pool 2,500 m³.",
                water(lo, hi)
            ));
        }
    }
    v.push("Equivalents: ADEME Impact CO2 (flights, meals), EPA (driving, phone charges), Carbon Trust (streaming), Wikipedia and Guinness (word counts), Claude Pro at $20 a month, Toast for the price of a coffee.".into());
    v
}

fn plural(n: u32, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

pub fn cards(w: &Wrapped) -> Vec<Card> {
    let mut out = vec![];
    let card = |id, kicker: &str, big: String, unit: &str, line: String, foot: String| Card {
        id,
        kicker: kicker.to_string(),
        big,
        unit: unit.to_string(),
        line,
        foot,
        viz: Viz::None,
    };
    let p = &w.period;
    let (big, line) = match p.year {
        Some(y) => (
            y.to_string(),
            format!(
                "Your year with {}{}, counted on this machine.",
                agent_name(w),
                if w.open { " so far" } else { "" }
            ),
        ),
        None => (
            format!("{} days", (p.to - p.from).num_days()),
            format!(
                "{} to {} with {}, counted on this machine.",
                date_y(p.from),
                date_y(p.to.pred_opt().unwrap_or(p.to)),
                agent_name(w)
            ),
        ),
    };
    out.push(card(
        "hello",
        "Wrapped",
        big,
        "",
        line,
        "Nothing left your computer to make these cards.".into(),
    ));

    if w.tokens_total > 0 {
        let foot = crate::equiv::line(
            w.tokens_total as f64 * crate::equiv::WORDS_PER_TOKEN,
            crate::equiv::WORDS,
        )
        .map(|l| {
            if w.tokens_total >= BIG_TOKENS {
                format!("{l} Light reading.")
            } else {
                l
            }
        })
        .unwrap_or_default();
        out.push(card(
            "tokens",
            "Tokens",
            short(w.tokens_total as f64),
            "tokens",
            format!(
                "{} came straight from the cache. Claude wrote you {} words.",
                pct(w.cache_share),
                short(w.words as f64)
            ),
            foot,
        ));
    }

    if let Some((day, peak)) = w.cost_peak.filter(|_| w.cost_usd > 0.0) {
        out.push(card(
            "cost",
            if w.cost_partial {
                "Cost, at least"
            } else {
                "Cost"
            },
            money(w.cost_usd),
            "",
            format!(
                "at API list prices.\nPriciest day: {}, {}.",
                date(day),
                money(peak)
            ),
            crate::equiv::line(w.cost_usd, crate::equiv::USD).unwrap_or_default(),
        ));
    }

    if let Some((n, start)) = w.longest_streak.filter(|_| w.active_days >= 7) {
        out.push(card(
            "streaks",
            "Streaks",
            n.to_string(),
            if n == 1 { "day" } else { "days" },
            format!(
                "Longest run, starting {}. You showed up {} in all.",
                date(start),
                plural(w.active_days, "day", "days")
            ),
            match w.current_streak {
                Some(c) if c == n => "And it's still going...".to_string(),
                Some(c) => format!("And {} into a new one.", plural(c, "day", "days")),
                None if n >= 30 => {
                    "A whole month without a day off. You really like it, huh?".into()
                }
                None if n >= 14 => "Two weeks straight. Respect.".into(),
                None => String::new(),
            },
        ));
    }

    let total_h: u32 = w.hours.iter().sum();
    if let Some(peak) = w.peak_hour.filter(|_| total_h >= 20) {
        let line = match w.busiest_weekday {
            Some(d) => format!("Peak hour. {} is your busiest day.", WEEKDAYS[d as usize]),
            None => "Peak hour.".to_string(),
        };
        let foot = if !(4..23).contains(&peak) {
            "Past midnight is prime time, apparently."
        } else if w.night_share >= archetype::NIGHT {
            "Most of your work happens after dark."
        } else if (4..8).contains(&peak) || w.early_share >= archetype::EARLY {
            "You start before the coffee is ready."
        } else if matches!(w.busiest_weekday, Some(5 | 6)) {
            "Weekends are for side projects, right?"
        } else if (12..14).contains(&peak) {
            "Lunch break? Never heard of it."
        } else {
            ""
        };
        let mut c = card("clock", "Clock", hour(peak), "", line, foot.into());
        c.viz = Viz::Clock(w.hours, w.weekday_cost);
        out.push(c);
    }

    if let Some(top) = top_lang(w).filter(|_| w.lang_edits >= 30) {
        let rest: Vec<&str> = w
            .langs
            .iter()
            .filter(|l| l.name != top.name && l.name != "Markdown" && l.name != "Config")
            .take(2)
            .map(|l| l.name.as_str())
            .collect();
        let mut line = format!("{} of your edits.", pct(top.share));
        match rest.as_slice() {
            [a, b] => line.push_str(&format!("\nThen {a} and {b}.")),
            [a] => line.push_str(&format!("\nThen {a}.")),
            _ => {}
        }
        if w.langs
            .iter()
            .any(|l| l.name == "Markdown" && l.share > 0.25)
        {
            line.push_str(" Plus a lot of Markdown.");
        }
        let n_langs = w
            .langs
            .iter()
            .filter(|l| l.share >= archetype::POLYGLOT_SHARE)
            .count();
        let foot = if top.share > 0.5 {
            format!("You really like {}, huh?", top.name)
        } else if n_langs >= archetype::POLYGLOT_LANGS {
            format!("{n_langs} languages. Who needs a favourite?")
        } else {
            "Counted from file extensions.".to_string()
        };
        let mut c = card("languages", "Languages", top.name.clone(), "", line, foot);
        c.viz = Viz::Bars(
            w.langs
                .iter()
                .take(5)
                .map(|l| (l.name.clone(), l.share))
                .collect(),
        );
        out.push(c);
    }

    if let Some(top) = w.models.first().filter(|_| w.cost_usd > 0.0) {
        let mut c = card(
            "models",
            "Models and helpers",
            top.name.clone(),
            &pct(top.share),
            format!(
                "Share of spend.\nSubagents: {}. {:.1} compactions per session.",
                pct(w.agent_share),
                w.compactions_per_session
            ),
            if w.agent_share >= archetype::DELEGATOR {
                "You run a team now. Good manager energy.".into()
            } else if top.share >= archetype::BIG_MODEL {
                "Found your model and stuck with it.".into()
            } else {
                String::new()
            },
        );
        c.viz = Viz::Bars(
            w.models
                .iter()
                .take(4)
                .map(|m| (m.name.clone(), m.share))
                .collect(),
        );
        out.push(c);
    }

    if let Some(m) = &w.marathon {
        let turns = m.turns.map(|t| format!(", {t} turns,")).unwrap_or_default();
        out.push(card(
            "marathon",
            "Marathon",
            format!("{} h {:02}", m.minutes / 60, m.minutes % 60),
            "",
            format!(
                "Your longest session{turns} on {}. {}",
                date(m.day),
                match m.compactions {
                    0 => "Context never reset.".to_string(),
                    1 => "Context reset once.".to_string(),
                    n => format!("Context reset {n} times."),
                }
            ),
            if m.minutes >= 12 * 60 {
                "We hope you slept well after that one.".into()
            } else if m.minutes >= 4 * 60 {
                "Snacks were had, we assume.".into()
            } else {
                String::new()
            },
        ));
    }

    let habit = w.habit_saved_usd.unwrap_or(0.0);
    if w.cache_saved_usd > 0.0 || habit > 0.0 {
        let (big, line, foot) = if habit > w.cache_saved_usd {
            (
                habit,
                format!(
                    "Each edit cost {} less by the end than at the start, at API list prices.",
                    pct(w.cost_per_edit_drop.unwrap_or(0.0))
                ),
                "Practice pays. Nicely done.",
            )
        } else {
            (
                w.cache_saved_usd,
                "What the cache saved you, at API list prices.".to_string(),
                if w.cache_saved_usd > w.cost_usd {
                    "More saved than spent. Not bad at all."
                } else {
                    "Reused context is cheap context."
                },
            )
        };
        out.push(card(
            "saving",
            "Biggest saving",
            money(big),
            "",
            line,
            foot.into(),
        ));
    }

    if let Some(g) = w.co2_g.filter(|g| *g > 0.0) {
        let mut c = card(
            "footprint",
            "Footprint",
            format!("about {}", co2(g)),
            "CO2e",
            co2_equiv(w).map_or_else(String::new, |p| format!("That is {p}.")),
            "A rough guess. Carbon is hard to count.".into(),
        );
        if let Some(fig) = water_figure(w) {
            c.big = format!("~{}", co2(g));
            c.line = co2_equiv(w).unwrap_or_default();
            c.foot = "Rough guesses. Servers get thirsty too.".into();
            c.viz = fig;
        }
        out.push(c);
    }

    let a = archetype::by_id(&w.archetype.primary);
    if w.active_days >= 14 {
        let n_langs = w
            .langs
            .iter()
            .filter(|l| l.share >= archetype::POLYGLOT_SHARE)
            .count();
        let traits: Vec<&str> = w
            .archetype
            .traits
            .iter()
            .map(|t| archetype::by_id(t).name)
            .collect();
        let mut foot = String::new();
        if !traits.is_empty() {
            foot.push_str(&format!("Also {}.", traits.join(" and ")));
        }
        if let Some(tip) = a.tip {
            if !foot.is_empty() {
                foot.push(' ');
            }
            foot.push_str(&format!("Tip: {tip}"));
        }
        out.push(card(
            "personality",
            "Personality",
            a.name.to_string(),
            "",
            a.line.replace("{n}", &n_langs.to_string()),
            foot,
        ));
    }

    let mut tiles = vec![
        ("Tokens".to_string(), short(w.tokens_total as f64)),
        ("Cost".to_string(), money(w.cost_usd)),
        (
            "Longest streak".to_string(),
            w.longest_streak
                .map_or("none".into(), |(n, _)| plural(n, "day", "days")),
        ),
        (
            "Peak hour".to_string(),
            w.peak_hour.map_or("none".into(), hour),
        ),
        (
            "Top language".to_string(),
            top_lang(w).map_or("none".into(), |l| l.name.clone()),
        ),
        ("Personality".to_string(), a.name.to_string()),
    ];
    tiles.retain(|t| t.1 != "none");
    let projects: Vec<&str> = w.projects.iter().map(|p| p.name.as_str()).collect();
    let mut c = card(
        "poster",
        "In short",
        match p.year {
            Some(y) if w.open => format!("{y} so far"),
            Some(y) => y.to_string(),
            None => format!(
                "{} to {}",
                date(p.from),
                date(p.to.pred_opt().unwrap_or(p.to))
            ),
        },
        "",
        format!(
            "{} across {}.",
            plural(w.active_days, "day", "days"),
            plural(w.project_count, "project", "projects")
        ),
        {
            if projects.is_empty() {
                String::new()
            } else {
                format!("Most time on {}.", projects.join(", "))
            }
        },
    );
    c.viz = Viz::Poster(tiles);
    out.push(c);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_round_like_the_copy_rules() {
        assert_eq!(money(1234.5), "$1,230");
        assert_eq!(money(45.234), "$45.2");
        assert_eq!(money(0.4213), "$0.42");
        assert_eq!(money(12_345.0), "$12,300");
        assert_eq!(co2(43_700.0), "40 kg");
        assert_eq!(co2(870.0), "900 g");
        assert_eq!(short(1_234_000_000.0), "1.2B");
        assert_eq!(short(340_400_000.0), "340M");
    }
}
