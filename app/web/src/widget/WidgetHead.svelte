<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  let { big, period }: { big: boolean; period?: string | null } = $props();
  const ICON: Record<string, string> = {
    grow: "M9 3h4v4M7 13H3V9M13 3 9 7M3 13l4-4",
    shrink: "M10 2v4h4M6 14v-4H2M14 2l-4 4M2 14l4-4",
    open: "M9 3h4v4M13 3 7 9M11 9v3.5a.5.5 0 0 1-.5.5h-7a.5.5 0 0 1-.5-.5v-7a.5.5 0 0 1 .5-.5H7",
    close: "M4 4l8 8M12 4l-8 8",
  };
  const btns = $derived([
    [big ? "shrink" : "grow", big ? "Collapse" : "Expand", "toggle"],
    ["open", "Open dashboard", "open"],
    ["close", "Hide widget", "hide"],
  ]);
</script>

<div class="w-head">
  {#if big}<span class="w-title">tokenwhy{#if period}<span class="w-period">{" · " + period}</span>{/if}</span>{/if}
  <span class="w-btns">
    {#each btns as [name, label, act] (act)}<button
        class="w-btn"
        type="button"
        title={label}
        aria-label={label}
        data-act={act}
      >
        <svg viewBox="0 0 16 16" aria-hidden="true">
          <g
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <path d={ICON[name]} />
          </g>
        </svg>
      </button>{/each}
  </span>
</div>
