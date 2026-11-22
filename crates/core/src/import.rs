// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Reads only the file it is given; never runs ccusage.

use crate::model::TimestampMs;
use chrono::{Local, NaiveDate, TimeZone};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub const PREFIX: &str = "import:";
const MAGIC: [u8; 8] = *b"TWIMPT01";
const VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportRow {
    pub source: u64,
    pub agent: String,
    pub day_ms: TimestampMs,
    pub model: String,
    pub input: u64,
    pub output: u64,
    pub cache_write: u64,
    pub cache_read: u64,
    pub cost_usd: Option<f64>,
}

pub fn native_of(agent: &str) -> Option<&'static str> {
    match agent {
        "claude" | "claude-code" => Some(crate::providers::claude::ID),
        "codex" => Some("codex"),
        "opencode" => Some("opencode"),
        "antigravity" | "agy" => Some("agy"),
        _ => None,
    }
}

pub fn label_of(agent: &str) -> String {
    let mut c = agent.chars();
    let name: String = c
        .next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default();
    format!("{name} (ccusage import)")
}

fn n(v: &Value, k: &str) -> u64 {
    v.get(k).and_then(Value::as_u64).unwrap_or(0)
}

fn local_day(date: &str) -> Option<TimestampMs> {
    let d = NaiveDate::parse_from_str(date.get(..10)?, "%Y-%m-%d").ok()?;
    Local
        .from_local_datetime(&d.and_hms_opt(0, 0, 0)?)
        .earliest()
        .map(|t| t.timestamp_millis())
}

/// A row without `agent` refuses the whole file: an unlabelled row cannot be kept apart from native data.
pub fn parse(text: &str, source: u64) -> Result<Vec<ImportRow>, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("not a JSON file: {e}"))?;
    let (rows, date_key) = if let Some(d) = v.get("daily").and_then(Value::as_array) {
        (d, "date")
    } else if let Some(s) = ["session", "sessions"]
        .iter()
        .find_map(|k| v.get(*k).and_then(Value::as_array))
    {
        (s, "lastActivity")
    } else {
        return Err(
            "no `daily` or `session` rows; export with `ccusage daily --by-agent --json`".into(),
        );
    };
    let mut out = vec![];
    for r in rows {
        let agent = r
            .get("agent")
            .and_then(Value::as_str)
            .filter(|a| !a.is_empty())
            .ok_or("a row has no `agent`; export with `ccusage daily --by-agent --json`")?;
        let date = r.get(date_key).and_then(Value::as_str).unwrap_or("");
        let day_ms = local_day(date).ok_or_else(|| format!("bad date '{date}'"))?;
        let mut push = |model: &str, m: &Value, cost: Option<f64>| {
            out.push(ImportRow {
                source,
                agent: agent.to_string(),
                day_ms,
                model: model.to_string(),
                input: n(m, "inputTokens"),
                output: n(m, "outputTokens"),
                cache_write: n(m, "cacheCreationTokens"),
                cache_read: n(m, "cacheReadTokens"),
                cost_usd: cost,
            })
        };
        match r.get("modelBreakdowns").and_then(Value::as_array) {
            Some(b) if !b.is_empty() => {
                for m in b {
                    let name = m
                        .get("modelName")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown");
                    push(name, m, m.get("cost").and_then(Value::as_f64));
                }
            }
            _ => {
                let name = r
                    .pointer("/modelsUsed/0")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                push(name, r, r.get("totalCost").and_then(Value::as_f64));
            }
        }
    }
    Ok(out)
}

#[derive(Serialize, Deserialize)]
struct Disk {
    magic: [u8; 8],
    version: u32,
    rows: Vec<ImportRow>,
}

#[derive(Debug, Default)]
pub struct Imports {
    path: Option<PathBuf>,
    rows: Vec<ImportRow>,
}

impl Imports {
    pub fn open(dir: Option<&Path>) -> Imports {
        let mut path = dir.map(|d| d.join("imports.bin"));
        let Some(bytes) = path.as_ref().and_then(|p| std::fs::read(p).ok()) else {
            return Imports { path, rows: vec![] };
        };
        let head = postcard::take_from_bytes::<([u8; 8], u32)>(&bytes)
            .ok()
            .map(|(h, _)| h);
        let rows = match head {
            Some((m, VERSION)) if m == MAGIC => {
                postcard::from_bytes::<Disk>(&bytes).ok().map(|d| d.rows)
            }
            Some((m, v)) if m == MAGIC && v > VERSION => {
                path = None;
                Some(vec![])
            }
            _ => None,
        };
        let rows = rows.unwrap_or_else(|| {
            if let Some(p) = &path {
                if crate::store::set_aside(p, ".bad").is_err() {
                    path = None;
                }
            }
            vec![]
        });
        Imports { path, rows }
    }

    pub fn rows(&self) -> &[ImportRow] {
        &self.rows
    }

    /// Rows of an earlier import of the same file, or of an older export, are replaced per agent and day, so importing twice never adds up.
    pub fn import_file(&mut self, file: &Path) -> Result<usize, String> {
        let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
        let source = crate::providers::common::fnv(file.to_string_lossy().as_bytes());
        let new = parse(&text, source)?;
        self.replace(source, new);
        self.save().map_err(|e| e.to_string())?;
        Ok(self.rows.iter().filter(|r| r.source == source).count())
    }

    pub fn replace(&mut self, source: u64, new: Vec<ImportRow>) {
        self.rows.retain(|r| {
            r.source != source
                && !new
                    .iter()
                    .any(|x| x.agent == r.agent && x.day_ms == r.day_ms)
        });
        self.rows.extend(new);
    }

    fn save(&self) -> std::io::Result<()> {
        let Some(p) = &self.path else {
            return Ok(());
        };
        let bytes = postcard::to_allocvec(&Disk {
            magic: MAGIC,
            version: VERSION,
            rows: self.rows.clone(),
        })
        .map_err(std::io::Error::other)?;
        let Some(_lock) = p.parent().and_then(crate::store::Lock::try_acquire) else {
            return Err(std::io::Error::other(
                "store in use by another tokenwhy process",
            ));
        };
        crate::store::write_atomic(p, &bytes)
    }
}
