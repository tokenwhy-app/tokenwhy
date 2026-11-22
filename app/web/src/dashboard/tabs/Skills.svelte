<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import Card from "../../lib/components/Card.svelte";
  import Empty from "../../lib/components/Empty.svelte";
  import Icon from "../../lib/components/Icon.svelte";
  import Table, { type Col } from "../../lib/components/Table.svelte";
  import type { Ctx } from "../../lib/ctx";
  import { bytes, exact, int, path, q, qfmt, tok, usd } from "../../lib/format";
  import AreaCard from "./AreaCard.svelte";
  import Na from "./Na.svelte";
  import { naMissing } from "./shared";

  let { ctx }: { ctx: Ctx } = $props();
  const ui = $derived(ctx.ui);

  const all = $derived(ctx.s.skills || []);
  const carried = (k: any) => q(k.carried_cost_usd) || 0;
  const used = $derived(
    all
      .filter((k: any) => k.activations > 0)
      .sort((a: any, b: any) => carried(b) - carried(a) || b.activations - a.activations),
  );
  const unused = $derived(
    all
      .filter((k: any) => !k.activations)
      .sort((a: any, b: any) => (q(b.listing_tokens_median) || 0) - (q(a.listing_tokens_median) || 0)),
  );
  const unusedTok = $derived(unused.reduce((a: number, k: any) => a + (q(k.listing_tokens_median) || 0), 0));
  const skillCols: Col[] = [
    { h: "Skill", name: true, get: (r) => r.skill },
    { h: "Uses", n: true, get: (r) => int(r.activations) },
    { h: "Listing", n: true, get: (r) => qfmt(r.listing_tokens_median, tok) },
    { h: "Body", n: true, get: (r) => (r.activations ? qfmt(r.body_tokens_median, tok) : "-") },
    { h: "Carried", n: true, get: (r) => qfmt(r.carried_cost_usd, usd) },
  ];

  const mem = $derived(ctx.s.memory || []);
  const sized = $derived(
    mem
      .filter((m: any) => m.bytes > 0 || (q(m.tokens) || 0) > 0)
      .sort((a: any, b: any) => (q(b.tokens) || 0) - (q(a.tokens) || 0)),
  );
  const zero = $derived(mem.length - sized.length);
  const memRows = $derived(
    ctx.open("mem-zero") ? sized.concat(mem.filter((m: any) => !sized.includes(m))) : sized,
  );
  const memCols = $derived.by<Col[]>(() => [
    { h: "File", name: true, cell: file },
    { h: "Size", n: true, get: (r) => bytes(r.bytes) },
    { h: "Tokens", n: true, get: (r) => qfmt(r.tokens, tok) },
    { h: "Carried", n: true, get: (r) => qfmt(r.carried_cost_usd, usd) },
  ]);
</script>

{#snippet file(r: any)}<span title={path(r.path, 400)}>{r.label || path(r.path, 46)}</span>{/snippet}

<div class="grid">
  <AreaCard {ctx} area="skills" />
  {#if naMissing(ctx, "skills")}
    <Na {ctx} cap="skills" cls="span-6" />
  {:else}
    <Card title="Skills" note="sorted by cost carried" cls="span-6 h-skill">
      {#if used.length}<Table cols={skillCols} rows={used} />{:else}<Empty
          text="No skill was used in this window."
        />{/if}
      {#if unused.length}<button
          class="summary-row"
          aria-expanded={ctx.open("unused")}
          onclick={() => ui.on.toggle("unused")}
        >
          <Icon name="chev" />
          <span>
            {`${unused.length} ${unused.length === 1 ? "skill" : "skills"} never used. Disable them to save ~${tok(unusedTok)} tokens a session.`}
          </span>
        </button>{/if}
      {#if ctx.open("unused")}<Table cols={skillCols} rows={unused} />{/if}
    </Card>
  {/if}
  {#if naMissing(ctx, "memory")}
    <Na {ctx} cap="memory" cls="span-6" />
  {:else}
    <Card title="Memory files" note="loaded into every session" cls="span-6 h-skill">
      <Table cols={memCols} rows={memRows} opts={{ empty: "No memory file loaded in this window." }} />
      {#if zero}<button
          class="more"
          aria-expanded={ctx.open("mem-zero")}
          onclick={() => ui.on.toggle("mem-zero")}
        >
          {ctx.open("mem-zero")
            ? "Hide files with no size"
            : `${zero} files read on demand with no size, hidden`}
        </button>{/if}
    </Card>
  {/if}
</div>
