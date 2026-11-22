<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import { pct } from "../format";
  type Week = { cur: number; prev: number; delta: number | null } | null | undefined;
  let { w, fmt }: { w: Week; fmt: (v: number) => string } = $props();
</script>

{#if w && w.delta != null}
  {@const d = w.delta}
  {@const flat = Math.abs(d) < 0.005}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <span
    class={"chip " + (flat ? "" : d > 0 ? "up" : "down")}
    tabindex="0"
    data-tip={`Last 7 days ${fmt(w.cur)}\nThe 7 days before ${fmt(w.prev)}`}
  >
    {(flat ? "±" : d > 0 ? "+" : "−") + pct(Math.abs(d)) + " vs last week"}
  </span>
{/if}
