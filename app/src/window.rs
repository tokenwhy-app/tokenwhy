// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Windows are destroyed, not hidden, when closed, so the WebView2 processes exit.

use crate::settings::{AppSettings, DashWindow, Performance};
use serde::Serialize;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::window::Color;
use tauri::Emitter;
use tauri::{
    AppHandle, LogicalSize, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};

pub const DASHBOARD: &str = "main";
pub const WIDGET: &str = "widget";
const MARGIN: f64 = 12.0;
const DASH_SIZE: (f64, f64) = (1400.0, 900.0);
const DASH_MIN: (f64, f64) = (720.0, 500.0);
const WIDGET_SIZE: (f64, f64) = (180.0, 76.0);
const WIDGET_BIG: (f64, f64) = (340.0, 380.0);

fn widget_size(expanded: bool) -> (f64, f64) {
    if expanded {
        WIDGET_BIG
    } else {
        WIDGET_SIZE
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum View {
    Compact,
    Expanded,
}

impl View {
    fn label(self) -> &'static str {
        match self {
            View::Compact => WIDGET,
            View::Expanded => DASHBOARD,
        }
    }
}

pub fn get(app: &AppHandle, view: View) -> Option<WebviewWindow> {
    app.get_webview_window(view.label())
}

pub fn show_dashboard(app: &AppHandle) -> tauri::Result<()> {
    if let Some(win) = get(app, View::Expanded) {
        crate::log::crumb("main: show");
        let _ = win.unminimize();
        win.show()?;
        return win.set_focus();
    }
    let perf = crate::shell::running(app);
    DASH_PLACED.hold();
    crate::log::crumb("main: build");
    let win = WebviewWindowBuilder::new(
        app,
        DASHBOARD,
        WebviewUrl::App("index.html#expanded".into()),
    )
    .additional_browser_args(&browser_args(perf))
    .title("tokenwhy")
    .inner_size(DASH_SIZE.0, DASH_SIZE.1)
    .min_inner_size(DASH_MIN.0, DASH_MIN.1)
    .resizable(true)
    // The page draws its own title bar; the shadow keeps the Windows 11 rounded corners and border.
    .decorations(false)
    .shadow(true)
    .transparent(perf == Performance::Normal)
    .visible(false)
    .icon(crate::tray::window_icon(
        &crate::shell::current(app).icon_variant,
    ))?
    .build()?;
    DASH_PLACED.hold();
    crate::log::crumb("main: built");
    crate::tray::set_taskbar_icon(&win, &crate::shell::current(app).icon_variant);
    dashboard_effects(&win, &crate::shell::current(app), perf);
    let w = win.clone();
    win.on_window_event(move |e| {
        if let tauri::WindowEvent::Focused(true) = e {
            if GLASS_SEEN.load(Ordering::Relaxed) != glass_support() as u8 {
                let app = w.app_handle();
                dashboard_effects(&w, &crate::shell::current(app), crate::shell::running(app));
            }
        }
    });
    let saved = crate::shell::current(app).dashboard;
    // Twice: landing on a screen of another DPI makes Windows rescale and shift the window.
    for _ in 0..2 {
        match saved {
            Some(d) if restore(&win, d)? => {}
            _ => center(&win)?,
        }
    }
    if saved.is_some_and(|d| d.maximized) {
        win.maximize()?;
    }
    DASH_PLACED.hold();
    set_memory_level(&win, false);
    win.show()?;
    win.set_focus()
}

pub fn show_widget(app: &AppHandle, s: &AppSettings, perf: Performance) -> tauri::Result<()> {
    let eco = perf == Performance::Eco;
    hold();
    let win = match get(app, View::Compact) {
        Some(win) => return if win.is_visible().unwrap_or(false) { Ok(()) } else { win.show() },
        None => {
            crate::log::crumb("widget: build");
            let win = WebviewWindowBuilder::new(app, WIDGET, WebviewUrl::App("index.html#compact".into()))
                .additional_browser_args(&browser_args(perf))
                .title("tokenwhy")
                .inner_size(
                    widget_size(s.widget_expanded).0,
                    widget_size(s.widget_expanded).1,
                )
                .decorations(false)
                .skip_taskbar(true)
                .resizable(false)
                // The shadow gives the Windows 11 rounded corners. No system effect: blur-behind and acrylic both turn
                // into a flat dark fill on a window that is never active.
                .transparent(!eco)
                .shadow(true)
                .focused(false)
                .always_on_top(s.core.always_on_top)
                .visible(false)
                .icon(crate::tray::window_icon(&s.icon_variant))?
                .build()?;
            hold();
            crate::log::crumb("widget: built");
            win
        }
    };
    crate::log::crumb("widget: show");
    crate::tray::set_taskbar_icon(&win, &s.icon_variant);
    let (w, h) = widget_size(s.widget_expanded);
    win.set_size(LogicalSize::new(w, h))?;
    place(&win, s)?;
    let _ = win.set_background_color(if eco { None } else { Some(Color(0, 0, 0, 0)) });
    set_memory_level(&win, eco);
    let a = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(1500));
        crate::shell::ui(&a, "widget: fallback show", |app| {
            if let Some(win) = get(app, View::Compact) {
                if !win.is_visible().unwrap_or(true) {
                    let _ = win.show();
                }
            }
        });
    });
    Ok(())
}

