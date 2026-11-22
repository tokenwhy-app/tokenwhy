// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokenwhy_core::TimestampMs;

use crate::week::Week;

/// Placeholder until the domain is chosen; `.invalid` never resolves, so every send fails as a
/// network error and waits in the outbox.
pub const BASE_URL: &str = "https://share.invalid";

const HOUR_MS: i64 = 3_600_000;
const OUTBOX_MAX: usize = 4;
/// The server refuses weeks older than 5; the outbox keeps 4.
const MAX_AGE_WEEKS: i64 = 4;
const BODY_MAX: usize = 8 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub enabled: bool,
    pub install: Option<String>,
    pub offset_h: u8,
    pub last_sent_week: Option<String>,
    pub last_status: Option<String>,
    pub consent_ms: Option<TimestampMs>,
    pub consent_version: Option<String>,
    pub prompted: bool,
    pub retired: bool,
    pub pending_delete: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pending {
    pub week: String,
    pub body: String,
    pub attempts: u32,
    pub next_ms: TimestampMs,
}

pub struct Store {
    dir: PathBuf,
}

fn read_json<T: for<'de> Deserialize<'de> + Default>(p: &Path) -> T {
    std::fs::read(p)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn write_json<T: Serialize>(p: &Path, v: &T) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(v).map_err(|e| e.to_string())?;
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    tokenwhy_core::store::write_atomic(p, &bytes).map_err(|e| e.to_string())
}

impl Store {
    pub fn new(dir: PathBuf) -> Store {
        Store { dir }
    }

    pub fn state(&self) -> State {
        read_json(&self.dir.join("share.json"))
    }

    pub fn save_state(&self, s: &State) -> Result<(), String> {
        write_json(&self.dir.join("share.json"), s)
    }

    pub fn outbox(&self) -> Vec<Pending> {
        read_json(&self.dir.join("outbox.json"))
    }

    fn save_outbox(&self, o: &[Pending]) -> Result<(), String> {
        write_json(&self.dir.join("outbox.json"), &o)
    }

