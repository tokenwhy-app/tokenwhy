// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProviderId(pub String);

/// FNV-1a 64, stable across Rust versions.
pub type IdHash = u64;

pub type TimestampMs = i64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Level {
    Full,
    Partial,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CostSource {
    Table,
    Stored,
    Estimate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Vendor {
    Anthropic,
    OpenAi,
    Google,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    pub turns: Level,
    pub cache_read: Level,
    pub cache_write: Level,
    pub tool_outputs: Level,
    pub skills: Level,
    pub startup: Level,
    pub memory: Level,
    pub subagents: Level,
    pub compactions: Level,
    pub mcp: Level,
    pub cost: CostSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FileRole {
    Main,
    Agent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceFile {
    pub provider: ProviderId,
    pub path: PathBuf,
    pub role: FileRole,
    pub project_key: String,
    pub session_id: String,
    pub agent_type: Option<String>,
    pub agent_group: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub input: u64,
    pub cache_write_5m: u64,
    pub cache_write_1h: u64,
    pub cache_read: u64,
    pub output: u64,
    pub web_search_requests: u32,
}

pub const MAX_CALL_TOKENS: u64 = 1_000_000_000;

impl Usage {
    pub fn plausible(&self) -> bool {
        [
            self.input,
            self.cache_write_5m,
            self.cache_write_1h,
            self.cache_read,
            self.output,
        ]
        .iter()
        .all(|&n| n <= MAX_CALL_TOKENS)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Turn {
    pub msg_id: IdHash,
    pub first_ts_ms: TimestampMs,
    pub model: u32,
    pub usage: Usage,
    pub attribution_skill: Option<u32>,
    pub segment: u32,
    pub order: u32,
    #[serde(default)]
    pub stored_cost: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    ReadFile,
    Search,
    List,
    Vcs,
    Build,
    Test,
    Logs,
    Script,
    ProjectTool,
    Web,
    Agent,
    Mcp,
    Edit,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub call_id: IdHash,
    pub turn_order: u32,
    pub tool: u32,
    pub category: Category,
    pub detail: Option<u32>,
    pub bounded: bool,
    pub path: Option<(IdHash, u32)>,
    pub writes_path: bool,
    pub result_chars: u64,
    pub is_error: bool,
    pub persisted: bool,
    pub images: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillLoad {
    pub skill: u32,
    pub body_chars: u64,
    pub order: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Compaction {
    pub order: u32,
    pub trigger: u32,
    pub pre_tokens: u64,
    pub post_tokens: u64,
    pub summary_chars: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StartupKind {
    SystemPrompt,
    BuiltinTools,
    McpTools,
    DeferredTools,
    SkillListing,
    AgentListing,
    McpInstructions,
    UserInstructions,
    ProjectInstructions,
    Memory,
    Base,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartupPart {
    pub kind: StartupKind,
    pub name: u32,
    pub chars: u64,
    pub content_hash: IdHash,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartupParts {
    pub parts: Vec<StartupPart>,
    pub has_full_shape: bool,
    pub instruction_lines: Vec<(u32, Vec<IdHash>)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpInventory {
    pub loaded: Vec<(u32, u32)>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FileRecords {
    pub strings: Vec<String>,
    pub project_name: Option<u32>,
    pub turns: Vec<Turn>,
    pub tool_calls: Vec<ToolCall>,
    pub skill_loads: Vec<SkillLoad>,
    pub compactions: Vec<Compaction>,
    pub startup: Option<StartupParts>,
    pub mcp: McpInventory,
    pub human_prompts: Vec<u32>,
    pub offset_done: u64,
    pub open_calls: Vec<IdHash>,
    #[serde(skip)]
    pub head_copied: bool,
}

impl FileRecords {
    pub fn segment_at(&self, order: u32) -> u32 {
        self.compactions.iter().filter(|c| c.order < order).count() as u32
    }

    pub fn resegment(&mut self) {
        let segs: Vec<u32> = self
            .turns
            .iter()
            .map(|t| self.segment_at(t.order))
            .collect();
        for (t, s) in self.turns.iter_mut().zip(segs) {
            t.segment = s;
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ParseResume {
    pub offset: u64,
    pub previous: FileRecords,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedFile {
    pub source: SourceFile,
    pub records: FileRecords,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelRate {
    pub prefix: String,
    pub input: f64,
    pub cache_write_5m: f64,
    pub cache_write_1h: f64,
    pub cache_read: f64,
    pub output: f64,
    pub web_search: f64,
    pub long_context_threshold: Option<u64>,
    pub long_context: Option<Box<ModelRate>>,
    pub checked: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PriceTable {
    pub version: u32,
    pub provider: ProviderId,
    pub rates: Vec<ModelRate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Window {
    pub from_ms: TimestampMs,
    pub to_ms: TimestampMs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Query {
    pub window: Window,
    pub project: Option<String>,
    pub anonymise: AnonLevel,
    #[serde(default)]
    pub provider: Option<ProviderId>,
    #[serde(default)]
    pub account: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AnonLevel {
    #[default]
    Off,
    Names,
    Strict,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompactView {
    A,
    B,
    #[default]
    C,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    #[default]
    BottomRight,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Thresholds {
    pub startup_median_tokens: u64,
    pub startup_min_sessions: u32,
    pub memory_bytes: u64,
    pub plugin_min_sessions: u32,
    pub mcp_min_tools: u32,
    pub mcp_min_sessions: u32,
    pub mcp_called_share: f64,
    pub skill_body_tokens: u64,
    pub skill_listing_chars: u64,
    #[serde(alias = "claude_md_duplicates")]
    pub instruction_duplicates: u32,
    pub subagent_cost_share: f64,
    pub reader_agent_opus_share: f64,
    pub compactions_per_session: u32,
    pub carry_ratio: f64,
    pub read_file_share: f64,
    pub search_share: f64,
    pub search_median_chars: u64,
    pub big_result_chars: u64,
    pub big_result_count: u32,
    pub list_share: f64,
    pub build_test_share: f64,
    pub reread_count: u32,
    pub segment_mean_ctx: u64,
    pub segment_turns: u32,
    pub cache_hit: f64,
    pub cache_hit_min_turns: u32,
    pub min_saved_per_week: u64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            startup_median_tokens: 45_000,
            startup_min_sessions: 3,
            memory_bytes: 6_000,
            plugin_min_sessions: 10,
            mcp_min_tools: 10,
            mcp_min_sessions: 2,
            mcp_called_share: 0.20,
            skill_body_tokens: 8_000,
            skill_listing_chars: 500,
            instruction_duplicates: 3,
            subagent_cost_share: 0.50,
            reader_agent_opus_share: 0.20,
            compactions_per_session: 2,
            carry_ratio: 3.0,
            read_file_share: 0.20,
            search_share: 0.15,
            search_median_chars: 2_000,
            big_result_chars: 25_000,
            big_result_count: 20,
            list_share: 0.05,
            build_test_share: 0.05,
            reread_count: 5,
            segment_mean_ctx: 450_000,
            segment_turns: 100,
            cache_hit: 0.90,
            cache_hit_min_turns: 50,
            min_saved_per_week: 1_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub compact_view: CompactView,
    pub always_on_top: bool,
    pub corner: Corner,
    pub autostart: bool,
    pub window_days: u32,
    pub project_tools: Vec<String>,
    pub reader_agent_types: Vec<String>,
    pub thresholds: Thresholds,
    pub anon_salt: u64,
    #[cfg(feature = "co2")]
    pub co2: crate::co2::Co2Settings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            compact_view: CompactView::C,
            always_on_top: false,
            corner: Corner::BottomRight,
            autostart: true,
            window_days: 14,
            project_tools: vec!["codeq".into(), "logq".into()],
            reader_agent_types: vec!["Explore".into(), "chore".into()],
            thresholds: Thresholds::default(),
            anon_salt: 0,
            #[cfg(feature = "co2")]
            co2: crate::co2::Co2Settings::default(),
        }
    }
}

// Snapshot: the JSON contract with app/web/snapshot.schema.json.

pub const SNAPSHOT_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Amount {
    pub value: Option<f64>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub est: bool,
}

impl Amount {
    pub fn none() -> Amount {
        Amount {
            value: None,
            est: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub schema: u32,
    pub generated_ms: TimestampMs,
    pub window: Window,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weeks: Option<f64>,
    pub anonymised: bool,
    pub progress: f64,
    #[serde(default)]
    pub providers: Vec<ProviderInfo>,
    #[serde(default)]
    pub accounts: Vec<AccountInfo>,
    pub live: Live,
    pub findings: Vec<Finding>,
    pub startup: Vec<StartupRow>,
    pub skills: Vec<SkillRow>,
    pub memory: Vec<MemoryRow>,
    pub subagents: Vec<SubagentRow>,
    pub compactions: Vec<CompactionRow>,
    pub tools: Vec<ToolRow>,
    pub unpriced_models: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_newer: Option<u32>,
    #[serde(default)]
    pub daily: Vec<DailyRow>,
    #[serde(default)]
    pub week: Option<WeekDeltas>,
    #[serde(default)]
    pub sessions: Vec<SessionRow>,
    #[serde(default)]
    pub hourly: Vec<HourRow>,
    #[cfg(feature = "co2")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub co2: Option<crate::co2::Co2Info>,
    #[cfg(feature = "plan")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plan: Vec<PlanRow>,
}

#[cfg(feature = "plan")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanRow {
    pub account: String,
    pub label: String,
    pub five_hour_pct: Option<f64>,
    pub five_hour_resets_ms: Option<TimestampMs>,
    pub seven_day_pct: Option<f64>,
    pub seven_day_resets_ms: Option<TimestampMs>,
    pub fetched_ms: TimestampMs,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderInfo {
    pub id: ProviderId,
    pub label: String,
    pub roots: u32,
    pub files: u32,
    pub caps: Capabilities,
    #[serde(default)]
    pub imported: bool,
    #[serde(default)]
    pub skipped: Vec<SkipCount>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountInfo {
    pub id: String,
    pub label: String,
    pub sessions: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkipCount {
    pub reason: String,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionRow {
    pub provider: ProviderId,
    pub project: String,
    pub session: String,
    pub start_ms: TimestampMs,
    pub turns: u32,
    pub mean_ctx: f64,
    pub peak_ctx: u64,
    pub cost_usd: Amount,
    #[serde(default = "Amount::none")]
    pub co2_g: Amount,
    pub compactions: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HourRow {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderId>,
    pub day_ms: TimestampMs,
    pub hour: u8,
    pub cost_usd: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DailyRow {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderId>,
    pub day_ms: TimestampMs,
    pub project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub fresh_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub cost_usd: Amount,
    pub sessions: u32,
    pub agents: u32,
    pub compactions: u32,
    #[serde(default)]
    pub output_tokens: Option<u64>,
    #[serde(default)]
    pub edits: Option<u32>,
    #[serde(default)]
    pub active_hours: Option<u32>,
    #[serde(default)]
    pub co2_g: Option<f64>,
    #[serde(default)]
    pub co2_g_max: Option<f64>,
    #[serde(default)]
    pub co2_cache_g: Option<f64>,
    #[serde(default)]
    pub water_l: Option<f64>,
    #[serde(default)]
    pub water_l_max: Option<f64>,
    #[serde(default)]
    pub agent_cost_usd: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeekDelta {
    pub this: f64,
    pub last: f64,
    pub delta_pct: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeekDeltas {
    pub fresh_tokens: WeekDelta,
    pub cache_read_tokens: WeekDelta,
    pub cost_usd: WeekDelta,
    pub sessions: WeekDelta,
    pub agents: WeekDelta,
    #[serde(default = "WeekDelta::zero")]
    pub agent_cost_usd: WeekDelta,
    #[serde(default)]
    pub output_tokens: Option<WeekDelta>,
    #[serde(default)]
    pub edits: Option<WeekDelta>,
    #[serde(default)]
    pub active_hours: Option<WeekDelta>,
    #[serde(default)]
    pub co2_g: Option<WeekDelta>,
    #[serde(default)]
    pub co2_g_max: Option<WeekDelta>,
    #[serde(default)]
    pub co2_cache_g: Option<WeekDelta>,
    #[serde(default)]
    pub water_l: Option<WeekDelta>,
    #[serde(default)]
    pub water_l_max: Option<WeekDelta>,
}

impl WeekDelta {
    fn zero() -> WeekDelta {
        WeekDelta {
            this: 0.0,
            last: 0.0,
            delta_pct: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Live {
    pub today: Today,
    pub sessions: Vec<ActiveSession>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Today {
    pub fresh_tokens: u64,
    pub cache_read_tokens: u64,
    pub cost_usd: Amount,
    pub sessions: u32,
    pub agents: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveSession {
    pub provider: ProviderId,
    pub project: String,
    pub session: String,
    pub ctx_tokens: u64,
    pub turns: u32,
    pub cost_usd: Amount,
    pub agents_running: u32,
    pub compactions: u32,
    pub ctx_growth_since_boundary: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderId>,
    pub rule: String,
    pub project: Option<String>,
    /// One number + one action, at most 110 chars, no grades or exclamation marks.
    pub line: String,
    pub tokens_saved_per_week: Amount,
    #[serde(default = "Amount::none")]
    pub usd_saved_per_week: Amount,
    #[serde(default = "Amount::none")]
    pub co2_g_saved_per_week: Amount,
    #[serde(default = "Amount::none")]
    pub usd_saved_last_7d: Amount,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartupRow {
    pub provider: ProviderId,
    pub project: String,
    pub sessions: u32,
    pub median: u64,
    pub p90: u64,
    pub min: u64,
    pub max: u64,
    pub ratio: Option<f64>,
    pub parts: Vec<StartupPartRow>,
    pub unattributed: Amount,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartupPartRow {
    pub kind: StartupKind,
    pub name: String,
    pub tokens: Amount,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillRow {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderId>,
    pub skill: String,
    pub listing_tokens_median: Amount,
    pub activations: u32,
    pub body_tokens_median: Amount,
    pub carried_cost_usd: Amount,
    pub sessions_loaded: u32,
    pub sessions_listed: u32,
    pub attributed_turns: u32,
    pub attributed_cost_usd: Amount,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryRow {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderId>,
    pub kind: StartupKind,
    pub path: String,
    #[serde(default)]
    pub label: String,
    pub bytes: u64,
    pub tokens: Amount,
    pub carried_cost_usd: Amount,
    pub on_demand_reads: u32,
    pub duplicate_lines: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubagentRow {
    pub provider: ProviderId,
    pub project: String,
    pub agent_type: Option<String>,
    pub model: Option<String>,
    pub agents: u32,
    pub cost_usd: Amount,
    pub median_cost_usd: Amount,
    pub cost_share: f64,
    pub fresh_share: f64,
    pub startup_share: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompactionRow {
    pub provider: ProviderId,
    pub project: String,
    pub session: String,
    pub ts_ms: TimestampMs,
    pub pre_tokens: u64,
    pub post_tokens: u64,
    pub first_after_ctx: u64,
    pub cost_per_turn_before: Amount,
    pub cost_per_turn_after: Amount,
    pub rebuild_tokens: Amount,
    pub carry_ratio: Option<f64>,
    pub fresh_restart_cost_usd: Amount,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolRow {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderId>,
    pub category: Category,
    pub detail: Option<String>,
    pub project: Option<String>,
    pub calls: u32,
    pub first_tokens: Amount,
    pub carried_tokens: Amount,
    pub share: f64,
    pub median_chars: u64,
    pub p95_chars: u64,
    pub over_25k: u32,
    pub persisted: u32,
}
