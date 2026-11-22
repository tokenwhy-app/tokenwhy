<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import Card from "../../lib/components/Card.svelte";
  import Bar from "../../lib/components/Bar.svelte";
  import Num from "../../lib/components/Num.svelte";
  import Slot from "../../lib/components/Slot.svelte";
  import DataTable from "../../lib/components/DataTable.svelte";
  import Table, { type Col } from "../../lib/components/Table.svelte";
  import { costColor, costRadius, scatter } from "../../lib/charts/chart";
  import type { Ctx } from "../../lib/ctx";
  import { exact, int, pct, q, qfmt, tok, usd } from "../../lib/format";
  import AreaCard from "./AreaCard.svelte";
  import Na from "./Na.svelte";
  import { naMissing, provOf, when } from "./shared";

  let { ctx }: { ctx: Ctx } = $props();

  // Core R17 thresholds (model.rs Thresholds: segment_turns, segment_mean_ctx).
  const LONG = { x: 100, y: 450000 };
  const ss = $derived(
    (ctx.s.sessions || []).filter(ctx.inProject).filter((x: any) => x.turns > 0 && x.mean_ctx > 0),
  );
  const map = $derived.by(() => {
    if (ss.length < 3) return null;
    const pts = ss.map((x: any) => {
      const c = q(x.cost_usd) || 0;
      const compactions = x.compactions
        ? " · " + x.compactions + " compaction" + (x.compactions > 1 ? "s" : "")
        : "";
      return {
        x: x.turns,
        y: x.mean_ctx,
        size: c,
        label: x.project + " " + usd(c),
        tip: `${x.project} · ${when(x.start_ms)}${provOf(ctx, x)}\n${int(x.turns)} turns · ${usd(c)}${compactions}`,
      };
    });
    const maxCost = Math.max(...pts.map((p) => p.size), 1e-9);
    const hot = pts.filter((p) => p.x >= LONG.x && p.y >= LONG.y);
    const hotCost = hot.reduce((a, p) => a + p.size, 0),
      all = pts.reduce((a, p) => a + p.size, 0) || 1;
    return {
      pts,
      maxCost,
      note: hot.length
        ? `${hot.length} long topics, ${pct(hotCost / all)} of session cost`
        : "none in the long-topic zone",
    };
  });
  const sessRows = $derived(
    ss
      .slice()
      .sort((a: any, b: any) => (q(b.cost_usd) || 0) - (q(a.cost_usd) || 0))
      .slice(0, 30)
      .map((x: any) => [
        x.project + " " + when(x.start_ms),
        int(x.turns),
        tok(x.mean_ctx),
        usd(q(x.cost_usd) || 0),
      ]),
  );

  const rows = $derived(
    (ctx.s.compactions || [])
      .filter(ctx.inProject)
      .slice()
      .sort((a: any, b: any) => b.ts_ms - a.ts_ms),
  );
  const max = $derived(Math.max(...rows.map((r: any) => r.pre_tokens), 1));

  const cols = $derived.by<Col[]>(() => [
    { h: "Session", name: true, cell: session },
    { h: "Before", n: true, cell: before },
    { h: "After", n: true, cell: after },
    { h: "", w: "barcol wide", cell: pair },
    { h: "Rebuild", n: true, get: (r) => qfmt(r.rebuild_tokens, tok) },
    {
      h: "Cost per turn",
      n: true,
      get: (r) => qfmt(r.cost_per_turn_before, usd) + " → " + qfmt(r.cost_per_turn_after, usd),
    },
  ]);
</script>

{#snippet session(r: any)}<span class="sess">
    <span>{r.project}</span>
    <span class="hint" title={r.session}>
      {when(r.ts_ms) + " · " + String(r.session).slice(0, 8) + provOf(ctx, r)}
    </span>
  </span>{/snippet}
{#snippet before(r: any)}<Num text={tok(r.pre_tokens)} tip={exact(r.pre_tokens) + " tokens"} />{/snippet}
{#snippet after(r: any)}<Num
    text={tok(r.first_after_ctx)}
    tip={exact(r.first_after_ctx) + " tokens"}
  />{/snippet}
{#snippet pair(r: any)}<span
    class="pair"
    data-tip={exact(r.pre_tokens) + " before, " + exact(r.first_after_ctx) + " after"}
  >
    <Bar frac={r.pre_tokens / max} cls="s1" /><Bar frac={r.first_after_ctx / max} cls="s2" />
  </span>{/snippet}

<div class="grid">
  <AreaCard {ctx} area="context" />
  {#if map}
    <Card title="Sessions" note={map.note} cls="span-12 h-sess">
      <Slot
        draw={(w) =>
          scatter(map.pts, w, 300, "Sessions by turns and mean context, bubble size is cost", LONG, tok)}
      />
      <div class="sc-key">
        <span>Cost, size and colour</span>
        {#each [0, 0.25, 0.6, 1] as f}<span class="sc-item">
            <i
              style:width={2 * costRadius(f) + "px"}
              style:height={2 * costRadius(f) + "px"}
              style:background={costColor(f)}
            ></i>{usd(f * map.maxCost)}
          </span>{/each}
        <span class="sc-note">Top right: many turns at a large context, the sessions to split.</span>
      </div>
      <DataTable
        caption="Sessions by cost"
        cols={["Session", "Turns", "Mean context", "Cost"]}
        rows={sessRows}
      />
    </Card>
  {/if}
  {#if naMissing(ctx, "compactions")}
    <Na {ctx} cap="compactions" cls="span-12" />
  {:else}
    <Card title="Compactions" note="context before and just after" cls="span-12 h-comp">
      {#if rows.length}<div class="key">
          <span>
            <span class="swatch" style:background="var(--series-1)"></span>before
          </span>
          <span>
            <span class="swatch" style:background="var(--series-2)"></span>first turn after
          </span>
        </div>{/if}
      <Table {cols} {rows} opts={{ empty: "No compaction in this window." }} />
    </Card>
  {/if}
</div>
