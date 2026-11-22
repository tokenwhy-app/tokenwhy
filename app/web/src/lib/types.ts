// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

// snapshot.schema.json is the contract.

export interface Quantity { value: number | null; est?: boolean; unknown?: boolean }

export interface ProviderInfo {
  id: string;
  label: string;
  roots: number;
  files: number;
  imported?: boolean;
  caps: Record<string, string>;
  [k: string]: any;
}

export interface Co2Info {
  method: string;
  data_date: string;
  grid_g_per_kwh: number;
  grid_is_default: boolean;
  cache_read_factor: number | null;
  water?: any;
  [k: string]: any;
}

export interface Finding {
  rule: string;
  line: string;
  tokens_saved_per_week?: Quantity;
  usd_saved_per_week?: Quantity;
  usd_saved_last_7d?: Quantity;
  [k: string]: any;
}

export interface DailyRow {
  day_ms: number;
  project?: string | null;
  cost_usd?: Quantity;
  fresh_tokens?: number;
  cache_read_tokens?: number;
  cache_write_tokens?: number;
  [k: string]: any;
}

export interface WeekDelta { this: number; last: number; delta_pct: number | null }

export interface PlanRow {
  account: string;
  label: string;
  five_hour_pct: number | null;
  five_hour_resets_ms: number | null;
  seven_day_pct: number | null;
  seven_day_resets_ms: number | null;
  fetched_ms: number;
}

export interface Snapshot {
  schema: number;
  generated_ms: number;
  window: any;
  weeks?: number;
  anonymised: boolean;
  progress?: number;
  providers: ProviderInfo[];
  accounts?: any[];
  co2?: Co2Info | null;
  plan?: PlanRow[];
  live: any;
  findings: Finding[];
  startup: any[];
  skills: any[];
  memory: any[];
  subagents: any[];
  compactions: any[];
  tools: any[];
  unpriced_models: any[];
  history_newer?: number;
  daily?: DailyRow[];
  week?: Record<string, WeekDelta | null>;
  sessions?: any[];
  hourly?: any[];
  recent?: Snapshot;
  [k: string]: any;
}

export type StatusKind = 'scanning' | 'ok' | 'no_root' | 'empty' | 'error';
export interface EngineStatus { kind: StatusKind; roots?: string[]; message?: string }

export interface Notice {
  full: boolean;
  short: string;
  title: string;
  text?: string;
  roots?: string[];
  hint?: string;
  log?: boolean;
  progress?: boolean;
}

export interface ShellState {
  performance?: string;
  running_performance?: string;
  theme?: string;
  mode?: string;
  widget?: boolean;
  corner?: string;
  always_on_top?: boolean;
  autostart?: boolean;
  widget_opacity?: number | null;
  glass?: string;
  version?: string;
  status?: EngineStatus;
  [k: string]: any;
}

export interface WrappedColors { series: string[]; [k: string]: any }
export interface WrappedArgs {
  range: { kind: 'this' | 'last' } | { kind: 'custom'; from: string; to: string };
  provider: string | null;
  names: boolean;
  colors: WrappedColors;
  variant?: string | null;
  [k: string]: any;
}
export interface WrappedCard { id: string; svg: string; text: string }
export interface WrappedResult { label: string; cards: WrappedCard[] }
