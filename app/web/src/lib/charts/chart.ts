// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

// No chart library: the app is offline and the CSP is default-src 'self'.
const NS = 'http://www.w3.org/2000/svg';
function s(tag: any, attrs: any, ...kids: any[]) {
  const e = document.createElementNS(NS, tag);
  // Styles go through CSSOM: the CSP forbids style attributes.
  for (const [k, v] of Object.entries(attrs || {})) if (v != null) { if (k === 'style') e.style.cssText = v; else e.setAttribute(k, v); }
  for (const c of kids.flat()) if (c != null) e.append(c.nodeType ? c : document.createTextNode(String(c)));
  return e;
}
function frame(w: any, hgt: any, label: any, desc: any) {
  return s('svg', { class: 'chart', viewBox: `0 0 ${w} ${hgt}`, width: w, height: hgt, role: 'group', 'aria-label': label },
    s('title', {}, label), s('desc', {}, desc));
}
const cut = (t: any, n: any) => (n < 2 ? '' : t.length > n ? t.slice(0, Math.max(1, n - 1)) + '…' : t);
function mark(tip: any, cls: any, ...kids: any[]) {
  return s('g', { class: 'c-mark ' + (cls || ''), tabindex: '0', role: 'img', 'aria-label': tip.replace(/\n/g, ', '), 'data-tip': tip }, ...kids);
}
function hit(x: any, y: any, w: any, h: any) {
  const W = Math.max(6, w), H = Math.max(6, h);
  return s('rect', { x: x - (W - w) / 2, y: y - (H - h) / 2, width: W, height: H, class: 'c-hit' });
}
function ticks(max: number) {
  const m = max / 10 ** Math.floor(Math.log10(max));
  return m === 2.5 || m === 5 ? 5 : 4;
}
function niceMax(v: any) {
  if (v <= 0) return 1;
  const p = 10 ** Math.floor(Math.log10(v)), m = v / p;
  return (m <= 1 ? 1 : m <= 2 ? 2 : m <= 2.5 ? 2.5 : m <= 5 ? 5 : 10) * p;
}

export function donut(parts: any, label: any, size: any, centre: any, sub: any, tipFmt: (v: number) => string = (v) => Math.round(v).toLocaleString('en-US') + ' tokens') {
  const total = parts.reduce((a: any, p: any) => a + p.value, 0) || 1;
  const R = size / 2, r = R - 14, C = 2 * Math.PI * r, gap = parts.length > 1 ? 2 : 0;
  const svg = frame(size, size, label, parts.map((p: any) => p.label + ' ' + p.value).join('; '));
  svg.append(s('circle', { cx: R, cy: R, r, class: 'c-ring' }));
  let at = 0;
  for (const p of parts) {
    const len = (C * p.value) / total;
    svg.append(s('circle', { cx: R, cy: R, r, class: 'c-arc', 'data-tip': p.label + '\n' + tipFmt(p.value),
      style: `stroke:${p.color}`, 'stroke-dasharray': `${Math.max(len - gap, 0.5)} ${C}`, 'stroke-dashoffset': -at,
      transform: `rotate(-90 ${R} ${R})` }));
    at += len;
  }
  svg.append(s('text', { x: R, y: R + 2, class: 'c-centre', 'text-anchor': 'middle' }, centre),
    s('text', { x: R, y: R + 20, class: 'c-sub', 'text-anchor': 'middle' }, sub));
  return svg;
}

