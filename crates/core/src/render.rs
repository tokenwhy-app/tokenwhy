// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use crate::anon::{self, AliasMap, Options};
use crate::metrics::util::fmt_tok;
use crate::metrics::Aggregates;
use crate::model::{
    Amount, Capabilities, Category, Finding, Level, ProviderId, Query, Snapshot, StartupKind,
    SNAPSHOT_SCHEMA_VERSION,
};

pub fn snapshot(
    agg: &Aggregates,
    findings: Vec<Finding>,
    query: &Query,
    progress: f64,
) -> Snapshot {
    snapshot_with(agg, findings, query, progress, &Options::default()).0
}

pub fn snapshot_with(
    agg: &Aggregates,
    findings: Vec<Finding>,
    query: &Query,
    progress: f64,
    anon: &Options,
) -> (Snapshot, AliasMap) {
    let mut s = Snapshot {
        schema: SNAPSHOT_SCHEMA_VERSION,
        generated_ms: agg.now_ms,
        window: agg.window.clone(),
        weeks: Some((agg.weeks * 100.0).round() / 100.0),
        anonymised: false,
        progress: progress.clamp(0.0, 1.0),
        providers: agg.providers.clone(),
        accounts: agg.accounts.clone(),
        live: agg.live.clone(),
        findings,
        startup: agg.startup.clone(),
        skills: agg.skills.clone(),
        memory: agg.memory.clone(),
        subagents: agg.subagents.clone(),
        compactions: agg.compactions.clone(),
        tools: agg.tools.clone(),
        unpriced_models: agg.unpriced_models.clone(),
        history_newer: None,
        daily: agg.daily.clone(),
        week: agg.week.clone(),
        sessions: agg.session_rows.clone(),
        hourly: agg.hourly.clone(),
        #[cfg(feature = "co2")]
        co2: None,
        #[cfg(feature = "plan")]
        plan: vec![],
    };
    let opts = Options {
        level: query.anonymise,
        salt: anon.salt,
    };
    let map = if opts.level != anon::Level::Off {
        anon::anonymise_with(&mut s, &agg.names, &opts)
    } else {
        AliasMap::default()
    };
    (s, map)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Text,
    Markdown,
    Json,
}

pub const SECTIONS: [&str; 8] = [
    "live",
    "findings",
    "startup",
    "skills",
    "memory",
    "subagents",
    "compactions",
    "tools",
];

pub fn render(snapshot: &Snapshot, format: Format, section: Option<&str>) -> String {
    render_with(snapshot, format, section, false)
}

pub fn render_with(
    snapshot: &Snapshot,
    format: Format,
    section: Option<&str>,
    summary: bool,
) -> String {
    render_styled(snapshot, format, section, summary, false)
}

