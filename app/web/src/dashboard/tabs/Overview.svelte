<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import { verdictWrap } from '../../lib/verdictWrap';
  import { minorTest } from "../../lib/minor";
  import type { Ctx } from "../../lib/ctx";
  import Card from "../../lib/components/Card.svelte";
  import Chip from "../../lib/components/Chip.svelte";
  import DataTable from "../../lib/components/DataTable.svelte";
  import FindingList from "../../lib/components/FindingList.svelte";
  import Link from "../../lib/components/Link.svelte";
  import CopyFixes from "../../lib/components/CopyFixes.svelte";
  import Slot from "../../lib/components/Slot.svelte";
  import Seg from "../../lib/components/Seg.svelte";
  import { features } from "../../features/registry";
  import { exact, int, pct, qfmt, tok, usd } from "../../lib/format";
  import {
    causeColor,
    causeOf,
    dollars,
    rate,
    saved,
    say,
    diagnose,
    days as dayRows,
    weekOverWeek,
    unitCost,
  } from "../../lib/diagnose";
  import { columns, donut, heatmap, pareto } from "../../lib/charts/chart";
  import { providerNotes, provOf } from "../Header.svelte";
  import { TAB_TIPS } from "../Tabs.svelte";
  import {
    NOUN,
    FRESH_TIP,
    CACHE_TIP,
    money,
    moneyTip,
    areaFindings,
    avgOver,
    moneyView,
    spendWeek,
    share100,
    unitUsd,
    UNIT_NOTE,
    kpiGrid,
    SCREEN,
    screenEnd,
    groupDays,
    type Scale,
  } from "./overview";
  import { spendSeries, spendScale } from "../../lib/bridge";
  import { dayStart, inDays, lastDays, spanDays } from "../../lib/days";
  let { ctx }: { ctx: Ctx } = $props();

  const DN = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
  const s = $derived(ctx.s);
  const ui = $derived(ctx.ui);
  const n = $derived(ctx.n);
  const r = $derived(rate(s));
  const minor = $derived(minorTest(ctx));
  const fs = $derived(areaFindings(ctx, null).filter((f: any) => !minor(f)));
  const groups = $derived(diagnose(fs, r) as any[]);
  const ds = $derived(dayRows(s, ctx.inProject) as any[]);
  const filtered = $derived(!!ui.project);
  const wow = $derived(weekOverWeek(s, ds, "cost_usd", filtered) as any);
  const sw = $derived(moneyView(ui) === "pct" ? spendWeek(ds, n, s.weeks) : null);
  // Calendar days in the window, holidays included; every average and axis counts these, not the days with use.
  const win = $derived(n || spanDays(ds));
  const winDays = $derived(lastDays(win));
  const wdCount = $derived.by(() => {
    const k = Array(7).fill(0);
    for (const d of winDays) k[(new Date(d).getDay() + 6) % 7]++;
    return k;
  });
  const notes = $derived(providerNotes(ctx));

  const tot = $derived(groups.reduce((a, g) => a + g.tokens, 0));
  const totUsd = $derived(
    groups.some((g) => g.usd != null) ? groups.reduce((a, g) => a + (g.usd || 0), 0) : null,
  );
  const g0 = $derived(groups[0]);
  const usdWeek = (v: number) => (v < 10 ? usd(v) : "$" + Math.round(v).toLocaleString("en-US"));
  const P = $derived(sw != null ? share100(totUsd, sw) : null);

  const swAvg = $derived(spendWeek(ds, n, s.weeks));
  const fixes = $derived(fs.length > 1 && ds.length >= 7 && !!r);
  const steps = $derived(
    groups.map((g) => ({
      label: g.cause.name,
      value: g.usd || 0,
      color: causeColor(g.cause),
      tip: tok(g.tokens) + " tokens",
    })),
  );
  const whole = $derived(groups.reduce((a, x) => a + (x.usd || 0), 0));
  const rest = $derived(groups.slice(3));

  const vfmt = $derived(r ? usd : tok);
  const items = $derived.by(() => {
    if (fs.length <= 1) return [];
    const N = 12;
    const val = (f: any) => (r ? dollars(f, r) || 0 : saved(f));
    const out: any[] = fs
      .slice(0, N)
      .map((f: any) => ({
        label: say(f).fact,
        short: NOUN[f.rule] || f.rule,
        value: val(f),
        color: causeColor(causeOf(f.rule)),
        tip:
          (r ? usd(val(f)) + " a week · " : "") +
          tok(saved(f)) +
          " tokens" +
          (f.project ? " · " + f.project : ""),
      }));
    if (fs.length > N) {
      const v = fs.slice(N).reduce((a: number, f: any) => a + saved(f), 0),
        vu = fs.slice(N).reduce((a: number, f: any) => a + val(f), 0);
      out.push({
        label: `${fs.length - N} other findings`,
        short: "others",
        value: vu,
        color: "var(--muted)",
        tip: (r ? usd(vu) + " a week · " : "") + tok(v) + " tokens",
      });
    }
    return out;
  });
  const paretoRows = $derived.by(() => {
    let c = 0;
    const T = items.reduce((a, i) => a + i.value, 0) || 1;
    return items.map((i) => {
      c += i.value;
      return [i.label, vfmt(i.value), pct(c / T)];
    });
  });

  const dd = (d: any) =>
    new Date(d.day_ms).toLocaleDateString("en-GB", { weekday: "short", day: "numeric", month: "short" });
  const toks = (d: any) => d.fresh + d.cache;
  const hourly = $derived((s.hourly || []).filter((x: any) => !x.project || ctx.inProject(x)));
  const cells = $derived.by(() => {
    const sum = Array.from({ length: 7 }, () => Array(24).fill(0));
    for (const x of hourly as any[]) {
      const wd = (new Date(x.day_ms).getDay() + 6) % 7;
      sum[wd][x.hour] += x.cost_usd || 0;
    }
    return sum.map((row, i) => row.map((v: number) => v / Math.max(wdCount[i], 1)));
  });
  const peak = (row: number[]) => String(row.indexOf(Math.max(...row))).padStart(2, "0") + ":00";
  const rowTip = (i: number) =>
    `${DN[i]} · ${usd(cells[i].reduce((a: number, v: number) => a + v, 0))} a day\nBusiest hour ${peak(cells[i])}`;
  const byDay = $derived.by(() => {
    const sum = Array(7).fill(0),
      k = wdCount;
    for (const d of ds) {
      const wd = (new Date(d.day_ms).getDay() + 6) % 7;
      sum[wd] += d.cost;
    }
    return { avg: sum.map((v, i) => (k[i] ? v / k[i] : 0)), k };
  });

  let scale = $state<Scale>(spendScale.get());
  let back = $state(0);
  let fetched = $state<any[] | null>(null);
  const count = $derived(scale === "day" ? n || SCREEN.day : SCREEN[scale]);
  const endMs = $derived(screenEnd(scale, back, count));
  const live = $derived(scale === "day" && back === 0);
  $effect(() => {
    const k = [scale, endMs, count, ui.project, ui.provider, ui.account, s] as const;
    if (live) {
      fetched = null;
      return;
    }
    let gone = false;
    spendSeries(k[0], k[1], k[2], k[3] || null, k[4] || null, k[5] || null).then((b) => {
      if (!gone) fetched = b;
    });
    return () => {
      gone = true;
    };
  });
  const ALL: Record<Scale, number> = { day: 730, week: 110, month: 26 };
  let fixMax = $state<{ max: number; lmax: number } | undefined>(undefined);
  $effect(() => {
    const k = [scale, ui.project, ui.provider, ui.account, s] as const;
    let gone = false;
    spendSeries(
      k[0],
      screenEnd(k[0], 0, ALL[k[0]]),
      ALL[k[0]],
      k[1] || null,
      k[2] || null,
      k[3] || null,
    ).then((b) => {
      if (!gone && b)
        fixMax = {
          max: Math.max(0, ...b.map((x) => x.cost || 0)),
          lmax: Math.max(0, ...b.map((x) => x.tokens || 0)),
        };
    });
    return () => {
      gone = true;
    };
  });
  const dsFull = $derived.by(() => {
    if (!ds.length) return ds;
    const by = new Map(ds.map((d: any) => [new Date(d.day_ms).toDateString(), d]));
    return winDays.map(
      (t) =>
        by.get(new Date(t).toDateString()) ?? {
          day_ms: t,
          cost: 0,
          fresh: 0,
          cache: 0,
          sessions: 0,
          agents: 0,
        },
    );
  });
  const bars = $derived(live ? null : (fetched ?? groupDays(ds, scale, endMs, count)));
  const range = $derived.by(() => {
    const xs = bars ?? dsFull;
    if (!xs || !xs.length) return "";
    const a = new Date(xs[0].start_ms ?? xs[0].day_ms),
      b = new Date(xs[xs.length - 1].start_ms ?? xs[xs.length - 1].day_ms);
    const f = (d: Date) =>
      scale === "month"
        ? d.toLocaleDateString("en-GB", { month: "short", year: "2-digit" })
        : d.toLocaleDateString("en-GB", { day: "numeric", month: "short" });
    return f(a) + " – " + f(b);
  });
  const setScale = (v: Scale) => {
    scale = v;
    back = 0;
    spendScale.set(v);
  };
  const NAME: Record<Scale, string> = { day: "Daily", week: "Weekly", month: "Monthly" };
  const bLabel = (b: any) => {
    const d = new Date(b.start_ms);
    return scale === "month"
      ? d.toLocaleDateString("en-GB", { month: "short" })
      : scale === "week"
        ? d.toLocaleDateString("en-GB", { day: "numeric", month: "short" })
        : String(d.getDate());
  };
  const bName = (b: any) => {
    const d = new Date(b.start_ms);
    return scale === "month"
      ? d.toLocaleDateString("en-GB", { month: "long", year: "numeric" })
      : scale === "week"
        ? "Week of " + d.toLocaleDateString("en-GB", { day: "numeric", month: "short", year: "numeric" })
        : dd({ day_ms: b.start_ms });
  };
  // Last complete bucket against the one before it; a partial bucket would compare part of a period with a whole one.
  const prevCmp = $derived.by(() => {
    if (!bars) return null;
    const done = bars.filter((b: any) => !b.partial);
    if (done.length < 2) return null;
    const a = done[done.length - 1],
      p = done[done.length - 2];
    if (a.cost == null || p.cost == null || p.cost <= 0) return null;
    return { delta: a.cost / p.cost - 1, cur: a, prev: p };
  });

  const t = $derived(s.live.today as any);
  const sess = $derived((s.live.sessions || []).filter(ctx.inProject) as any[]);
  const wk = (key: string) => weekOverWeek(s, ds, key, filtered) as any;
  const c = $derived(wk("cost_usd"));
  const f = $derived(c ? wk("fresh_tokens") : null);
  const rd = $derived(c ? wk("cache_read_tokens") : null);
  const sn = $derived(c ? wk("sessions") : null);
  const ag = $derived(c ? wk("agents") : null);
  const openNote = $derived(
    sess.length
      ? sess
          .slice(0, 2)
          .map((x) => x.project + " " + tok(x.ctx_tokens))
          .join(" · ")
      : "no session",
  );
  const openTip = $derived(
    sess.length
      ? sess
          .slice(0, 2)
          .map((x) => `${x.project}${provOf(ctx, x)}: ${tok(x.ctx_tokens)} context`)
          .concat(sess.length > 2 ? [`and ${sess.length - 2} more`] : [])
          .join("\n")
      : null,
  );
  const uc = $derived(c ? (unitCost(s, ds, filtered) as any) : null);
  const ucCls = $derived(uc ? (uc.verdict === "better" ? "down" : uc.verdict === "worse" ? "up" : "") : "");
  const unitTip = (u: any) =>
    [u.bias, u.w ? `Last 7 days ${unitUsd(u.w.cur)}, before ${unitUsd(u.w.prev)}` : null]
      .filter(Boolean)
      .join("\n");
