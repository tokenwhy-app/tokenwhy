<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import { look } from '../lib/stores/look.svelte';
  import { logo } from '../lib/brand';

  const img = $derived(logo(look.variant, 32));
</script>

<img class="mark" src={img.src} srcset={img.srcset} width="32" height="32" alt="" draggable="false">
