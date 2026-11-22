// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use crate::settings::{AppSettings, Performance};
use crate::shell;
use crate::window::View;
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::image::Image;
use tauri::menu::{
    CheckMenuItem, CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder, SubmenuBuilder,
};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};
use tokenwhy_core::Snapshot;

#[derive(Clone)]
pub struct Tray {
    icon: TrayIcon,
    on_top: CheckMenuItem<tauri::Wry>,
    autostart: CheckMenuItem<tauri::Wry>,
    anonymise: CheckMenuItem<tauri::Wry>,
    widget: CheckMenuItem<tauri::Wry>,
    perf: Vec<CheckMenuItem<tauri::Wry>>,
    restart: tauri::menu::MenuItem<tauri::Wry>,
}

pub struct TrayState(pub Mutex<Option<Tray>>);

/// Raw RGBA, so no image decoder feature is compiled in.
struct Icons {
    tray_dark: &'static [u8],
    tray_light: &'static [u8],
    window: &'static [u8],
}

macro_rules! variants {
    ($($v:literal),*) => {
        &[$(($v, Icons {
            tray_dark: include_bytes!(concat!("../icons/themes/", $v, "/tray-dark-32.rgba")),
            tray_light: include_bytes!(concat!("../icons/themes/", $v, "/tray-light-32.rgba")),
            window: include_bytes!(concat!("../icons/themes/", $v, "/window-64.rgba")),
        })),*]
    };
}

static VARIANTS: &[(&str, Icons)] = variants!(
    "light",
    "dark",
    "neu",
    "neu-dark",
    "clay",
    "clay-dark",
    "glass",
    "green",
    "green-dark",
    "orange",
    "orange-dark",
    "pastel",
    "pastel-dark"
);

pub const DEFAULT_VARIANT: &str = "dark";

fn icons(variant: &str) -> &'static Icons {
    let find = |id: &str| VARIANTS.iter().find(|(v, _)| *v == id).map(|(_, i)| i);
    find(variant)
        .or_else(|| find(DEFAULT_VARIANT))
        .expect("default variant in table")
}

pub fn is_variant(id: &str) -> bool {
    VARIANTS.iter().any(|(v, _)| *v == id)
}

fn icon_image(variant: &str, light_taskbar: bool) -> Image<'static> {
    let i = icons(variant);
    Image::new(
        if light_taskbar {
            i.tray_light
        } else {
            i.tray_dark
        },
        32,
        32,
    )
}

pub fn window_icon(variant: &str) -> Image<'static> {
    Image::new(icons(variant).window, 64, 64)
}

/// Tauri's `set_icon` only sets ICON_SMALL; the taskbar button reads ICON_BIG and otherwise falls back to the exe
/// icon. A pinned shortcut keeps its own icon whatever the window says.
#[cfg(windows)]
pub fn set_taskbar_icon(win: &tauri::WebviewWindow, variant: &str) {
    let Ok(hwnd) = win.hwnd() else { return };
    let hwnd = hwnd.0 as isize;
    let variant = variant.to_string();
    // SendMessageW blocks until the window's thread answers: called from an IPC thread while the main thread waits
    // on that command, it hangs the app. Run it on the main thread instead.
    let _ = win.run_on_main_thread(move || set_big_icon(hwnd, &variant));
}

#[cfg(windows)]
fn set_big_icon(hwnd: isize, variant: &str) {
    #[link(name = "user32")]
    extern "system" {
        fn CreateIcon(inst: isize, w: i32, h: i32, planes: u8, bpp: u8, and: *const u8, xor: *const u8) -> isize;
        fn DestroyIcon(icon: isize) -> i32;
        fn SendMessageW(hwnd: isize, msg: u32, wp: usize, lp: isize) -> isize;
    }
    const WM_SETICON: u32 = 0x0080;
    const ICON_BIG: usize = 1;
    static MADE: std::sync::Mutex<Vec<isize>> = std::sync::Mutex::new(Vec::new());
    let bgra: Vec<u8> = icons(variant)
        .window
        .chunks_exact(4)
        .flat_map(|p| [p[2], p[1], p[0], p[3]])
        .collect();
    let mask = vec![0u8; 64 * 64 / 8];
    let icon = unsafe { CreateIcon(0, 64, 64, 1, 32, mask.as_ptr(), bgra.as_ptr()) };
    if icon == 0 {
        return;
    }
    let old = unsafe { SendMessageW(hwnd, WM_SETICON, ICON_BIG, icon) };
    let mut made = MADE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(i) = made.iter().position(|&h| h == old) {
        made.swap_remove(i);
        unsafe { DestroyIcon(old) };
    }
    made.push(icon);
}

#[cfg(not(windows))]
pub fn set_taskbar_icon(_win: &tauri::WebviewWindow, _variant: &str) {}

/// Windows taskbar colour: `SystemUsesLightTheme` is the taskbar and Start, `AppsUseLightTheme` the windows.
#[cfg(windows)]
fn light_taskbar() -> bool {
    let mut data = [0u8; 4];
    crate::window::reg_value(
        r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
        "SystemUsesLightTheme",
        0x10,
        &mut data,
    )
    .is_some_and(|_| u32::from_le_bytes(data) == 1)
}

#[cfg(not(windows))]
fn light_taskbar() -> bool {
    false
}

static LIGHT_TASKBAR: AtomicBool = AtomicBool::new(false);

pub fn set_variant(app: &AppHandle, variant: &str) {
    let Some(state) = app.try_state::<TrayState>() else {
        return;
    };
    let icon = state
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map(|t| t.icon.clone());
    if let Some(icon) = icon {
        let _ = icon.set_icon(Some(icon_image(
            variant,
            LIGHT_TASKBAR.load(Ordering::Relaxed),
        )));
    }
}

