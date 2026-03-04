use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::config::HashMode;

pub mod apply;
pub mod conflict;
pub mod plan;
pub mod scan;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryType {
    File,
    Dir,
    Symlink,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMeta {
    pub size: u64,
    pub mtime_secs: i64,
    pub mtime_nanos: i64,
    pub mode: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blake3: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryRecord {
    pub entry_type: EntryType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<FileMeta>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_target: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Snapshot {
    pub entries: BTreeMap<String, EntryRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlannedAction {
    pub path: String,
    pub entry: EntryRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Conflict {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncPlan {
    pub creates: Vec<PlannedAction>,
    pub updates: Vec<PlannedAction>,
    pub deletes: Vec<String>,
    pub conflicts: Vec<Conflict>,
}

#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub respect_gitignore: bool,
    pub follow_symlinks: bool,
    pub exclude: Vec<String>,
    pub hash_mode: HashMode,
}

#[derive(Debug, Clone)]
pub struct PlanOptions {
    pub delete_extra: bool,
    pub hash_mode: HashMode,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ApplyOptions {
    pub set_mtime: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ApplyStats {
    pub created: usize,
    pub updated: usize,
    pub deleted: usize,
}

impl SyncPlan {
    pub fn total_changes(&self) -> usize {
        self.creates.len() + self.updates.len() + self.deletes.len()
    }
}
