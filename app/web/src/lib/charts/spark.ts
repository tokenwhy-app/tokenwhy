// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

export interface Pt { v: number; tip: string }
export interface Part { label: string; v: number; color: string }

export function sparkGeom(pts: Pt[], w: number, hh: number, label: string) {
  const out = { id: 'wg' + label.replace(/\W/g, ''), area: '', line: '', hits: [] as { x: number; width: number; tip: string }[], dot: null as { cx: number; cy: number } | null };
  if (!pts.length) return out;
  const max = Math.max(...pts.map((p) => p.v), 1e-9), pad = 4;
  const X = (i: number) => pad + (pts.length < 2 ? (w - 2 * pad) / 2 : (i * (w - 2 * pad)) / (pts.length - 1));
  const Y = (v: number) => hh - pad - ((hh - 2 * pad) * v) / max;
  out.line = pts.map((p, i) => `${X(i).toFixed(1)},${Y(p.v).toFixed(1)}`).join(' ');
  out.area = `${X(0)},${hh - pad} ${out.line} ${X(pts.length - 1)},${hh - pad}`;
  const step = pts.length < 2 ? w : (w - 2 * pad) / (pts.length - 1);
  out.hits = pts.map((p, i) => ({ x: X(i) - step / 2, width: step, tip: p.tip }));
  const l = pts.length - 1;
  out.dot = { cx: X(l), cy: Y(pts[l].v) };
  return out;
}

export function barsGeom(vals: number[], w: number, hh: number, tipOf: (i: number, v: number) => string, now: number) {
  const max = Math.max(...vals, 1e-9), bw = w / vals.length;
  return vals.map((v, i) => {
    const bh = v > 0 ? Math.max(1.5, ((hh - 2) * v) / max) : 0;
    return { x: i * bw + 0.75, y: hh - bh, width: bw - 1.5, height: bh, hx: i * bw, hw: bw, now: i === now, tip: tipOf(i, v) };
  });
}

export const RING = { S: 64, R: 26, C: 2 * Math.PI * 26 };
export function ringDash(frac: number | null | undefined): string {
  const f = Math.max(0, Math.min(1, frac || 0));
  return `${(RING.C * f).toFixed(1)} ${RING.C}`;
}

export function pieGeom(parts: Part[], usd: (n: number) => string, pct: (n: number) => string) {
  const S = 64, R = 28, c = S / 2, total = parts.reduce((a, p) => a + p.v, 0);
  if (!(total > 0)) return { c, R, empty: true, slices: [] as { d: string | null; color: string; tip: string }[] };
  let a = -Math.PI / 2;
  const slices = parts.map((p) => {
    const da = (2 * Math.PI * p.v) / total, b = a + da;
    const full = da >= 2 * Math.PI - 1e-6;
    const d = full ? null : `M${c},${c} L${c + R * Math.cos(a)},${c + R * Math.sin(a)} A${R},${R} 0 ${da > Math.PI ? 1 : 0} 1 ${c + R * Math.cos(b)},${c + R * Math.sin(b)} Z`;
    a = b;
    return { d, color: p.color, tip: `${p.label}\n${usd(p.v)}, ${pct(p.v / total)}` };
  });
  return { c, R, empty: false, slices };
}
