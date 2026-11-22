// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

// To add a shared theme: write app/themes/tokens/<id>.css, add its line to app/themes/registry.ts and its @import
// in styles/app.css. settings.rs migrates saves that stored a variant id.
import { FAMILIES, familyOf } from '../../../themes/registry';

export { THEMES, FAMILIES, familyOf } from '../../../themes/registry';
export type { Theme, Family } from '../../../themes/registry';

export const MODES: [string, string][] = [['system', 'System'], ['light', 'Light'], ['dark', 'Dark']];

export function effectiveTheme(family: string, mode: string, eco: boolean, osDark: boolean): string {
  const f = familyOf(family);
  const dark = mode === 'dark' || (mode !== 'light' && osDark);
  const t = dark ? f.dark : f.light;
  return t.needs === 'normal' && eco ? FAMILIES[0][t.scheme].id : t.id;
}
