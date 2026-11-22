//! Third-party CO2 code and data used by `tokenwhy-core` (feature `co2`).
//! - `ecologits`: port of EcoLogits 0.11.3, MPL-2.0, github.com/mlco2/ecologits.
//! - `JEGHAM_JSON`: emission factors of claude-carbon, MIT, github.com/gwittebolle/claude-carbon.

pub mod ecologits;

pub const JEGHAM_JSON: &str = include_str!("jegham.json");
