// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Orders of magnitude only: no provider publishes per-token figures for these models.

pub use tokenwhy_co2_data::ecologits;

use crate::model::Usage;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::sync::LazyLock;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Method {
    #[default]
    Ecologits,
    Jegham,
}

impl Method {
    pub fn id(self) -> &'static str {
        match self {
            Method::Ecologits => "ecologits",
            Method::Jegham => "jegham",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Co2Settings {
    pub method: Method,
    pub cache_read_factor: Option<f64>,
    pub grid_g_per_kwh: Option<f64>,
}

impl Co2Settings {
    pub fn validate(&self) -> Result<(), String> {
        if self
            .grid_g_per_kwh
            .is_some_and(|g| !(g > 0.0 && g <= 2000.0))
        {
            return Err("grid intensity must be in (0, 2000] g/kWh".into());
        }
        if self
            .cache_read_factor
            .is_some_and(|f| !(0.0..=1.0).contains(&f))
        {
            return Err("cache-read factor must be in [0, 1]".into());
        }
        Ok(())
    }

    pub fn cache_factor(&self) -> Option<f64> {
        match self.method {
            Method::Ecologits => None,
            Method::Jegham => Some(self.cache_read_factor.unwrap_or(JEGHAM.cache_read_factor)),
        }
    }
    pub fn grid(&self) -> f64 {
        self.grid_g_per_kwh.unwrap_or(match self.method {
            Method::Ecologits => {
                ecologits::provider("anthropic").map_or(0.0, |p| p.grid_kg_per_kwh * 1000.0)
            }
            Method::Jegham => JEGHAM_GRID_G_PER_KWH,
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Co2Rate {
    pub input: f64,
    pub cache_read: f64,
    pub output: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Band {
    pub lo: Co2Rate,
    pub hi: Co2Rate,
    pub water: Option<(f64, f64)>,
}

impl Band {
    pub fn mid(&self) -> Co2Rate {
        let m = |a: f64, b: f64| (a + b) / 2.0;
        Co2Rate {
            input: m(self.lo.input, self.hi.input),
            cache_read: m(self.lo.cache_read, self.hi.cache_read),
            output: m(self.lo.output, self.hi.output),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Co2Factor {
    pub prefix: String,
    pub input: f64,
    pub output: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Co2Table {
    pub version: u32,
    pub provider: String,
    pub source: String,
    pub cache_read_factor: f64,
    pub factors: Vec<Co2Factor>,
}

pub const JEGHAM_GRID_G_PER_KWH: f64 = 287.0;
pub const JEGHAM_DATA_DATE: &str = "2026-10-22";

pub fn parse_table(json: &str) -> serde_json::Result<Co2Table> {
    serde_json::from_str(json)
}

pub static JEGHAM: LazyLock<Co2Table> = LazyLock::new(|| {
    parse_table(tokenwhy_co2_data::JEGHAM_JSON).expect("built-in jegham.json is valid")
});

fn jegham(cfg: &Co2Settings, model: &str) -> Option<Co2Rate> {
    let scale = cfg.grid() / JEGHAM_GRID_G_PER_KWH;
    let cache = cfg.cache_factor().unwrap_or(JEGHAM.cache_read_factor);
    JEGHAM
        .factors
        .iter()
        .filter(|f| model.starts_with(f.prefix.as_str()))
        .max_by_key(|f| f.prefix.len())
        .map(|f| Co2Rate {
            input: f.input * scale,
            cache_read: f.input * cache * scale,
            output: f.output * scale,
        })
}

fn ecologits_band(cfg: &Co2Settings, model: &str) -> Option<Band> {
    let m = ecologits::find(model)?;
    let p = ecologits::provider(&m.provider)?;
    let i = ecologits::request(m, &p, cfg.grid() / 1000.0, 1e6, 0.0, None);
    let g = i.gwp_kg();
    let rate = |kg: f64| Co2Rate {
        output: kg * 1000.0,
        ..Co2Rate::default()
    };
    Some(Band {
        lo: rate(g.min),
        hi: rate(g.max),
        water: Some((i.water_l.min, i.water_l.max)),
    })
}

pub fn band(cfg: &Co2Settings, model: &str) -> Option<Band> {
    match cfg.method {
        Method::Ecologits => ecologits_band(cfg, model),
        Method::Jegham => jegham(cfg, model).map(|r| Band {
            lo: r,
            hi: r,
            water: None,
        }),
    }
}

pub fn grams_by_kind(rate: &Co2Rate, usage: &Usage) -> [f64; 4] {
    grams_of(
        rate,
        usage.input,
        usage.cache_write_5m + usage.cache_write_1h,
        usage.cache_read,
        usage.output,
    )
}

pub fn grams_of(
    rate: &Co2Rate,
    input: u64,
    cache_write: u64,
    cache_read: u64,
    output: u64,
) -> [f64; 4] {
    [
        input as f64 * rate.input / 1e6,
        cache_write as f64 * rate.input / 1e6,
        cache_read as f64 * rate.cache_read / 1e6,
        output as f64 * rate.output / 1e6,
    ]
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Co2Info {
    pub method: String,
    pub grid_g_per_kwh: f64,
    pub grid_is_default: bool,
    pub cache_read_factor: Option<f64>,
    pub data_date: String,
    pub water: bool,
    pub water_units: Vec<WaterUnit>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaterUnit {
    pub litres: f64,
    pub one: Cow<'static, str>,
    pub many: Cow<'static, str>,
    pub source: Cow<'static, str>,
    pub url: Cow<'static, str>,
    pub read: Cow<'static, str>,
}

pub const WATER_UNITS: &[WaterUnit] = &[
    // 2,500,000 L: specifications table, 50 x 25 x 2 m nominal (read 2026-10-23).
    WaterUnit {
        litres: 2_500_000.0,
        one: Cow::Borrowed("Olympic swimming pool"),
        many: Cow::Borrowed("Olympic swimming pools"),
        source: Cow::Borrowed("Wikipedia, Olympic-size swimming pool: 2,500 m³ at a nominal 2 m depth"),
        url: Cow::Borrowed("https://en.wikipedia.org/wiki/Olympic-size_swimming_pool"),
        read: Cow::Borrowed("2026-10-23"),
    },
    // 143 L: "chaque habitant utilise en moyenne 143 litres d'eau par jour", C.I.EAU (France), read 2026-10-23.
    WaterUnit {
        litres: 143.0,
        one: Cow::Borrowed("day of one person's water at home"),
        many: Cow::Borrowed("days of one person's water at home"),
        source: Cow::Borrowed("C.I.EAU, each inhabitant uses on average 143 litres a day"),
        url: Cow::Borrowed("https://www.cieau.com/le-metier-de-leau/ressource-en-eau-eau-potable-eaux-usees/quels-sont-les-usages-domestiques-de-leau/"),
        read: Cow::Borrowed("2026-10-23"),
    },
    // 0.25 L: the metric cup of 250 millilitres, Wikipedia "Cup (unit)" (read 2026-10-23).
    WaterUnit {
        litres: 0.25,
        one: Cow::Borrowed("glass of water"),
        many: Cow::Borrowed("glasses of water"),
        source: Cow::Borrowed("Wikipedia, Cup (unit): the metric cup of 250 mL"),
        url: Cow::Borrowed("https://en.wikipedia.org/wiki/Cup_(unit)"),
        read: Cow::Borrowed("2026-10-23"),
    },
    // 0.05 mL: the metric drop, 20 drops per mL, Wikipedia "Drop (unit)" (read 2026-10-23).
    WaterUnit {
        litres: 0.000_05,
        one: Cow::Borrowed("drop"),
        many: Cow::Borrowed("drops"),
        source: Cow::Borrowed("Wikipedia, Drop (unit): 0.05 mL, 20 drops per millilitre"),
        url: Cow::Borrowed("https://en.wikipedia.org/wiki/Drop_(unit)"),
        read: Cow::Borrowed("2026-10-23"),
    },
];

pub fn info(cfg: &Co2Settings) -> Co2Info {
    Co2Info {
        method: cfg.method.id().into(),
        grid_g_per_kwh: cfg.grid(),
        grid_is_default: cfg.grid_g_per_kwh.is_none(),
        cache_read_factor: cfg.cache_factor(),
        data_date: match cfg.method {
            Method::Ecologits => ecologits::DATA_DATE,
            Method::Jegham => JEGHAM_DATA_DATE,
        }
        .into(),
        water: cfg.method == Method::Ecologits,
        water_units: WATER_UNITS.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // claude-carbon tests/methodology-vectors-per-model.json, case "opus-5-5-claude-code-session".
    #[test]
    fn opus_session_matches_source_vector() {
        let cfg = Co2Settings {
            method: Method::Jegham,
            ..Co2Settings::default()
        };
        let r = band(&cfg, "claude-opus-5-5-20260922").unwrap().lo;
        let u = Usage {
            input: 20_000,
            cache_write_5m: 0,
            cache_write_1h: 400_000,
            cache_read: 5_000_000,
            output: 30_000,
            web_search_requests: 0,
        };
        let g: f64 = grams_by_kind(&r, &u).iter().sum();
        assert!((g - 113.52).abs() < 1e-9, "{g}");
    }
}
