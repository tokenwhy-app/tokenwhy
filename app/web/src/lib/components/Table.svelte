<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts" module>
  import type { Snippet } from "svelte";
  export interface Col<R = any> {
    h: string;
    get?: (r: R) => string | number | null;
    cell?: Snippet<[R]>;
    n?: boolean;
    name?: boolean;
    w?: string;
  }
  export interface TableOpts<R = any> {
    empty?: string;
    cls?: string;
    pick?: (r: R) => void;
    sel?: (r: R) => boolean;
    expanded?: (r: R) => boolean | null;
  }
</script>

<script lang="ts">
  import Empty from "./Empty.svelte";
  let { cols, rows, opts = {} }: { cols: Col[]; rows: any[]; opts?: TableOpts } = $props();
  const key = (ev: KeyboardEvent, pick: () => void) => {
    if (ev.key === "Enter" || ev.key === " ") {
      ev.preventDefault();
      pick();
    }
  };
</script>

{#if !rows.length}
  <Empty text={opts.empty || "Nothing recorded in this window."} />
{:else}
  <div class="table-wrap">
    <table class={opts.cls || ""}>
      <thead>
        <tr>
          {#each cols as c}<th class={(c.n ? "n " : "") + (c.w || "")}>{c.h}</th>{/each}
        </tr>
      </thead>
      <tbody>
        {#each rows as r}{@const pick = opts.pick && !r.__detail ? () => opts.pick!(r) : null}
          <tr
            class={(pick ? "pick " : "") +
              (opts.sel && opts.sel(r) ? "sel " : "") +
              (r.__detail ? "detail " : "") +
              (r.__more ? "more-row" : "")}
            onclick={pick}
            tabindex={pick ? 0 : null}
            aria-expanded={opts.expanded ? !!opts.expanded(r) : null}
            onkeydown={pick ? (ev) => key(ev, pick) : null}
          >
            {#each cols as c}<td class={(c.n ? "n num " : "") + (c.name ? "name " : "") + (c.w || "")}>
                {#if c.cell}{@render c.cell(r)}{:else}{c.get ? c.get(r) : ""}{/if}
              </td>{/each}
          </tr>{/each}
      </tbody>
    </table>
  </div>
{/if}
