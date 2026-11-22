// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

// A theme token as an opaque "rgb(r, g, b)": translucent tokens (Glass) are laid over the surface under them, because the
// PNG has no wallpaper or blur behind it.
import type { WrappedColors } from '../../lib/types';

type Rgba = number[];
function rgba(v: string): Rgba {
  const m = /rgba?\(([^)]+)\)/.exec(v);
  if (!m) return [0, 0, 0, 1];
  const p = m[1].split(/[ ,/]+/).filter(Boolean).map(Number);
  return [p[0], p[1], p[2], p.length > 3 ? p[3] : 1];
}
function over(top: Rgba, base: Rgba): Rgba {
  const a = top[3];
  return [0, 1, 2].map((i) => Math.round(top[i] * a + base[i] * (1 - a))).concat(1);
}
const css = (c: Rgba) => `rgb(${c[0]}, ${c[1]}, ${c[2]})`;

export function palette(): WrappedColors {
  const probe = document.createElement('span');
  probe.style.position = 'absolute';
  probe.style.visibility = 'hidden';
  document.body.append(probe);
  const root = getComputedStyle(document.documentElement);
  const read = (name: string): Rgba => {
    if (!root.getPropertyValue(name).trim()) return null as unknown as Rgba;
    probe.style.color = `var(${name})`;
    return rgba(getComputedStyle(probe).color);
  };
  const dark = root.colorScheme.includes('dark');
  const solid = read('--overlay-solid');
  const ground = solid && solid[3] === 1 ? solid : dark ? [16, 16, 18, 1] : [244, 244, 242, 1];
  const plane = over(read('--plane'), ground);
  const card = over(read('--card'), plane);
  const on = (n: string) => css(over(read(n), card));
  const out = {
    plane: css(plane), card: css(card), fg: on('--fg'), fg2: on('--fg-2'), muted: on('--muted'), rule: on('--rule'),
    accent: on('--accent'), accent_soft: on('--accent-soft'), accent_ink: css(over(read('--accent-ink'), over(read('--accent'), card))),
    series: [1, 2, 3, 4, 5, 6, 7].map((i) => on('--series-' + i)),
  };
  probe.remove();
  return out;
}