const WIDGET_FIT: (f64, f64) = (40.0, 640.0);
const WIDGET_FIT_W: (f64, f64) = (140.0, 420.0);

pub fn fit_widget(app: &AppHandle, width: Option<f64>, height: f64) {
    let Some(win) = get(app, View::Compact) else { return };
    let (Ok(pos), Ok(size), Ok(Some(m))) = (win.outer_position(), win.outer_size(), win.current_monitor()) else {
        return;
    };
    let k = m.scale_factor();
    let wa = m.work_area();
    let max = (wa.size.height as f64 / k - 2.0 * MARGIN).min(WIDGET_FIT.1).max(WIDGET_FIT.0);
    let lh = height.round().clamp(WIDGET_FIT.0, max);
    let lw = width.map_or(WIDGET_BIG.0, |w| w.round().clamp(WIDGET_FIT_W.0, WIDGET_FIT_W.1));
    let (pw, ph) = ((lw * k).round() as i32, (lh * k).round() as i32);
    let (w, h) = (size.width as i32, size.height as i32);
    let shown = win.is_visible().unwrap_or(true);
    if (ph - h).abs() < 2 && (pw - w).abs() < 2 {
        if !shown {
            let _ = win.show();
        }
        return;
    }
    let (right, bottom) = anchor(&m, pos.x + w / 2, pos.y + h / 2);
    let x = if right { pos.x + w - pw } else { pos.x };
    let y = if bottom { pos.y + h - ph } else { pos.y };
    hold();
    let _ = win.set_size(PhysicalSize::new(pw as u32, ph as u32));
    let _ = win.set_position(clamp_box(&m, (x, y), (pw, ph)));
    if !shown {
        let _ = win.show();
    }
    hold();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
#[repr(u8)]
pub enum GlassSupport {
    Native = 1,
    /// Windows with "Transparency effects" switched off: acrylic paints a solid fill.
    Off = 2,
    #[allow(dead_code)]
    None = 3,
}

static GLASS_SEEN: AtomicU8 = AtomicU8::new(0);

#[cfg(windows)]
pub fn glass_support() -> GlassSupport {
    const RRF_RT_REG_DWORD: u32 = 0x10;
    let mut data = [1u8, 0, 0, 0];
    let off = reg_value(
        r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
        "EnableTransparency",
        RRF_RT_REG_DWORD,
        &mut data,
    )
    .is_some_and(|_| u32::from_le_bytes(data) == 0);
    if off {
        GlassSupport::Off
    } else {
        GlassSupport::Native
    }
}

#[cfg(windows)]
pub fn reg_value(sub: &str, val: &str, flags: u32, buf: &mut [u8]) -> Option<usize> {
    #[link(name = "advapi32")]
    extern "system" {
        fn RegGetValueW(
            key: isize,
            sub: *const u16,
            val: *const u16,
            flags: u32,
            ty: *mut u32,
            data: *mut u8,
            len: *mut u32,
        ) -> i32;
    }
    const HKCU: isize = 0x8000_0001u32 as i32 as isize;
    let w = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let (sub, val) = (w(sub), w(val));
    let mut len = buf.len() as u32;
    // SAFETY: the buffer and its size are passed together; the strings are NUL-terminated.
    let rc = unsafe {
        RegGetValueW(
            HKCU,
            sub.as_ptr(),
            val.as_ptr(),
            flags,
            std::ptr::null_mut(),
            buf.as_mut_ptr(),
            &mut len,
        )
    };
    (rc == 0).then_some(len as usize)
}

#[cfg(target_os = "macos")]
pub fn glass_support() -> GlassSupport {
    GlassSupport::Native
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn glass_support() -> GlassSupport {
    GlassSupport::None
}

pub fn dashboard_effects(win: &WebviewWindow, s: &AppSettings, perf: Performance) {
    crate::log::crumb("main: effects");
    let support = glass_support();
    GLASS_SEEN.store(support as u8, Ordering::Relaxed);
    let glass = perf == Performance::Normal && s.theme == "glass";
    let _ = win.set_background_color(if glass { Some(Color(0, 0, 0, 0)) } else { None });
    // No system effect: acrylic adds a pale layer and blur-behind stutters while moving; the page
    // draws the wallpaper itself.
    native_glass(win, false);
    let _ = win.emit("glass", support);
}

#[cfg(windows)]
fn native_glass(win: &WebviewWindow, on: bool) {
    let _ = if on {
        window_vibrancy::apply_acrylic(win, Some((16, 16, 28, 96)))
    } else {
        window_vibrancy::clear_acrylic(win)
    };
}

#[cfg(target_os = "macos")]
fn native_glass(win: &WebviewWindow, on: bool) {
    use window_vibrancy::{NSVisualEffectMaterial, NSVisualEffectState};
    let _ = if on {
        window_vibrancy::apply_vibrancy(
            win,
            NSVisualEffectMaterial::HudWindow,
            Some(NSVisualEffectState::Active),
            None,
        )
    } else {
        window_vibrancy::clear_vibrancy(win).map(|_| ())
    };
}

#[cfg(not(any(windows, target_os = "macos")))]
fn native_glass(_win: &WebviewWindow, _on: bool) {}

/// Runtimes without ICoreWebView2_19 are left as they are.
#[cfg(windows)]
pub fn set_memory_level(win: &WebviewWindow, low: bool) {
    let _ = win.with_webview(move |wv| unsafe {
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
        };
        use windows_core::Interface;
        let Ok(core) = wv.controller().CoreWebView2() else {
            return;
        };
        let Ok(core19) = core.cast::<ICoreWebView2_19>() else {
            return;
        };
        let level = if low {
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
        } else {
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
        };
        let _ = core19.SetMemoryUsageTargetLevel(level);
    });
}

#[cfg(not(windows))]
pub fn set_memory_level(_win: &WebviewWindow, _low: bool) {}

pub fn close(app: &AppHandle, view: View) {
    if let Some(win) = get(app, view) {
        crate::log::crumb(format!("{}: destroy", view.label()));
        let _ = win.destroy();
    }
}

fn primary(win: &WebviewWindow) -> tauri::Result<Option<tauri::Monitor>> {
    Ok(match win.primary_monitor()? {
        Some(m) => Some(m),
        None => win.current_monitor()?,
    })
}

fn center(win: &WebviewWindow) -> tauri::Result<()> {
    let Some(mon) = primary(win)? else {
        return Ok(());
    };
    let wa = mon.work_area();
    let scale = mon.scale_factor();
    let lw = DASH_SIZE
        .0
        .min(wa.size.width as f64 / scale - 2.0 * MARGIN)
        .max(DASH_MIN.0);
    let lh = DASH_SIZE
        .1
        .min(wa.size.height as f64 / scale - 2.0 * MARGIN)
        .max(DASH_MIN.1);
    win.set_size(LogicalSize::new(lw, lh))?;
    let outer = win.outer_size()?;
    let x = wa.position.x + (wa.size.width as i32 - outer.width as i32) / 2;
    let y = wa.position.y + (wa.size.height as i32 - outer.height as i32) / 2;
    win.set_position(PhysicalPosition::new(
        x.max(wa.position.x),
        y.max(wa.position.y),
    ))
}

fn restore(win: &WebviewWindow, d: DashWindow) -> tauri::Result<bool> {
    let mons = win.available_monitors()?;
    let Some(m) = monitor_at(&mons, d.x + d.w as i32 / 2, d.y + d.h as i32 / 2) else {
        return Ok(false);
    };
    let wa = m.work_area();
    let k = m.scale_factor();
    let size = (
        d.w.min(wa.size.width).max((DASH_MIN.0 * k) as u32),
        d.h.min(wa.size.height).max((DASH_MIN.1 * k) as u32),
    );
    win.set_size(PhysicalSize::new(size.0, size.1))?;
    win.set_position(clamp_box(m, (d.x, d.y), (size.0 as i32, size.1 as i32)))?;
    Ok(true)
}

pub fn record_dashboard(win: &WebviewWindow, s: &mut AppSettings) {
    if win.is_minimized().unwrap_or(false) {
        return;
    }
    let maximized = win.is_maximized().unwrap_or(false);
    if let Some(d) = s.dashboard.as_mut().filter(|_| maximized) {
        d.maximized = true;
        return;
    }
    let (Ok(pos), Ok(size)) = (win.outer_position(), win.outer_size()) else {
        return;
    };
    s.dashboard = Some(DashWindow {
        x: pos.x,
        y: pos.y,
        w: size.width,
        h: size.height,
        maximized,
    });
}

fn monitor_key(m: &Monitor) -> String {
    m.name()
        .cloned()
        .unwrap_or_else(|| format!("{},{}", m.position().x, m.position().y))
}

fn monitor_at(mons: &[Monitor], x: i32, y: i32) -> Option<&Monitor> {
    mons.iter().find(|m| {
        let (p, z) = (m.position(), m.size());
        x >= p.x && y >= p.y && x < p.x + z.width as i32 && y < p.y + z.height as i32
    })
}

fn widget_px(m: &Monitor, expanded: bool) -> (i32, i32) {
    let k = m.scale_factor();
    let (w, h) = widget_size(expanded);
    ((w * k).round() as i32, (h * k).round() as i32)
}

fn clamp_box(m: &Monitor, (x, y): (i32, i32), (pw, ph): (i32, i32)) -> PhysicalPosition<i32> {
    let wa = m.work_area();
    let (l, t) = (wa.position.x, wa.position.y);
    let (r, b) = (
        (l + wa.size.width as i32 - pw).max(l),
        (t + wa.size.height as i32 - ph).max(t),
    );
    PhysicalPosition::new(x.clamp(l, r), y.clamp(t, b))
}

fn anchor(m: &Monitor, cx: i32, cy: i32) -> (bool, bool) {
    let wa = m.work_area();
    (
        cx > wa.position.x + wa.size.width as i32 / 2,
        cy > wa.position.y + wa.size.height as i32 / 2,
    )
}

fn clamp_into(m: &Monitor, p: (i32, i32), expanded: bool) -> PhysicalPosition<i32> {
    let small = widget_px(m, false);
    let c = clamp_box(m, p, small);
    if !expanded {
        return c;
    }
    let big = widget_px(m, true);
    let (right, bottom) = anchor(m, c.x + small.0 / 2, c.y + small.1 / 2);
    let x = if right { c.x + small.0 - big.0 } else { c.x };
    let y = if bottom { c.y + small.1 - big.1 } else { c.y };
    clamp_box(m, (x, y), big)
}

fn corner_of(m: &Monitor, expanded: bool) -> PhysicalPosition<i32> {
    let wa = m.work_area();
    let (pw, ph) = widget_px(m, expanded);
    let g = (MARGIN * m.scale_factor()) as i32;
    PhysicalPosition::new(
        wa.position.x + wa.size.width as i32 - pw - g,
        wa.position.y + wa.size.height as i32 - ph - g,
    )
}

// Moves right after the app's own (creation, `set_position`, a DPI rescale) are not the user's; filing them would save a fallback spot.
struct Placed(Mutex<Option<Instant>>);

impl Placed {
    fn hold(&self) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
    }

    fn settled(&self) -> bool {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_none_or(|t| t.elapsed() > Duration::from_millis(800))
    }
}

