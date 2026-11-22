// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use super::ID;
use crate::model::{FileRole, ProviderId, SourceFile};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn sorted_entries(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).map(|e| e.path()).collect(),
        Err(_) => Vec::new(),
    };
    v.sort();
    v
}

fn name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub(super) fn discover(root: &Path) -> io::Result<Vec<SourceFile>> {
    let mut out = Vec::new();
    let mut projects: Vec<PathBuf> = fs::read_dir(root)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    projects.sort();
    for proj in projects {
        let key = name(&proj);
        for entry in sorted_entries(&proj) {
            let n = name(&entry);
            if entry.is_file() && n.ends_with(".jsonl") {
                let stem = n.trim_end_matches(".jsonl").to_string();
                let role = if n.starts_with("agent-") {
                    FileRole::Agent
                } else {
                    FileRole::Main
                };
                out.push(make(entry.clone(), role, &key, stem, None, None));
            } else if entry.is_dir() {
                walk_agents(&entry.join("subagents"), &key, &n, None, &mut out);
            }
        }
    }
    Ok(out)
}

fn make(
    path: PathBuf,
    role: FileRole,
    key: &str,
    session: String,
    agent_type: Option<String>,
    group: Option<String>,
) -> SourceFile {
    SourceFile {
        provider: ProviderId(ID.to_string()),
        path,
        role,
        project_key: key.to_string(),
        session_id: session,
        agent_type,
        agent_group: group,
    }
}

fn walk_agents(dir: &Path, key: &str, session: &str, wf: Option<&str>, out: &mut Vec<SourceFile>) {
    for entry in sorted_entries(dir) {
        let n = name(&entry);
        if entry.is_dir() {
            let child_wf = if wf.is_none() && name(dir) == "workflows" {
                Some(n.clone())
            } else {
                wf.map(str::to_string)
            };
            walk_agents(&entry, key, session, child_wf.as_deref(), out);
        } else if n.starts_with("agent-") && n.ends_with(".jsonl") {
            let (agent_type, phase) = agent_meta(&entry, &n);
            let group = phase.or_else(|| wf.map(str::to_string));
            out.push(make(
                entry,
                FileRole::Agent,
                key,
                session.to_string(),
                agent_type,
                group,
            ));
        }
    }
}

pub(super) fn source_of(root: &Path, path: &Path) -> Option<SourceFile> {
    let rel = path.strip_prefix(root).ok()?;
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    if !path.is_file() {
        return None;
    }
    let n = parts.last()?;
    match parts.len() {
        2 if n.ends_with(".jsonl") => {
            let role = if n.starts_with("agent-") {
                FileRole::Agent
            } else {
                FileRole::Main
            };
            Some(make(
                path.to_path_buf(),
                role,
                &parts[0],
                n.trim_end_matches(".jsonl").to_string(),
                None,
                None,
            ))
        }
        len if len >= 4
            && parts[2] == "subagents"
            && n.starts_with("agent-")
            && n.ends_with(".jsonl") =>
        {
            let dirs = &parts[3..len - 1];
            let wf = (1..dirs.len())
                .find(|&i| dirs[i - 1] == "workflows")
                .map(|i| dirs[i].clone());
            let (agent_type, phase) = agent_meta(path, n);
            let group = phase.or(wf);
            Some(make(
                path.to_path_buf(),
                FileRole::Agent,
                &parts[0],
                parts[1].clone(),
                agent_type,
                group,
            ))
        }
        _ => None,
    }
}

fn agent_meta(path: &Path, n: &str) -> (Option<String>, Option<String>) {
    let meta = path.with_file_name(format!("{}.meta.json", n.trim_end_matches(".jsonl")));
    let Ok(text) = fs::read_to_string(meta) else {
        return (None, None);
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        return (None, None);
    };
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
    (s("agentType"), s("workflowPhase"))
}