pub fn render_styled(
    snapshot: &Snapshot,
    format: Format,
    section: Option<&str>,
    summary: bool,
    color: bool,
) -> String {
    let color = color && format == Format::Text;
    let summary = summary && section.is_none();
    if format == Format::Json {
        let v = serde_json::to_value(snapshot).expect("snapshot serialises");
        let v = match section {
            Some(name) => v.get(name).cloned().unwrap_or(serde_json::Value::Null),
            None => v,
        };
        return serde_json::to_string_pretty(&v).expect("json value serialises") + "\n";
    }
    if let Some(name) = section.filter(|s| !SECTIONS.contains(s)) {
        return format!(
            "Unknown section {name}. Sections: {}
",
            SECTIONS.join(", ")
        );
    }
    let md = format == Format::Markdown;
    let mut out = String::new();
    for name in SECTIONS.iter().filter(|n| section.is_none_or(|s| s == **n)) {
        let (t, hidden) = if summary {
            capped_table(snapshot, name)
        } else {
            (table(snapshot, name), 0)
        };
        let na = not_available(snapshot, name);
        if section.is_none() && t.rows.is_empty() && na.is_none() {
            continue;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        if !t.rows.is_empty() || na.is_none() {
            out.push_str(&if md { t.markdown() } else { t.text(color) });
            if hidden > 0 {
                let hint = format!("+{hidden} more (tokenwhy report --section {name})");
                out.push_str(&if md { format!("
_{hint}_
") } else { format!("{}
", paint(color, DIM, &hint)) });
            }
        }
        if let Some(line) = na {
            out.push_str(&line);
            out.push('\n');
        }
        if *name == "findings" && !t.rows.is_empty() {
            out.push_str(&legend(&t.rows, md));
        }
    }
    if section.is_none() && !snapshot.unpriced_models.is_empty() {
        let line = format!(
            "Models without a price (cost n/a): {}",
            snapshot.unpriced_models.join(", ")
        );
        out.push_str(&format!("\n{line}\n"));
    }
    if out.is_empty() {
        out = match section {
            Some(s) if !SECTIONS.contains(&s) => {
                format!("Unknown section {s}. Sections: {}\n", SECTIONS.join(", "))
            }
            _ => "No data in this window.\n".into(),
        };
    }
    out
}

fn not_available(s: &Snapshot, section: &str) -> Option<String> {
    let (title, cap): (&str, fn(&Capabilities) -> Level) = match section {
        "startup" => ("Startup breakdown", |c| c.startup),
        "skills" => ("Skills", |c| c.skills),
        "memory" => ("Memory files", |c| c.memory),
        "subagents" => ("Subagents", |c| c.subagents),
        "compactions" => ("Compactions", |c| c.compactions),
        "tools" => ("Tool outputs", |c| c.tool_outputs),
        _ => return None,
    };
    let missing: Vec<&str> = s
        .providers
        .iter()
        .filter(|p| cap(&p.caps) == Level::None)
        .map(|p| p.label.as_str())
        .collect();
    (!missing.is_empty()).then(|| {
        format!(
            "{title}: not available for {} transcripts",
            missing.join(", ")
        )
    })
}

fn provider_tag(s: &Snapshot, id: &ProviderId) -> String {
    if s.providers.len() < 2 {
        return String::new();
    }
    s.providers
        .iter()
        .find(|p| &p.id == id)
        .map_or_else(|| format!("[{}] ", id.0), |p| format!("[{}] ", p.label))
}

pub fn category_label(c: Category) -> &'static str {
    match c {
        Category::ReadFile => "read-file",
        Category::Search => "search",
        Category::List => "list",
        Category::Vcs => "vcs",
        Category::Build => "build",
        Category::Test => "test",
        Category::Logs => "logs",
        Category::Script => "script",
        Category::ProjectTool => "project-tool",
        Category::Web => "web",
        Category::Agent => "agent",
        Category::Mcp => "mcp",
        Category::Edit => "edit",
        Category::Other => "other",
    }
}

fn kind_label(k: StartupKind) -> &'static str {
    match k {
        StartupKind::SystemPrompt => "system",
        StartupKind::BuiltinTools => "tools",
        StartupKind::McpTools => "mcp-tools",
        StartupKind::DeferredTools => "deferred",
        StartupKind::SkillListing => "skill",
        StartupKind::AgentListing => "agents",
        StartupKind::McpInstructions => "mcp-instr",
        StartupKind::UserInstructions => "user",
        StartupKind::ProjectInstructions => "project",
        StartupKind::Memory => "memory",
        StartupKind::Base => "base",
    }
}

fn tok(a: &Amount) -> String {
    match a.value {
        None => "n/a".into(),
        Some(v) if a.est => format!("{} est.", fmt_tok(v)),
        Some(v) => fmt_tok(v),
    }
}

fn usd(a: &Amount) -> String {
    match a.value {
        None => "n/a".into(),
        Some(v) if a.est => format!("${v:.2} est."),
        Some(v) => format!("${v:.2}"),
    }
}

fn pct(v: f64) -> String {
    format!("{:.0} %", v * 100.0)
}

fn n(v: u64) -> String {
    fmt_tok(v as f64)
}

fn opt(s: &Option<String>) -> String {
    s.clone().unwrap_or_else(|| "-".into())
}

const TEXT_WIDTH: usize = 100;

fn wrap(s: &str, room: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    for word in s.split(' ') {
        let cur = lines.last_mut().expect("never empty");
        if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > room {
            lines.push(word.to_string());
        } else {
            if !cur.is_empty() {
                cur.push(' ');
            }
            cur.push_str(word);
        }
    }
    lines
}

const RULES: [(&str, &str); 18] = [
    ("R1", "startup load per session"),
    ("R2", "memory file size"),
    ("R3", "plugin listed, never used"),
    ("R4", "MCP server, few tools called"),
    ("R5", "skill body per activation"),
    ("R6", "skill description length"),
    ("R7", "instruction lines repeated across files"),
    ("R8", "subagent share of cost"),
    ("R9", "read-only agents on the largest model"),
    ("R10", "compactions in a session"),
    ("R11", "file reads in tool output"),
    ("R12", "searches in tool output"),
    ("R13", "very large tool results"),
    ("R14", "listings in tool output"),
    ("R15", "build and test output"),
    ("R16", "same file read again"),
    ("R17", "long topic at high context"),
    ("R18", "low cache hit"),
];

pub fn rule_label(rule: &str) -> Option<&'static str> {
    RULES.iter().find(|(r, _)| *r == rule).map(|(_, l)| *l)
}

fn legend(rows: &[Vec<String>], md: bool) -> String {
    let used: Vec<String> = RULES
        .iter()
        .filter(|(c, _)| rows.iter().any(|r| r[0] == *c))
        .map(|(c, d)| format!("{c} {d}"))
        .collect();
    let text = format!("Rules: {}.", used.join("; "));
    if md {
        format!("\n{text}\n")
    } else {
        wrap(&text, TEXT_WIDTH).join("\n") + "\n"
    }
}

const BOLD: &str = "1";
const DIM: &str = "2";
const DIM_YELLOW: &str = "2;33";
const ACCENT: &str = "36";

fn paint(on: bool, code: &str, text: &str) -> String {
    if on && !text.is_empty() {
        format!("[{code}m{text}[0m")
    } else {
        text.to_string()
    }
}

fn figure(c: &str, always: bool) -> String {
    if c == "n/a" {
        return paint(true, DIM, c);
    }
    let (body, est) = match c.strip_suffix(" est.") {
        Some(b) => (b, true),
        None => (c, false),
    };
    let body = if always || body.starts_with('$') { paint(true, ACCENT, body) } else { body.to_string() };
    if est { format!("{body}{}", paint(true, DIM, " est.")) } else { body }
}

struct Table {
    title: &'static str,
    head: Vec<&'static str>,
    rows: Vec<Vec<String>>,
}

impl Table {
    fn text(&self, color: bool) -> String {
        let mut w: Vec<usize> = self.head.iter().map(|h| h.chars().count()).collect();
        for r in &self.rows {
            for (i, c) in r.iter().enumerate() {
                w[i] = w[i].max(c.chars().count());
            }
        }
        let last = w.len() - 1;
        let indent: usize = w[..last].iter().map(|x| x + 2).sum();
        let room = TEXT_WIDTH.saturating_sub(indent).max(30);
        let findings = self.title == "Findings";
        // Padding counts visible characters; escape codes wrap the text before the spaces are added.
        let line = |cells: Vec<String>, head: bool| {
            let style = |i: usize, c: &str| -> String {
                if !color {
                    c.to_string()
                } else if head {
                    paint(true, DIM, c)
                } else if findings && i == 0 {
                    paint(true, DIM_YELLOW, c)
                } else if findings && i == last {
                    c.to_string()
                } else {
                    figure(c, findings && i == 1)
                }
            };
            let s: Vec<String> = cells[..last]
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let pad = w[i].saturating_sub(c.chars().count());
                    format!("{}{}", style(i, c), " ".repeat(pad))
                })
                .collect();
            let mut parts = wrap(&cells[last], room).into_iter();
            let first = parts.next().unwrap_or_default();
            let mut out = format!("{}  {}", s.join("  "), style(last, &first))
                .trim_end()
                .to_string()
                + "\n";
            for p in parts {
                out.push_str(&format!("{:indent$}{}\n", "", style(last, &p)));
            }
            out
        };
        let mut out = format!("{}\n", paint(color, BOLD, self.title));
        if self.head.len() > 1 {
            out.push_str(&line(self.head.iter().map(|h| h.to_string()).collect(), true));
        }
        for r in &self.rows {
            out.push_str(&line(r.clone(), false));
        }
        out
    }

    fn markdown(&self) -> String {
        let esc = |s: &str| s.replace('|', "\\|");
        let mut out = format!("## {}\n\n", self.title);
        if self.rows.is_empty() {
            return out + "None.\n";
        }
        out.push_str(&format!("| {} |\n", self.head.join(" | ")));
        out.push_str(&format!("|{}\n", "---|".repeat(self.head.len())));
        for r in &self.rows {
            out.push_str(&format!(
                "| {} |\n",
                r.iter().map(|c| esc(c)).collect::<Vec<_>>().join(" | ")
            ));
        }
        out
    }
}

