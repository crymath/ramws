use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::sync::Snapshot;
use crate::util::time::unix_timestamp_secs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub project_root: String,
    pub pool_name: String,
    pub workspace_id: String,
    pub created_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sync_in: Option<Snapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sync_in_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sync_out_at: Option<u64>,
}

impl Manifest {
    pub fn new(project_root: String, pool_name: String, workspace_id: String) -> Self {
        Self {
            project_root,
            pool_name,
            workspace_id,
            created_at: unix_timestamp_secs(),
            config_hash: None,
            last_sync_in: None,
            last_sync_in_at: None,
            last_sync_out_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceRecord {
    pub workspace_id: String,
    pub project_root: String,
    pub pool_name: String,
    pub workspace_root: String,
    pub manifest_path: String,
    pub created_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sync_in_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sync_out_at: Option<u64>,
}

pub fn manifest_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join(".ramws").join("manifest.json")
}

pub fn load_manifest(workspace_root: &Path) -> Result<Manifest> {
    let path = manifest_path(workspace_root);
    let text = fs::read_to_string(&path)
        .with_context(|| format!("failed to read manifest {}", path.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("failed to parse manifest {}", path.display()))
}

pub fn save_manifest(workspace_root: &Path, manifest: &Manifest) -> Result<()> {
    let path = manifest_path(workspace_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create manifest dir {}", parent.display()))?;
    }

    let text = serde_json::to_string_pretty(manifest).context("failed to render manifest")?;
    fs::write(&path, text).with_context(|| format!("failed to write manifest {}", path.display()))
}

pub fn ensure_manifest(
    workspace_root: &Path,
    project_root: &Path,
    pool_name: &str,
) -> Result<Manifest> {
    if manifest_path(workspace_root).exists() {
        return load_manifest(workspace_root);
    }

    let project_root = project_root.canonicalize().unwrap_or_else(|_| project_root.to_path_buf());
    let manifest = Manifest::new(
        project_root.to_string_lossy().to_string(),
        pool_name.to_string(),
        uuid::Uuid::new_v4().to_string(),
    );
    save_manifest(workspace_root, &manifest)?;
    Ok(manifest)
}

pub fn workspace_record_path(workspace_state_dir: &Path, workspace_id: &str) -> PathBuf {
    workspace_state_dir.join(format!("{workspace_id}.json"))
}

pub fn save_workspace_record(workspace_state_dir: &Path, record: &WorkspaceRecord) -> Result<()> {
    fs::create_dir_all(workspace_state_dir).with_context(|| {
        format!("failed to create workspace state dir {}", workspace_state_dir.display())
    })?;

    let path = workspace_record_path(workspace_state_dir, &record.workspace_id);
    let text = serde_json::to_string_pretty(record).context("failed to render workspace record")?;
    fs::write(&path, text)
        .with_context(|| format!("failed to write workspace record {}", path.display()))
}

pub fn remove_workspace_record(workspace_state_dir: &Path, workspace_id: &str) -> Result<()> {
    let path = workspace_record_path(workspace_state_dir, workspace_id);
    if path.exists() {
        fs::remove_file(&path)
            .with_context(|| format!("failed to remove workspace record {}", path.display()))?;
    }
    Ok(())
}

pub fn list_workspace_records(workspace_state_dir: &Path) -> Result<Vec<WorkspaceRecord>> {
    if !workspace_state_dir.exists() {
        return Ok(Vec::new());
    }

    let mut records = Vec::new();
    for entry in fs::read_dir(workspace_state_dir).with_context(|| {
        format!("failed to read workspace state dir {}", workspace_state_dir.display())
    })? {
        let entry = entry.context("failed to read workspace state dir entry")?;
        let path = entry.path();
        if path.extension().and_then(|v| v.to_str()) != Some("json") {
            continue;
        }

        let text = fs::read_to_string(&path)
            .with_context(|| format!("failed to read workspace record {}", path.display()))?;
        let record = serde_json::from_str::<WorkspaceRecord>(&text)
            .with_context(|| format!("failed to parse workspace record {}", path.display()))?;
        records.push(record);
    }

    Ok(records)
}
