// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Read from the history, so it reaches past the transcript purge.

use chrono::{Datelike, Days, Months, NaiveDate, TimeZone};
use serde::{Deserialize, Serialize};

use crate::model::{DailyRow, TimestampMs};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scale {
    Day,
    Week,
    Month,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bucket {
    pub start_ms: TimestampMs,
    pub cost: Option<f64>,
    pub tokens: u64,
    pub partial: bool,
}

fn date_of<Tz: TimeZone>(ms: TimestampMs, tz: &Tz) -> NaiveDate {
    tz.timestamp_millis_opt(ms).earliest().map_or_else(
        || {
            chrono::DateTime::from_timestamp_millis(ms)
                .unwrap_or_default()
                .date_naive()
        },
        |d| d.date_naive(),
    )
}

fn midnight<Tz: TimeZone>(d: NaiveDate, tz: &Tz) -> TimestampMs {
    let n = d.and_hms_opt(0, 0, 0).unwrap_or_default();
    tz.from_local_datetime(&n)
        .earliest()
        .map_or_else(|| n.and_utc().timestamp_millis(), |t| t.timestamp_millis())
}

fn start_of(d: NaiveDate, scale: Scale) -> NaiveDate {
    match scale {
        Scale::Day => d,
        Scale::Week => d - Days::new(u64::from(d.weekday().num_days_from_monday())),
        Scale::Month => d.with_day(1).unwrap_or(d),
    }
}

fn back(d: NaiveDate, scale: Scale, n: u32) -> NaiveDate {
    match scale {
        Scale::Day => d - Days::new(n.into()),
        Scale::Week => d - Days::new(7 * u64::from(n)),
        Scale::Month => d - Months::new(n),
    }
}

pub fn buckets<Tz: TimeZone>(
    rows: &[DailyRow],
    scale: Scale,
    end_ms: TimestampMs,
    count: u32,
    now_ms: TimestampMs,
    tz: &Tz,
) -> Vec<Bucket> {
    let count = count.clamp(1, 400);
    let last = start_of(date_of(end_ms, tz), scale);
    let starts: Vec<NaiveDate> = (0..count).rev().map(|i| back(last, scale, i)).collect();
    let now = start_of(date_of(now_ms, tz), scale);
    let mut out: Vec<Bucket> = starts
        .iter()
        .map(|&d| Bucket {
            start_ms: midnight(d, tz),
            cost: None,
            tokens: 0,
            partial: d == now,
        })
        .collect();
    for r in rows {
        let k = start_of(date_of(r.day_ms, tz), scale);
        let Ok(i) = starts.binary_search(&k) else {
            continue;
        };
        let b = &mut out[i];
        if let Some(c) = r.cost_usd.value {
            b.cost = Some(b.cost.unwrap_or(0.0) + c);
        }
        b.tokens += r.fresh_tokens + r.cache_read_tokens + r.cache_write_tokens;
    }
    out
}

pub fn span<Tz: TimeZone>(
    scale: Scale,
    end_ms: TimestampMs,
    count: u32,
    tz: &Tz,
) -> (TimestampMs, TimestampMs) {
    let last = start_of(date_of(end_ms, tz), scale);
    let first = back(last, scale, count.clamp(1, 400) - 1);
    let next = match scale {
        Scale::Day => last + Days::new(1),
        Scale::Week => last + Days::new(7),
        Scale::Month => last + Months::new(1),
    };
    (midnight(first, tz), midnight(next, tz))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Amount;
    use chrono::Utc;

    fn day(y: i32, m: u32, d: u32, cost: f64, tokens: u64) -> DailyRow {
        let ms = midnight(NaiveDate::from_ymd_opt(y, m, d).unwrap(), &Utc);
        DailyRow {
            provider: None,
            day_ms: ms,
            project: None,
            model: None,
            fresh_tokens: tokens,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            cost_usd: Amount {
                value: Some(cost),
                est: false,
            },
            sessions: 1,
            agents: 0,
            compactions: 0,
            output_tokens: None,
            edits: None,
            active_hours: None,
            agent_cost_usd: 0.0,
            co2_g: None,
            co2_g_max: None,
            co2_cache_g: None,
            water_l: None,
            water_l_max: None,
        }
    }

    fn at(y: i32, m: u32, d: u32) -> TimestampMs {
        midnight(NaiveDate::from_ymd_opt(y, m, d).unwrap(), &Utc)
    }

    // ISO 8601 calendar: 2025-12-29 is the Monday of 2026-W01, 2025-12-28 the Sunday of 2025-W52.
    fn rows() -> Vec<DailyRow> {
        vec![
            day(2025, 12, 28, 1.0, 10),
            day(2025, 12, 29, 2.0, 20),
            day(2025, 12, 31, 3.0, 30),
            day(2026, 1, 1, 4.0, 40),
            day(2026, 1, 4, 5.0, 50),
            day(2026, 1, 5, 6.0, 60),
        ]
    }

    #[test]
    fn weeks_follow_iso_across_the_year() {
        let now = at(2026, 1, 6) + 3_600_000;
        let b = buckets(&rows(), Scale::Week, now, 3, now, &Utc);
        let got: Vec<_> = b
            .iter()
            .map(|b| (b.start_ms, b.cost, b.tokens, b.partial))
            .collect();
        assert_eq!(
            got,
            vec![
                (at(2025, 12, 22), Some(1.0), 10, false),
                (at(2025, 12, 29), Some(14.0), 140, false),
                (at(2026, 1, 5), Some(6.0), 60, true),
            ]
        );
    }

    #[test]
    fn months_split_at_the_calendar_boundary() {
        let now = at(2026, 1, 6);
        let b = buckets(&rows(), Scale::Month, now, 3, now, &Utc);
        let got: Vec<_> = b
            .iter()
            .map(|b| (b.start_ms, b.cost, b.tokens, b.partial))
            .collect();
        assert_eq!(
            got,
            vec![
                (at(2025, 11, 1), None, 0, false),
                (at(2025, 12, 1), Some(6.0), 60, false),
                (at(2026, 1, 1), Some(15.0), 150, true),
            ]
        );
        let b = buckets(&rows(), Scale::Month, at(2025, 12, 15), 1, now, &Utc);
        assert_eq!(
            (b[0].start_ms, b[0].cost, b[0].partial),
            (at(2025, 12, 1), Some(6.0), false)
        );
    }
}