const CAP_STARTUP_PARTS: usize = 5;
const CAP_SKILLS: usize = 10;
const CAP_SUBAGENTS: usize = 10;
const CAP_COMPACTIONS: usize = 5;
const CAP_TOOLS: usize = 15;

fn capped_table(s: &Snapshot, name: &str) -> (Table, usize) {
    let mut t = table(s, name);
    let full = t.rows.len();
    match name {
        "startup" => {
            let mut kept = Vec::new();
            let mut parts = 0;
            for r in t.rows {
                if r[0].starts_with("  ") {
                    parts += 1;
                    if parts > CAP_STARTUP_PARTS {
                        continue;
                    }
                } else {
                    parts = 0;
                }
                kept.push(r);
            }
            t.rows = kept;
        }
        "skills" => {
            let used: Vec<bool> = s
                .skills
                .iter()
                .map(|r| r.activations > 0 || r.attributed_turns > 0 || r.attributed_cost_usd.value.is_some_and(|v| v > 0.0))
                .collect();
            let mut i = 0;
            t.rows.retain(|_| {
                i += 1;
                used[i - 1]
            });
            t.rows.truncate(CAP_SKILLS);
        }
        "subagents" => {
            let mut idx: Vec<usize> = (0..t.rows.len()).collect();
            idx.sort_by(|a, b| {
                let c = |i: usize| s.subagents[i].cost_usd.value.unwrap_or(0.0);
                c(*b).total_cmp(&c(*a))
            });
            idx.truncate(CAP_SUBAGENTS);
            t.rows = idx.into_iter().map(|i| t.rows[i].clone()).collect();
        }
        "compactions" => t.rows.truncate(CAP_COMPACTIONS),
        "tools" => {
            let mut idx: Vec<usize> = (0..t.rows.len()).collect();
            idx.sort_by(|a, b| {
                let c = |i: usize| s.tools[i].carried_tokens.value.unwrap_or(0.0);
                c(*b).total_cmp(&c(*a))
            });
            idx.truncate(CAP_TOOLS);
            t.rows = idx.into_iter().map(|i| t.rows[i].clone()).collect();
        }
        _ => {}
    }
    let hidden = full - t.rows.len();
    (t, hidden)
}

