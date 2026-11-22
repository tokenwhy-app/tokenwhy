<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  // Links go through `open_credit`, which opens only the links listed there.
  import { tick } from "svelte";
  import { openCredit } from "../../lib/bridge";
  import { sources, closeSources } from "./store.svelte";

  let root: HTMLDivElement | undefined = $state();

  $effect(() => {
    if (!sources.open) return;
    void sources.seq;
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        closeSources();
      }
    };
    document.addEventListener("keydown", key);
    tick().then(() => {
      if (!root) return;
      root.querySelector<HTMLButtonElement>(".src-close")?.focus();
      const target = sources.section && root.querySelector(`[data-title="${CSS.escape(sources.section)}"]`);
      if (target) {
        target.scrollIntoView({ block: "start" });
        target.classList.remove("src-hl");
        void (target as HTMLElement).offsetWidth;
        target.classList.add("src-hl");
      }
    });
    return () => document.removeEventListener("keydown", key);
  });
</script>

{#if sources.open}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div
    class="src-root"
    bind:this={root}
    onclick={(e) => {
      if (e.target === root) closeSources();
    }}
  >
    <div class="panel src-dialog" role="dialog" aria-modal="true" aria-labelledby="src-title">
      <header class="src-top">
        <div>
          <h2 id="src-title">Sources and credits</h2>
          <p class="sw-hint">
            Code, fonts and data tokenwhy uses, with the day each source was read. Licence texts are in
            NOTICE.
          </p>
        </div>
        <button class="icon-btn src-close" onclick={() => closeSources()} aria-label="Close">Close</button>
      </header>
      <div class="src-body">
        {#if sources.list}
          {#each sources.list as s}
            <section class="src-sec" data-title={s.title}>
              <h3>{s.title}</h3>
              <ul>
                {#each s.entries as e}
                  <li class="src-entry">
                    <div class="src-head">
                      <span class="src-name">{e.name}</span>
                      {#if e.read}<span class="src-read">{"read " + e.read}</span>{/if}
                    </div>
                    <p class="src-detail">{e.detail}</p>
                    <a
                      class="src-link"
                      href={e.url}
                      onclick={(ev) => {
                        ev.preventDefault();
                        openCredit(e.url);
                      }}
                    >
                      {e.url.replace(/^https:\/\//, "")}
                    </a>
                  </li>
                {/each}
              </ul>
            </section>
          {/each}
        {:else}
          <p class="sw-hint">The list comes from the app. In a terminal: tokenwhy about</p>
        {/if}
      </div>
    </div>
  </div>
{/if}
