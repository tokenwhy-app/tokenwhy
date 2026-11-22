<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import { icon16 } from "../../lib/brand";

  let { variant }: { variant: string } = $props();
  const ico = $derived(icon16(variant));
</script>

<span class="pv" data-preview={variant} class:glass={variant === "glass"} aria-hidden="true">
  <span class="pv-plane">
    <span class="pv-top">
      <img src={ico.src} srcset={ico.srcset} width="12" height="12" alt="" />
      <span class="pv-line pv-title"></span>
    </span>
    <span class="pv-card">
      <span class="pv-line"></span>
      <span class="pv-line pv-short"></span>
      <span class="pv-chip"></span>
    </span>
  </span>
</span>

<style>
  .pv {
    display: block;
    width: 100%;
    height: 56px;
    border-radius: 7px;
    overflow: hidden;
    box-shadow: 0 0 0 1px var(--rule-strong);
    background: var(--plane);
  }
  .pv.glass {
    background: var(--mesh);
  }
  .pv-plane {
    position: relative;
    display: block;
    width: 100%;
    height: 100%;
    background: var(--plane);
  }
  .pv-top {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 16px;
    padding: 0 5px;
    background: var(--card);
    border-bottom: 1px solid var(--rule);
  }
  .pv-top img {
    display: block;
    flex: none;
  }
  .pv-card {
    position: absolute;
    left: 7px;
    right: 7px;
    top: 22px;
    bottom: 6px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 5px 6px;
    background: var(--card);
    border: 1px solid var(--rule);
    border-radius: 5px;
  }
  .pv-line {
    display: block;
    height: 3px;
    width: 70%;
    border-radius: 2px;
    background: var(--fg);
    opacity: 0.75;
  }
  .pv-title {
    width: 28px;
    opacity: 0.6;
  }
  .pv-short {
    width: 40%;
    background: var(--muted);
  }
  .pv-chip {
    position: absolute;
    right: 6px;
    bottom: 5px;
    width: 16px;
    height: 7px;
    border-radius: 4px;
    background: var(--accent);
  }
</style>
