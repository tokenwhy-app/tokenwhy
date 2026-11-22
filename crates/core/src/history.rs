// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! A file absent from the corpus keeps its last entry, so days outlive their transcripts.

use crate::metrics::days::DayPart;
use crate::metrics::FileSummary;
use crate::model::{Amount, DailyRow, ProviderId, TimestampMs, WeekDelta, WeekDeltas};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};

const MAGIC: [u8; 8] = *b"TWHIST01";
const VERSION: u32 = 6;
const V2_PROVIDER: &str = crate::providers::claude::ID;
const DAY_MS: i64 = 86_400_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entry {
    provider: String,
    session: u64,
    project: String,
    main: bool,
    floor_ms: TimestampMs,
    days: Vec<DayPart>,
    account: Option<String>,
    last_ms: Option<TimestampMs>,
    turns: Option<u32>,
}

#[derive(Deserialize)]
struct DayPartV4 {
    day_ms: TimestampMs,
    model: String,
    fresh: u64,
    cache_read: u64,
    cache_write: u64,
    cost_usd: f64,
    cost_known: bool,
    compactions: u32,
    output: Option<u64>,
    edits: Option<u32>,
    hours: Option<u32>,
}

impl From<DayPartV4> for DayPart {
    fn from(d: DayPartV4) -> DayPart {
        DayPart {
            day_ms: d.day_ms,
            model: d.model,
            fresh: d.fresh,
            cache_read: d.cache_read,
            cache_write: d.cache_write,
            cost_usd: d.cost_usd,
            cost_known: d.cost_known,
            compactions: d.compactions,
            output: d.output,
            edits: d.edits,
            hours: d.hours,
            langs: None,
        }
    }
}

fn days_v4(days: Vec<DayPartV4>) -> Vec<DayPart> {
    days.into_iter().map(DayPart::from).collect()
}

#[derive(Deserialize)]
struct EntryV4 {
    provider: String,
    session: u64,
    project: String,
    main: bool,
    floor_ms: TimestampMs,
    days: Vec<DayPartV4>,
    account: Option<String>,
}

#[derive(Deserialize)]
struct DiskV4 {
    magic: [u8; 8],
    version: u32,
    files: BTreeMap<u64, EntryV4>,
}

impl From<EntryV4> for Entry {
    fn from(e: EntryV4) -> Entry {
        Entry {
            provider: e.provider,
            session: e.session,
            project: e.project,
            main: e.main,
            floor_ms: e.floor_ms,
            days: days_v4(e.days),
            account: e.account,
            last_ms: None,
            turns: None,
        }
    }
}

#[derive(Deserialize)]
struct EntryV3 {
    provider: String,
    session: u64,
    project: String,
    main: bool,
    floor_ms: TimestampMs,
    days: Vec<DayPartV4>,
}

#[derive(Deserialize)]
struct DiskV3 {
    magic: [u8; 8],
    version: u32,
    files: BTreeMap<u64, EntryV3>,
}

impl From<EntryV3> for Entry {
    fn from(e: EntryV3) -> Entry {
        Entry {
            provider: e.provider,
            session: e.session,
            project: e.project,
            main: e.main,
            floor_ms: e.floor_ms,
            days: days_v4(e.days),
            account: None,
            last_ms: None,
            turns: None,
        }
    }
}

#[derive(Deserialize)]
struct DayPartV1 {
    day_ms: TimestampMs,
    model: String,
    fresh: u64,
    cache_read: u64,
    cache_write: u64,
    cost_usd: f64,
    cost_known: bool,
    compactions: u32,
}

#[derive(Deserialize)]
struct EntryV1 {
    session: u64,
    project: String,
    main: bool,
    floor_ms: TimestampMs,
    days: Vec<DayPartV1>,
}

#[derive(Deserialize)]
struct DiskV1 {
    magic: [u8; 8],
    version: u32,
    files: BTreeMap<u64, EntryV1>,
}

impl From<EntryV1> for Entry {
    fn from(e: EntryV1) -> Entry {
        let days = e
            .days
            .into_iter()
            .map(|d| DayPartV4 {
                day_ms: d.day_ms,
                model: d.model,
                fresh: d.fresh,
                cache_read: d.cache_read,
                cache_write: d.cache_write,
                cost_usd: d.cost_usd,
                cost_known: d.cost_known,
                compactions: d.compactions,
                output: None,
                edits: None,
                hours: None,
            })
            .collect();
        EntryV2 {
            session: e.session,
            project: e.project,
            main: e.main,
            floor_ms: e.floor_ms,
            days,
        }
        .into()
    }
}

