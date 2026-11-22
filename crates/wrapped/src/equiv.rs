// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Every value names its source and the day it was read.

pub struct Equiv {
    pub value: f64,
    pub one: &'static str,
    pub many: &'static str,
    pub part: &'static str,
    pub source: &'static str,
    pub url: &'static str,
    pub read: &'static str,
}

pub const MIN: f64 = 0.25;
pub const MAX: f64 = 300.0;

pub const CO2: &[Equiv] = &[
    // 1,037 kg: one way Paris to New York per passenger, ADEME Impact CO2 "Avion trajet long" over 5,837 km
    // (178 g CO2e per passenger km, contrails included), https://impactco2.fr/outils/transport/avion-pny, value
    // from https://impactco2.fr/api/v1/transport?km=5837&transports=1 (read 2026-10-23).
    Equiv {
        value: 1_037_000.0,
        one: "one-way Paris to New York flight",
        many: "one-way Paris to New York flights",
        part: "1 in {n} seats on a Paris to New York flight",
        source: "ADEME Impact CO2, long-haul flight, 178 g CO2e per passenger km over 5,837 km",
        url: "https://impactco2.fr/outils/transport/avion-pny",
        read: "2026-10-23",
    },
    // 4.97 kg: one meal with beef, ADEME Impact CO2, https://impactco2.fr/outils/alimentation (read 2026-10-23).
    Equiv {
        value: 4_970.0,
        one: "beef meal",
        many: "beef meals",
        part: "{f} of a beef meal",
        source: "ADEME Impact CO2, meal with beef",
        url: "https://impactco2.fr/outils/alimentation",
        read: "2026-10-23",
    },
    // 244 g/km: EPA, average gasoline passenger vehicle, 3.93e-4 t CO2e per mile,
    // https://www.epa.gov/energy/greenhouse-gas-equivalencies-calculator-revision-history (read 2026-10-23).
    Equiv {
        value: 244.0,
        one: "km in an average petrol car",
        many: "km in an average petrol car",
        part: "{f} of a km in a petrol car",
        source: "EPA, average gasoline passenger vehicle, 3.93e-4 t CO2e per mile",
        url: "https://www.epa.gov/energy/greenhouse-gas-equivalencies-calculator-revision-history",
        read: "2026-10-23",
    },
    // 55 g: one hour of video streaming in Europe, Carbon Trust 2021,
    // https://digitaltveurope.com/2021/06/11/one-hour-of-video-streaming-equivalent-to-boiling-a-kettle-three-times-claims-environmental-report
    // (read 2026-10-23).
    Equiv {
        value: 55.0,
        one: "hour of video streaming",
        many: "hours of video streaming",
        part: "{f} of an hour of streaming",
        source: "Carbon Trust 2021, one hour of video streaming in Europe",
        url: "https://digitaltveurope.com/2021/06/11/one-hour-of-video-streaming-equivalent-to-boiling-a-kettle-three-times-claims-environmental-report",
        read: "2026-10-23",
    },
    // 12.4 g: one full smartphone charge (iPhone 16), EPA, 1.24e-5 t CO2, same revision history page (read 2026-10-23).
    Equiv {
        value: 12.4,
        one: "full smartphone charge",
        many: "full smartphone charges",
        part: "{f} of a smartphone charge",
        source: "EPA, one full smartphone charge, 1.24e-5 t CO2",
        url: "https://www.epa.gov/energy/greenhouse-gas-equivalencies-calculator-revision-history",
        read: "2026-10-23",
    },
];

