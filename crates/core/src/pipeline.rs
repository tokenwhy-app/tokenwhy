// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use crate::account::{self, Accounts, NamedRoot, Switch};
use crate::anon::{self, AliasMap};
use crate::cache::cache_dir;
use crate::corpus::{Corpus, Root, Stats};
use crate::history::{By, History};
use crate::import::{self, Imports};
use crate::metrics::FileSummary;
use crate::model::{
    AccountInfo, AnonLevel, Capabilities, CostSource, Level, ParsedFile, PriceTable, ProviderId,
    ProviderInfo, Query, Settings, SkipCount, Snapshot, TimestampMs, Window,
};
use crate::provider::{self, Provider};
use crate::{metrics, render, rules};
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootSpec {
    pub provider: usize,
    pub path: PathBuf,
}

pub struct Pipeline {
    providers: Vec<Box<dyn Provider>>,
    prices: Vec<PriceTable>,
    corpus: Corpus,
    roots: Vec<RootSpec>,
    history: History,
    imports: Imports,
    use_imports: bool,
    accounts: Accounts,
    account_log: account::Log,
    signed: Vec<Signed>,
    #[cfg(feature = "plan")]
    plan: std::collections::BTreeMap<String, crate::plan::Reading>,
    #[cfg(feature = "plan")]
    plan_seen: Vec<(PathBuf, std::time::SystemTime)>,
    #[cfg(feature = "claude-stats")]
    stats_seen: Vec<StatsSeen>,
    store: Option<PathBuf>,
}

#[cfg(feature = "claude-stats")]
struct StatsSeen {
    file: PathBuf,
    mtime: std::time::SystemTime,
    days: Vec<(Option<String>, crate::stats_cache::StatsDay)>,
}

struct Signed {
    file: PathBuf,
    mtime: std::time::SystemTime,
    checked_ms: TimestampMs,
}

type SessionAccounts<'a> = std::collections::HashMap<(&'a ProviderId, &'a str), Option<&'a str>>;

impl Pipeline {
    pub fn open(use_cache: bool) -> Pipeline {
        Self::with_extras(Self::with_providers(
            provider::all(),
            if use_cache { cache_dir() } else { None },
        ))
    }

    fn with_extras(mut p: Pipeline) -> Pipeline {
        if let Some(tsv) = crate::history::claude_carbon_tsv() {
            p.history.import_claude_carbon(&tsv);
        }
        if let Some(v) = p.history.newer() {
            eprintln!(
                "tokenwhy: history.bin was written by a newer tokenwhy (format {v}); it is left untouched and                  this run shows only transcripts still on disk"
            );
        }
        p
    }

    pub fn with_providers(providers: Vec<Box<dyn Provider>>, store: Option<PathBuf>) -> Pipeline {
        let corpus = store.clone();
        Self::build(providers, store, corpus)
    }

    pub fn open_without_corpus() -> Pipeline {
        Self::with_extras(Self::build(provider::all(), cache_dir(), None))
    }

    fn build(
        providers: Vec<Box<dyn Provider>>,
        store: Option<PathBuf>,
        corpus_dir: Option<PathBuf>,
    ) -> Pipeline {
        let prices: Vec<PriceTable> = providers.iter().map(|p| p.prices()).collect();
        let history = History::open(store.as_deref());
        let imports = Imports::open(store.as_deref());
        let account_log = account::Log::open(store.as_deref());
        let accounts = Accounts {
            switches: account_log.read(),
            ..Accounts::default()
        };
        let corpus = Corpus::open(corpus_dir, &prices);
        let mut p = Pipeline {
            providers,
            prices,
            corpus,
            roots: vec![],
            history,
            imports,
            use_imports: true,
            accounts,
            account_log,
            signed: vec![],
            #[cfg(feature = "plan")]
            plan: Default::default(),
            #[cfg(feature = "plan")]
            plan_seen: vec![],
            #[cfg(feature = "claude-stats")]
            stats_seen: vec![],
            store,
        };
        p.sync_imports();
        p
    }

