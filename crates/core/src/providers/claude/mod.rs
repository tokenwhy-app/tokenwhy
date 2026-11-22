// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

pub(crate) mod account;
mod dedup;
mod discover;
mod parse;

use crate::model::{
    Capabilities, CostSource, FileRecords, IdHash, Level, ParseResume, ParsedFile, ProviderId,
    SourceFile, Vendor,
};
use crate::provider::Provider;
use std::io;
use std::path::{Path, PathBuf};

pub const ID: &str = "claude-code";

pub struct ClaudeCode;

impl Provider for ClaudeCode {
    fn id(&self) -> ProviderId {
        ProviderId(ID.to_string())
    }

    fn label(&self) -> &'static str {
        "Claude Code"
    }

    fn caps(&self) -> Capabilities {
        Capabilities {
            turns: Level::Full,
            cache_read: Level::Full,
            cache_write: Level::Full,
            tool_outputs: Level::Full,
            skills: Level::Full,
            startup: Level::Full,
            memory: Level::Full,
            subagents: Level::Full,
            compactions: Level::Full,
            mcp: Level::Full,
            cost: CostSource::Table,
        }
    }

    /// `CLAUDE_CONFIG_DIR` (comma-separated) replaces the home folders, as in Claude Code.
    fn default_roots(&self) -> Vec<PathBuf> {
        if let Some(dirs) = std::env::var_os("CLAUDE_CONFIG_DIR") {
            let mut roots: Vec<PathBuf> = vec![];
            for d in dirs.to_string_lossy().split(',').map(str::trim) {
                let p = PathBuf::from(d).join("projects");
                if !d.is_empty() && p.is_dir() && !roots.contains(&p) {
                    roots.push(p);
                }
            }
            if !roots.is_empty() {
                return roots;
            }
        }
        let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))
        else {
            return vec![];
        };
        let home = PathBuf::from(home);
        [
            home.join(".claude").join("projects"),
            home.join(".config").join("claude").join("projects"),
        ]
        .into_iter()
        .filter(|p| p.is_dir())
        .collect()
    }

    fn discover(&self, root: &Path) -> io::Result<Vec<SourceFile>> {
        discover::discover(root)
    }

    fn accepts(&self, _root: &Path, path: &Path) -> bool {
        path.extension().is_some_and(|e| e == "jsonl")
    }

    fn parse_file(
        &self,
        file: &SourceFile,
        resume: Option<ParseResume>,
    ) -> io::Result<FileRecords> {
        parse::parse_file(file, resume)
    }

    fn sources_of(&self, root: &Path, path: &Path) -> Vec<SourceFile> {
        discover::source_of(root, path).into_iter().collect()
    }

    fn config_roots(&self, dir: &Path) -> Vec<PathBuf> {
        if dir.file_name().is_some_and(|n| n == "projects") {
            vec![dir.to_path_buf()]
        } else {
            vec![dir.join("projects")]
        }
    }

    fn account_file(&self, root: &Path) -> Option<PathBuf> {
        let config = root.parent()?;
        if config.file_name().is_some_and(|n| n == ".claude") {
            Some(config.parent()?.join(".claude.json"))
        } else {
            Some(config.join(".claude.json"))
        }
    }

    fn signed_in(&self, file: &Path) -> Option<crate::account::SignedIn> {
        account::signed_in(&std::fs::read(file).ok()?)
    }

    fn drop_copies(&self, file: &mut ParsedFile, owned: &dyn Fn(IdHash) -> bool) {
        dedup::drop_copies(file, owned)
    }

    fn vendors(&self) -> &'static [Vendor] {
        &[Vendor::Anthropic]
    }
}