fn table(s: &Snapshot, name: &str) -> Table {
    match name {
        "live" => {
            let t = &s.live.today;
            let mut rows = vec![vec![
                "today".into(),
                String::new(),
                format!("{} fresh", n(t.fresh_tokens)),
                format!("{} cache read", n(t.cache_read_tokens)),
                usd(&t.cost_usd),
                format!("{} sessions, {} agents", t.sessions, t.agents),
            ]];
            rows.extend(s.live.sessions.iter().map(|a| {
                vec![
                    format!(
                        "{}{} {}",
                        provider_tag(s, &a.provider),
                        a.project,
                        a.session
                    ),
                    n(a.ctx_tokens),
                    format!("{} turns", a.turns),
                    format!("+{} since boundary", n(a.ctx_growth_since_boundary)),
                    usd(&a.cost_usd),
                    format!(
                        "{} agents running, {} compactions",
                        a.agents_running, a.compactions
                    ),
                ]
            }));
            Table {
                title: "Live",
                head: vec!["", "ctx now", "activity", "tokens", "cost", ""],
                rows,
            }
        }
        "findings" => Table {
            title: "Findings",
            head: vec!["rule", "saved/wk", "finding"],
            rows: s
                .findings
                .iter()
                .map(|f| {
                    vec![
                        f.rule.clone(),
                        tok(&f.tokens_saved_per_week),
                        f.line.clone(),
                    ]
                })
                .collect(),
        },
        "startup" => {
            let mut rows = vec![];
            for r in &s.startup {
                rows.push(vec![
                    r.project.clone(),
                    r.sessions.to_string(),
                    n(r.median),
                    n(r.p90),
                    n(r.min),
                    n(r.max),
                    r.ratio.map_or("-".into(), |x| format!("{x:.2}")),
                ]);
                for p in &r.parts {
                    let name = if p.name.is_empty() {
                        kind_label(p.kind).to_string()
                    } else {
                        format!("{} {}", kind_label(p.kind), p.name)
                    };
                    rows.push(vec![
                        format!("  {name}"),
                        String::new(),
                        tok(&p.tokens),
                        String::new(),
                        String::new(),
                        String::new(),
                        String::new(),
                    ]);
                }
                if r.unattributed.value.is_some_and(|v| v > 0.0) {
                    rows.push(vec![
                        "  unattributed".into(),
                        String::new(),
                        tok(&r.unattributed),
                        String::new(),
                        String::new(),
                        String::new(),
                        String::new(),
                    ]);
                }
            }
            Table {
                title: "Startup load (first turn ctx, tokens)",
                head: vec![
                    "project", "sessions", "median", "p90", "min", "max", "ratio",
                ],
                rows,
            }
        }
        "skills" => Table {
            title: "Skills",
            head: vec![
                "skill",
                "listing",
                "activations",
                "body",
                "carried",
                "loaded in",
                "listed in",
                "attributed turns",
                "attributed",
            ],
            rows: s
                .skills
                .iter()
                .map(|r| {
                    vec![
                        r.skill.clone(),
                        tok(&r.listing_tokens_median),
                        r.activations.to_string(),
                        tok(&r.body_tokens_median),
                        usd(&r.carried_cost_usd),
                        r.sessions_loaded.to_string(),
                        r.sessions_listed.to_string(),
                        r.attributed_turns.to_string(),
                        usd(&r.attributed_cost_usd),
                    ]
                })
                .collect(),
        },
        "memory" => Table {
            title: "Instructions and memory",
            head: vec![
                "kind",
                "path",
                "bytes",
                "tokens",
                "carried",
                "on-demand reads",
                "duplicate lines",
            ],
            rows: s
                .memory
                .iter()
                .map(|r| {
                    vec![
                        kind_label(r.kind).into(),
                        if r.label.is_empty() {
                            r.path.clone()
                        } else {
                            r.label.clone()
                        },
                        r.bytes.to_string(),
                        tok(&r.tokens),
                        usd(&r.carried_cost_usd),
                        r.on_demand_reads.to_string(),
                        r.duplicate_lines.to_string(),
                    ]
                })
                .collect(),
        },
        "subagents" => Table {
            title: "Subagents",
            head: vec![
                "project",
                "agent type",
                "model",
                "agents",
                "cost",
                "median",
                "cost share",
                "fresh share",
                "startup share",
            ],
            rows: s
                .subagents
                .iter()
                .map(|r| {
                    vec![
                        r.project.clone(),
                        opt(&r.agent_type),
                        opt(&r.model),
                        r.agents.to_string(),
                        usd(&r.cost_usd),
                        usd(&r.median_cost_usd),
                        pct(r.cost_share),
                        pct(r.fresh_share),
                        pct(r.startup_share),
                    ]
                })
                .collect(),
        },
        "compactions" => Table {
            title: "Compactions",
            head: vec![
                "project",
                "session",
                "pre",
                "post",
                "first after",
                "$/turn before",
                "$/turn after",
                "rebuild",
                "carry",
                "fresh restart",
            ],
            rows: s
                .compactions
                .iter()
                .map(|r| {
                    vec![
                        r.project.clone(),
                        r.session.chars().take(8).collect(),
                        n(r.pre_tokens),
                        n(r.post_tokens),
                        n(r.first_after_ctx),
                        usd(&r.cost_per_turn_before),
                        usd(&r.cost_per_turn_after),
                        tok(&r.rebuild_tokens),
                        r.carry_ratio.map_or("-".into(), |x| format!("{x:.1}")),
                        usd(&r.fresh_restart_cost_usd),
                    ]
                })
                .collect(),
        },
        "tools" => Table {
            title: "Tool output (carried: re-read from cache on each later turn)",
            head: vec![
                "category",
                "detail",
                "calls",
                "first",
                "carried",
                "share",
                "median chars",
                "p95 chars",
                "over 25k",
                "persisted",
            ],
            rows: s
                .tools
                .iter()
                .map(|r| {
                    vec![
                        category_label(r.category).into(),
                        opt(&r.detail),
                        r.calls.to_string(),
                        tok(&r.first_tokens),
                        tok(&r.carried_tokens),
                        pct(r.share),
                        r.median_chars.to_string(),
                        r.p95_chars.to_string(),
                        r.over_25k.to_string(),
                        r.persisted.to_string(),
                    ]
                })
                .collect(),
        },
        _ => Table {
            title: "",
            head: vec![""],
            rows: vec![],
        },
    }
}

