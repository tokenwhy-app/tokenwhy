<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  let { text, tip = null }: { text: string | number; tip?: string | null } = $props();
</script>

<span data-tip={tip || null}>{text}</span>
