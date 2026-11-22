<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import { agentBrief, mcpSetupLine } from "../bridge";

  let { report = false, mcp = false }: { report?: boolean; mcp?: boolean } = $props();

  let done = $state<"" | "brief" | "mcp" | "err">("");
  let timer: ReturnType<typeof setTimeout> | undefined;
  async function copy(kind: "brief" | "mcp") {
    try {
      await navigator.clipboard.writeText(kind === "brief" ? await agentBrief() : await mcpSetupLine());
      done = kind;
    } catch {
      done = "err";
    }
    clearTimeout(timer);
    timer = setTimeout(() => (done = ""), 2000);
  }
</script>

<span class="copy-fixes">
  <button class="icon-btn" onclick={() => copy("brief")}>
    {done === "brief" ? "Copied" : done === "err" ? "Could not copy" : report ? "Copy report for your agent" : "Copy fixes for your agent"}
  </button>
  {#if mcp}<button class="icon-btn" onclick={() => copy("mcp")}>{done === "mcp" ? "Copied" : "Copy MCP setup"}</button>{/if}
</span>

<style>
  .copy-fixes {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }
</style>
