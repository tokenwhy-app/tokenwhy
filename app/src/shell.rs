// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Shell state and the one place where a setting change or a view change has its effects.
//! Tray menu and IPC commands both call here; neither holds logic of its own.
//!
//! Threading rules, from the November 2026 audit of WebView2 deadlocks:
//! 1. Window, tray, menu and HWND effects run on the main thread, posted through `ui` and never awaited; each
//!    posted effect re-reads `current(app)`, so rapid toggles end on the latest state.
//! 2. No lock guard is held across a window, tray, menu, emit or engine call: copy out, drop, then call.
//! 3. A `#[tauri::command]` is `async` unless it only reads memory (allowlist in `tests::sync_commands`);
//!    anything that waits on the engine goes through `spawn_blocking`.

use crate::engine::{Engine, Sink, Status};
use crate::settings::{self, AppSettings, Performance};
use crate::tray;
use crate::window::{self, View};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_autostart::ManagerExt;
use tokenwhy_core::Snapshot;

pub struct Shell {
    pub settings: Arc<RwLock<AppSettings>>,
    pub path: Option<PathBuf>,
    pub engine: Arc<Engine>,
    pub running: Performance,
}

pub fn running(app: &AppHandle) -> Performance {
    app.state::<Shell>().running
}

pub fn restart_pending(app: &AppHandle) -> bool {
    current(app).performance != running(app)
}

pub struct TauriSink(pub AppHandle);

impl Sink for TauriSink {
    fn snapshot(&self, dashboard: &Snapshot, widget: &Snapshot, recent: &Snapshot) {
        if window::get(&self.0, View::Expanded).is_some() {
            let _ = self.0.emit_to(window::DASHBOARD, "snapshot", dashboard);
        }
        if window::get(&self.0, View::Compact).is_some() {
            let _ = self
                .0
                .emit_to(window::WIDGET, "snapshot", widget_json(Some(widget), Some(recent)));
        }
        tray::set_tooltip(&self.0, widget);
    }
    fn progress(&self, fraction: f64) {
        let _ = self.0.emit("scan_progress", fraction);
    }
    fn status(&self, status: &Status) {
        let _ = self.0.emit("engine_status", status);
    }
}

pub fn current(app: &AppHandle) -> AppSettings {
    let sh = app.state::<Shell>();
    let s = sh
        .settings
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    s
}

pub fn persist(app: &AppHandle) {
    let sh = app.state::<Shell>();
    if let Some(p) = &sh.path {
        let _ = current(app).save(p);
    }
}

pub fn widget_json(widget: Option<&Snapshot>, recent: Option<&Snapshot>) -> Value {
    let mut v = json!(widget);
    if let (Some(o), Some(r)) = (v.as_object_mut(), recent) {
        o.insert("recent".into(), json!(r));
    }
    v
}

pub fn state_json(app: &AppHandle, widget: bool) -> Value {
    let sh = app.state::<Shell>();
    let snapshot = if widget {
        widget_json(sh.engine.latest(true).as_ref(), sh.engine.latest_recent().as_ref())
    } else {
        json!(sh.engine.latest(false))
    };
    json!({ "settings": current(app), "snapshot": snapshot, "status": sh.engine.status(), "version": env!("CARGO_PKG_VERSION"), "running_performance": sh.running, "glass": window::glass_support() })
}

pub fn open_initial(app: &AppHandle, at_login: bool) {
    ui(app, "open initial", move |app| {
        let s = current(app);
        if s.widget {
            let _ = window::show_widget(app, &s, running(app));
        }
        if !at_login {
            let _ = window::show_dashboard(app);
        }
    });
}

pub fn ui(app: &AppHandle, op: &'static str, f: impl FnOnce(&AppHandle) + Send + 'static) {
    let a = app.clone();
    let _ = app.run_on_main_thread(move || {
        crate::log::crumb(op);
        f(&a);
    });
}

pub fn show_running(app: &AppHandle) {
    ui(app, "show dashboard", |app| {
        let _ = window::show_dashboard(app);
    });
}

pub fn set_view(app: &AppHandle, view: View) {
    ui(app, "set view", move |app| {
        let _ = match view {
            View::Expanded => window::show_dashboard(app),
            View::Compact => window::show_widget(app, &current(app), running(app)),
        };
    });
}

