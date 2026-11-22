<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import Card from "../../lib/components/Card.svelte";
  import Bar from "../../lib/components/Bar.svelte";
  import Num from "../../lib/components/Num.svelte";
  import Icon from "../../lib/components/Icon.svelte";
  import Table, { type Col } from "../../lib/components/Table.svelte";
  import { cut, type Ctx } from "../../lib/ctx";
  import { exact, int, pct, q, tok } from "../../lib/format";
  import AreaCard from "./AreaCard.svelte";
  import Na from "./Na.svelte";
  import { naMissing } from "./shared";

  let { ctx }: { ctx: Ctx } = $props();
  const ui = $derived(ctx.ui);

  function toolCats() {
    const byCat = new Map<string, any>();
    for (const r of (ctx.s.tools || []).filter(ctx.inProject)) {
      const c = byCat.get(r.category) || {
        category: r.category,
        calls: 0,
        first: 0,
        carried: 0,
        share: 0,
        est: false,
        rows: [],
      };
      c.calls += r.calls;
      c.first += q(r.first_tokens) || 0;
      c.carried += q(r.carried_tokens) || 0;
      c.share += r.share || 0;
      c.est = c.est || !!(r.first_tokens && r.first_tokens.est);
      c.rows.push(r);
      byCat.set(r.category, c);
    }
    for (const c of byCat.values())
      c.rows.sort((a: any, b: any) => (q(b.carried_tokens) || 0) - (q(a.carried_tokens) || 0));
    return [...byCat.values()].sort((a, b) => b.carried - a.carried);
  }
  function catHint(c: any): string | null {
    const d = c.rows
      .map((r: any) => r.detail)
      .filter(Boolean)
      .slice(0, 4);
    if (c.category === "other")
      return d.length ? "commands with no category: " + d.join(", ") : "commands with no category";
    return null;
  }
  const cats = $derived(toolCats());
  const tot = $derived(cats.reduce((a, c) => a + c.carried, 0) || 1);
  const max = $derived(cats.length ? cats[0].carried : 1);
  const rows = $derived.by(() => {
    const out: any[] = [];
    for (const c of cats) {
      out.push(c);
      const dets: any[] = c.rows.filter((x: any) => x.detail);
      if (ui.openCat === c.category && dets.length) {
        const key = "det-" + c.category;
        for (const r of cut(ctx, key, dets, 10)) {
          out.push({
            __detail: true,
            category: r.detail,
            calls: r.calls,
            first: q(r.first_tokens) || 0,
            carried: q(r.carried_tokens) || 0,
            est: !!(r.first_tokens && r.first_tokens.est),
          });
        }
        if (dets.length > 10)
          out.push({
            __detail: true,
            __more: key,
            category: ctx.open(key) ? "Show fewer" : `Show all ${dets.length}`,
          });
      }
    }
    return out;
  });
  const e = (r: any) => (r.est ? "~" : "");
  const hasDet = (r: any) => r.rows.some((x: any) => x.detail);

  const cols = $derived.by<Col[]>(() => [
    { h: "Category", name: true, cell: category },
    { h: "Calls", n: true, cell: calls },
    { h: "First read", n: true, cell: first },
    { h: "Carried", n: true, cell: carried },
    { h: "Share", w: "barcol", cell: share },
  ]);
</script>

{#snippet category(r: any)}
  {#if r.__more}<button
      class="more inline"
      onclick={(ev) => {
        ev.stopPropagation();
        ui.on.toggle(r.__more);
      }}
    >
      {r.category}
    </button>
  {:else if r.__detail}<span class="mono">{r.category}</span>
  {:else}<span class="cat">
      {#if hasDet(r)}<Icon name="chev" />{:else}<span class="chev-pad"></span>{/if}
      <span>{r.category}</span>
      {#if catHint(r)}<span class="hint">{catHint(r)}</span>{/if}
    </span>{/if}
{/snippet}
{#snippet calls(r: any)}{#if !r.__more}<Num
      text={int(r.calls)}
      tip={exact(r.calls) + " calls"}
    />{/if}{/snippet}
{#snippet first(r: any)}{#if !r.__more}<Num
      text={e(r) + tok(r.first)}
      tip={exact(r.first) + " tokens when first returned"}
    />{/if}{/snippet}
{#snippet carried(r: any)}{#if !r.__more}<Num
      text={e(r) + tok(r.carried)}
      tip={exact(r.carried) + " tokens carried in later turns"}
    />{/if}{/snippet}
{#snippet share(r: any)}{#if !r.__more}<span class="sharecell">
      <Bar frac={r.carried / max} tip={exact(r.carried) + " tokens"} cls={r.__detail ? "soft" : ""} />
      <span class="num">{pct(r.carried / tot)}</span>
    </span>{/if}{/snippet}

<div class="grid">
  <AreaCard {ctx} area="tools" />
  {#if naMissing(ctx, "tool_outputs")}
    <Na {ctx} cap="tool_outputs" cls="span-12" />
  {:else}
    <Card title="Tool output" note="~ estimated from output length" cls="span-12 h-tools">
      <Table
        {cols}
        {rows}
        opts={{
          pick: (r) => r.rows && hasDet(r) && ui.on.cat(r.category),
          sel: (r) => !r.__detail && r.category === ui.openCat,
          expanded: (r) => (!r.__detail && hasDet(r) ? ui.openCat === r.category : null),
          empty: "No tool call in this window.",
          cls: "tools",
        }}
      />
    </Card>
  {/if}
</div>
