<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import { onMount, tick } from "svelte";
  import { wrappedCards, wrappedSave } from "../../lib/bridge";
  import type { WrappedArgs, WrappedCard } from "../../lib/types";
  import Seg from "../../lib/components/Seg.svelte";
  import { openSources } from "../sources/store.svelte";
  import { svgNode, arrive, fitBig } from "./cards";
  import { look } from "../../lib/stores/look.svelte";
  import { palette } from "./palette";
  import { loadFonts } from "./open";

  let { opener, onclose }: { opener: HTMLElement | null; onclose: () => void } = $props();

  const PROVIDERS: [string, string][] = [
    ["claude-code", "Claude Code"],
    ["", "All agents"],
  ];
  const today = () => new Date();
  const iso = (d: Date) =>
    `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  const DAY = 86400000;
  const y = today().getFullYear();
  const years = [y, y - 1];

  const st = $state({
    range: "this",
    from: iso(new Date(+today() - 30 * DAY)),
    to: iso(today()),
    provider: "claude-code",
    names: false,
    cards: [] as WrappedCard[],
    label: "",
    about: [] as string[],
    i: 0,
    busy: false,
    msg: "",
    req: 0,
    info: false,
  });
  let seen = new Set<string>();
  let shown: string | null = null;
  let dir = "";
  let entering = $state(true);
  let root: HTMLDivElement | undefined = $state();

  const n = $derived(st.cards.length);
  const c = $derived(st.cards[st.i]);
  const has = $derived(n > 0);

  function args(): WrappedArgs {
    const range = (
      st.range === "custom" ? { kind: "custom", from: st.from, to: st.to } : { kind: st.range }
    ) as WrappedArgs["range"];
    return {
      range,
      provider: st.provider || null,
      names: st.names,
      colors: palette(),
      variant: look.variant,
    };
  }

  function customError() {
    if (st.range !== "custom") return "";
    const a = Date.parse(st.from),
      b = Date.parse(st.to);
    if (!(a && b)) return "Pick both dates.";
    return (b - a) / DAY + 1 < 7 ? "A Wrapped needs at least 7 days." : "";
  }

  async function fetchCards() {
    const err = customError();
    if (err) {
      st.msg = err;
      return;
    }
    const req = ++st.req;
    st.busy = true;
    st.msg = "";
    seen = new Set();
    try {
      const r: any = await wrappedCards(args());
      if (req !== st.req) return;
      st.cards = r.cards || [];
      st.label = r.label || "";
      st.about = r.about || [];
      st.i = Math.min(st.i, Math.max(0, st.cards.length - 1));
    } catch (e: any) {
      if (req !== st.req) return;
      st.cards = [];
      st.msg = String(e && e.message ? e.message : e);
    }
    st.busy = false;
    keepFocus();
  }

  async function save(all: boolean) {
    if (!st.cards.length) return;
    st.msg = "Saving...";
    try {
      const d = await wrappedSave(args(), all ? null : st.i);
      st.msg = (all ? `${st.cards.length} cards saved to ` : "Saved to ") + d;
    } catch (e: any) {
      st.msg = String(e && e.message ? e.message : e);
    }
  }

  function go(i: number) {
    if (!st.cards.length) return;
    const was = st.i;
    st.i = Math.max(0, Math.min(st.cards.length - 1, i));
    dir = st.i > was ? "next" : st.i < was ? "prev" : "";
    tick().then(() => root?.querySelector<HTMLElement>(".wrap-stage")?.focus());
  }

  const set = (k: "range" | "provider" | "names" | "from" | "to") => (v: any) => {
    (st as any)[k] = v;
    fetchCards();
  };

  function card(node: HTMLElement, p: { svg: string; id: string; text: string }) {
    const el = svgNode(p.svg);
    if (el) {
      // In the page first: the figure is measured (fitBig) before the count-up shortens its text.
      node.prepend(el);
      fitBig(el);
      // Measured again once the card font is in: a box measured on the fallback font is too narrow to trigger the fit.
      document.fonts.ready.then(() => {
        if (el.isConnected) fitBig(el);
      });
      if (p.svg !== shown) {
        shown = p.svg;
        arrive(el, p.id, dir, seen.has(p.svg));
        seen.add(p.svg);
      }
    } else {
      const t = document.createElement("p");
      t.textContent = p.text;
      node.prepend(t);
    }
    dir = "";
  }

  function focusables() {
    return root ? [...root.querySelectorAll<HTMLElement>("button, input, [tabindex]")] : [];
  }
  function keepFocus() {
    tick().then(() => {
      if (root && !root.contains(document.activeElement))
        (root.querySelector<HTMLElement>(".wrap-stage") || focusables()[0])?.focus();
    });
  }

  function keys(ev: KeyboardEvent) {
    if (ev.key === "Escape") {
      ev.stopPropagation();
      if (st.info) st.info = false;
      else onclose();
      return;
    }
    const typing = (ev.target as Element).closest("input, select");
    if (!typing) {
      const k = (
        { ArrowLeft: st.i - 1, ArrowRight: st.i + 1, Home: 0, End: st.cards.length - 1 } as Record<
          string,
          number
        >
      )[ev.key];
      if (k != null) {
        ev.preventDefault();
        go(k);
        return;
      }
    }
    if (ev.key !== "Tab") return;
    const f = [
      ...(ev.currentTarget as HTMLElement).querySelectorAll<HTMLElement>('button, input, [tabindex="0"]'),
    ].filter((x) => !(x as HTMLButtonElement).disabled);
    if (!f.length) return;
    const first = f[0],
      last = f[f.length - 1];
    if (ev.shiftKey && document.activeElement === first) {
      ev.preventDefault();
      last.focus();
    } else if (!ev.shiftKey && document.activeElement === last) {
      ev.preventDefault();
      first.focus();
    }
  }

  onMount(() => {
    void opener;
    (root!.querySelector<HTMLElement>(".wrap-stage") || focusables()[0])?.focus();
    loadFonts();
    fetchCards();
  });
</script>

<div
  class={"wrap-root" + (entering ? " enter" : "")}
  bind:this={root}
  onanimationend={(e) => {
    if ((e.target as Element).classList.contains("wrap")) entering = false;
  }}
>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="scrim" onclick={onclose}></div>
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions, a11y_no_noninteractive_element_to_interactive_role -->
  <section class="wrap" role="dialog" aria-modal="true" aria-label="Your Wrapped" onkeydown={keys}>
    <div class="wrap-head">
      <h2>
        Your Wrapped{#if st.label}<span class="wrap-label">{st.label.replace("_", " to ")}</span>{/if}
      </h2>
      <div class="wrap-controls">
        <Seg
          items={[
            ["this", String(years[0])],
            ["last", String(years[1])],
            ["custom", "Custom"],
          ]}
          cur={st.range}
          on={set("range")}
          label="Period"
        />
        <Seg items={PROVIDERS} cur={st.provider} on={set("provider")} label="Agents" />
        <label class="wrap-names">
          <span>Project names</span>
          <input
            type="checkbox"
            class="switch"
            checked={st.names}
            onchange={(e) => set("names")(e.currentTarget.checked)}
          />
        </label>
      </div>
      <button
        class="icon-btn wrap-info"
        aria-label="About these numbers"
        aria-expanded={!!st.info}
        disabled={!st.about || !st.about.length}
        onclick={() => {
          st.info = !st.info;
        }}
      >
        i
      </button>
      <button class="icon-btn wrap-close" aria-label="Close Wrapped" onclick={onclose}>×</button>
    </div>
    {#if st.info}
      <div class="wrap-about" role="region" aria-label="About these numbers">
        <h3>About these numbers</h3>
        <ul>
          {#each st.about as t}<li>{t}</li>{/each}
        </ul>
        <button class="icon-btn" onclick={() => openSources("Wrapped equivalents")}>
          Sources and credits
        </button>
      </div>
    {/if}
    {#if !n}
      <div class="wrap-stage wrap-empty" tabindex="-1">
        {st.busy ? "Counting your tokens..." : st.msg || "Nothing to show for this period."}
      </div>
    {:else}
      {#key c.svg}
        <div class="wrap-carousel">
          <button
            class="icon-btn wrap-nav"
            aria-label="Previous card"
            disabled={st.i === 0}
            onclick={() => go(st.i - 1)}
          >
            ‹
          </button>
          <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
          <div
            class={"wrap-stage" + (st.busy ? " busy" : "")}
            tabindex="0"
            role="group"
            aria-roledescription="card"
            aria-label={`Card ${st.i + 1} of ${n}`}
            use:card={{ svg: c.svg, id: c.id, text: c.text }}
          >
            <p class="sr" aria-live="polite">{c.text}</p>
          </div>
          <button
            class="icon-btn wrap-nav"
            aria-label="Next card"
            disabled={st.i === n - 1}
            onclick={() => go(st.i + 1)}
          >
            ›
          </button>
        </div>
      {/key}
    {/if}
    <div class="wrap-foot">
      {#if n >= 2}
        <div class="wrap-dots" role="group" aria-label="Cards">
          {#each st.cards as _, i}<button
              class="wrap-dot"
              aria-label={`Card ${i + 1}`}
              aria-current={i === st.i ? "true" : null}
              onclick={() => go(i)}
            ></button>{/each}
        </div>
      {/if}
      <div class="wrap-left">
        {#if st.range === "custom"}
          <span class="wrap-dates">
            <input
              type="date"
              class="select"
              value={st.from}
              max={st.to}
              aria-label="From"
              onchange={(e) => set("from")(e.currentTarget.value)}
            />
            <span class="wrap-to">to</span>
            <input
              type="date"
              class="select"
              value={st.to}
              min={st.from}
              max={iso(today())}
              aria-label="To"
              onchange={(e) => set("to")(e.currentTarget.value)}
            />
          </span>
        {/if}
        <p class="wrap-msg" role="status">{has ? st.msg : ""}</p>
      </div>
      <div class="wrap-actions">
        <button class="icon-btn" disabled={!has || st.busy} onclick={() => save(false)}>
          Save this card
        </button>
        <button class="icon-btn primary" disabled={!has || st.busy} onclick={() => save(true)}>
          Save all
        </button>
      </div>
    </div>
  </section>
</div>
