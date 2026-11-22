// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use crate::metrics::NameKind;
use crate::model::{Category, Snapshot};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub use crate::model::AnonLevel as Level;

#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub level: Level,
    pub salt: u64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            level: Level::Names,
            salt: 0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AliasMap {
    pub aliases: BTreeMap<String, String>,
}

pub fn anonymise(snapshot: &mut Snapshot) {
    anonymise_with(snapshot, &[], &Options::default());
}

pub fn anonymise_with(
    snapshot: &mut Snapshot,
    names: &[(NameKind, String)],
    opts: &Options,
) -> AliasMap {
    if opts.level == Level::Off {
        return AliasMap::default();
    }
    let mut a = Aliaser {
        opts: *opts,
        map: BTreeMap::new(),
        counters: BTreeMap::new(),
    };
    for (k, n) in names {
        a.alias(*k, n);
    }
    let s = snapshot;
    for r in &mut s.live.sessions {
        r.project = a.alias(NameKind::Project, &r.project);
        r.session = a.alias(NameKind::Session, &r.session);
    }
    for r in &mut s.daily {
        if let Some(p) = &r.project {
            r.project = Some(a.alias(NameKind::Project, p));
        }
    }
    for f in &mut s.findings {
        if let Some(p) = &f.project {
            f.project = Some(a.alias(NameKind::Project, p));
        }
    }
    for r in &mut s.startup {
        r.project = a.alias(NameKind::Project, &r.project);
        for p in &mut r.parts {
            let kind = match p.kind {
                crate::model::StartupKind::UserInstructions
                | crate::model::StartupKind::ProjectInstructions
                | crate::model::StartupKind::Memory => Some(NameKind::Path),
                crate::model::StartupKind::SkillListing => Some(NameKind::Skill),
                crate::model::StartupKind::McpTools
                | crate::model::StartupKind::DeferredTools
                | crate::model::StartupKind::McpInstructions => Some(NameKind::McpServer),
                _ => None,
            };
            if let Some(k) = kind {
                p.name = a.alias(k, &p.name);
            }
        }
    }
    for r in &mut s.skills {
        r.skill = a.alias(NameKind::Skill, &r.skill);
    }
    for r in &mut s.memory {
        r.path = a.alias(NameKind::Path, &r.path);
        r.label = r.path.clone();
    }
    for r in &mut s.subagents {
        r.project = a.alias(NameKind::Project, &r.project);
        if let Some(t) = &r.agent_type {
            r.agent_type = Some(a.alias(NameKind::AgentType, t));
        }
    }
    for r in &mut s.sessions {
        r.project = a.alias(NameKind::Project, &r.project);
        r.session = a.alias(NameKind::Session, &r.session);
    }
    for r in &mut s.compactions {
        r.project = a.alias(NameKind::Project, &r.project);
        r.session = a.alias(NameKind::Session, &r.session);
    }
    for r in &mut s.tools {
        if let Some(p) = &r.project {
            r.project = Some(a.alias(NameKind::Project, p));
        }
        if let Some(d) = &r.detail {
            r.detail = Some(match detail_kind(r.category) {
                Some(k) => a.alias(k, d),
                None => d.clone(),
            });
        }
    }
    let mut pairs: Vec<(String, String)> = a
        .map
        .iter()
        .filter(|(r, al)| r != al)
        .map(|(r, al)| (r.clone(), al.clone()))
        .collect();
    pairs.sort_by(|x, y| y.0.len().cmp(&x.0.len()).then_with(|| x.0.cmp(&y.0)));
    for f in &mut s.findings {
        let mut line = f.line.clone();
        for (real, alias) in &pairs {
            line = replace_word(&line, real, alias);
        }
        f.line = scrub_home(&line);
    }
    s.anonymised = true;
    AliasMap {
        aliases: a
            .map
            .into_iter()
            .filter(|(r, al)| r != al)
            .map(|(r, al)| (al, r))
            .collect(),
    }
}

pub fn detail_kind(category: Category) -> Option<NameKind> {
    match category {
        Category::Script => Some(NameKind::Path),
        Category::Mcp => Some(NameKind::McpServer),
        Category::Other | Category::ProjectTool | Category::Vcs => Some(NameKind::Command),
        _ => None,
    }
}

