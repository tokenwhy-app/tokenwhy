// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

import { trim } from '../../lib/format';
import { weekOverWeek } from '../../lib/diagnose';
import type { Co2Info } from '../../lib/types';

// CO2e is an order of magnitude, never shown as a measure.
export function co2(g: number | null | undefined): string {
  if (g == null) return '-';
  if (g < 1) return '<1 g';
  const r = Number(g.toPrecision(2));
  return '~' + (r >= 1000 ? trim(r / 1000) + ' kg' : r + ' g');
}
export function co2Range(lo: number | null | undefined, hi: number | null | undefined): string {
  if (lo == null) return '-';
  if (hi == null || hi - lo <= 1e-9 * hi) return co2(lo);
  if (hi < 1) return '<1 g';
  const kg = hi >= 1000;
  const f = (g: number) => String(Number((kg ? g / 1000 : g).toPrecision(2)));
  return '~' + f(lo) + ' to ' + f(hi) + (kg ? ' kg' : ' g');
}
export function waterRange(lo: number | null | undefined, hi: number | null | undefined): string {
  if (lo == null) return '-';
  if (hi == null) hi = lo;
  if (hi < 1) return '<1 L';
  const m3 = hi >= 1000;
  const f = (l: number) => String(Number((m3 ? l / 1000 : l).toPrecision(2)));
  const unit = m3 ? ' m³' : ' L';
  return hi - lo <= 1e-9 * hi ? '~' + f(lo) + unit : '~' + f(lo) + ' to ' + f(hi) + unit;
}
export interface WaterUnit { litres: number; one: string; many: string }
export function waterEquiv(litres: number, units: WaterUnit[] | null | undefined): string | null {
  if (!(litres > 0) || !units || !units.length) return null;
  const u = units.find((x) => litres / x.litres >= 0.25 && litres / x.litres <= 300);
  if (!u) return null;
  const c = litres / u.litres;
  if (c >= 0.95) { const n = c < 10 ? Math.round(c * 10) / 10 : Number(c.toPrecision(2)); return 'about ' + (n === 1 ? 'one ' + u.one : n + ' ' + u.many); }
  const d = Math.max(2, Math.round(1 / c));
  const frac = ({ 2: 'half', 3: 'a third', 4: 'a quarter', 5: 'a fifth' } as Record<number, string>)[d] || '1/' + d;
  return `about ${frac} of ${/^[aeiou]/i.test(u.one) ? 'an' : 'a'} ${u.one}`;
}
export function co2Tip(_info?: Co2Info | null): string {
  return 'Estimate, not a measure';
}

export function co2Week(s: any, ds: any, filtered: any = false) {
  if (!s.co2) return null;
  const lo = weekOverWeek(s, ds, 'co2_g', filtered);
  if (!lo) return null;
  const hi = weekOverWeek(s, ds, 'co2_g_max', filtered) || lo;
  const cache = weekOverWeek(s, ds, 'co2_cache_g', filtered);
  const f = weekOverWeek(s, ds, 'fresh_tokens', filtered), rd = weekOverWeek(s, ds, 'cache_read_tokens', filtered);
  const cur = (lo.cur + hi.cur) / 2, prev = (lo.prev + hi.prev) / 2;
  const tokShare = f && rd && f.cur + rd.cur > 0 ? rd.cur / (f.cur + rd.cur) : null;
  const gShare = cache && cur > 0 ? cache.cur / cur : null;
  const wl = s.co2.water ? weekOverWeek(s, ds, 'water_l', filtered) : null;
  const wh = wl ? weekOverWeek(s, ds, 'water_l_max', filtered) || wl : null;
  return { water: wl && wh && wh.cur > 0 ? { lo: wl.cur, hi: wh.cur } : null, lo: lo.cur, hi: hi.cur, prevLo: lo.prev, prevHi: hi.prev, mid: { cur, prev, delta: prev > 0 ? (cur - prev) / prev : null },
    cacheNote: s.co2.cache_read_factor == null
      ? (tokShare == null ? null : `cache reads ${Math.round(tokShare * 100)} % of tokens, not counted`)
      : (gShare == null ? null : `cache reads ${Math.round(gShare * 100)} % of it`) };
}
