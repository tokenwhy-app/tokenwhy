// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use chrono::{Datelike, NaiveDate};
use lexopt::prelude::*;
use std::path::PathBuf;
use tokenwhy_core::pipeline::{self, Pipeline};
use tokenwhy_wrapped::svg::Palette;
use tokenwhy_wrapped::{cards, svg, Period};

pub const HELP: &str = "\
tokenwhy wrapped: your year (or any range of 7 days or more) as 12 cards.

USAGE
  tokenwhy wrapped [options]

OPTIONS
  --year <N>               calendar year (default: this year so far)
  --from <YYYY-MM-DD> --to <YYYY-MM-DD>
                           any range, both days included, 7 days or more
  --provider <id>|all      agent to count (default claude-code)
  --account <id>           keep one account
  --png [<dir>]            write one PNG per card (default <Pictures>/tokenwhy-wrapped/<period>)
  --json                   print the computed figures as JSON
  --about                  print how the numbers are counted and their sources
  --names                  show project names instead of project1, project2, project3
  --theme light|dark       card colours (default light)
  --root [<provider>=]<path>, --no-cache   as for the other commands
";

struct Opts {
    year: Option<i32>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    provider: Option<String>,
    account: Option<String>,
    png: Option<Option<PathBuf>>,
    json: bool,
    about: bool,
    names: bool,
    dark: bool,
    roots: Vec<(Option<String>, PathBuf)>,
    no_cache: bool,
}

fn val(p: &mut lexopt::Parser) -> Result<String, String> {
    p.value()
        .map_err(|e| e.to_string())?
        .string()
        .map_err(|e| e.to_string())
}

fn day(s: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| format!("bad date '{s}', use YYYY-MM-DD"))
}

fn parse(argv: Vec<String>) -> Result<Option<Opts>, String> {
    let mut p = lexopt::Parser::from_args(argv);
    let mut o = Opts {
        year: None,
        from: None,
        to: None,
        provider: Some("claude-code".into()),
        account: None,
        png: None,
        json: false,
        about: false,
        names: false,
        dark: false,
        roots: vec![],
        no_cache: false,
    };
    while let Some(arg) = p.next().map_err(|e| e.to_string())? {
        match arg {
            Short('h') | Long("help") => return Ok(None),
            Long("year") => {
                let v = val(&mut p)?;
                o.year = Some(v.parse().map_err(|_| format!("bad year '{v}'"))?);
            }
            Long("from") => o.from = Some(day(&val(&mut p)?)?),
            Long("to") => o.to = Some(day(&val(&mut p)?)?),
            Long("provider") => {
                let v = val(&mut p)?;
                o.provider = (v != "all").then_some(v);
            }
            Long("account") => o.account = Some(val(&mut p)?),
            Long("png") => {
                o.png = Some(
                    p.optional_value()
                        .map(|v| v.string().map(PathBuf::from).map_err(|e| e.to_string()))
                        .transpose()?,
                )
            }
            Long("json") => o.json = true,
            Long("about") => o.about = true,
            Long("names") => o.names = true,
            Long("theme") => {
                o.dark = match val(&mut p)?.as_str() {
                    "light" => false,
                    "dark" => true,
                    v => return Err(format!("bad --theme '{v}', use light or dark")),
                }
            }
            Long("root") => {
                let v = val(&mut p)?;
                match v.split_once('=') {
                    Some((id, path)) if !id.is_empty() => {
                        o.roots.push((Some(id.to_string()), path.into()))
                    }
                    _ => o.roots.push((None, v.into())),
                }
            }
            Long("no-cache") => o.no_cache = true,
            Value(v) if o.png == Some(None) => {
                o.png = Some(Some(v.string().map_err(|e| e.to_string())?.into()))
            }
            other => return Err(other.unexpected().to_string()),
        }
    }
    Ok(Some(o))
}

