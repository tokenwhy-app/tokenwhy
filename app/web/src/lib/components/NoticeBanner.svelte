<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import NoticeActions from './NoticeActions.svelte';
  import type { Notice } from '../types';
  let { n }: { n: Notice } = $props();
</script>

<div class="banner" role="alert"><span><strong>{n.title}</strong>{n.text ? ' ' + n.text : ''}</span><NoticeActions {n} /></div>
