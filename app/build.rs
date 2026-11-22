// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

fn main() {
    // tauri-build only watches tauri.conf.json and capabilities/, so a swapped icon file would ship stale.
    println!("cargo:rerun-if-changed=icons");
    tauri_build::build()
}
