<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  let { items, cur, on, label }: { items: [any, string][]; cur: any; on: (v: any) => void; label: string } =
    $props();
</script>

<div class="seg" role="group" aria-label={label}>
  {#each items as [v, text] (v)}<button aria-pressed={String(cur) === String(v)} onclick={() => on(v)}>
      {text}
    </button>{/each}
</div>
