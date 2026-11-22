<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import type { Snapshot } from "../lib/types";
  import { usd, tok, pct } from "../lib/format";
  import { causeColor } from "../lib/diagnose";
  import { sparkGeom, barsGeom, ringDash, pieGeom, RING } from "../lib/charts/spark";
  import { features } from "../features/registry";
  import { extendedData, unitLine } from "./lines";

  let { s }: { s: Snapshot } = $props();
  const d = $derived(extendedData(s));
  const unit = $derived(unitLine(s));
  const co2 = $derived(features.compactLines.map((f) => f(s.recent ?? s)).filter((x) => x != null));
  const weekLabel = "Spend per day, last 7 days";
  const spark = $derived(sparkGeom(d.week, 150, 26, weekLabel));
  const bars = $derived(
    barsGeom(d.hours, 150, 26, (i, v) => `${String(i).padStart(2, "0")}:00\n${usd(v)}`, d.hNow),
  );
  const pie = $derived(pieGeom(d.parts, tok, pct));
  const C = RING.S / 2;
</script>

<div class="w-tiles">
  <div class="w-tile">
    <div class="w-k">Last 7 days</div>
    <div class="w-big num">{usd(d.weekSum)}</div>
    <svg class="w-svg" viewBox="0 0 150 26" width="150" height="26" role="img" aria-label={weekLabel}>
      {#if spark.dot}
        <defs>
          <linearGradient id={spark.id} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" class="w-stop-a" />
            <stop offset="1" class="w-stop-b" />
          </linearGradient>
        </defs>
        <polygon points={spark.area} fill={`url(#${spark.id})`} />
        <polyline points={spark.line} class="w-line" />
        {#each spark.hits as hit, i (i)}<rect
            x={hit.x}
            y="0"
            width={hit.width}
            height="26"
            class="w-hit"
            data-tip={hit.tip}
          />{/each}
        <circle cx={spark.dot.cx} cy={spark.dot.cy} r="3" class="w-dot" />
      {/if}
    </svg>
  </div>
  <div class="w-tile">
    <div class="w-k">Today by hour</div>
    <div class="w-big num">{usd(d.todayCost)}</div>
    <svg
      class="w-svg"
      viewBox="0 0 150 26"
      width="150"
      height="26"
      role="img"
      aria-label="Spend per hour today"
    >
      {#each bars as b, i (i)}
        <rect
          x={b.x}
          y={b.y}
          width={b.width}
          height={b.height}
          rx="1"
          class={"w-bar" + (b.now ? " now" : "")}
        />
        <rect x={b.hx} y="0" width={b.hw} height="26" class="w-hit" data-tip={b.tip} />
      {/each}
      <line x1="0" x2="150" y1="33.5" y2="33.5" class="w-base" />
    </svg>
  </div>
</div>
<div class="w-burn num">
  <span><span class="w-k">Per hour</span> <b>{usd(d.perHour)}</b></span>
  <span><span class="w-k">By midnight</span> <b>{usd(d.projected)}</b></span>
</div>
<div class="w-stats">
  <div class="w-stat">
    <svg class="w-svg" viewBox="0 0 64 64" width="54" height="54" role="img" aria-label="Context used">
      <circle cx={C} cy={C} r={RING.R} class="w-track" />
      <circle
        cx={C}
        cy={C}
        r={RING.R}
        class={"w-arc " + d.ctx.lvl}
        stroke-dasharray={ringDash(d.ctx.frac)}
        transform={`rotate(-90 ${C} ${C})`}
      />
      <text x={C} y={C + 4.5} class="w-centre" text-anchor="middle">{d.ctx.centre}</text>
      <circle cx={C} cy={C} r={RING.R + 5} class="w-hit" data-tip={d.ctx.tip} />
    </svg>
    <div class="w-k">Context</div>
  </div>
  <div class="w-stat">
    <svg class="w-svg" viewBox="0 0 64 64" width="54" height="54" role="img" aria-label="Startup load">
      <circle cx={C} cy={C} r={RING.R} class="w-track" />
      <circle
        cx={C}
        cy={C}
        r={RING.R}
        class={"w-arc " + d.startup.lvl}
        stroke-dasharray={ringDash(d.startup.frac)}
        transform={`rotate(-90 ${C} ${C})`}
      />
      <text x={C} y={C + 4.5} class="w-centre" text-anchor="middle">{d.startup.centre}</text>
      <circle cx={C} cy={C} r={RING.R + 5} class="w-hit" data-tip={d.startup.tip} />
    </svg>
    <div class="w-k">Startup</div>
  </div>
  <div class="w-stat">
    <svg
      class="w-svg"
      viewBox="0 0 64 64"
      width="54"
      height="54"
      role="img"
      aria-label="Tokens by model"
    >
      {#if pie.empty}
        <circle cx={pie.c} cy={pie.c} r={pie.R} class="w-track" />
      {:else}
        {#each pie.slices as sl, i (i)}
          {#if sl.d}<path d={sl.d} class="w-slice" style:fill={sl.color} data-tip={sl.tip} />
          {:else}<circle
              cx={pie.c}
              cy={pie.c}
              r={pie.R}
              class="w-slice"
              style:fill={sl.color}
              data-tip={sl.tip}
            />{/if}
        {/each}
      {/if}
    </svg>
    <div class="w-k">Models</div>
  </div>
</div>
<div class="w-stats num">
  <div title={unit.na ? null : unit.tip}><b class={unit.na ? "muted" : unit.cls.trim()}>{unit.na ? "-" : unit.text}</b><span class="w-k">per 1k out</span></div>
  {#each co2 as l, i (i)}{#if l}{#each l.stats as st}<div title={l.tip ?? null}><b>{st.v.replace("~", "")}</b><span class="w-k">{st.k}</span></div>{/each}{/if}{/each}
</div>
{#if d.waste}
  <div class="w-kv w-waste" title={d.waste.cause.name + ": " + d.waste.fix}>
    <span class="w-k"><span class="w-sw" style:background={causeColor(d.waste.cause)}></span>{d.waste.cause.name}</span>
    <b class="num">{d.waste.usd != null ? usd(d.waste.usd) + "/wk" : tok(d.waste.tokens) + "/wk"}</b>
    {#if d.waste.act}<span class="act">{d.waste.act}</span>{/if}
  </div>
{/if}
