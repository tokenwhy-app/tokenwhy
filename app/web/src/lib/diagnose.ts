// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

import { inDays, spanDays } from './days';
import { q } from './format';
import type { Finding } from './types';

export const CAUSES = [
  { id: 'reads', name: 'Broad file reads', rules: ['R11', 'R12', 'R13', 'R14', 'R15', 'R16'], tab: 'tools',
    why: 'Re-read on every later turn',
    fix: 'Read by symbol or line range; cap long outputs.', start: 'Start by reading files by symbol or line range.' },
  { id: 'topics', name: 'Long topics', rules: ['R17', 'R18'], tab: 'context',
    why: 'Each turn re-reads a huge context',
    fix: 'One topic per session: write a handoff, start fresh.', start: 'Start by keeping one topic per session.' },
  { id: 'agents', name: 'Subagents', rules: ['R8', 'R9'], tab: 'agents',
    why: 'Each subagent reloads full context',
    fix: 'Give subagents file paths, not file contents.', start: 'Start by giving subagents file paths, not file contents.' },
  { id: 'loaded', name: 'Loaded for nothing', rules: ['R1', 'R2', 'R3', 'R4', 'R5', 'R6', 'R7'], tab: 'skills',
    why: 'Loaded every session, used or not',
    fix: 'Trim memory, disable unused plugins and servers.', start: 'Start by trimming what loads in every session.' },
  { id: 'compact', name: 'Compactions', rules: ['R10'], tab: 'context',
    why: 'Summary carried from then on',
    fix: 'End topics with a handoff before the context fills.', start: 'Start by writing a handoff before the context fills.' },
];
export const causeOf = (rule: any) => CAUSES.find((c: any) => c.rules.includes(rule)) || CAUSES[3];
export const causeColor = (c: any) => `var(--series-${CAUSES.indexOf(c) + 1})`;

const MON = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
const day = (md: any): string => { const m = /^(\d\d)-(\d\d)$/.exec(md || ''); return m ? Number(m[2]) + ' ' + MON[Number(m[1]) - 1] : md; };
const k = (t: any) => String(t).replace(/(\d+)\.\d+k$/, '$1k');

const SAY: [string, RegExp, (m: RegExpExecArray) => { fact: string; fix: string | null; more?: string | null }][] = [
  ['R1', /^(.+?): startup (\S+) tokens\.(?: Largest part: (.+) (\S+)\.)?/, (m: any) => ({ fact: `${m[2]} tokens load before the first message in ${m[1]}`, more: m[3] ? `Largest part: ${m[3]}, ${m[4]}` : null, fix: 'Trim what loads at startup' })],
  ['R2', /^(.+?) ([\d.]+) KB \((\S+) tok every session and agent\)/, (m: any) => ({ fact: `${m[3]} tokens of ${m[1]} load in every session and agent`, more: `${m[2]} KB on disk`, fix: 'One short line per memory entry' })],
  ['R3', /^(.+?): (\S+) tok per session here, never used in (\d+) days/, (m: any) => ({ fact: `${m[2]} tokens per session for ${m[1]}, unused in ${m[3]} days`, fix: 'Disable it for this project' })],
  ['R4', /^(.+?): (\d+) of (\d+) tools called in (\d+) sessions/, (m: any) => ({ fact: `${m[2]} of ${m[3]} ${m[1]} tools used, over ${m[4]} sessions`, fix: 'Remove the server or scope it' })],
  ['R5', /^(.+?): (\S+) tok per activation/, (m: any) => ({ fact: `${m[2]} tokens each time ${m[1]} loads`, fix: 'Split the body, load sections on demand' })],
  ['R6', /^(.+?) description (\d+) chars/, (m: any) => ({ fact: `${m[2]}-character description for ${m[1]}, listed every session`, fix: 'Cut it to one sentence' })],
  ['R7', /^(.+?) repeats (\d+) user-level lines \((\S+) tok\)/, (m: any) => ({ fact: `${m[2]} lines of ${m[1]} repeat your CLAUDE.md`, more: `${m[3]} tokens`, fix: 'Delete the repeated lines' })],
  ['R8', /^(.+?): subagents (\d+ %) of cost; (\d+) agents, median startup (\S+)\./, (m: any) => ({ fact: `${m[2]} of ${m[1]} spend went to ${m[3]} subagents`, more: `Each starts at ${k(m[4])} tokens`, fix: 'Give subagents file paths, not file contents' })],
  ['R9', /^(.+?) agents ran on opus (\d+) times \((\$\d+)\)/, (m: any) => ({ fact: `${m[3]} on ${m[2]} Opus runs of ${m[1]}`, fix: 'Set their model to Haiku' })],
  ['R10', /^(.+?) (\d\d-\d\d): (\d+) compactions?, ctx (\S+) peak/, (m: any) => ({ fact: `${m[3]} compaction${m[3] === '1' ? '' : 's'} in one ${m[1]} session`, more: `${day(m[2])}, peak ${k(m[4])}`, fix: 'Write a handoff, then start a new session' })],
  ['R11', /^File reads: (\d+ %) of tool output\.(?: (.+?) (\d+ %)\.)?/, (m: any) => ({ fact: `${m[1]} of tool output is file reads`, more: m[2] ? `${m[2]}: ${m[3]}` : null, fix: 'Read by symbol or line range' })],
  ['R12', /^(.+?): (\d+ %) of tool output, median (\S+) chars per call/, (m: any) => ({ fact: `${m[2]} of tool output is searches`, more: `${m[1]}, ${m[3]} characters per call`, fix: 'Scope the path, cap the matches' })],
  ['R13', /^(\d+) tool results over (\S+) chars(?: \(top: (.+)\))?/, (m: any) => ({ fact: `${m[1]} tool results over ${m[2]} characters`, more: m[3] ? `Mostly ${m[3]}` : null, fix: 'Cap or tail the output' })],
  ['R14', /^(.+?): (\d+ %) of tool output\. Use a scoped glob/, (m: any) => ({ fact: `${m[2]} of tool output is file listings`, more: m[1] !== 'Listings' ? m[1] : null, fix: 'Use a scoped glob' })],
  ['R15', /^(.+?) output (\d+ %)\./, (m: any) => ({ fact: `${m[2]} of tool output is build and test logs`, more: m[1] !== 'Build and test' ? m[1] : null, fix: 'Run builds with quiet flags' })],
  ['R16', /^(.+?) read (\d+) times in session (\S+) \((\S+) tok\)/, (m: any) => ({ fact: `${m[1]} read ${m[2]} times in one session`, more: `${m[4]} tokens, session ${m[3]}`, fix: 'Keep the part you need in the brief' })],
  ['R17', /^(.+?) (\d\d-\d\d): (\d+) turns at (\S+) mean ctx/, (m: any) => ({ fact: `${m[3]} turns at ${k(m[4])} context in one ${m[1]} session`, more: day(m[2]), fix: 'Split the topic' })],
  ['R18', /^Session (\S+): cache hit (\d+ %)/, (m: any) => ({ fact: `${m[2]} cache hit in session ${m[1]}`, fix: 'No edits to instruction files mid-session' })],
];

