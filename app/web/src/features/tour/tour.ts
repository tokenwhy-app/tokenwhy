// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

// Anchors are found again by selector each time the tour is placed, never kept as nodes.
import { tick } from 'svelte';
import { setSetting } from '../../lib/bridge';
import type { UiStore } from '../../lib/stores/ui.svelte';
import { view } from './run.svelte';

export const STEPS = [
  { id: 'verdict', title: 'Your verdict', sel: '.verdict', text: 'This line says how much you could save each week and the biggest reason.' },
  { id: 'causes', title: 'Top causes', sel: '.grid.causes', text: 'Each card is one cause of waste with its weekly cost and a fix to apply.' },
  { id: 'filters', title: 'Filters', sel: '.top .controls .seg, .top .controls .sel-wrap', text: 'The stats get recalculated based on the agent, account, days or project that you choose.' },
  { id: 'tabs', title: 'Detail tabs', sel: 'nav.tabs', text: 'Navigate through tabs in order to learn more.' },
  { id: 'settings', title: 'Settings', sel: '.top .icon-btn[aria-expanded]', text: 'Turn on the widget, change theme, optimize performances and more...' },
];
type Step = (typeof STEPS)[number];

const PAD = 6, GAP = 12, EDGE = 16;
const forced = Number(new URLSearchParams(location.search).get('tour')) || 0;
let run: { ui: UiStore; id: string | null; back: Element | null; start: number } | null = null;
let shown = false;

function rectOf(sel: string) {
  const rs = [...document.querySelectorAll(sel)].map((e) => e.getBoundingClientRect()).filter((r) => r.width && r.height);
  if (!rs.length) return null;
  const l = Math.min(...rs.map((r) => r.left)), t = Math.min(...rs.map((r) => r.top));
  return { left: l, top: t, width: Math.max(...rs.map((r) => r.right)) - l, height: Math.max(...rs.map((r) => r.bottom)) - t };
}
const live = () => STEPS.filter((s) => rectOf(s.sel));
const still = () => matchMedia('(prefers-reduced-motion: reduce)').matches || document.documentElement.dataset.perf === 'eco';

export function maybeStartTour(ui: UiStore): void {
  if (run || shown || !ui.s || (ui.s as any).progress < 1 || ui.settingsOpen) return;
  if (!forced && !(ui.stateReady && ui.settings.first_run_done === false && ui.status && ui.status.kind === 'ok')) return;
  startTour(ui, { replay: false });
}

export function startTour(ui: UiStore, { replay }: { replay: boolean }): void {
  if (run) return;
  shown = true;
  run = { ui, id: null, back: document.activeElement, start: replay ? 0 : Math.max(0, forced - 1) };
  addEventListener('keydown', onKey, true);
  addEventListener('scroll', onScroll, true);
  addEventListener('resize', onScroll);
  if (ui.tab !== 'overview') ui.on.tab('overview');
  tick().then(() => requestAnimationFrame(() => go(0)));
}

function go(delta: number): void {
  if (!run) return;
  const steps = live();
  if (!steps.length) return end();
  const cur = run.id;
  let i = steps.findIndex((s) => s.id === cur);
  i = i < 0 ? Math.min(run.start, steps.length - 1) : i + delta;
  if (i < 0) i = 0;
  if (i >= steps.length) return end();
  run.id = steps[i].id;
  const r = rectOf(steps[i].sel)!;
  if (r.top < 0 || r.top + r.height > innerHeight) {
    document.querySelector(steps[i].sel)!.scrollIntoView({ block: 'center', behavior: still() ? 'auto' : 'smooth' });
  }
  paint(steps, i);
}

function paint(steps: Step[], i: number) {
  const s = steps[i];
  Object.assign(view, { id: s.id, title: s.title, text: s.text, i, n: steps.length, go, end });
  tick().then(() => {
    placeTour();
    const el = document.getElementById('tour');
    const next = el && el.querySelector<HTMLButtonElement>('.tour-next');
    if (next) next.focus({ preventScroll: true });
    const card = el && el.querySelector('.tour-card');
    if (card) requestAnimationFrame(() => card.classList.add('in'));
  });
}

export function placeTour(): void {
  if (!run || !run.id) return;
  const id = run.id;
  const el = document.getElementById('tour');
  const step = STEPS.find((s) => s.id === id)!;
  const r = rectOf(step.sel);
  if (!el || !r) { run.id = null; run.start = 0; return go(0); }
  view.ring = { left: r.left - PAD, top: r.top - PAD, width: r.width + 2 * PAD, height: r.height + 2 * PAD };
  const card = el.querySelector<HTMLElement>('.tour-card');
  if (!card) return;
  const cw = card.offsetWidth, ch = card.offsetHeight;
  const below = r.top + r.height + PAD + GAP;
  const top = below + ch <= innerHeight - EDGE ? below : Math.max(EDGE, r.top - PAD - GAP - ch);
  const left = Math.min(Math.max(EDGE, r.left), Math.max(EDGE, innerWidth - EDGE - cw));
  view.card = { left, top };
}

function end(): void {
  if (!run) return;
  const { ui, back } = run;
  run = null;
  removeEventListener('keydown', onKey, true);
  removeEventListener('scroll', onScroll, true);
  removeEventListener('resize', onScroll);
  Object.assign(view, { id: null, ring: null, card: null });
  ui.settings.first_run_done = true;
  setSetting('first_run_done', true);
  const to = back && back.isConnected && back !== document.body ? back : document.querySelector('.top .icon-btn[aria-expanded]');
  if (to) (to as HTMLElement).focus();
}

let frame = 0;
function onScroll() {
  if (frame) return;
  frame = requestAnimationFrame(() => { frame = 0; placeTour(); });
}

function onKey(e: KeyboardEvent) {
  const el = document.getElementById('tour');
  if (!run || !el) return;
  const k = e.key;
  if (k === 'Escape') { e.preventDefault(); e.stopPropagation(); end(); return; }
  if (k === 'ArrowRight') { e.preventDefault(); go(1); return; }
  if (k === 'ArrowLeft') { e.preventDefault(); go(-1); return; }
  if (k === 'Enter' && !(e.target instanceof HTMLButtonElement && el.contains(e.target))) { e.preventDefault(); go(1); return; }
  if (k === 'Tab') {
    const bs = [...el.querySelectorAll('button')];
    const at = bs.indexOf(document.activeElement as HTMLButtonElement);
    e.preventDefault();
    bs[(at < 0 ? 0 : at + (e.shiftKey ? bs.length - 1 : 1)) % bs.length].focus();
  }
}
