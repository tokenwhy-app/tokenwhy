// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use crate::bucket;
use crate::week::Week;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use tokenwhy_core::history::{By, History};
use tokenwhy_core::{lang, Category, DailyRow, Snapshot, StartupKind};

pub const VERSION: u8 = 1;

pub const NOT_SHARED: &str = "not-shared";

/// Released `major.minor` versions the Worker accepts; a release adds its own.
pub const APP_VERSIONS: &[&str] = &["0.1"];

pub const RULES: std::ops::RangeInclusive<u8> = 1..=18;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Os {
    Windows,
    Macos,
    Linux,
}

pub const OS: &[&str] = &["windows", "macos", "linux"];

impl Os {
    pub fn current() -> Os {
        if cfg!(windows) {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::Macos
        } else {
            Os::Linux
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Provider {
    #[serde(rename = "claude-code")]
    ClaudeCode,
    #[serde(rename = "codex")]
    Codex,
    #[serde(rename = "opencode")]
    Opencode,
    #[serde(rename = "agy")]
    Agy,
}

pub const PROVIDERS: &[&str] = &["claude-code", "codex", "opencode", "agy"];

impl Provider {
    fn of(id: &str) -> Option<Provider> {
        Some(match id {
            "claude-code" => Provider::ClaudeCode,
            "codex" => Provider::Codex,
            "opencode" => Provider::Opencode,
            "agy" => Provider::Agy,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Family {
    ClaudeOpus,
    ClaudeSonnet,
    ClaudeHaiku,
    Gpt,
    Gemini,
    Other,
}

pub const FAMILIES: &[&str] = &[
    "claude-opus",
    "claude-sonnet",
    "claude-haiku",
    "gpt",
    "gemini",
    "other",
];

impl Family {
    pub fn of(model: &str) -> Family {
        let m = model.to_ascii_lowercase();
        let o_series = ["o1", "o3", "o4"].iter().any(|p| m.starts_with(p));
        if m.contains("opus") {
            Family::ClaudeOpus
        } else if m.contains("sonnet") {
            Family::ClaudeSonnet
        } else if m.contains("haiku") {
            Family::ClaudeHaiku
        } else if m.contains("gpt") || m.contains("codex") || o_series {
            Family::Gpt
        } else if m.contains("gemini") {
            Family::Gemini
        } else {
            Family::Other
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Rule(pub u8);

impl Rule {
    fn of(id: &str) -> Option<Rule> {
        let n: u8 = id.strip_prefix('R')?.parse().ok()?;
        RULES.contains(&n).then_some(Rule(n))
    }
}

impl Serialize for Rule {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("R{}", self.0))
    }
}

impl<'de> Deserialize<'de> for Rule {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Rule::of(&s).ok_or_else(|| serde::de::Error::custom("rule"))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Payload {
    pub v: u8,
    pub install: String,
    pub week: String,
    pub app: String,
    pub os: Os,
    pub providers: Vec<Provider>,
    pub active_days: u8,
    pub sessions: u64,
    pub turns: u64,
    pub fresh_tok: u64,
    pub cache_read_tok: u64,
    pub cache_write_tok: u64,
    pub output_tok: Option<u64>,
    pub cost_cents: Option<u64>,
    pub model_1: Option<Family>,
    pub model_1_s: Option<u16>,
    pub model_2: Option<Family>,
    pub model_2_s: Option<u16>,
    pub ctx_mean: u64,
    pub ctx_peak: u64,
    pub startup_med: u64,
    pub startup_p90: u64,
    pub su_system_s: u16,
    pub su_tools_s: u16,
    pub su_mcp_s: u16,
    pub su_skills_s: u16,
    pub su_agents_s: u16,
    pub su_instr_s: u16,
    pub su_memory_s: u16,
    pub skills_listed: u64,
    pub skills_used: u64,
    pub skill_listing_tok: u64,
    pub skill_top_s: u16,
    pub memory_files: u64,
    pub memory_tok: u64,
    pub memory_dup_lines: u64,
    pub agents: u64,
    pub agent_cost_s: Option<u16>,
    #[serde(rename = "tool_read-file_s")]
    pub tool_read_file_s: u16,
    pub tool_search_s: u16,
    pub tool_list_s: u16,
    pub tool_vcs_s: u16,
    pub tool_build_s: u16,
    pub tool_test_s: u16,
    pub tool_logs_s: u16,
    pub tool_script_s: u16,
    #[serde(rename = "tool_project-tool_s")]
    pub tool_project_tool_s: u16,
    pub tool_web_s: u16,
    pub tool_agent_s: u16,
    pub tool_mcp_s: u16,
    pub tool_edit_s: u16,
    pub tool_other_s: u16,
    pub tool_over_25k: u64,
    pub compactions: u64,
    pub carry_ratio: Option<u16>,
    pub lang_1: Option<u8>,
    pub lang_1_s: Option<u16>,
    pub lang_2: Option<u8>,
    pub lang_2_s: Option<u16>,
    pub lang_3: Option<u8>,
    pub lang_3_s: Option<u16>,
    pub rules: Vec<Rule>,
}

impl Payload {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn to_pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

pub struct Inputs<'a> {
    pub snap: &'a Snapshot,
    pub days: Vec<DailyRow>,
    pub by_model: Vec<DailyRow>,
    pub by_provider: Vec<DailyRow>,
    pub langs: Vec<(u8, u32, u32)>,
}

pub fn query(week: Week) -> tokenwhy_core::Query {
    tokenwhy_core::Query {
        window: tokenwhy_core::Window {
            from_ms: week.from_ms(),
            to_ms: week.to_ms(),
        },
        project: None,
        anonymise: tokenwhy_core::AnonLevel::Strict,
        provider: None,
        account: None,
    }
}

pub fn build(snap: &Snapshot, hist: &History, week: Week, install: Option<&str>) -> Payload {
    let (from, to) = (week.from_ms(), week.to_ms());
    let mut langs: BTreeMap<u8, (u32, u32)> = BTreeMap::new();
    for e in hist.entries_for(from, to, None, None) {
        for d in &e.days {
            for (id, ed, rd) in d.langs.iter().flatten() {
                let a = langs.entry(*id).or_default();
                a.0 += ed;
                a.1 += rd;
            }
        }
    }
    assemble(
        &Inputs {
            snap,
            days: hist.daily(from, to, By::None, None, None),
            by_model: hist.daily(from, to, By::Model, None, None),
            by_provider: hist.daily(from, to, By::Provider, None, None),
            langs: langs.into_iter().map(|(k, (e, r))| (k, e, r)).collect(),
        },
        week,
        install,
    )
}

fn tokens(r: &DailyRow) -> u64 {
    r.fresh_tokens + r.cache_read_tokens + r.cache_write_tokens + r.output_tokens.unwrap_or(0)
}

fn ratio_of(part: f64, total: f64) -> f64 {
    if total > 0.0 {
        part / total
    } else {
        0.0
    }
}

fn weighted(mut v: Vec<(f64, f64)>, q: f64) -> f64 {
    v.retain(|(x, w)| x.is_finite() && *w > 0.0);
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total: f64 = v.iter().map(|x| x.1).sum();
    let mut acc = 0.0;
    for (x, w) in &v {
        acc += w;
        if acc >= total * q {
            return *x;
        }
    }
    0.0
}

fn quantile(v: Vec<f64>, q: f64) -> f64 {
    weighted(v.into_iter().map(|x| (x, 1.0)).collect(), q)
}

fn sum_opt(it: impl Iterator<Item = Option<f64>>) -> Option<f64> {
    it.fold(None, |acc, x| match (acc, x) {
        (a, None) => a,
        (None, Some(x)) => Some(x),
        (Some(a), Some(x)) => Some(a + x),
    })
}

pub fn assemble(i: &Inputs, week: Week, install: Option<&str>) -> Payload {
    let s = i.snap;
    let days = &i.days;
    let active: BTreeSet<i64> = days
        .iter()
        .filter(|r| tokens(r) > 0)
        .map(|r| r.day_ms)
        .collect();
    let cost = sum_opt(days.iter().map(|r| r.cost_usd.value));
    let output = days
        .iter()
        .filter_map(|r| r.output_tokens)
        .fold(None, |a: Option<u64>, x| Some(a.unwrap_or(0) + x));

    let mut fam: BTreeMap<Family, f64> = BTreeMap::new();
    for r in &i.by_model {
        if let (Some(m), Some(c)) = (&r.model, r.cost_usd.value) {
            *fam.entry(Family::of(m)).or_default() += c;
        }
    }
    let fam_total: f64 = fam.values().sum();
    let mut fams: Vec<(Family, f64)> = fam.into_iter().filter(|x| x.1 > 0.0).collect();
    fams.sort_by(|a, b| b.1.total_cmp(&a.1));
    let fam_at = |k: usize| {
        fams.get(k)
            .map(|(f, c)| (*f, bucket::share(ratio_of(*c, fam_total))))
    };

    let providers: BTreeSet<Provider> = i
        .by_provider
        .iter()
        .filter(|r| tokens(r) > 0)
        .filter_map(|r| r.provider.as_ref().and_then(|p| Provider::of(&p.0)))
        .collect();

    let mut su: BTreeMap<&str, f64> = BTreeMap::new();
    let mut su_total = 0.0;
    for row in &s.startup {
        let w = f64::from(row.sessions);
        for p in &row.parts {
            let t = p.tokens.value.unwrap_or(0.0) * w;
            su_total += t;
            let k = match p.kind {
                StartupKind::SystemPrompt => "system",
                StartupKind::BuiltinTools => "tools",
                StartupKind::McpTools
                | StartupKind::DeferredTools
                | StartupKind::McpInstructions => "mcp",
                StartupKind::SkillListing => "skills",
                StartupKind::AgentListing => "agents",
                StartupKind::UserInstructions | StartupKind::ProjectInstructions => "instr",
                StartupKind::Memory => "memory",
                StartupKind::Base => continue,
            };
            *su.entry(k).or_default() += t;
        }
        su_total += row.unattributed.value.unwrap_or(0.0) * w;
    }
    let su_s = |k: &str| bucket::share(ratio_of(su.get(k).copied().unwrap_or(0.0), su_total));

    let listed: Vec<f64> = s
        .skills
        .iter()
        .filter(|r| r.sessions_listed > 0)
        .map(|r| r.listing_tokens_median.value.unwrap_or(0.0))
        .collect();
    let listing_tok: f64 = listed.iter().sum();
    let top = listed.iter().copied().fold(0.0, f64::max);
    let skills_listed = listed.len() as u64;
    let skills_used =
        (s.skills.iter().filter(|r| r.activations > 0).count() as u64).min(skills_listed);

    let mut tool: BTreeMap<Category, f64> = BTreeMap::new();
    for r in &s.tools {
        *tool.entry(r.category).or_default() += r.share;
    }
    let tool_s = |c: Category| bucket::share(tool.get(&c).copied().unwrap_or(0.0));

    let agent_cost: f64 = days.iter().map(|r| r.agent_cost_usd).sum();
    let carry: Vec<f64> = s.compactions.iter().filter_map(|c| c.carry_ratio).collect();

    let mut ls: Vec<(u8, u32, u32)> = i
        .langs
        .iter()
        .copied()
        .filter(|(id, e, r)| usize::from(*id) < lang::NAMES.len() && e + r > 0)
        .collect();
    let ls_total: f64 = ls.iter().map(|(_, e, r)| f64::from(e + r)).sum();
    ls.sort_by(|a, b| (b.1, b.2, a.0).cmp(&(a.1, a.2, b.0)));
    let lang_at = |k: usize| {
        ls.get(k)
            .map(|(id, e, r)| (*id, bucket::share(ratio_of(f64::from(e + r), ls_total))))
    };

    let rules: BTreeSet<Rule> = s
        .findings
        .iter()
        .filter_map(|f| Rule::of(&f.rule))
        .collect();

    let mut p = Payload {
        v: VERSION,
        install: install.unwrap_or(NOT_SHARED).to_string(),
        week: week.label(),
        app: app_version(),
        os: Os::current(),
        providers: providers.into_iter().collect(),
        active_days: active.len().min(7) as u8,
        sessions: bucket::count(days.iter().map(|r| u64::from(r.sessions)).sum()),
        turns: bucket::count(s.sessions.iter().map(|r| u64::from(r.turns)).sum()),
        fresh_tok: bucket::count(days.iter().map(|r| r.fresh_tokens).sum()),
        cache_read_tok: bucket::count(days.iter().map(|r| r.cache_read_tokens).sum()),
        cache_write_tok: bucket::count(days.iter().map(|r| r.cache_write_tokens).sum()),
        output_tok: output.map(bucket::count),
        cost_cents: cost.map(bucket::cents),
        model_1: fam_at(0).map(|x| x.0),
        model_1_s: fam_at(0).map(|x| x.1),
        model_2: fam_at(1).map(|x| x.0),
        model_2_s: fam_at(1).map(|x| x.1),
        ctx_mean: bucket::count_f(quantile(
            s.sessions.iter().map(|r| r.mean_ctx).collect(),
            0.5,
        )),
        ctx_peak: bucket::count_f(quantile(
            s.sessions.iter().map(|r| r.peak_ctx as f64).collect(),
            0.9,
        )),
        startup_med: bucket::count_f(weighted(
            s.startup
                .iter()
                .map(|r| (r.median as f64, f64::from(r.sessions)))
                .collect(),
            0.5,
        )),
        startup_p90: bucket::count_f(weighted(
            s.startup
                .iter()
                .map(|r| (r.p90 as f64, f64::from(r.sessions)))
                .collect(),
            0.5,
        )),
        su_system_s: su_s("system"),
        su_tools_s: su_s("tools"),
        su_mcp_s: su_s("mcp"),
        su_skills_s: su_s("skills"),
        su_agents_s: su_s("agents"),
        su_instr_s: su_s("instr"),
        su_memory_s: su_s("memory"),
        skills_listed: bucket::count(skills_listed),
        skills_used: bucket::count(skills_used),
        skill_listing_tok: bucket::count_f(listing_tok),
        skill_top_s: bucket::share(ratio_of(top, listing_tok)),
        memory_files: bucket::count(s.memory.len() as u64),
        memory_tok: bucket::count_f(s.memory.iter().map(|m| m.tokens.value.unwrap_or(0.0)).sum()),
        memory_dup_lines: bucket::count(
            s.memory.iter().map(|m| u64::from(m.duplicate_lines)).sum(),
        ),
        agents: bucket::count(days.iter().map(|r| u64::from(r.agents)).sum()),
        agent_cost_s: cost
            .filter(|c| *c > 0.0)
            .map(|c| bucket::share(agent_cost / c)),
        tool_read_file_s: tool_s(Category::ReadFile),
        tool_search_s: tool_s(Category::Search),
        tool_list_s: tool_s(Category::List),
        tool_vcs_s: tool_s(Category::Vcs),
        tool_build_s: tool_s(Category::Build),
        tool_test_s: tool_s(Category::Test),
        tool_logs_s: tool_s(Category::Logs),
        tool_script_s: tool_s(Category::Script),
        tool_project_tool_s: tool_s(Category::ProjectTool),
        tool_web_s: tool_s(Category::Web),
        tool_agent_s: tool_s(Category::Agent),
        tool_mcp_s: tool_s(Category::Mcp),
        tool_edit_s: tool_s(Category::Edit),
        tool_other_s: tool_s(Category::Other),
        tool_over_25k: bucket::count(s.tools.iter().map(|r| u64::from(r.over_25k)).sum()),
        compactions: bucket::count(days.iter().map(|r| u64::from(r.compactions)).sum()),
        carry_ratio: (!carry.is_empty()).then(|| bucket::ratio(quantile(carry, 0.5))),
        lang_1: lang_at(0).map(|x| x.0),
        lang_1_s: lang_at(0).map(|x| x.1),
        lang_2: lang_at(1).map(|x| x.0),
        lang_2_s: lang_at(1).map(|x| x.1),
        lang_3: lang_at(2).map(|x| x.0),
        lang_3_s: lang_at(2).map(|x| x.1),
        rules: rules.into_iter().collect(),
    };
    if p.active_days == 0 {
        zero_counts(&mut p);
    }
    p
}

fn zero_counts(p: &mut Payload) {
    let Ok(Value::Object(mut m)) = serde_json::to_value(&*p) else {
        return;
    };
    for (k, kind) in FIELDS {
        if matches!(kind, Kind::Count | Kind::CountOrNull) {
            if let Some(v) = m.get_mut(*k) {
                if !v.is_null() {
                    *v = Value::from(0);
                }
            }
        }
    }
    if let Ok(z) = serde_json::from_value(Value::Object(m)) {
        *p = z;
    }
}

pub fn app_version() -> String {
    let v = env!("CARGO_PKG_VERSION");
    let mut it = v.split('.');
    format!("{}.{}", it.next().unwrap_or("0"), it.next().unwrap_or("0"))
}

// Closed schema: the checks the Worker runs (design section 8).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Version,
    Install,
    Week,
    App,
    Os,
    Providers,
    Days,
    Count,
    CountOrNull,
    Share,
    ShareOrNull,
    FamilyOrNull,
    RatioOrNull,
    LangOrNull,
    Rules,
}

pub const FIELDS: &[(&str, Kind)] = &[
    ("v", Kind::Version),
    ("install", Kind::Install),
    ("week", Kind::Week),
    ("app", Kind::App),
    ("os", Kind::Os),
    ("providers", Kind::Providers),
    ("active_days", Kind::Days),
    ("sessions", Kind::Count),
    ("turns", Kind::Count),
    ("fresh_tok", Kind::Count),
    ("cache_read_tok", Kind::Count),
    ("cache_write_tok", Kind::Count),
    ("output_tok", Kind::CountOrNull),
    ("cost_cents", Kind::CountOrNull),
    ("model_1", Kind::FamilyOrNull),
    ("model_1_s", Kind::ShareOrNull),
    ("model_2", Kind::FamilyOrNull),
    ("model_2_s", Kind::ShareOrNull),
    ("ctx_mean", Kind::Count),
    ("ctx_peak", Kind::Count),
    ("startup_med", Kind::Count),
    ("startup_p90", Kind::Count),
    ("su_system_s", Kind::Share),
    ("su_tools_s", Kind::Share),
    ("su_mcp_s", Kind::Share),
    ("su_skills_s", Kind::Share),
    ("su_agents_s", Kind::Share),
    ("su_instr_s", Kind::Share),
    ("su_memory_s", Kind::Share),
    ("skills_listed", Kind::Count),
    ("skills_used", Kind::Count),
    ("skill_listing_tok", Kind::Count),
    ("skill_top_s", Kind::Share),
    ("memory_files", Kind::Count),
    ("memory_tok", Kind::Count),
    ("memory_dup_lines", Kind::Count),
    ("agents", Kind::Count),
    ("agent_cost_s", Kind::ShareOrNull),
    ("tool_read-file_s", Kind::Share),
    ("tool_search_s", Kind::Share),
    ("tool_list_s", Kind::Share),
    ("tool_vcs_s", Kind::Share),
    ("tool_build_s", Kind::Share),
    ("tool_test_s", Kind::Share),
    ("tool_logs_s", Kind::Share),
    ("tool_script_s", Kind::Share),
    ("tool_project-tool_s", Kind::Share),
    ("tool_web_s", Kind::Share),
    ("tool_agent_s", Kind::Share),
    ("tool_mcp_s", Kind::Share),
    ("tool_edit_s", Kind::Share),
    ("tool_other_s", Kind::Share),
    ("tool_over_25k", Kind::Count),
    ("compactions", Kind::Count),
    ("carry_ratio", Kind::RatioOrNull),
    ("lang_1", Kind::LangOrNull),
    ("lang_1_s", Kind::ShareOrNull),
    ("lang_2", Kind::LangOrNull),
    ("lang_2_s", Kind::ShareOrNull),
    ("lang_3", Kind::LangOrNull),
    ("lang_3_s", Kind::ShareOrNull),
    ("rules", Kind::Rules),
];

const GROUPS: &[&str] = &["su_", "tool_", "model_", "lang_"];

fn is_hex_id(s: &str) -> bool {
    s.len() == 32
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn in_str(v: &Value, set: &[&str]) -> bool {
    v.as_str().is_some_and(|s| set.contains(&s))
}

fn in_u64(v: &Value, set: &[u64]) -> bool {
    v.as_u64().is_some_and(|x| set.contains(&x))
}

fn in_u16(v: &Value, set: &[u16]) -> bool {
    v.as_u64()
        .is_some_and(|x| set.iter().any(|e| u64::from(*e) == x))
}

fn set_of(v: &Value, ok: impl Fn(&Value) -> bool) -> bool {
    let Some(a) = v.as_array() else { return false };
    let mut seen = BTreeSet::new();
    a.iter().all(|x| ok(x) && seen.insert(x.to_string()))
}

fn rule_ok(v: &Value) -> bool {
    v.as_str().and_then(Rule::of).is_some()
}

fn value_ok(kind: Kind, v: &Value, last: Week) -> bool {
    let null = v.is_null();
    match kind {
        Kind::Version => v.as_u64() == Some(u64::from(VERSION)),
        Kind::Install => v.as_str().is_some_and(is_hex_id),
        Kind::Week => v
            .as_str()
            .and_then(Week::parse)
            .is_some_and(|w| w <= last && w >= last.back(5)),
        Kind::App => in_str(v, APP_VERSIONS),
        Kind::Os => in_str(v, OS),
        Kind::Providers => set_of(v, |x| in_str(x, PROVIDERS)),
        Kind::Days => v.as_u64().is_some_and(|x| x <= 7),
        Kind::Count => in_u64(v, &bucket::COUNT),
        Kind::CountOrNull => null || in_u64(v, &bucket::COUNT),
        Kind::Share => in_u16(v, &bucket::SHARE),
        Kind::ShareOrNull => null || in_u16(v, &bucket::SHARE),
        Kind::FamilyOrNull => null || in_str(v, FAMILIES),
        Kind::RatioOrNull => null || in_u16(v, &bucket::RATIO),
        Kind::LangOrNull => null || v.as_u64().is_some_and(|x| (x as usize) < lang::NAMES.len()),
        Kind::Rules => set_of(v, rule_ok),
    }
}

pub fn validate(body: &Value, last: Week) -> Result<(), String> {
    let m = body.as_object().ok_or("body")?;
    if let Some(k) = m
        .keys()
        .find(|k| !FIELDS.iter().any(|(f, _)| *f == k.as_str()))
    {
        return Err(k.clone());
    }
    for (k, kind) in FIELDS {
        let v = m.get(*k).ok_or_else(|| k.to_string())?;
        if !value_ok(*kind, v, last) {
            return Err(k.to_string());
        }
    }
    for g in GROUPS {
        let shares: Vec<u64> = FIELDS
            .iter()
            .filter(|(k, kind)| k.starts_with(g) && matches!(kind, Kind::Share | Kind::ShareOrNull))
            .filter_map(|(k, _)| m[*k].as_u64())
            .collect();
        if shares.iter().sum::<u64>() > 1000 + 100 * shares.len() as u64 {
            return Err(format!("{g}*"));
        }
    }
    if m["active_days"].as_u64() == Some(0) {
        let counted = FIELDS.iter().find(|(k, kind)| {
            matches!(kind, Kind::Count | Kind::CountOrNull) && m[*k].as_u64().is_some_and(|x| x > 0)
        });
        if let Some((k, _)) = counted {
            return Err(k.to_string());
        }
    }
    if m["skills_used"].as_u64() > m["skills_listed"].as_u64() {
        return Err("skills_used".into());
    }
    Ok(())
}

pub fn schema() -> Value {
    let kinds: Vec<Value> = FIELDS
        .iter()
        .map(|(k, kind)| serde_json::json!({ "key": k, "kind": format!("{kind:?}") }))
        .collect();
    serde_json::json!({
        "v": VERSION,
        "fields": kinds,
        "count_edges": bucket::COUNT,
        "share_edges": bucket::SHARE,
        "ratio_edges": bucket::RATIO,
        "app": APP_VERSIONS,
        "os": OS,
        "providers": PROVIDERS,
        "families": FAMILIES,
        "langs": lang::NAMES.len(),
        "rules": [RULES.start(), RULES.end()],
        "max_week_age": 5,
    })
}

#[cfg(test)]
mod tests;
