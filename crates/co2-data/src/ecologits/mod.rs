// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of the MPL was not
// distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/ (copy in ./LICENSE).
//
// Port of EcoLogits 0.11.3 (GenAI Impact, github.com/mlco2/ecologits, commit d586c3566da3, 2026-10-23):
// ecologits/impacts/llm.py (formula and constants), ecologits/tracers/utils.py (provider PUE, WUE and zone) and the
// GWP and WUE of the zones those use from ecologits/data/electricity_mixes.json. ./models.json is that commit's
// ecologits/data/models.json, unmodified. Changes in this port: GWP, energy and water only (no ADPe or PE); the
// time to first token and the measured request latency are left out, so every quantity is per output token
// (tokenwhy keeps token totals per day, not per request); the grid intensity can be replaced by the user's.

use std::sync::LazyLock;

const GPU_ENERGY_ALPHA: f64 = 1.166_527_317_045_191_4e-6;
const GPU_ENERGY_BETA: f64 = -0.011_205_921_025_579_175;
const GPU_ENERGY_GAMMA: f64 = 4.052_928_146_734_005e-5;
const LATENCY_ALPHA: f64 = 0.000_678_508_809_435_366_3;
const LATENCY_BETA: f64 = 0.000_311_931_031_168_825_9;
const LATENCY_GAMMA: f64 = 0.019_473_717_579_473_387;
const QUANTIZATION_BITS: f64 = 16.0;
const GPU_MEMORY_GB: f64 = 80.0;
const GPU_EMBODIED_GWP: f64 = 273.0;
const SERVER_GPUS: f64 = 8.0;
const SERVER_POWER_KW: f64 = 1.2;
const SERVER_EMBODIED_GWP: f64 = 5700.0;
const HARDWARE_LIFESPAN_S: f64 = 3.0 * 365.0 * 24.0 * 60.0 * 60.0;
const BATCH_SIZE: f64 = 64.0;

pub const VERSION: &str = "0.11.3";
pub const DATA_DATE: &str = "2026-10-23";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Range {
    pub min: f64,
    pub max: f64,
}

