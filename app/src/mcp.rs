// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! `tokenwhy.exe mcp`: the installer ships only the app exe, so it serves the MCP server itself, before any window.
//! Same pipeline as the CLI's `tokenwhy mcp`, with default settings; stdout carries JSON-RPC only.

use tokenwhy_core::model::{AnonLevel, ProviderId, Settings};
use tokenwhy_core::pipeline::{self, Pipeline, RootSpec};
use tokenwhy_core::Snapshot;
use tokenwhy_mcp::{Request, Server, Source};

const DAY_MS: i64 = 86_400_000;

struct PipeSource {
    pipe: Pipeline,
    roots: Vec<RootSpec>,
}

impl Source for PipeSource {
    fn snapshot(&mut self, req: &Request) -> Result<Snapshot, String> {
        self.pipe
            .scan(&self.roots, &|_: f64| {})
            .map_err(|e| format!("scan failed: {e}"))?;
        if let Err(e) = self.pipe.save_cache() {
            eprintln!("tokenwhy: cache write failed: {e}");
        }
        let settings = Settings::default();
        let now = pipeline::now_ms();
        let days = req.window_days.unwrap_or(settings.window_days);
        let mut query = pipeline::query(
            Some(i64::from(days) * DAY_MS),
            now,
            req.project.clone(),
            AnonLevel::Off,
        );
        query.provider = req.provider.clone().map(ProviderId);
        Ok(self.pipe.report(&settings, &query, now).0)
    }
}

pub fn run_if_asked() -> bool {
    if std::env::args().nth(1).as_deref() != Some("mcp") {
        return false;
    }
    let pipe = Pipeline::open(true);
    let code = match pipe.roots(&[]) {
        Ok(roots) => {
            let mut src = PipeSource { pipe, roots };
            match Server::new(&mut src).serve(std::io::stdin().lock(), std::io::stdout().lock()) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("tokenwhy: {e}");
                    1
                }
            }
        }
        Err(e) => {
            eprintln!("tokenwhy: {e}");
            1
        }
    };
    std::process::exit(code);
}
