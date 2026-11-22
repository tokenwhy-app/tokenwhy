<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import SettingsPanel from "./SettingsPanel.svelte";
  import { ui } from "../../lib/stores/ui.svelte";

  const FOCUSABLE = "button, input, select";
  let root: HTMLDivElement;
  const close = () => ui.on.settings();

  function trap(ev: KeyboardEvent) {
    if (ev.key !== "Tab") return;
    const f = [...(ev.currentTarget as HTMLElement).querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
      (x) => !(x as HTMLButtonElement).disabled,
    );
    if (!f.length) return;
    const first = f[0],
      last = f[f.length - 1];
    if (ev.shiftKey && document.activeElement === first) {
      ev.preventDefault();
      last.focus();
    } else if (!ev.shiftKey && document.activeElement === last) {
      ev.preventDefault();
      first.focus();
    }
  }

  onMount(() => {
    (root.querySelector<HTMLElement>(FOCUSABLE) || root).focus();
    return () => {
      const b = document.querySelector<HTMLElement>(".icon-btn[aria-expanded]");
      if (b) b.focus();
    };
  });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions, a11y_no_noninteractive_element_interactions -->
<!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
<div class="drawer-root enter" bind:this={root}>
  <div class="scrim" onclick={close}></div>
  <aside class="drawer" role="dialog" aria-modal="true" aria-label="Settings" onkeydown={trap}>
    <div class="drawer-head">
      <h2>Settings</h2>
      <button class="icon-btn" aria-label="Close settings" onclick={close}>×</button>
    </div>
    <SettingsPanel />
  </aside>
</div>
