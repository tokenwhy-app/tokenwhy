// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

// Steps with setDate, so a DST change never shifts a day by an hour.
export function dayStart(back = 0, now = Date.now()): number {
  const d = new Date(now);
  d.setHours(0, 0, 0, 0);
  d.setDate(d.getDate() - back);
  return d.getTime();
}
export const lastDays = (count: number, now = Date.now()) => Array.from({ length: count }, (_, i) => dayStart(count - 1 - i, now));
export function spanDays(rows: { day_ms: number }[], now = Date.now()): number {
  if (!rows.length) return 0;
  const first = dayStart(0, rows[0].day_ms), d = new Date(dayStart(0, now));
  let k = 1;
  while (d.getTime() > first) { d.setDate(d.getDate() - 1); k++; }
  return k;
}
export const inDays = <T extends { day_ms: number }>(rows: T[], from: number, to = 0, now = Date.now()) => {
  const a = dayStart(from - 1, now), b = to > 0 ? dayStart(to - 1, now) : Infinity;
  return rows.filter((r) => r.day_ms >= a && r.day_ms < b);
};
