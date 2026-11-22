// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use crate::settings::{AccountRoot, AppSettings};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;
use std::time::Duration;
use tokenwhy_core::pipeline::{self, Pipeline, RootSpec};
use tokenwhy_core::{AnonLevel, ProviderId, Snapshot};

pub trait Sink: Send + Sync + 'static {
    fn snapshot(&self, dashboard: &Snapshot, widget: &Snapshot, recent: &Snapshot);
    fn progress(&self, fraction: f64);
    fn status(&self, status: &Status);
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Status {
    #[default]
    Scanning,
    Ok,
    NoRoot {
        roots: Vec<String>,
    },
    Empty {
        roots: Vec<String>,
    },
    Error {
        message: String,
    },
}

enum Msg {
    Dirty(Vec<PathBuf>),
    Rescan,
    Refold,
    Import(PathBuf, Sender<Result<usize, String>>),
    #[cfg(any(feature = "wrapped", feature = "share"))]
    Read(Box<dyn FnOnce(&Pipeline) + Send>),
    Quit,
}

pub struct Engine {
    tx: Sender<Msg>,
    latest: Arc<Mutex<Option<Snapshot>>>,
    latest_all: Arc<Mutex<Option<Snapshot>>>,
    latest_recent: Arc<Mutex<Option<Snapshot>>>,
    status: Arc<Mutex<Status>>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

type SharedWatcher = Arc<Mutex<Option<tokenwhy_core::watch::Watcher>>>;

fn watch(
    pipe: &Pipeline,
    specs: &[RootSpec],
    tx: &Sender<Msg>,
) -> Option<tokenwhy_core::watch::Watcher> {
    let roots: Vec<_> = pipe
        .watch_dirs(specs)
        .into_iter()
        .map(|(path, deep)| {
            let mode = if deep {
                tokenwhy_core::watch::RecursiveMode::Recursive
            } else {
                tokenwhy_core::watch::RecursiveMode::NonRecursive
            };
            (path, mode)
        })
        .collect();
    let tx = tx.clone();
    let matcher = pipe.matcher(specs);
    catch_unwind(AssertUnwindSafe(|| {
        tokenwhy_core::watch::watch(roots, matcher, move |dirty| {
            let _ = tx.send(Msg::Dirty(dirty));
        })
    }))
    .ok()
    .and_then(|r| r.ok())
}

fn accounts_of(s: &AppSettings) -> Vec<(String, PathBuf, String)> {
    s.account_roots
        .iter()
        .filter(|r| !r.path.trim().is_empty())
        .map(|r| {
            (
                r.provider.clone(),
                PathBuf::from(r.path.trim()),
                r.label.clone(),
            )
        })
        .collect()
}

const CACHE_SAVE_EVERY: i64 = 300_000;

impl Engine {
    pub fn start(settings: Arc<RwLock<AppSettings>>, sink: Arc<dyn Sink>) -> Arc<Engine> {
        let pipe = catch_unwind(|| Pipeline::open(true)).unwrap_or_else(|_| {
            crate::log::write("cache store failed to open; running without it");
            Pipeline::open(false)
        });
        Self::start_with(pipe, root_override(), settings, sink)
    }

