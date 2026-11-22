<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import { openWrapped } from "./open";
</script>

<button
  class="icon-btn wrapped-btn"
  aria-haspopup="dialog"
  title="Your Wrapped"
  onclick={(e) => openWrapped(e.currentTarget)}
>
  <svg viewBox="0 0 16 16" aria-hidden="true">
    <path
      d="M8 1.5l1.7 4.1 4.3.3-3.3 2.8 1 4.3L8 10.7 4.3 13l1-4.3L2 5.9l4.3-.3z"
      fill="none"
      stroke="currentColor"
      stroke-linejoin="round"
    />
  </svg>
  <span>Wrapped</span>
</button>
