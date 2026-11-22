// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone, Weekday};
use tokenwhy_core::TimestampMs;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Week {
    monday: NaiveDate,
}

impl Week {
    pub fn last_complete(now_ms: TimestampMs) -> Week {
        let today = date_of(now_ms);
        let this_monday = today - Duration::days(i64::from(today.weekday().num_days_from_monday()));
        Week {
            monday: this_monday - Duration::days(7),
        }
    }

    pub fn parse(s: &str) -> Option<Week> {
        let (y, w) = s.split_once("-W")?;
        if w.len() != 2 || y.len() != 4 {
            return None;
        }
        let monday = NaiveDate::from_isoywd_opt(y.parse().ok()?, w.parse().ok()?, Weekday::Mon)?;
        Some(Week { monday })
    }

    pub fn label(self) -> String {
        let iw = self.monday.iso_week();
        format!("{:04}-W{:02}", iw.year(), iw.week())
    }

    pub fn back(self, weeks: i64) -> Week {
        Week {
            monday: self.monday - Duration::days(7 * weeks),
        }
    }

    pub fn from_ms(self) -> TimestampMs {
        midnight(self.monday)
    }

    pub fn to_ms(self) -> TimestampMs {
        midnight(self.monday + Duration::days(7))
    }
}

fn date_of(ms: TimestampMs) -> NaiveDate {
    Local.timestamp_millis_opt(ms).earliest().map_or_else(
        || NaiveDate::from_ymd_opt(1970, 1, 1).unwrap(),
        |d| d.date_naive(),
    )
}

fn midnight(d: NaiveDate) -> TimestampMs {
    let n = d.and_hms_opt(0, 0, 0).unwrap_or_default();
    Local
        .from_local_datetime(&n)
        .earliest()
        .map_or_else(|| n.and_utc().timestamp_millis(), |t| t.timestamp_millis())
}