    fn start_with(
        mut pipe: Pipeline,
        overrides: Vec<(Option<String>, PathBuf)>,
        settings: Arc<RwLock<AppSettings>>,
        sink: Arc<dyn Sink>,
    ) -> Arc<Engine> {
        let (tx, rx) = channel::<Msg>();
        let latest = Arc::new(Mutex::new(None));
        let latest_all = Arc::new(Mutex::new(None));
        let latest_recent = Arc::new(Mutex::new(None));
        let status = Arc::new(Mutex::new(Status::Scanning));
        let s = settings.read().unwrap_or_else(|e| e.into_inner()).clone();
        pipe.set_accounts(&accounts_of(&s), s.account_labels.clone());
        let specs = pipe.roots(&overrides).unwrap_or_else(|e| {
            crate::log::write(&format!("no transcript root to watch: {e}"));
            vec![]
        });
        let watcher: SharedWatcher = Arc::new(Mutex::new(watch(&pipe, &specs, &tx)));

        let worker = {
            let shared = Shared {
                latest: latest.clone(),
                latest_all: latest_all.clone(),
                latest_recent: latest_recent.clone(),
                status: status.clone(),
                watcher,
                watched: specs,
                overrides,
                applied: Some((s.account_roots, s.account_labels)),
                tx: tx.clone(),
            };
            std::thread::Builder::new()
                .name("tokenwhy-engine".into())
                .spawn(move || run(rx, pipe, settings, sink, shared))
                .expect("spawn engine thread")
        };
        let _ = tx.send(Msg::Rescan);
        Arc::new(Engine {
            tx,
            latest,
            latest_all,
            latest_recent,
            status,
            worker: Mutex::new(Some(worker)),
        })
    }

    pub fn refresh(&self) {
        let _ = self.tx.send(Msg::Refold);
    }

    pub fn import_ccusage(&self, file: PathBuf) -> Result<usize, String> {
        let (tx, rx) = channel();
        self.tx
            .send(Msg::Import(file, tx))
            .map_err(|_| "engine stopped".to_string())?;
        rx.recv().map_err(|_| "engine stopped".to_string())?
    }

    /// A panic in `f` is logged and returned as an error; the engine keeps running.
    pub fn read<R: Send + 'static>(
        &self,
        f: impl FnOnce(&Pipeline) -> R + Send + 'static,
    ) -> Result<R, String> {
        let (tx, rx) = channel();
        self.tx
            .send(Msg::Read(Box::new(move |p| {
                let _ = tx.send(catch_unwind(AssertUnwindSafe(|| f(p))).ok());
            })))
            .map_err(|_| "engine stopped".to_string())?;
        rx.recv()
            .map_err(|_| "engine stopped".to_string())?
            .ok_or_else(|| "failed; see the log".to_string())
    }

    pub fn rescan(&self) {
        let _ = self.tx.send(Msg::Rescan);
    }

    pub fn latest(&self, widget: bool) -> Option<Snapshot> {
        let all = widget
            .then(|| {
                self.latest_all
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone()
            })
            .flatten();
        all.or_else(|| {
            self.latest
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
        })
    }

