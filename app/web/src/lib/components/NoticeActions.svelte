<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import { rescan, openLog, inApp } from "../bridge";
  import type { Notice } from "../types";
  let { n }: { n: Notice } = $props();
</script>

{#if inApp}<div class="notice-actions">
    <button class="icon-btn" onclick={() => rescan()}>Rescan</button>
    {#if n.log}<button class="icon-btn" onclick={() => openLog()}>Open log</button>{/if}
  </div>{/if}
