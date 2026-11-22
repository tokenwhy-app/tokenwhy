// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use crate::metrics::util::fmt_tok;
use crate::model::{Finding, Snapshot, Window};
use serde::{Deserialize, Serialize};

/// Bumped on any breaking change to `Brief` or the MCP tool results; see docs/json-contract.md.
pub const BRIEF_SCHEMA_VERSION: u32 = 1;

const DAY_MS: f64 = 86_400_000.0;

pub struct Fix {
    pub rule: &'static str,
    pub apply: &'static str,
}

pub const FIXES: [Fix; 18] = [
    Fix {
        rule: "R1",
        apply: "Trim what this project loads at startup, largest part first: move rarely needed sections of CLAUDE.md into files read on demand, and disable the plugins, MCP servers and skills the project does not use.",
    },
    Fix {
        rule: "R2",
        apply: "Rewrite the memory file as an index of one short line per entry, each pointing to a detail file that is read only when the topic comes up.",
    },
    Fix {
        rule: "R3",
        apply: "Disable the plugin for this project: in .claude/settings.json set \"enabledPlugins\": {\"<plugin>\": false}, or turn it off with /plugin.",
    },
    Fix {
        rule: "R4",
        apply: "Remove the MCP server (claude mcp remove <name>) or register it only in the projects that call it (claude mcp add --scope project ...).",
    },
    Fix {
        rule: "R5",
        apply: "Split the skill: keep the steps in SKILL.md and move references and examples to separate files the skill reads only when a step needs them.",
    },
    Fix {
        rule: "R6",
        apply: "Cut the description in the skill's SKILL.md frontmatter to one sentence saying when to use it; every session lists it.",
    },
    Fix {
        rule: "R7",
        apply: "Delete the lines this file copies from ~/.claude/CLAUDE.md; the user-level file already loads in every session.",
    },
    Fix {
        rule: "R8",
        apply: "Add to CLAUDE.md: \"Brief subagents by path: give file:line entry points, the rule and the expected proof, never pasted file contents. Use haiku for read-only agents. Split agents by independent area, not into slices that re-read the same files.\"",
    },
    Fix {
        rule: "R9",
        apply: "Set `model: haiku` in the frontmatter of this agent type (.claude/agents/<type>.md), or pass model \"haiku\" when spawning it.",
    },
    Fix {
        rule: "R10",
        apply: "Add to CLAUDE.md: \"One session per topic. At the end of a topic, write a short handoff (state, next step, paths) and start a fresh session instead of compacting.\"",
    },
    Fix {
        rule: "R11",
        apply: "Add to CLAUDE.md: \"Read code by symbol or by line range (Read with offset and limit, or an outline tool); never read a large file whole.\"",
    },
    Fix {
        rule: "R12",
        apply: "Add to CLAUDE.md: \"Scope every search to a known folder and a glob, and cap the matches (head_limit, -m); never search the whole repo.\"",
    },
    Fix {
        rule: "R13",
        apply: "Add to CLAUDE.md: \"Cap command output: pipe long output through tail or grep for the error lines, and read logs by range.\"",
    },
    Fix {
        rule: "R14",
        apply: "Add to CLAUDE.md: \"List files with a scoped glob in a known folder and a depth limit; never list a whole tree.\"",
    },
    Fix {
        rule: "R15",
        apply: "Add to CLAUDE.md: \"Run builds and tests with quiet flags (gradle --console=plain -q, cargo -q, npm --silent, pytest -q) and show only the failures.\"",
    },
    Fix {
        rule: "R16",
        apply: "Add to CLAUDE.md: \"Do not read a file again in the same session unless it changed; keep the lines you need in your notes or in the brief.\"",
    },
    Fix {
        rule: "R17",
        apply: "Split the topic: when context stays high for many turns, write a handoff and start a fresh session. Add \"One session per topic\" to CLAUDE.md.",
    },
    Fix {
        rule: "R18",
        apply: "Do not edit CLAUDE.md, memory files, settings or MCP servers during a session; each edit invalidates the prompt cache. Batch those edits for the next session.",
    },
];

