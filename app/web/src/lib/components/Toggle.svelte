<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  let {
    label,
    hint = null,
    on,
    change,
  }: { label: string; hint?: string | null; on: boolean; change: (v: boolean) => void } = $props();
</script>

<label class="switch-row" data-tip={hint}>
  <span><span class="sw-label">{label}</span></span>
  <input type="checkbox" class="switch" checked={!!on} onchange={(e) => change(e.currentTarget.checked)} />
</label>