fn period(o: &Opts, today: NaiveDate) -> Result<Period, String> {
    match (o.year, o.from, o.to) {
        (Some(_), Some(_), _) | (Some(_), _, Some(_)) => {
            Err("use --year or --from/--to, not both".into())
        }
        (Some(y), None, None) => Period::year(y),
        (None, Some(f), Some(t)) => Period::range(f, t),
        (None, Some(f), None) => Period::range(f, today),
        (None, None, Some(_)) => Err("--to needs --from".into()),
        (None, None, None) => Period::year(today.year()),
    }
}

fn pictures() -> Option<PathBuf> {
    let var = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let base = if cfg!(windows) {
        var("USERPROFILE")?.join("Pictures")
    } else {
        var("XDG_PICTURES_DIR").or_else(|| var("HOME").map(|h| h.join("Pictures")))?
    };
    Some(base.join("tokenwhy-wrapped"))
}

pub fn main(argv: Vec<String>) -> i32 {
    match run(argv) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("tokenwhy: {e}");
            1
        }
    }
}

fn run(argv: Vec<String>) -> Result<(), String> {
    let Some(o) = parse(argv)? else {
        print!("{HELP}");
        return Ok(());
    };
    let now = pipeline::now_ms();
    let per = period(&o, tokenwhy_wrapped::date_of(now))?;
    let mut pipe = Pipeline::open(!o.no_cache);
    let roots = pipe.roots(&o.roots)?;
    pipe.scan(&roots, &|_| {})
        .map_err(|e| format!("scan failed: {e}"))?;
    pipe.save_cache()
        .map_err(|e| format!("cache write failed: {e}"))?;
    let w = tokenwhy_wrapped::compute(
        pipe.history(),
        &per,
        o.provider.as_deref(),
        o.account.as_deref(),
        o.names,
        now,
    );
    let cards = cards::cards(&w);
    if o.about {
        for l in cards::about(&w) {
            println!("{l}");
        }
    } else if o.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&w).map_err(|e| e.to_string())?
        );
    } else {
        for (i, c) in cards.iter().enumerate() {
            println!("{:>2}. {}\n", i + 1, c.text());
        }
    }
    if let Some(dir) = o.png {
        let label = match per.year {
            Some(y) => y.to_string(),
            None => format!("{}_{}", per.from, per.to.pred_opt().unwrap_or(per.to)),
        };
        let dir = match dir {
            Some(d) => d,
            None => pictures()
                .ok_or("no Pictures folder; give --png <dir>")?
                .join(label),
        };
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let pal = if o.dark {
            Palette::dark()
        } else {
            Palette::light()
        };
        for (i, c) in cards.iter().enumerate() {
            let png = tokenwhy_card::png(
                &svg::render(c, i + 1, cards.len(), &pal, None),
                &tokenwhy_wrapped::FONTS,
            )?;
            let f = dir.join(format!("{:02}-{}.png", i + 1, c.id));
            std::fs::write(&f, png).map_err(|e| format!("{}: {e}", f.display()))?;
        }
        eprintln!("{} cards written to {}", cards.len(), dir.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(v: &[&str]) -> Opts {
        parse(v.iter().map(|s| s.to_string()).collect())
            .unwrap()
            .unwrap()
    }

    #[test]
    fn period_flags() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 23).unwrap();
        let p = period(&opts(&[]), today).unwrap();
        assert_eq!(
            (p.from.to_string(), p.to.to_string()),
            ("2026-01-01".into(), "2027-01-01".into())
        );
        let p = period(
            &opts(&["--from", "2026-03-01", "--to", "2026-03-31"]),
            today,
        )
        .unwrap();
        assert_eq!(p.to.to_string(), "2026-04-01");
        assert!(period(
            &opts(&["--from", "2026-03-01", "--to", "2026-03-03"]),
            today
        )
        .is_err());
        assert!(period(&opts(&["--year", "2025", "--from", "2025-03-01"]), today).is_err());
        let o = opts(&["--provider", "all", "--png", "--theme", "dark"]);
        assert_eq!((o.provider, o.png, o.dark), (None, Some(None), true));
        assert_eq!(
            opts(&["--png", "out"]).png,
            Some(Some(PathBuf::from("out")))
        );
    }
}
