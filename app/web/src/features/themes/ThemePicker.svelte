<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import ThemePreview from "./ThemePreview.svelte";
  import { FAMILIES, effectiveTheme } from "../../lib/themes";
  import { ui } from "../../lib/stores/ui.svelte";

  let { cur }: { cur: string } = $props();
  const os = matchMedia("(prefers-color-scheme: dark)");
  let osDark = $state(os.matches);
  $effect(() => {
    const f = () => (osDark = os.matches);
    os.addEventListener("change", f);
    return () => os.removeEventListener("change", f);
  });
  const mode = $derived(ui.settings.mode || "system");
</script>

<div class="tp" role="radiogroup" aria-label="Theme">
  {#each FAMILIES as f (f.id)}
    {@const on = cur === f.id}
    <button class="tp-item" class:on role="radio" aria-checked={on} onclick={() => ui.on.set("theme", f.id)}>
      <ThemePreview variant={effectiveTheme(f.id, mode, false, osDark)} />
      <span class="tp-name">{f.label}</span>
    </button>
  {/each}
</div>

<style>
  .tp {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: var(--s2);
    width: 100%;
    max-width: 330px;
  }
  .tp-item {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 5px;
    border: 1px solid var(--rule);
    border-radius: 10px;
    background: var(--card);
    color: var(--fg-2);
    font: inherit;
    font-size: var(--fs-s);
    text-align: left;
    cursor: pointer;
  }
  .tp-item:hover {
    color: var(--fg);
    border-color: var(--rule-strong);
  }
  .tp-item.on {
    color: var(--fg);
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
    font-weight: 500;
  }
  .tp-name {
    padding: 0 2px;
  }
</style>
