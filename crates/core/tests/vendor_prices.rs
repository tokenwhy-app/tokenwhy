// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Each expected cost is computed by hand from the vendor page named above it (read 2026-10-23).

use tokenwhy_core::price::{cost, provider_table, rate_for, vendor_table};
use tokenwhy_core::{ProviderId, Usage, Vendor};

fn usd(v: Vendor, model: &str, u: Usage) -> f64 {
    cost(rate_for(vendor_table(v), model).expect("model priced"), &u)
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}

fn usage(input: u64, cache_read: u64, cache_write: u64, output: u64) -> Usage {
    Usage {
        input,
        cache_read,
        cache_write_5m: cache_write,
        output,
        ..Usage::default()
    }
}

#[test]
fn openai_page_rates() {
    // developers.openai.com/api/docs/pricing, Standard tier.
    close(
        usd(Vendor::OpenAi, "gpt-5.4", usage(100_000, 50_000, 0, 4_000)),
        0.3225,
    );
    close(
        usd(
            Vendor::OpenAi,
            "gpt-5.4",
            usage(200_000, 100_000, 0, 10_000),
        ),
        1.275,
    );
    close(
        usd(
            Vendor::OpenAi,
            "gpt-5-mini-2025-08-07",
            usage(1_000_000, 1_000_000, 0, 1_000_000),
        ),
        2.275,
    );
}

#[test]
fn google_page_rates() {
    // ai.google.dev/gemini-api/docs/pricing, paid Standard tier.
    close(
        usd(
            Vendor::Google,
            "gemini-2.5-pro",
            usage(100_000, 50_000, 0, 2_000),
        ),
        0.15125,
    );
    close(
        usd(
            Vendor::Google,
            "gemini-2.5-pro",
            usage(150_000, 100_000, 0, 2_000),
        ),
        0.43,
    );
    close(
        usd(
            Vendor::Google,
            "gemini-3.8-flash",
            usage(1_000_000, 1_000_000, 0, 1_000_000),
        ),
        4.575,
    );
}

#[test]
fn anthropic_page_rates_and_vendor_mix() {
    // platform.claude.com pricing, checked 2026-10-21.
    close(
        usd(
            Vendor::Anthropic,
            "claude-opus-4-5-20251101",
            usage(10_000, 100_000, 20_000, 1_000),
        ),
        0.25,
    );
    let t = provider_table(
        ProviderId("multi".into()),
        &[Vendor::Anthropic, Vendor::Google, Vendor::OpenAi],
    );
    let r = |m: &str| tokenwhy_core::price::rate_for(&t, m).map(|r| r.output);
    assert_eq!(
        (r("claude-opus-4-5"), r("gemini-2.5-flash"), r("gpt-4.1")),
        (Some(25.0), Some(2.5), Some(8.0))
    );
    assert_eq!(r("gemini-3.7-flash"), None);
}