export function pareto(items: any, W: any, label: any, fmt: any) {
  const H = 248, L = 52, R = 44, T = 14, B = 34;
  const iw = W - L - R, ih = H - T - B;
  const total = items.reduce((a: any, i: any) => a + i.value, 0) || 1;
  const max = niceMax(items.length ? items[0].value : 1);
  const bw = iw / Math.max(items.length, 1), gap = Math.min(10, bw * 0.28);
  const y = (v: any) => T + ih - (ih * v) / max, yc = (f: any) => T + ih - ih * f;
  let cum = 0, reach = -1;
  const pts = items.map((it: any, i: any) => { cum += it.value; if (reach < 0 && cum / total >= 0.8) reach = i; return [L + bw * i + bw / 2, yc(cum / total), cum / total]; });
  const svg = frame(W, H, label, `${items.length} items; the first ${reach + 1} make 80 % of the total.`);
  for (let g = 0; g <= 4; g++) {
    const v = (max * g) / 4, yy = y(v);
    svg.append(s('line', { x1: L, x2: W - R, y1: yy, y2: yy, class: g ? 'c-grid' : 'c-axis' }),
      s('text', { x: L - 8, y: yy + 4, class: 'c-tick', 'text-anchor': 'end' }, fmt(v)),
      s('text', { x: W - R + 8, y: yc(g / 4) + 4, class: 'c-tick' }, g * 25 + ' %'));
  }
  const y80 = yc(0.8);
  svg.append(s('line', { x1: L, x2: W - R, y1: y80, y2: y80, class: 'c-mark80' }),
    s('text', { x: W - R - 4, y: y80 + 14, class: 'c-note', 'text-anchor': 'end' }, '80 % line'));
  items.forEach((it: any, i: any) => {
    const w = Math.min(bw - gap, 64), x = L + bw * i + (bw - w) / 2, top = y(it.value);
    const lab = bw >= 30 ? cut(it.short || it.label, Math.floor(bw / 6.6)) : String(i + 1);
    svg.append(mark(`${it.label}\n${it.tip}\nRunning total ${Math.round(pts[i][2] * 100)} %`, i > reach ? 'tail' : '',
      hit(x, top, w, Math.max(1.5, T + ih - top)),
      s('rect', { x, y: top, width: w, height: Math.max(1.5, T + ih - top), rx: Math.min(4, w / 3), class: 'c-fill', style: `--c:${it.color};fill:${it.color}` }),
      s('rect', { x: x - 2, y: top - 2, width: w + 4, height: Math.max(1.5, T + ih - top) + 4, rx: 5, class: 'c-focus' }),
      s('text', { x: x + w / 2, y: H - B + 18, class: 'c-tick', 'text-anchor': 'middle' }, lab)));
  });
  svg.append(s('polyline', { points: pts.map((p: any) => p[0] + ',' + p[1]).join(' '), class: 'c-cum' }),
    ...pts.map((p: any) => s('circle', { cx: p[0], cy: p[1], r: 3, class: 'c-cumdot' })));
  return svg;
}

function squarify(items: any, x: any, y: any, w: any, h: any) {
  const total = items.reduce((a: any, i: any) => a + i.value, 0);
  const out: any[] = [];
  if (total <= 0 || w <= 0 || h <= 0) return out;
  const scale = (w * h) / total;
  let rest = items.map((i: any) => ({ item: i, a: i.value * scale })).filter((r: any) => r.a > 0);
  const worst = (row: any, side: any) => {
    const sum = row.reduce((a: any, r: any) => a + r.a, 0), mx = Math.max(...row.map((r: any) => r.a)), mn = Math.min(...row.map((r: any) => r.a));
    return Math.max((side * side * mx) / (sum * sum), (sum * sum) / (side * side * mn));
  };
  while (rest.length) {
    const side = Math.min(w, h);
    let row: any[] = [rest[0]], i = 1;
    while (i < rest.length && worst(row.concat(rest[i]), side) <= worst(row, side)) { row.push(rest[i]); i++; }
    rest = rest.slice(i);
    const sum = row.reduce((a: any, r: any) => a + r.a, 0);
    if (w >= h) {
      const cw = sum / h; let yy = y;
      for (const r of row) { const ch = r.a / cw; out.push({ item: r.item, x, y: yy, w: cw, h: ch }); yy += ch; }
      x += cw; w -= cw;
    } else {
      const ch = sum / w; let xx = x;
      for (const r of row) { const cw = r.a / ch; out.push({ item: r.item, x: xx, y, w: cw, h: ch }); xx += cw; }
      y += ch; h -= ch;
    }
  }
  return out;
}
export function treemap(groups: any, W: any, H: any, label: any) {
  const svg = frame(W, H, label, groups.map((g: any) => g.name + ' ' + g.tip.split('\n')[0]).join('; '));
  const HEAD = 22, P = 2;
  for (const b of squarify(groups, 0, 0, W, H)) {
    const g = b.item, gx = b.x + P, gy = b.y + P, gw = b.w - 2 * P, gh = b.h - 2 * P;
    if (gw < 4 || gh < 4) continue;
    const head = gh > HEAD + 18 && gw > 40;
    const blk = s('g', { class: 'c-block' },
      s('rect', { x: gx, y: gy, width: gw, height: gh, rx: 6, class: 'c-blockbg', style: `--c:${g.color};fill:${g.color}` }));
    if (head) blk.append(mark(g.name + '\n' + g.tip, 'c-head',
      s('rect', { x: gx, y: gy, width: gw, height: HEAD, class: 'c-hit' }),
      s('rect', { x: gx + 1, y: gy + 1, width: gw - 2, height: HEAD - 2, rx: 5, class: 'c-focus' }),
      s('text', { x: gx + 8, y: gy + 15, class: 'c-tmhead' }, cut(g.name + '  ' + g.value_text, Math.floor((gw - 12) / 6.8)))));
    const kids = squarify(g.children, gx, gy + (head ? HEAD : 0), gw, gh - (head ? HEAD : 0));
    kids.forEach((k: any, i: any) => {
      const c = k.item, kx = k.x + 1, ky = k.y + 1, kw = k.w - 2, kh = k.h - 2;
      if (kw < 2 || kh < 2) return;
      const op = Math.max(0.16, 0.62 - i * 0.09);
      const lines = kh > 34 && kw > 46 ? [cut(c.name, Math.floor((kw - 10) / 6.6)), c.value_text] : kh > 18 && kw > 36 ? [cut(c.name, Math.floor((kw - 10) / 6.6))] : [];
      blk.append(mark(`${g.name} · ${c.name}\n${c.tip}`, '',
        s('rect', { x: kx, y: ky, width: kw, height: kh, rx: 4, class: 'c-tile', style: `--c:${g.color};--op:${op};fill:${g.color};fill-opacity:${op}` }),
        s('rect', { x: kx, y: ky, width: kw, height: kh, rx: 4, class: 'c-focus' }),
        ...lines.map((t: any, j: any) => s('text', { x: kx + 7, y: ky + 16 + j * 15, class: j ? 'c-tmval' : 'c-tmlbl' }, t))));
    });
    svg.append(blk);
  }
  return svg;
}

