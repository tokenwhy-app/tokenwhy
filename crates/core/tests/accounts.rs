// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use std::fs;
use std::path::{Path, PathBuf};
use tokenwhy_core::account::UNATTRIBUTED;
use tokenwhy_core::history::By;
use tokenwhy_core::pipeline::{self, Pipeline};
use tokenwhy_core::*;

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let t = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &t);
        } else {
            fs::copy(e.path(), t).unwrap();
        }
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/claude")
        .join(name)
}

fn snap(pipe: &Pipeline, account: Option<&str>) -> Snapshot {
    let now = pipeline::now_ms();
    let mut q = pipeline::query(None, now, None, AnonLevel::Off);
    q.account = account.map(str::to_string);
    pipe.report(&Settings::default(), &q, now).0
}

fn cost(s: &Snapshot) -> f64 {
    s.sessions.iter().filter_map(|r| r.cost_usd.value).sum()
}

/// One root alone, with no account: the oracle for each account's share.
fn alone(root: &Path) -> Snapshot {
    let mut p = Pipeline::with_providers(provider::all(), None);
    let roots = p
        .roots(&[(Some("claude-code".into()), root.to_path_buf())])
        .unwrap();
    p.scan(&roots, &|_| {}).unwrap();
    snap(&p, None)
}

#[test]
fn named_folder_and_login_log_split_sessions() {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("accounts");
    let _ = fs::remove_dir_all(&d);
    let perso = d.join("perso").join("projects");
    let pro = d.join("pro");
    copy_dir(&fixture("resume_copy"), &perso);
    copy_dir(&fixture("workflow_agents"), &pro.join("projects"));
    let store = d.join("store");
    fs::create_dir_all(&store).unwrap();
    fs::write(
        store.join("accounts.log"),
        "1000\tclaude-code\tclaude-code:aaaa\tPerso\n",
    )
    .unwrap();

    let mut pipe = Pipeline::with_providers(provider::all(), Some(store.clone()));
    pipe.set_accounts(
        &[("claude-code".into(), pro.clone(), "Pro".into())],
        Default::default(),
    );
    let roots = pipe
        .roots(&[(Some("claude-code".into()), perso.clone())])
        .unwrap();
    assert_eq!(roots.len(), 2, "the named folder joins the roots");
    pipe.scan(&roots, &|_| {}).unwrap();

    let all = snap(&pipe, None);
    let ids: Vec<&str> = all.accounts.iter().map(|a| a.label.as_str()).collect();
    assert_eq!(ids, ["Perso", "Pro"]);
    let pro_id = all.accounts[1].id.clone();

    let (p, o) = (
        snap(&pipe, Some("claude-code:aaaa")),
        snap(&pipe, Some(&pro_id)),
    );
    let (p1, o1) = (alone(&perso), alone(&pro.join("projects")));
    assert_eq!(p.sessions.len(), p1.sessions.len());
    assert_eq!(o.sessions.len(), o1.sessions.len());
    assert!((cost(&p) - cost(&p1)).abs() < 1e-9 && (cost(&o) - cost(&o1)).abs() < 1e-9);
    assert!((cost(&all) - cost(&p) - cost(&o)).abs() < 1e-9);
    assert!(snap(&pipe, Some(UNATTRIBUTED)).sessions.is_empty());

    let h = pipe.history();
    let day_cost = |a: Option<&str>| -> f64 {
        h.daily_for(0, i64::MAX, By::None, None, None, a)
            .iter()
            .filter_map(|r| r.cost_usd.value)
            .sum()
    };
    let total = day_cost(None);
    assert!(total > 0.0);
    assert!((day_cost(Some("claude-code:aaaa")) + day_cost(Some(&pro_id)) - total).abs() < 1e-9);
    assert_eq!(day_cost(Some(UNATTRIBUTED)), 0.0);
}

#[test]
fn first_login_seen_leaves_older_sessions_unattributed() {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("accounts_login");
    let _ = fs::remove_dir_all(&d);
    let root = d.join("cfg").join("projects");
    copy_dir(&fixture("resume_copy"), &root);
    fs::write(
        d.join("cfg").join(".claude.json"),
        r#"{"oauthAccount": {"accountUuid": "u-1", "emailAddress": "me@example.com", "organizationName": "Acme"}}"#,
    )
    .unwrap();
    let store = d.join("store");
    let mut pipe = Pipeline::with_providers(provider::all(), Some(store.clone()));
    let roots = pipe.roots(&[(Some("claude-code".into()), root)]).unwrap();
    pipe.scan(&roots, &|_| {}).unwrap();
    assert!(pipe.check_accounts(pipeline::now_ms()));
    assert!(
        !pipe.check_accounts(pipeline::now_ms()),
        "same file, no second switch"
    );
    let log = fs::read_to_string(store.join("accounts.log")).unwrap();
    assert_eq!(log.lines().count(), 1);
    assert!(log.contains("\tAcme") && !log.contains("example.com") && !log.contains("u-1"));
    let h = pipe.history();
    let un: f64 = h
        .daily_for(0, i64::MAX, By::None, None, None, Some(UNATTRIBUTED))
        .iter()
        .filter_map(|r| r.cost_usd.value)
        .sum();
    let all: f64 = h
        .daily(0, i64::MAX, By::None, None, None)
        .iter()
        .filter_map(|r| r.cost_usd.value)
        .sum();
    assert!(all > 0.0 && (un - all).abs() < 1e-9);
}
