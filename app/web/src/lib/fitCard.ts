// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

// Past the hard cap a card scrolls, so a scrollbar always hides at least 20 % of the content.
const GROW = 1.2;

export function fitCard(card: HTMLElement) {
  const body = card.querySelector<HTMLElement>(':scope > .card-body');
  if (!body) return;
  let frame = 0;
  const apply = () => {
    frame = 0;
    const cap = parseFloat(getComputedStyle(card).getPropertyValue('--h'));
    if (!cap) { card.style.maxHeight = ''; return; }
    const natural = card.offsetHeight - body.clientHeight + body.scrollHeight;
    card.style.maxHeight = natural > cap && natural <= cap * GROW ? 'none' : '';
  };
  const queue = () => { if (!frame) frame = requestAnimationFrame(apply); };
  const sizes = new ResizeObserver(queue);
  const watch = () => { sizes.disconnect(); sizes.observe(card); for (const c of body.children) sizes.observe(c); queue(); };
  const tree = new MutationObserver(watch);
  tree.observe(body, { childList: true, subtree: true });
  watch();
  return { destroy() { cancelAnimationFrame(frame); sizes.disconnect(); tree.disconnect(); } };
}
