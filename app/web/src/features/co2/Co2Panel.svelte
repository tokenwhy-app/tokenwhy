<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import { ui } from "../../lib/stores/ui.svelte";
  import Seg from "../../lib/components/Seg.svelte";
  import { openSources } from "../sources/store.svelte";

  const c = $derived(ui.settings && ui.settings.co2);
  const info = $derived(ui.s && (ui.s as any).co2);
  const eco = $derived(c && c.method !== "jegham");
  const set = (k: string, v: unknown) => ui.on.co2({ ...c, [k]: v });
  const change = (k: string) => (e: Event) => {
    const t = e.currentTarget as HTMLInputElement;
    const v = t.value.trim() === "" ? null : Number(t.value);
    const ok = v == null || (Number.isFinite(v) && v >= 0);
    if (!ok) t.value = val(k);
    set(k, ok ? v : c[k]);
  };
  const val = (k: string) => (c[k] == null ? "" : String(c[k]));
</script>

{#if c}
  <div class="settings-col">
    <h2>CO2 estimate</h2>
    <div class="field">
      <span>Method</span>
      <Seg
        items={[
          ["ecologits", "EcoLogits"],
          ["jegham", "Jegham"],
        ]}
        cur={c.method || "ecologits"}
        on={(v) => set("method", v)}
        label="CO2 method"
      />
    </div>
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <p
      class="sw-hint"
      tabindex="0"
      data-tip={eco ? "Output tokens only, model size guessed" : "All tokens; cache reads at a share"}
    >
      {eco ? "A range from output tokens only." : "One value from every token."}
    </p>
    {#if !eco}
      <div class="field">
        <span>Cache read, share of input</span>
        <input
          class="text"
          type="number"
          min="0"
          step="any"
          value={val("cache_read_factor")}
          placeholder="0.08"
          aria-label="Cache-read factor"
          onchange={change("cache_read_factor")}
        />
      </div>
    {/if}
    <div class="field">
      <span>Grid, g CO2e/kWh</span>
      <input
        class="text"
        type="number"
        min="0"
        step="any"
        value={val("grid_g_per_kwh")}
        placeholder={eco ? "384" : "287"}
        aria-label="Grid intensity"
        onchange={change("grid_g_per_kwh")}
      />
    </div>
    <p class="sw-hint">
      {"Leave a box empty for the method's value" + (info ? `, data from ${info.data_date}.` : ".")}
    </p>
    <button class="icon-btn" onclick={() => openSources("CO2 estimates")}>CO2 sources</button>
  </div>
{/if}
