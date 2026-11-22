// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { CONSENT_VERSION } from './consent';

export interface ShareState {
  enabled: boolean; prompted: boolean; last_sent_week: string | null; last_status: string | null;
  retired: boolean; queued: number; pending_delete: boolean; has_id: boolean;
}

export const share = $state<{
  available: boolean; st: ShareState | null; text: string | null; textTitle: string; msg: string | null; asking: boolean;
}>({ available: false, st: null, text: null, textTitle: '', msg: null, asking: false });

const call = <T>(cmd: string, args?: Record<string, unknown>): Promise<T> =>
  isTauri() ? invoke<T>(cmd, args) : Promise.reject(new Error('Sharing needs the app.'));
const err = (e: unknown) => { share.msg = String(e && (e as Error).message ? (e as Error).message : e); };

let probed: Promise<boolean> | null = null;
export function probe(): Promise<boolean> {
  if (!probed) probed = (async () => {
    if (!isTauri()) return false;
    share.available = await invoke('share_available').then((v) => v === true, () => false);
    if (share.available) {
      await refresh();
      listen<ShareState>('share_state', (e) => { share.st = e.payload; });
    }
    return share.available;
  })();
  return probed;
}

export async function refresh() { try { share.st = await call<ShareState>('share_state'); } catch (e) { err(e); } }

export async function turnOn() {
  try { share.st = await call<ShareState>('share_set', { enabled: true, consentVersion: CONSENT_VERSION, delete: false }); share.msg = null; } catch (e) { err(e); }
}

export async function turnOff(del: boolean) {
  share.asking = false;
  try { share.st = await call<ShareState>('share_set', { enabled: false, consentVersion: null, delete: del }); share.msg = del ? 'Sharing is off and the weeks already sent are deleted.' : 'Sharing is off.'; } catch (e) { err(e); await refresh(); }
}

export async function dismiss() { try { await call('share_dismiss'); await refresh(); } catch (e) { err(e); } }

export async function showPreview() {
  try { share.textTitle = 'What is sent'; share.text = await call<string>('share_preview'); } catch (e) { err(e); }
}

export async function showLog() {
  try { share.textTitle = 'Everything sent so far'; share.text = (await call<string>('share_log')) || 'Nothing sent yet.'; } catch (e) { err(e); }
}

export async function saveFile() {
  try { share.msg = 'Saved to ' + (await call<string>('share_export')); } catch (e) { err(e); }
}

export async function deleteSent() {
  try { const n = await call<number>('share_delete'); share.msg = `Deleted ${n} weeks from the server.`; } catch (e) { err(e); }
  await refresh();
}

export function statusLine(st: ShareState | null): string | null {
  if (!st) return null;
  if (st.retired) return 'This version can no longer share, update tokenwhy.';
  if (st.pending_delete) return 'Deleting what you sent: waiting for a connection, will retry.';
  if (!st.enabled) return null;
  if (st.queued > 0) return 'Waiting for a connection, will retry.';
  if (st.last_sent_week) {
    const wk = st.last_sent_week.split('-W')[1];
    const how = st.last_status && st.last_status.startsWith('rejected') ? 'refused by the server' : 'accepted';
    return `Last sent: week ${Number(wk)}, ${how}.`;
  }
  return 'Nothing sent yet: the first summary goes after the next full scan.';
}

export function aboutNote(): string | null {
  return share.st && share.st.enabled ? 'tokenwhy reads the log files on this computer and sends a weekly rounded summary because you turned on sharing.' : null;
}
