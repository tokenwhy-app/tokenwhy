<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import { ui } from '../../lib/stores/ui.svelte';
  import { view } from './run.svelte';
  import { maybeStartTour, placeTour } from './tour';

  $effect(() => {
    void [ui.s, ui.tab, ui.settingsOpen, ui.status, ui.stateReady, ui.settings, ui.open.size, ui.selKind, ui.openCat, ui.selStartup];
    tick().then(() => { placeTour(); maybeStartTour(ui); });
  });
</script>

{#if view.id}
  <div id="tour" class="tour-root">
    <div class="tour-ring" style:left={view.ring ? view.ring.left + 'px' : null} style:top={view.ring ? view.ring.top + 'px' : null}
      style:width={view.ring ? view.ring.width + 'px' : null} style:height={view.ring ? view.ring.height + 'px' : null}></div>
    {#key view.id}
      <div class="panel tour-card" role="dialog" aria-modal="true" aria-labelledby="tour-title" aria-describedby="tour-text"
        style:left={view.card ? view.card.left + 'px' : null} style:top={view.card ? view.card.top + 'px' : null}>
        <h2 id="tour-title">{view.title}</h2>
        <p id="tour-text">{view.text}</p>
        <div class="tour-foot">
          <button class="link tour-skip" onclick={() => view.end()}>Skip</button>
          <span class="tour-count num">{`${view.i + 1} of ${view.n}`}</span>
          {#if view.i > 0}<button class="icon-btn" onclick={() => view.go(-1)}>Back</button>{/if}
          <button class="icon-btn tour-next" onclick={() => view.go(1)}>{view.i === view.n - 1 ? 'Done' : 'Next'}</button>
        </div>
      </div>
    {/key}
    <span class="sr-only" aria-live="polite">{`Step ${view.i + 1} of ${view.n}: ${view.title}`}</span>
  </div>
{/if}
