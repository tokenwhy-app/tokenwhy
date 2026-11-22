<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import { tick } from "svelte";
  import { share } from "./store.svelte";

  let root: HTMLDivElement | undefined = $state();
  const close = () => {
    share.text = null;
  };

  $effect(() => {
    if (share.text == null) return;
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        close();
      }
    };
    document.addEventListener("keydown", key);
    tick().then(() => root?.querySelector<HTMLButtonElement>(".src-close")?.focus());
    return () => document.removeEventListener("keydown", key);
  });
</script>

{#if share.text != null}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div
    class="src-root share-top"
    bind:this={root}
    onclick={(e) => {
      if (e.target === root) close();
    }}
  >
    <div class="panel src-dialog" role="dialog" aria-modal="true" aria-labelledby="share-text-title">
      <header class="src-top">
        <div>
          <h2 id="share-text-title">{share.textTitle}</h2>
          {#if share.textTitle === "What is sent"}<p class="sw-hint">
              tokenwhy sends exactly this text, without the spaces.
            </p>{/if}
        </div>
        <button class="icon-btn src-close" onclick={close} aria-label="Close">Close</button>
      </header>
      <div class="src-body"><pre class="share-json">{share.text}</pre></div>
    </div>
  </div>
{/if}

<style>
  .share-top {
    z-index: 60;
  }
  .share-json {
    margin: 0;
    font: var(--fs-s) / 1.5 var(--mono, ui-monospace, monospace);
    white-space: pre-wrap;
    word-break: break-all;
    user-select: text;
  }
</style>
