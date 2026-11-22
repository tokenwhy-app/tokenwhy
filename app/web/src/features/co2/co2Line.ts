// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

import type { Snapshot } from '../../lib/types';
import { co2, co2Range, co2Tip, waterRange } from './format';

export function co2Line(s: Snapshot): { label: string; text: string; tip?: string; stats: { v: string; k: string }[] } | null {
  if (!s.co2) return null;
  const rows = s.daily || [];
  const sum = (k: string) => rows.reduce((x: number | null, r: any) => (r[k] == null ? x : (x || 0) + r[k]), null);
  const lo = sum('co2_g'), hi = sum('co2_g_max') ?? lo, wl = s.co2.water ? sum('water_l') : null, wh = sum('water_l_max') ?? wl;
  const w = lo != null && lo > 0 ? { lo, hi: hi!, water: wl != null && wh! > 0 ? { lo: wl, hi: wh! } : null } : null;
  return {
    label: 'CO2e',
    text: 'CO2e: ' + (w ? co2Range(w.lo, w.hi) + (w.water ? ', water ' + waterRange(w.water.lo, w.water.hi) : ' (estimate)') : 'n/a'),
    stats: w
      ? [{ v: co2((w.lo + w.hi) / 2), k: 'CO2e' }, ...(w.water ? [{ v: waterRange((w.water.lo + w.water.hi) / 2, null), k: 'water' }] : [])]
      : [],
    tip: co2Tip(s.co2),
  };
}