fn is_provider_public(real: &str) -> bool {
    static NAMES: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    NAMES
        .get_or_init(|| {
            crate::provider::all()
                .iter()
                .flat_map(|p| p.public_names().iter().copied())
                .collect()
        })
        .contains(&real)
}

/// Any other command name may be a project or user name and is aliased.
const PUBLIC_COMMANDS: &[&str] = &[
    "cargo",
    "dotnet",
    "go",
    "gh",
    "npm",
    "pnpm",
    "yarn",
    "npx",
    "docker",
    "podman",
    "kubectl",
    "echo",
    "cat",
    "cd",
    "rm",
    "cp",
    "mv",
    "mkdir",
    "touch",
    "chmod",
    "sleep",
    "kill",
    "ps",
    "which",
    "where",
    "sed",
    "awk",
    "head",
    "tail",
    "wc",
    "sort",
    "uniq",
    "tar",
    "unzip",
    "zip",
    "ssh",
    "scp",
    "rsync",
    "java",
    "javac",
    "rustc",
    "rustup",
    "pip",
    "pip3",
    "uv",
    "winget",
    "choco",
    "brew",
    "apt",
    "sudo",
    "start",
    "explorer",
    "taskkill",
    "tasklist",
    "set-location",
    "remove-item",
    "copy-item",
    "move-item",
    "new-item",
    "test-path",
    "write-host",
    "write-output",
    "stop-process",
    "get-process",
    "start-process",
    "ffmpeg",
    "magick",
    "jq",
    "code",
    "diff",
    "xargs",
    "env",
    "export",
    "status",
    "diff",
    "log",
    "show",
    "add",
    "commit",
    "push",
    "pull",
    "fetch",
    "checkout",
    "switch",
    "branch",
    "merge",
    "rebase",
    "reset",
    "restore",
    "stash",
    "tag",
    "remote",
    "clone",
    "init",
    "blame",
    "rev-parse",
    "worktree",
    "config",
    "cherry-pick",
    "revert",
    "clean",
    "mv",
    "rm",
    "ls-files",
    "describe",
    "grep",
];

/// Any other skill may be private and is aliased.
const PUBLIC_SKILLS: &[&str] = &[
    "init",
    "review",
    "security-review",
    "code-review",
    "simplify",
    "loop",
    "schedule",
    "run",
    "claude-api",
    "update-config",
    "keybindings-help",
    "fewer-permission-prompts",
    "plugin-authoring",
    "workflow-authoring",
    "artifact-design",
    "artifact-capabilities",
    "artifact-diagramming",
    "dataviz",
    "unslop",
];
const PUBLIC_PLUGINS: &[&str] = &[
    "anthropic-skills",
    "cloudflare",
    "supabase",
    "frontend-design",
    "impeccable",
    "last30days",
    "superpowers",
    "github",
    "vercel",
    "stripe",
    "figma",
    "notion",
    "linear",
    "sentry",
    "playwright",
    "context7",
];

fn is_public_skill(name: &str) -> bool {
    let n = name.to_lowercase();
    PUBLIC_SKILLS.contains(&n.as_str())
        || n.split_once(':')
            .is_some_and(|(ns, _)| PUBLIC_PLUGINS.contains(&ns))
}

struct Aliaser {
    opts: Options,
    map: BTreeMap<String, String>,
    counters: BTreeMap<NameKind, u32>,
}

