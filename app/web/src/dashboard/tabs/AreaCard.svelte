<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import Card from '../../lib/components/Card.svelte';
  import Empty from '../../lib/components/Empty.svelte';
  import FindingList from '../../lib/components/FindingList.svelte';
  import { rate } from '../../lib/diagnose';
  import type { Ctx } from '../../lib/ctx';
  import { areaFindings } from './shared';
  let { ctx, area }: { ctx: Ctx; area: string } = $props();
  const rows = $derived(areaFindings(ctx, area));
</script>

<Card title="What to change" note={rate(ctx.s) ? 'cost a week' : 'tokens a week'} cls="span-12 h-find">
  {#if rows.length}<FindingList {ctx} {rows} n={3} key={'f-' + area} />{:else}<Empty text="Nothing to change here for this window and project." />{/if}
</Card>