export function say(f: any): { fact: string; fix: string | null; more?: string | null } {
  const line = f.line || '';
  for (const [rule, re, out] of SAY) {
    if (rule !== f.rule) continue;
    const m = re.exec(line);
    if (m) return out(m);
  }
  const i = line.indexOf('. ');
  return i > 0 ? { fact: line.slice(0, i), fix: line.slice(i + 2).replace(/\.$/, '') } : { fact: line, fix: null };
}

export const saved = (f: any) => q(f.tokens_saved_per_week) || 0;
export const dollars = (f: any, r: any) => { const u = q(f.usd_saved_per_week); return u != null ? u : r ? saved(f) * r.usd : null; };
export const recent = (f: any) => q(f.usd_saved_last_7d);
export const drifts = (avg: any, last: any) => avg != null && last != null && (avg > 0 ? Math.abs(last - avg) / avg > 0.2 : last > 0);

export function rate(s: any) {
  const d = s.daily || [];
  let cost = 0, toks = 0;
  if (d.length) for (const r of d) { cost += q(r.cost_usd) || 0; toks += (r.fresh_tokens || 0) + (r.cache_read_tokens || 0) + (r.cache_write_tokens || 0); }
  else { const t = s.live.today; cost = q(t.cost_usd) || 0; toks = (t.fresh_tokens || 0) + (t.cache_read_tokens || 0); }
  return cost > 0 && toks > 0 ? { usd: cost / toks, from: d.length ? 'window' : 'today' } : null;
}

export function diagnose(findings: any, r: any) {
  const out = CAUSES.map((c: any) => ({ cause: c, findings: [] as Finding[], tokens: 0, usd: null as number | null, last: 0 as number | null }));
  for (const f of findings) {
    const g = out[CAUSES.indexOf(causeOf(f.rule))];
    g.findings.push(f); g.tokens += saved(f);
    const u = dollars(f, r); if (u != null) g.usd = (g.usd || 0) + u;
    const l = recent(f); g.last = g.last == null || l == null ? null : g.last + l;
  }
  const by = (f: any) => { const u = dollars(f, r); return u != null ? u : saved(f) * 1e-9; };
  for (const g of out) g.findings.sort((a: any, b: any) => by(b) - by(a));
  const used = out.filter((g: any) => g.findings.length);
  return used.every((g: any) => g.usd != null) ? used.sort((a: any, b: any) => b.usd - a.usd) : used.sort((a: any, b: any) => b.tokens - a.tokens);
}