</script>

{#snippet k(
  label: string,
  value: string,
  note: string | null,
  tip: string | null,
  w: any,
  fmt: (v: number) => string,
)}<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div class="kpi" data-tip={tip || null} tabindex={tip ? 0 : null}>
    <span class="kpi-label">{label}</span>
    <span class="kpi-val num">{value}</span>
    <span class="kpi-note">
      <Chip {w} {fmt} />{#if note}<span>{note}</span>{/if}
    </span>
  </div>{/snippet}

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
{#if !groups.length}<section class="verdict">
    <p class="v-line">Nothing to optimize.</p>
    <p class="v-sub">
      <span>Already in good shape, or not enough history yet.</span>
      <span class="v-act"><CopyFixes report mcp /></span>
    </p>
  </section>{:else}<section class="verdict">
    {#if P != null}<p class="v-line" use:verdictWrap>
        You could cut about <b class="num">{pct(P)}</b>
        of your spend. <span class="v-start">{g0.cause.start}</span>
      </p>{:else if totUsd != null}<p class="v-line" use:verdictWrap>
        About <b class="num">{usdWeek(totUsd)}</b>
        a week is avoidable. <span class="v-start">{g0.cause.start}</span>
      </p>{:else}<p class="v-line" use:verdictWrap>
        About <b class="num">{tok(tot)}</b>
        tokens a week are avoidable. <span class="v-start">{g0.cause.start}</span>
      </p>{/if}{#if P != null}<p
        class="v-last"
        tabindex="0"
        data-tip={`${usd(totUsd)} a week${avgOver(s.weeks)}, times 4.35 weeks`}
      >
        {`About ${usd((totUsd || 0) * 4.35)} a month.`}
      </p>{/if}
    <p class="v-sub"><span class="v-act"><CopyFixes mcp /></span></p>
  </section>{/if}
{#if ui.wrapped}<button class="wrap-cta" onclick={(e) => ui.wrapped?.open(e.currentTarget)}>
    <span class="wrap-cta-star" aria-hidden="true">★</span>
    <span>
      <b>{"Your " + new Date().getFullYear() + " Wrapped"}</b>
      <span class="wrap-cta-sub">Your year with AI agents, in 12 cards</span>
    </span>
    <span class="wrap-cta-go" aria-hidden="true">›</span>
  </button>{/if}
<div class="pnotes-row">
  {#each features.overviewLines as L}<L {ctx} />{/each}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  {#if notes.length}<p class="pnotes">
      <span tabindex="0" data-tip={notes.join("\n")}>
        {`${notes.length} data ${notes.length === 1 ? "note" : "notes"}`}
      </span>
    </p>{/if}
</div>

{#if groups.length}
  <div class="grid causes" style:--n={Math.min(groups.length, 3)}>
    {#each groups.slice(0, 3) as g, i (g.cause.id)}{@const cs = g.cause}{@const key =
        "cause-" + cs.id}{@const pc =
        whole > 0 && g.usd != null ? Math.round((100 * g.usd) / whole) : null}{@const open =
        ctx.open(key)}<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <section class="card cause h-cause span-4" style:--c={causeColor(cs)} style:--share={pc ?? 0}>
        <div class="cause-head">
          <span class="cause-rank num">{String(i + 1)}</span>
          <h2>{cs.name}</h2>
        </div>
        <div class="cause-num" tabindex="0" data-tip={moneyTip(g.usd, g.tokens)}>
          {#if sw != null && g.usd != null}<span class="big num">{pct(g.usd / sw)}</span>
            <span class="unit">of spend</span>{:else}<span class="big num">{money(g.usd, g.tokens)}</span>
            <span class="unit">{g.usd != null ? "a week" : "tokens a week"}</span>{/if}
        </div>
        <p class="cause-fact" tabindex="0" data-tip={cs.why}>{say(g.findings[0]).fact}</p>
        <p class="cause-fix">
          <span>Fix</span>{say(g.findings[0]).fix ?? cs.fix}
        </p>
        {#if open}<div class="cause-list" data-sk={key}>
            <FindingList {ctx} rows={g.findings} n={50} key={key + "-l"} />
          </div>{:else}<div class="cause-list"></div>{/if}
        <div class="cause-foot">
          {#if g.findings.length > 1}<button
              class="more inline"
              aria-expanded={open}
              onclick={() => ui.on.toggle(key)}
            >
              {open ? "Hide findings" : `${g.findings.length} findings`}
            </button>{:else}<span></span>{/if}<Link text="Details" on={() => ui.on.tab(cs.tab)} />
        </div>
      </section>{/each}
  </div>
  {#if rest.length}<p class="also">
      <span>Also</span>
      {#each rest as g (g.cause.id)}<button
          class="link"
          onclick={() => ui.on.tab(g.cause.tab)}
          data-tip={moneyTip(g.usd, g.tokens)}
        >
          <span class="swatch" style:background={causeColor(g.cause)}></span>
          {g.cause.name + " "}
          <span class="num muted">{money(g.usd, g.tokens)}</span>
        </button>{/each}
    </p>{/if}
{/if}

{#if fs.length > 1 || ds.length}
  <div class="grid">
    {#if fs.length > 1}
      <Card
        title="Where the waste is"
        note="one bar per finding"
        cls={(fixes ? "span-8" : "span-12") + " h-c1"}
      >
        <div class="key">
          {#each groups as g (g.cause.id)}<span>
              <span class="swatch" style:background={causeColor(g.cause)}></span>{g.cause.name}
            </span>{/each}
        </div>
        <Slot
          draw={(w) => pareto(items, w, "Waste by finding, largest first, with running total", vfmt)}
        /><DataTable
          caption="Waste by finding, a week"
          cols={["Finding", r ? "Cost a week" : "Tokens a week", "Running total"]}
          rows={paretoRows}
        />
      </Card>
    {/if}
    {#if fixes}
      <Card
        title="What fixing saves"
        note={sw != null
          ? pct(whole / sw) + " of spend"
          : swAvg != null
            ? pct(whole / swAvg) + " of avg week"
            : undefined}
        cls="span-4 h-c1"
      >
        <div class="save-split">
          <Slot
            cls="donut"
            draw={() =>
              donut(steps, "Weekly saving by cause", 176, usd(whole), "a week", (v) => usd(v) + " a week")}
          />
          <ul class="save-list">
            {#each steps as st (st.label)}<li>
                <span class="swatch" style:background={st.color}></span>
                <span>{st.label}</span>
                <span class="num">{usd(st.value)}</span>
              </li>{/each}
          </ul>
        </div>
        <DataTable
          caption="Weekly saving by cause"
          cols={["Cause", "Cost a week"]}
          rows={[...steps.map((st) => [st.label, usd(st.value)]), ["All fixes", usd(whole)]]}
        />
      </Card>
    {/if}
    {#if ds.length}
      <section class="card span-8 h-c2">
        <div class="card-head">
          <h2>{NAME[scale] + " spend"}</h2>
          <span class="card-note spend-ctl">
            {#if live}{#if wow && wow.delta != null}<Chip
                  w={wow}
                  fmt={usd}
                />{/if}{:else if prevCmp}{@const d =
                prevCmp.delta}<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
              <span
                class={"chip " + (Math.abs(d) < 0.005 ? "" : d > 0 ? "up" : "down")}
                tabindex="0"
                data-tip={`${bName(prevCmp.cur)} ${usd(prevCmp.cur.cost)}
${bName(prevCmp.prev)} ${usd(prevCmp.prev.cost)}`}
              >
                {(Math.abs(d) < 0.005 ? "±" : d > 0 ? "+" : "−") + pct(Math.abs(d)) + " vs previous " + scale}
              </span>{/if}
            <button class="nav" aria-label={"Previous " + scale + "s"} onclick={() => back++}>
              <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M10 3 5 8l5 5" /></svg>
            </button>
            <span class="range num">{range}</span>
            <button
              class="nav"
              aria-label={"Next " + scale + "s"}
              disabled={back === 0}
              onclick={() => back--}
            >
              <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M6 3l5 5-5 5" /></svg>
            </button>
            {#if back}<button class="nav today" onclick={() => (back = 0)}>Back to today</button>{/if}<Seg
              items={[
                ["day", "Days"],
                ["week", "Weeks"],
                ["month", "Months"],
              ]}
              cur={scale}
              on={setScale}
              label="Scale"
            />
          </span>
        </div>
        <div class="card-body">
          <div class="key">
            <span>
              <span class="swatch" style:background="var(--series-1)"></span>
              cost
            </span>
            <span>
              <span class="swatch line" style:background="var(--series-2)"></span>
              tokens
            </span>
          </div>
          {#if bars}<Slot
              draw={(w) =>
                columns(
                  bars.map((b: any) => ({
                    label: bLabel(b),
                    value: b.cost || 0,
                    dim: b.partial,
                    tip: `${bName(b)}${b.partial ? " (so far)" : ""}
${b.cost == null ? "no priced usage" : usd(b.cost)} · ${tok(b.tokens)} tokens`,
                  })),
                  w,
                  200,
                  `Cost and tokens per ${scale}`,
                  usd,
                  { values: bars.map((b: any) => b.tokens), fmt: tok, name: "tokens" },
                  fixMax,
                )}
            /><DataTable
              caption={`Cost and tokens per ${scale}`}
              cols={[scale === "day" ? "Day" : scale === "week" ? "Week" : "Month", "Cost", "Tokens"]}
              rows={bars.map((b: any) => [bName(b), b.cost == null ? "" : usd(b.cost), tok(b.tokens)])}
            />{:else}<Slot
              draw={(w) =>
                columns(
                  dsFull.map((d, i) => ({
                    label: new Date(d.day_ms).getDate(),
                    value: d.cost,
                    dim: i < dsFull.length - 7 || d.day_ms >= dayStart(),
                    tip: `${dd(d)}${d.day_ms >= dayStart() ? " (so far)" : ""}\n${usd(d.cost)} · ${tok(toks(d))} tokens`,
                  })),
                  w,
                  200,
                  "Cost and tokens per day",
                  usd,
                  { values: dsFull.map(toks), fmt: tok, name: "tokens" },
                  scale === "day" ? fixMax : undefined,
                )}
            /><DataTable
              caption="Cost and tokens per day"
              cols={["Day", "Cost", "Fresh tokens", "Cached tokens"]}
              rows={dsFull.map((d) => [dd(d), usd(d.cost), tok(d.fresh), tok(d.cache)])}
            />{/if}
        </div>
      </section>
      {#if hourly.length}
        <Card title="When you spend" note="average per hour" cls="span-4 h-c2">
          <Slot draw={(w) => heatmap(cells, w, "Average spend by weekday and hour", usd, rowTip)} /><DataTable
            caption="Average spend by weekday"
            cols={["Day", "Per day", "Busiest hour"]}
            rows={cells.map((row, i) => [
              DN[i],
              usd(row.reduce((a: number, v: number) => a + v, 0)),
              peak(row),
            ])}
          />
        </Card>
      {:else}
        <Card title="By weekday" note="average per day" cls="span-4 h-c2">
          <Slot
            draw={(w) =>
              columns(
                byDay.avg.map((v, i) => ({
                  label: DN[i].slice(0, 3),
                  value: v,
                  dim: i >= 5,
                  tip: `${DN[i]}\n${usd(v)} on average, ${byDay.k[i]} days`,
                })),
                w,
                200,
                "Average spend by weekday",
                usd,
                undefined,
              )}
          /><DataTable
            caption="Average spend by weekday"
            cols={["Day", "Per day", "Days"]}
            rows={byDay.avg.map((v, i) => [DN[i], usd(v), String(byDay.k[i])])}
          />
        </Card>
      {/if}
    {/if}
  </div>
{/if}

{#if c}
  <h3 class="section-label">Last 7 days</h3>
  <div class="kpis" use:kpiGrid={true}>
    {@render k("Cost", usd(c.cur), "today " + qfmt(t.cost_usd, usd), null, c, usd)}{#if f}{@render k(
        "Fresh tokens",
        tok(f.cur),
        "today " + tok(t.fresh_tokens),
        exact(f.cur) + " tokens\n" + FRESH_TIP,
        f,
        tok,
      )}{/if}{#if rd}{@render k(
        "Cache reads",
        tok(rd.cur),
        "today " + tok(t.cache_read_tokens),
        exact(rd.cur) + " tokens\n" + CACHE_TIP,
        rd,
        tok,
      )}{/if}{#each features.overviewKpis as K}<K {ctx} {ds} />{/each}{#if sn && ag}{@render k(
        "Sessions",
        int(sn.cur),
        int(ag.cur) + " subagents",
        `${int(ag.cur)} subagents\nThe 7 days before ${int(ag.prev)}`,
        sn,
        int,
      )}{/if}{@render k("Open now", String(sess.length), openNote, openTip, null, usd)}
  </div>
  {#if uc && uc.units.some((u: any) => u.w)}
    <h3 class="section-label">
      {"Cost per unit of work: " + UNIT_NOTE[uc.verdict] + (uc.mix && uc.mix.changed ? ", mix changed" : "")}
    </h3>
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div class="kpis" use:kpiGrid={false}>
      {#each uc.units.filter((u: any) => u.w) as u (u.key)}<div
          class="kpi"
          data-tip={unitTip(u)}
          tabindex="0"
        >
          <span class="kpi-label">{u.label}</span>
          <span class="kpi-val num">{u.w ? unitUsd(u.w.cur) : "n/a"}</span>
          <span class="kpi-note">
            {#if u.w && u.w.delta != null}<span class={"chip " + ucCls}>
                {(u.w.delta >= 0 ? "+" : "−") + pct(Math.abs(u.w.delta)) + " vs last week"}
              </span>{/if}
          </span>
        </div>{/each}
    </div>
  {/if}
{:else}
  <h3 class="section-label">Today</h3>
  <div class="kpis" use:kpiGrid={false}>
    {@render k("Cost", qfmt(t.cost_usd, usd), int(t.sessions) + " sessions", null, null, usd)}{@render k(
      "Fresh tokens",
      tok(t.fresh_tokens),
      "not cached",
      exact(t.fresh_tokens) + " tokens\n" + FRESH_TIP,
      null,
      tok,
    )}{@render k(
      "Cache reads",
      tok(t.cache_read_tokens),
      "at the cache rate",
      exact(t.cache_read_tokens) + " tokens\n" + CACHE_TIP,
      null,
      tok,
    )}{@render k("Subagents", int(t.agents), "started", TAB_TIPS.agents, null, int)}{@render k(
      "Open now",
      String(sess.length),
      openNote,
      openTip,
      null,
      usd,
    )}
  </div>
{/if}

<style>
  .spend-ctl {
    display: inline-flex;
    align-items: center;
    gap: var(--s2);
    flex-wrap: wrap;
    justify-content: flex-end;
  }
  .nav {
    display: inline-grid;
    place-items: center;
    min-width: 28px;
    height: 28px;
    padding: 0 var(--s2);
    border: 1px solid var(--rule);
    border-radius: 8px;
    background: var(--card);
    color: var(--fg);
    font: inherit;
    cursor: pointer;
  }
  .wrap-cta {
    display: flex;
    align-items: center;
    gap: var(--s4);
    width: 100%;
    margin: 0 0 var(--s5);
    padding: var(--s4) var(--s5);
    border: 1px solid color-mix(in srgb, var(--accent) 45%, transparent);
    border-radius: var(--r-l);
    background: linear-gradient(100deg, var(--accent-soft), var(--card) 70%);
    box-shadow: var(--shadow);
    color: var(--fg);
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition:
      transform 0.15s var(--ease),
      box-shadow 0.15s var(--ease);
  }
  .wrap-cta:hover {
    transform: translateY(-1px);
    box-shadow: var(--shadow-pop);
  }
  .wrap-cta b {
    display: block;
    font-size: var(--fs-m);
    font-weight: 600;
  }
  .wrap-cta-sub {
    color: var(--fg-2);
    font-size: var(--fs-s);
  }
  .wrap-cta-star {
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    border-radius: 50%;
    background: var(--accent);
    color: var(--card);
    font-size: 18px;
  }
  .wrap-cta-go {
    margin-left: auto;
    color: var(--accent);
    font-size: 22px;
  }
  .nav svg {
    width: 14px;
    height: 14px;
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .range {
    min-width: 108px;
    text-align: center;
    font-size: var(--fs-s);
    color: var(--fg-2);
  }
  .nav:hover:not(:disabled) {
    background: var(--sunken);
    color: var(--fg);
  }
  .nav:disabled {
    opacity: 0.35;
    cursor: default;
  }
  .nav.today {
    font-size: var(--fs-xs);
    font-weight: 600;
  }
</style>
