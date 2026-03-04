use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserConfig {
    pub defaults: DefaultConfig,
    pub pools: BTreeMap<String, PoolConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultConfig {
    pub pool: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PoolConfig {
    ManagedRamdisk(ManagedRamdiskPoolConfig),
    ExternalDir(ExternalDirPoolConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedRamdiskPoolConfig {
    pub volume_name: String,
    pub filesystem: String,
    pub size_gib: u64,
    #[serde(default)]
    pub nobrowse: bool,
    #[serde(default)]
    pub extra_hdiutil_args: Vec<String>,
    #[serde(default)]
    pub extra_diskutil_args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalDirPoolConfig {
    pub path: PathBuf,
    #[serde(default)]
    pub ram_like: bool,
}

impl Default for UserConfig {
    fn default() -> Self {
        let mut pools = BTreeMap::new();
        pools.insert(
            "main".to_string(),
            PoolConfig::ManagedRamdisk(ManagedRamdiskPoolConfig {
                volume_name: "RAMWS".to_string(),
                filesystem: "APFS".to_string(),
                size_gib: 8,
                nobrowse: true,
                extra_hdiutil_args: Vec::new(),
                extra_diskutil_args: Vec::new(),
            }),
        );

        Self { defaults: DefaultConfig { pool: "main".to_string() }, pools }
    }
}
