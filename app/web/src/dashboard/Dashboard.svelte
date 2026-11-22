<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import { mount, onMount, unmount, tick } from "svelte";
  import Header from "./Header.svelte";
  import Overlays from "./Overlays.svelte";
  import Overview from "./tabs/Overview.svelte";
  import Startup from "./tabs/Startup.svelte";
  import Tools from "./tabs/Tools.svelte";
  import Agents from "./tabs/Agents.svelte";
  import Context from "./tabs/Context.svelte";
  import Skills from "./tabs/Skills.svelte";
  import NoticePage from "../lib/components/NoticePage.svelte";
  import NoticeBanner from "../lib/components/NoticeBanner.svelte";
  import ScanBar from "../lib/components/ScanBar.svelte";
  import { features } from "../features/registry";
  import { makeCtx } from "../lib/ctx";
  import { notice } from "../lib/format";
  import { ui } from "../lib/stores/ui.svelte";

  const BODY = {
    overview: Overview,
    startup: Startup,
    tools: Tools,
    agents: Agents,
    context: Context,
    skills: Skills,
  } as const;

  const n = $derived(notice(ui.s, ui.status));
  const projects = $derived.by(() => {
    const s = ui.s;
    if (!s) return [];
    return [
      ...new Set(
        [...(s.startup || []), ...(s.subagents || []), ...(s.live.sessions || [])]
          .map((r: any) => r.project)
          .filter(Boolean),
      ),
    ].sort() as string[];
  });
  const ctx = $derived(ui.s ? makeCtx(ui, ui.s) : null);
  const Body = $derived(BODY[ui.tab as keyof typeof BODY] || Overview);
  const scan = $derived(
    ui.s && ui.s.progress != null && ui.s.progress < 1 ? Math.max(ui.s.progress, ui.progress ?? 0) : null,
  );

  let lastTab = ui.tab;
  $effect(() => {
    const t = ui.tab;
    if (t === lastTab) return;
    lastTab = t;
    tick().then(() => {
      document.body.scrollTop = 0;
    });
  });

  onMount(() => {
    const overlays = mount(Overlays, { target: document.body });
    const esc = (e: KeyboardEvent) => {
      if (e.key === "Escape" && ui.settingsOpen) ui.on.settings();
    };
    addEventListener("keydown", esc);
    const openNow = new URLSearchParams(location.search).get("wrapped_open") === "1";
    for (const load of features.lazy)
      load().then((w) => {
        if (!w) return;
        ui.wrapped = w;
        if (openNow) w.open(null);
      });
    return () => {
      removeEventListener("keydown", esc);
      unmount(overlays);
    };
  });
</script>

{#if ui.s || ui.status}
  {#if n && n.full}
    <Header projects={[]} />
    <main class="page"><NoticePage {n} progress={ui.progress} /></main>
  {:else if ctx}
    <Header {projects} />
    <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
    <main class="page" id={"panel-" + ui.tab} role="tabpanel" aria-labelledby={"tab-" + ui.tab}>
      {#if scan != null}<ScanBar p={scan} label="Reading session logs" />{/if}{#if n}<NoticeBanner
          {n}
        />{/if}<Body {ctx} />
    </main>
  {/if}
{/if}
