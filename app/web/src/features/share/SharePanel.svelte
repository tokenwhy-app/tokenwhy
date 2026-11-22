<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import Toggle from '../../lib/components/Toggle.svelte';
  import { share, probe, turnOn, turnOff, showPreview, showLog, saveFile, deleteSent, statusLine } from './store.svelte';

  onMount(() => { probe(); });
  const st = $derived(share.st);
  const line = $derived(statusLine(st));
  const change = (v: boolean) => { if (v) turnOn(); else share.asking = true; };
</script>

{#if share.available && st}
  <div class="settings-col">
    <h2>Sharing</h2>
    <Toggle label="Share anonymous numbers and compare with others" hint="Weekly, rounded" on={st.enabled && !share.asking} {change} />
    {#if share.asking}
      <p class="sw-hint">Stop sharing. Also delete the weeks already sent?</p>
      <div class="notice-actions">
        <button class="icon-btn" onclick={() => turnOff(true)}>Delete them</button>
        <button class="icon-btn" onclick={() => turnOff(false)}>Keep until they expire</button>
        <button class="icon-btn" onclick={() => { share.asking = false; }}>Keep sharing</button>
      </div>
    {/if}
    {#if line}<p class="sw-hint">{line}</p>{/if}
    <div class="notice-actions">
      <button class="icon-btn" onclick={showPreview}>See what is sent</button>
      <button class="icon-btn" onclick={saveFile}>Save to a file</button>
      <button class="icon-btn" disabled={!st.has_id && !st.pending_delete} onclick={deleteSent}>Delete what I sent</button>
      <button class="icon-btn" onclick={showLog}>Send log</button>
    </div>
    {#if share.msg}<p class="sw-hint">{share.msg}</p>{/if}
  </div>
{/if}
