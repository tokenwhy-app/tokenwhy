// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! All HTTP stays in tokenwhy-share's client, never in the webview.

use serde_json::{json, Value};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokenwhy_core::pipeline;
use tokenwhy_share::client::Store;
use tokenwhy_share::{payload, Payload, Week};

const CHECK: Duration = Duration::from_secs(6 * 3600);
const FIRST_CHECK: Duration = Duration::from_secs(90);

static WAKE: OnceLock<Mutex<Sender<()>>> = OnceLock::new();

fn store() -> Result<Store, String> {
    crate::settings::config_dir()
        .map(|d| Store::new(d.join("share")))
        .ok_or_else(|| "no config folder".to_string())
}

fn wake() {
    if let Some(tx) = WAKE.get() {
        let _ = tx.lock().unwrap_or_else(|e| e.into_inner()).send(());
    }
}

fn build(app: &AppHandle, week: Week, install: Option<String>) -> Result<Payload, String> {
    let sh = app.state::<crate::shell::Shell>();
    let core = sh
        .settings
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .core
        .clone();
    sh.engine.read(move |pipe| {
        let (snap, _) = pipe.report(&core, &payload::query(week), week.to_ms());
        payload::build(&snap, pipe.history(), week, install.as_deref())
    })
}

/// Only a finished scan reports: a partial one would ship partial numbers.
fn scan_done(app: &AppHandle) -> bool {
    let e = &app.state::<crate::shell::Shell>().engine;
    e.status() == crate::engine::Status::Ok && e.latest(false).is_some_and(|s| s.progress >= 1.0)
}

fn tick(app: &AppHandle) {
    let Ok(st) = store() else { return };
    let now = pipeline::now_ms();
    let week = Week::last_complete(now);
    if st.due(week, now) && scan_done(app) {
        let install = st.state().install;
        match build(app, week, install) {
            Ok(p) => {
                let _ = st.enqueue(week, p.to_json(), now);
            }
            Err(e) => crate::log::write(&format!("share: build failed: {e}")),
        }
    }
    if st.state().enabled || st.state().pending_delete.is_some() {
        st.flush(now);
    }
    let _ = app.emit("share_state", state_of(&st));
}

pub fn start(app: AppHandle) {
    let (tx, rx) = channel::<()>();
    let _ = WAKE.set(Mutex::new(tx));
    std::thread::spawn(move || {
        let mut wait = FIRST_CHECK;
        loop {
            match rx.recv_timeout(wait) {
                Ok(()) | Err(RecvTimeoutError::Timeout) => tick(&app),
                Err(RecvTimeoutError::Disconnected) => return,
            }
            let now = pipeline::now_ms();
            wait = store()
                .ok()
                .and_then(|s| s.next_try())
                .map(|t| Duration::from_millis(u64::try_from(t - now).unwrap_or(0).max(60_000)))
                .map_or(CHECK, |d| d.min(CHECK));
        }
    });
}

fn state_of(st: &Store) -> Value {
    let s = st.state();
    json!({
        "enabled": s.enabled,
        "prompted": s.prompted,
        "last_sent_week": s.last_sent_week,
        "last_status": s.last_status,
        "retired": s.retired,
        "queued": st.outbox().len(),
        "pending_delete": s.pending_delete.is_some(),
        "has_id": s.install.is_some(),
    })
}

#[tauri::command]
pub fn share_available() -> bool {
    true
}

#[tauri::command]
pub async fn share_state() -> Result<Value, String> {
    Ok(state_of(&store()?))
}

#[tauri::command]
pub async fn share_preview(app: AppHandle) -> Result<String, String> {
    let s = store()?.state();
    let install = s.enabled.then_some(s.install).flatten();
    let week = Week::last_complete(pipeline::now_ms());
    tauri::async_runtime::spawn_blocking(move || build(&app, week, install).map(|p| p.to_pretty()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn share_export(app: AppHandle) -> Result<String, String> {
    let s = store()?.state();
    let install = s.enabled.then_some(s.install).flatten();
    let week = Week::last_complete(pipeline::now_ms());
    let text = tauri::async_runtime::spawn_blocking(move || {
        build(&app, week, install).map(|p| p.to_pretty())
    })
    .await
    .map_err(|e| e.to_string())??;
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(std::path::PathBuf::from)
        .ok_or("no home folder")?;
    let dir = home.join("Downloads");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("tokenwhy-share-{}.json", week.label()));
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    crate::shell::reveal(&path);
    Ok(path.display().to_string())
}

#[tauri::command]
pub async fn share_set(
    enabled: bool,
    consent_version: Option<String>,
    delete: bool,
) -> Result<Value, String> {
    let st = store()?;
    let now = pipeline::now_ms();
    if enabled {
        st.enable(now, consent_version.as_deref().unwrap_or("draft"))?;
        wake();
        return Ok(state_of(&st));
    }
    st.disable()?;
    if delete {
        let r = tauri::async_runtime::spawn_blocking(move || {
            let st = store()?;
            st.delete(pipeline::now_ms()).map(|_| state_of(&st))
        })
        .await
        .map_err(|e| e.to_string())?;
        wake();
        return r;
    }
    Ok(state_of(&st))
}

#[tauri::command]
pub async fn share_dismiss() -> Result<(), String> {
    store()?.mark_prompted()
}

#[tauri::command]
pub async fn share_delete() -> Result<u64, String> {
    let r = tauri::async_runtime::spawn_blocking(|| store()?.delete(pipeline::now_ms()))
        .await
        .map_err(|e| e.to_string())?;
    wake();
    r
}

#[tauri::command]
pub async fn share_log() -> Result<String, String> {
    Ok(store()?.sent_log())
}
