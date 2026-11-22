<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import { ui } from "../../lib/stores/ui.svelte";
  import { int } from "../../lib/format";
  const as = $derived(((ui.s && (ui.s as any).accounts) || []) as any[]);
  const short = (label: string) => {
    const name = label.replace(/'s Organization$/, "").split("@")[0];
    return name.length > 12 ? name.slice(0, 11) + "…" : name;
  };
</script>

{#if as.length >= 2}
  <div class="seg" role="group" aria-label="Account">
    {#each [{ id: "", label: "All", sessions: null }, ...as] as a (a.id)}
      <button
        aria-pressed={(ui.account || "") === a.id}
        data-tip={a.sessions != null
          ? `${a.label}
${int(a.sessions)} ${a.sessions === 1 ? "session" : "sessions"}`
          : null}
        onclick={() => ui.on.account(a.id || null)}
      >
        {a.id ? short(a.label) : a.label}
      </button>
    {/each}
  </div>
{/if}
