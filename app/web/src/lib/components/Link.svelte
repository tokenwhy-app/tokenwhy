<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import Icon from './Icon.svelte';
  let { text, on }: { text: string; on: (e: MouseEvent) => void } = $props();
</script>

<button class="link" onclick={on}><span>{text}</span><Icon name="chev" /></button>
