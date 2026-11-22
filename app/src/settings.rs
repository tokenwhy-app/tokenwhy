// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tokenwhy_core::Settings;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    #[serde(flatten)]
    pub core: Settings,
    pub anonymise: bool,
    pub project: Option<String>,
    pub provider: Option<String>,
    pub account: Option<String>,
    pub account_roots: Vec<AccountRoot>,
    pub account_labels: BTreeMap<String, String>,
    pub ccusage_import: bool,
    pub first_run_done: bool,
    pub pos: Option<(i32, i32)>,
    pub positions: BTreeMap<String, (i32, i32)>,
    /// Written for older builds; not read.
    pub monitor: Option<String>,
    pub dashboard: Option<DashWindow>,
    pub widget: bool,
    pub widget_expanded: bool,
    pub widget_opacity: Option<u8>,
    /// Read once at launch: WebView2 switches are fixed per environment, so a change waits for a restart.
    pub performance: Performance,
    /// A free string, so a new UI theme needs no Rust change; the UI shows the default family for an unknown id.
    pub theme: String,
    #[serde(default = "system_mode")]
    pub mode: String,
    pub money_view: String,
    pub icon_variant: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashWindow {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    pub maximized: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountRoot {
    #[serde(default = "claude_code")]
    pub provider: String,
    pub path: String,
    pub label: String,
}

fn claude_code() -> String {
    "claude-code".into()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Performance {
    Eco,
    #[default]
    Normal,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            core: Settings::default(),
            anonymise: false,
            project: None,
            provider: None,
            account: None,
            account_roots: vec![],
            account_labels: BTreeMap::new(),
            ccusage_import: false,
            first_run_done: false,
            pos: None,
            positions: BTreeMap::new(),
            monitor: None,
            dashboard: None,
            widget: false,
            widget_expanded: false,
            widget_opacity: None,
            performance: Performance::Normal,
            theme: "default".into(),
            mode: "system".into(),
            money_view: "pct".into(),
            icon_variant: crate::tray::DEFAULT_VARIANT.into(),
        }
    }
}

impl AppSettings {
    pub fn set(&mut self, key: &str, value: Value) -> Result<(), String> {
        let mut v = serde_json::to_value(&*self).map_err(|e| e.to_string())?;
        let obj = v.as_object_mut().ok_or("settings is not an object")?;
        if !obj.contains_key(key) {
            return Err(format!("unknown setting: {key}"));
        }
        obj.insert(key.to_string(), value);
        let new: Self =
            serde_json::from_value(v).map_err(|e| format!("bad value for {key}: {e}"))?;
        #[cfg(feature = "co2")]
        new.core.co2.validate()?;
        *self = new;
        Ok(())
    }

    /// A missing anonymisation salt is drawn once from the clock and written back, so aliases stay stable. A file
    /// this build cannot fully read is moved to `settings.json.bad` first; each readable key is kept.
    pub fn load(path: &Path) -> Self {
        let bytes = std::fs::read(path).ok();
        let raw: Option<Value> = bytes.as_ref().and_then(|b| serde_json::from_slice(b).ok());
        let whole: Option<Self> = raw
            .as_ref()
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        let mut s = match whole {
            Some(s) => s,
            None => {
                let s = raw
                    .as_ref()
                    .map_or_else(Self::default, Self::from_readable_keys);
                if bytes.is_some() {
                    set_bad_aside(path);
                    let _ = s.save(path);
                }
                s
            }
        };
        if raw
            .as_ref()
            .is_some_and(|v| v.get("theme").is_some() && v.get("mode").is_none())
        {
            s.migrate_theme();
        }
        if s.core.anon_salt == 0 {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |d| d.as_nanos());
            s.core.anon_salt = (nanos as u64) | 1;
            let _ = s.save(path);
        }
        s
    }

    fn from_readable_keys(raw: &Value) -> Self {
        let mut s = Self::default();
        let Some(obj) = raw.as_object() else { return s };
        for (k, v) in obj {
            let _ = s.set(k, v.clone());
        }
        s
    }

    fn migrate_theme(&mut self) {
        let t = std::mem::take(&mut self.theme);
        (self.theme, self.mode) = match t.as_str() {
            "system" | "light" | "dark" => ("default".into(), t.clone()),
            "glass" => (t.clone(), "system".into()),
            _ => match t.strip_suffix("-dark") {
                Some(f) => (f.into(), "dark".into()),
                None => (t.clone(), "light".into()),
            },
        };
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        tokenwhy_core::store::write_atomic(
            path,
            &serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?,
        )
    }
}

