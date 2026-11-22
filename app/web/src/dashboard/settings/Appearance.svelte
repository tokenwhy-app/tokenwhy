<!--
  Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
  SPDX-License-Identifier: Apache-2.0
  Redistributions must keep the NOTICE file and credit tokenwhy.
-->
<script lang="ts">
  import Seg from "../../lib/components/Seg.svelte";
  import ThemePicker from "../../features/themes/ThemePicker.svelte";
  import GlassNote from "../../features/glass/GlassNote.svelte";
  import { FAMILIES, MODES, familyOf } from "../../lib/themes";
  import { ui } from "../../lib/stores/ui.svelte";

  const st = $derived(ui.settings);
  const perf = $derived(st.performance || "normal");
  const running = $derived(st.running_performance || perf);
  const fam = $derived(familyOf(st.theme || ""));
</script>

<div class="settings-col">
  <div class="field field-theme">
    <span>Theme</span>
    <ThemePicker cur={fam.id} />
  </div>
  <div class="field">
    <span>Mode</span>
    <Seg
      items={MODES}
      cur={MODES.some(([m]) => m === st.mode) ? st.mode : "system"}
      on={(v) => ui.on.set("mode", v)}
      label="Mode"
    />
  </div>
  <GlassNote family={fam.id} {running} />
  <h3 class="more-h">More</h3>
  <div class="field" data-tip={perf === "eco" ? "Hardware acceleration off" : "Hardware acceleration on"}>
    <span>Performance</span>
    <Seg
      items={[
        ["eco", "Eco"],
        ["normal", "Normal"],
      ]}
      cur={perf}
      on={(v) => ui.on.set("performance", v)}
      label="Performance"
    />
  </div>
  {#if perf !== running}<div class="field restart">
      <span>Restart to apply</span>
      <button class="icon-btn" onclick={ui.on.restart}>Restart now</button>
    </div>{/if}
</div>