export const costColor = (f: any) => `color-mix(in srgb, var(--series-1) ${Math.round(22 + 78 * Math.sqrt(Math.max(0, Math.min(1, f))))}%, var(--card))`;
export const costRadius = (f: any) => 4 + 18 * Math.sqrt(Math.max(0, f));
export function scatter(pts: any, W: any, H: any, label: any, zone: any, fmtY: any) {
  const L = 52, R = 16, T = 14, B = 34, iw = W - L - R, ih = H - T - B;
  const xs = pts.map((p: any) => p.x), ys = pts.map((p: any) => p.y);
  const x0 = Math.max(1, Math.min(...xs, zone.x) / 1.3), x1 = Math.max(...xs, zone.x) * 1.3;
  const ymax = niceMax(Math.max(...ys, zone.y) * 1.08);
  const X = (v: any) => L + (iw * Math.log(v / x0)) / Math.log(x1 / x0), Y = (v: any) => T + ih - (ih * v) / ymax;
  const smax = Math.max(...pts.map((p: any) => p.size), 1e-9), rad = (v: any) => costRadius(v / smax);
  const hot = pts.filter((p: any) => p.x >= zone.x && p.y >= zone.y);
  const svg = frame(W, H, label, `${pts.length} sessions; ${hot.length} in the zone above ${zone.x} turns and ${fmtY(zone.y)} mean context.`);
  svg.append(s('rect', { x: X(zone.x), y: T, width: W - R - X(zone.x), height: Y(zone.y) - T, class: 'c-zone' }),
    s('text', { x: W - R - 8, y: T + 16, class: 'c-note', 'text-anchor': 'end' }, 'long-topic zone'));
  for (let g = 0; g <= 4; g++) {
    const v = (ymax * g) / 4;
    svg.append(s('line', { x1: L, x2: W - R, y1: Y(v), y2: Y(v), class: g ? 'c-grid' : 'c-axis' }),
      s('text', { x: L - 8, y: Y(v) + 4, class: 'c-tick', 'text-anchor': 'end' }, fmtY(v)));
  }
  for (const t of [1, 3, 10, 30, 100, 300, 1000, 3000, 10000]) {
    if (t < x0 || t > x1) continue;
    svg.append(s('line', { x1: X(t), x2: X(t), y1: T, y2: T + ih, class: 'c-grid' }),
      s('text', { x: X(t), y: H - B + 18, class: 'c-tick', 'text-anchor': 'middle' }, String(t)));
  }
  svg.append(s('text', { x: W - R, y: H - 4, class: 'c-tick', 'text-anchor': 'end' }, 'turns →'),
    s('line', { x1: X(zone.x), x2: X(zone.x), y1: T, y2: T + ih, class: 'c-zoneline' }),
    s('line', { x1: L, x2: W - R, y1: Y(zone.y), y2: Y(zone.y), class: 'c-zoneline' }));
  // Large bubbles first, so small ones stay on top and reachable.
  const order = pts.slice().sort((a: any, b: any) => b.size - a.size);
  for (const p of order) {
    const cx = X(p.x), cy = Y(p.y), r = rad(p.size), c = costColor(p.size / smax);
    svg.append(mark(p.tip, '',
      s('circle', { cx, cy, r, class: 'c-dot', style: `fill:${c};stroke:${c}` }),
      s('circle', { cx, cy, r: r + 3, class: 'c-focus' })));
  }
  return svg;
}