#[derive(Deserialize)]
struct EntryV2 {
    session: u64,
    project: String,
    main: bool,
    floor_ms: TimestampMs,
    days: Vec<DayPartV4>,
}

#[derive(Deserialize)]
struct DiskV2 {
    magic: [u8; 8],
    version: u32,
    files: BTreeMap<u64, EntryV2>,
}

impl From<EntryV2> for Entry {
    fn from(e: EntryV2) -> Entry {
        Entry {
            provider: V2_PROVIDER.to_string(),
            session: e.session,
            project: e.project,
            main: e.main,
            floor_ms: e.floor_ms,
            days: days_v4(e.days),
            account: None,
            last_ms: None,
            turns: None,
        }
    }
}

fn key_of(provider: &str, path_hash: u64) -> u64 {
    path_hash ^ fnv(provider.as_bytes())
}

#[derive(Serialize, Deserialize)]
struct Disk {
    magic: [u8; 8],
    version: u32,
    files: BTreeMap<u64, Entry>,
    plan: Vec<PlanWeek>,
}

#[derive(Deserialize)]
struct DiskV5 {
    magic: [u8; 8],
    version: u32,
    files: BTreeMap<u64, Entry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanWeek {
    pub account: String,
    pub resets_ms: TimestampMs,
    pub max_pct: f64,
    pub fetched_ms: TimestampMs,
}

const PLAN_WEEKS: usize = 53;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum By {
    Project,
    Model,
    Provider,
    None,
}

#[derive(Debug, Default)]
pub struct History {
    files: BTreeMap<u64, Entry>,
    plan: Vec<PlanWeek>,
    path: Option<PathBuf>,
    imported: Vec<Entry>,
    ccusage: Vec<(Option<&'static str>, Entry)>,
    claude_stats: Vec<Entry>,
    newer: Option<u32>,
    #[cfg(feature = "co2")]
    co2: std::sync::RwLock<crate::co2::Co2Settings>,
}

fn fnv(s: &[u8]) -> u64 {
    s.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3)
    })
}

pub fn claude_carbon_tsv() -> Option<PathBuf> {
    let var = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let dir = if let Some(db) = var("CLAUDE_CARBON_DB") {
        db.parent()?.to_path_buf()
    } else if let Some(cfg) = var("CLAUDE_CONFIG_DIR") {
        cfg.join("claude-carbon")
    } else {
        var("USERPROFILE")
            .or_else(|| var("HOME"))?
            .join(".claude")
            .join("claude-carbon")
    };
    Some(dir.join("sessions-history.tsv"))
}

impl History {
    #[cfg(feature = "co2")]
    pub fn set_co2(&self, cfg: &crate::co2::Co2Settings) {
        let mut w = self.co2.write().unwrap_or_else(|e| e.into_inner());
        if *w != *cfg {
            *w = cfg.clone();
        }
    }

    #[cfg(feature = "co2")]
    fn co2_of(&self, d: &DayPart) -> Option<[f64; 5]> {
        let out = d.output?;
        let input = d.fresh.saturating_sub(d.cache_write + out);
        let cfg = self.co2.read().unwrap_or_else(|e| e.into_inner());
        let Some(b) = crate::co2::band(&cfg, &d.model) else {
            return Some([0.0; 5]);
        };
        let g = |r| crate::co2::grams_of(r, input, d.cache_write, d.cache_read, out);
        let (lo, hi) = (g(&b.lo), g(&b.hi));
        let (wl, wh) = b.water.unwrap_or_default();
        let m = out as f64 / 1e6;
        Some([
            lo.iter().sum(),
            hi.iter().sum(),
            (lo[2] + hi[2]) / 2.0,
            wl * m,
            wh * m,
        ])
    }

    #[cfg(feature = "co2")]
    fn has_water(&self) -> bool {
        self.co2.read().unwrap_or_else(|e| e.into_inner()).method == crate::co2::Method::Ecologits
    }

    #[cfg(not(feature = "co2"))]
    fn has_water(&self) -> bool {
        false
    }

