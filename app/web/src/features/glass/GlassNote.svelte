<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  let { family, running }: { family: string; running: string } = $props();
  const note = $derived(family === 'glass' && running === 'eco' ? 'Glass needs Normal mode; Eco shows the Dark look.' : null);
</script>

{#if note}
  <div class="glass-note" role="note"><span>{note}</span></div>
{/if}
