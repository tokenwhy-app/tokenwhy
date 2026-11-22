// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use crate::args::{Args, WindowArg, DAY_MS};
use crate::run;
use tokenwhy_core::pipeline::{self, Pipeline, RootSpec};
use tokenwhy_core::Snapshot;
use tokenwhy_mcp::{Request, Server, Source};

struct PipeSource {
    pipe: Pipeline,
    roots: Vec<RootSpec>,
    args: Args,
}

impl Source for PipeSource {
    fn snapshot(&mut self, req: &Request) -> Result<Snapshot, String> {
        self.pipe
            .scan(&self.roots, &|_: f64| {})
            .map_err(|e| format!("scan failed: {e}"))?;
        if let Err(e) = self.pipe.save_cache() {
            eprintln!("tokenwhy: cache write failed: {e}");
        }
        let settings = run::settings(&self.args, &self.pipe);
        let now = pipeline::now_ms();
        let window = req
            .window_days
            .map(|d| WindowArg(Some(i64::from(d) * DAY_MS)))
            .or(self.args.window);
        let query = run::query(
            &settings,
            window,
            req.project.clone().or_else(|| self.args.project.clone()),
            req.provider.clone().or_else(|| self.args.provider.clone()),
            self.args.anon,
            now,
        );
        Ok(self.pipe.report(&settings, &query, now).0)
    }
}

pub fn main(args: Args) -> Result<(), String> {
    let pipe = Pipeline::open(!args.no_cache);
    let roots = pipe.roots(&args.roots)?;
    let mut src = PipeSource { pipe, roots, args };
    Server::new(&mut src)
        .serve(std::io::stdin().lock(), std::io::stdout().lock())
        .map_err(|e| e.to_string())
}
