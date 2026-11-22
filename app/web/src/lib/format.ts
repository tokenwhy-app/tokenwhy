// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

import type { Quantity, EngineStatus, Snapshot, Notice } from './types';

export function tok(n: number | null | undefined): string {
  if (n == null) return '-';
  const a = Math.abs(n);
  if (a >= 1e9) return trim(n / 1e9) + 'B';
  if (a >= 1e6) return trim(n / 1e6) + 'M';
  if (a >= 1e4) return Math.round(n / 1e3) + 'k';
  if (a >= 1e3) return trim(n / 1e3) + 'k';
  return String(Math.round(n));
}
export function trim(x: number): string { return x >= 100 ? String(Math.round(x)) : x.toFixed(1).replace(/\.0$/, ''); }
export function usd(n: number | null | undefined): string {
  if (n == null) return '-';
  if (n < 10) return '$' + n.toFixed(2);
  if (n < 100) return '$' + n.toFixed(1).replace(/\.0$/, '');
  return '$' + Math.round(n).toLocaleString('en-US');
}
export function pct(x: number | null | undefined): string {
  if (x == null) return '-';
  const p = x * 100;
  return (p > 0 && p < 1 ? '<1' : String(Math.round(p))) + ' %';
}
export function int(n: number | null | undefined): string { return n == null ? '-' : String(Math.round(n)); }
export function exact(n: number | null | undefined): string { return n == null ? '-' : Math.round(n).toLocaleString('en-US'); }
export function bytes(n: number): string { return n >= 1000 ? (n / 1000).toFixed(1) + ' KB' : n + ' B'; }
export function q(x: Quantity | null | undefined): number | null { return x == null || x.value == null ? null : x.value; }
export function qfmt(x: Quantity | null | undefined, f: (v: number) => string): string {
  const v = q(x);
  if (v == null) return '-';
  return (x!.est ? '~' : '') + f(v);
}
export function model(m: string | null | undefined): string {
  if (!m) return '-';
  const r = /^claude-([a-z]+)-(\d+)(?:-(\d+))?/.exec(m);
  return r ? r[1][0].toUpperCase() + r[1].slice(1) + ' ' + r[2] + (r[3] ? '.' + r[3] : '') : m;
}
export function path(p: string | null | undefined, max = 56): string {
  if (!p) return '';
  const s = String(p).replace(/\\/g, '/')
    .replace(/^[A-Za-z]:\/Users\/[^/]+/, '~')
    .replace(/^\/[a-zA-Z]\/Users\/[^/]+/, '~')
    .replace(/^\/(?:home|Users)\/[^/]+/, '~');
  if (s.length <= max) return s;
  const parts = s.split('/');
  let out = parts.pop()!;
  while (parts.length && out.length + parts[parts.length - 1].length + 3 <= max) out = parts.pop() + '/' + out;
  return '…/' + out;
}

export const withData = (s: Snapshot | null | undefined) => ((s && s.providers) || []).filter((p) => p.files > 0 || p.imported);

export function notice(s: Snapshot | null | undefined, st: EngineStatus | null | undefined): Notice | null {
  const k = st && st.kind;
  if (k === 'no_root') return { full: true, short: 'no agent logs found', title: 'No agent logs found',
    text: 'tokenwhy reads the session logs that coding agents write, and none of these folders exists:', roots: st!.roots,
    hint: 'Run a session, then rescan.' };
  if (k === 'empty') return { full: true, short: 'no sessions yet', title: 'No sessions yet',
    text: 'These agent folders hold no session log yet:', roots: st!.roots,
    hint: 'Run a session; it appears here.' };
  if (k === 'error') return { full: !s, short: 'scan failed', title: 'The last scan failed.', text: st!.message, log: true };
  if (!s) return { full: true, short: 'reading logs', title: 'Reading session logs', progress: true };
  return null;
}
