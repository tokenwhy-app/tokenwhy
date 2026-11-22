// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Entry {
    pub name: String,
    pub detail: String,
    pub url: String,
    pub read: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Section {
    pub title: String,
    pub entries: Vec<Entry>,
}

/// Every row is credited in NOTICE.
const THIRD_PARTY: &[(&str, &str, &str, &str)] = &[
    (
        "codeburn",
        "MIT, AgentSeal",
        "https://github.com/getagentseal/codeburn",
        "parsing rules and provider formats",
    ),
    (
        "ccusage",
        "MIT, ryoppippi",
        "https://github.com/ryoppippi/ccusage",
        "default data folders",
    ),
    (
        "claude-carbon",
        "MIT, Gaetan Wittebolle",
        "https://github.com/gwittebolle/claude-carbon",
        "CO2 factors per model family",
    ),
    (
        "EcoLogits",
        "MPL-2.0, GenAI Impact",
        "https://github.com/mlco2/ecologits",
        "CO2 formula and model sizes, ported to Rust",
    ),
    (
        "SQLite",
        "public domain",
        "https://sqlite.org",
        "compiled into the binary",
    ),
    (
        "rusqlite",
        "MIT",
        "https://github.com/rusqlite/rusqlite",
        "SQLite bindings",
    ),
    (
        "Bricolage Grotesque",
        "SIL Open Font License 1.1, The Bricolage Grotesque Project Authors",
        "https://github.com/ateliertriay/bricolage",
        "Wrapped card font",
    ),
];

#[cfg(feature = "co2")]
fn date_in(s: &str) -> Option<String> {
    let b = s.as_bytes();
    (0..b.len().saturating_sub(9)).find_map(|i| {
        let d = &s[i..i + 10];
        let ok = d.bytes().enumerate().all(|(j, c)| {
            if j == 4 || j == 7 {
                c == b'-'
            } else {
                c.is_ascii_digit()
            }
        });
        ok.then(|| d.to_string())
    })
}

fn prices() -> Vec<Entry> {
    [
        include_str!("prices/anthropic.json"),
        include_str!("prices/openai.json"),
        include_str!("prices/google.json"),
    ]
    .iter()
    .map(|json| {
        let v: serde_json::Value =
            serde_json::from_str(json).expect("built-in price table is JSON");
        let s = |k: &str| v[k].as_str().unwrap_or_default().to_string();
        Entry {
            name: format!("{} prices", s("provider")),
            detail: "API list prices per million tokens.".into(),
            url: s("source_url"),
            read: Some(s("source_checked")),
        }
    })
    .collect()
}

#[cfg(feature = "co2")]
fn co2() -> Vec<Entry> {
    let t = &*crate::co2::JEGHAM;
    let read = date_in(&t.source);
    let e = |name: &str, detail: &str, url: &str, read: &str| Entry {
        name: name.into(),
        detail: detail.into(),
        url: url.into(),
        read: Some(read.into()),
    };
    vec![
        e(
            "EcoLogits",
            "Energy, CO2e and water per output token; low/high range.",
            "https://ecologits.ai/latest/methodology/llm_inference/",
            crate::co2::ecologits::DATA_DATE,
        ),
        Entry {
            name: "claude-carbon".into(),
            detail: "CO2 factor per model family, from Jegham et al. 2025.".into(),
            url: "https://github.com/gwittebolle/claude-carbon/blob/main/data/factors.json".into(),
            read,
        },
    ]
    .into_iter()
    .chain(crate::co2::WATER_UNITS.iter().map(|u| Entry {
        name: format!("Water equivalent: {}", u.one),
        detail: u.source.to_string(),
        url: u.url.to_string(),
        read: Some(u.read.to_string()),
    }))
    .collect()
}

pub fn sections(extra: Vec<Section>) -> Vec<Section> {
    let third = THIRD_PARTY
        .iter()
        .map(|(name, licence, url, what)| Entry {
            name: (*name).into(),
            detail: format!("{licence}; {what}"),
            url: (*url).into(),
            read: None,
        })
        .collect();
    #[allow(unused_mut)]
    let mut out = vec![
        Section {
            title: "Third-party code and assets".into(),
            entries: third,
        },
        Section {
            title: "Prices".into(),
            entries: prices(),
        },
    ];
    #[cfg(feature = "co2")]
    out.push(Section {
        title: "CO2 estimates".into(),
        entries: co2(),
    });
    #[cfg(feature = "claude-stats")]
    out.push(Section {
        title: "History".into(),
        entries: vec![Entry {
            name: "Claude Code stats-cache.json".into(),
            detail: "Daily tokens for days without a transcript.".into(),
            url: "https://docs.anthropic.com/en/docs/claude-code".into(),
            read: None,
        }],
    });
    out.extend(extra);
    out
}

pub fn text(sections: &[Section]) -> String {
    let mut s = String::from("tokenwhy: sources and credits. Full licence texts are in NOTICE.\n");
    for sec in sections {
        s.push_str(&format!("\n{}\n", sec.title.to_uppercase()));
        for e in &sec.entries {
            s.push_str(&format!("  {}\n    {}\n    {}", e.name, e.detail, e.url));
            if let Some(d) = &e.read {
                s.push_str(&format!(" (read {d})"));
            }
            s.push('\n');
        }
    }
    s
}

pub fn is_listed(sections: &[Section], url: &str) -> bool {
    sections
        .iter()
        .flat_map(|s| &s.entries)
        .any(|e| e.url == url)
}

#[cfg(test)]
mod tests {
    use super::*;

    // NOTICE is the legal record; every credit shown must be one NOTICE carries, with the same licence.
    #[test]
    fn every_third_party_row_is_in_notice() {
        let notice = include_str!("../../../NOTICE");
        for (name, licence, url, _) in THIRD_PARTY {
            assert!(notice.contains(name), "{name} missing from NOTICE");
            let lic = licence.split(',').next().unwrap();
            assert!(notice.contains(lic), "{name}: licence {lic} not in NOTICE");
            let host = url.trim_start_matches("https://");
            if *name != "rusqlite" {
                assert!(notice.contains(host), "{name}: {host} not in NOTICE");
            }
        }
    }

    #[test]
    fn data_sources_carry_their_dates() {
        let s = sections(Vec::new());
        let p = &s[1].entries;
        assert_eq!(p.len(), 3);
        assert_eq!(
            p[0].url,
            "https://platform.claude.com/docs/en/about-claude/pricing"
        );
        assert!(p
            .iter()
            .all(|e| e.read.as_deref().is_some_and(|d| d.starts_with("2026-"))));
        #[cfg(feature = "co2")]
        {
            assert_eq!(s[2].entries[0].read.as_deref(), Some("2026-10-23"));
            assert_eq!(s[2].entries[1].read.as_deref(), Some("2026-10-22"));
            assert!(is_listed(&s, "https://ecologits.ai/latest/methodology/llm_inference/"));
        }
        assert!(!is_listed(&s, "https://example.com"));
    }
}
