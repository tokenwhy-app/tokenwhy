// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

// Removing a feature: delete its folder, its line here and, for tour, its @import in styles/app.css.
import Co2Panel from './co2/Co2Panel.svelte';
import Co2Kpis from './co2/Co2Kpis.svelte';
import { co2Line } from './co2/co2Line';
import AccountsPanel from './accounts/AccountsPanel.svelte';
import AccountChips from './accounts/AccountChips.svelte';
import PlanLine from './plan/PlanLine.svelte';
import { planLine } from './plan/planLine';
import CcusageImport from './ccusage/CcusageImport.svelte';
import { load as wrappedLoad } from './wrapped/index';
import SharePanel from './share/SharePanel.svelte';
import { load as shareLoad, aboutNote as shareAbout } from './share/index';

export const features = {
  settingsPanels: [
    { section: 'data', panel: AccountsPanel },
    { section: 'data', panel: SharePanel },
    { section: 'data', panel: Co2Panel },
    { section: 'data', panel: CcusageImport },
  ],
  headerChips: [AccountChips],
  overviewKpis: [Co2Kpis],
  overviewLines: [PlanLine],
  // Plan limits stay out of the widget: the user reads them in Claude Code with /usage.
  compactLines: [co2Line],
  lazy: [wrappedLoad, shareLoad],
  aboutNotes: [shareAbout],
};
