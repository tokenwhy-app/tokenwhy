// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

import type { Ctx } from './ctx';
import type { Finding } from './types';
import { dollars, rate, saved, days as dayRows } from './diagnose';
import { spanDays } from './days';
import { spendWeek } from '../dashboard/tabs/overview';

export function minorTest(ctx: Ctx): (f: Finding) => boolean {
  const r = rate(ctx.s);
  const ds = dayRows(ctx.s, ctx.inProject) as any[];
  const sw = spendWeek(ds, ctx.n, ctx.s.weeks);
  const d = ctx.n || spanDays(ds);
  const wk = ctx.s.weeks || (d ? d / 7 : 0);
  const wt = wk ? ds.reduce((a, x) => a + x.fresh + x.cache, 0) / wk : 0;
  return (f) => {
    const u = dollars(f, r);
    if (u != null && sw != null) return u < 0.5 && u < 0.005 * sw;
    return wt > 0 && saved(f) < 0.005 * wt;
  };
}