    #[cfg(feature = "co2")]
    pub fn co2_method(&self) -> Option<&'static str> {
        Some(
            self.co2
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .method
                .id(),
        )
    }

    #[cfg(not(feature = "co2"))]
    pub fn co2_method(&self) -> Option<&'static str> {
        None
    }

    #[cfg(not(feature = "co2"))]
    fn co2_of(&self, _: &DayPart) -> Option<[f64; 5]> {
        None
    }

    /// Versions 1 to 5 migrate (the old file kept as `history.bin.v<N>`); any other version is set aside, never deleted.
    pub fn open(dir: Option<&Path>) -> History {
        let Some(d) = dir else {
            return History::default();
        };
        let path = d.join("history.bin");
        let mut h = History {
            files: BTreeMap::new(),
            plan: vec![],
            path: Some(path.clone()),
            imported: vec![],
            ccusage: vec![],
            claude_stats: vec![],
            newer: None,
            #[cfg(feature = "co2")]
            co2: Default::default(),
        };
        let Ok(bytes) = std::fs::read(&path) else {
            return h;
        };
        let head = postcard::take_from_bytes::<([u8; 8], u32)>(&bytes)
            .ok()
            .map(|(h, _)| h);
        match head {
            Some((m, VERSION)) if m == MAGIC => match postcard::from_bytes::<Disk>(&bytes) {
                Ok(disk) => {
                    h.files = disk.files;
                    h.plan = disk.plan;
                }
                Err(_) => h.set_aside(VERSION),
            },
            Some((m, 5)) if m == MAGIC => match postcard::from_bytes::<DiskV5>(&bytes) {
                Ok(disk) if disk.version == 5 && disk.magic == MAGIC => {
                    h.files = disk.files;
                    h.keep_old(5);
                }
                _ => h.set_aside(5),
            },
            Some((m, 4)) if m == MAGIC => match postcard::from_bytes::<DiskV4>(&bytes) {
                Ok(disk) if disk.version == 4 && disk.magic == MAGIC => {
                    h.files = disk.files.into_iter().map(|(k, e)| (k, e.into())).collect();
                    h.keep_old(4);
                }
                _ => h.set_aside(4),
            },
            Some((m, 3)) if m == MAGIC => match postcard::from_bytes::<DiskV3>(&bytes) {
                Ok(disk) if disk.version == 3 && disk.magic == MAGIC => {
                    h.files = disk.files.into_iter().map(|(k, e)| (k, e.into())).collect();
                    h.keep_old(3);
                }
                _ => h.set_aside(3),
            },
            Some((m, 2)) if m == MAGIC => match postcard::from_bytes::<DiskV2>(&bytes) {
                Ok(disk) if disk.version == 2 && disk.magic == MAGIC => {
                    h.files = migrate(disk.files);
                    h.keep_old(2);
                }
                _ => h.set_aside(2),
            },
            Some((m, 1)) if m == MAGIC => match postcard::from_bytes::<DiskV1>(&bytes) {
                Ok(disk) if disk.version == 1 && disk.magic == MAGIC => {
                    h.files = migrate(disk.files);
                    h.keep_old(1);
                }
                _ => h.set_aside(1),
            },
            Some((m, v)) if m == MAGIC && v > VERSION => {
                h.path = None;
                h.newer = Some(v);
            }
            other => h.set_aside(other.map_or(0, |(_, v)| v)),
        }
        h
    }

    /// If the copy fails, nothing is written this run, so the old file stays the only one on disk.
    fn keep_old(&mut self, v: u32) {
        let Some(path) = self.path.clone() else {
            return;
        };
        if crate::store::copy_aside(&path, &format!(".v{v}")).is_err() || self.save().is_err() {
            self.path = None;
        }
    }

    fn set_aside(&mut self, v: u32) {
        if let Some(path) = &self.path {
            if crate::store::set_aside(path, &format!(".v{v}")).is_err() {
                self.path = None;
            }
        }
    }

    pub fn record_plan(
        &mut self,
        account: &str,
        resets_ms: TimestampMs,
        pct: f64,
        fetched_ms: TimestampMs,
    ) {
        if let Some(w) = self
            .plan
            .iter_mut()
            .find(|w| w.account == account && w.resets_ms == resets_ms)
        {
            if pct > w.max_pct {
                w.max_pct = pct;
            }
            w.fetched_ms = w.fetched_ms.max(fetched_ms);
            return;
        }
        self.plan.push(PlanWeek {
            account: account.to_string(),
            resets_ms,
            max_pct: pct,
            fetched_ms,
        });
        self.plan
            .sort_by(|a, b| (&a.account, a.resets_ms).cmp(&(&b.account, b.resets_ms)));
        let n = self.plan.iter().filter(|w| w.account == account).count();
        if n > PLAN_WEEKS {
            let first = self.plan.iter().position(|w| w.account == account);
            if let Some(i) = first {
                self.plan.remove(i);
            }
        }
    }

    pub fn plan_weeks(&self) -> &[PlanWeek] {
        &self.plan
    }

    pub fn newer(&self) -> Option<u32> {
        self.newer
    }

    pub fn absorb<'a>(
        &mut self,
        present: impl Iterator<Item = (&'a FileSummary, Option<&'a str>)>,
    ) {
        for (f, account) in present {
            let key = key_of(&f.provider.0, fnv(f.path.to_string_lossy().as_bytes()));
            let account = account.map(str::to_string);
            let (last_ms, turns) = (f.last_ts, Some(f.turns));
            if self.files.get(&key).is_some_and(|e| {
                e.days == f.days
                    && e.project == f.project
                    && e.account == account
                    && e.last_ms == last_ms
                    && e.turns == turns
            }) {
                continue;
            }
            let first = f.first_ts.unwrap_or(TimestampMs::MAX);
            let entry = Entry {
                provider: f.provider.0.clone(),
                session: fnv(format!("{}\0{}", f.provider.0, f.session).as_bytes()),
                project: f.project.clone(),
                main: f.main,
                floor_ms: first,
                days: f.days.clone(),
                account,
                last_ms,
                turns,
            };
            match self.files.get_mut(&key) {
                Some(old) if first < old.floor_ms => {
                    let floor_day = crate::metrics::local_midnight(old.floor_ms);
                    old.days.retain(|d| d.day_ms <= floor_day);
                    old.days
                        .extend(entry.days.into_iter().filter(|d| d.day_ms > floor_day));
                    old.project = entry.project;
                    old.account = entry.account;
                    old.last_ms = entry.last_ms;
                    old.turns = entry.turns;
                }
                Some(old) => {
                    let floor = old.floor_ms;
                    *old = entry;
                    old.floor_ms = floor;
                }
                None if entry.days.is_empty() => {}
                None => {
                    self.files.insert(key, entry);
                }
            }
        }
    }

    pub fn save(&self) -> io::Result<()> {
        let Some(p) = &self.path else { return Ok(()) };
        #[derive(Serialize)]
        struct Out<'a> {
            magic: [u8; 8],
            version: u32,
            files: &'a BTreeMap<u64, Entry>,
            plan: &'a [PlanWeek],
        }
        let bytes = postcard::to_allocvec(&Out {
            magic: MAGIC,
            version: VERSION,
            files: &self.files,
            plan: &self.plan,
        })
        .map_err(io::Error::other)?;
        crate::store::write_atomic(p, &bytes)
    }

    pub fn daily(
        &self,
        from: TimestampMs,
        to: TimestampMs,
        by: By,
        project: Option<&str>,
        provider: Option<&str>,
    ) -> Vec<DailyRow> {
        self.daily_for(from, to, by, project, provider, None)
    }

    pub fn daily_for(
        &self,
        from: TimestampMs,
        to: TimestampMs,
        by: By,
        project: Option<&str>,
        provider: Option<&str>,
        account: Option<&str>,
    ) -> Vec<DailyRow> {
        #[derive(Default)]
        struct Acc {
            fresh: u64,
            read: u64,
            write: u64,
            cost: f64,
            known: bool,
            sessions: BTreeSet<u64>,
            agents: u32,
            compactions: u32,
            output: Option<u64>,
            edits: Option<u32>,
            hours: Option<u32>,
            agent_cost: f64,
            mains: u32,
            co2: Option<[f64; 5]>,
            parts: u32,
            est: bool,
        }
        let mut acc: BTreeMap<(TimestampMs, String), Acc> = BTreeMap::new();
        let own_first = self
            .files
            .values()
            .flat_map(|e| e.days.iter().map(|d| d.day_ms))
            .min()
            .unwrap_or(TimestampMs::MAX);
        let gaps = self.ccusage_gaps();
        let stats = self.claude_stats_gaps();
        let entries = self
            .files
            .values()
            .map(|e| (e, to, false))
            .chain(self.imported.iter().map(|e| (e, to.min(own_first), false)))
            .chain(gaps.iter().map(|e| (e, to, false)))
            .chain(stats.iter().map(|e| (e, to, true)));
        for (e, to, est) in entries.filter(|(e, _, _)| {
            project.is_none_or(|p| e.project == p)
                && provider.is_none_or(|p| e.provider == p)
                && account.is_none_or(|a| {
                    e.account.as_deref().unwrap_or(crate::account::UNATTRIBUTED) == a
                })
        }) {
            let mut agent_days: BTreeSet<(TimestampMs, String)> = BTreeSet::new();
            for d in e.days.iter().filter(|d| d.day_ms >= from && d.day_ms < to) {
                let k = match by {
                    By::Project => e.project.clone(),
                    By::Model => d.model.clone(),
                    By::Provider => e.provider.clone(),
                    By::None => String::new(),
                };
                let a = acc.entry((d.day_ms, k.clone())).or_default();
                a.fresh += d.fresh;
                a.read += d.cache_read;
                a.write += d.cache_write;
                a.cost += d.cost_usd;
                a.known |= d.cost_known;
                a.est |= est;
                a.compactions += d.compactions;
                let g = self.co2_of(d);
                a.co2 = if a.parts == 0 {
                    g
                } else {
                    a.co2
                        .zip(g)
                        .map(|(x, y)| std::array::from_fn(|i| x[i] + y[i]))
                };
                a.parts += 1;
                if est || e.provider.starts_with(crate::import::PREFIX) {
                } else if e.main {
                    // Work done is counted on main threads only: subagents add cost, never units of work.
                    let first = a.mains == 0;
                    a.mains += 1;
                    let join = |acc: Option<u64>, v: Option<u64>| {
                        if first {
                            v
                        } else {
                            acc.zip(v).map(|(x, y)| x + y)
                        }
                    };
                    a.output = join(a.output, d.output);
                    a.edits =
                        join(a.edits.map(u64::from), d.edits.map(u64::from)).map(|v| v as u32);
                    a.hours = if first {
                        d.hours
                    } else {
                        a.hours.zip(d.hours).map(|(x, y)| x | y)
                    };
                    a.sessions.insert(e.session);
                } else {
                    a.agent_cost += d.cost_usd;
                    if agent_days.insert((d.day_ms, k)) {
                        a.agents += 1;
                    }
                }
            }
        }
        let water = self.has_water();
        acc.into_iter()
            .map(|((day, k), a)| DailyRow {
                provider: match by {
                    By::Provider => Some(ProviderId(k.clone())),
                    _ => provider.map(|p| ProviderId(p.to_string())),
                },
                day_ms: day,
                project: (by == By::Project).then(|| k.clone()),
                model: (by == By::Model).then_some(k),
                fresh_tokens: a.fresh,
                cache_read_tokens: a.read,
                cache_write_tokens: a.write,
                cost_usd: Amount {
                    value: a.known.then_some(a.cost),
                    est: a.est,
                },
                sessions: a.sessions.len() as u32,
                agents: a.agents,
                compactions: a.compactions,
                output_tokens: if a.mains == 0 { Some(0) } else { a.output },
                edits: if a.mains == 0 { Some(0) } else { a.edits },
                active_hours: if a.mains == 0 {
                    Some(0)
                } else {
                    a.hours.map(u32::count_ones)
                },
                agent_cost_usd: a.agent_cost,
                co2_g: a.co2.map(|g| g[0]),
                co2_g_max: a.co2.map(|g| g[1]),
                co2_cache_g: a.co2.map(|g| g[2]),
                water_l: a.co2.filter(|_| water).map(|g| g[3]),
                water_l_max: a.co2.filter(|_| water).map(|g| g[4]),
            })
            .collect()
    }

    pub fn week(
        &self,
        now_ms: TimestampMs,
        project: Option<&str>,
        provider: Option<&str>,
    ) -> WeekDeltas {
        self.week_for(now_ms, project, provider, None)
    }

    pub fn week_for(
        &self,
        now_ms: TimestampMs,
        project: Option<&str>,
        provider: Option<&str>,
        account: Option<&str>,
    ) -> WeekDeltas {
        let today = crate::metrics::local_midnight(now_ms);
        let start = crate::metrics::local_midnight(today - 6 * DAY_MS + DAY_MS / 2);
        let prev = crate::metrics::local_midnight(start - 7 * DAY_MS + DAY_MS / 2);
        let end = crate::metrics::local_midnight(today + DAY_MS + DAY_MS / 2);
        let sum = |from, to| {
            let rows = self.daily_for(from, to, By::None, project, provider, account);
            let f = |g: &dyn Fn(&DailyRow) -> f64| rows.iter().map(g).sum::<f64>();
            [
                f(&|r| r.fresh_tokens as f64),
                f(&|r| r.cache_read_tokens as f64),
                f(&|r| r.cost_usd.value.unwrap_or(0.0)),
                f(&|r| f64::from(r.sessions)),
                f(&|r| f64::from(r.agents)),
                f(&|r| r.agent_cost_usd),
            ]
        };
        let (t, l) = (sum(start, end), sum(prev, start));
        let delta = |t: f64, l: f64| WeekDelta {
            this: t,
            last: l,
            delta_pct: (l > 0.0).then(|| (t - l) / l * 100.0),
        };
        let d = |i: usize| delta(t[i], l[i]);
        let opt = |g: &dyn Fn(&DailyRow) -> Option<f64>| {
            let s = |from, to| {
                self.daily_for(from, to, By::None, project, provider, account)
                    .iter()
                    .map(g)
                    .sum::<Option<f64>>()
            };
            s(start, end).zip(s(prev, start)).map(|(t, l)| delta(t, l))
        };
        WeekDeltas {
            fresh_tokens: d(0),
            cache_read_tokens: d(1),
            cost_usd: d(2),
            sessions: d(3),
            agents: d(4),
            agent_cost_usd: d(5),
            output_tokens: opt(&|r| r.output_tokens.map(|v| v as f64)),
            edits: opt(&|r| r.edits.map(f64::from)),
            active_hours: opt(&|r| r.active_hours.map(f64::from)),
            co2_g: opt(&|r| r.co2_g),
            co2_g_max: opt(&|r| r.co2_g_max),
            co2_cache_g: opt(&|r| r.co2_cache_g),
            water_l: opt(&|r| r.water_l),
            water_l_max: opt(&|r| r.water_l_max),
        }
    }

    /// Reads claude-carbon's `sessions-history.tsv` (see NOTICE), read-only.
    pub fn import_claude_carbon(&mut self, path: &Path) {
        let Ok(text) = std::fs::read_to_string(path) else {
            return;
        };
        self.imported.clear();
        for line in text.lines() {
            let c: Vec<&str> = line.split('\t').collect();
            if c.len() < 11 {
                continue;
            }
            let n = |i: usize| c[i].trim().parse::<u64>().ok();
            let (Some(start_s), Some(input), Some(cw), Some(cr), Some(out)) =
                (n(1), n(7), n(8), n(9), n(10))
            else {
                continue;
            };
            let day = crate::metrics::local_midnight(start_s as TimestampMs * 1000);
            if input + cw + cr + out == 0 {
                continue;
            }
            let cost = c[5].trim().parse::<f64>().ok();
            self.imported.push(Entry {
                provider: V2_PROVIDER.to_string(),
                session: fnv(c[0].as_bytes()),
                project: c[4].to_string(),
                main: true,
                floor_ms: day,
                days: vec![DayPart {
                    day_ms: day,
                    model: c[6].to_string(),
                    fresh: input + cw + out,
                    cache_read: cr,
                    cache_write: cw,
                    cost_usd: cost.unwrap_or(0.0),
                    cost_known: cost.is_some(),
                    compactions: 0,
                    output: Some(out),
                    edits: None,
                    hours: None,
                    langs: None,
                }],
                account: None,
                last_ms: None,
                turns: None,
            });
        }
    }

    pub fn set_ccusage(&mut self, rows: &[crate::import::ImportRow]) {
        self.ccusage = rows
            .iter()
            .map(|r| {
                let cost = r.cost_usd;
                let entry = Entry {
                    provider: format!("{}{}", crate::import::PREFIX, r.agent),
                    session: fnv(r.agent.as_bytes()),
                    project: "(imported)".into(),
                    main: true,
                    floor_ms: r.day_ms,
                    days: vec![DayPart {
                        day_ms: r.day_ms,
                        model: r.model.clone(),
                        fresh: r.input + r.cache_write + r.output,
                        cache_read: r.cache_read,
                        cache_write: r.cache_write,
                        cost_usd: cost.unwrap_or(0.0),
                        cost_known: cost.is_some(),
                        compactions: 0,
                        output: Some(r.output),
                        edits: None,
                        hours: None,
                        langs: None,
                    }],
                    account: None,
                    last_ms: None,
                    turns: None,
                };
                (crate::import::native_of(&r.agent), entry)
            })
            .collect();
    }

    #[cfg(feature = "claude-stats")]
    pub fn set_claude_stats(&mut self, days: &[(Option<String>, crate::stats_cache::StatsDay)]) {
        let claude = crate::providers::claude::ID;
        self.claude_stats = days
            .iter()
            .map(|(account, d)| Entry {
                provider: claude.into(),
                session: fnv(b"stats-cache"),
                project: "(stats-cache)".into(),
                main: true,
                floor_ms: d.day_ms,
                days: vec![DayPart {
                    day_ms: d.day_ms,
                    model: d.model.clone(),
                    fresh: d.input + d.cache_write + d.output,
                    cache_read: d.cache_read,
                    cache_write: d.cache_write,
                    cost_usd: d.cost_usd.unwrap_or(0.0),
                    cost_known: d.cost_usd.is_some(),
                    compactions: 0,
                    output: Some(d.output),
                    edits: None,
                    hours: None,
                    langs: None,
                }],
                account: account.clone(),
                last_ms: None,
                turns: None,
            })
            .collect();
    }

    fn claude_stats_gaps(&self) -> Vec<Entry> {
        if self.claude_stats.is_empty() {
            return vec![];
        }
        let claude = crate::providers::claude::ID;
        let native: BTreeSet<(TimestampMs, Option<&str>)> = self
            .files
            .values()
            .filter(|e| e.provider == claude)
            .flat_map(|e| e.days.iter().map(move |d| (d.day_ms, e.account.as_deref())))
            .collect();
        let days: BTreeSet<TimestampMs> = native.iter().map(|(d, _)| *d).collect();
        self.claude_stats
            .iter()
            .filter(|s| {
                let d = s.floor_ms;
                let taken = match s.account.as_deref() {
                    None => days.contains(&d),
                    Some(a) => native.contains(&(d, None)) || native.contains(&(d, Some(a))),
                };
                !taken
            })
            .cloned()
            .collect()
    }

    fn ccusage_gaps(&self) -> Vec<Entry> {
        if self.ccusage.is_empty() {
            return vec![];
        }
        let native: BTreeSet<(&str, TimestampMs)> = self
            .files
            .values()
            .flat_map(|e| e.days.iter().map(move |d| (e.provider.as_str(), d.day_ms)))
            .collect();
        self.ccusage
            .iter()
            .filter(|(n, e)| n.is_none_or(|n| !native.contains(&(n, e.floor_ms))))
            .map(|(_, e)| e.clone())
            .collect()
    }

    pub fn entries_for(
        &self,
        from: TimestampMs,
        to: TimestampMs,
        provider: Option<&str>,
        account: Option<&str>,
    ) -> Vec<EntryView<'_>> {
        self.files
            .values()
            .filter(|e| {
                provider.is_none_or(|p| e.provider == p)
                    && account.is_none_or(|a| {
                        e.account.as_deref().unwrap_or(crate::account::UNATTRIBUTED) == a
                    })
            })
            .map(|e| EntryView {
                provider: &e.provider,
                project: &e.project,
                main: e.main,
                floor_ms: e.floor_ms,
                last_ms: e.last_ms,
                turns: e.turns,
                days: e
                    .days
                    .iter()
                    .filter(|d| d.day_ms >= from && d.day_ms < to)
                    .collect(),
            })
            .filter(|v| !v.days.is_empty())
            .collect()
    }

    pub fn file_bytes(&self) -> u64 {
        self.path
            .as_ref()
            .and_then(|p| std::fs::metadata(p).ok())
            .map_or(0, |m| m.len())
    }
}

#[derive(Debug, Clone)]
pub struct EntryView<'a> {
    pub provider: &'a str,
    pub project: &'a str,
    pub main: bool,
    pub floor_ms: TimestampMs,
    pub last_ms: Option<TimestampMs>,
    pub turns: Option<u32>,
    pub days: Vec<&'a DayPart>,
}

fn migrate<E: Into<Entry>>(files: BTreeMap<u64, E>) -> BTreeMap<u64, Entry> {
    files
        .into_iter()
        .map(|(k, e)| (key_of(V2_PROVIDER, k), e.into()))
        .collect()
}
