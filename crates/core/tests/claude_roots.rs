// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Own test binary, since it sets a process-wide variable.

use std::fs;
use std::path::PathBuf;
use tokenwhy_core::providers::claude::ClaudeCode;
use tokenwhy_core::Provider;

#[test]
fn config_dir_list_gives_every_existing_root() {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("claude_roots");
    let _ = fs::remove_dir_all(&d);
    for x in ["a", "b"] {
        fs::create_dir_all(d.join(x).join("projects")).unwrap();
    }
    fs::create_dir_all(d.join("c")).unwrap();
    let list = format!(
        "{}, {},{}",
        d.join("a").display(),
        d.join("b").display(),
        d.join("c").display()
    );
    std::env::set_var("CLAUDE_CONFIG_DIR", list);
    assert_eq!(
        ClaudeCode.default_roots(),
        vec![d.join("a").join("projects"), d.join("b").join("projects")]
    );
}
