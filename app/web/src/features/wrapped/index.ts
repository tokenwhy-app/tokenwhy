// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

import type { WrappedHandle } from '../../lib/stores/ui.svelte';
import { wrappedAvailable } from '../../lib/bridge';

export async function load(): Promise<WrappedHandle | null> {
  if (!(await wrappedAvailable())) return null;
  const m = await import('./open');
  return { Button: m.WrappedButton, open: m.openWrapped };
}
