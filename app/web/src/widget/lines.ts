// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

import { tok, usd, int, q, pct, model } from '../lib/format';
import { diagnose, rate, say, unitCost, UNITS } from '../lib/diagnose';
import type { Snapshot } from '../lib/types';
import { lastDays } from '../lib/days';

export interface Cell { id?: string; text: string; v?: number | null; act?: string | null; tip?: string }
export interface Cells { today?: Cell[] | null; session?: Cell[] | null }

// Snapshots do not carry the context window; a context past 200k can only be the 1M window.
const ctxWindow = (n: number) => (n > 200000 ? 1000000 : 200000);

export const recentOf = (s: any): any => s.recent || s;
export function periodLabel(s: any): string | null {
  const w = s.recent && s.recent.window;
  if (!w) return null;
  const h = Math.round((w.to_ms - w.from_ms) / 3600000);
  return h > 72 ? `last ${Math.round(h / 24)} days` : `last ${h} h`;
}

export function cells(s: any): Cells {
  const t = s.live && s.live.today;
  const r = recentOf(s);
  const a = r.live && r.live.sessions && r.live.sessions[0];
  const win = a ? ctxWindow(a.ctx_tokens) : 0, f = a ? a.ctx_tokens / win : 0;
  return {
    today: t && [{ id: 'tcost', text: usd(q(t.cost_usd)), v: q(t.cost_usd), tip: `${tok(t.fresh_tokens)} fresh, ${int(t.sessions)} sessions` }],
    session: a && [{
      id: 'ctx', text: pct(f), v: a.ctx_tokens, act: f >= 0.85 ? 'Handoff, new session' : null,
      tip: `Context ${tok(a.ctx_tokens)} of ${tok(win)}, ${usd(q(a.cost_usd))}, ${int(a.agents_running)} agents`,
    }],
  };
}

export function startupLine(s: any): Cell[] | undefined {
  const p = (recentOf(s).startup || []).slice().sort((x: any, y: any) => y.median - x.median)[0];
  return p && [{ id: 'st', text: tok(p.median), v: p.median, tip: p.project + ', median' }];
}

const ACT: Record<string, string> = {
  reads: 'Read by line range', topics: 'One topic per session', agents: 'Pass paths, not contents',
  loaded: 'Trim startup load', compact: 'Handoff before it fills',
};

const TAG: Record<string, string> = { 'claude-code': 'CC', codex: 'Cx', agy: 'AG', opencode: 'OC' };
export function tagOf(s: any, a: any): { tag: string; label: string } | null {
  const ps = (s.providers || []).filter((p: any) => p.files > 0 || p.imported);
  if (ps.length < 2 || !a || !a.provider) return null;
  const p = ps.find((x: any) => x.id === a.provider);
  return { tag: TAG[a.provider] || a.provider.replace(/^import:/, '').slice(0, 2), label: p ? p.label : a.provider };
}

export function topFinding(s: any): Cell | null {
  const r = recentOf(s);
  const g: any = diagnose(r.findings || [], rate(r))[0];
  if (!g) return null;
  const fix = say(g.findings[0]).fix ?? g.cause.fix;
  return { text: (g.usd != null ? usd(g.usd) : tok(g.tokens)) + '/wk', act: ACT[g.cause.id] ?? null, tip: g.cause.name + ': ' + fix };
}

let prev: Cells | null = null;
export function hotId(cur: Cells): string | null {
  let best: string | null = null, bestD = 0;
  if (prev) for (const k of ['today', 'session'] as const) {
    (cur[k] || []).forEach((c, i) => {
      const o = prev![k] && prev![k]![i];
      if (!o || o.v == null || c.v == null || o.id !== c.id) return;
      const d = Math.abs(c.v - o.v) / Math.max(Math.abs(o.v), 1);
      if (d > bestD) { bestD = d; best = c.id ?? null; }
    });
  }
  prev = cur;
  return best;
}

export type UnitLine = { na: true } | { na: false; text: string; tip: string; cls: string };
export function unitLine(s: any): UnitLine {
  if (s.recent) {
    let cost = 0, out = 0;
    for (const r of s.recent.daily || []) { cost += q(r.cost_usd) || 0; out += r.output_tokens || 0; }
    if (!(out > 0) || !(cost > 0)) return { na: true };
    return { na: false, text: `$${(cost / (out / 1000)).toFixed(3)}`, cls: '', tip: `All cost / 1k output tokens\n${UNITS[0].bias}` };
  }
  const uc: any = unitCost(s, []);
  const u = uc && uc.units[0];
  if (!u || !u.w) return { na: true };
  const d = u.w.delta;
  const cls = uc.verdict === 'better' ? ' good' : uc.verdict === 'worse' ? ' accent' : '';
  return {
    na: false, text: `$${u.w.cur.toFixed(3)}`, cls,
    tip: `All cost / 1k output tokens\n${u.bias}` + (d != null ? `\n${d >= 0 ? '+' : '−'}${pct(Math.abs(d))} vs last week` : ''),
  };
}

const dayName = (ms: number) => new Date(ms).toLocaleDateString('en-GB', { weekday: 'short', day: 'numeric', month: 'short' });
const level = (f: number, warn: number, high: number) => (f >= high ? 'high' : f >= warn ? 'warn' : 'ok');

