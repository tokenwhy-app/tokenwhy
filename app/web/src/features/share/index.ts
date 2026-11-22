// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

import { mount } from 'svelte';
import { probe } from './store.svelte';
import ConsentModal from './ConsentModal.svelte';
import TextModal from './TextModal.svelte';

export async function load(): Promise<null> {
  if (!(await probe())) return null;
  mount(ConsentModal, { target: document.body });
  mount(TextModal, { target: document.body });
  return null;
}

export { aboutNote } from './store.svelte';