#[cfg(feature = "co2")]
pub static WATER: std::sync::LazyLock<Vec<Equiv>> = std::sync::LazyLock::new(|| {
    #[allow(clippy::ptr_arg)]
    fn s(c: &std::borrow::Cow<'static, str>) -> &'static str {
        match c {
            std::borrow::Cow::Borrowed(s) => s,
            std::borrow::Cow::Owned(_) => "",
        }
    }
    tokenwhy_core::co2::WATER_UNITS
        .iter()
        .map(|u| {
            let one = s(&u.one);
            let a = if one.starts_with(['a', 'e', 'i', 'o', 'u', 'A', 'E', 'I', 'O', 'U']) {
                "an"
            } else {
                "a"
            };
            Equiv {
                value: u.litres,
                one,
                many: s(&u.many),
                part: Box::leak(format!("{{f}} of {a} {one}").into_boxed_str()),
                source: s(&u.source),
                url: s(&u.url),
                read: s(&u.read),
            }
        })
        .collect()
});

/// 0.75 words per token: OpenAI, "What are tokens and how to count them?",
/// https://help.openai.com/en/articles/4936856-what-are-tokens-and-how-to-count-them (read 2026-10-23).
pub const WORDS_PER_TOKEN: f64 = 0.75;
const WORDS_PER_TOKEN_SOURCE: (&str, &str) = (
    "OpenAI, What are tokens and how to count them?",
    "https://help.openai.com/en/articles/4936856-what-are-tokens-and-how-to-count-them",
);

pub const WORDS: &[Equiv] = &[
    // 5.29 billion words: Wikipedia:Size of Wikipedia, https://en.wikipedia.org/wiki/Wikipedia:Size_of_Wikipedia
    // ("over 5.29 billion words", read 2026-10-23).
    Equiv {
        value: 5.29e9,
        one: "English Wikipedia",
        many: "times all of English Wikipedia",
        part: "{f} of all of English Wikipedia",
        source: "Wikipedia:Size of Wikipedia, over 5.29 billion words",
        url: "https://en.wikipedia.org/wiki/Wikipedia:Size_of_Wikipedia",
        read: "2026-10-23",
    },
    // 44 million words: last print Britannica (15th edition, 2010, 32 volumes), Guinness World Records,
    // https://www.guinnessworldrecords.com/world-records/67347-largest-encyclopedia-english (read 2026-10-23).
    Equiv {
        value: 44e6,
        one: "printed Encyclopaedia Britannica",
        many: "printed Encyclopaedia Britannicas",
        part: "{f} of the printed Encyclopaedia Britannica",
        source: "Guinness World Records, last print Britannica, 44 million words",
        url:
            "https://www.guinnessworldrecords.com/world-records/67347-largest-encyclopedia-english",
        read: "2026-10-23",
    },
    // 563,286 words: War and Peace, Maude translation, Project Gutenberg ebook #2600, counted as
    // whitespace-separated words between the START and END markers of
    // https://www.gutenberg.org/cache/epub/2600/pg2600.txt (counted 2026-10-23).
    Equiv {
        value: 563_286.0,
        one: "War and Peace",
        many: "War and Peaces",
        part: "{f} of War and Peace",
        source: "War and Peace, Project Gutenberg #2600, words counted",
        url: "https://www.gutenberg.org/cache/epub/2600/pg2600.txt",
        read: "2026-10-23",
    },
    // 127,360 words: Pride and Prejudice, Project Gutenberg ebook #1342, counted as whitespace-separated words
    // between the START and END markers of https://www.gutenberg.org/cache/epub/1342/pg1342.txt (counted 2026-10-23).
    Equiv {
        value: 127_360.0,
        one: "Pride and Prejudice",
        many: "Pride and Prejudices",
        part: "{f} of Pride and Prejudice",
        source: "Pride and Prejudice, Project Gutenberg #1342, words counted",
        url: "https://www.gutenberg.org/cache/epub/1342/pg1342.txt",
        read: "2026-10-23",
    },
];

pub const USD: &[Equiv] = &[
    // $20: Claude Pro, billed monthly, https://claude.com/pricing (read 2026-10-23).
    Equiv {
        value: 20.0,
        one: "month of Claude Pro",
        many: "months of Claude Pro",
        part: "{f} of a month of Claude Pro",
        source: "Claude Pro, billed monthly",
        url: "https://claude.com/pricing",
        read: "2026-10-23",
    },
    // $3.52: median regular hot coffee in the US, Toast, August 2025,
    // https://pos.toasttab.com/blog/data/coffee-prices-trends-by-state (read 2026-10-23).
    Equiv {
        value: 3.52,
        one: "cup of coffee",
        many: "cups of coffee",
        part: "{f} of a cup of coffee",
        source: "Toast, median regular hot coffee in the US, August 2025",
        url: "https://pos.toasttab.com/blog/data/coffee-prices-trends-by-state",
        read: "2026-10-23",
    },
];

pub fn pick(amount: f64, table: &'static [Equiv]) -> Option<(&'static Equiv, f64)> {
    if amount <= 0.0 {
        return None;
    }
    let count = |e: &Equiv| amount / e.value;
    if let Some(e) = table.iter().find(|e| (MIN..=MAX).contains(&count(e))) {
        return Some((e, count(e)));
    }
    let off = |c: f64| {
        if c < MIN {
            (MIN / c).ln()
        } else {
            (c / MAX).ln()
        }
    };
    table
        .iter()
        .min_by(|a, b| off(count(a)).total_cmp(&off(count(b))))
        .map(|e| (e, count(e)))
}

fn number(c: f64) -> String {
    if c > 3.0 && c < 10.0 {
        format!("{:.0}", c.round())
    } else if c <= 3.0 {
        let r = (c * 10.0).round() / 10.0;
        if r.fract() == 0.0 {
            format!("{r:.0}")
        } else {
            format!("{r:.1}")
        }
    } else {
        let p = 10f64.powi(c.log10().floor() as i32 - 1);
        format!("{:.0}", (c / p).round() * p)
    }
}

pub fn phrase(e: &Equiv, count: f64) -> String {
    if count >= 0.95 {
        let n = number(count);
        if n == "1" {
            format!("about one {}", e.one)
        } else {
            format!("about {n} {}", e.many)
        }
    } else {
        let d = (1.0 / count).round().max(2.0) as u32;
        let f = match d {
            2 => "half".to_string(),
            3 => "a third".to_string(),
            4 => "a quarter".to_string(),
            5 => "a fifth".to_string(),
            d => format!("1/{d}"),
        };
        format!(
            "about {}",
            e.part.replace("{n}", &d.to_string()).replace("{f}", &f)
        )
    }
}

pub fn line(amount: f64, table: &'static [Equiv]) -> Option<String> {
    pick(amount, table).map(|(e, c)| format!("That is {}.", phrase(e, c)))
}

pub fn sources() -> Vec<tokenwhy_core::about::Section> {
    use tokenwhy_core::about::{Entry, Section};
    let rows = |t: &[Equiv]| -> Vec<Entry> {
        t.iter()
            .map(|e| Entry {
                name: e.one.to_string(),
                detail: e.source.to_string(),
                url: e.url.into(),
                read: Some(e.read.into()),
            })
            .collect()
    };
    let mut words = vec![Entry {
        name: "Words per token".into(),
        detail: WORDS_PER_TOKEN_SOURCE.0.to_string(),
        url: WORDS_PER_TOKEN_SOURCE.1.into(),
        read: Some("2026-10-23".into()),
    }];
    words.extend(rows(WORDS));
    vec![Section {
        title: "Wrapped equivalents".into(),
        entries: [rows(CO2), words, rows(USD)].concat(),
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    // Factors pinned to the sources named on each table entry.
    #[test]
    fn factors_match_their_sources() {
        assert_eq!(CO2[0].value, (1_036.651_2_f64).round() * 1e3);
        assert_eq!(WORDS[2].value, 563_286.0);
        assert_eq!(CO2[2].value, (3.93e-4 * 1e6 / 1.609_344_f64).round());
        assert_eq!(CO2[4].value, 1.24e-5 * 1e6);
        assert_eq!(WORDS[0].value, 5.29e9);
        assert_eq!(USD[0].value, 20.0);
    }

    // Counts worked by hand from the sourced volumes.
    #[cfg(feature = "co2")]
    #[test]
    fn water_ladder_steps() {
        let w = |l: f64| line(l, &WATER).unwrap();
        assert_eq!(w(5e6), "That is about 2 Olympic swimming pools.");
        assert_eq!(
            w(720.0),
            "That is about 5 days of one person's water at home."
        );
        assert_eq!(w(30.0), "That is about 120 glasses of water.");
        assert_eq!(w(1.0), "That is about 4 glasses of water.");
        assert_eq!(w(0.005), "That is about 100 drops.");
    }

    #[test]
    fn pick_lands_in_a_readable_range() {
        assert_eq!(
            line(300_000.0, CO2).unwrap(),
            "That is about 1 in 3 seats on a Paris to New York flight."
        );
        assert_eq!(
            line(1_600.0, CO2).unwrap(),
            "That is about a third of a beef meal."
        );
        assert_eq!(
            line(23.7e9 * WORDS_PER_TOKEN, WORDS).unwrap(),
            "That is about 3 times all of English Wikipedia."
        );
        assert_eq!(
            line(5_000.0, USD).unwrap(),
            "That is about 250 months of Claude Pro."
        );
        assert_eq!(
            line(0.5, USD).unwrap(),
            "That is about 1/7 of a cup of coffee."
        );
    }
}