impl Range {
    fn one(v: f64) -> Range {
        Range { min: v, max: v }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Provider {
    pub pue: Range,
    pub wue: Range,
    pub grid_kg_per_kwh: f64,
    pub grid_wue: f64,
}

const USA: (f64, f64) = (0.3844, 3.1321);
const SWE: (f64, f64) = (0.03526, 6.0315);

pub fn provider(name: &str) -> Option<Provider> {
    let p = |pue: (f64, f64), wue: (f64, f64), zone: (f64, f64)| Provider {
        pue: Range {
            min: pue.0,
            max: pue.1,
        },
        wue: Range {
            min: wue.0,
            max: wue.1,
        },
        grid_kg_per_kwh: zone.0,
        grid_wue: zone.1,
    };
    Some(match name {
        "anthropic" => p((1.09, 1.14), (0.13, 0.999), USA),
        "huggingface_hub" => p((1.09, 1.14), (0.13, 0.99), USA),
        "cohere" | "google_genai" => p((1.09, 1.09), (0.999, 0.999), USA),
        "mistralai" => p((1.16, 1.16), (0.09, 0.09), SWE),
        "openai" => p((1.20, 1.20), (0.569, 0.569), USA),
        _ => return None,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub provider: String,
    pub name: String,
    pub active: Range,
    pub total: Range,
    pub tps: Option<f64>,
}

fn range(v: &serde_json::Value) -> Option<Range> {
    v.as_f64().map(Range::one).or_else(|| {
        Some(Range {
            min: v.get("min")?.as_f64()?,
            max: v.get("max")?.as_f64()?,
        })
    })
}

type Catalog = (Vec<Model>, Vec<(String, String)>);

fn parse(json: &str) -> Catalog {
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
    let list = |k: &str| {
        v.get(k)
            .and_then(|a| a.as_array())
            .cloned()
            .unwrap_or_default()
    };
    let models = list("models")
        .iter()
        .filter_map(|m| {
            let arch = m.get("architecture")?;
            let p = arch.get("parameters")?;
            let (active, total) = match arch.get("type")?.as_str()? {
                "moe" => (range(p.get("active")?)?, range(p.get("total")?)?),
                _ => (range(p)?, range(p)?),
            };
            Some(Model {
                provider: m.get("provider")?.as_str()?.to_string(),
                name: m.get("name")?.as_str()?.to_string(),
                active,
                total,
                tps: m.pointer("/deployment/tps").and_then(|t| t.as_f64()),
            })
        })
        .collect();
    let aliases = list("aliases")
        .iter()
        .filter_map(|a| {
            Some((
                a.get("name")?.as_str()?.to_string(),
                a.get("alias")?.as_str()?.to_string(),
            ))
        })
        .collect();
    (models, aliases)
}

static MODELS: LazyLock<Catalog> = LazyLock::new(|| parse(include_str!("models.json")));

/// Falls back to the longest listed name `id` starts with before a `-`, `@` or `[`.
pub fn find(id: &str) -> Option<&'static Model> {
    let (models, aliases) = &*MODELS;
    let by_name = |n: &str| models.iter().find(|m| m.name == n);
    if let Some(m) = by_name(id) {
        return Some(m);
    }
    if let Some((_, target)) = aliases.iter().find(|(n, _)| n == id) {
        return by_name(target);
    }
    models
        .iter()
        .filter(|m| {
            id.strip_prefix(m.name.as_str())
                .is_some_and(|rest| rest.starts_with(['-', '@', '[']))
        })
        .max_by_key(|m| m.name.len())
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Impacts {
    pub energy_kwh: Range,
    pub water_l: Range,
    pub usage_kg: Range,
    pub embodied_kg: Range,
}

impl Impacts {
    pub fn gwp_kg(&self) -> Range {
        Range {
            min: self.usage_kg.min + self.embodied_kg.min,
            max: self.usage_kg.max + self.embodied_kg.max,
        }
    }
}

struct Point {
    it: f64,
    energy: Range,
    embodied: f64,
}

#[allow(clippy::too_many_arguments)]
fn point(
    active: f64,
    total: f64,
    out: f64,
    pue: Range,
    tps: Option<f64>,
    ttft: f64,
    latency_cap: Option<f64>,
) -> Point {
    let gpu_kwh = out
        * (GPU_ENERGY_ALPHA * (GPU_ENERGY_BETA * BATCH_SIZE).exp() * active + GPU_ENERGY_GAMMA)
        / 1000.0;
    let per_token = tps.map_or(
        LATENCY_ALPHA * active + LATENCY_BETA * BATCH_SIZE + LATENCY_GAMMA,
        |t| 1.0 / t,
    );
    let latency = (out * per_token + ttft).min(latency_cap.unwrap_or(f64::INFINITY));
    let memory = 1.2 * total * QUANTIZATION_BITS / 8.0;
    let gpus = 2f64.powf((memory / GPU_MEMORY_GB).ceil().log2().ceil());
    let server_kwh = (latency / 3600.0) * SERVER_POWER_KW * (gpus / SERVER_GPUS) / BATCH_SIZE;
    let it = server_kwh + gpus * gpu_kwh;
    let embodied_hw = (gpus / SERVER_GPUS) * SERVER_EMBODIED_GWP + gpus * GPU_EMBODIED_GWP;
    Point {
        it,
        energy: Range {
            min: pue.min * it,
            max: pue.max * it,
        },
        embodied: latency * embodied_hw / (HARDWARE_LIFESPAN_S * BATCH_SIZE),
    }
}

pub fn request(
    m: &Model,
    p: &Provider,
    grid_kg_per_kwh: f64,
    out: f64,
    ttft: f64,
    latency_cap: Option<f64>,
) -> Impacts {
    let lo = point(
        m.active.min,
        m.total.min,
        out,
        p.pue,
        m.tps,
        ttft,
        latency_cap,
    );
    let hi = point(
        m.active.max,
        m.total.max,
        out,
        p.pue,
        m.tps,
        ttft,
        latency_cap,
    );
    let energy = Range {
        min: lo.energy.min,
        max: hi.energy.max,
    };
    Impacts {
        energy_kwh: energy,
        water_l: Range {
            min: lo.it * (p.wue.min + p.pue.min * p.grid_wue),
            max: hi.it * (p.wue.max + p.pue.max * p.grid_wue),
        },
        usage_kg: Range {
            min: energy.min * grid_kg_per_kwh,
            max: energy.max * grid_kg_per_kwh,
        },
        embodied_kg: Range {
            min: lo.embodied,
            max: hi.embodied,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        ((a - b) / b).abs() < 1e-9
    }

    // EcoLogits.js spec/vectors/estimator.json, case "no-request-latency" (Python ecologits 0.11.2, commit c739e8d).
    #[test]
    fn estimator_vector_without_latency() {
        let m = Model {
            provider: String::new(),
            name: String::new(),
            active: Range::one(7.3),
            total: Range::one(7.3),
            tps: None,
        };
        let p = Provider {
            pue: Range::one(1.2),
            wue: Range::one(0.569),
            grid_kg_per_kwh: 0.590_478,
            grid_wue: 5.04,
        };
        let i = request(&m, &p, p.grid_kg_per_kwh, 200.0, 0.0, None);
        assert!(close(i.energy_kwh.min, 1.766_064_347_655_742e-5), "{i:?}");
        assert!(close(i.usage_kg.min, 1.042_822_143_875_067e-5), "{i:?}");
        assert!(
            close(i.embodied_kg.min, 1.445_000_582_133_998_2e-6),
            "{i:?}"
        );
        assert!(close(i.gwp_kg().min, 1.187_322_202_088_466_8e-5), "{i:?}");
        assert!(close(i.water_l.min, 9.738_373_157_031_703e-5), "{i:?}");
    }

    // Same spec, high-level.json case "anthropic/claude-opus-5-5", with that release's model data from repository.json.
    #[test]
    fn claude_opus_5_5_high_level_vector() {
        let m = Model {
            provider: "anthropic".into(),
            name: "claude-opus-5-5".into(),
            active: Range {
                min: 70.0,
                max: 200.0,
            },
            total: Range::one(670.0),
            tps: Some(75.5),
        };
        let p = provider(&m.provider).unwrap();
        let i = request(&m, &p, p.grid_kg_per_kwh, 200.0, 3.43, Some(5.0));
        assert!(
            close(i.energy_kwh.min, 0.000_674_330_123_518_519_4),
            "{i:?}"
        );
        assert!(
            close(i.energy_kwh.max, 0.001_245_340_381_603_235_5),
            "{i:?}"
        );
        assert!(close(i.gwp_kg().min, 0.000_285_254_166_147_185_56), "{i:?}");
        assert!(close(i.gwp_kg().max, 0.000_504_750_509_354_950_4), "{i:?}");
        assert!(close(i.water_l.min, 0.002_192_494_073_503_003_7), "{i:?}");
        assert!(close(i.water_l.max, 0.004_991_842_048_887_592_6), "{i:?}");
    }

    // Same spec, case "openai/gpt-5", where the request latency cap applies.
    #[test]
    fn gpt_5_high_level_vector() {
        let m = Model {
            provider: "openai".into(),
            name: "gpt-5".into(),
            active: Range {
                min: 30.0,
                max: 90.0,
            },
            total: Range::one(300.0),
            tps: Some(59.0),
        };
        let p = provider(&m.provider).unwrap();
        let i = request(&m, &p, p.grid_kg_per_kwh, 200.0, 3.02, Some(5.0));
        assert!(
            close(i.energy_kwh.min, 0.000_283_728_920_625_442_8),
            "{i:?}"
        );
        assert!(close(i.gwp_kg().max, 0.000_172_516_804_084_964_46), "{i:?}");
        assert!(close(i.water_l.min, 0.001_023_202_148_820_846_9), "{i:?}");
    }

    #[test]
    fn dated_and_suffixed_ids_find_their_model() {
        assert_eq!(find("claude-opus-5-5[1m]").unwrap().name, "claude-opus-5-5");
        assert_eq!(
            find("claude-haiku-4-5").unwrap().name,
            "claude-haiku-4-5-20251001"
        );
        assert!(find("claude-3-7-sonnet-20250219").is_none());
    }
}
