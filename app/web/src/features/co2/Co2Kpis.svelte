<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import type { Ctx } from '../../lib/ctx';
  import Chip from '../../lib/components/Chip.svelte';
  import InfoBubble from '../../lib/components/InfoBubble.svelte';
  import { co2, co2Range, co2Week, waterEquiv, waterRange } from './format';
  let { ctx, ds, filtered = false }: { ctx: Ctx; ds: any[]; filtered?: boolean } = $props();

  const s = $derived(ctx.s as any);
  const g = $derived(co2Week(s, ds, filtered));
</script>

{#if g}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div class="kpi has-info" tabindex="0">
    <span class="kpi-label">CO2e</span><span class="kpi-val num">{co2Range(g.lo, g.hi)}</span>
    <span class="kpi-note"><Chip w={g.mid} fmt={co2} /><span>{g.cacheNote || 'estimate'}</span></span>
    <InfoBubble section="CO2 estimates" label="CO2e and water" />
  </div>
  {#if g.water}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div class="kpi has-info" tabindex="0">
      <span class="kpi-label">Water</span><span class="kpi-val num">{waterRange(g.water.lo, g.water.hi)}</span>
      <span class="kpi-note"><span>{waterEquiv((g.water.lo + g.water.hi) / 2, s.co2.water_units) || 'estimate'}</span></span>
      <InfoBubble section="CO2 estimates" label="CO2e and water" />
    </div>
  {:else if !s.co2.water}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div class="kpi has-info" tabindex="0">
      <span class="kpi-label">Water</span><span class="kpi-val num">not covered</span>
      <span class="kpi-note"><span>Jegham method</span></span>
      <InfoBubble section="CO2 estimates" label="CO2e and water" />
    </div>
  {/if}
{/if}