export function days(s: any, inProject: any) {
  const m = new Map<number, any>();
  for (const r of (s.daily || []).filter((x: any) => !x.project || inProject(x))) {
    const d: any = m.get(r.day_ms) || { day_ms: r.day_ms, cost: 0, fresh: 0, read: 0, cache: 0, sessions: 0, agents: 0, out: 0, edits: 0, hours: 0, agentCost: 0, co2: 0, co2Max: 0, co2Cache: 0, water: 0, waterMax: 0 };
    d.cost += q(r.cost_usd) || 0; d.fresh += r.fresh_tokens || 0; d.read += r.cache_read_tokens || 0; d.cache += (r.cache_read_tokens || 0) + (r.cache_write_tokens || 0);
    d.sessions += r.sessions || 0; d.agents += r.agents || 0; d.agentCost += r.agent_cost_usd || 0;
    // null = a day stored before the core recorded it; it makes its whole week unknown.
    const add = (k: any, v: any) => { d[k] = d[k] == null || v == null ? null : d[k] + v; };
    add('out', r.output_tokens); add('edits', r.edits); add('hours', r.active_hours); add('co2', r.co2_g); add('co2Max', r.co2_g_max); add('co2Cache', r.co2_cache_g); add('water', r.water_l); add('waterMax', r.water_l_max);
    m.set(r.day_ms, d);
  }
  return [...m.values()].sort((a: any, b: any) => a.day_ms - b.day_ms);
}
const WK: Record<string, string> = { cost_usd: 'cost', fresh_tokens: 'fresh', cache_read_tokens: 'read', sessions: 'sessions', agents: 'agents',
  agent_cost_usd: 'agentCost', output_tokens: 'out', edits: 'edits', active_hours: 'hours', co2_g: 'co2', co2_g_max: 'co2Max', co2_cache_g: 'co2Cache', water_l: 'water', water_l_max: 'waterMax' };
export function weekOverWeek(s: any, ds: any, key: any = 'cost_usd', filtered: any = false) {
  if (!filtered && s.week && key in s.week && s.week[key] == null) return null;
  const w = !filtered && s.week && s.week[key];
  if (w) return { cur: w.this, prev: w.last, delta: w.delta_pct == null ? null : w.delta_pct / 100 };
  if (spanDays(ds) < 14) return null;
  const sum = (a: any) => a.reduce((x: any, d: any) => (x == null || d[WK[key]] == null ? null : x + d[WK[key]]), 0);
  const cur = sum(inDays(ds, 7)), prev = sum(inDays(ds, 14, 7));
  if (cur == null || prev == null) return null;
  return { cur, prev, delta: prev > 0 ? (cur - prev) / prev : null };
}

export const UNITS = [
  { key: 'output_tokens', label: 'Per 1k output tokens', per: 1000, bias: 'Retries count as output' },
  { key: 'edits', label: 'Per edit', per: 1, bias: 'Big and small edits count alike' },
  { key: 'active_hours', label: 'Per active hour', per: 1, bias: 'Idle time inside an hour counts' },
];
export function unitCost(s: any, ds: any, filtered: any = false) {
  const c = weekOverWeek(s, ds, 'cost_usd', filtered);
  if (!c) return null;
  const ac = weekOverWeek(s, ds, 'agent_cost_usd', filtered);
  let swing = false;
  const units = UNITS.map((u: any) => {
    const w = weekOverWeek(s, ds, u.key, filtered);
    if (!w || !(w.cur > 0) || !(w.prev > 0)) return { ...u, w: null };
    if (Math.abs(w.cur - w.prev) / w.prev > 0.25) swing = true;
    const cur = c.cur / (w.cur / u.per), prev = c.prev / (w.prev / u.per);
    return { ...u, w: { cur, prev, delta: prev > 0 ? (cur - prev) / prev : null }, units: w };
  });
  const moves = units.filter((u: any) => u.w && u.w.delta != null && Math.abs(u.w.delta) >= 0.005).map((u: any) => Math.sign(u.w.delta));
  const down = moves.filter((m: any) => m < 0).length, up = moves.filter((m: any) => m > 0).length;
  const verdict = swing ? 'swing' : down >= 2 && down > up ? 'better' : up >= 2 && up > down ? 'worse' : 'mixed';
  const share = (a: any, t: any) => (t > 0 ? a / t : null);
  const mix = ac ? { cur: share(ac.cur, c.cur), prev: share(ac.prev, c.prev) } : null;
  if (mix) (mix as any).changed = mix.cur != null && mix.prev != null && Math.abs(mix.cur - mix.prev) > 0.10;
  return { units, verdict, mix };
}
