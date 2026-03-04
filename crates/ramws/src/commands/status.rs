use std::path::Path;
use std::process::Command;

use anyhow::Result;
use walkdir::WalkDir;

use crate::commands::{load_runtime, require_project_config};

pub fn execute(pool: Option<String>) -> Result<i32> {
    let runtime = load_runtime(true)?;
    let project_config = require_project_config(&runtime)?;
    let pool_name = ramws_core::pool::resolve_pool_name(
        pool.as_deref(),
        &project_config.workspace.pool,
        &runtime.user_config,
    );

    let maybe_pool = runtime.pool_manager.inspect_pool(&pool_name)?;

    println!("pool: {}", pool_name);
    match &maybe_pool {
        Some(pool) => {
            println!("pool_kind: {:?}", pool.kind);
            println!("pool_root: {}", pool.root.display());
        }
        None => {
            println!("pool_kind: managed_ramdisk (not created)");
            println!("pool_root: <not mounted>");
        }
    }

    if let Some(pool) = maybe_pool {
        let workspace_root = ramws_core::workspace::resolve_workspace_path(
            &pool.root,
            &runtime.project_root,
            &project_config.workspace.layout,
        )?;
        println!("workspace: {}", workspace_root.display());

        let manifest_path = workspace_root.join(".ramws/manifest.json");
        println!("manifest: {}", manifest_path.display());

        if manifest_path.exists() {
            let manifest = ramws_core::workspace::load_manifest(&workspace_root)?;
            println!("workspace_id: {}", manifest.workspace_id);
            println!("last_sync_in_at: {}", format_timestamp(manifest.last_sync_in_at));
            println!("last_sync_out_at: {}", format_timestamp(manifest.last_sync_out_at));
        } else {
            println!("workspace_id: <missing>");
            println!("last_sync_in_at: <none>");
            println!("last_sync_out_at: <none>");
        }

        if workspace_root.exists() {
            let approx_size = approximate_size(&workspace_root);
            println!("workspace_bytes_approx: {}", approx_size);
        }
    }

    Ok(0)
}

fn approximate_size(root: &Path) -> u64 {
    WalkDir::new(root)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter_map(|entry| entry.metadata().ok())
        .filter(|meta| meta.is_file())
        .map(|meta| meta.len())
        .sum()
}

fn format_timestamp(value: Option<u64>) -> String {
    let Some(epoch) = value else {
        return "<none>".to_string();
    };

    if let Ok(output) =
        Command::new("date").args(["-r", &epoch.to_string(), "+%Y-%m-%d %H:%M:%S %Z"]).output()
        && output.status.success()
    {
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !text.is_empty() {
            return format!("{text} (epoch={epoch})");
        }
    }

    format!("epoch={epoch}")
}
