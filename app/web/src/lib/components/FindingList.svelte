<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import Icon from "./Icon.svelte";
  import More from "./More.svelte";
  import { RULES, cut, type Ctx } from "../ctx";
  import { exact, q, tok, usd } from "../format";
  import { causeColor, causeOf, dollars, rate, saved, say } from "../diagnose";
  import { co2, co2Tip } from "../../features/co2/format";
  import type { Finding } from "../types";
  import { minorTest } from "../minor";

  let { ctx, rows, n, key }: { ctx: Ctx; rows: Finding[]; n: number; key: string } = $props();

  const money = (u: number | null, t: number) => (u != null ? usd(u) : tok(t));
  const r = $derived(rate(ctx.s));
  const w = (f: Finding) => {
    const u = dollars(f, r);
    return u != null ? u : saved(f);
  };
  const minor = $derived(minorTest(ctx));
  const main = $derived(rows.filter((f) => !minor(f)));
  const low = $derived(rows.filter(minor));
  const max = $derived(Math.max(...rows.map(w), 1e-9));
  const shown = $derived(cut(ctx, key, main, n));
  const mkey = $derived(key + "-min");
  const lowOpen = $derived(ctx.open(mkey));
  const pfx = $derived(key + ":");
  const idOf = (f: Finding) =>
    pfx + f.rule + "|" + (f.provider || "") + "|" + (f.project || "") + "|" + (f.line || "");
  function flip(id: string) {
    for (const x of [...ctx.ui.open]) if (x.startsWith(pfx) && x !== id) ctx.ui.open.delete(x);
    ctx.ui.on.toggle(id);
  }
  const onKey = (ev: KeyboardEvent, id: string) => {
    if (ev.key === "Enter" || ev.key === " ") {
      ev.preventDefault();
      flip(id);
    }
  };
</script>

{#snippet items(list: Finding[])}
  <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
  <ol class="findings">
    {#each list as f (idOf(f))}{@const t = say(f)}{@const id = idOf(f)}{@const open = ctx.open(id)}{@const u =
        dollars(f, r)}
      <li
        tabindex="0"
        role="button"
        aria-expanded={open}
        class={open ? "open" : ""}
        onclick={() => flip(id)}
        onkeydown={(ev) => onKey(ev, id)}
      >
        <div class="f-body">
          <div class="f-title">
            <Icon name="chev" />
            <span>{t.fact}</span>
          </div>
          {#if t.fix}<p>{t.fix}</p>{/if}
        </div>
        {#if u != null ? u >= 0.005 : saved(f) >= 1}<span class="f-saved num">
            {money(u, saved(f))}<small>{u != null ? " /wk" : " tok/wk"}</small>
          </span>{:else}<span class="f-saved"></span>{/if}
        <span class="f-meter">
          <i
            style:width={Math.max(1.5, (100 * w(f)) / max) + "%"}
            style:background={causeColor(causeOf(f.rule))}
          ></i>
        </span>
        {#if open}<dl class="f-detail">
            <div>
              <dt>Rule</dt>
              <dd>{(RULES[f.rule] || f.rule) + " (" + f.rule + ")"}</dd>
            </div>
            {#if t.more}<div>
                <dt>Detail</dt>
                <dd>{t.more}</dd>
              </div>{/if}{#if f.project}<div>
                <dt>Project</dt>
                <dd>{f.project}</dd>
              </div>{/if}
            <div>
              <dt>Tokens a week</dt>
              <dd class="num">
                {exact(saved(f)) +
                  (q(f.tokens_saved_per_week) && q((f.tokens_saved_per_week as any).est)
                    ? " (estimate)"
                    : "")}
              </dd>
            </div>
            {#if u != null}<div>
                <dt>Dollars a week</dt>
                <dd class="num">
                  {"$" + u.toFixed(2) + ", each token priced as what it is (cache read, cache write, input)"}
                </dd>
              </div>{/if}{#if q(f.co2_g_saved_per_week) != null && ctx.s.co2}<div
                data-tip={co2Tip(ctx.s.co2)}
              >
                <dt>CO2e a week</dt>
                <dd class="num">{co2(q(f.co2_g_saved_per_week)) + " CO2e, estimate"}</dd>
              </div>{/if}
          </dl>{/if}
      </li>{/each}
  </ol>
{/snippet}

{#if main.length}
  {@render items(shown)}
  <More {ctx} {key} total={main.length} shown={n} noun="findings" />
  {#if low.length}
    <button class="more" aria-expanded={lowOpen} onclick={() => ctx.ui.on.toggle(mkey)}>
      {lowOpen ? "Hide minor optimizations" : `+${low.length} minor optimizations`}
    </button>
    {#if lowOpen}{@render items(low)}{/if}
  {/if}
{:else if low.length}
  <p class="f-good">Nothing to optimize here.</p>
  <p class="f-good-sub">Already in good shape.</p>
  <button class="more" aria-expanded={lowOpen} onclick={() => ctx.ui.on.toggle(mkey)}>
    {lowOpen ? "Hide minor optimizations" : `Show ${low.length} minor optimizations`}
  </button>
  {#if lowOpen}{@render items(low)}{/if}
{/if}

<style>
  .f-good {
    margin: 0;
    font-weight: 500;
  }
  .f-good-sub {
    margin: var(--s1, 4px) 0 0;
    color: var(--muted);
    font-size: var(--fs-s);
  }
</style>
