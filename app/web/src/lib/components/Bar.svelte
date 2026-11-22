<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  let { frac, tip = null, cls = '' }: { frac: number; tip?: string | null; cls?: string } = $props();
</script>

<span class={'cbar ' + (cls || '')} data-tip={tip || null}><i style:width={Math.max(frac > 0 ? 1.5 : 0, Math.min(100, frac * 100)) + '%'}></i></span>
