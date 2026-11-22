// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! PNGs are written from Rust, so the webview needs no fs or dialog capability.

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use tokenwhy_core::pipeline;
use tokenwhy_wrapped::svg::{self, Palette};
use tokenwhy_wrapped::{cards, Period};

#[derive(Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Range {
    This,
    Last,
    Custom { from: String, to: String },
}

#[derive(Deserialize)]
pub struct Colors {
    plane: String,
    card: String,
    fg: String,
    fg2: String,
    muted: String,
    rule: String,
    accent: String,
    accent_soft: String,
    accent_ink: String,
    series: [String; 7],
}

#[derive(Serialize)]
pub struct CardOut {
    id: &'static str,
    svg: String,
    text: String,
}

#[derive(Serialize)]
pub struct Out {
    label: String,
    cards: Vec<CardOut>,
    about: Vec<String>,
}

fn color(s: &str) -> Result<String, String> {
    let s = s.trim();
    let ok = s.len() <= 40
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || " #(),.%".contains(c));
    if ok && !s.is_empty() {
        Ok(s.to_string())
    } else {
        Err(format!("bad colour: {s}"))
    }
}

fn palette(c: &Colors) -> Result<Palette, String> {
    let series: Result<Vec<String>, String> = c.series.iter().map(|s| color(s)).collect();
    Ok(Palette {
        plane: color(&c.plane)?,
        card: color(&c.card)?,
        fg: color(&c.fg)?,
        fg2: color(&c.fg2)?,
        muted: color(&c.muted)?,
        rule: color(&c.rule)?,
        accent: color(&c.accent)?,
        accent_soft: color(&c.accent_soft)?,
        accent_ink: color(&c.accent_ink)?,
        series: series?.try_into().map_err(|_| "seven series colours")?,
    })
}

fn period(r: &Range, today: NaiveDate) -> Result<Period, String> {
    let day = |s: &str| {
        NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").map_err(|_| format!("bad date: {s}"))
    };
    match r {
        Range::This => Period::year(today.year()),
        Range::Last => Period::year(today.year() - 1),
        Range::Custom { from, to } => Period::range(day(from)?, day(to)?),
    }
}

fn label(p: &Period) -> String {
    match p.year {
        Some(y) => y.to_string(),
        None => format!("{}_{}", p.from, p.to.pred_opt().unwrap_or(p.to)),
    }
}

fn pictures() -> Option<PathBuf> {
    let var = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let base = if cfg!(windows) {
        var("USERPROFILE")?.join("Pictures")
    } else {
        var("XDG_PICTURES_DIR").or_else(|| var("HOME").map(|h| h.join("Pictures")))?
    };
    Some(base.join("tokenwhy-wrapped"))
}

fn build(
    app: &AppHandle,
    range: Range,
    provider: Option<String>,
    names: bool,
    colors: &Colors,
    variant: Option<&str>,
) -> Result<(Period, Vec<CardOut>, Vec<String>), String> {
    let now = pipeline::now_ms();
    let per = period(&range, tokenwhy_wrapped::date_of(now))?;
    let pal = palette(colors)?;
    let p2 = per.clone();
    let (cs, about) = app
        .state::<crate::shell::Shell>()
        .engine
        .read(move |pipe| {
            let w = tokenwhy_wrapped::compute(
                pipe.history(),
                &p2,
                provider.as_deref(),
                None,
                names,
                now,
            );
            (cards::cards(&w), cards::about(&w))
        })?;
    let total = cs.len();
    let out = cs
        .iter()
        .enumerate()
        .map(|(i, c)| CardOut {
            id: c.id,
            svg: svg::render(c, i + 1, total, &pal, variant),
            text: c.text(),
        })
        .collect();
    Ok((per, out, about))
}

#[tauri::command]
pub fn wrapped_available() -> bool {
    true
}

#[tauri::command]
pub fn wrapped_font(which: usize) -> Result<tauri::ipc::Response, String> {
    let f = tokenwhy_wrapped::FONTS.get(which).ok_or("no such font")?;
    Ok(tauri::ipc::Response::new(f.to_vec()))
}

#[tauri::command]
pub async fn wrapped(
    app: AppHandle,
    range: Range,
    provider: Option<String>,
    names: bool,
    colors: Colors,
    variant: Option<String>,
) -> Result<Out, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (per, cards, about) = build(&app, range, provider, names, &colors, variant.as_deref())?;
        Ok(Out {
            label: label(&per),
            cards,
            about,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn wrapped_save(
    app: AppHandle,
    range: Range,
    provider: Option<String>,
    names: bool,
    colors: Colors,
    variant: Option<String>,
    which: Option<usize>,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (per, cards, _) = build(&app, range, provider, names, &colors, variant.as_deref())?;
        let dir = pictures().ok_or("no Pictures folder")?.join(label(&per));
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for (i, c) in cards.iter().enumerate() {
            if which.is_some_and(|w| w != i) {
                continue;
            }
            let png = tokenwhy_card::png(&c.svg, &tokenwhy_wrapped::FONTS)?;
            let f = dir.join(format!("{:02}-{}.png", i + 1, c.id));
            std::fs::write(&f, png).map_err(|e| format!("{}: {e}", f.display()))?;
        }
        crate::shell::reveal(&dir);
        Ok(dir.display().to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_range_under_a_week_is_refused() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 23).unwrap();
        let r = |f: &str, t: &str| Range::Custom {
            from: f.into(),
            to: t.into(),
        };
        assert!(period(&r("2026-03-01", "2026-03-06"), today).is_err());
        let p = period(&r("2026-03-01", "2026-03-07"), today).unwrap();
        assert_eq!(label(&p), "2026-03-01_2026-03-07");
        assert_eq!(label(&period(&Range::Last, today).unwrap()), "2025");
    }

    #[test]
    fn colour_cannot_break_out_of_the_attribute() {
        assert!(color("rgba(23, 23, 35, 0.34)").is_ok());
        assert!(color("#fff\"/><script").is_err());
    }
}
