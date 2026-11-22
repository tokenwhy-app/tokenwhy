// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

import { SvelteSet } from 'svelte/reactivity';
import type { Component } from 'svelte';
import {
  importCcusage, getSnapshot, setSetting, getSetting, getState, onSettings, onSnapshot, onStatus, onProgress,
  restartApp, onGlass,
} from '../bridge';
import type { EngineStatus, ShellState, Snapshot } from '../types';
import { look } from './look.svelte';
import { startTour } from '../../features/tour/tour';

export const TABS: [string, string][] = [['overview', 'Overview'], ['startup', 'Startup'], ['tools', 'Tools'], ['agents', 'Agents'], ['context', 'Context'], ['skills', 'Skills, memory']];
export const TAB_IDS = TABS.map((t) => t[0]);

export interface WrappedHandle { Button: Component; open: (opener: HTMLElement | null) => void }

const params = new URLSearchParams(location.search);
const VIEWS = ['A', 'B', 'C'];

export const mode: 'compact' | 'expanded' = (location.hash.slice(1) || params.get('mode')) === 'compact' ? 'compact' : 'expanded';

class Ui {
  readonly mode = mode;
  s = $state.raw<Snapshot | null>(null);
  status = $state<EngineStatus | null>(null);
  progress = $state<number | null>(null);
  project = $state<string | null>(null);
  provider = $state<string | null>(null);
  account = $state<string | null>(null);
  days = $state(Number(getSetting('window_days', '14')));
  anon = $state(getSetting('anonymise', 'false') === 'true');
  view = $state(VIEWS.includes(getSetting('compact_view', 'C')) ? getSetting('compact_view', 'C') : 'C');
  settings = $state<ShellState>({ money_view: getSetting('money_view', 'pct'), widget: false, corner: 'bottom-right', always_on_top: true, autostart: false });
  settingsOpen = $state(params.get('settings') === '1');
  tab = $state([params.get('tab'), getSetting('tab', '')].map((x) => (x === 'compaction' ? 'context' : x)).find((x) => x != null && TAB_IDS.includes(x)) || 'overview');
  open = new SvelteSet((params.get('open') || '').split(',').filter(Boolean));
  selStartup = $state<string | null>(null);
  selKind = $state<string | null>(null);
  openCat = $state<string | null>(params.get('cat'));
  importMsg = $state<string | null>(null);
  stateReady = $state(false);
  settingsApplied = $state(false);
  wrapped = $state.raw<WrappedHandle | null>(null);

  async load() {
    try { this.s = (await getSnapshot(this.days, this.project, this.mode === 'compact')) || this.s; } catch (e) { this.status = { kind: 'error', message: String(e) }; }
  }

  applySettings(st: ShellState | null | undefined) {
    if (!st) return;
    if (st.status) this.status = st.status;
    this.settings = { ...this.settings, ...st };
    look.apply();
    if (st.anonymise != null) this.anon = !!st.anonymise;
    if (st.provider !== undefined) this.provider = st.provider || null;
    if (st.account !== undefined) this.account = st.account || null;
    if (VIEWS.includes(st.compact_view)) this.view = st.compact_view;
    this.settingsApplied = true;
  }

  on = {
    view: (v: string) => { this.view = v; setSetting('compact_view', v); },
    days: (d: number) => { this.days = d; setSetting('window_days', d); this.load(); },
    project: (p: string | null) => { this.project = p; },
    provider: (p: string | null) => { this.provider = p; setSetting('provider', p); },
    account: (a: string | null) => { this.account = a; setSetting('account', a); },
    anon: (a: boolean) => { this.anon = a; setSetting('anonymise', a).then(() => this.load()); },
    startup: (p: string | null) => { this.selStartup = p; },
    tab: (t: string) => { this.tab = t; try { localStorage.setItem('tokenwhy.tab', t); } catch { /* storage may be blocked */ } },
    toggle: (k: string) => { if (this.open.has(k)) this.open.delete(k); else this.open.add(k); },
    kind: (k: string | null) => { this.selKind = k; },
    cat: (c: string) => { this.openCat = this.openCat === c ? null : c; },
    settings: () => { this.settingsOpen = !this.settingsOpen; },
    set: (k: string, v: unknown) => { this.settings[k] = v; setSetting(k, v); look.apply(); },
    co2: (c: unknown) => { this.settings.co2 = c; setSetting('co2', c).then(() => this.load()); },
    restart: () => restartApp(),
    tour: () => { this.settingsOpen = false; startTour(this, { replay: true }); },
    importCcusage: (path: string) => {
      this.importMsg = 'Reading...';
      importCcusage(path).then((n) => { this.importMsg = `${n} rows imported.`; }, (e) => { this.importMsg = String(e && e.message ? e.message : e); });
    },
  };

  start() {
    look.install(() => this.settings);
    onGlass((g) => { this.settings = { ...this.settings, glass: g }; look.apply(); });
    const th = params.get('theme'), thMode = params.get('mode'), perf = params.get('perf');
    if (th) this.settings.theme = th;
    if (thMode) this.settings.mode = thMode;
    if (perf) this.settings.running_performance = this.settings.performance = perf;
    look.apply();
    const v = params.get('view');
    if (v && VIEWS.includes(v)) this.view = v;
    onSnapshot((s) => { this.s = s; }, () => this.load());
    onSettings((st) => this.applySettings(st));
    onStatus((st) => { this.status = st; });
    onProgress((f) => { this.progress = f; });
    getState().then((st) => { this.stateReady = true; return st; }).then((st) => this.applySettings(th ? { ...st, theme: th, ...(thMode ? { mode: thMode } : {}), ...(perf ? { performance: perf, running_performance: perf } : {}) } : st));
    this.load();
  }
}

export type UiStore = Ui;
export const ui = new Ui();
