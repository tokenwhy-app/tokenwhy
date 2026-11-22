<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import type { Ctx } from '../../lib/ctx';
  import { CAP, naText } from './shared';
  let { ctx, cap, cls }: { ctx: Ctx; cap: string; cls: string } = $props();
</script>

<section class={'card ' + cls}><div class="card-head"><h2>{CAP[cap][0]}</h2></div><div class="card-body">
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div class="empty na" data-tip={CAP[cap][1]} tabindex="0"><span class="empty-dot"></span>{naText(ctx, cap)}</div>
</div></section>
