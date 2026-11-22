// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

import './styles/app.css';
import { mount, type Component } from 'svelte';
import { ui, mode } from './lib/stores/ui.svelte';
import { installTips } from './lib/tips';
import { installThemeIcon } from './features/themes/icon.svelte';
import { installWall } from './features/glass/wall';

document.body.dataset.mode = mode;
// Windows 11 clips frameless windows to its own rounded corners; the page then leaves the corners to the OS.
if (/Windows/.test(navigator.userAgent)) document.documentElement.dataset.os = 'win';

installWall();
installTips();
ui.start();
installThemeIcon();

// The dynamic import keeps the dashboard code out of the widget's WebView2 heap.
const root = mode === 'compact'
  ? (await import('./widget/Widget.svelte')).default
  : (await import('./dashboard/Dashboard.svelte')).default;
mount(root as Component, { target: document.getElementById(mode)! });
