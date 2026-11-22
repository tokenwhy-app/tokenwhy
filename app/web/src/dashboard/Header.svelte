<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts" module>
  import { int, withData } from "../lib/format";
  import type { Ctx } from "../lib/ctx";

  export const shownProviders = (ctx: Ctx) =>
    withData(ctx.s).filter((p: any) => !ctx.ui.provider || p.id === ctx.ui.provider);
  export const CAP: Record<string, [string, string]> = {
    startup: ["Startup breakdown", "Not recorded in these transcripts"],
    tool_outputs: ["Tool output", "Not recorded in these transcripts"],
    subagents: ["Subagents", "Not recorded in these transcripts"],
    compactions: ["Compactions", "Not recorded in these transcripts"],
    skills: ["Skills", "Not recorded in these transcripts"],
    memory: ["Memory files", "Not recorded in these transcripts"],
  };
  export function na(ctx: Ctx, cap: string): { name: string; tip: string; text: string } | null {
    const ps = shownProviders(ctx);
    if (!ps.length || ps.some((p: any) => p.caps[cap] !== "none")) return null;
    const [name, tip] = CAP[cap];
    return {
      name,
      tip,
      text: `${name}: not available for ${ps.map((p: any) => p.label).join(", ")} transcripts`,
    };
  }
  export function provOf(ctx: Ctx, r: any) {
    if (ctx.ui.provider || withData(ctx.s).length < 2 || !r.provider) return "";
    const p = withData(ctx.s).find((x: any) => x.id === r.provider);
    return " · " + (p ? p.label : r.provider);
  }
  export function providerNotes(ctx: Ctx): string[] {
    const lines: string[] = [];
    for (const p of shownProviders(ctx) as any[]) {
      if (p.imported) lines.push(`${p.label}: daily totals imported from ccusage`);
      else if (p.caps.cost === "estimate") lines.push(`${p.label}: API-equivalent price estimate`);
      else if (p.caps.cost === "stored") lines.push(`${p.label}: cost as logged, rate table where missing`);
      for (const k of p.skipped || []) lines.push(`${p.label}: ${int(k.count)} skipped, ${k.reason}.`);
    }
    const up = ctx.s.unpriced_models || [];
    if (up.length) lines.push(`No price for ${up.join(", ")}, counted as $0`);
    if (ctx.s.history_newer != null)
      lines.push(`history.bin is from a newer tokenwhy (format ${ctx.s.history_newer}), ignored`);
    return lines;
  }
  const provTip = (p: any) =>
    p.imported
      ? "Imported ccusage totals"
      : `${int(p.files)} ${p.files === 1 ? "source" : "sources"} in ${p.roots} ${p.roots === 1 ? "folder" : "folders"}`;
</script>

<script lang="ts">
  import Mark from "./Mark.svelte";
  import Caption from "./Caption.svelte";
  import Tabs from "./Tabs.svelte";
  import Seg from "../lib/components/Seg.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import { features } from "../features/registry";
  import { inApp, startDrag, windowAction } from "../lib/bridge";
  import { ui } from "../lib/stores/ui.svelte";
  import { moneyView } from "./tabs/overview";
  let { projects }: { projects: string[] } = $props();

  const ps = $derived(withData(ui.s) as any[]);
  const provs = $derived([{ id: "", label: "All" }, ...ps] as any[]);
  const who = $derived.by(() => {
    const cur = ps.find((p) => p.id === ui.provider);
    return cur ? cur.label : ps.length > 1 ? "agent" : ps.length ? ps[0].label : "Claude Code";
  });

  const interactive = (e: MouseEvent) =>
    !!(e.target as Element).closest("button, select, input, a, label, .seg");
  function dragFrom(e: MouseEvent) {
    if (inApp && e.button === 0 && e.detail === 1 && !interactive(e)) startDrag();
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions, a11y_no_static_element_interactions -->
<header
  class="top"
  onmousedown={dragFrom}
  ondblclick={(e) => !interactive(e) && windowAction("toggleMaximize")}
>
  {#if inApp}<Caption />{/if}
  <div class="top-row">
    <div class="brand">
      <Mark />
      <div>
        <h1>tokenwhy</h1>
        <p>{"Where your " + who + " tokens went, " + (ui.days ? "last " + ui.days + " days" : "all time")}</p>
      </div>
    </div>
    <div class="controls">
      {#if ps.length >= 2}<div class="seg" role="group" aria-label="Agent">
          {#each provs as p (p.id)}<button
              aria-pressed={(ui.provider || "") === p.id}
              data-tip={p.caps ? provTip(p) : null}
              onclick={() => ui.on.provider(p.id || null)}
            >
              {p.label}
            </button>{/each}
        </div>{/if}{#each features.headerChips as Chips}<Chips />{/each}<Seg
        items={[
          [7, "7 days"],
          [14, "14 days"],
          [30, "30 days"],
          [0, "All"],
        ]}
        cur={ui.days}
        on={(v) => ui.on.days(Number(v))}
        label="Window"
      /><Seg
        items={[
          ["usd", "$"],
          ["pct", "%"],
        ]}
        cur={moneyView(ui)}
        on={(v) => ui.on.set("money_view", v)}
        label="Show savings as"
      />
      <span class="sel-wrap">
        <select
          class="select"
          aria-label="Project"
          onchange={(e) => ui.on.project(e.currentTarget.value || null)}
        >
          <option value="">All projects</option>
          {#each projects as p (p)}<option value={p} selected={p === ui.project}>{p}</option>{/each}
        </select>
      </span>
      {#if ui.wrapped}<ui.wrapped.Button />{/if}
      <button class="icon-btn" aria-expanded={!!ui.settingsOpen} onclick={ui.on.settings} title="Settings">
        <Icon name="gear" />
        <span>Settings</span>
      </button>
    </div>
  </div>
  <Tabs />
</header>
