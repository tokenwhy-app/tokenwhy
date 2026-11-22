// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use std::collections::VecDeque;
use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::sync::{Mutex, MutexGuard, TryLockError};
use std::time::{Duration, Instant};

const MAX_BYTES: u64 = 256 * 1024;
static LOCK: Mutex<()> = Mutex::new(());

static CRUMBS: Mutex<VecDeque<(Instant, String)>> = Mutex::new(VecDeque::new());
const CRUMBS_KEPT: usize = 8;
const PING_EVERY: Duration = Duration::from_secs(5);
const STALL_AFTER: Duration = Duration::from_secs(10);

pub fn path() -> Option<PathBuf> {
    tokenwhy_core::cache::cache_dir().map(|d| d.join("tokenwhy.log"))
}

/// Appends one line. The home folder is written as `~`, so a shared log names no account.
pub fn write(line: &str) {
    write_locked(line, LOCK.lock().unwrap_or_else(|e| e.into_inner()));
}

fn write_locked(line: &str, _g: MutexGuard<'_, ()>) {
    let Some(p) = path() else { return };
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if std::fs::metadata(&p).is_ok_and(|m| m.len() > MAX_BYTES) {
        let _ = std::fs::rename(&p, p.with_extension("log.1"));
    }
    let mut text = line.to_string();
    if let Some(home) = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .filter(|h| !h.is_empty())
    {
        text = text.replace(&*home.to_string_lossy(), "~");
    }
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&p)
    {
        let _ = writeln!(f, "{secs} v{} {text}", env!("CARGO_PKG_VERSION"));
    }
}

pub fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current().name().unwrap_or("?").to_string();
        // A panic inside `write` itself holds LOCK on this thread; locking again would hang.
        let guard = match LOCK.try_lock() {
            Ok(g) => Some(g),
            Err(TryLockError::Poisoned(e)) => Some(e.into_inner()),
            Err(TryLockError::WouldBlock) => None,
        };
        if let Some(g) = guard {
            write_locked(&format!("panic in thread {thread}: {info}"), g);
        }
        default(info);
    }));
}

pub fn open() {
    let Some(p) = path() else { return };
    if !p.exists() {
        write("log opened");
    }
    crate::shell::reveal(&p);
}

pub fn start() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    write(&format!("start args={args:?}"));
}

pub fn crumb(op: impl Into<String>) {
    let mut c = CRUMBS.lock().unwrap_or_else(|e| e.into_inner());
    if c.len() == CRUMBS_KEPT {
        c.pop_front();
    }
    c.push_back((Instant::now(), op.into()));
}

fn crumbs() -> String {
    let c = CRUMBS.lock().unwrap_or_else(|e| e.into_inner());
    c.iter()
        .map(|(t, op)| format!("{op} ({} ms ago)", t.elapsed().as_millis()))
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn watch_main_thread(app: tauri::AppHandle) {
    let _ = std::thread::Builder::new()
        .name("tokenwhy-watchdog".into())
        .spawn(move || loop {
            std::thread::sleep(PING_EVERY);
            let (tx, rx) = channel();
            let sent = Instant::now();
            if app
                .run_on_main_thread(move || {
                    let _ = tx.send(());
                })
                .is_err()
            {
                return;
            }
            match rx.recv_timeout(STALL_AFTER) {
                Ok(()) => {}
                Err(RecvTimeoutError::Disconnected) => return,
                Err(RecvTimeoutError::Timeout) => {
                    write(&format!(
                        "main thread stalled {} s, last ops: {}",
                        STALL_AFTER.as_secs(),
                        crumbs()
                    ));
                    if rx.recv().is_err() {
                        return;
                    }
                    write(&format!(
                        "main thread answered after {} s",
                        sent.elapsed().as_secs()
                    ));
                }
            }
        });
}
