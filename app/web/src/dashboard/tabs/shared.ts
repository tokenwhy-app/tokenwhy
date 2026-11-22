// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

import type { Ctx } from '../../lib/ctx';
import { AREA } from '../../lib/ctx';
import { withData } from '../../lib/format';
import { dollars, rate, saved } from '../../lib/diagnose';

export const CAP: Record<string, [string, string]> = {
  startup: ['Startup breakdown', 'Not recorded in these transcripts'],
  tool_outputs: ['Tool output', 'Not recorded in these transcripts'],
  subagents: ['Subagents', 'Not recorded in these transcripts'],
  compactions: ['Compactions', 'Not recorded in these transcripts'],
  skills: ['Skills', 'Not recorded in these transcripts'],
  memory: ['Memory files', 'Not recorded in these transcripts'],
};

export const shownProviders = (ctx: Ctx) => withData(ctx.s).filter((p) => !ctx.ui.provider || p.id === ctx.ui.provider);

export function naMissing(ctx: Ctx, cap: string): boolean {
  const ps = shownProviders(ctx);
  return !!ps.length && !ps.some((p) => p.caps[cap] !== 'none');
}
export const naText = (ctx: Ctx, cap: string) => `${CAP[cap][0]}: not available for ${shownProviders(ctx).map((p) => p.label).join(', ')} transcripts`;

export function provOf(ctx: Ctx, r: any): string {
  if (ctx.ui.provider || withData(ctx.s).length < 2 || !r.provider) return '';
  const p = withData(ctx.s).find((x) => x.id === r.provider);
  return ' · ' + (p ? p.label : r.provider);
}

export function areaFindings(ctx: Ctx, area: string) {
  const ids = AREA[area], r = rate(ctx.s);
  const by = (f: any) => { const u = dollars(f, r); return u != null ? u : saved(f) * 1e-9; };
  return (ctx.s.findings || []).filter((f) => (!ids || ids.includes(f.rule)) && ctx.inProject(f)).sort((a, b) => by(b) - by(a));
}

export const when = (ms: number) => {
  const d = new Date(ms);
  return d.toLocaleDateString('en-GB', { day: 'numeric', month: 'short' }) + ' ' + d.toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' });
};
