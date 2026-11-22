// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! `stats-cache.json` is undocumented (shape v5); its split by kind and its cost are estimates from the model's all-time `modelUsage` ratios.

use crate::model::{TimestampMs, Vendor};
use chrono::{Local, NaiveDate, TimeZone};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const FILE: &str = "stats-cache.json";
const VERSION: u64 = 5;

#[derive(Debug, Clone, PartialEq)]
pub struct StatsDay {
    pub day_ms: TimestampMs,
    pub model: String,
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub cost_usd: Option<f64>,
}

pub fn file_of(projects_root: &Path) -> PathBuf {
    let config = if projects_root.file_name().is_some_and(|n| n == "projects") {
        projects_root.parent().unwrap_or(projects_root)
    } else {
        projects_root
    };
    config.join(FILE)
}

fn local_day(date: &str) -> Option<TimestampMs> {
    let d = NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?;
    Local
        .from_local_datetime(&d.and_hms_opt(0, 0, 0)?)
        .earliest()
        .map(|t| t.timestamp_millis())
}

pub fn parse(text: &str) -> Result<Vec<StatsDay>, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("not JSON: {e}"))?;
    match v.get("version").and_then(Value::as_u64) {
        Some(VERSION) => {}
        other => return Err(format!("version {other:?}, expected {VERSION}")),
    }
    let usage = v
        .get("modelUsage")
        .and_then(Value::as_object)
        .ok_or("no `modelUsage` object")?;
    let mut ratio: BTreeMap<&str, [f64; 4]> = BTreeMap::new();
    for (model, u) in usage {
        let k = |key: &str| u.get(key).and_then(Value::as_u64).unwrap_or(0) as f64;
        let parts = [
            k("inputTokens"),
            k("outputTokens"),
            k("cacheReadInputTokens"),
            k("cacheCreationInputTokens"),
        ];
        let total: f64 = parts.iter().sum();
        if total > 0.0 {
            ratio.insert(model, parts.map(|p| p / total));
        }
    }
    let days = v
        .get("dailyModelTokens")
        .and_then(Value::as_array)
        .ok_or("no `dailyModelTokens` array")?;
    let table = crate::price::vendor_table(Vendor::Anthropic);
    let mut out = vec![];
    for d in days {
        let date = d
            .get("date")
            .and_then(Value::as_str)
            .ok_or("a day has no `date`")?;
        let day_ms = local_day(date).ok_or_else(|| format!("bad date '{date}'"))?;
        let by = d
            .get("tokensByModel")
            .and_then(Value::as_object)
            .ok_or("a day has no `tokensByModel`")?;
        for (model, t) in by {
            let t = t.as_u64().ok_or("a token count is not an integer")?;
            if t == 0 {
                continue;
            }
            let Some(r) = ratio.get(model.as_str()) else {
                out.push(StatsDay {
                    day_ms,
                    model: model.clone(),
                    input: t,
                    output: 0,
                    cache_read: 0,
                    cache_write: 0,
                    cost_usd: None,
                });
                continue;
            };
            let [input, output, cache_read, _] = r.map(|x| (t as f64 * x).round() as u64);
            let cache_write = t.saturating_sub(input + output + cache_read);
            let cost_usd = crate::price::rate_for(table, model).map(|rate| {
                (input as f64 * rate.input
                    + output as f64 * rate.output
                    + cache_read as f64 * rate.cache_read
                    + cache_write as f64 * rate.cache_write_5m)
                    / 1e6
            });
            out.push(StatsDay {
                day_ms,
                model: model.clone(),
                input,
                output,
                cache_read,
                cache_write,
                cost_usd,
            });
        }
    }
    Ok(out)
}