export function columns(items: any, W: any, H: any, label: any, fmt: any, line: any, fix?: { max: number; lmax: number }) {
  const L = 48, R = line ? 48 : 8, T = 10, B = 26, iw = W - L - R, ih = H - T - B;
  const max = niceMax(Math.max(...items.map((d: any) => d.value), fix?.max ?? 0, 0));
  const lmax = line ? niceMax(Math.max(...line.values, fix?.lmax ?? 0, 0)) : 1;
  const nt = ticks(max);
  const bw = iw / Math.max(items.length, 1), gap = Math.min(10, bw * 0.3);
  const Y = (v: any) => T + ih - (ih * v) / max, YL = (v: any) => T + ih - (ih * v) / lmax;
  const svg = frame(W, H, label, items.map((d: any) => d.tip.replace(/\n/g, ' ')).join('; '));
  for (let g = 0; g <= nt; g++) {
    const v = (max * g) / nt;
    svg.append(s('line', { x1: L, x2: W - R, y1: Y(v), y2: Y(v), class: g ? 'c-grid' : 'c-axis' }),
      s('text', { x: L - 8, y: Y(v) + 4, class: 'c-tick', 'text-anchor': 'end' }, fmt(v)),
      line ? s('text', { x: W - R + 8, y: Y(v) + 4, class: 'c-tick c-tick2' }, line.fmt((lmax * g) / nt)) : null);
  }
  const step = Math.ceil(34 / bw);
  items.forEach((d: any, i: any) => {
    const x = L + bw * i + gap / 2, w = bw - gap, top = Y(d.value);
    svg.append(mark(d.tip, d.dim ? 'tail' : '',
      hit(x, top, w, Math.max(1.5, T + ih - top)),
      s('rect', { x, y: top, width: w, height: Math.max(1.5, T + ih - top), rx: Math.min(4, w / 3), class: 'c-fill', style: '--c:var(--series-1);fill:var(--series-1)' }),
      s('rect', { x: x - 2, y: top - 2, width: w + 4, height: Math.max(1.5, T + ih - top) + 4, rx: 5, class: 'c-focus' }),
      (items.length - 1 - i) % step === 0 ? s('text', { x: x + w / 2, y: H - 8, class: 'c-tick', 'text-anchor': 'middle' }, d.label) : null));
  });
  if (line) {
    const pts = line.values.map((v: any, i: any) => [L + bw * i + bw / 2, YL(v)]);
    svg.append(s('polyline', { points: pts.map((p: any) => p.join(',')).join(' '), class: 'c-line' }),
      ...pts.map((p: any) => s('circle', { cx: p[0], cy: p[1], r: 2.5, class: 'c-linedot' })));
  }
  return svg;
}

export function heatmap(cells: any, W: any, label: any, fmt: any, rowTip: any) {
  const L = 36, T = 4, B = 20, cw = (W - L) / 24, ch = Math.min(26, Math.max(16, cw * 1.4)), H = T + ch * 7 + B;
  const max = Math.max(...cells.flat(), 1e-9);
  const DN = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
  const svg = frame(W, H, label, DN.map((d: any, i: any) => rowTip(i).replace(/\n/g, ' ')).join('; '));
  cells.forEach((row: any, i: any) => {
    const g = mark(rowTip(i), 'c-hrow', s('rect', { x: 0, y: T + i * ch, width: L - 4, height: ch, class: 'c-hit' }),
      s('rect', { x: 1, y: T + i * ch + 1, width: W - 2, height: ch - 2, rx: 4, class: 'c-focus' }),
      s('text', { x: 0, y: T + i * ch + ch / 2 + 4, class: 'c-tick' }, DN[i]));
    row.forEach((v: any, hr: any) => {
      g.append(s('rect', { x: L + hr * cw + 1, y: T + i * ch + 1, width: cw - 2, height: ch - 2, rx: 3, class: v ? 'c-cell' : 'c-cell0',
        style: v ? `--op:${(0.12 + 0.88 * v / max).toFixed(3)};fill:var(--series-1);fill-opacity:${(0.12 + 0.88 * v / max).toFixed(3)}` : null,
        'data-tip': `${DN[i]} ${String(hr).padStart(2, '0')}:00\n${fmt(v)} on average` }));
    });
    svg.append(g);
  });
  for (const hr of [0, 6, 12, 18, 23]) svg.append(s('text', { x: L + hr * cw + cw / 2, y: H - 4, class: 'c-tick', 'text-anchor': 'middle' }, String(hr).padStart(2, '0') + 'h'));
  return svg;
}
