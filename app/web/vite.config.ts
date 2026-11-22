// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

import { defineConfig, searchForWorkspaceRoot, type Plugin } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Same string as app/tauri.conf.json; added to the built page only so `vite dev` can inject styles.
const CSP =
  "default-src 'self'; connect-src 'self' ipc: http://ipc.localhost; style-src 'self'; img-src 'self' data:; script-src 'self'";

const cspMeta = (): Plugin => ({
  name: "tokenwhy-csp",
  apply: "build",
  transformIndexHtml: () => [
    { tag: "meta", attrs: { "http-equiv": "Content-Security-Policy", content: CSP }, injectTo: "head-prepend" },
  ],
});

export default defineConfig({
  plugins: [svelte(), cspMeta()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/app/src/**", "**/target/**"] },
    // The shared theme package sits next to app/web, outside the default serving root.
    fs: { allow: [searchForWorkspaceRoot(process.cwd()), "../themes"] },
  },
  build: {
    outDir: "../ui-dist",
    emptyOutDir: true,
    target: "chrome111",
    cssTarget: "chrome111",
  },
});
