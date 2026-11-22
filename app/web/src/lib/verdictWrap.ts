// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

// One line if it fits; else the action starts its own line, unless that takes a third line, then plain wrapping.
export function verdictWrap(p: HTMLElement) {
  let frame = 0;
  const lines = () => Math.round(p.getBoundingClientRect().height / parseFloat(getComputedStyle(p).lineHeight));
  const apply = () => {
    frame = 0;
    p.classList.remove('flow');
    if (lines() > 2) p.classList.add('flow');
  };
  const sizes = new ResizeObserver(() => { if (!frame) frame = requestAnimationFrame(apply); });
  sizes.observe(p.parentElement ?? p);
  apply();
  return { destroy() { cancelAnimationFrame(frame); sizes.disconnect(); } };
}
