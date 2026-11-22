// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! A summary is never subtracted: a changed session's summary is replaced and the fold reruns, since medians and sets cannot be undone.

use super::startup::{session_parts, SessionParts};
use super::{
    compaction, ctx, live, memory, prepare, segments, skills, subagents, tools, FileCtx, NameKind,
};
use crate::model::{ParsedFile, PriceTable, ProviderId, TimestampMs};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSummary {
    pub(crate) provider: ProviderId,
    pub(crate) session: String,
    pub(crate) project: String,
    pub(crate) path: PathBuf,
    pub(crate) main: bool,
    pub(crate) agent_type: Option<String>,
    pub(crate) head_copied: bool,
    pub(crate) first_ts: Option<TimestampMs>,
    pub(crate) last_ts: Option<TimestampMs>,
    pub(crate) turns: u32,
    pub(crate) n_compactions: u32,
    pub(crate) first_ctx: Option<u64>,
    pub(crate) parts: Option<SessionParts>,
    pub(crate) skills: skills::Part,
    pub(crate) memory: memory::Part,
    pub(crate) sub: subagents::Part,
    pub(crate) compactions: Vec<compaction::Part>,
    pub(crate) segments: segments::Part,
    pub(crate) tools: tools::Part,
    pub(crate) live: live::Part,
    pub(crate) names: Vec<(NameKind, String)>,
    pub(crate) unpriced: Vec<String>,
    pub(crate) days: Vec<super::days::DayPart>,
    pub(crate) spend: super::spend::Part,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub provider: ProviderId,
    pub session: String,
    pub files: Vec<FileSummary>,
}

impl FileSummary {
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
    pub fn is_main(&self) -> bool {
        self.main
    }
    pub fn project(&self) -> &str {
        &self.project
    }
    pub fn first_ts(&self) -> Option<TimestampMs> {
        self.first_ts
    }
    pub fn last_ts(&self) -> Option<TimestampMs> {
        self.last_ts
    }
}

impl SessionSummary {
    pub fn roll(&mut self, midnight: TimestampMs) {
        self.files.iter_mut().for_each(|f| f.live.roll(midnight));
    }
    pub fn midnight(&self) -> TimestampMs {
        self.files
            .iter()
            .map(|f| f.live.midnight)
            .min()
            .unwrap_or(0)
    }
}

fn file(f: &FileCtx, now_ms: TimestampMs, unpriced: &BTreeSet<String>) -> FileSummary {
    let pf = f.pf;
    FileSummary {
        provider: f.provider().clone(),
        session: f.session().to_string(),
        project: f.project.clone(),
        path: pf.source.path.clone(),
        main: f.is_main(),
        agent_type: pf.source.agent_type.clone(),
        head_copied: pf.records.head_copied,
        first_ts: f.first_ts(),
        last_ts: f.last_ts(),
        turns: f.turns.len() as u32,
        n_compactions: pf.records.compactions.len() as u32,
        first_ctx: f.turns.first().map(|t| ctx(&t.usage)),
        parts: if f.is_main() { session_parts(f) } else { None },
        skills: skills::part(f),
        memory: memory::part(f),
        sub: subagents::part(f),
        compactions: if f.is_main() {
            compaction::parts(f)
        } else {
            vec![]
        },
        segments: if f.is_main() {
            segments::part(f)
        } else {
            Default::default()
        },
        tools: tools::part(f),
        live: live::part(f, now_ms),
        names: names(f),
        days: super::days::part(f),
        spend: super::spend::part(f),
        unpriced: f
            .turns
            .iter()
            .map(|t| f.model_of(t))
            .filter(|m| unpriced.contains(*m))
            .map(str::to_string)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
    }
}