pub fn history(rows: &[crate::model::DailyRow], format: Format) -> String {
    if format == Format::Json {
        return serde_json::to_string_pretty(rows).expect("rows serialise") + "\n";
    }
    if rows.is_empty() {
        return "No history in this window.\n".into();
    }
    use chrono::TimeZone;
    let day = |ms: i64| {
        chrono::Local
            .timestamp_millis_opt(ms)
            .single()
            .map_or_else(|| ms.to_string(), |d| d.format("%Y-%m-%d").to_string())
    };
    let key = |r: &crate::model::DailyRow| {
        r.project
            .clone()
            .or_else(|| r.model.clone())
            .unwrap_or_else(|| "-".into())
    };
    let head = if rows[0].model.is_some() {
        "model"
    } else {
        "project"
    };
    let line =
        |d: String, k: String, f: u64, cr: u64, cw: u64, c: &Amount, s: u32, a: u32, cp: u32| {
            vec![
                d,
                k,
                n(f),
                n(cr),
                n(cw),
                usd(c),
                s.to_string(),
                a.to_string(),
                cp.to_string(),
            ]
        };
    let mut out: Vec<Vec<String>> = rows
        .iter()
        .map(|r| {
            line(
                day(r.day_ms),
                key(r),
                r.fresh_tokens,
                r.cache_read_tokens,
                r.cache_write_tokens,
                &r.cost_usd,
                r.sessions,
                r.agents,
                r.compactions,
            )
        })
        .collect();
    let cost = rows.iter().filter_map(|r| r.cost_usd.value).sum::<f64>();
    let known = rows.iter().any(|r| r.cost_usd.value.is_some());
    out.push(line(
        "total".into(),
        String::new(),
        rows.iter().map(|r| r.fresh_tokens).sum(),
        rows.iter().map(|r| r.cache_read_tokens).sum(),
        rows.iter().map(|r| r.cache_write_tokens).sum(),
        &Amount {
            value: known.then_some(cost),
            est: false,
        },
        rows.iter().map(|r| r.sessions).sum(),
        rows.iter().map(|r| r.agents).sum(),
        rows.iter().map(|r| r.compactions).sum(),
    ));
    let t = Table {
        title: "History",
        head: vec![
            "day",
            head,
            "fresh",
            "cache read",
            "cache write",
            "cost",
            "sessions",
            "agents",
            "compactions",
        ],
        rows: out,
    };
    if format == Format::Markdown {
        t.markdown()
    } else {
        t.text(false)
    }
}
