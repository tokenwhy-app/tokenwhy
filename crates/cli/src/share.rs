// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! The CLI never sends; this build links no HTTP client.

use lexopt::prelude::*;
use std::path::PathBuf;
use tokenwhy_core::pipeline::{self, Pipeline};
use tokenwhy_core::Settings;
use tokenwhy_share::{payload, Week};

pub const HELP: &str = "\
tokenwhy share: the anonymous weekly summary the app sends when sharing is on, for the last complete week.

USAGE
  tokenwhy share preview          print it
  tokenwhy share export <file>    write it to <file>

Every value is rounded into a fixed range; no project, file, skill or server name is in it. Its `install` field
reads \"not-shared\": the CLI has no id and sends nothing.

OPTIONS
  --root [<provider>=]<path>, --no-cache   as for the other commands
";

pub fn main(argv: Vec<String>) -> i32 {
    match run(argv) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("tokenwhy: {e}\n\n{HELP}");
            2
        }
    }
}

fn run(argv: Vec<String>) -> Result<(), String> {
    let mut p = lexopt::Parser::from_args(argv);
    let mut words: Vec<String> = vec![];
    let mut roots: Vec<(Option<String>, PathBuf)> = vec![];
    let mut no_cache = false;
    while let Some(arg) = p.next().map_err(|e| e.to_string())? {
        match arg {
            Short('h') | Long("help") => {
                print!("{HELP}");
                return Ok(());
            }
            Long("root") => {
                let v = p
                    .value()
                    .map_err(|e| e.to_string())?
                    .string()
                    .map_err(|e| e.to_string())?;
                match v.split_once('=') {
                    Some((id, path)) if !id.is_empty() => {
                        roots.push((Some(id.to_string()), path.into()))
                    }
                    _ => roots.push((None, v.into())),
                }
            }
            Long("no-cache") => no_cache = true,
            Value(v) => words.push(v.string().map_err(|e| e.to_string())?),
            _ => return Err(arg.unexpected().to_string()),
        }
    }
    let out = match words.as_slice() {
        [w] if w == "preview" => None,
        [w, f] if w == "export" => Some(PathBuf::from(f)),
        _ => return Err("use 'share preview' or 'share export <file>'".into()),
    };
    let mut pipe = Pipeline::open(!no_cache);
    let roots = pipe.roots(&roots)?;
    pipe.scan(&roots, &|_| {})
        .map_err(|e| format!("scan failed: {e}"))?;
    pipe.save_cache()
        .map_err(|e| format!("cache write failed: {e}"))?;
    let week = Week::last_complete(pipeline::now_ms());
    let (snap, _) = pipe.report(&Settings::default(), &payload::query(week), week.to_ms());
    let text = payload::build(&snap, pipe.history(), week, None).to_pretty();
    match out {
        None => println!("{text}"),
        Some(f) => {
            std::fs::write(&f, text + "\n").map_err(|e| format!("{}: {e}", f.display()))?;
            eprintln!("week {} written to {}", week.label(), f.display());
        }
    }
    Ok(())
}