pub fn summarize(
    files: &[&ParsedFile],
    prices: &[PriceTable],
    now_ms: TimestampMs,
) -> Vec<SessionSummary> {
    let mut unpriced = BTreeSet::new();
    let all = prepare(files, prices, &mut unpriced);
    let mut out: Vec<SessionSummary> = vec![];
    for f in &all {
        let s = file(f, now_ms, &unpriced);
        match out
            .iter_mut()
            .rev()
            .find(|x| x.provider == s.provider && x.session == s.session)
        {
            Some(x) => x.files.push(s),
            None => out.push(SessionSummary {
                provider: s.provider.clone(),
                session: s.session.clone(),
                files: vec![s],
            }),
        }
    }
    for s in &mut out {
        let mut names: BTreeSet<(NameKind, String)> = BTreeSet::new();
        let mut loaded: BTreeSet<(String, String, String)> = BTreeSet::new();
        for f in &mut s.files {
            f.names.retain(|n| names.insert(n.clone()));
            let project = f.project.clone();
            f.tools.retain_loaded(|(server, tool)| {
                loaded.insert((project.clone(), server.clone(), tool.clone()))
            });
        }
    }
    out
}

fn names(f: &FileCtx) -> Vec<(NameKind, String)> {
    let mut set: BTreeSet<(NameKind, String)> = BTreeSet::new();
    set.insert((NameKind::Project, f.project.clone()));
    set.insert((NameKind::Project, f.pf.source.project_key.clone()));
    set.insert((NameKind::Session, f.session().to_string()));
    if let Some(a) = &f.pf.source.agent_type {
        set.insert((NameKind::AgentType, a.clone()));
    }
    if let Some(sp) = &f.pf.records.startup {
        for p in &sp.parts {
            use crate::model::StartupKind::*;
            let kind = match p.kind {
                UserInstructions | ProjectInstructions | Memory => NameKind::Path,
                SkillListing => NameKind::Skill,
                McpTools | DeferredTools | McpInstructions => NameKind::McpServer,
                _ => continue,
            };
            set.insert((kind, f.s(p.name).to_string()));
        }
    }
    for l in &f.pf.records.skill_loads {
        set.insert((NameKind::Skill, f.s(l.skill).to_string()));
    }
    for c in &f.pf.records.tool_calls {
        if let Some((_, p)) = c.path {
            set.insert((NameKind::Path, f.s(p).to_string()));
        }
        if let (Some(d), Some(k)) = (c.detail, crate::anon::detail_kind(c.category)) {
            set.insert((k, f.s(d).to_string()));
        }
    }
    set.into_iter().collect()
}

pub fn size_breakdown<'a>(
    sessions: impl Iterator<Item = &'a SessionSummary>,
) -> Vec<(&'static str, usize)> {
    let len = |r: Result<Vec<u8>, postcard::Error>| r.map_or(0, |b| b.len());
    let mut t = [
        ("parts", 0),
        ("skills", 0),
        ("memory", 0),
        ("sub", 0),
        ("compactions", 0),
        ("segments", 0),
        ("tools", 0),
        ("live", 0),
        ("names", 0),
        ("other", 0),
    ];
    for f in sessions.flat_map(|s| &s.files) {
        let v = [
            len(postcard::to_allocvec(&f.parts)),
            len(postcard::to_allocvec(&f.skills)),
            len(postcard::to_allocvec(&f.memory)),
            len(postcard::to_allocvec(&f.sub)),
            len(postcard::to_allocvec(&f.compactions)),
            len(postcard::to_allocvec(&f.segments)),
            len(postcard::to_allocvec(&f.tools)),
            len(postcard::to_allocvec(&f.live)),
            len(postcard::to_allocvec(&f.names)),
            len(postcard::to_allocvec(&(
                &f.provider,
                &f.session,
                &f.project,
                &f.path,
                &f.agent_type,
                &f.unpriced,
            ))),
        ];
        for (i, x) in v.iter().enumerate() {
            t[i].1 += x;
        }
    }
    let mut t = t.to_vec();
    t.sort_by(|a, b| b.1.cmp(&a.1));
    t
}
