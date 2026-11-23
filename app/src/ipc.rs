// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use crate::shell;
use crate::window::{self, View};
use serde_json::{json, Value};
use tauri::AppHandle;

#[tauri::command]
pub async fn get_state(app: AppHandle, win: tauri::WebviewWindow) -> Value {
    shell::state_json(&app, win.label() == crate::window::WIDGET)
}

#[tauri::command]
pub async fn snapshot(
    app: AppHandle,
    win: tauri::WebviewWindow,
    window_days: u32,
    project: Option<String>,
) -> Result<Value, String> {
    shell::change(&app, "window_days", json!(window_days))?;
    shell::change(&app, "project", json!(project))?;
    use tauri::Manager;
    let engine = &app.state::<shell::Shell>().engine;
    Ok(if win.label() == crate::window::WIDGET {
        shell::widget_json(engine.latest(true).as_ref(), engine.latest_recent().as_ref())
    } else {
        json!(engine.latest(false))
    })
}

// async: a sync command runs on the main thread inside a WebView2 callback, and building the widget's webview there
// (widget on, from Settings) deadlocks WebView2 on Windows.
#[tauri::command]
pub async fn set_setting(app: AppHandle, key: String, value: Value) -> Result<(), String> {
    shell::change(&app, &key, value)
}

#[tauri::command]
pub async fn import_ccusage(app: AppHandle, path: String) -> Result<usize, String> {
    use tauri::Manager;
    let sh = app.state::<shell::Shell>();
    if !sh
        .settings
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .ccusage_import
    {
        return Err("turn on the ccusage import in Settings first".into());
    }
    let engine = sh.engine.clone();
    let path = std::path::PathBuf::from(path.trim().trim_matches('"'));
    tauri::async_runtime::spawn_blocking(move || engine.import_ccusage(path))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn rescan(app: AppHandle) {
    use tauri::Manager;
    app.state::<shell::Shell>().engine.rescan();
}

#[tauri::command]
pub async fn open_log() {
    crate::log::open();
}

#[tauri::command]
pub async fn set_view(app: AppHandle, view: String) -> Result<(), String> {
    let v = match view.as_str() {
        "compact" => View::Compact,
        "expanded" => View::Expanded,
        other => return Err(format!("unknown view: {other}")),
    };
    shell::set_view(&app, v);
    Ok(())
}

#[tauri::command]
pub async fn fit_widget(app: AppHandle, width: Option<f64>, height: f64) {
    let width = width.filter(|w| w.is_finite() && *w > 0.0);
    if height.is_finite() && height > 0.0 {
        shell::ui(&app, "fit_widget", move |app| window::fit_widget(app, width, height));
    }
}

#[tauri::command]
pub async fn restart(app: AppHandle) {
    shell::restart(&app);
}

#[tauri::command]
pub async fn open_transparency_settings() {
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer")
        .arg("ms-settings:colors")
        .spawn();
}

#[tauri::command]
pub async fn wallpaper(win: tauri::WebviewWindow, known: Option<u64>) -> Option<Value> {
    tauri::async_runtime::spawn_blocking(move || crate::window::wallpaper(&win, known))
        .await
        .ok()
        .flatten()
}

fn credit_sections() -> Vec<tokenwhy_core::about::Section> {
    #[cfg(feature = "wrapped")]
    let extra = tokenwhy_wrapped::equiv::sources();
    #[cfg(not(feature = "wrapped"))]
    let extra = Vec::new();
    tokenwhy_core::about::sections(extra)
}

#[tauri::command]
pub fn credits() -> Vec<tokenwhy_core::about::Section> {
    credit_sections()
}

/// Opens one of the credited links in the default browser; any other URL is refused, so the page gets no
/// general way to launch things.
#[tauri::command]
pub async fn open_credit(url: String) -> Result<(), String> {
    if !url.starts_with("https://") || !tokenwhy_core::about::is_listed(&credit_sections(), &url) {
        return Err("not a credited link".into());
    }
    #[cfg(windows)]
    std::process::Command::new("explorer")
        .arg(&url)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn theme_icon(app: AppHandle, variant: String) -> Result<(), String> {
    if !crate::tray::is_variant(&variant) || shell::current(&app).icon_variant == variant {
        return Ok(());
    }
    shell::change(&app, "icon_variant", json!(variant))
}

#[tauri::command]
pub fn exe_path() -> Result<String, String> {
    if crate::package::packaged() {
        return Ok("tokenwhy".into());
    }
    std::env::current_exe()
        .map(|p| p.display().to_string())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn agent_brief(app: AppHandle) -> Result<String, String> {
    use tauri::Manager;
    use tokenwhy_core::agent;
    let snap = app
        .state::<shell::Shell>()
        .engine
        .latest(false)
        .ok_or("no report yet; wait for the scan to finish")?;
    Ok(agent::markdown(&agent::brief(
        &snap,
        &agent::Filter::default(),
    )))
}

#[tauri::command]
pub async fn spend_series(
    app: AppHandle,
    scale: tokenwhy_core::series::Scale,
    end_ms: i64,
    count: u32,
    project: Option<String>,
    provider: Option<String>,
    account: Option<String>,
) -> Result<Vec<tokenwhy_core::series::Bucket>, String> {
    use tauri::Manager;
    // `read` waits for a running scan; on a tokio worker that would starve the pool set_setting and set_view use.
    let engine = app.state::<shell::Shell>().engine.clone();
    tauri::async_runtime::spawn_blocking(move || {
        engine.read(move |p| {
            p.spend_series(
                scale,
                end_ms,
                count,
                project.as_deref(),
                provider.as_deref(),
                account.as_deref(),
            )
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
