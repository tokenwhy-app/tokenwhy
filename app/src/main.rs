// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod engine;
mod ipc;
mod log;
mod mcp;
mod mem;
mod settings;
#[cfg(feature = "share")]
mod share;
mod shell;
mod tray;
mod window;
#[cfg(feature = "wrapped")]
mod wrapped;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tauri::{Manager, RunEvent, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

static MOVES: AtomicU64 = AtomicU64::new(0);

fn persist_soon(app: &tauri::AppHandle) {
    let gen = MOVES.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(500));
        if MOVES.load(Ordering::SeqCst) == gen {
            shell::persist(&app);
        }
    });
}

macro_rules! handlers {
    ($($extra:path),*) => {
        tauri::generate_handler![ipc::get_state, ipc::open_transparency_settings, ipc::snapshot, ipc::set_setting, ipc::rescan, ipc::import_ccusage, ipc::open_log, ipc::set_view, ipc::fit_widget, ipc::restart, ipc::wallpaper, ipc::credits, ipc::open_credit, ipc::theme_icon, ipc::exe_path, ipc::agent_brief, ipc::spend_series, $($extra),*]
    };
}

macro_rules! handlers_for_features {
    () => {{
        #[cfg(all(feature = "wrapped", feature = "share"))]
        {
            handlers!(
                wrapped::wrapped_available,
                wrapped::wrapped_font,
                wrapped::wrapped,
                wrapped::wrapped_save,
                share::share_available,
                share::share_state,
                share::share_preview,
                share::share_export,
                share::share_set,
                share::share_dismiss,
                share::share_delete,
                share::share_log
            )
        }
        #[cfg(all(feature = "wrapped", not(feature = "share")))]
        {
            handlers!(
                wrapped::wrapped_available,
                wrapped::wrapped_font,
                wrapped::wrapped,
                wrapped::wrapped_save
            )
        }
        #[cfg(all(not(feature = "wrapped"), feature = "share"))]
        {
            handlers!(
                share::share_available,
                share::share_state,
                share::share_preview,
                share::share_export,
                share::share_set,
                share::share_dismiss,
                share::share_delete,
                share::share_log
            )
        }
        #[cfg(not(any(feature = "wrapped", feature = "share")))]
        {
            handlers!()
        }
    }};
}

fn main() {
    if mcp::run_if_asked() {
        return;
    }
    log::install_panic_hook();
    log::start();
    let app = tauri::Builder::default()
        // First plugin, so a second launch (autostart plus manual) is caught before anything else starts.
        // The callback runs inside the WndProc answering the second process's WM_COPYDATA, on the main thread, where
        // `run_on_main_thread` would run inline; a helper thread queues the open so the WndProc returns first.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let app = app.clone();
            std::thread::spawn(move || shell::show_running(&app));
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .invoke_handler(handlers_for_features!())
        .setup(|app| {
            let handle = app.handle().clone();
            let path = shell::config_path();
            let loaded = path
                .as_deref()
                .map(settings::AppSettings::load)
                .unwrap_or_default();
            let settings = Arc::new(RwLock::new(loaded.clone()));
            let engine =
                engine::Engine::start(settings.clone(), Arc::new(shell::TauriSink(handle.clone())));
            app.manage(shell::Shell {
                settings,
                path,
                engine,
                running: loaded.performance,
            });
            tray::build(&handle, &loaded)?;
            #[cfg(feature = "share")]
            share::start(handle.clone());
            shell::open_initial(&handle, std::env::args().any(|a| a == "--autostart"));
            log::watch_main_thread(handle);
            Ok(())
        })
        .on_window_event(|win, event| match event {
            WindowEvent::Moved(pos) if win.label() == window::WIDGET && window::user_move() => {
                let Some(ww) = win.app_handle().get_webview_window(window::WIDGET) else {
                    return;
                };
                let sh = win.app_handle().state::<shell::Shell>();
                window::record_move(
                    &ww,
                    &mut sh.settings.write().unwrap_or_else(|e| e.into_inner()),
                    *pos,
                );
                persist_soon(win.app_handle());
            }
            WindowEvent::Moved(_) | WindowEvent::Resized(_)
                if win.label() == window::DASHBOARD && window::user_dashboard_move() =>
            {
                let Some(wd) = win.app_handle().get_webview_window(window::DASHBOARD) else {
                    return;
                };
                let sh = win.app_handle().state::<shell::Shell>();
                window::record_dashboard(
                    &wd,
                    &mut sh.settings.write().unwrap_or_else(|e| e.into_inner()),
                );
                persist_soon(win.app_handle());
            }
            _ => {}
        })
        .build(tauri::generate_context!())
        .expect("build tokenwhy app");

    app.run(|app, event| match event {
        RunEvent::ExitRequested { api, code, .. } if code.is_none() => api.prevent_exit(),
        RunEvent::Exit => {
            if let Some(sh) = app.try_state::<shell::Shell>() {
                shell::persist(app);
                sh.engine.quit_within(shell::QUIT_WAIT);
            }
        }
        _ => {}
    });
}
