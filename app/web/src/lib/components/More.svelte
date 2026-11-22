<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import type { Ctx } from "../ctx";
  let {
    ctx,
    key,
    total,
    shown,
    noun,
  }: { ctx: Ctx; key: string; total: number; shown: number; noun: string } = $props();
  const open = $derived(ctx.open(key));
</script>

{#if total > shown || open}<button class="more" aria-expanded={open} onclick={() => ctx.ui.on.toggle(key)}>
    {open ? "Show fewer" : `Show all ${total} ${noun}`}
  </button>{/if}
