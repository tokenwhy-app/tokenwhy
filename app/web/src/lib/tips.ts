// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

let tip: HTMLDivElement | null = null;
function showTip(el: Element, x: any, y: any) {
  if (!tip) { tip = document.createElement('div'); tip.className = 'tip'; tip.setAttribute('role', 'tooltip'); document.body.append(tip); }
  const [head, ...rest] = el.getAttribute('data-tip')!.split('\n');
  tip.replaceChildren();
  const b = document.createElement('strong'); b.textContent = head; tip.append(b);
  for (const l of rest) { const d = document.createElement('span'); d.textContent = l; tip.append(d); }
  tip.classList.add('on');
  const r = tip!.getBoundingClientRect();
  const left = Math.min(Math.max(8, x - r.width / 2), innerWidth - r.width - 8);
  const top = y - r.height - 12 < 8 ? y + 16 : y - r.height - 12;
  tip!.style.left = left + 'px'; tip!.style.top = top + 'px';
}
function hideTip() { if (tip) tip.classList.remove('on'); }
export function installTips() {
  const at = (e: any): Element | null => e.target.closest && e.target.closest('[data-tip]');
  document.addEventListener('pointermove', (e: any) => { const el = at(e); if (el) showTip(el, e.clientX, e.clientY); else hideTip(); });
  document.addEventListener('focusin', (e: any) => {
    const el = at(e); if (!el) return hideTip();
    const r = el.getBoundingClientRect(); showTip(el, r.left + r.width / 2, r.top);
  });
  document.addEventListener('focusout', hideTip);
  document.addEventListener('scroll', hideTip, true);
}