    pub fn providers(&self) -> &[Box<dyn Provider>] {
        &self.providers
    }

    pub fn roots(&self, overrides: &[(Option<String>, PathBuf)]) -> Result<Vec<RootSpec>, String> {
        for (id, _) in overrides {
            match id {
                Some(id) if !self.providers.iter().any(|p| &p.id().0 == id) => {
                    return Err(format!("unknown provider '{id}'"))
                }
                None if self.providers.len() != 1 => {
                    return Err(
                        "--root needs <provider>=<path> when several providers exist".into(),
                    )
                }
                _ => {}
            }
        }
        let mut roots = Vec::new();
        for (i, p) in self.providers.iter().enumerate() {
            if overrides.is_empty() {
                roots.extend(
                    p.default_roots()
                        .into_iter()
                        .map(|path| RootSpec { provider: i, path }),
                );
                continue;
            }
            for (id, path) in overrides {
                if id.as_deref().is_none_or(|x| x == p.id().0) {
                    roots.push(RootSpec {
                        provider: i,
                        path: path.clone(),
                    });
                }
            }
        }
        for n in &self.accounts.named {
            let Some(i) = self.providers.iter().position(|p| p.id() == n.provider) else {
                continue;
            };
            for path in &n.roots {
                let spec = RootSpec {
                    provider: i,
                    path: path.clone(),
                };
                if path.is_dir() && !roots.contains(&spec) {
                    roots.push(spec);
                }
            }
        }
        if roots.is_empty() {
            return Err("no transcript root found; pass --root".into());
        }
        Ok(roots)
    }

    pub fn set_accounts(
        &mut self,
        named: &[(String, PathBuf, String)],
        labels: std::collections::BTreeMap<String, String>,
    ) {
        self.accounts.named = named
            .iter()
            .filter_map(|(id, dir, label)| {
                let p = self.providers.iter().find(|p| &p.id().0 == id)?;
                Some(NamedRoot {
                    provider: p.id(),
                    roots: p.config_roots(dir),
                    id: account::root_id(dir),
                    label: label.clone(),
                })
            })
            .collect();
        self.accounts.labels = labels;
        self.absorb();
    }

    /// A switch is dated just after the last check that still saw the previous account; a run's first check uses the file's modification time.
    pub fn check_accounts(&mut self, now_ms: TimestampMs) -> bool {
        #[cfg(feature = "plan")]
        self.check_plan();
        let mut files: Vec<(usize, PathBuf)> = vec![];
        for r in &self.roots {
            if self.accounts.is_named(&r.path) {
                continue;
            }
            if let Some(f) = self.providers[r.provider].account_file(&r.path) {
                if !files.iter().any(|(i, x)| *i == r.provider && *x == f) {
                    files.push((r.provider, f));
                }
            }
        }
        let mut switched = false;
        for (i, file) in files {
            let Ok(mtime) = std::fs::metadata(&file).and_then(|m| m.modified()) else {
                continue;
            };
            let slot = self.signed.iter().position(|s| s.file == file);
            let prev_check = slot.map(|k| self.signed[k].checked_ms);
            let unchanged = slot.is_some_and(|k| self.signed[k].mtime == mtime);
            let fresh = Signed {
                file,
                mtime,
                checked_ms: now_ms,
            };
            let file = &fresh.file;
            let read = if unchanged {
                None
            } else {
                self.providers[i].signed_in(file)
            };
            let file_ms = mtime
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(now_ms, |d| d.as_millis() as TimestampMs)
                .min(now_ms);
            match slot {
                Some(k) => self.signed[k] = fresh,
                None => self.signed.push(fresh),
            }
            let Some(read) = read else { continue };
            let pid = self.providers[i].id().0;
            let id = account::signed_in_id(&pid, &read.key);
            if self.accounts.current(&pid).is_some_and(|s| s.id == id) {
                continue;
            }
            let label = if read.label.is_empty() {
                self.accounts
                    .switches
                    .iter()
                    .find(|s| s.id == id)
                    .map_or_else(|| self.accounts.next_label(), |s| s.label.clone())
            } else {
                read.label
            };
            let s = Switch {
                at_ms: prev_check.map_or(file_ms, |t| t + 1),
                provider: pid,
                id,
                label,
            };
            let _ = self.account_log.append(&s);
            self.accounts.switches.push(s);
            self.accounts.switches.sort_by_key(|s| s.at_ms);
            switched = true;
        }
        if switched {
            self.absorb();
        }
        switched
    }

