// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use crate::model::{ModelRate, PriceTable, ProviderId, Usage, Vendor};
use std::sync::OnceLock;

pub fn parse_table(json: &str) -> serde_json::Result<PriceTable> {
    serde_json::from_str(json)
}

/// `None` means cost "n/a", never a default rate.
pub fn rate_for<'a>(table: &'a PriceTable, model: &str) -> Option<&'a ModelRate> {
    table
        .rates
        .iter()
        .filter(|r| model.starts_with(r.prefix.as_str()))
        .max_by_key(|r| r.prefix.len())
}

/// The 1h write rate is read from the table, not derived: the price page lists each token class.
pub fn cost(rate: &ModelRate, usage: &Usage) -> f64 {
    cost_by_kind(rate, usage).iter().sum::<f64>()
        + usage.web_search_requests as f64 * rate.web_search
}

pub fn cost_by_kind(rate: &ModelRate, usage: &Usage) -> [f64; 4] {
    let prompt = usage
        .input
        .saturating_add(usage.cache_write_5m)
        .saturating_add(usage.cache_write_1h)
        .saturating_add(usage.cache_read);
    let r = match (&rate.long_context, rate.long_context_threshold) {
        (Some(lc), Some(t)) if prompt > t => lc.as_ref(),
        _ => rate,
    };
    [
        usage.input as f64 * r.input / 1e6,
        (usage.cache_write_5m as f64 * r.cache_write_5m
            + usage.cache_write_1h as f64 * r.cache_write_1h)
            / 1e6,
        usage.cache_read as f64 * r.cache_read / 1e6,
        usage.output as f64 * r.output / 1e6,
    ]
}

pub fn vendor_table(v: Vendor) -> &'static PriceTable {
    static TABLES: OnceLock<[PriceTable; 3]> = OnceLock::new();
    let t = TABLES.get_or_init(|| {
        let p = |json: &str| parse_table(json).expect("built-in vendor prices are valid");
        [
            p(include_str!("prices/anthropic.json")),
            p(include_str!("prices/openai.json")),
            p(include_str!("prices/google.json")),
        ]
    });
    &t[match v {
        Vendor::Anthropic => 0,
        Vendor::OpenAi => 1,
        Vendor::Google => 2,
    }]
}

pub fn provider_table(provider: ProviderId, vendors: &[Vendor]) -> PriceTable {
    let tables: Vec<&PriceTable> = vendors.iter().map(|v| vendor_table(*v)).collect();
    PriceTable {
        version: tables.iter().map(|t| t.version).sum(),
        provider,
        rates: tables
            .iter()
            .flat_map(|t| t.rates.iter().cloned())
            .collect(),
    }
}
