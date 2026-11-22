<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import type { Snippet } from "svelte";
  import { fitCard } from "../fitCard";
  let {
    title,
    note = null,
    cls = "",
    children,
    foot,
  }: { title: string; note?: string | null; cls?: string; children: Snippet; foot?: Snippet } = $props();
</script>

<section class={"card " + (cls || "")} use:fitCard>
  <div class="card-head">
    <h2>{title}</h2>
    {#if note}<span class="card-note">{note}</span>{/if}
  </div>
  <div class="card-body">{@render children()}</div>
  {#if foot}<div class="card-foot">{@render foot()}</div>{/if}
</section>
