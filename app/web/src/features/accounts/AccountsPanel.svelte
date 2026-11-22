<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import { ui } from "../../lib/stores/ui.svelte";
  import { int } from "../../lib/format";

  const roots = $derived((ui.settings.account_roots || []) as any[]);
  const labels = $derived((ui.settings.account_labels || {}) as Record<string, string>);
  const logged = $derived(
    (((ui.s && (ui.s as any).accounts) || []) as any[]).filter(
      (a) => a.id !== "unattributed" && !a.id.startsWith("root:"),
    ),
  );
  let label: HTMLInputElement | undefined = $state();
  let path: HTMLInputElement | undefined = $state();

  const setRoots = (v: any[]) => ui.on.set("account_roots", v);
  const add = () => {
    const p = path!.value.trim().replace(/^"|"$/g, "");
    if (p)
      setRoots([
        ...roots,
        { provider: "claude-code", path: p, label: label!.value.trim() || "Account " + (roots.length + 1) },
      ]);
    if (p) {
      path!.value = "";
      label!.value = "";
    }
  };
  const rename = (id: string, v: string) => {
    const next = { ...labels };
    if (v.trim()) next[id] = v.trim();
    else delete next[id];
    ui.on.set("account_labels", next);
  };
</script>

<div class="settings-col">
  <h2>Accounts</h2>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <p class="sw-hint" tabindex="0" data-tip="Closed-app switches stay unattributed">
    One Claude config folder per account, or one folder with /login.
  </p>
  {#each roots as r, i}
    <div class="field">
      <input
        class="text"
        type="text"
        value={r.label}
        aria-label="Account label"
        onchange={(e) => {
          const v = e.currentTarget.value.trim();
          if (!v) e.currentTarget.value = r.label;
          setRoots(roots.map((x, j) => (j === i ? { ...x, label: v || x.label } : x)));
        }}
      />
      <span class="sw-hint" title={r.path}>{r.path}</span>
      <button class="icon-btn" onclick={() => setRoots(roots.filter((_, j) => j !== i))}>Remove</button>
    </div>
  {/each}
  <div class="field">
    <input
      bind:this={label}
      class="text"
      type="text"
      spellcheck="false"
      placeholder="Label, e.g. Pro"
      aria-label="Account label"
    />
    <input
      bind:this={path}
      class="text"
      type="text"
      spellcheck="false"
      placeholder={"C:\\Users\\me\\.claude-pro"}
      aria-label="Claude config folder"
      onkeydown={(e) => {
        if (e.key === "Enter") add();
      }}
    />
    <button class="icon-btn" onclick={add}>Add folder</button>
  </div>
  {#if logged.length}<p class="sw-hint">Accounts seen at /login</p>{/if}
  {#each logged as a (a.id)}
    <div class="field">
      <input
        class="text"
        type="text"
        value={labels[a.id] || a.label}
        aria-label="Account label"
        onchange={(e) => rename(a.id, e.currentTarget.value)}
      />
      <span class="sw-hint">{`${int(a.sessions)} ${a.sessions === 1 ? "session" : "sessions"}`}</span>
    </div>
  {/each}
</div>
