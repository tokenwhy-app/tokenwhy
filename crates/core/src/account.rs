// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Transcripts carry no account id, so attribution is best effort. Only hashes and labels are stored, never an email or a name.

use crate::model::{ProviderId, TimestampMs};
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub const UNATTRIBUTED: &str = "unattributed";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedRoot {
    pub provider: ProviderId,
    pub roots: Vec<PathBuf>,
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedIn {
    pub key: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Switch {
    pub at_ms: TimestampMs,
    pub provider: String,
    pub id: String,
    pub label: String,
}

#[derive(Debug, Default, Clone)]
pub struct Accounts {
    pub named: Vec<NamedRoot>,
    pub switches: Vec<Switch>,
    pub labels: BTreeMap<String, String>,
}

pub fn fnv(s: &[u8]) -> u64 {
    s.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3)
    })
}

pub fn root_id(dir: &Path) -> String {
    let norm = dir
        .to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_lowercase();
    format!("root:{:016x}", fnv(norm.as_bytes()))
}

pub fn signed_in_id(provider: &str, key: &str) -> String {
    format!(
        "{provider}:{:016x}",
        fnv(format!("{provider}\0{key}").as_bytes())
    )
}

impl Accounts {
    pub fn of(
        &self,
        provider: &ProviderId,
        path: &Path,
        start: Option<TimestampMs>,
    ) -> Option<&str> {
        if let Some(n) = self
            .named
            .iter()
            .find(|n| &n.provider == provider && n.roots.iter().any(|r| path.starts_with(r)))
        {
            return Some(&n.id);
        }
        let start = start?;
        self.switches
            .iter()
            .rfind(|s| s.provider == provider.0 && s.at_ms <= start)
            .map(|s| s.id.as_str())
    }

    pub fn label(&self, id: &str) -> String {
        if let Some(l) = self.labels.get(id).filter(|l| !l.trim().is_empty()) {
            return l.clone();
        }
        if let Some(n) = self.named.iter().find(|n| n.id == id) {
            return n.label.clone();
        }
        if id == UNATTRIBUTED {
            return "Unattributed".into();
        }
        self.switches
            .iter()
            .rev()
            .find(|s| s.id == id)
            .map_or_else(|| id.to_string(), |s| s.label.clone())
    }

    pub fn is_named(&self, path: &Path) -> bool {
        self.named
            .iter()
            .any(|n| n.roots.iter().any(|r| path.starts_with(r)))
    }

    pub fn current(&self, provider: &str) -> Option<&Switch> {
        self.switches.iter().rev().find(|s| s.provider == provider)
    }

    pub fn next_label(&self) -> String {
        let mut ids: Vec<&str> = self.switches.iter().map(|s| s.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        format!("Account {}", ids.len() + 1)
    }
}

pub struct Log {
    path: Option<PathBuf>,
}

impl Log {
    pub fn open(dir: Option<&Path>) -> Log {
        Log {
            path: dir.map(|d| d.join("accounts.log")),
        }
    }

    pub fn read(&self) -> Vec<Switch> {
        let Some(text) = self
            .path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
        else {
            return vec![];
        };
        let mut v: Vec<Switch> = text
            .lines()
            .filter_map(|l| {
                let mut c = l.splitn(4, '\t');
                Some(Switch {
                    at_ms: c.next()?.parse().ok()?,
                    provider: c.next()?.to_string(),
                    id: c.next()?.to_string(),
                    label: c.next().unwrap_or("").to_string(),
                })
            })
            .collect();
        v.sort_by_key(|s| s.at_ms);
        v
    }

    pub fn append(&self, s: &Switch) -> io::Result<()> {
        let Some(p) = &self.path else { return Ok(()) };
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d)?;
        }
        let clean = |x: &str| x.replace(['\t', '\r', '\n'], " ");
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(p)?;
        writeln!(
            f,
            "{}\t{}\t{}\t{}",
            s.at_ms,
            clean(&s.provider),
            clean(&s.id),
            clean(&s.label)
        )
    }
}
