use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::{ExternalDirPoolConfig, ManagedRamdiskPoolConfig};

#[derive(Debug, Clone)]
pub enum PoolKind {
    ManagedRamdisk(ManagedRamdiskPoolConfig),
    ExternalDir(ExternalDirPoolConfig),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PoolKindTag {
    ManagedRamdisk,
    ExternalDir,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedPoolState {
    pub pool_name: String,
    pub device: String,
    pub volume_name: String,
    pub mountpoint: PathBuf,
    pub mounted_device: String,
    pub created_at: u64,
}

#[derive(Debug, Clone)]
pub struct PoolInstance {
    pub name: String,
    pub kind: PoolKindTag,
    pub root: PathBuf,
    pub managed: Option<ManagedPoolState>,
}
