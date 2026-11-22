<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  let { p, label = null, pct = true }: { p: number | null; label?: string | null; pct?: boolean } = $props();
  const busy = $derived(p == null || p <= 0 || p >= 1);
  const w = $derived(p == null ? 0 : Math.round(Math.min(1, Math.max(0, p)) * 100));
</script>

<div class="scan" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={busy ? undefined : w}>
  {#if label}<span>{label}</span>{/if}
  <span class="scan-bar" class:busy><i style:transform={busy ? null : 'scaleX(' + w / 100 + ')'}></i></span>
  {#if pct && !busy}<span class="num">{w + ' %'}</span>{/if}
</div>
