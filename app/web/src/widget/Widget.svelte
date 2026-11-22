<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import WidgetHead from "./WidgetHead.svelte";
  import Extended from "./Extended.svelte";
  import ScanBar from "../lib/components/ScanBar.svelte";
  import { ui } from "../lib/stores/ui.svelte";
  import { fitWidget, openDashboard, startDrag } from "../lib/bridge";
  import { notice } from "../lib/format";
  import { cells, hotId, periodLabel, recentOf, startupLine, tagOf, topFinding, type Cell } from "./lines";

  const big = $derived(!!ui.settings.widget_expanded);
  const n = $derived(notice(ui.s, ui.status));
  const sn = $derived(ui.s as any);
  const cur = $derived(ui.s ? cells(sn) : null);
  let hot = $state<ReturnType<typeof hotId> | null>(null);
  $effect.pre(() => {
    hot = cur ? hotId(cur) : null;
  });
  const session = $derived(sn && recentOf(sn).live?.sessions?.[0]);
  const tag = $derived(ui.s ? tagOf(sn, session) : null);
  const scanning = $derived(!!sn && sn.progress != null && sn.progress < 1);
  const view = $derived(ui.view);
  const startup = $derived(ui.s ? startupLine(sn) : undefined);
  const top = $derived(ui.s ? topFinding(sn) : null);
  const period = $derived(big && ui.s ? periodLabel(sn) : null);

  $effect(() => {
    document.getElementById("compact")?.classList.toggle("big", big);
  });

  onMount(() => {
    const el = document.getElementById("compact")!;
    let down: { x: number; y: number } | null = null;
    const click = (e: MouseEvent) => {
      const b = (e.target as Element).closest(".w-btn") as HTMLElement | null;
      if (!b) return;
      if (b.dataset.act === "toggle") ui.on.set("widget_expanded", !ui.settings.widget_expanded);
      else if (b.dataset.act === "open") openDashboard();
      else ui.on.set("widget", false);
    };
    const mousedown = (e: MouseEvent) => {
      if (e.button === 0 && !(e.target as Element).closest(".w-btn")) down = { x: e.screenX, y: e.screenY };
    };
    const mousemove = (e: MouseEvent) => {
      if (down && Math.hypot(e.screenX - down.x, e.screenY - down.y) > 3) {
        down = null;
        startDrag();
      }
    };
    const mouseup = () => {
      if (down) {
        down = null;
        openDashboard();
      }
    };
    const mouseleave = () => {
      down = null;
    };
    let sent = { w: 0, h: 0 };
    let frame = 0;
    const fit = new ResizeObserver(() => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const r = el.getBoundingClientRect();
        const wide = el.classList.contains("big");
        const w = wide ? 0 : Math.ceil(r.width);
        const h = Math.ceil(r.height);
        if (Math.abs(h - sent.h) >= 2 || Math.abs(w - sent.w) >= 2) {
          sent = { w, h };
          fitWidget(wide ? null : w, h);
        }
      });
    });
    fit.observe(el);
    el.addEventListener("click", click);
    el.addEventListener("mousedown", mousedown);
    el.addEventListener("mousemove", mousemove);
    el.addEventListener("mouseup", mouseup);
    el.addEventListener("mouseleave", mouseleave);
    return () => {
      fit.disconnect();
      cancelAnimationFrame(frame);
      el.removeEventListener("click", click);
      el.removeEventListener("mousedown", mousedown);
      el.removeEventListener("mousemove", mousemove);
      el.removeEventListener("mouseup", mouseup);
      el.removeEventListener("mouseleave", mouseleave);
    };
  });
</script>

{#snippet line(label: string, parts: Cell[] | null | undefined, hotCell: string | null)}
  <div class="row">
    <span class="lbl" title={label === "session" && tag ? tag.label + " session" : null}>
      {label === "session" && tag ? "sess " + tag.tag : label}
    </span>
    {#if parts && parts[0]}{@const c = parts[0]}<span
        class={"v" + (c.id && c.id === hotCell ? " accent" : "")}
        title={[c.act, c.tip].filter(Boolean).join(" · ") || null}>{c.text}</span
      >{:else}<span class="txt muted">-</span>{/if}
  </div>
{/snippet}

{#if ui.s || ui.status}
  <WidgetHead {big} {period} />
  {#if n && n.full}
    <div class="row">
      <span class="lbl">tokenwhy</span>
      <span class="txt muted">{n.short}</span>
    </div>
    {#if n.progress}<div class="w-scan"><ScanBar p={ui.progress} /></div>{/if}
  {:else if cur}
    {#if scanning}
      {@render line("today", cur.today, null)}
      <div class="w-scan"><ScanBar p={Math.max(sn.progress, ui.progress ?? 0)} /></div>
    {:else if big}
      <Extended s={ui.s!} />
    {:else if view === "A"}
      {@render line("today", cur.today, hot)}
      {@render line("session", cur.session, hot)}
    {:else if view === "B"}
      {@render line("startup", startup, null)}
      {@render line("fix", top ? [top] : null, null)}
    {:else}
      {@render line("today", cur.today, hot)}
      {@render line("session", cur.session, hot)}
      {@render line("fix", top ? [top] : null, null)}
    {/if}
    {#if n}
      <div class="row">
        <span class="lbl">error</span>
        <span class="txt muted">{n.short}</span>
      </div>
    {/if}
  {/if}
{/if}