function todayHours(s: any) {
  const rows = s.hourly || [];
  const last = rows.reduce((a: number, r: any) => Math.max(a, r.day_ms), 0);
  const v: number[] = new Array(24).fill(0);
  for (const r of rows) if (r.day_ms === last) v[r.hour] += r.cost_usd || 0;
  return { day: last, v };
}

export function extendedData(s: Snapshot) {
  const sn: any = s, rc: any = recentOf(s);
  const t = sn.live && sn.live.today, a = rc.live && rc.live.sessions && rc.live.sessions[0];
  const now = new Date(), today0 = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  const byDay = new Map<number, number>();
  for (const r of sn.daily || []) byDay.set(r.day_ms, (byDay.get(r.day_ms) || 0) + (q(r.cost_usd) || 0));
  const hrs = todayHours(sn), hNow = now.getHours();
  const todayCost: number = t ? q(t.cost_usd) || 0 : hrs.v.reduce((x, y) => x + y, 0);
  const week: { v: number; tip: string }[] = [];
  for (const d of lastDays(7, now.getTime())) {
    const v = d === today0 ? todayCost : byDay.get(d) || 0;
    week.push({ v, tip: `${dayName(d)}\n${usd(v)}` });
  }
  const weekSum = week.reduce((x, p) => x + p.v, 0);

  const frac = now.getMinutes() / 60;
  const perHour = hrs.day >= today0 ? hrs.v[hNow] + (hNow > 0 ? hrs.v[hNow - 1] * (1 - frac) : 0) : 0;
  const projected = todayCost + perHour * (24 - hNow - frac);

  const ctxN: number | null = a ? a.ctx_tokens : null, win = ctxWindow(ctxN || 0), cf = ctxN != null ? ctxN / win : 0;
  const read: number = t ? t.cache_read_tokens || 0 : 0;
  const hit: number | null = t && read + t.fresh_tokens > 0 ? read / (read + t.fresh_tokens) : null;
  const models = new Map<string, number>();
  for (const r of (sn.recent && sn.recent.daily) || []) {
    const k = r.model || 'other';
    models.set(k, (models.get(k) || 0) + (r.fresh_tokens || 0) + (r.cache_read_tokens || 0) + (r.cache_write_tokens || 0));
  }
  const ranked = [...models.entries()].filter((e) => e[1] > 0).sort((x, y) => y[1] - x[1]);
  const shortId = (m: string) => { const l = model(m); return l === m ? m.replace(/^claude-/, '') : l; };
  const top3 = ranked.slice(0, 3).map(([m, v]) => ({ label: shortId(m), v }));
  const rest = ranked.slice(3).reduce((x, e) => x + e[1], 0);
  const parts = (rest > 0 ? [...top3, { label: 'other', v: rest }] : top3)
    .map((p, i) => ({ ...p, color: `var(--series-${i + 1})` }));
  const g: any = diagnose(rc.findings || [], rate(rc))[0];

  const st: any[] = rc.startup || [];
  const stRow = (a && st.find((x) => x.project === a.project)) || null;
  const stMed: number | null = stRow ? stRow.median : st.length ? st.map((x) => x.median).sort((x, y) => x - y)[Math.floor(st.length / 2)] : null;
  const kinds = new Map<string, number>();
  for (const p of (stRow && stRow.parts) || []) kinds.set(p.kind, (kinds.get(p.kind) || 0) + (q(p.tokens) || 0));
  const kindName = (k: string) => { const n = k.replace(/-/g, ' '); return n[0].toUpperCase() + n.slice(1); };
  const top2 = [...kinds.entries()].sort((x, y) => y[1] - x[1]).slice(0, 2).map(([k, v]) => `${kindName(k)} ${tok(v)}`);
  const sf = stMed != null ? stMed / ctxWindow(stMed) : 0;

  return {
    week, weekSum, todayCost, perHour, projected,
    hoursLive: hrs.day >= today0,
    hours: (hrs.day >= today0 ? hrs.v : new Array(24).fill(0)) as number[], hNow,
    ctx: { frac: cf, centre: ctxN != null ? pct(cf).replace(' ', '') : 'idle', tip: ctxN != null ? `Active session context\n${tok(ctxN)} of ${tok(win)}` : 'No active session', lvl: level(cf, 0.6, 0.85) },
    startup: { frac: sf, centre: stMed != null ? tok(stMed).replace(' ', '') : '-', tip: stMed != null ? ['Loaded before the first message' + (stRow ? '' : ' (median)'), ...(top2.length ? [top2.join(', ')] : [])].join('\n') : 'No startup data', lvl: level(sf, 0.15, 0.3) },
    cache: { frac: hit, centre: hit != null ? pct(hit).replace(' ', '') : '-', tip: hit != null ? `Cache hit today\n${tok(read)} read from cache, ${tok(t.fresh_tokens)} fresh` : 'No tokens today', lvl: hit == null ? 'ok' : level(1 - hit, 0.3, 0.6) },
    parts,
    waste: g ? { cause: g.cause, act: ACT[g.cause.id] ?? null, fix: (say(g.findings[0]).fix ?? g.cause.fix) as string, usd: g.usd as number | null, tokens: g.tokens as number } : null,
  };
}
