// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

export function svgNode(text: string): SVGSVGElement | null {
  const doc = new DOMParser().parseFromString(text, 'image/svg+xml');
  const el = doc.documentElement;
  if (el.nodeName !== 'svg') return null;
  el.querySelectorAll('script, foreignObject').forEach((n) => n.remove());
  // The <title> would pop the browser's own tooltip over the card; screen readers get the same text as a label.
  const title = el.querySelector('title');
  if (title) { el.setAttribute('aria-label', (title.textContent || '').trim()); title.remove(); }
  el.removeAttribute('width'); el.removeAttribute('height');
  el.setAttribute('class', 'wrap-card');
  return document.importNode(el, true) as unknown as SVGSVGElement;
}

const still = () => matchMedia('(prefers-reduced-motion: reduce)').matches;
// Rust sizes the figure from the font's advances; the webview can render it wider. Measure the real box once the card
// is in the page and scale the figure down so it keeps the same margin on the right as on the left.
export function fitBig(svg: SVGSVGElement): void {
  const vb = svg.viewBox.baseVal;
  if (!vb || !vb.width) return;
  for (const big of svg.querySelectorAll<SVGTextElement>('[id^="big"]')) {
    const x = Number(big.getAttribute('x') || 0), max = vb.width - 2 * (x - vb.x);
    const w = big.getBBox().width;
    if (w > max) big.setAttribute('font-size', String(Number(big.getAttribute('font-size')) * max / w * 0.98));
  }
}

export function arrive(svg: SVGSVGElement, id: string, dir: string, again: boolean): void {
  if (still()) return;
  if (dir) svg.classList.add('from-' + dir);
  if (again) return;
  const big = svg.querySelector('#big');
  if (id === 'personality') {
    if (big) big.classList.add('wrap-in');
    svg.querySelectorAll<SVGElement>('[id^="line"]').forEach((t) => { t.classList.add('wrap-in'); t.style.animationDelay = '350ms'; });
  } else if (id !== 'hello') svg.querySelectorAll('[id^="big"]').forEach(countUp);
  const bars = [...svg.querySelectorAll('[id^="bar"]')].map((t) => t.nextElementSibling as SVGElement | null).filter((r): r is SVGElement => !!r && r.tagName === 'rect');
  bars.forEach((r, i) => { r.classList.add('wrap-grow'); r.style.animationDelay = (bars.length > 1 ? i * 350 / (bars.length - 1) : 0) + 'ms'; });
}

export function countUp(big: Element): void {
  // Only the figure's own text node counts: the unit is a smaller <tspan> beside it, and writing textContent on the
  // <text> would replace it with the unit at the figure's size ("33.0Btok" overflowing the card).
  const el = [...big.childNodes].find((c) => c.nodeType === Node.TEXT_NODE);
  if (!el) return;
  const final = el.textContent || '';
  const m = /^([^\dA-Za-z]*)(\d[\d,]*(?:\.\d+)?)(\D*)$/.exec(final);
  if (!m) return;
  const target = Number(m[2].replace(/,/g, ''));
  const dec = (m[2].split('.')[1] || '').length, commas = m[2].includes(',');
  const fmt = (v: number) => {
    const t = v.toFixed(dec);
    return commas ? Number(t).toLocaleString('en-US', { minimumFractionDigits: dec, maximumFractionDigits: dec }) : t;
  };
  const t0 = performance.now(), D = 1100;
  const step = (now: number) => {
    if (!big.isConnected) return;
    const k = Math.min(1, (now - t0) / D);
    if (k >= 1) { el.textContent = final; return; }
    el.textContent = m[1] + fmt(target * (1 - (1 - k) ** 3)) + m[3];
    requestAnimationFrame(step);
  };
  el.textContent = m[1] + fmt(0) + m[3];
  requestAnimationFrame(step);
}
