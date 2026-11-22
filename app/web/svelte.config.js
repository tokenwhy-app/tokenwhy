// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

/** @type {import('svelte/compiler').CompileOptions} */
const config = {
  preprocess: vitePreprocess(),
};

export default config;
