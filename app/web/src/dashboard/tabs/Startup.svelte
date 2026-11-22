<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import Card from "../../lib/components/Card.svelte";
  import Empty from "../../lib/components/Empty.svelte";
  import Bar from "../../lib/components/Bar.svelte";
  import Num from "../../lib/components/Num.svelte";
  import More from "../../lib/components/More.svelte";
  import Slot from "../../lib/components/Slot.svelte";
  import Table, { type Col } from "../../lib/components/Table.svelte";
  import { donut } from "../../lib/charts/chart";
  import { KIND, SERIES, cut, type Ctx } from "../../lib/ctx";
  import { exact, int, path, pct, q, tok } from "../../lib/format";
  import AreaCard from "./AreaCard.svelte";
  import Na from "./Na.svelte";
  import { naMissing } from "./shared";

  let { ctx }: { ctx: Ctx } = $props();
  const ui = $derived(ctx.ui);

  const rows = $derived(
    (ctx.s.startup || [])
      .filter(ctx.inProject)
      .slice()
      .sort((a: any, b: any) => b.median - a.median),
  );
  const sel = $derived(rows.find((r: any) => r.project === ui.selStartup) || rows[0]);

  function startupKinds(sel: any) {
    const m = new Map<string, any>();
    const add = (kind: string, name: string, v: number) => {
      const k = m.get(kind) || { kind, value: 0, parts: [] };
      k.value += v;
      k.parts.push({ name, value: v });
      m.set(kind, k);
    };
    for (const p of sel.parts) add(p.kind, p.name, q(p.tokens) || 0);
    if ((q(sel.unattributed) || 0) > 0) add("unattributed", "not traced to a file", q(sel.unattributed)!);
    const kinds = [...m.values()].sort((a, b) => b.value - a.value);
    kinds.forEach((k, i) => {
      k.color = SERIES(i);
      k.parts.sort((a: any, b: any) => b.value - a.value);
    });
    return kinds;
  }
  const kinds = $derived(sel ? startupKinds(sel) : []);
  const total = $derived(kinds.reduce((a, k) => a + k.value, 0) || 1);
  const kind = $derived(kinds.find((k) => k.kind === ui.selKind) || kinds[0]);
  const pkey = $derived("parts-" + (kind ? kind.kind : ""));
  const parts = $derived(kind ? cut<any>(ctx, pkey, kind.parts as any[], 12) : []);

  const cols = $derived.by<Col[]>(() => [
    { h: "Project", name: true, get: (r) => r.project },
    { h: "Sessions", n: true, get: (r) => int(r.sessions) },
    { h: "Median", n: true, cell: median },
    { h: "", w: "barcol", cell: medianBar },
    { h: "p90", n: true, cell: p90 },
    { h: "Min", n: true, cell: min },
    { h: "Max", n: true, cell: max },
  ]);
</script>

{#snippet median(r: any)}<Num text={tok(r.median)} tip={exact(r.median)} />{/snippet}
{#snippet medianBar(r: any)}<Bar
    frac={r.median / rows[0].median}
    tip={exact(r.median) + " tokens"}
  />{/snippet}
{#snippet p90(r: any)}<Num text={tok(r.p90)} tip={exact(r.p90)} />{/snippet}
{#snippet min(r: any)}<Num text={tok(r.min)} tip={exact(r.min)} />{/snippet}
{#snippet max(r: any)}<Num text={tok(r.max)} tip={exact(r.max)} />{/snippet}

<div class="grid">
  <AreaCard {ctx} area="startup" />
  {#if naMissing(ctx, "startup")}
    <Na {ctx} cap="startup" cls="span-12" />
  {:else if !sel}
    <Card title="Startup load" cls="span-12 h-su">
      <Empty text="No session start recorded in this window." />
    </Card>
  {:else}
    <section class="card h-su span-12">
      <div class="card-head">
        <h2>Startup load</h2>
        <div class="head-ctl">
          <span class="card-note">composition of</span>
          <span class="sel-wrap">
            <select
              class="select"
              aria-label="Project"
              onchange={(e) => ui.on.startup(e.currentTarget.value)}
            >
              {#each rows as r (r.provider + "|" + r.project)}<option value={r.project} selected={r === sel}>
                  {r.project}
                </option>{/each}
            </select>
          </span>
        </div>
      </div>
      <div class="composition">
        <div class="donut-col">
          <Slot
            cls="donut"
            draw={() =>
              donut(
                kinds.map((k) => ({ label: KIND[k.kind] || k.kind, value: k.value, color: k.color })),
                "Startup load of " + sel.project + ", by kind",
                184,
                tok(total),
                "tokens at start",
              )}
          />
        </div>
        <ul class="kinds" role="listbox" aria-label="Kinds of startup content">
          {#each kinds as k (k.kind)}<li>
              <button
                class={"kind" + (k === kind ? " on" : "")}
                role="option"
                aria-selected={k === kind}
                onclick={() => ui.on.kind(k.kind)}
                title={exact(k.value) + " tokens in " + k.parts.length + " parts"}
              >
                <span class="swatch" style:background={k.color}></span>
                <span class="k-name">{KIND[k.kind] || k.kind}</span>
                <span class="k-count num">{k.parts.length}</span>
                <span class="k-share num">{pct(k.value / total)}</span>
                <span class="k-val num">{tok(k.value)}</span>
              </button>
            </li>{/each}
        </ul>
        <div class="parts">
          <div class="parts-head">
            <h3>{KIND[kind.kind] || kind.kind}</h3>
            <span class="card-note">
              {kind.parts.length +
                (kind.parts.length === 1 ? " part, " : " parts, ") +
                tok(kind.value) +
                " tokens"}
            </span>
          </div>
          <ul class="mini" data-sk={pkey + "|" + sel.project}>
            {#each parts as p}<li>
                <span class="m-name" title={path(p.name, 400)}>{path(p.name, 52)}</span>
                <Bar frac={p.value / kind.parts[0].value} tip={exact(p.value) + " tokens"} />
                <span class="m-val num" title={exact(p.value) + " tokens"}>{tok(p.value)}</span>
              </li>{/each}
          </ul>
          <More {ctx} key={pkey} total={kind.parts.length} shown={12} noun="parts" />
        </div>
      </div>
    </section>
    <Card title="By project" note="tokens at session start" cls="span-12 h-tab">
      <Table {cols} {rows} opts={{ pick: (r) => ui.on.startup(r.project), sel: (r) => r === sel }} />
    </Card>
  {/if}
</div>