    fn log(&self, now_ms: TimestampMs, path: &str, body: &str, status: &str) {
        let line =
            serde_json::json!({ "ms": now_ms, "path": path, "status": status, "body": body });
        let _ = std::fs::create_dir_all(&self.dir);
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir.join("sent.jsonl"))
        {
            let _ = writeln!(f, "{line}");
        }
    }

    pub fn sent_log(&self) -> String {
        std::fs::read_to_string(self.dir.join("sent.jsonl")).unwrap_or_default()
    }

    pub fn enable(&self, now_ms: TimestampMs, consent_version: &str) -> Result<State, String> {
        let mut s = self.state();
        if s.install.is_none() {
            let mut b = [0u8; 17];
            getrandom::fill(&mut b).map_err(|e| e.to_string())?;
            s.install = Some(b[..16].iter().map(|x| format!("{x:02x}")).collect());
            s.offset_h = b[16] % 73;
        }
        s.enabled = true;
        s.prompted = true;
        s.retired = false;
        s.consent_ms = Some(now_ms);
        s.consent_version = Some(consent_version.to_string());
        self.save_state(&s)?;
        Ok(s)
    }

    pub fn disable(&self) -> Result<State, String> {
        let mut s = self.state();
        s.enabled = false;
        s.prompted = true;
        self.save_state(&s)?;
        self.save_outbox(&[])?;
        Ok(s)
    }

    pub fn mark_prompted(&self) -> Result<(), String> {
        let mut s = self.state();
        s.prompted = true;
        self.save_state(&s)
    }

    pub fn due(&self, week: Week, now_ms: TimestampMs) -> bool {
        let s = self.state();
        let label = week.label();
        s.enabled
            && !s.retired
            && s.install.is_some()
            && s.last_sent_week.as_deref() != Some(label.as_str())
            && !self.outbox().iter().any(|p| p.week == label)
            && now_ms >= week.to_ms() + i64::from(s.offset_h) * HOUR_MS
    }

    pub fn enqueue(&self, week: Week, body: String, now_ms: TimestampMs) -> Result<(), String> {
        let label = week.label();
        let mut o = self.outbox();
        o.retain(|p| p.week != label);
        o.push(Pending {
            week: label,
            body,
            attempts: 0,
            next_ms: now_ms,
        });
        o.sort_by(|a, b| a.week.cmp(&b.week));
        let extra = o.len().saturating_sub(OUTBOX_MAX);
        o.drain(..extra);
        self.save_outbox(&o)
    }

    pub fn next_try(&self) -> Option<TimestampMs> {
        let s = self.state();
        if s.pending_delete.is_some() {
            return Some(0);
        }
        self.outbox().iter().map(|p| p.next_ms).min()
    }

    pub fn flush(&self, now_ms: TimestampMs) -> State {
        let mut s = self.state();
        if let Some(id) = s.pending_delete.clone() {
            if self.try_delete(&mut s, &id, now_ms).is_err() {
                return s;
            }
        }
        let mut o = self.outbox();
        let oldest = Week::last_complete(now_ms).back(MAX_AGE_WEEKS).label();
        o.retain(|p| p.week >= oldest);
        if !s.enabled || s.retired {
            return s;
        }
        let mut keep = vec![];
        for mut p in o {
            if p.next_ms > now_ms {
                keep.push(p);
                continue;
            }
            let r = post("/v1/report", &p.body);
            let status = match &r {
                Ok((c, _, _)) => c.to_string(),
                Err(e) => format!("network: {e}"),
            };
            self.log(now_ms, "/v1/report", &p.body, &status);
            match r {
                Ok((200..=299, _, _)) => {
                    s.last_sent_week = Some(p.week.clone());
                    s.last_status = Some("accepted".into());
                }
                Ok((400, _, text)) => {
                    s.last_status = Some(format!("rejected {}", field_of(&text)));
                }
                Ok((410, _, _)) => {
                    s.retired = true;
                    s.last_status = Some("retired".into());
                    keep.clear();
                    break;
                }
                Ok((429, retry, _)) => {
                    let wait_s = retry.unwrap_or(0).max(3_600);
                    p.next_ms = now_ms + i64::try_from(wait_s).unwrap_or(i64::MAX / 2) * 1000;
                    s.last_status = Some("waiting".into());
                    keep.push(p);
                }
                _ => {
                    p.attempts += 1;
                    p.next_ms = now_ms + backoff(p.attempts);
                    s.last_status = Some("waiting".into());
                    keep.push(p);
                }
            }
        }
        let _ = self.save_outbox(&keep);
        let _ = self.save_state(&s);
        s
    }

    pub fn delete(&self, now_ms: TimestampMs) -> Result<u64, String> {
        let mut s = self.state();
        let Some(id) = s.install.clone().or(s.pending_delete.clone()) else {
            return Ok(0);
        };
        s.pending_delete = Some(id.clone());
        s.install = None;
        s.last_sent_week = None;
        self.save_state(&s)?;
        self.save_outbox(&[])?;
        self.try_delete(&mut s, &id, now_ms)
    }

    fn try_delete(&self, s: &mut State, id: &str, now_ms: TimestampMs) -> Result<u64, String> {
        let body = serde_json::json!({ "install": id }).to_string();
        let r = post("/v1/delete", &body);
        let status = match &r {
            Ok((c, _, _)) => c.to_string(),
            Err(e) => format!("network: {e}"),
        };
        self.log(now_ms, "/v1/delete", &body, &status);
        match r {
            Ok((200..=299, _, text)) => {
                s.pending_delete = None;
                if s.enabled {
                    let v = s.consent_version.clone().unwrap_or_default();
                    self.save_state(s)?;
                    *s = self.enable(now_ms, &v)?;
                } else {
                    self.save_state(s)?;
                }
                Ok(serde_json::from_str::<serde_json::Value>(&text)
                    .ok()
                    .and_then(|v| v["deleted"].as_u64())
                    .unwrap_or(0))
            }
            Ok((c, _, _)) => Err(format!("the server answered {c}; will retry")),
            Err(e) => Err(format!("no connection ({e}); will retry")),
        }
    }
}

fn backoff(attempts: u32) -> i64 {
    match attempts {
        0 | 1 => HOUR_MS,
        2 => 6 * HOUR_MS,
        _ => 24 * HOUR_MS,
    }
}

/// Kept only when it is one of the schema's keys, so the log holds no server text.
fn field_of(text: &str) -> String {
    let f = serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .and_then(|v| v["field"].as_str().map(str::to_string))
        .unwrap_or_default();
    if crate::payload::FIELDS.iter().any(|(k, _)| *k == f) {
        f
    } else {
        "body".into()
    }
}

fn post(path: &str, body: &str) -> Result<(u16, Option<u64>, String), String> {
    if body.len() > BODY_MAX {
        return Err("body over 8 KB".into());
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .http_status_as_error(false)
        .build()
        .into();
    let mut resp = agent
        .post(format!("{BASE_URL}{path}"))
        .header("content-type", "application/json")
        .send(body.as_bytes())
        .map_err(|e| e.to_string())?;
    let status = resp.status().as_u16();
    let retry = resp
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse().ok());
    let text = resp
        .body_mut()
        .with_config()
        .limit(64 * 1024)
        .read_to_string()
        .unwrap_or_default();
    Ok((status, retry, text))
}
