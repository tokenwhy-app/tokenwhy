// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

import { invoke as tauriInvoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { EngineStatus, ShellState, Snapshot, WrappedArgs, WrappedResult } from './types';

export const inApp: boolean = isTauri();
const invoke = inApp ? tauriInvoke : null;
const win = () => (inApp ? getCurrentWindow() : null);

const LS = {
  get(k: string): string | null { try { return localStorage.getItem('tokenwhy.' + k); } catch { return null; } },
  set(k: string, v: unknown) { try { localStorage.setItem('tokenwhy.' + k, String(v)); } catch { /* storage may be blocked */ } },
};

function on<T>(event: string, cb: (p: T) => void) {
  if (inApp) listen<T>(event, (e) => cb(e.payload));
}

// The widget only reads: the `snapshot` command would write its own window and project over the dashboard's.
export async function getSnapshot(windowDays: number, project: string | null, widget = false): Promise<Snapshot> {
  if (invoke && widget) return (await invoke<any>('get_state')).snapshot;
  if (invoke) return invoke<Snapshot>('snapshot', { windowDays, project: project || null });
  const url = new URLSearchParams(location.search).get('snapshot') || 'sample-snapshot.json';
  const r = await fetch(url);
  return r.json();
}

export function setSetting(key: string, value: unknown): Promise<void> {
  LS.set(key, value);
  if (invoke) return invoke<void>('set_setting', { key, value }).catch(() => {});
  return Promise.resolve();
}
export function getSetting(key: string, dflt: string): string { const v = LS.get(key); return v == null ? dflt : v; }
// Relaunches the app; WebView2 switches only change with a new process.
export function restartApp(): Promise<void> { return invoke ? invoke<void>('restart').catch(() => {}) : Promise.resolve(); }
export function importCcusage(path: string): Promise<number> { return invoke ? invoke<number>('import_ccusage', { path }) : Promise.reject(new Error('Import needs the app.')); }
export function rescan(): Promise<void> { return invoke ? invoke<void>('rescan').catch(() => {}) : Promise.resolve(); }
export function openLog(): Promise<void> { return invoke ? invoke<void>('open_log').catch(() => {}) : Promise.resolve(); }
export function onStatus(cb: (s: EngineStatus) => void) { on('engine_status', cb); }
export function onProgress(cb: (f: number) => void) { on('scan_progress', cb); }

export function onSnapshot(cb: (s: Snapshot) => void, poll: () => void) {
  // Scoped to this window: a global listener also receives the snapshot sent to the other window.
  if (inApp) { getCurrentWindow().listen<Snapshot>('snapshot', (e) => cb(e.payload)); return; }
  setInterval(poll, 5000);
}
export async function getState(): Promise<ShellState> {
  if (invoke) { try { const r = await invoke<any>('get_state'); return { ...r.settings, running_performance: r.running_performance, glass: r.glass, version: r.version, status: r.status }; } catch { /* fall through */ } }
  const b = (k: string, d: boolean) => (LS.get(k) == null ? d : LS.get(k) === 'true');
  const perf = LS.get('performance') || 'normal';
  return { performance: perf, running_performance: LS.get('running_performance') || perf, theme: LS.get('theme') || 'system', widget: b('widget', false), corner: LS.get('corner') || 'bottom-right', always_on_top: b('always_on_top', true), autostart: b('autostart', false), glass: new URLSearchParams(location.search).get('glass') || 'none' };
}
export function onGlass(cb: (g: string) => void) { on('glass', cb); }
export function onSettings(cb: (s: ShellState) => void) { on('settings', cb); }
export function openDashboard(): Promise<void> { return invoke ? invoke<void>('set_view', { view: 'expanded' }).catch(() => {}) : Promise.resolve(); }
export function windowAction(act: 'minimize' | 'toggleMaximize' | 'close'): Promise<void> {
  const w = win();
  return w ? w[act]().catch(() => {}) : Promise.resolve();
}
export function startDrag(): Promise<void> {
  const w = win();
  return w ? w.startDragging().catch(() => {}) : Promise.resolve();
}
export function getWallpaper(known: number | null): Promise<any> { return invoke ? invoke('wallpaper', { known: known ?? null }).catch(() => null) : Promise.resolve(null); }
export function onMoved(cb: (x: number, y: number) => void) {
  const w = win();
  if (w) w.onMoved((e) => cb(e.payload.x, e.payload.y));
}
export function onScale(cb: (scale: number) => void) {
  const w = win();
  if (w) w.onScaleChanged((e) => cb(e.payload.scaleFactor));
}

// In a browser, ?wrapped=<json> serves sample cards and ?wrapped_fonts=<dir> the card fonts.
const WP = new URLSearchParams(location.search);
export function wrappedAvailable(): Promise<boolean> {
  if (invoke) return invoke('wrapped_available').then((v) => v === true, () => false);
  return Promise.resolve(WP.has('wrapped'));
}
const FONT_FILES = ['twcard-Regular.ttf', 'twcard-Heavy.ttf'];
export async function wrappedFont(i: number): Promise<ArrayBuffer | number[] | null> {
  if (invoke) return invoke('wrapped_font', { which: i });
  const dir = WP.get('wrapped_fonts');
  if (!dir) return null;
  const r = await fetch(dir.replace(/\/?$/, '/') + FONT_FILES[i]);
  return r.ok ? r.arrayBuffer() : null;
}
export async function wrappedCards(args: WrappedArgs): Promise<WrappedResult> {
  if (invoke) return invoke<WrappedResult>('wrapped', { ...args });
  const r: WrappedResult = await (await fetch(WP.get('wrapped')!)).json();
  const c = args.colors, map: Record<string, string> = { ...c, ...Object.fromEntries(c.series.map((v, i) => ['s' + (i + 1), v])) };
  for (const k of r.cards) k.svg = k.svg.replace(/\{\{(\w+)\}\}/g, (m, t) => map[t] || m);
  return r;
}
export function wrappedSave(args: WrappedArgs, which: number | null): Promise<string> {
  return invoke ? invoke<string>('wrapped_save', { ...args, which }) : Promise.reject(new Error('Saving needs the app.'));
}

export function credits(): Promise<any> { return invoke ? invoke('credits').catch(() => null) : Promise.resolve(null); }
export function openCredit(url: string): Promise<void> { return invoke ? invoke<void>('open_credit', { url }).catch(() => {}) : Promise.resolve(); }
export function setThemeIcon(variant: string): Promise<void> { return invoke ? invoke<void>('theme_icon', { variant }).catch(() => {}) : Promise.resolve(); }
export async function mcpSetupLine(): Promise<string> {
  if (!invoke) throw new Error('MCP setup needs the app.');
  const exe = await invoke<string>('exe_path');
  return `claude mcp add tokenwhy -- "${exe}" mcp`;
}
export function agentBrief(): Promise<string> { return invoke ? invoke<string>('agent_brief') : Promise.reject(new Error('The brief needs the app.')); }
export type SpendScale = 'day' | 'week' | 'month';
export interface SpendBucket { start_ms: number; cost: number | null; tokens: number; partial: boolean }
export function spendSeries(scale: SpendScale, endMs: number, count: number, project: string | null, provider: string | null, account: string | null): Promise<SpendBucket[] | null> {
  if (!invoke) return Promise.resolve(null);
  return invoke<SpendBucket[]>('spend_series', { scale, endMs, count, project, provider, account }).catch(() => null);
}
export const spendScale = {
  get(): SpendScale { const v = LS.get('spend_scale'); return v === 'week' || v === 'month' ? v : 'day'; },
  set(v: SpendScale) { LS.set('spend_scale', v); },
};

export function fitWidget(width: number | null, height: number): void {
  if (invoke) invoke<void>('fit_widget', { width, height }).catch(() => {});
}
