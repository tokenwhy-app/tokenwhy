// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Expected values follow from the days written below and design-wrapped.md sections 4 and 6.

use chrono::NaiveDate;
use std::collections::BTreeMap;
use std::path::PathBuf;
use tokenwhy_core::history::History;
use tokenwhy_core::metrics::days::DayPart;
use tokenwhy_wrapped::{cards, compute, midnight, svg, Period};

#[derive(serde::Serialize)]
struct E {
    provider: String,
    session: u64,
    project: String,
    main: bool,
    floor_ms: i64,
    days: Vec<DayPart>,
    account: Option<String>,
    last_ms: Option<i64>,
    turns: Option<u32>,
}

#[derive(serde::Serialize)]
struct Disk {
    magic: [u8; 8],
    version: u32,
    files: BTreeMap<u64, E>,
}

fn d(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 3, day).unwrap()
}

fn part(day: u32, model: &str, cost: f64, hours: u32) -> DayPart {
    DayPart {
        day_ms: midnight(d(day)),
        model: model.into(),
        fresh: 1000,
        cache_read: 1000,
        cache_write: 100,
        cost_usd: cost,
        cost_known: true,
        compactions: 0,
        output: Some(200),
        edits: Some(0),
        hours: Some(hours),
        langs: None,
    }
}

struct Builder(BTreeMap<u64, E>);

impl Builder {
    fn file(&mut self, main: bool, project: &str, days: Vec<DayPart>) -> &mut Self {
        let n = self.0.len() as u64;
        let floor = days[0].day_ms + 14 * 3_600_000;
        self.0.insert(
            n,
            E {
                provider: "claude-code".into(),
                session: n,
                project: project.into(),
                main,
                floor_ms: floor,
                days,
                account: None,
                last_ms: Some(floor + 3_600_000),
                turns: Some(40),
            },
        );
        self
    }

    fn open(&mut self, name: &str) -> History {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let bytes = postcard::to_allocvec(&Disk {
            magic: *b"TWHIST01",
            version: 5,
            files: std::mem::take(&mut self.0),
        })
        .unwrap();
        std::fs::write(dir.join("history.bin"), bytes).unwrap();
        History::open(Some(&dir))
    }
}

const OPUS: &str = "claude-opus-4-5";
const HAIKU: &str = "claude-haiku-4-5";
const H14: u32 = 1 << 14;
fn later() -> i64 {
    midnight(NaiveDate::from_ymd_opt(2027, 2, 1).unwrap())
}

fn year(h: &History) -> tokenwhy_wrapped::Wrapped {
    compute(
        h,
        &Period::year(2026).unwrap(),
        Some("claude-code"),
        None,
        false,
        later(),
    )
}

#[test]
fn longest_streak_and_night_shift() {
    let mut b = Builder(BTreeMap::new());
    for day in [2, 3, 4, 5, 6, 9, 10] {
        let late = (1 << 23) | 1;
        let mask = if day >= 6 { H14 } else { late };
        b.file(true, "demo", vec![part(day, HAIKU, 1.0, mask)]);
    }
    let h = b.open("wrapped_streak");
    let w = year(&h);
    assert_eq!(w.longest_streak, Some((5, d(2))));
    assert_eq!(w.active_days, 7);
    assert!((w.night_share - 8.0 / 11.0).abs() < 1e-9);
    assert_eq!(w.archetype.primary, "night-shift");
    assert_eq!(w, year(&h), "same history, same Wrapped");
}

#[test]
fn archetype_profiles() {
    let weekdays = [2, 3, 4, 5, 6, 9, 10, 11, 12, 13, 16, 17, 18, 19];

    let mut b = Builder(BTreeMap::new());
    for day in weekdays {
        b.file(true, "a", vec![part(day, HAIKU, 3.0, H14)]);
        b.file(false, "a", vec![part(day, OPUS, 7.0, 0)]);
    }
    let w = year(&b.open("wrapped_delegator"));
    assert_eq!(w.archetype.primary, "delegator");

    let mut b = Builder(BTreeMap::new());
    for day in weekdays {
        for i in 0..10 {
            let model = if i % 2 == 0 { HAIKU } else { OPUS };
            b.file(true, "a", vec![part(day, model, 1.0, H14)]);
        }
    }
    let w = year(&b.open("wrapped_sprinter"));
    assert_eq!(w.archetype.primary, "sprinter");

    let mut b = Builder(BTreeMap::new());
    for (i, day) in weekdays.into_iter().enumerate() {
        let model = if i % 2 == 0 { HAIKU } else { OPUS };
        b.file(true, "a", vec![part(day, model, 1.0, H14)]);
    }
    let w = year(&b.open("wrapped_steady"));
    assert_eq!(w.archetype.primary, "steady-hand");
    assert!(w.archetype.traits.is_empty());
}

