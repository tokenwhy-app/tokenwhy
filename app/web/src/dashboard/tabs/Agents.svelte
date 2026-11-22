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
  import Icon from "../../lib/components/Icon.svelte";
  import More from "../../lib/components/More.svelte";
  import Slot from "../../lib/components/Slot.svelte";
  import DataTable from "../../lib/components/DataTable.svelte";
  import Table, { type Col } from "../../lib/components/Table.svelte";
  import { treemap } from "../../lib/charts/chart";
  import { SERIES, cut, type Ctx } from "../../lib/ctx";
  import { int, model, pct, q, usd } from "../../lib/format";
  import AreaCard from "./AreaCard.svelte";
  import Na from "./Na.svelte";
  import { naMissing } from "./shared";

  let { ctx }: { ctx: Ctx } = $props();
  const ui = $derived(ctx.ui);

  function agentTypes() {
    const m = new Map<string, any>();
    for (const r of (ctx.s.subagents || []).filter(ctx.inProject)) {
      const k = r.agent_type || "(untyped)";
      const t = m.get(k) || { type: k, agents: 0, cost: 0, models: new Set<string>(), rows: [] };
      t.agents += r.agents;
      t.cost += q(r.cost_usd) || 0;
      if (r.model) t.models.add(model(r.model));
      t.rows.push(r);
      m.set(k, t);
    }
    for (const t of m.values()) t.rows.sort((a: any, b: any) => (q(b.cost_usd) || 0) - (q(a.cost_usd) || 0));
    return [...m.values()].sort((a, b) => b.cost - a.cost);
  }
  const types = $derived(agentTypes());
  const tot = $derived(types.reduce((a, t) => a + t.cost, 0) || 1);
  const max = $derived(types.length ? types[0].cost : 1);
  const rows = $derived.by(() => {
    const out: any[] = [];
    for (const t of cut(ctx, "types", types, 10)) {
      out.push(t);
      if (ctx.open("type-" + t.type))
        for (const r of t.rows)
          out.push({
            __detail: true,
            type: r.project,
            models: new Set([model(r.model)]),
            agents: r.agents,
            cost: q(r.cost_usd) || 0,
            median: q(r.median_cost_usd),
            share: r.cost_share,
          });
    }
    return out;
  });
  const proj = $derived.by(() => {
    const byProj = new Map<string, number>();
    for (const r of (ctx.s.subagents || []).filter(ctx.inProject))
      byProj.set(r.project, (byProj.get(r.project) || 0) + (r.cost_share || 0));
    return [...byProj.entries()].sort((a, b) => b[1] - a[1]);
  });

  const groups = $derived.by(() => {
    const m = new Map<string, any>();
    for (const r of (ctx.s.subagents || []).filter(ctx.inProject)) {
      const c = q(r.cost_usd) || 0;
      if (c <= 0) continue;
      const p = m.get(r.project) || { name: r.project, value: 0, agents: 0, kids: new Map<string, any>() };
      const k = r.agent_type || "(untyped)",
        t = p.kids.get(k) || { name: k, value: 0, agents: 0, models: new Set<string>() };
      p.value += c;
      p.agents += r.agents;
      t.value += c;
      t.agents += r.agents;
      if (r.model) t.models.add(model(r.model));
      p.kids.set(k, t);
      m.set(r.project, p);
    }
    const gs = [...m.values()].sort((a, b) => b.value - a.value);
    const total = gs.reduce((a, g) => a + g.value, 0);
    gs.forEach((g, i) => {
      g.color = SERIES(i);
      g.value_text = usd(g.value);
      g.tip = `${usd(g.value)} · ${pct(g.value / total)} of subagent cost\n${g.agents} agents`;
      g.children = [...g.kids.values()]
        .sort((a: any, b: any) => b.value - a.value)
        .map((k: any) => ({
          ...k,
          value_text: usd(k.value),
          tip: `${usd(k.value)} · ${pct(k.value / g.value)} of ${g.name}\n${k.agents} agents · ${[...k.models].join(", ") || "model unknown"}`,
        }));
    });
    return gs;
  });
  const mapRows = $derived(
    groups.flatMap((g) => g.children.map((k: any) => [g.name + " · " + k.name, usd(k.value), int(k.agents)])),
  );

  const cols = $derived.by<Col[]>(() => [
    { h: "Type", name: true, cell: type },
    { h: "Model", get: (r) => [...r.models].join(", ") || "-" },
    { h: "Agents", n: true, get: (r) => int(r.agents) },
    { h: "Cost", n: true, cell: cost },
    { h: "Share of subagent cost", w: "barcol", cell: share },
  ]);
</script>

{#snippet type(r: any)}
  {#if r.__detail}{r.type}{:else}<span class="cat">
      <Icon name="chev" />
      <span>{r.type}</span>
      <span class="hint">{r.rows.length + (r.rows.length === 1 ? " project" : " projects")}</span>
    </span>{/if}
{/snippet}
{#snippet cost(r: any)}<Num text={usd(r.cost)} tip={"$" + r.cost.toFixed(2)} />{/snippet}
{#snippet share(r: any)}<span class="sharecell">
    <Bar frac={r.cost / max} tip={"$" + r.cost.toFixed(2)} cls={r.__detail ? "soft" : ""} />
    <span class="num">{pct(r.cost / tot)}</span>
  </span>{/snippet}

<div class="grid">
  <AreaCard {ctx} area="agents" />
  {#if naMissing(ctx, "subagents")}
    <Na {ctx} cap="subagents" cls="span-12" />
  {:else}
    {#if groups.length}
      <Card title="Where subagent money goes" note="project, then agent type" cls="span-12 h-money">
        <Slot
          draw={(w) =>
            treemap(
              groups,
              w,
              groups.reduce((n, g) => n + (g.children?.length || 0), 0) < 3 ? 170 : 320,
              "Subagent cost by project and agent type",
            )}
        />
        <DataTable
          caption="Subagent cost by project and agent type"
          cols={["Project · type", "Cost", "Agents"]}
          rows={mapRows}
        />
      </Card>
    {/if}
    <Card title="By agent type" note="click a type for its projects" cls="span-8 h-ag">
      <Table
        {cols}
        {rows}
        opts={{
          pick: (r) => ui.on.toggle("type-" + r.type),
          sel: (r) => !r.__detail && ctx.open("type-" + r.type),
          expanded: (r) => (r.__detail ? null : ctx.open("type-" + r.type)),
          empty: "No subagent ran in this window.",
          cls: "tools",
        }}
      />
      <More {ctx} key="types" total={types.length} shown={10} noun="types" />
    </Card>
    <Card title="Share of project cost" note="spent in subagents" cls="span-4 h-ag">
      {#if proj.length}
        <ul class="mini">
          {#each proj as [p, v] (p)}<li>
              <span class="m-name" title={p}>{p}</span>
              <Bar frac={v} tip={pct(v) + " of " + p + " cost went to subagents"} />
              <span class="m-val num">{pct(v)}</span>
            </li>{/each}
        </ul>
      {:else}
        <Empty text="No subagent ran in this window." />
      {/if}
    </Card>
  {/if}
</div>