    pub fn latest_recent(&self) -> Option<Snapshot> {
        self.latest_recent
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn status(&self) -> Status {
        self.status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn quit_within(&self, wait: Duration) {
        let _ = self.tx.send(Msg::Quit);
        let worker = self.worker.lock().unwrap_or_else(|e| e.into_inner()).take();
        let Some(h) = worker else { return };
        let (done, joined) = channel();
        std::thread::spawn(move || {
            let _ = h.join();
            let _ = done.send(());
        });
        if joined.recv_timeout(wait).is_err() {
            crate::log::write("quit: engine still busy, exiting without waiting");
        }
    }
}

struct Shared {
    latest: Arc<Mutex<Option<Snapshot>>>,
    latest_all: Arc<Mutex<Option<Snapshot>>>,
    latest_recent: Arc<Mutex<Option<Snapshot>>>,
    status: Arc<Mutex<Status>>,
    watcher: SharedWatcher,
    watched: Vec<RootSpec>,
    overrides: Vec<(Option<String>, PathBuf)>,
    applied: Option<(Vec<AccountRoot>, std::collections::BTreeMap<String, String>)>,
    tx: Sender<Msg>,
}

impl Shared {
    /// The sink is called with no lock held: the shell's sink may wait on the main thread, which reads this state.
    fn set_status(&self, sink: &dyn Sink, s: Status) {
        {
            let mut cur = self.status.lock().unwrap_or_else(|e| e.into_inner());
            if *cur == s {
                return;
            }
            *cur = s.clone();
        }
        if let Status::Error { message } = &s {
            crate::log::write(&format!("engine error: {message}"));
        }
        sink.status(&s);
    }
}

fn save_cache(pipe: &mut Pipeline) {
    let _ = catch_unwind(AssertUnwindSafe(|| pipe.save_cache()));
    crate::mem::trim();
}

/// A panic may leave the pipeline half updated, so it is dropped, reopened and rescanned once.
fn run(
    rx: Receiver<Msg>,
    mut pipe: Pipeline,
    settings: Arc<RwLock<AppSettings>>,
    sink: Arc<dyn Sink>,
    mut shared: Shared,
) {
    let mut unsaved_since: Option<i64> = None;
    let mut deadline = pipeline::now_ms();
    let mut recover = false;
    let mut recovered = false;
    loop {
        let wait = (deadline - pipeline::now_ms()).clamp(0, 86_400_000) as u64;
        let (mut dirty, mut rescan, mut refold, mut quit, mut timer) =
            (Vec::<PathBuf>::new(), false, false, false, false);
        match rx.recv_timeout(Duration::from_millis(wait)) {
            Ok(Msg::Quit) | Err(RecvTimeoutError::Disconnected) => quit = true,
            Ok(Msg::Dirty(p)) => dirty = p,
            Ok(Msg::Rescan) => rescan = true,
            Ok(Msg::Refold) => refold = true,
            Ok(Msg::Import(f, back)) => {
                let _ = back.send(pipe.import_ccusage(&f));
                refold = true;
            }
            #[cfg(any(feature = "wrapped", feature = "share"))]
            Ok(Msg::Read(f)) => {
                f(&pipe);
                continue;
            }
            Err(RecvTimeoutError::Timeout) => timer = true,
        }
        if std::mem::take(&mut recover) {
            rescan = true;
        }
        while let Ok(m) = rx.try_recv() {
            match m {
                Msg::Quit => quit = true,
                Msg::Dirty(p) => dirty.extend(p),
                Msg::Rescan => rescan = true,
                Msg::Refold => refold = true,
                Msg::Import(f, back) => {
                    let _ = back.send(pipe.import_ccusage(&f));
                    refold = true;
                }
                #[cfg(any(feature = "wrapped", feature = "share"))]
                Msg::Read(f) => f(&pipe),
            }
        }
        if quit {
            save_cache(&mut pipe);
            return;
        }
        let s = settings.read().unwrap_or_else(|e| e.into_inner()).clone();
        pipe.use_imports(s.ccusage_import);
        let wanted = (s.account_roots.clone(), s.account_labels.clone());
        if shared.applied.as_ref() != Some(&wanted) {
            if shared.applied.as_ref().is_some_and(|a| a.0 != wanted.0) {
                rescan = true;
            }
            pipe.set_accounts(&accounts_of(&s), s.account_labels.clone());
            shared.applied = Some(wanted);
            refold = true;
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut changed = false;
            let mut found = None;
            if rescan {
                let roots = pipe.roots(&shared.overrides)?;
                let names: Vec<String> = roots
                    .iter()
                    .map(|r| match pipe.providers().get(r.provider) {
                        Some(p) => format!("{}: {}", p.label(), r.path.display()),
                        None => r.path.display().to_string(),
                    })
                    .collect();
                if !roots.iter().any(|r| r.path.is_dir()) {
                    return Ok((false, None, Some(Status::NoRoot { roots: names })));
                }
                if roots != shared.watched {
                    let w = watch(&pipe, &roots, &shared.tx);
                    *shared.watcher.lock().unwrap_or_else(|e| e.into_inner()) = w;
                    shared.watched = roots.clone();
                }
                shared.set_status(&*sink, Status::Scanning);
                let n = pipe
                    .scan(&roots, &|f| sink.progress(f))
                    .map_err(|e| e.to_string())?;
                found = Some(if n == 0 {
                    Status::Empty { roots: names }
                } else {
                    Status::Ok
                });
                changed = true;
            } else if !dirty.is_empty() {
                dirty.sort();
                dirty.dedup();
                changed |= pipe.update(&dirty).map_err(|e| e.to_string())?;
            }
            let now = pipeline::now_ms();
            if rescan || !dirty.is_empty() {
                changed |= pipe.check_accounts(now);
            }
            changed |= pipe.tick(now).map_err(|e| e.to_string())?;
            if !(changed || refold || timer) {
                return Ok::<_, String>((false, None, found));
            }
            let level = if s.anonymise {
                AnonLevel::Names
            } else {
                AnonLevel::Off
            };
            let mut query = pipeline::query(
                (s.core.window_days > 0).then(|| i64::from(s.core.window_days) * 86_400_000),
                now,
                s.project.clone(),
                level,
            );
            query.provider = s.provider.clone().map(ProviderId);
            query.account = s.account.clone();
            let (mut snap, _aliases) = pipe.report(&s.core, &query, now);
            // An account filter left over from a time with more accounts would hide everything.
            if query
                .account
                .as_ref()
                .is_some_and(|a| !snap.accounts.iter().any(|x| &x.id == a))
            {
                query.account = None;
                snap = pipe.report(&s.core, &query, now).0;
            }
            let all = (query.provider.is_some() || query.account.is_some()).then(|| {
                query.provider = None;
                query.account = None;
                pipe.report(&s.core, &query, now).0
            });
            let recent = pipe.recent(&s.core, now, level);
            Ok((changed, Some((snap, all, recent)), found))
        }));
        match result {
            Ok(Ok((changed, snap, found))) => {
                recovered = false;
                if changed {
                    unsaved_since.get_or_insert(pipeline::now_ms());
                }
                if let Some((snap, all, recent)) = snap {
                    let mut last = shared.latest.lock().unwrap_or_else(|e| e.into_inner());
                    let mut last_all = shared.latest_all.lock().unwrap_or_else(|e| e.into_inner());
                    let mut last_recent =
                        shared.latest_recent.lock().unwrap_or_else(|e| e.into_inner());
                    let same = |l: &Option<Snapshot>, s: &Option<Snapshot>| match (l, s) {
                        (Some(l), Some(s)) => {
                            Snapshot {
                                generated_ms: s.generated_ms,
                                ..l.clone()
                            } == *s
                        }
                        (None, None) => true,
                        _ => false,
                    };
                    let snap = Some(snap);
                    let recent = Some(recent);
                    let send = !(same(&last, &snap)
                        && same(&last_all, &all)
                        && same(&last_recent, &recent));
                    *last = snap.clone();
                    *last_all = all.clone();
                    *last_recent = recent.clone();
                    drop((last, last_all, last_recent));
                    if let (true, Some(s), Some(r)) = (send, &snap, &recent) {
                        sink.snapshot(s, all.as_ref().unwrap_or(s), r);
                    }
                }
                let cur = shared
                    .status
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                if let Some(st) = found.or_else(|| {
                    (matches!(cur, Status::Error { .. })
                        || changed && matches!(cur, Status::Empty { .. }))
                    .then_some(Status::Ok)
                }) {
                    shared.set_status(&*sink, st);
                }
            }
            Ok(Err(e)) => shared.set_status(&*sink, Status::Error { message: e }),
            Err(_) => {
                shared.set_status(
                    &*sink,
                    Status::Error {
                        message: "the engine stopped on an internal error; see the log".into(),
                    },
                );
                if !recovered {
                    if let Ok(fresh) = catch_unwind(Pipeline::open_without_corpus) {
                        drop(std::mem::replace(&mut pipe, fresh));
                        shared.applied = None;
                        recover = true;
                        recovered = true;
                    }
                }
            }
        }
        if rescan {
            crate::mem::trim();
        }
        let now = pipeline::now_ms();
        if unsaved_since.is_some_and(|t| now - t >= CACHE_SAVE_EVERY) {
            save_cache(&mut pipe);
            unsaved_since = None;
        }
        deadline = if recover {
            now
        } else {
            pipe.next_deadline(now)
        };
        if let Some(t) = unsaved_since {
            deadline = deadline.min(t + CACHE_SAVE_EVERY);
        }
    }
}

