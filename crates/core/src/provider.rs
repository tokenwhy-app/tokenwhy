// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Rules for implementers:
//! - A source that is not one real file uses a virtual path `<real file>#<unit>`; only `fingerprint` and `parse_file` resolve it.
//! - `Turn.msg_id` is the dedup key, equal across copies of the same call.
//! - Nothing here writes in the agent's folders.

pub use crate::account::SignedIn;
pub use crate::model::{Capabilities, CostSource, Level, Vendor};
use crate::model::{
    FileRecords, IdHash, ParseResume, ParsedFile, PriceTable, ProviderId, SkipCount, SourceFile,
};
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Change {
    Append,
    Whole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fingerprint {
    pub size: u64,
    pub mtime_ns: u64,
    pub head_len: u32,
    pub head_hash: u64,
    pub change: Change,
}

pub trait Provider: Send + Sync {
    fn id(&self) -> ProviderId;

    fn label(&self) -> &'static str;

    fn caps(&self) -> Capabilities;

    fn default_roots(&self) -> Vec<PathBuf>;

    fn discover(&self, root: &Path) -> io::Result<Vec<SourceFile>>;

    fn watch_dirs(&self, root: &Path) -> Vec<(PathBuf, bool)> {
        vec![(root.to_path_buf(), true)]
    }

    fn accepts(&self, root: &Path, path: &Path) -> bool;

    fn sources_of(&self, root: &Path, path: &Path) -> Vec<SourceFile>;

    fn fingerprint(&self, src: &SourceFile) -> io::Result<Fingerprint> {
        crate::corpus::append_fingerprint(&src.path)
    }

    fn parse_file(&self, file: &SourceFile, resume: Option<ParseResume>)
        -> io::Result<FileRecords>;

    fn drop_copies(&self, file: &mut ParsedFile, owned: &dyn Fn(IdHash) -> bool) {
        let r = &mut file.records;
        let mut dropped = std::collections::HashSet::new();
        r.turns
            .retain(|t| owned(t.msg_id) || !dropped.insert(t.order));
        r.tool_calls.retain(|c| !dropped.contains(&c.turn_order));
    }

    fn dedup(&self, files: &mut [ParsedFile]) {
        crate::corpus::dedup_all(self, files)
    }

    fn skipped(&self, _root: &Path) -> Vec<SkipCount> {
        vec![]
    }

    fn vendors(&self) -> &'static [Vendor];

    fn config_roots(&self, dir: &Path) -> Vec<PathBuf> {
        vec![dir.to_path_buf()]
    }

    fn account_file(&self, _root: &Path) -> Option<PathBuf> {
        None
    }

    /// Never an email or a person's name.
    fn signed_in(&self, _file: &Path) -> Option<SignedIn> {
        None
    }

    fn public_names(&self) -> &'static [&'static str] {
        &[]
    }

    fn prices(&self) -> PriceTable {
        crate::price::provider_table(self.id(), self.vendors())
    }
}

pub fn all() -> Vec<Box<dyn Provider>> {
    #[cfg_attr(not(feature = "sqlite"), allow(unused_mut))]
    let mut v: Vec<Box<dyn Provider>> = vec![
        Box::new(crate::providers::claude::ClaudeCode),
        Box::new(crate::providers::codex::Codex),
        Box::new(crate::providers::opencode::Opencode),
    ];
    #[cfg(feature = "sqlite")]
    v.push(Box::new(crate::providers::agy::Antigravity));
    v
}