    #[cfg(feature = "claude-stats")]
    fn check_claude_stats(&mut self) {
        let claude = crate::providers::claude::ID;
        let mut wanted: Vec<(PathBuf, Option<String>)> = vec![];
        for r in &self.roots {
            let p = &self.providers[r.provider];
            if p.id().0 != claude {
                continue;
            }
            let file = crate::stats_cache::file_of(&r.path);
            if wanted.iter().any(|(f, _)| *f == file) {
                continue;
            }
            let named = self
                .accounts
                .named
                .iter()
                .find(|n| n.provider.0 == claude && n.roots.iter().any(|x| r.path.starts_with(x)))
                .map(|n| n.id.clone());
            let account = named.or_else(|| {
                let read = p.signed_in(&p.account_file(&r.path)?)?;
                Some(account::signed_in_id(claude, &read.key))
            });
            wanted.push((file, account));
        }
        let before = self.stats_seen.len();
        self.stats_seen
            .retain(|s| wanted.iter().any(|(f, _)| *f == s.file));
        let mut changed = self.stats_seen.len() != before;
        for (file, acc) in wanted {
            let Ok(mtime) = std::fs::metadata(&file).and_then(|m| m.modified()) else {
                let n = self.stats_seen.len();
                self.stats_seen.retain(|s| s.file != file);
                changed |= self.stats_seen.len() != n;
                continue;
            };
            let slot = self.stats_seen.iter().position(|s| s.file == file);
            if slot.is_some_and(|k| self.stats_seen[k].mtime == mtime) {
                continue;
            }
            let days = match std::fs::read_to_string(&file)
                .map_err(|e| e.to_string())
                .and_then(|t| crate::stats_cache::parse(&t))
            {
                Ok(d) => d.into_iter().map(|d| (acc.clone(), d)).collect(),
                Err(e) => {
                    eprintln!("tokenwhy: {} ignored ({e})", file.display());
                    vec![]
                }
            };
            let seen = StatsSeen { file, mtime, days };
            match slot {
                Some(k) => self.stats_seen[k] = seen,
                None => self.stats_seen.push(seen),
            }
            changed = true;
        }
        if changed {
            let all: Vec<_> = self
                .stats_seen
                .iter()
                .flat_map(|s| s.days.iter().cloned())
                .collect();
            self.history.set_claude_stats(&all);
        }
    }

    #[cfg(feature = "plan")]
    fn check_plan(&mut self) {
        let claude = crate::providers::claude::ID;
        let mut files: Vec<(PathBuf, Option<String>)> = vec![];
        for r in &self.roots {
            let p = &self.providers[r.provider];
            if p.id().0 != claude {
                continue;
            }
            let Some(f) = p.account_file(&r.path) else {
                continue;
            };
            let named = self
                .accounts
                .named
                .iter()
                .find(|n| n.provider.0 == claude && n.roots.iter().any(|x| r.path.starts_with(x)))
                .map(|n| n.id.clone());
            if !files.iter().any(|(x, _)| *x == f) {
                files.push((f, named));
            }
        }
        for (file, named) in files {
            let Ok(mtime) = std::fs::metadata(&file).and_then(|m| m.modified()) else {
                continue;
            };
            match self.plan_seen.iter_mut().find(|(f, _)| *f == file) {
                Some((_, t)) if *t == mtime => continue,
                Some((_, t)) => *t = mtime,
                None => self.plan_seen.push((file.clone(), mtime)),
            }
            let Some(read) = std::fs::read(&file)
                .ok()
                .and_then(|b| crate::plan::parse(&b))
            else {
                continue;
            };
            let Some(id) =
                named.or_else(|| read.key.as_ref().map(|k| account::signed_in_id(claude, k)))
            else {
                continue;
            };
            if let Some(w) = read.seven_day {
                if let Some(end) = w.resets_ms {
                    self.history.record_plan(&id, end, w.pct, read.fetched_ms);
                }
            }
            if self
                .plan
                .get(&id)
                .is_none_or(|old| old.fetched_ms <= read.fetched_ms)
            {
                self.plan.insert(id, read);
            }
        }
    }

