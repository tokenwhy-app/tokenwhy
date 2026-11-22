// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

pub use notify::RecursiveMode;
use notify::{Event, EventKind, Watcher as _};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::{Duration, Instant, SystemTime};

const DEBOUNCE: Duration = Duration::from_secs(2);
const POLL_EVERY: Duration = Duration::from_secs(60);
/// Each change of a SQLite database means a full copy of it (`sqlite::open_copy`), so a live one is read at most this
/// often.
const SQLITE_EVERY: Duration = Duration::from_secs(15);

fn is_sqlite(p: &Path) -> bool {
    let n = p
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default();
    [".db", ".db-wal", ".sqlite", ".sqlite-wal"]
        .iter()
        .any(|e| n.ends_with(e))
}

enum Msg {
    Fs(notify::Result<Event>),
    Stop,
}

pub struct Watcher {
    _notify: Option<notify::RecommendedWatcher>,
    tx: Sender<Msg>,
}

impl Drop for Watcher {
    fn drop(&mut self) {
        let _ = self.tx.send(Msg::Stop);
    }
}

pub fn watch(
    roots: Vec<(PathBuf, RecursiveMode)>,
    is_transcript: impl Fn(&Path) -> bool + Send + 'static,
    on_dirty: impl Fn(Vec<PathBuf>) + Send + 'static,
) -> std::io::Result<Watcher> {
    watch_with(roots, is_transcript, on_dirty, DEBOUNCE, POLL_EVERY)
}

pub fn watch_with(
    roots: Vec<(PathBuf, RecursiveMode)>,
    is_transcript: impl Fn(&Path) -> bool + Send + 'static,
    on_dirty: impl Fn(Vec<PathBuf>) + Send + 'static,
    debounce: Duration,
    poll_every: Duration,
) -> std::io::Result<Watcher> {
    let (tx, rx) = mpsc::channel::<Msg>();
    let cb_tx = tx.clone();
    let mut started = true;
    let mut w = notify::recommended_watcher(move |ev| {
        let _ = cb_tx.send(Msg::Fs(ev));
    })
    .ok();
    if let Some(watcher) = w.as_mut() {
        for (r, mode) in &roots {
            if watcher.watch(r, *mode).is_err() {
                started = false;
            }
        }
    } else {
        started = false;
    }
    let keep = if started { w } else { None };

    std::thread::spawn(move || {
        let mut dirty: BTreeSet<PathBuf> = BTreeSet::new();
        let mut first: Option<Instant> = None;
        let mut slow: BTreeSet<PathBuf> = BTreeSet::new();
        let mut slow_due: Option<Instant> = None;
        let mut slow_last: Option<Instant> = None;
        let mut polling = !started;
        let mut last_poll = SystemTime::now();
        let mut next_poll = Instant::now() + poll_every;
        loop {
            let fast = match first {
                Some(t) => (t + debounce).saturating_duration_since(Instant::now()),
                None if polling => next_poll.saturating_duration_since(Instant::now()),
                None => Duration::from_secs(3600),
            };
            let timeout = slow_due.map_or(fast, |t| {
                fast.min(t.saturating_duration_since(Instant::now()))
            });
            match rx.recv_timeout(timeout) {
                Ok(Msg::Stop) | Err(RecvTimeoutError::Disconnected) => return,
                Ok(Msg::Fs(Ok(ev))) => {
                    if !matches!(ev.kind, EventKind::Access(_)) {
                        for p in ev.paths.into_iter().filter(|p| is_transcript(p)) {
                            if is_sqlite(&p) {
                                slow.insert(p);
                            } else {
                                dirty.insert(p);
                            }
                        }
                        if !slow.is_empty() && slow_due.is_none() {
                            let at = Instant::now() + debounce;
                            slow_due = Some(slow_last.map_or(at, |l| at.max(l + SQLITE_EVERY)));
                        }
                        if !dirty.is_empty() && first.is_none() {
                            first = Some(Instant::now());
                        }
                    }
                }
                Ok(Msg::Fs(Err(_))) => {
                    if !polling {
                        polling = true;
                        last_poll = SystemTime::now() - poll_every;
                        next_poll = Instant::now();
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
            if first.is_some_and(|t| t.elapsed() >= debounce) {
                first = None;
                on_dirty(std::mem::take(&mut dirty).into_iter().collect());
            }
            if slow_due.is_some_and(|t| Instant::now() >= t) {
                slow_due = None;
                slow_last = Some(Instant::now());
                on_dirty(std::mem::take(&mut slow).into_iter().collect());
            }
            if polling && first.is_none() && Instant::now() >= next_poll {
                next_poll = Instant::now() + poll_every;
                let since = last_poll;
                last_poll = SystemTime::now();
                let mut changed = Vec::new();
                for (r, mode) in &roots {
                    let deep = *mode == RecursiveMode::Recursive;
                    collect_newer(r, deep, since, &is_transcript, &mut changed);
                }
                if !changed.is_empty() {
                    on_dirty(changed);
                }
            }
        }
    });
    Ok(Watcher { _notify: keep, tx })
}

fn collect_newer(
    dir: &Path,
    deep: bool,
    since: SystemTime,
    is_transcript: &dyn Fn(&Path) -> bool,
    out: &mut Vec<PathBuf>,
) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        let Ok(t) = e.file_type() else { continue };
        if t.is_dir() {
            if deep {
                collect_newer(&p, true, since, is_transcript, out);
            }
        } else if is_transcript(&p)
            && e.metadata()
                .and_then(|m| m.modified())
                .is_ok_and(|m| m > since)
        {
            out.push(p);
        }
    }
}
