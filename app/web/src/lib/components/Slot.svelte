<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  let { draw, cls = '' }: { draw: (w: number) => Element; cls?: string } = $props();
  let el: HTMLDivElement;
  let cw = $state(0);
  let w = $state(0);
  let t: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const next = Math.floor(cw);
    if (!w) { w = next; return; }
    clearTimeout(t);
    t = setTimeout(() => { w = next; }, 60);
    return () => clearTimeout(t);
  });
  $effect(() => {
    el.replaceChildren();
    if (w > 0) el.append(draw(w));
  });
</script>

<div class={'slot ' + (cls || '')} bind:this={el} bind:clientWidth={cw}></div>