pub fn change(app: &AppHandle, key: &str, value: Value) -> Result<(), String> {
    let sh = app.state::<Shell>();
    let (old, new) = {
        let mut w = sh.settings.write().unwrap_or_else(|e| e.into_inner());
        let old = w.clone();
        w.set(key, value)?;
        (old, w.clone())
    };
    persist(app);

    if old.core.autostart != new.core.autostart
        && std::env::var_os("TOKENWHY_NO_AUTOSTART").is_none()
    {
        let al = app.autolaunch();
        let _ = if new.core.autostart {
            al.enable()
        } else {
            al.disable()
        };
    }
    if old.core.window_days != new.core.window_days
        || old.project != new.project
        || old.provider != new.provider
        || old.account != new.account
        || old.account_roots != new.account_roots
        || old.account_labels != new.account_labels
        || old.ccusage_import != new.ccusage_import
        || old.anonymise != new.anonymise
        || old.core.thresholds != new.core.thresholds
        || old.core.project_tools != new.core.project_tools
        || old.core.reader_agent_types != new.core.reader_agent_types
        || co2_changed(&old, &new)
    {
        sh.engine.refresh();
    }

    let on_top = old.core.always_on_top != new.core.always_on_top;
    let theme = old.theme != new.theme;
    let widget = old.widget != new.widget || old.widget_expanded != new.widget_expanded;
    let icon = old.icon_variant != new.icon_variant;
    ui(app, "setting effects", move |app| {
        let s = current(app);
        if on_top {
            if let Some(win) = window::get(app, View::Compact) {
                let _ = win.set_always_on_top(s.core.always_on_top);
            }
        }
        if theme {
            if let Some(win) = window::get(app, View::Expanded) {
                window::dashboard_effects(&win, &s, running(app));
            }
        }
        if widget {
            if s.widget {
                let _ = window::show_widget(app, &s, running(app));
            } else {
                window::close(app, View::Compact);
            }
        }
        if icon {
            tray::set_variant(app, &s.icon_variant);
            for win in app.webview_windows().values() {
                let _ = win.set_icon(tray::window_icon(&s.icon_variant));
                tray::set_taskbar_icon(win, &s.icon_variant);
            }
        }
        tray::sync(app, &s);
        let _ = app.emit("settings", &s);
    });
    Ok(())
}

#[cfg(feature = "co2")]
fn co2_changed(old: &AppSettings, new: &AppSettings) -> bool {
    old.core.co2 != new.core.co2
}

#[cfg(not(feature = "co2"))]
fn co2_changed(_: &AppSettings, _: &AppSettings) -> bool {
    false
}

/// `restart` skips `RunEvent::Exit`, so the engine saves its cache here first.
pub fn restart(app: &AppHandle) {
    ui(app, "restart", |app| {
        persist(app);
        app.state::<Shell>().engine.quit_within(QUIT_WAIT);
        app.restart();
    });
}

pub const QUIT_WAIT: std::time::Duration = std::time::Duration::from_secs(3);

pub fn open_cache_folder() {
    let Some(dir) = tokenwhy_core::cache::cache_dir() else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    reveal(&dir);
}

pub fn reveal(path: &std::path::Path) {
    let opener = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(opener).arg(path).spawn();
}

pub fn config_path() -> Option<PathBuf> {
    settings::config_dir().map(|d| d.join("settings.json"))
}

#[cfg(test)]
mod tests {
    /// Rule 3: a sync command runs on the main thread inside a WebView2 callback, so only memory reads may be sync.
    #[test]
    fn sync_commands() {
        const MEMORY_READS: &[&str] = &[
            "credits",
            "exe_path",
            "agent_brief",
            "share_available",
            "wrapped_available",
            "wrapped_font",
        ];
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut sync = Vec::new();
        let mut seen = 0;
        for file in ["ipc.rs", "share.rs", "wrapped.rs"] {
            let text = std::fs::read_to_string(src.join(file)).expect("read command source");
            let mut lines = text.lines().map(str::trim);
            while let Some(line) = lines.next() {
                if line != "#[tauri::command]" {
                    continue;
                }
                let Some(sig) = lines.find(|l| !l.starts_with("#[") && !l.starts_with("//")) else {
                    break;
                };
                seen += 1;
                if let Some(rest) = sig.strip_prefix("pub fn ") {
                    let name = rest.split(['(', '<']).next().unwrap_or(rest);
                    if !MEMORY_READS.contains(&name) {
                        sync.push(format!("{file}: {name}"));
                    }
                } else {
                    assert!(sig.starts_with("pub async fn "), "{file}: unexpected command line `{sig}`");
                }
            }
        }
        assert!(seen > 20, "only {seen} commands found; parser out of step with the sources");
        assert!(sync.is_empty(), "sync commands outside the memory-read allowlist: {sync:?}");
    }
}
