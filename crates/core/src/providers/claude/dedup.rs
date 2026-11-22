// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use crate::model::{IdHash, ParsedFile};
use std::collections::HashSet;

pub(super) fn drop_copies(file: &mut ParsedFile, owned: &dyn Fn(IdHash) -> bool) {
    let r = &mut file.records;
    let mut dropped: HashSet<u32> = HashSet::new();
    r.turns.retain(|t| {
        let keep = owned(t.msg_id);
        if !keep {
            dropped.insert(t.order);
        }
        keep
    });
    let Some(&max_dropped) = dropped.iter().max() else {
        return;
    };
    let first_kept = r.turns.iter().map(|t| t.order).min().unwrap_or(u32::MAX);
    r.head_copied = dropped.iter().any(|&o| o < first_kept);
    let in_copy = |o: u32| o < first_kept && o < max_dropped;
    r.tool_calls.retain(|c| !dropped.contains(&c.turn_order));
    r.skill_loads.retain(|s| !in_copy(s.order));
    r.compactions.retain(|c| !in_copy(c.order));
    r.human_prompts.retain(|&o| !in_copy(o));
}
