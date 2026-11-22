// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

import { ui } from '../../lib/stores/ui.svelte';
import { look } from '../../lib/stores/look.svelte';
import { setThemeIcon } from '../../lib/bridge';

export function installThemeIcon() {
  $effect.root(() => {
    // Wait for the real settings: before them the variant is the OS default, and Rust would store it.
    $effect(() => { if (ui.settingsApplied) setThemeIcon(look.variant); });
  });
}
