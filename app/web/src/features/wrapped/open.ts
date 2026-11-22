// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

import { mount, unmount } from 'svelte';
import './wrapped.css';
import Wrapped from './Wrapped.svelte';
import WrappedButton from './WrappedButton.svelte';
import { wrappedFont } from '../../lib/bridge';

export { WrappedButton };

let fontsReady: Promise<unknown> | null = null;
export function loadFonts() {
  fontsReady = fontsReady || Promise.all([0, 1].map(async (i) => {
    try {
      const buf = await wrappedFont(i);
      if (!buf) return;
      const f = new FontFace('twcard', buf instanceof ArrayBuffer ? buf : new Uint8Array(buf), { weight: i ? '700' : '400' });
      await f.load();
      document.fonts.add(f);
    } catch { /* the card falls back to the UI font */ }
  }));
  return fontsReady;
}

let inst: Record<string, any> | null = null;
export function openWrapped(opener: HTMLElement | null): void {
  if (inst) return;
  loadFonts();
  const onclose = () => {
    if (!inst) return;
    const back = opener && opener.isConnected ? opener : document.querySelector<HTMLElement>('.wrapped-btn');
    const i = inst;
    inst = null;
    unmount(i);
    if (back) back.focus();
  };
  inst = mount(Wrapped, { target: document.body, props: { opener, onclose } });
}