static PLACED: Placed = Placed(Mutex::new(None));
static DASH_PLACED: Placed = Placed(Mutex::new(None));

fn hold() {
    PLACED.hold();
}

pub fn user_move() -> bool {
    PLACED.settled()
}

pub fn user_dashboard_move() -> bool {
    DASH_PLACED.settled()
}

fn target(win: &WebviewWindow, s: &AppSettings) -> tauri::Result<Option<PhysicalPosition<i32>>> {
    let mons = win.available_monitors()?;
    let main = primary(win)?;
    if let Some(p) = s.pos {
        let (sw, sh) = WIDGET_SIZE;
        let held = mons.iter().find(|m| {
            let k = m.scale_factor();
            monitor_at(
                std::slice::from_ref(*m),
                p.0 + (sw * k / 2.0) as i32,
                p.1 + (sh * k / 2.0) as i32,
            )
            .is_some()
        });
        if let Some(m) = held {
            return Ok(Some(clamp_into(m, p, s.widget_expanded)));
        }
    }
    let Some(m) = main else { return Ok(None) };
    if let Some(&p) = s.positions.get(&monitor_key(&m)).or(s.pos.as_ref()) {
        return Ok(Some(clamp_into(&m, p, s.widget_expanded)));
    }
    Ok(Some(corner_of(&m, s.widget_expanded)))
}

