// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

import { effectiveTheme, familyOf } from '../themes';
import { loadWallpaper, stopWallpaper } from '../../features/glass/wall';

class Look {
  variant = $state('dark');
  private settings: () => Record<string, any> = () => ({});
  private osDark = matchMedia('(prefers-color-scheme: dark)');

  install(read: () => Record<string, any>) {
    this.settings = read;
    this.osDark.addEventListener('change', () => this.apply());
    addEventListener('focus', () => this.apply());
    addEventListener('blur', () => this.apply());
  }

  apply() {
    const st = this.settings(), root = document.documentElement;
    const eco = (st.running_performance || st.performance) === 'eco';
    const t = effectiveTheme(st.theme, st.mode, eco, this.osDark.matches);
    root.dataset.theme = t;
    root.dataset.family = familyOf(st.theme).id;
    root.dataset.perf = eco ? 'eco' : 'normal';
    root.dataset.glass = 'fallback';
    this.variant = t;
    const a = st.widget_opacity;
    if (typeof a === 'number') root.style.setProperty('--w-alpha', String(a / 100));
    else root.style.removeProperty('--w-alpha');
    root.dataset.wclear = typeof a === 'number' && a <= 30 ? '1' : '0';
    if (t === 'glass' && !eco) loadWallpaper(); else stopWallpaper();
  }
}

export const look = new Look();
