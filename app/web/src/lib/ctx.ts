// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

import type { UiStore } from './stores/ui.svelte';
import type { Snapshot } from './types';

// Same list as core render.rs RULES, as titles.
export const RULES: Record<string, string> = {
  R1: 'Startup load per session', R2: 'Memory file size', R3: 'Plugin listed, never used', R4: 'MCP server, few tools called',
  R5: 'Skill body per activation', R6: 'Skill description length', R7: 'Instruction lines repeated', R8: 'Subagent share of cost',
  R9: 'Read-only agents on the largest model', R10: 'Compactions in a session', R11: 'File reads in tool output',
  R12: 'Searches in tool output', R13: 'Very large tool results', R14: 'Listings in tool output', R15: 'Build and test output',
  R16: 'Same file read again', R17: 'Long topic at high context', R18: 'Low cache hit',
};
export const AREA: Record<string, string[]> = {
  startup: ['R1', 'R2', 'R3', 'R4', 'R7'],
  skills: ['R5', 'R6', 'R2', 'R3', 'R7'],
  agents: ['R8', 'R9'],
  context: ['R10', 'R17', 'R18'],
  tools: ['R11', 'R12', 'R13', 'R14', 'R15', 'R16'],
};
export const KIND: Record<string, string> = {
  'system-prompt': 'System prompt', 'builtin-tools': 'Built-in tools', 'mcp-tools': 'MCP tools', 'deferred-tools': 'Deferred tools',
  'skill-listing': 'Skill listing', 'agent-listing': 'Agent listing', 'mcp-instructions': 'MCP instructions',
  'user-instructions': 'Your CLAUDE.md', 'project-instructions': 'Project CLAUDE.md', memory: 'Memory', base: 'Base', unattributed: 'Not traced to a file',
};
export const SERIES = (i: number) => (i < 7 ? `var(--series-${i + 1})` : 'var(--muted)');

export interface Ctx {
  s: Snapshot;
  ui: UiStore;
  inProject: (r: any) => boolean;
  n: number | null;
  open: (k: string) => boolean;
}

export function makeCtx(ui: UiStore, s: Snapshot): Ctx {
  const win = s.window, days = win && win.from_ms > 0 ? Math.round((win.to_ms - win.from_ms) / 864e5) : null;
  return { s, ui, inProject: (r) => !ui.project || r.project == null || r.project === ui.project, n: days, open: (k) => ui.open.has(k) };
}

export function cut<T>(ctx: Ctx, key: string, rows: T[], n: number): T[] { return ctx.open(key) ? rows : rows.slice(0, n); }
