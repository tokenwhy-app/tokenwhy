<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  // Stacking comes from each overlay's z-index, not from this order.
  import Drawer from './settings/Drawer.svelte';
  import Sources from '../features/sources/Sources.svelte';
  import Tour from '../features/tour/Tour.svelte';
  import { ui } from '../lib/stores/ui.svelte';
</script>

{#if ui.settingsOpen}<Drawer />{/if}<Sources /><Tour />