pub fn place(win: &WebviewWindow, s: &AppSettings) -> tauri::Result<()> {
    hold();
    // Twice: landing on a screen of another DPI makes Windows rescale and shift the window.
    for _ in 0..2 {
        if let Some(p) = target(win, s)? {
            win.set_position(p)?;
        }
    }
    hold();
    Ok(())
}

pub fn record_move(win: &WebviewWindow, s: &mut AppSettings, pos: PhysicalPosition<i32>) {
    let (Ok(mons), Ok(size)) = (win.available_monitors(), win.outer_size()) else {
        s.pos = Some((pos.x, pos.y));
        return;
    };
    let (w, h) = (size.width as i32, size.height as i32);
    let (cx, cy) = (pos.x + w / 2, pos.y + h / 2);
    let Some(m) = monitor_at(&mons, cx, cy) else {
        s.pos = Some((pos.x, pos.y));
        return;
    };
    let (sw, sh) = widget_px(m, false);
    let compact = |(right, bottom): (bool, bool)| {
        (
            if right { pos.x + w - sw } else { pos.x },
            if bottom { pos.y + h - sh } else { pos.y },
        )
    };
    let first = anchor(m, cx, cy);
    let (x, y) = [
        first,
        (first.0, !first.1),
        (!first.0, first.1),
        (!first.0, !first.1),
    ]
    .into_iter()
    .map(|a| (a, compact(a)))
    .find(|&(a, (x, y))| anchor(m, x + sw / 2, y + sh / 2) == a)
    .map_or_else(|| compact(first), |(_, p)| p);
    s.pos = Some((x, y));
    let k = monitor_key(m);
    s.positions.insert(k.clone(), (x, y));
    s.monitor = Some(k);
}

