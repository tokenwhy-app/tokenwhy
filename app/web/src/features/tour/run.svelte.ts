// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

export interface Box { left: number; top: number; width: number; height: number }
export const view = $state<{
  id: string | null; title: string; text: string; i: number; n: number;
  ring: Box | null; card: { left: number; top: number } | null; go: (d: number) => void; end: () => void;
}>({ id: null, title: '', text: '', i: 0, n: 0, ring: null, card: null, go: () => {}, end: () => {} });