impl Aliaser {
    fn alias(&mut self, kind: NameKind, real: &str) -> String {
        if real.is_empty() {
            return String::new();
        }
        if let Some(a) = self.map.get(real) {
            return a.clone();
        }
        let strict = self.opts.level == Level::Strict;
        let alias = match kind {
            NameKind::Project => format!("project{}", self.next(kind)),
            NameKind::Session => {
                let a = hex(fnv(self.opts.salt, real), 8);
                let short: String = real.chars().take(8).collect();
                self.map.entry(short).or_insert_with(|| a.clone());
                a
            }
            NameKind::Path => {
                let base = real
                    .rsplit(['/', '\\'])
                    .find(|s| !s.is_empty())
                    .unwrap_or(real);
                let ext = base
                    .rsplit_once('.')
                    .map(|(_, e)| format!(".{e}"))
                    .unwrap_or_default();
                let a = format!("file-{}{}", hex(fnv(self.opts.salt, real), 4), ext);
                if base != real && !self.map.contains_key(base) {
                    self.map.insert(base.to_string(), a.clone());
                }
                a
            }
            NameKind::Skill if strict && !is_public_skill(real) => {
                format!("skill-{}", self.next(kind))
            }
            NameKind::AgentType if strict && !is_provider_public(real) => {
                format!("agent-type-{}", self.next(kind))
            }
            NameKind::McpServer if strict => format!("mcp-{}", self.next(kind)),
            NameKind::Command
                if !PUBLIC_COMMANDS.contains(&real.to_lowercase().as_str())
                    && !is_provider_public(real) =>
            {
                format!("cmd-{}", self.next(kind))
            }
            _ => real.to_string(),
        };
        self.map.insert(real.to_string(), alias.clone());
        alias
    }

    fn next(&mut self, kind: NameKind) -> u32 {
        let c = self.counters.entry(kind).or_default();
        *c += 1;
        *c
    }
}

fn fnv(salt: u64, s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in salt.to_le_bytes().iter().chain(s.as_bytes()) {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

fn hex(h: u64, n: usize) -> String {
    format!("{h:016x}")[..n].to_string()
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

fn replace_word(text: &str, real: &str, alias: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find(real) {
        let before = if i > 0 {
            rest[..i].chars().last()
        } else {
            out.chars().last()
        };
        let after = rest[i + real.len()..].chars().next();
        out.push_str(&rest[..i]);
        if before.is_none_or(|c| !is_word(c)) && after.is_none_or(|c| !is_word(c)) {
            out.push_str(alias);
        } else {
            out.push_str(real);
        }
        rest = &rest[i + real.len()..];
    }
    out.push_str(rest);
    out
}

fn scrub_home(text: &str) -> String {
    let mut s = text.to_string();
    for marker in [":\\Users\\", ":/Users/", "/Users/", "/home/", "\\Users\\"] {
        while let Some(i) = s.find(marker) {
            let start = if marker.starts_with(':') {
                i.saturating_sub(1)
            } else {
                i
            };
            let user_start = i + marker.len();
            let user_end = s[user_start..]
                .find(['/', '\\', ' '])
                .map_or(s.len(), |j| user_start + j);
            s.replace_range(start..user_end, "~");
        }
    }
    s
}

pub fn anonymise_daily(rows: &mut [crate::model::DailyRow], opts: &Options) -> AliasMap {
    if opts.level == Level::Off {
        return AliasMap::default();
    }
    let mut cost: BTreeMap<String, f64> = BTreeMap::new();
    for r in rows.iter() {
        if let Some(p) = &r.project {
            *cost.entry(p.clone()).or_default() += r.cost_usd.value.unwrap_or(0.0);
        }
    }
    let mut ranked: Vec<(String, f64)> = cost.into_iter().collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut a = Aliaser {
        opts: *opts,
        map: BTreeMap::new(),
        counters: BTreeMap::new(),
    };
    for (p, _) in &ranked {
        a.alias(NameKind::Project, p);
    }
    for r in rows.iter_mut() {
        if let Some(p) = &r.project {
            r.project = Some(a.alias(NameKind::Project, p));
        }
    }
    AliasMap {
        aliases: a
            .map
            .into_iter()
            .map(|(real, alias)| (alias, real))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_aliases_private_plugin_skills() {
        let mut a = Aliaser {
            opts: Options {
                level: Level::Strict,
                salt: 0,
            },
            map: BTreeMap::new(),
            counters: BTreeMap::new(),
        };
        assert_eq!(
            a.alias(NameKind::Skill, "cloudflare:wrangler"),
            "cloudflare:wrangler"
        );
        assert_eq!(a.alias(NameKind::Skill, "simplify"), "simplify");
        let private = a.alias(NameKind::Skill, "acmecorp-internal:deploy-prod");
        assert!(private.starts_with("skill-"), "{private}");
        assert!(!private.contains("acmecorp"));
        assert!(a
            .alias(NameKind::Skill, "my-own-skill")
            .starts_with("skill-"));
    }
}