#[test]
fn longest_values_stay_inside_the_card() {
    let mut b = Builder(BTreeMap::new());
    for day in 2..=31 {
        b.file(true, "a", vec![part(day, OPUS, 1.0, H14)]);
    }
    let mut w = year(&b.open("wrapped_layout"));
    let share = |n: &str, s| tokenwhy_wrapped::Share {
        name: n.into(),
        share: s,
    };
    w.tokens_total = 999_400_000_000_000;
    w.words = 999_000_000_000;
    w.cost_usd = 999_400_000.0;
    w.cost_partial = true;
    w.cost_peak = Some((d(30), 99_940_000.0));
    w.longest_streak = Some((365, NaiveDate::from_ymd_opt(2026, 9, 30).unwrap()));
    w.current_streak = Some(365);
    w.active_days = 366;
    w.hours = [999; 24];
    w.busiest_weekday = Some(2);
    w.langs = vec![
        share("Minecraft data", 0.24),
        share("JavaScript", 0.2),
        share("TypeScript", 0.2),
        share("Markdown", 0.26),
        share("Shaders", 0.1),
    ];
    w.lang_edits = 99_999;
    w.langs_since = Some(NaiveDate::from_ymd_opt(2026, 9, 30).unwrap());
    w.models = vec![share("sonnet 3.5", 0.5), share("mythos 5.1", 0.5)];
    w.marathon = Some(tokenwhy_wrapped::Marathon {
        minutes: 23 * 60 + 59,
        turns: Some(99_999),
        day: NaiveDate::from_ymd_opt(2026, 9, 30).unwrap(),
        compactions: 999,
    });
    w.habit_saved_usd = Some(999_400_000.0);
    w.cost_per_edit_drop = Some(0.99);
    w.co2_g = Some(9e8);
    w.co2_coverage = 0.99;
    w.project_count = 999;
    w.projects = vec![
        share("project1", 0.4),
        share("project2", 0.3),
        share("project3", 0.3),
    ];
    w.archetype.primary = "big-model-energy".into();
    w.archetype.traits = vec!["cache-whisperer".into(), "weekend-builder".into()];
    let all = cards::cards(&w);
    assert_eq!(all.len(), 12);
    for pal in [svg::Palette::light(), svg::Palette::dark()] {
        for (i, c) in all.iter().enumerate() {
            let s = svg::render(c, i + 1, all.len(), &pal, None);
            let boxes = tokenwhy_card::text_boxes(&s, &tokenwhy_wrapped::FONTS).unwrap();
            for (id, [x0, y0, x1, y1]) in &boxes {
                assert!(
                    *x0 >= 96.0 && *x1 <= 984.0 && *y0 >= 96.0 && *y1 <= 1302.0,
                    "{} {id}: {x0} {y0} {x1} {y1}",
                    c.id
                );
            }
            for (i, (a, ab)) in boxes.iter().enumerate() {
                for (b, bb) in &boxes[i + 1..] {
                    let apart =
                        ab[2] <= bb[0] || bb[2] <= ab[0] || ab[3] <= bb[1] || bb[3] <= ab[1];
                    assert!(apart, "{}: {a} overlaps {b}", c.id);
                }
            }
        }
    }
}

fn foot(w: &tokenwhy_wrapped::Wrapped, id: &str) -> String {
    cards::cards(w)
        .into_iter()
        .find(|c| c.id == id)
        .map(|c| c.foot)
        .unwrap_or_default()
}

#[test]
fn foot_quips_follow_the_data() {
    let lang = |n: &str, s| tokenwhy_wrapped::Share {
        name: n.into(),
        share: s,
    };

    let mut b = Builder(BTreeMap::new());
    for day in 2..=31 {
        b.file(
            true,
            "a",
            vec![part(
                day,
                OPUS,
                1.0,
                if day % 4 == 0 { H14 } else { 1 << 23 },
            )],
        );
    }
    let mut w = year(&b.open("wrapped_quips_night"));
    assert_eq!(w.peak_hour, Some(23));
    w.langs = vec![lang("Rust", 0.7), lang("Python", 0.3)];
    w.lang_edits = 100;
    w.longest_streak = Some((21, d(2)));
    assert_eq!(
        foot(&w, "clock"),
        "Past midnight is prime time, apparently."
    );
    assert_eq!(foot(&w, "languages"), "You really like Rust, huh?");
    assert_eq!(foot(&w, "streaks"), "Two weeks straight. Respect.");

    let mut b = Builder(BTreeMap::new());
    for day in [2, 3, 4, 5, 6, 9, 10, 11] {
        b.file(true, "a", vec![part(day, OPUS, 1.0, H14)]);
    }
    let mut w = year(&b.open("wrapped_quips_day"));
    w.habit_saved_usd = Some(w.cache_saved_usd + 50.0);
    w.cost_per_edit_drop = Some(0.4);
    assert_eq!(foot(&w, "clock"), "");
    assert_eq!(foot(&w, "saving"), "Practice pays. Nicely done.");

    w.longest_streak = Some((5, d(2)));
    w.current_streak = Some(3);
    assert_eq!(foot(&w, "streaks"), "And 3 days into a new one.");
}
