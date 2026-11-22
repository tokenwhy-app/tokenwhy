<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  // Shown after the first full scan and a look at the Overview, never during the tour.
  import { tick } from 'svelte';
  import { ui } from '../../lib/stores/ui.svelte';
  import { view as tour } from '../tour/run.svelte';
  import { CONSENT } from './consent';
  import { share, turnOn, dismiss, showPreview } from './store.svelte';

  const SEEN_MS = 4000;
  let open = $state(false);
  let root: HTMLDivElement | undefined = $state();

  const ready = $derived(!!(share.st && !share.st.prompted && !share.st.enabled && ui.settings.first_run_done === true
    && ui.status && ui.status.kind === 'ok' && ui.s && (ui.s.progress ?? 0) >= 1 && ui.tab === 'overview' && !tour.id && !ui.settingsOpen));

  $effect(() => {
    if (!ready || open) return;
    const t = setTimeout(() => { open = true; }, SEEN_MS);
    return () => clearTimeout(t);
  });

  $effect(() => {
    if (!open) return;
    tick().then(() => root?.querySelector<HTMLButtonElement>('.share-on')?.focus());
  });

  const answer = async (on: boolean) => { open = false; if (on) await turnOn(); else await dismiss(); };
</script>

{#if open}
  <div class="src-root" bind:this={root}>
    <div class="panel src-dialog share-consent" role="dialog" aria-modal="true" aria-labelledby="share-consent-title">
      <header class="src-top"><h2 id="share-consent-title">{CONSENT.title}</h2></header>
      <div class="src-body">
        {#each CONSENT.paragraphs as p}<p>{p}</p>{/each}
        <div class="notice-actions">
          <button class="icon-btn" onclick={showPreview}>{CONSENT.see}</button>
          <button class="icon-btn share-on" onclick={() => answer(true)}>{CONSENT.on}</button>
          <button class="icon-btn" onclick={() => answer(false)}>{CONSENT.later}</button>
        </div>
        {#if share.msg}<p class="sw-hint">{share.msg}</p>{/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .share-consent { width: min(560px, 100%); }
  .share-consent p { margin: 0 0 var(--s4); line-height: 1.5; }
</style>
