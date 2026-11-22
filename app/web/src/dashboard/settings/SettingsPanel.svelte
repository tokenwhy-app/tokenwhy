<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import { look } from "../../lib/stores/look.svelte";
  import { about } from "../../lib/brand";
  import Appearance from "./Appearance.svelte";
  import Seg from "../../lib/components/Seg.svelte";
  import Toggle from "../../lib/components/Toggle.svelte";
  import { features } from "../../features/registry";
  import { openSources } from "../../features/sources/store.svelte";
  import { ui } from "../../lib/stores/ui.svelte";
  import { getSetting, mcpSetupLine } from "../../lib/bridge";
  import CopyFixes from "../../lib/components/CopyFixes.svelte";

  const SECTIONS = [
    { id: "appearance", label: "Appearance", core: true },
    { id: "widget", label: "Widget", core: true },
    { id: "data", label: "Data and privacy", core: true },
    { id: "agents", label: "Agents", core: true },
    { id: "about", label: "About", core: true },
  ];
  const panels = (id: string) => features.settingsPanels.filter((p) => p.section === id).map((p) => p.panel);
  const list = SECTIONS.filter((s) => s.core || panels(s.id).length);

  let mcpNote = $state("");
  async function copyMcp() {
    try {
      await navigator.clipboard.writeText(await mcpSetupLine());
      mcpNote = "Copied. Paste it in a terminal to add tokenwhy to Claude Code.";
    } catch (e) {
      mcpNote = "Could not copy: " + (e instanceof Error ? e.message : String(e));
    }
  }

  const st = $derived(ui.settings);

  const opacity = $derived(typeof st.widget_opacity === "number" ? st.widget_opacity : 100);
  const coin = $derived(about(look.variant));
  const saved = String(ui.settings.settings_section || getSetting("settings_section", list[0].id));
  let cur = $state(list.some((s) => s.id === saved) ? saved : list[0].id);
  let nav: HTMLElement;
  const pick = (id: string) => {
    cur = id;
    ui.on.set("settings_section", id);
  };

  function keys(ev: KeyboardEvent) {
    const i = list.findIndex((s) => s.id === cur);
    const j = { ArrowDown: i + 1, ArrowUp: i - 1, Home: 0, End: list.length - 1 }[ev.key];
    if (j == null) return;
    ev.preventDefault();
    pick(list[(j + list.length) % list.length].id);
    nav.querySelector<HTMLElement>(`[data-sec="${cur}"]`)?.focus();
  }
</script>

<div class="panel settings sections">
  <!-- svelte-ignore a11y_interactive_supports_focus -->
  <div
    class="sec-list"
    role="tablist"
    aria-orientation="vertical"
    aria-label="Settings sections"
    bind:this={nav}
    onkeydown={keys}
  >
    {#each list as s (s.id)}<button
        role="tab"
        data-sec={s.id}
        id={"sec-" + s.id}
        aria-selected={cur === s.id}
        aria-controls="sec-body"
        tabindex={cur === s.id ? 0 : -1}
        onclick={() => pick(s.id)}
      >
        {s.label}
      </button>{/each}
  </div>
  <div class="sec-body" id="sec-body" role="tabpanel" aria-labelledby={"sec-" + cur}>
    {#if cur === "appearance"}<Appearance />
    {:else if cur === "widget"}
      <div class="settings-col">
        <Toggle
          label="Show the compact widget"
          hint="Always-on summary, draggable"
          on={!!st.widget}
          change={(v) => ui.on.set("widget", v)}
        />
        <div class={"field" + (st.widget ? "" : " off")}>
          <span>Widget shows</span>
          <Seg
            items={[
              ["A", "Today and session"],
              ["B", "Startup and top fix"],
              ["C", "Everything"],
            ]}
            cur={ui.view}
            on={ui.on.view}
            label="Widget shows"
          />
        </div>
        <div class={"field" + (st.widget ? "" : " off")}>
          <span>Background opacity</span>
          <span class="w-opacity">
            <input type="range" min="0" max="100" step="5" value={opacity} aria-label="Background opacity"
              oninput={(e) => ui.on.set("widget_opacity", Number(e.currentTarget.value))} />
            <span class="num">{opacity} %</span>
            {#if typeof st.widget_opacity === "number"}
              <button class="icon-btn" onclick={() => ui.on.set("widget_opacity", null)}>Reset</button>
            {/if}
          </span>
        </div>
        <Toggle
          label="Keep the widget on top"
          on={!!st.always_on_top}
          change={(v) => ui.on.set("always_on_top", v)}
        />
        <Toggle
          label="Launch at login"
          hint="Starts in the tray"
          on={!!st.autostart}
          change={(v) => ui.on.set("autostart", v)}
        />
      </div>
    {:else if cur === "agents"}
      <div class="settings-col">
        <p class="sw-hint">Let Claude Code ask tokenwhy where its tokens went, over a local MCP server.</p>
        <div class="notice-actions">
          <CopyFixes />
          <button class="icon-btn" onclick={copyMcp}>Copy MCP setup</button>
        </div>
        {#if mcpNote}<p class="sw-hint" role="status">{mcpNote}</p>{/if}
      </div>
    {:else if cur === "about"}
      <div class="settings-col">
        <div class="about-art">
          <img class="about-coin" src={coin.src} srcset={coin.srcset} width="96" height="96" alt="" />
          <div>
            <p class="about-name">tokenwhy</p>
            <p class="sw-hint">Where your agent tokens went, and why</p>
          </div>
        </div>
        {#if st.version}<p class="sw-hint">{"tokenwhy " + st.version}</p>{/if}
        <p class="sw-hint">
          {features.aboutNotes.map((f) => f()).find(Boolean) ||
            "tokenwhy reads the log files on this computer and sends nothing over the network."}
        </p>
        <div class="notice-actions">
          <button class="icon-btn" onclick={() => openSources()}>Sources and credits</button>
          <button class="icon-btn" onclick={ui.on.tour}>Show the tour again</button>
        </div>
      </div>
    {/if}
    {#if cur === "data"}<div class="settings-col">
        <Toggle label="Anonymise project names" hint="For screenshots" on={ui.anon} change={ui.on.anon} />
      </div><p class="sw-hint data-local">
        By default, everything stays on this computer: tokenwhy reads your agents' log files locally and sends
        nothing over the network.
      </p>{/if}
    {#each panels(cur) as P}<P />{/each}
  </div>
</div>
