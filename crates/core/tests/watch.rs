#![cfg(feature = "watch")]
// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use std::fs;
use std::sync::mpsc;
use std::time::Duration;
use tokenwhy_core::watch::{watch_with, RecursiveMode};

#[test]
fn burst_gives_one_debounced_call() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("watch");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let (tx, rx) = mpsc::channel();
    let _w = watch_with(
        vec![(dir.clone(), RecursiveMode::Recursive)],
        |p: &std::path::Path| p.extension().is_some_and(|e| e == "json"),
        move |p| tx.send(p).unwrap(),
        Duration::from_millis(300),
        Duration::from_secs(60),
    )
    .unwrap();
    let f = dir.join("s.json");
    for i in 0..5 {
        fs::write(&f, format!("{i}\n")).unwrap();
        fs::write(dir.join("note.jsonl"), "x").unwrap();
    }
    let got = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("a callback within 5 s");
    assert_eq!(got.len(), 1);
    assert!(got[0].ends_with("s.json"));
    assert!(
        rx.recv_timeout(Duration::from_millis(800)).is_err(),
        "burst must collapse into one call"
    );
}