    #[cfg(feature = "plan")]
    fn plan_rows(&self, anonymise: bool) -> Vec<crate::model::PlanRow> {
        self.plan
            .iter()
            .enumerate()
            .map(|(i, (id, r))| crate::model::PlanRow {
                account: id.clone(),
                label: if anonymise {
                    format!("Account {}", i + 1)
                } else {
                    self.accounts.label(id)
                },
                five_hour_pct: r.five_hour.map(|w| w.pct),
                five_hour_resets_ms: r.five_hour.and_then(|w| w.resets_ms),
                seven_day_pct: r.seven_day.map(|w| w.pct),
                seven_day_resets_ms: r.seven_day.and_then(|w| w.resets_ms),
                fetched_ms: r.fetched_ms,
            })
            .collect()
    }

    fn session_accounts<'a>(corpus: &'a Corpus, accounts: &'a Accounts) -> SessionAccounts<'a> {
        corpus
            .summaries()
            .map(|s| {
                let main = s.files.iter().find(|f| f.main).or(s.files.first());
                let start = s.files.iter().filter_map(|f| f.first_ts).min();
                let acc = main.and_then(|m| accounts.of(&s.provider, &m.path, start));
                ((&s.provider, s.session.as_str()), acc)
            })
            .collect()
    }

    fn absorb(&mut self) {
        let acc = Self::session_accounts(&self.corpus, &self.accounts);
        self.history.absorb(self.corpus.summaries().flat_map(|s| {
            let a = acc
                .get(&(&s.provider, s.session.as_str()))
                .copied()
                .flatten();
            s.files.iter().map(move |f| (f, a))
        }));
    }

    fn account_infos(&self, acc: &SessionAccounts<'_>, anonymise: bool) -> Vec<AccountInfo> {
        let mut count: std::collections::BTreeMap<&str, u32> = Default::default();
        for s in self
            .corpus
            .summaries()
            .filter(|s| s.files.iter().any(|f| f.main))
        {
            let a = acc
                .get(&(&s.provider, s.session.as_str()))
                .copied()
                .flatten();
            *count.entry(a.unwrap_or(account::UNATTRIBUTED)).or_default() += 1;
        }
        let un = count.remove(account::UNATTRIBUTED);
        if count.len() < 2 {
            return vec![];
        }
        let mut v: Vec<AccountInfo> = count
            .into_iter()
            .map(|(id, sessions)| AccountInfo {
                id: id.to_string(),
                label: self.accounts.label(id),
                sessions,
            })
            .collect();
        v.sort_by(|a, b| a.label.cmp(&b.label));
        if anonymise {
            for (i, a) in v.iter_mut().enumerate() {
                a.label = format!("Account {}", i + 1);
            }
        }
        if let Some(sessions) = un {
            v.push(AccountInfo {
                id: account::UNATTRIBUTED.into(),
                label: self.accounts.label(account::UNATTRIBUTED),
                sessions,
            });
        }
        v
    }

