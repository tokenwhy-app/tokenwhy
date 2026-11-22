// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

// Windows' blur turns flat on an inactive window and its acrylic adds a pale layer, so the page draws the wallpaper itself.
// `wall` stays a plain variable: a data URL of several MB must not sit in a reactive proxy.
import { getWallpaper, onMoved, onScale } from '../../lib/bridge';

const WALL_PAD = 48;
const WALL_EVERY = 5 * 60000;
let wall: any = null, wallTimer: ReturnType<typeof setTimeout> | undefined;

let pending: Promise<void> | null = null;

export function loadWallpaper(): Promise<void> {
  if (pending) return pending;
  clearTimeout(wallTimer);
  wallTimer = setTimeout(loadWallpaper, WALL_EVERY);
  return (pending = fetchWallpaper().finally(() => { pending = null; }));
}

async function fetchWallpaper() {
  const w = await getWallpaper(wall && wall.mtime);
  const root = document.documentElement, el = document.getElementById('wall')!;
  if (!w) { wall = null; delete root.dataset.wall; return; }
  if (w.data) {
    try {
      const img = new Image();
      img.src = w.data;
      await img.decode();
      el.style.backgroundImage = 'url("' + w.data + '")';
      wall = { ...w, iw: img.naturalWidth, ih: img.naturalHeight };
    } catch { wall = null; delete root.dataset.wall; return; }
  } else wall = { ...wall, ...w, data: null };
  root.dataset.wall = 'on';
  placeWall(w.x, w.y);
}

export function stopWallpaper() {
  clearTimeout(wallTimer);
  delete document.documentElement.dataset.wall;
}

function placeWall(x: number, y: number) {
  if (!wall) return;
  wall.x = x; wall.y = y;
  const m = wall.monitors.find((r: any) => x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h) || wall.monitors[0];
  if (!m) return;
  let dw = wall.iw, dh = wall.ih;
  if (wall.style === 'stretch') { dw = m.w; dh = m.h; } else if (wall.style !== 'center') {
    const k = (wall.style === 'fit' ? Math.min : Math.max)(m.w / wall.iw, m.h / wall.ih);
    dw = wall.iw * k; dh = wall.ih * k;
  }
  const k = wall.scale, g = document.getElementById('wall')!.style;
  g.backgroundSize = dw / k + 'px ' + dh / k + 'px';
  g.backgroundPosition = (m.x + (m.w - dw) / 2 - x) / k + WALL_PAD + 'px ' + ((m.y + (m.h - dh) / 2 - y) / k + WALL_PAD) + 'px';
}

export function installWall() {
  onMoved(placeWall);
  onScale((k) => { if (wall) { wall.scale = k; placeWall(wall.x, wall.y); } });
}
