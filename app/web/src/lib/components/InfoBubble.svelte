<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import { openSources } from "../../features/sources/store.svelte";
  let { section, label }: { section: string; label: string } = $props();
</script>

<button
  class="info-bubble"
  type="button"
  aria-label={"Learn more about " + label}
  onclick={(e) => {
    e.stopPropagation();
    openSources(section);
  }}
>
  Learn more
</button>
