<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import NoticeActions from "./NoticeActions.svelte";
  import ScanBar from "./ScanBar.svelte";
  import type { Notice } from "../types";

  let { n, progress }: { n: Notice; progress: number | null } = $props();
</script>

<section class="panel notice" role={n.progress ? "status" : "alert"}>
  <h2>{n.title}</h2>
  {#if n.text}<p>{n.text}</p>{/if}{#if n.roots && n.roots.length}<ul class="notice-roots">
      {#each n.roots as r}<li>{r}</li>{/each}
    </ul>{/if}{#if n.hint}<p class="notice-hint">{n.hint}</p>{/if}{#if n.progress}<ScanBar
      p={progress}
    />{:else}<NoticeActions {n} />{/if}
</section>
