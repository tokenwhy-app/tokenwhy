// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! The `cachedUsageUtilization` key is undocumented: each window is read on its own, and a shape it does not match leaves that window absent.

use crate::account::fnv;
use crate::model::TimestampMs;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Window {
    pub pct: f64,
    pub resets_ms: Option<TimestampMs>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    pub fetched_ms: TimestampMs,
    pub key: Option<String>,
    pub five_hour: Option<Window>,
    pub seven_day: Option<Window>,
    pub seven_day_opus: Option<Window>,
    pub seven_day_sonnet: Option<Window>,
}

#[derive(Deserialize)]
struct Config {
    #[serde(rename = "cachedUsageUtilization")]
    cached: Option<Value>,
}

fn window(v: Option<&Value>) -> Option<Window> {
    let o = v?.as_object()?;
    let pct = o.get("utilization")?.as_f64()?;
    if !pct.is_finite() || pct < 0.0 {
        return None;
    }
    let resets_ms = o
        .get("resets_at")
        .and_then(Value::as_str)
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|t| t.timestamp_millis());
    Some(Window { pct, resets_ms })
}

pub fn parse(bytes: &[u8]) -> Option<Reading> {
    let c: Config = serde_json::from_slice(bytes).ok()?;
    let c = c.cached?;
    let u = c.get("utilization")?;
    let r = Reading {
        fetched_ms: c.get("fetchedAtMs")?.as_i64()?,
        key: c
            .get("accountUuid")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(|s| format!("{:016x}", fnv(s.as_bytes()))),
        five_hour: window(u.get("five_hour")),
        seven_day: window(u.get("seven_day")),
        seven_day_opus: window(u.get("seven_day_opus")),
        seven_day_sonnet: window(u.get("seven_day_sonnet")),
    };
    (r.five_hour.is_some() || r.seven_day.is_some()).then_some(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Shape of a real cached `/usage` reading (values and uuid invented), with a null window and an unknown field.
    #[test]
    fn reads_the_cached_usage() {
        let json = br#"{"numStartups": 40, "oauthAccount": {"accountUuid": "11111111-2222-3333-4444-555555555555"},
          "cachedUsageUtilization": {"fetchedAtMs": 1761400000000, "accountUuid": "11111111-2222-3333-4444-555555555555",
            "utilization": {
              "five_hour": {"utilization": 25.0, "resets_at": "2025-10-25T17:00:00.000000+00:00"},
              "seven_day": {"utilization": 15.5, "resets_at": "2025-10-30T08:00:00+00:00"},
              "seven_day_oauth_apps": null,
              "seven_day_opus": null,
              "seven_day_sonnet": {"utilization": 3, "resets_at": null},
              "iguana_necktie": {"utilization": 99, "extra": [1, 2]}
            }}}"#;
        let r = parse(json).unwrap();
        assert_eq!(r.fetched_ms, 1_761_400_000_000);
        assert_eq!(
            r.five_hour,
            Some(Window {
                pct: 25.0,
                resets_ms: Some(1_761_411_600_000)
            })
        );
        assert_eq!(
            r.seven_day,
            Some(Window {
                pct: 15.5,
                resets_ms: Some(1_761_811_200_000)
            })
        );
        assert_eq!(r.seven_day_opus, None);
        assert_eq!(
            r.seven_day_sonnet,
            Some(Window {
                pct: 3.0,
                resets_ms: None
            })
        );
        let signed = crate::providers::claude::account::signed_in(json).unwrap();
        assert_eq!(r.key, Some(signed.key));
        assert_eq!(
            parse(br#"{"cachedUsageUtilization": {"fetchedAtMs": 1, "utilization": []}}"#),
            None
        );
    }
}
