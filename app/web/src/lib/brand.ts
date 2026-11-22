// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

// Per-variant logo assets in public/brand/themes/<variant>/, written by app/icons/themes/import.py.
import { THEMES } from './themes';

export interface Img { src: string; srcset: string }

const known = (v: string) => (THEMES.some((t) => t.id === v) ? v : 'dark');
const dir = (v: string) => `brand/themes/${known(v)}/`;
const pair = (v: string, name: string): Img => {
  const d = dir(v);
  return { src: `${d}${name}.png`, srcset: `${d}${name}.png 1x, ${d}${name}@2x.png 2x` };
};

export const logo = (v: string, size: 24 | 32 | 48): Img => pair(v, `logo-${size}`);
export const icon16 = (v: string): Img => ({ src: `${dir(v)}icon-16.png`, srcset: `${dir(v)}icon-16.png 1x, ${dir(v)}logo-32.png 2x` });
export const about = (v: string): Img => {
  const d = dir(v);
  return { src: `${d}about-240.png`, srcset: `${d}about-240.png 1x, ${d}about-480.png 2x` };
};
