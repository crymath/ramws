use std::path::PathBuf;

use anyhow::{Context, Result};
use ramws_core::pool::PoolManager;

use crate::commands::{
    list_pool_records, load_runtime, remove_workspace_record_by_id, require_project_config,
};

pub fn execute(pool: Option<String>, all: bool) -> Result<i32> {
    let runtime = load_runtime(true)?;
    let project_config = require_project_config(&runtime)?;
    let pool_name = ramws_core::pool::resolve_pool_name(
        pool.as_deref(),
        &project_config.workspace.pool,
        &runtime.user_config,
    );

    let records = list_pool_records(&runtime, &pool_name)?;
    if all {
        let mut removed = 0_usize;
        for record in &records {
            let path = PathBuf::from(&record.workspace_root);
            if path.exists() {
                std::fs::remove_dir_all(&path).with_context(|| {
                    format!("failed to remove workspace directory {}", path.display())
                })?;
            }
            remove_workspace_record_by_id(&runtime, &record.workspace_id)?;
            removed += 1;
        }

        if matches!(
            runtime.user_config.pools.get(&pool_name),
            Some(ramws_core::config::PoolConfig::ManagedRamdisk(_))
        ) {
            runtime.pool_manager.destroy_pool(&pool_name, false)?;
        }

        println!("destroyed {} workspace(s) in pool {}", removed, pool_name);
        return Ok(0);
    }

    let canonical_project = runtime
        .project_root
        .canonicalize()
        .unwrap_or_else(|_| runtime.project_root.clone())
        .to_string_lossy()
        .to_string();

    if let Some(record) = records.iter().find(|item| item.project_root == canonical_project) {
        let path = PathBuf::from(&record.workspace_root);
        if path.exists() {
            std::fs::remove_dir_all(&path).with_context(|| {
                format!("failed to remove workspace directory {}", path.display())
            })?;
        }
        remove_workspace_record_by_id(&runtime, &record.workspace_id)?;
        println!("destroyed workspace {}", path.display());
    } else {
        println!("no workspace found for current project in pool {}", pool_name);
    }

    Ok(0)
}
