// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

import { exact, pct, tok, usd } from '../../lib/format';
import { dollars, drifts, rate, saved } from '../../lib/diagnose';
import { spanDays } from '../../lib/days';
import type { Ctx } from '../../lib/ctx';
import type { UiStore } from '../../lib/stores/ui.svelte';

export const NOUN: Record<string, string> = {
  R1: 'startup', R2: 'memory', R3: 'plugin', R4: 'MCP', R5: 'skill', R6: 'skill text', R7: 'duplicates', R8: 'agents', R9: 'Opus', R10: 'compaction',
  R11: 'file reads', R12: 'search', R13: 'big outputs', R14: 'listings', R15: 'build logs', R16: 're-reads', R17: 'long topic', R18: 'cache',
};
export const FRESH_TIP = 'Billed at full rate, not from cache';
export const CACHE_TIP = 'Reused from cache, cheaper rate';

export const money = (u: number | null | undefined, t: number) => (u != null ? usd(u) : tok(t));
export const moneyTip = (u: number | null | undefined, t: number) => exact(t) + ' tokens a week' + (u != null ? `\nAbout $${u.toFixed(2)}` : '');

export function areaFindings(ctx: Ctx, ids: string[] | null) {
  const r = rate(ctx.s), by = (f: any) => { const u = dollars(f, r); return u != null ? u : saved(f) * 1e-9; };
  return (ctx.s.findings || []).filter((f: any) => (!ids || ids.includes(f.rule)) && ctx.inProject(f)).sort((a: any, b: any) => by(b) - by(a));
}

export const avgOver = (weeks: number | null | undefined) => (weeks && weeks >= 1 ? ` (average over ${Math.round(weeks)} active week${Math.round(weeks) === 1 ? '' : 's'})` : '');
export const moneyView = (ui: UiStore) => (ui.settings && ui.settings.money_view === 'usd' ? 'usd' : 'pct');
export function spendWeek(ds: { cost: number; day_ms: number }[], n: number | null, weeks?: number) {
  const days = n || spanDays(ds);
  const wk = weeks || (days ? days / 7 : 0);
  if (!wk) return null;
  const w = ds.reduce((a, d) => a + d.cost, 0) / wk;
  return w >= 1 ? w : null;
}
export const share100 = (v: number | null | undefined, of: number | null | undefined) => (v != null && of != null && of > 0 ? v / of : null);
export function lastShare(avgShare: number | null, last: number | null, week: number, n: number | null) {
  const ls = share100(last, week);
  return n != null && n > 7 && avgShare != null && ls != null && Math.abs(ls - avgShare) > 0.03 ? `last 7 days: ${pct(ls)} of spend` : null;
}
export const lastWeek = (avg: number | null, last: number | null, n: number | null) => (n != null && n > 7 && drifts(avg, last) ? `last 7 days: ${usd(last)} a week` : null);

export const unitUsd = (v: number) => (v >= 1 ? usd(v) : '$' + v.toFixed(v >= 0.1 ? 2 : 3));
export const UNIT_NOTE: Record<string, string> = { better: 'down from last week', worse: 'up from last week', mixed: 'mixed against last week', swing: 'activity changed a lot, rough comparison' };

// Cells with a change chip need about 250 px, so five or more of them go in rows of three.
export function kpiGrid(el: HTMLElement, chips: boolean) {
  const fit = () => {
    const cells = [...el.children] as HTMLElement[], n = cells.length;
    const wide = chips && n > 4 ? 3 : n;
    el.style.setProperty('--cols-l', String(wide));
    el.style.setProperty('--cols-m', String(wide));
    el.style.setProperty('--cols-s', String(Math.ceil(n / 2)));
    cells.forEach((c) => { c.style.gridColumn = ''; });
    const cols = getComputedStyle(el).gridTemplateColumns.split(' ').filter(Boolean).length, rest = cols ? n % cols : 0;
    if (rest && n) cells[n - 1].style.gridColumn = 'span ' + (cols - rest + 1);
  };
  fit();
  const mo = new MutationObserver(fit), ro = new ResizeObserver(fit);
  mo.observe(el, { childList: true });
  ro.observe(el);
  return { destroy() { mo.disconnect(); ro.disconnect(); } };
}

export type Scale = 'day' | 'week' | 'month';
export const SCREEN: Record<Scale, number> = { day: 30, week: 26, month: 12 };
export function screenEnd(scale: Scale, back: number, count: number, now = Date.now()): number {
  const d = new Date(now);
  if (scale === 'day') d.setDate(d.getDate() - back * count);
  else if (scale === 'week') d.setDate(d.getDate() - 7 * back * count);
  else d.setMonth(d.getMonth() - back * count, 1);
  return d.getTime();
}
export function bucketStart(scale: Scale, ms: number): number {
  const d = new Date(ms);
  d.setHours(0, 0, 0, 0);
  if (scale === 'week') d.setDate(d.getDate() - ((d.getDay() + 6) % 7));
  else if (scale === 'month') d.setDate(1);
  return d.getTime();
}
export function groupDays(ds: any[], scale: Scale, endMs: number, count: number, now = Date.now()) {
  const starts: number[] = [];
  let d = new Date(bucketStart(scale, endMs));
  for (let i = 0; i < count; i++) {
    starts.unshift(d.getTime());
    d = new Date(d);
    if (scale === 'day') d.setDate(d.getDate() - 1); else if (scale === 'week') d.setDate(d.getDate() - 7); else d.setMonth(d.getMonth() - 1);
  }
  const cur = bucketStart(scale, now);
  const m = new Map(starts.map((s) => [s, { start_ms: s, cost: null as number | null, tokens: 0, partial: s === cur }]));
  for (const r of ds) {
    const b = m.get(bucketStart(scale, r.day_ms));
    if (b) { b.cost = (b.cost || 0) + r.cost; b.tokens += r.fresh + r.cache; }
  }
  return [...m.values()];
}