/// Follows a taskbar colour switch made while the app runs. A registry read every few seconds costs less than a
/// hidden window listening for WM_SETTINGCHANGE.
fn watch_taskbar(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(5));
        let light = light_taskbar();
        if LIGHT_TASKBAR.swap(light, Ordering::Relaxed) != light {
            shell::ui(&app, "tray icon: taskbar colour", |app| {
                set_variant(app, &shell::current(app).icon_variant);
            });
        }
    });
}

pub fn build(app: &AppHandle, s: &AppSettings) -> tauri::Result<()> {
    let check = |id: &str, text: &str, on: bool| {
        CheckMenuItemBuilder::with_id(id, text)
            .checked(on)
            .build(app)
    };
    let on_top = check("on_top", "Always on top", s.core.always_on_top)?;
    let autostart = check("autostart", "Start with OS", s.core.autostart)?;
    let anonymise = check("anonymise", "Anonymise", s.anonymise)?;
    let widget = check("widget", "Compact widget", s.widget)?;
    let perf = vec![
        check("perf:eco", "Eco", s.performance == Performance::Eco)?,
        check(
            "perf:normal",
            "Normal",
            s.performance == Performance::Normal,
        )?,
    ];
    let perf_menu = SubmenuBuilder::new(app, "Performance")
        .item(&perf[0])
        .item(&perf[1])
        .build()?;
    let restart = MenuItemBuilder::with_id("restart", "Restart to apply")
        .enabled(shell::restart_pending(app))
        .build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&MenuItemBuilder::with_id("expand", "Open dashboard").build(app)?)
        .separator()
        .item(&widget)
        .item(&on_top)
        .item(&autostart)
        .item(&anonymise)
        .item(&perf_menu)
        .item(&restart)
        .separator()
        .item(&MenuItemBuilder::with_id("rescan", "Rescan").build(app)?)
        .item(&MenuItemBuilder::with_id("cache", "Open cache folder").build(app)?)
        .item(&MenuItemBuilder::with_id("log", "Open log").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("quit", "Quit").build(app)?)
        .build()?;

    let icon = TrayIconBuilder::with_id("main")
        .icon({
            LIGHT_TASKBAR.store(light_taskbar(), Ordering::Relaxed);
            icon_image(&s.icon_variant, LIGHT_TASKBAR.load(Ordering::Relaxed))
        })
        .tooltip("tokenwhy")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                shell::show_running(tray.app_handle());
            }
        })
        .on_menu_event(|app, event| {
            let cur = shell::current(app);
            let id = event.id().as_ref();
            let _ = match id {
                "expand" => {
                    shell::set_view(app, View::Expanded);
                    Ok(())
                }
                "on_top" => shell::change(app, "always_on_top", json!(!cur.core.always_on_top)),
                "autostart" => shell::change(app, "autostart", json!(!cur.core.autostart)),
                "anonymise" => shell::change(app, "anonymise", json!(!cur.anonymise)),
                "widget" => shell::change(app, "widget", json!(!cur.widget)),
                "rescan" => {
                    app.state::<shell::Shell>().engine.rescan();
                    Ok(())
                }
                "cache" => {
                    shell::open_cache_folder();
                    Ok(())
                }
                "log" => {
                    crate::log::open();
                    Ok(())
                }
                "perf:eco" => shell::change(app, "performance", json!("eco")),
                "perf:normal" => shell::change(app, "performance", json!("normal")),
                "restart" => {
                    shell::restart(app);
                    Ok(())
                }
                "quit" => {
                    app.exit(0);
                    Ok(())
                }
                _ => Ok(()),
            };
        })
        .build(app)?;

    app.manage(TrayState(Mutex::new(Some(Tray {
        icon,
        on_top,
        autostart,
        anonymise,
        widget,
        perf,
        restart,
    }))));
    if cfg!(windows) {
        watch_taskbar(app.clone());
    }
    Ok(())
}

pub fn sync(app: &AppHandle, s: &AppSettings) {
    let Some(state) = app.try_state::<TrayState>() else {
        return;
    };
    let tray = state
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map(Tray::clone);
    let Some(t) = tray else { return };
    let _ = t.on_top.set_checked(s.core.always_on_top);
    let _ = t.autostart.set_checked(s.core.autostart);
    let _ = t.anonymise.set_checked(s.anonymise);
    let _ = t.widget.set_checked(s.widget);
    let _ = t.perf[0].set_checked(s.performance == Performance::Eco);
    let _ = t.perf[1].set_checked(s.performance == Performance::Normal);
    let _ = t.restart.set_enabled(s.performance != shell::running(app));
}

fn short(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => format!("{:.0}k", n as f64 / 1e3),
        _ => format!("{:.1}M", n as f64 / 1e6),
    }
}

pub fn set_tooltip(app: &AppHandle, snap: &Snapshot) {
    let Some(state) = app.try_state::<TrayState>() else {
        return;
    };
    let Some(icon) = state
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map(|t| t.icon.clone())
    else {
        return;
    };
    let mut text = format!(
        "tokenwhy: today {} tokens",
        short(snap.live.today.fresh_tokens)
    );
    if let Some(s) = snap.live.sessions.first() {
        text.push_str(&format!(
            ", open session {} in context",
            short(s.ctx_tokens)
        ));
    }
    // Posted, not awaited: the engine thread calls this, and a tray setter waits for the main thread, which may
    // itself be waiting on the engine.
    let _ = app.run_on_main_thread(move || {
        let _ = icon.set_tooltip(Some(text));
    });
}
