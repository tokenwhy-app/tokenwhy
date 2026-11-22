<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import { ui } from "../../lib/stores/ui.svelte";
  import Toggle from "../../lib/components/Toggle.svelte";
  let input: HTMLInputElement | undefined = $state();
  const go = () => {
    const v = input?.value.trim();
    if (v) ui.on.importCcusage(v);
  };
</script>

<div class="settings-col">
  <h2>ccusage import</h2>
  <Toggle
    label="Count an imported ccusage export"
    hint="Daily totals; transcripts win"
    on={ui.settings.ccusage_import}
    change={(v) => ui.on.set("ccusage_import", v)}
  />
  {#if ui.settings.ccusage_import}
    <div class="import">
      <p class="sw-hint">
        Run <code>ccusage daily --by-agent --json &gt; ccusage.json</code>
         yourself, then give the file here. Importing it again replaces its rows.
      </p>
      <div class="field">
        <input
          bind:this={input}
          class="text"
          type="text"
          spellcheck="false"
          placeholder={"C:\\path\\to\\ccusage.json"}
          aria-label="ccusage export file"
          onkeydown={(e) => {
            if (e.key === "Enter") go();
          }}
        />
        <button class="icon-btn" onclick={go}>Import</button>
      </div>
      {#if ui.importMsg}<p class="sw-hint" role="status">{ui.importMsg}</p>{/if}
    </div>
  {/if}
</div>