pub fn fix(rule: &str) -> Option<&'static Fix> {
    FIXES.iter().find(|f| f.rule == rule)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Brief {
    pub schema: u32,
    pub window: Window,
    pub weeks: f64,
    pub spend_usd: Option<f64>,
    pub spend_usd_per_week: Option<f64>,
    pub saved_usd_per_week: f64,
    pub anonymised: bool,
    pub partial: bool,
    pub items: Vec<BriefItem>,
    pub more: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BriefItem {
    pub rank: u32,
    pub rule: String,
    pub what: String,
    pub project: Option<String>,
    pub provider: Option<String>,
    pub findings: u32,
    pub usd_saved_per_week: Option<f64>,
    pub share_of_spend: Option<f64>,
    pub tokens_saved_per_week: Option<f64>,
    pub evidence: Vec<String>,
    pub fix: String,
}

#[derive(Debug, Clone, Default)]
pub struct Filter {
    pub project: Option<String>,
    pub provider: Option<String>,
    pub limit: usize,
}

pub const DEFAULT_LIMIT: usize = 8;

fn cents(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

pub fn weeks(snap: &Snapshot) -> f64 {
    let w = &snap.window;
    snap.weeks
        .unwrap_or_else(|| (w.to_ms - w.from_ms) as f64 / (7.0 * DAY_MS))
        .max(1.0 / 7.0)
}

pub fn spend_usd(snap: &Snapshot) -> Option<f64> {
    let sum = |it: &mut dyn Iterator<Item = Option<f64>>| {
        it.fold(None, |acc: Option<f64>, v| match v {
            Some(v) => Some(acc.unwrap_or(0.0) + v),
            None => acc,
        })
    };
    if !snap.daily.is_empty() {
        sum(&mut snap.daily.iter().map(|d| d.cost_usd.value))
    } else {
        sum(&mut snap.sessions.iter().map(|s| s.cost_usd.value))
    }
}

fn keep(f: &Finding, filter: &Filter) -> bool {
    filter
        .project
        .as_ref()
        .is_none_or(|p| f.project.as_ref() == Some(p))
        && filter
            .provider
            .as_ref()
            .is_none_or(|p| f.provider.as_ref().is_none_or(|x| &x.0 == p))
}

pub fn brief(snap: &Snapshot, filter: &Filter) -> Brief {
    let weeks = weeks(snap);
    let spend = spend_usd(snap);
    let per_week = spend.map(|s| cents(s / weeks));
    let mut groups: Vec<BriefItem> = Vec::new();
    let mut sorted: Vec<&Finding> = snap.findings.iter().filter(|f| keep(f, filter)).collect();
    sorted.sort_by(|a, b| {
        let u = |f: &Finding| f.usd_saved_per_week.value.unwrap_or(-1.0);
        u(b).total_cmp(&u(a))
    });
    for f in sorted {
        let provider = f.provider.as_ref().map(|p| p.0.clone());
        let add = |a: Option<f64>, b: Option<f64>| match (a, b) {
            (None, None) => None,
            (a, b) => Some(a.unwrap_or(0.0) + b.unwrap_or(0.0)),
        };
        match groups
            .iter_mut()
            .find(|g| g.rule == f.rule && g.project == f.project && g.provider == provider)
        {
            Some(g) => {
                g.findings += 1;
                g.usd_saved_per_week = add(g.usd_saved_per_week, f.usd_saved_per_week.value);
                g.tokens_saved_per_week =
                    add(g.tokens_saved_per_week, f.tokens_saved_per_week.value);
                if g.evidence.len() < 2 {
                    g.evidence.push(f.line.clone());
                }
            }
            None => groups.push(BriefItem {
                rank: 0,
                rule: f.rule.clone(),
                what: crate::render::rule_label(&f.rule).unwrap_or("").to_string(),
                project: f.project.clone(),
                provider,
                findings: 1,
                usd_saved_per_week: f.usd_saved_per_week.value,
                share_of_spend: None,
                tokens_saved_per_week: f.tokens_saved_per_week.value,
                evidence: vec![f.line.clone()],
                fix: fix(&f.rule).map_or("", |x| x.apply).to_string(),
            }),
        }
    }
    for g in &mut groups {
        g.usd_saved_per_week = g.usd_saved_per_week.map(cents);
        g.share_of_spend = g
            .usd_saved_per_week
            .zip(per_week)
            .filter(|(_, s)| *s > 0.0)
            .map(|(u, s)| ((u / s).min(1.0) * 1000.0).round() / 1000.0);
    }
    groups.sort_by(|a, b| {
        let u = |g: &BriefItem| g.usd_saved_per_week.unwrap_or(-1.0);
        let t = |g: &BriefItem| g.tokens_saved_per_week.unwrap_or(-1.0);
        u(b).total_cmp(&u(a))
            .then_with(|| t(b).total_cmp(&t(a)))
            .then_with(|| a.rule.cmp(&b.rule))
    });
    let saved = cents(groups.iter().filter_map(|g| g.usd_saved_per_week).sum());
    let limit = if filter.limit == 0 {
        groups.len()
    } else {
        filter.limit
    };
    let more = groups.len().saturating_sub(limit);
    groups.truncate(limit);
    for (i, g) in groups.iter_mut().enumerate() {
        g.rank = i as u32 + 1;
    }
    Brief {
        schema: BRIEF_SCHEMA_VERSION,
        window: snap.window.clone(),
        weeks: (weeks * 100.0).round() / 100.0,
        spend_usd: spend.map(cents),
        spend_usd_per_week: per_week,
        saved_usd_per_week: saved,
        anonymised: snap.anonymised,
        partial: snap.progress < 1.0,
        items: groups,
        more,
    }
}

fn pct(v: f64) -> String {
    let p = v * 100.0;
    if p > 0.0 && p < 10.0 {
        format!("{p:.1}%")
    } else {
        format!("{p:.0}%")
    }
}

pub fn markdown(b: &Brief) -> String {
    let days = ((b.window.to_ms - b.window.from_ms) as f64 / DAY_MS).round().max(1.0);
    let mut s = String::from("# tokenwhy: fixes for your agent\n\n");
    let spend = match (b.spend_usd, b.spend_usd_per_week) {
        (Some(t), Some(w)) => format!("spend ${t:.2} (${w:.2}/week)"),
        _ => "spend not priced".to_string(),
    };
    let share = b
        .spend_usd_per_week
        .filter(|w| *w > 0.0)
        .map(|w| format!(" ({} of spend)", pct((b.saved_usd_per_week / w).min(1.0))))
        .unwrap_or_default();
    s += &format!(
        "Last {days} days, {spend}. These fixes save about ${:.2}/week{share}.\n",
        b.saved_usd_per_week
    );
    s += "Apply them in order; savings are estimates from local transcripts.\n";
    if b.partial {
        s += "Scan not finished: numbers are partial.\n";
    }
    if b.items.is_empty() {
        s += "\nNo findings in this window.\n";
        return s;
    }
    let mut rules: Vec<&str> = vec![];
    for g in &b.items {
        if !rules.contains(&g.rule.as_str()) {
            rules.push(&g.rule);
        }
    }
    let sum = |it: &mut dyn Iterator<Item = Option<f64>>| {
        it.fold(None, |acc: Option<f64>, v| {
            v.map(|v| acc.unwrap_or(0.0) + v).or(acc)
        })
    };
    let total = |r: &str| -> f64 {
        b.items
            .iter()
            .filter(|g| g.rule == r)
            .filter_map(|g| g.usd_saved_per_week)
            .sum()
    };
    rules.sort_by(|x, y| total(y).total_cmp(&total(x)));
    for (i, rule) in rules.iter().enumerate() {
        let gs: Vec<&BriefItem> = b.items.iter().filter(|g| g.rule == *rule).collect();
        let usd = sum(&mut gs.iter().map(|g| g.usd_saved_per_week));
        let share = sum(&mut gs.iter().map(|g| g.share_of_spend));
        let usd_s = usd.map_or("n/a".to_string(), |u| format!("${u:.2}/week"));
        let share_s = share
            .map(|v| format!(" ({} of spend)", pct(v.min(1.0))))
            .unwrap_or_default();
        s += &format!(
            "
## {}. {} {}: {usd_s}{share_s}
",
            i + 1,
            rule,
            gs[0].what
        );
        for g in &gs {
            let mut ev = vec![];
            if let Some(p) = &g.project {
                ev.push(format!("project {p}"));
            }
            if let Some(p) = &g.provider {
                ev.push(p.clone());
            }
            if gs.len() > 1 {
                if let Some(u) = g.usd_saved_per_week {
                    ev.push(format!("${u:.2}/week"));
                }
            }
            if let Some(t) = g.tokens_saved_per_week {
                ev.push(format!("{} tokens/week", fmt_tok(t)));
            }
            if g.findings > 1 {
                ev.push(format!("{} findings", g.findings));
            }
            if !ev.is_empty() {
                s += &format!(
                    "- Scope: {}
",
                    ev.join(", ")
                );
            }
        }
        for line in gs.iter().flat_map(|g| &g.evidence).take(2) {
            s += &format!(
                "- Evidence: {line}
"
            );
        }
        s += &format!(
            "- Apply: {}
",
            gs[0].fix
        );
    }
    if b.more > 0 {
        s += &format!(
            "\n{} smaller groups left out; `tokenwhy findings` lists them.\n",
            b.more
        );
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_fix_per_rule_without_em_dash() {
        for n in 1..=18 {
            let f = fix(&format!("R{n}")).expect("every rule has a fix");
            assert!(!f.apply.contains('\u{2014}'), "{} has an em dash", f.rule);
        }
    }
}
