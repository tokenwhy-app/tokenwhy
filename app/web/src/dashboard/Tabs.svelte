<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts" module>
  export const TAB_TIPS: Record<string, string> = {
    startup: "Loaded at session start",
    agents: "Helper agents a session starts",
    context: "Compaction: long chat summarised",
  };
  export const TAB_SUBS: Record<string, string> = {
    startup: "What every session loads before your first message.",
    tools: "What tool calls and their outputs cost.",
    agents: "Subagents: helper agents a session starts for part of the work.",
    context: "Long chats and compaction, when the agent summarises the chat to keep going.",
    skills: "Skills and memory files the agent reads.",
  };
</script>

<script lang="ts">
  import { ui, TABS } from "../lib/stores/ui.svelte";
  const go = (i: number) => {
    const id = TABS[(i + TABS.length) % TABS.length][0];
    ui.on.tab(id);
    requestAnimationFrame(() => {
      const b = document.getElementById("tab-" + id);
      if (b) b.focus();
    });
  };
  function key(e: KeyboardEvent, i: number) {
    if (e.key === "ArrowRight") {
      e.preventDefault();
      go(i + 1);
    } else if (e.key === "ArrowLeft") {
      e.preventDefault();
      go(i - 1);
    } else if (e.key === "Home") {
      e.preventDefault();
      go(0);
    } else if (e.key === "End") {
      e.preventDefault();
      go(TABS.length - 1);
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
<nav class="tabs" role="tablist" aria-label="Sections">
  {#each TABS as [id, label], i (id)}<button
      id={"tab-" + id}
      role="tab"
      aria-selected={ui.tab === id}
      aria-controls={"panel-" + id}
      tabindex={ui.tab === id ? 0 : -1}
      onclick={() => ui.on.tab(id)}
      onkeydown={(e) => key(e, i)}
    >
      {label}
    </button>{/each}
</nav>
{#if TAB_SUBS[ui.tab]}<p class="tab-sub">{TAB_SUBS[ui.tab]}</p>{/if}
