// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use crate::args::{Args, Command, WindowArg, DAY_MS};
use std::io::{IsTerminal, Write};
use tokenwhy_core::metrics::local_midnight;
use tokenwhy_core::model::{AnonLevel, ProviderId, Query, Settings};
use tokenwhy_core::pipeline::{self, Pipeline};
use tokenwhy_core::{anon, render};

pub fn run(args: Args) -> Result<(), String> {
    let mut pipe = Pipeline::open(!args.no_cache);
    if let Some(file) = &args.import {
        let n = pipe.import_ccusage(file)?;
        println!("{n} rows imported from {}", file.display());
        return Ok(());
    }
    let roots = pipe.roots(&args.roots)?;

    let tty = std::io::stderr().is_terminal();
    let progress = move |f: f64| {
        if tty {
            eprint!("\rscanning {:3.0}%", f * 100.0);
        }
    };
    let started = std::time::Instant::now();
    let n_files = pipe
        .scan(&roots, &progress)
        .map_err(|e| format!("scan failed: {e}"))?;
    if tty {
        eprint!("\r              \r");
    }
    pipe.save_cache()
        .map_err(|e| format!("cache write failed: {e}"))?;
    if args.stats {
        let s = pipe.stats();
        eprintln!(
            "store: {} files, {} sessions, {} hot; RAM: index {} KB, summaries {} KB, id index {} KB ({} ids), hot records {} KB",
            s.files,
            s.sessions,
            s.hot_sessions,
            s.index_bytes / 1024,
            s.summary_bytes / 1024,
            s.id_index_bytes / 1024,
            s.ids,
            s.hot_bytes / 1024
        );
        eprintln!(
            "summary fields (KB): {}",
            s.summary_fields
                .iter()
                .map(|(n, b)| format!("{n} {}", b / 1024))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    let mut out = std::io::stdout().lock();
    if args.command == Command::Scan {
        for r in &roots {
            let id = pipe.providers()[r.provider].id();
            let n = pipe.file_count(r.provider);
            writeln!(out, "{}: {} files under {}", id.0, n, r.path.display()).ok();
        }
        writeln!(
            out,
            "{} files in {:.1}s",
            n_files,
            started.elapsed().as_secs_f64()
        )
        .ok();
        return Ok(());
    }

    let settings = settings(&args, &pipe);
    let now = pipeline::now_ms();
    if args.command == Command::History {
        let len = args.window.map_or(Some(90 * DAY_MS), |w| w.0);
        let from = len.map_or(0, |l| local_midnight(now - l + DAY_MS));
        let mut rows = pipe.history().daily(
            from,
            now + DAY_MS,
            args.by,
            args.project.as_deref(),
            args.provider.as_deref(),
        );
        anon::anonymise_daily(
            &mut rows,
            &anon::Options {
                level: args.anon,
                salt: settings.anon_salt,
            },
        );
        let text = render::history(&rows, args.format);
        out.write_all(text.as_bytes()).or_else(closed_pipe_ok)?;
        if args.stats {
            eprintln!(
                "history: {} bytes, {:.2}s",
                pipe.history().file_bytes(),
                started.elapsed().as_secs_f64()
            );
        }
        return Ok(());
    }
    let query = query(
        &settings,
        args.window,
        args.project.clone(),
        args.provider.clone(),
        args.anon,
        now,
    );
    let color = !args.for_agent && crate::color::enabled(args.color);
    let (snap, _aliases) = pipe.report(&settings, &query, now);
    #[cfg(feature = "agent")]
    let text = if args.for_agent {
        use tokenwhy_core::agent;
        let b = agent::brief(
            &snap,
            &agent::Filter {
                limit: agent::DEFAULT_LIMIT,
                ..Default::default()
            },
        );
        if args.format == render::Format::Json {
            serde_json::to_string_pretty(&b).map_err(|e| e.to_string())?
        } else {
            agent::markdown(&b)
        }
    } else {
        render::render_styled(
            &snap,
            args.format,
            args.section.as_deref().or(args.command.section()),
            args.command == Command::Report && !args.full,
            color,
        )
    };
    #[cfg(not(feature = "agent"))]
    let text = render::render_styled(
            &snap,
            args.format,
            args.section.as_deref().or(args.command.section()),
            args.command == Command::Report && !args.full,
            color,
        );
    out.write_all(text.as_bytes()).or_else(closed_pipe_ok)?;
    if !text.ends_with('\n') {
        out.write_all(b"\n").ok();
    }
    Ok(())
}

pub fn settings(args: &Args, pipe: &Pipeline) -> Settings {
    #[allow(unused_mut)]
    let mut settings = Settings::default();
    #[cfg(feature = "co2")]
    {
        settings.co2 = args.co2.clone();
        pipe.history().set_co2(&settings.co2);
    }
    #[cfg(not(feature = "co2"))]
    let _ = (args, pipe);
    settings
}

pub fn query(
    settings: &Settings,
    window: Option<WindowArg>,
    project: Option<String>,
    provider: Option<String>,
    anon: AnonLevel,
    now: i64,
) -> Query {
    let window_len = match window {
        Some(w) => w.0,
        None => Some(i64::from(settings.window_days) * DAY_MS),
    };
    let mut query = pipeline::query(window_len, now, project, anon);
    query.provider = provider.map(ProviderId);
    query
}

fn closed_pipe_ok(e: std::io::Error) -> Result<(), String> {
    if e.kind() == std::io::ErrorKind::BrokenPipe {
        Ok(())
    } else {
        Err(e.to_string())
    }
}