/// `TOKENWHY_ROOT=<provider>=<path>[;...]` points the app at other transcript roots (measurement runs), like the
/// CLI's `--root`. An entry without `<provider>=` is accepted while one provider is compiled.
fn root_override() -> Vec<(Option<String>, PathBuf)> {
    std::env::var("TOKENWHY_ROOT")
        .map(|v| parse_roots(&v))
        .unwrap_or_default()
}

fn parse_roots(v: &str) -> Vec<(Option<String>, PathBuf)> {
    v.split(';')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(|e| match e.split_once('=') {
            Some((id, path)) if !id.is_empty() && !id.contains(['/', '\\', ':']) => {
                (Some(id.to_string()), PathBuf::from(path))
            }
            _ => (None, PathBuf::from(e)),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{parse_roots, Engine, Sink, Status};
    use crate::settings::AppSettings;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::{channel, Sender};
    use std::sync::{Arc, Mutex, OnceLock, RwLock};
    use std::time::{Duration, Instant};
    use tokenwhy_core::pipeline::Pipeline;
    use tokenwhy_core::Snapshot;

    // Tauri's tray setters wait on the main thread, which may be inside a sync IPC command reading the engine.
    struct MainThreadSink {
        engine: Arc<OnceLock<Arc<Engine>>>,
        done: Mutex<Option<Sender<bool>>>,
        status_blocked: AtomicBool,
    }

    impl MainThreadSink {
        fn wait_for_main_thread(&self) -> bool {
            let t = Instant::now();
            let engine = loop {
                if let Some(e) = self.engine.get() {
                    break e.clone();
                }
                if t.elapsed() > Duration::from_secs(5) {
                    return false;
                }
                std::thread::sleep(Duration::from_millis(5));
            };
            let (tx, rx) = channel();
            std::thread::spawn(move || {
                let _ = engine.latest(false);
                let _ = engine.status();
                let _ = tx.send(());
            });
            rx.recv_timeout(Duration::from_secs(5)).is_ok()
        }
    }

    impl Sink for MainThreadSink {
        fn snapshot(&self, _dashboard: &Snapshot, _widget: &Snapshot, _recent: &Snapshot) {
            let ok = self.wait_for_main_thread();
            if let Some(tx) = self.done.lock().unwrap().take() {
                let _ = tx.send(ok);
            }
        }
        fn progress(&self, _fraction: f64) {}
        fn status(&self, _status: &Status) {
            if !self.wait_for_main_thread() {
                self.status_blocked.store(true, Ordering::SeqCst);
            }
        }
    }

    #[test]
    fn sink_may_wait_on_a_thread_that_reads_the_engine() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/engine-test-root");
        std::fs::create_dir_all(&root).unwrap();
        let (tx, rx) = channel();
        let cell = Arc::new(OnceLock::new());
        let sink = Arc::new(MainThreadSink {
            engine: cell.clone(),
            done: Mutex::new(Some(tx)),
            status_blocked: AtomicBool::new(false),
        });
        let pipe = Pipeline::with_providers(tokenwhy_core::provider::all(), None);
        let engine = Engine::start_with(
            pipe,
            vec![(Some("claude-code".into()), root)],
            Arc::new(RwLock::new(AppSettings::default())),
            sink.clone(),
        );
        let _ = cell.set(engine.clone());
        let ok = rx.recv_timeout(Duration::from_secs(60));
        engine.quit_within(Duration::from_secs(10));
        assert_eq!(
            ok,
            Ok(true),
            "engine state stayed locked while the sink waited"
        );
        assert!(
            !sink.status_blocked.load(Ordering::SeqCst),
            "engine status stayed locked while the sink waited"
        );
    }

    #[test]
    fn root_override_with_and_without_provider() {
        assert_eq!(
            parse_roots(r"claude-code=D:\fx\projects; C:\bare;"),
            vec![
                (Some("claude-code".into()), PathBuf::from(r"D:\fx\projects")),
                (None, PathBuf::from(r"C:\bare")),
            ]
        );
    }
}