    fn bind<'a>(providers: &'a [Box<dyn Provider>], roots: &[RootSpec]) -> Vec<Root<'a>> {
        roots
            .iter()
            .map(|s| Root {
                provider: providers[s.provider].as_ref(),
                path: s.path.clone(),
            })
            .collect()
    }

    pub fn scan(&mut self, roots: &[RootSpec], progress: &dyn Fn(f64)) -> io::Result<usize> {
        self.roots = roots.to_vec();
        let r = Self::bind(&self.providers, roots);
        let refreshed = self
            .corpus
            .refresh_all(&r, &self.prices, now_ms(), progress);
        #[cfg(feature = "sqlite")]
        crate::sqlite::release_cached();
        refreshed?;
        self.absorb();
        #[cfg(feature = "plan")]
        self.check_plan();
        #[cfg(feature = "claude-stats")]
        self.check_claude_stats();
        Ok(self
            .providers
            .iter()
            .map(|p| self.corpus.file_count(&p.id()))
            .sum())
    }

    pub fn update(&mut self, dirty: &[PathBuf]) -> io::Result<bool> {
        let r = Self::bind(&self.providers, &self.roots);
        let updated = self.corpus.update(&r, dirty, &self.prices, now_ms());
        #[cfg(feature = "sqlite")]
        crate::sqlite::release_cached();
        let changed = !updated?.is_empty();
        self.absorb();
        Ok(changed)
    }

    pub fn tick(&mut self, now_ms: TimestampMs) -> io::Result<bool> {
        let r = Self::bind(&self.providers, &self.roots);
        let changed = self
            .corpus
            .roll(metrics::local_midnight(now_ms), &r, &self.prices, now_ms);
        self.corpus.evict(now_ms)?;
        Ok(changed)
    }

    pub fn next_deadline(&self, now_ms: TimestampMs) -> TimestampMs {
        let midnight = metrics::local_midnight(now_ms);
        let next_midnight = metrics::local_midnight(midnight + 36 * 3_600_000);
        let files: Vec<&FileSummary> = self.corpus.summaries().flat_map(|s| &s.files).collect();
        [
            Some(next_midnight),
            metrics::next_live_expiry(&files, now_ms),
            self.corpus.next_eviction(),
        ]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(next_midnight)
        .max(now_ms + 1)
    }

    /// When another process holds the store lock nothing is written; a failed corpus write never stops the history save.
    pub fn save_cache(&mut self) -> io::Result<()> {
        let Some(dir) = &self.store else {
            return Ok(());
        };
        let Some(_lock) = crate::store::Lock::try_acquire(dir) else {
            return Ok(());
        };
        let h = self.history.save();
        let c = self.corpus.save();
        h.and(c)
    }

    pub fn provider_infos(&self) -> Vec<ProviderInfo> {
        self.providers
            .iter()
            .enumerate()
            .filter_map(|(i, p)| {
                let mine: Vec<&RootSpec> = self.roots.iter().filter(|r| r.provider == i).collect();
                let roots = mine.len();
                let mut skipped: Vec<SkipCount> = vec![];
                for r in &mine {
                    for s in p.skipped(&r.path) {
                        match skipped.iter_mut().find(|x| x.reason == s.reason) {
                            Some(x) => x.count += s.count,
                            None => skipped.push(s),
                        }
                    }
                }
                (roots > 0).then(|| ProviderInfo {
                    id: p.id(),
                    label: p.label().to_string(),
                    roots: roots as u32,
                    files: self.file_count(i) as u32,
                    caps: p.caps(),
                    imported: false,
                    skipped,
                })
            })
            .collect()
    }

    pub fn file_count(&self, provider: usize) -> usize {
        self.corpus.file_count(&self.providers[provider].id())
    }

    pub fn stats(&self) -> Stats {
        self.corpus.stats()
    }

    pub fn files(&self) -> Vec<ParsedFile> {
        self.corpus
            .deduped_files(&Self::bind(&self.providers, &self.roots))
    }

    pub fn report(
        &self,
        settings: &Settings,
        query: &Query,
        now_ms: TimestampMs,
    ) -> (Snapshot, AliasMap) {
        let agg = self.aggregate(settings, query, now_ms);
        let mut found = rules::findings(&agg, &settings.thresholds);
        let week_from = now_ms - rules::RECENT_MS;
        if query.window.from_ms < week_from {
            let recent = Query {
                window: Window {
                    from_ms: week_from,
                    to_ms: query.window.to_ms,
                },
                ..query.clone()
            };
            let r = self.aggregate(settings, &recent, now_ms);
            rules::mark_recent(&mut found, &rules::findings(&r, &settings.thresholds));
        }
        let opts = anon::Options {
            level: query.anonymise,
            salt: settings.anon_salt,
        };
        let mut out = render::snapshot_with(&agg, found, query, 1.0, &opts);
        out.0.history_newer = self.history.newer();
        #[cfg(feature = "plan")]
        {
            out.0.plan = self.plan_rows(out.0.anonymised);
        }
        #[cfg(feature = "co2")]
        {
            out.0.co2 = Some(crate::co2::info(&settings.co2));
        }
        out
    }

    pub fn recent(&self, settings: &Settings, now_ms: TimestampMs, anonymise: AnonLevel) -> Snapshot {
        const SPANS: [i64; 3] = [24 * 3_600_000, 72 * 3_600_000, 7 * 86_400_000];
        let mut snap = None;
        for (i, span) in SPANS.into_iter().enumerate() {
            let s = self.report(settings, &query(Some(span), now_ms, None, anonymise), now_ms).0;
            let turns: u32 = s.sessions.iter().map(|r| r.turns).sum();
            let priced = s.sessions.iter().any(|r| r.cost_usd.value.is_some_and(|c| c > 0.0));
            if (turns >= 3 && priced) || i == SPANS.len() - 1 {
                snap = Some(s);
                break;
            }
        }
        let mut snap = snap.expect("the last span always ends the loop");
        snap.daily = self.history.daily_for(
            metrics::local_midnight(snap.window.from_ms),
            now_ms + 1,
            By::Model,
            None,
            None,
            None,
        );
        snap
    }

    pub fn spend_series(
        &self,
        scale: crate::series::Scale,
        end_ms: TimestampMs,
        count: u32,
        project: Option<&str>,
        provider: Option<&str>,
        account: Option<&str>,
    ) -> Vec<crate::series::Bucket> {
        let tz = chrono::Local;
        let (from, to) = crate::series::span(scale, end_ms, count, &tz);
        let rows = self
            .history
            .daily_for(from, to, By::None, project, provider, account);
        crate::series::buckets(&rows, scale, end_ms, count, now_ms(), &tz)
    }

    fn aggregate(
        &self,
        settings: &Settings,
        query: &Query,
        now_ms: TimestampMs,
    ) -> metrics::Aggregates {
        #[cfg(feature = "co2")]
        self.history.set_co2(&settings.co2);
        let root_of = |p: &Path| {
            self.roots
                .iter()
                .position(|r| p.starts_with(&r.path))
                .unwrap_or(usize::MAX)
        };
        let acc = Self::session_accounts(&self.corpus, &self.accounts);
        let wanted = |s: &&crate::metrics::SessionSummary| {
            query.account.as_deref().is_none_or(|q| {
                acc.get(&(&s.provider, s.session.as_str()))
                    .copied()
                    .flatten()
                    .unwrap_or(account::UNATTRIBUTED)
                    == q
            })
        };
        let mut files: Vec<(usize, &FileSummary)> = self
            .corpus
            .summaries()
            .filter(wanted)
            .flat_map(|s| &s.files)
            .map(|f| (root_of(f.path()), f))
            .collect();
        files.sort_by(|a, b| (a.0, a.1.path()).cmp(&(b.0, b.1.path())));
        let files: Vec<&FileSummary> = files.into_iter().map(|(_, f)| f).collect();
        let mut agg = metrics::fold(&files, query, settings, now_ms);
        let provider = query.provider.as_ref().map(|p| p.0.as_str());
        let account = query.account.as_deref();
        agg.daily = self.history.daily_for(
            query.window.from_ms,
            query.window.to_ms,
            By::Project,
            query.project.as_deref(),
            provider,
            account,
        );
        agg.week = Some(
            self.history
                .week_for(now_ms, query.project.as_deref(), provider, account),
        );
        agg.providers = self.provider_infos();
        agg.accounts = self.account_infos(&acc, query.anonymise != AnonLevel::Off);
        agg.providers.extend(self.import_infos());
        let today = metrics::local_midnight(now_ms);
        for r in self
            .history
            .daily_for(
                today,
                now_ms + 1,
                By::Provider,
                query.project.as_deref(),
                provider,
                account,
            )
            .iter()
            .filter(|r| {
                r.provider
                    .as_ref()
                    .is_some_and(|p| p.0.starts_with(import::PREFIX))
            })
        {
            let t = &mut agg.live.today;
            t.fresh_tokens += r.fresh_tokens;
            t.cache_read_tokens += r.cache_read_tokens;
            if let Some(c) = r.cost_usd.value {
                t.cost_usd.value = Some(t.cost_usd.value.unwrap_or(0.0) + c);
            }
        }
        agg
    }

    pub fn import_ccusage(&mut self, file: &Path) -> Result<usize, String> {
        let n = self.imports.import_file(file)?;
        self.sync_imports();
        Ok(n)
    }

    pub fn use_imports(&mut self, on: bool) {
        if self.use_imports != on {
            self.use_imports = on;
            self.sync_imports();
        }
    }

    fn sync_imports(&mut self) {
        let rows = if self.use_imports {
            self.imports.rows()
        } else {
            &[]
        };
        self.history.set_ccusage(rows);
    }

    fn import_infos(&self) -> Vec<ProviderInfo> {
        if !self.use_imports {
            return vec![];
        }
        let agents: std::collections::BTreeSet<&str> = self
            .imports
            .rows()
            .iter()
            .map(|r| r.agent.as_str())
            .collect();
        let none = Level::None;
        agents
            .into_iter()
            .map(|a| ProviderInfo {
                id: ProviderId(format!("{}{a}", import::PREFIX)),
                label: import::label_of(a),
                roots: 0,
                files: 0,
                caps: Capabilities {
                    turns: none,
                    cache_read: none,
                    cache_write: none,
                    tool_outputs: none,
                    skills: none,
                    startup: none,
                    memory: none,
                    subagents: none,
                    compactions: none,
                    mcp: none,
                    cost: CostSource::Stored,
                },
                imported: true,
                skipped: vec![],
            })
            .collect()
    }

    pub fn history(&self) -> &History {
        &self.history
    }

    pub fn watch_dirs(&self, roots: &[RootSpec]) -> Vec<(PathBuf, bool)> {
        let mut out: Vec<(PathBuf, bool)> = vec![];
        for r in roots {
            for d in self.providers[r.provider].watch_dirs(&r.path) {
                if !out.contains(&d) {
                    out.push(d);
                }
            }
        }
        out
    }

    pub fn matcher(&self, roots: &[RootSpec]) -> impl Fn(&Path) -> bool + Send + 'static {
        let providers = provider::all();
        let pairs: Vec<(PathBuf, usize)> = roots
            .iter()
            .filter_map(|r| {
                let id = self.providers[r.provider].id();
                let i = providers.iter().position(|p| p.id() == id)?;
                Some((r.path.clone(), i))
            })
            .collect();
        move |p: &Path| {
            pairs
                .iter()
                .find(|(root, _)| p.starts_with(root))
                .is_some_and(|(root, i)| providers[*i].accepts(root, p))
        }
    }
}

pub fn query(
    window_ms: Option<i64>,
    now_ms: i64,
    project: Option<String>,
    anonymise: AnonLevel,
) -> Query {
    Query {
        window: Window {
            from_ms: window_ms.map_or(0, |l| now_ms - l),
            to_ms: now_ms,
        },
        project,
        anonymise,
        provider: None,
        account: None,
    }
}

/// With the `pinned-clock` feature (proof builds only), `TOKENWHY_NOW_MS` pins the clock.
pub fn now_ms() -> i64 {
    #[cfg(any(test, feature = "pinned-clock"))]
    if let Some(v) = std::env::var("TOKENWHY_NOW_MS")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        return v;
    }
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}
