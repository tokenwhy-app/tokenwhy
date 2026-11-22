<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import { windowAction } from "../lib/bridge";
  const BTNS: ["minimize" | "toggleMaximize" | "close", string][] = [
    ["minimize", "Minimize"],
    ["toggleMaximize", "Maximize"],
    ["close", "Close"],
  ];
</script>

<div class="caption">
  {#each BTNS as [act, t] (act)}<button
      class={"cap-btn" + (act === "close" ? " cap-close" : "")}
      title={t}
      aria-label={t}
      onclick={() => windowAction(act)}
    >
      <svg viewBox="0 0 11 11" aria-hidden="true">
        {#if act === "minimize"}<path
            d="M1 5.5h9"
            stroke="currentColor"
          />{:else if act === "toggleMaximize"}<rect
            x="1.5"
            y="1.5"
            width="8"
            height="8"
            rx="1.2"
            fill="none"
            stroke="currentColor"
          />{:else}<path d="M1.5 1.5l8 8M9.5 1.5l-8 8" stroke="currentColor" stroke-linecap="round" />{/if}
      </svg>
    </button>{/each}
</div>
