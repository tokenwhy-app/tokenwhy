// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

import type { PlanRow, Snapshot } from '../../lib/types';

const hm = (ms: number) => new Date(ms).toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' });
const when = (ms: number) => new Date(ms).toLocaleString('en-GB', { weekday: 'short', hour: '2-digit', minute: '2-digit' });
const p = (v: number) => Math.round(v) + ' %';

function part(name: string, pct: number | null, resets: number | null, now: number): string | null {
  if (pct == null || (resets != null && resets <= now)) return null;
  return name + ' ' + p(pct);
}

export function planLine(s: Snapshot, account?: string | null): { label: string; text: string; tip: string; rows: PlanRow[]; stat: { v: string; k: string } | null; asOf: string } | null {
  let rows = s.plan || [];
  if (account && rows.some((r) => r.account === account)) rows = rows.filter((r) => r.account === account);
  const now = s.generated_ms || Date.now();
  const shown = rows.map((r) => {
    const parts = [part('5 h', r.five_hour_pct, r.five_hour_resets_ms, now), part('week', r.seven_day_pct, r.seven_day_resets_ms, now)].filter(Boolean);
    return parts.length ? (rows.length > 1 ? r.label + ' ' : '') + parts.join(', ') : null;
  }).filter(Boolean);
  if (!shown.length) return null;
  const asOf = Math.max(...rows.map((r) => r.fetched_ms));
  const resets = rows.flatMap((r) => [
    r.five_hour_resets_ms && r.five_hour_resets_ms > now ? `${rows.length > 1 ? r.label + ': ' : ''}5-hour limit resets ${hm(r.five_hour_resets_ms)}` : null,
    r.seven_day_resets_ms && r.seven_day_resets_ms > now ? `${rows.length > 1 ? r.label + ': ' : ''}weekly limit resets ${when(r.seven_day_resets_ms)}` : null,
  ]).filter(Boolean);
  let top: { pct: number; k: string } | null = null;
  for (const r of rows) for (const [k, v, rs] of [['plan 5 h', r.five_hour_pct, r.five_hour_resets_ms], ['plan week', r.seven_day_pct, r.seven_day_resets_ms]] as const) {
    if (v == null || (rs != null && rs <= now)) continue;
    if (!top || v > top.pct) top = { pct: v, k };
  }
  return {
    label: 'Plan',
    stat: top && { v: p(top.pct), k: top.k },
    asOf: 'as of ' + hm(asOf),
    text: 'Plan: ' + shown.join(' · ') + ' · as of ' + hm(asOf),
    tip: ['From the last /usage reading', ...resets].join('\n'),
    rows,
  };
}
