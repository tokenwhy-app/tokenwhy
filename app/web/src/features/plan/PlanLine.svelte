<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import type { Ctx } from '../../lib/ctx';
  import { planLine } from './planLine';
  let { ctx }: { ctx: Ctx } = $props();
  const l = $derived(planLine(ctx.s, ctx.ui.account));
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
{#if l}<p class="pnotes"><span class="num" tabindex="0" data-tip={l.tip}>{l.text}</span></p>{/if}
