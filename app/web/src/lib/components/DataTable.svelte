<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  let { caption, cols, rows }: { caption: string; cols: string[]; rows: (string | number)[][] } = $props();
</script>

<details class="chart-data">
  <summary>Data</summary>
  <table>
    <caption>{caption}</caption>
    <thead>
      <tr>
        {#each cols as c, i}<th class={i ? "n" : ""}>{c}</th>{/each}
      </tr>
    </thead>
    <tbody>
      {#each rows as r}<tr>
          {#each r as v, i}<td class={i ? "n num" : ""}>{v}</td>{/each}
        </tr>{/each}
    </tbody>
  </table>
</details>
