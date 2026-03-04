use serde::{Deserialize, Serialize};

use crate::{ConflictPolicy, HashMode, SyncOnExit};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub version: u32,
    pub workspace: WorkspaceConfig,
    pub sync: SyncConfig,
    pub lifecycle: LifecycleConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub pool: String,
    pub layout: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    #[serde(default)]
    pub delete_extra: bool,
    #[serde(default = "default_true")]
    pub respect_gitignore: bool,
    #[serde(default)]
    pub follow_symlinks: bool,
    #[serde(default)]
    pub conflict_policy: ConflictPolicy,
    #[serde(default)]
    pub hash_mode: HashMode,
    #[serde(default = "default_excludes")]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleConfig {
    #[serde(default)]
    pub sync_on_exit: SyncOnExit,
}

fn default_true() -> bool {
    true
}

fn default_excludes() -> Vec<String> {
    vec![
        ".git/**".to_string(),
        "target/**".to_string(),
        "build/**".to_string(),
        ".ramws/**".to_string(),
    ]
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            version: 1,
            workspace: WorkspaceConfig {
                pool: "main".to_string(),
                layout: "${user}/${project}".to_string(),
            },
            sync: SyncConfig {
                delete_extra: false,
                respect_gitignore: true,
                follow_symlinks: false,
                conflict_policy: ConflictPolicy::Abort,
                hash_mode: HashMode::Metadata,
                exclude: default_excludes(),
            },
            lifecycle: LifecycleConfig { sync_on_exit: SyncOnExit::Ask },
        }
    }
}