/// Normal keeps the GPU: with `--disable-gpu` the transparent page shows no system glass. `TOKENWHY_WEBVIEW_ARGS` replaces the list.
fn browser_args(perf: Performance) -> String {
    std::env::var("TOKENWHY_WEBVIEW_ARGS").unwrap_or_else(|_| match perf {
        Performance::Eco => {
            "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --disable-gpu \
             --renderer-process-limit=1 --disable-background-networking --disable-component-update"
                .into()
        }
        Performance::Normal => {
            "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection".into()
        }
    })
}

/// The widget cannot see what Windows draws behind it, so the page lays this image out and blurs the part under the window.
pub fn wallpaper(win: &WebviewWindow, known: Option<u64>) -> Option<serde_json::Value> {
    let path = std::path::Path::new(&std::env::var_os("APPDATA")?)
        .join(r"Microsoft\Windows\Themes\TranscodedWallpaper");
    let mtime = std::fs::metadata(&path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis() as u64;
    let data = if known == Some(mtime) {
        None
    } else {
        let bytes = std::fs::read(&path).ok()?;
        let mime = if bytes.starts_with(&[0x89, b'P']) {
            "png"
        } else {
            "jpeg"
        };
        Some(format!("data:image/{mime};base64,{}", base64(&bytes)))
    };
    let monitors: Vec<_> = win
        .available_monitors()
        .ok()?
        .iter()
        .map(|m| {
            let (p, z) = (m.position(), m.size());
            serde_json::json!({ "x": p.x, "y": p.y, "w": z.width, "h": z.height })
        })
        .collect();
    let pos = win.outer_position().ok()?;
    Some(serde_json::json!({
        "mtime": mtime, "data": data, "style": wallpaper_style(), "monitors": monitors,
        "x": pos.x, "y": pos.y, "scale": win.scale_factor().unwrap_or(1.0),
    }))
}

/// WallpaperStyle in HKCU\Control Panel\Desktop: 10 fill, 6 fit, 2 stretch, 22 span, 0 centre or tile.
fn wallpaper_style() -> &'static str {
    #[cfg(windows)]
    {
        const RRF_RT_REG_SZ: u32 = 0x2;
        let mut buf = [0u8; 16];
        let n = reg_value(
            r"Control Panel\Desktop",
            "WallpaperStyle",
            RRF_RT_REG_SZ,
            &mut buf,
        )
        .unwrap_or(0);
        let text: String = buf[..n]
            .chunks(2)
            .map(|c| c[0] as char)
            .take_while(|c| c.is_ascii_digit())
            .collect();
        match text.as_str() {
            "6" => "fit",
            "2" => "stretch",
            "0" => "center",
            _ => "fill",
        }
    }
    #[cfg(not(windows))]
    "fill"
}

fn base64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            out.push(if k <= c.len() {
                A[(n >> (18 - 6 * k) & 63) as usize] as char
            } else {
                '='
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn base64_matches_rfc_4648_vectors() {
        for (raw, enc) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(super::base64(raw.as_bytes()), enc);
        }
    }
}
