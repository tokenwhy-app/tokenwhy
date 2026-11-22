// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

import { credits } from '../../lib/bridge';

export const sources = $state<{ open: boolean; section: string | null; list: any[] | null; back: Element | null; seq: number }>({
  open: false, section: null, list: null, back: null, seq: 0,
});

export async function openSources(section?: string | null): Promise<void> {
  const back = sources.open ? sources.back : document.activeElement;
  closeSources(false);
  const list = await credits();
  Object.assign(sources, { open: true, section: section ?? null, list: list || null, back, seq: sources.seq + 1 });
}

export function closeSources(refocus = true): void {
  if (!sources.open) return;
  const back = sources.back as HTMLElement | null;
  Object.assign(sources, { open: false, section: null, list: null, back: null });
  if (refocus && back && back.focus) back.focus();
}