fn set_bad_aside(path: &Path) {
    let mut bad = path.as_os_str().to_owned();
    bad.push(".bad");
    let bad = PathBuf::from(bad);
    if bad.exists() || std::fs::hard_link(path, &bad).is_err() {
        let _ = tokenwhy_core::store::set_aside(path, ".bad");
    } else {
        let _ = std::fs::remove_file(path);
    }
}

fn system_mode() -> String {
    "system".into()
}

pub fn config_dir() -> Option<PathBuf> {
    let env = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let base = if cfg!(windows) {
        env("APPDATA")?
    } else if cfg!(target_os = "macos") {
        env("HOME")?.join("Library/Application Support")
    } else {
        env("XDG_CONFIG_HOME").or_else(|| env("HOME").map(|h| h.join(".config")))?
    };
    Some(base.join("tokenwhy"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokenwhy_core::{CompactView, Corner};

    #[test]
    fn owner_defaults() {
        let s = AppSettings::default();
        assert_eq!(s.core.compact_view, CompactView::C);
        assert!(!s.core.always_on_top);
        assert!(!s.first_run_done);
    }

    #[test]
    fn set_by_key_and_refuse_bad_input() {
        let mut s = AppSettings::default();
        s.set("always_on_top", json!(true)).unwrap();
        s.set("corner", json!("top-left")).unwrap();
        s.set("project", json!("alpha")).unwrap();
        assert!(s.core.always_on_top);
        assert_eq!(s.core.corner, Corner::TopLeft);
        assert_eq!(s.project.as_deref(), Some("alpha"));
        let before = s.clone();
        assert!(s.set("no_such_key", json!(1)).is_err());
        assert!(s.set("window_days", json!("many")).is_err());
        assert_eq!(s, before);
    }

    #[test]
    fn file_with_unknown_and_missing_keys_loads() {
        let dir = std::env::temp_dir().join(format!("tw-settings-{}", std::process::id()));
        let path = dir.join("settings.json");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, br#"{"window_days": 30, "future_key": 1}"#).unwrap();
        let s = AppSettings::load(&path);
        assert_eq!(s.core.window_days, 30);
        assert_eq!(s.core.compact_view, CompactView::C);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn variant_theme_saves_become_family_and_mode() {
        let dir = std::env::temp_dir().join(format!("tw-theme-{}", std::process::id()));
        let path = dir.join("settings.json");
        std::fs::create_dir_all(&dir).unwrap();
        for (old, theme, mode) in [
            ("dark", "default", "dark"),
            ("neu-dark", "neu", "dark"),
            ("clay", "clay", "light"),
            ("glass", "glass", "system"),
        ] {
            std::fs::write(&path, format!(r#"{{"theme": "{old}"}}"#)).unwrap();
            let s = AppSettings::load(&path);
            assert_eq!((s.theme.as_str(), s.mode.as_str()), (theme, mode), "{old}");
        }
        std::fs::write(&path, br#"{"theme": "neu", "mode": "system"}"#).unwrap();
        assert_eq!(AppSettings::load(&path).mode, "system");
        std::fs::remove_dir_all(&dir).ok();
    }

    // A theme id this build does not know (removed, or written by a newer build) once failed the whole parse and
    // reset every setting to its default.
    #[test]
    fn unknown_theme_keeps_other_settings() {
        let s: AppSettings = serde_json::from_slice(br#"{"window_days": 30, "widget": true, "theme": "retired-theme", "performance": "eco"}"#).unwrap();
        assert_eq!(s.core.window_days, 30);
        assert!(s.widget);
        assert_eq!(s.performance, Performance::Eco);
    }

    #[test]
    fn old_tray_only_key_is_ignored_and_dashboard_box_round_trips() {
        let s: AppSettings = serde_json::from_slice(br#"{"tray_only": true, "window_days": 30, "dashboard": {"x": -1920, "y": 40, "w": 1400, "h": 900, "maximized": true}}"#).unwrap();
        assert_eq!(s.core.window_days, 30);
        let d = s.dashboard.unwrap();
        assert_eq!((d.x, d.w, d.maximized), (-1920, 1400, true));
        assert!(!serde_json::to_string(&s).unwrap().contains("tray_only"));
    }
}
