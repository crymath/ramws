use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use ramws_core::config::{
    AppPaths, ConflictPolicy, ProjectConfig, SyncOnExit, app_paths, ensure_app_dirs,
    load_or_initialize_user_config, load_project_config,
};
use ramws_core::pool::{DefaultPoolManager, PoolManager, resolve_pool_name};
use ramws_core::sync::{
    ApplyOptions, ApplyStats, ScanOptions, SyncPlan, apply::apply_plan, conflict::detect_conflicts,
    plan::plan_sync, scan::scan_snapshot,
};
use ramws_core::util::{fs::stable_hash_str, time::unix_timestamp_secs};
use ramws_core::workspace::{
    Manifest, WorkspaceRecord, ensure_manifest, list_workspace_records, remove_workspace_record,
    save_manifest, save_workspace_record,
};

pub mod admin;
pub mod destroy;
pub mod init;
pub mod run;
pub mod shell;
pub mod status;
pub mod sync;
pub mod up;

#[derive(Debug, Clone)]
pub struct Runtime {
    pub paths: AppPaths,
    pub project_root: PathBuf,
    pub project_config: Option<ProjectConfig>,
    pub user_config: ramws_core::config::UserConfig,
    pub pool_manager: DefaultPoolManager,
}

#[derive(Debug, Clone)]
pub struct WorkspaceContext {
    pub runtime: Runtime,
    pub pool_name: String,
    pub workspace_root: PathBuf,
    pub manifest: Manifest,
}

#[derive(Debug, Clone)]
pub struct SyncExecution {
    pub stats: ApplyStats,
    pub skipped_conflicts: Vec<String>,
}

pub fn load_runtime(require_project_config: bool) -> Result<Runtime> {
    let cwd = std::env::current_dir().context("failed to read current directory")?;
    let paths = app_paths(&cwd)?;
    ensure_app_dirs(&paths)?;

    let user_config = load_or_initialize_user_config(&paths.user_config_path)?;
    let project_config = if paths.project_config_path.exists() {
        Some(load_project_config(&paths.project_config_path)?)
    } else if require_project_config {
        bail!("{} not found; run 'ramws init' first", paths.project_config_path.display());
    } else {
        None
    };

    let pool_manager = DefaultPoolManager::new(paths.clone(), user_config.clone());

    Ok(Runtime {
        project_root: paths.project_root.clone(),
        paths,
        project_config,
        user_config,
        pool_manager,
    })
}

pub fn require_project_config(runtime: &Runtime) -> Result<&ProjectConfig> {
    runtime
        .project_config
        .as_ref()
        .ok_or_else(|| anyhow!("project config is required; run 'ramws init' first"))
}

pub fn resolve_workspace_context(
    runtime: Runtime,
    pool_override: Option<&str>,
) -> Result<WorkspaceContext> {
    let project_config = require_project_config(&runtime)?;
    let pool_name =
        resolve_pool_name(pool_override, &project_config.workspace.pool, &runtime.user_config);

    let pool = runtime.pool_manager.ensure_pool(&pool_name)?;
    let workspace_root = ramws_core::workspace::resolve_workspace_path(
        &pool.root,
        &runtime.project_root,
        &project_config.workspace.layout,
    )?;
    std::fs::create_dir_all(&workspace_root).with_context(|| {
        format!("failed to create workspace root directory {}", workspace_root.display())
    })?;

    let mut manifest = ensure_manifest(&workspace_root, &runtime.project_root, &pool_name)?;
    manifest.config_hash = Some(project_config_hash(project_config)?);
    save_manifest(&workspace_root, &manifest)?;
    persist_workspace_record(&runtime.paths.workspace_state_dir, &workspace_root, &manifest)?;

    Ok(WorkspaceContext { runtime, pool_name, workspace_root, manifest })
}

pub fn project_config_hash(config: &ProjectConfig) -> Result<String> {
    let rendered =
        toml::to_string(config).context("failed to serialize project config for hash")?;
    Ok(stable_hash_str(&rendered))
}

pub fn build_scan_options(config: &ProjectConfig) -> ScanOptions {
    ScanOptions {
        respect_gitignore: config.sync.respect_gitignore,
        follow_symlinks: config.sync.follow_symlinks,
        exclude: config.sync.exclude.clone(),
        hash_mode: config.sync.hash_mode,
    }
}

pub fn sync_in(ctx: &mut WorkspaceContext) -> Result<SyncExecution> {
    let project_config = require_project_config(&ctx.runtime)?;

    let scan_options = build_scan_options(project_config);
    let src_snapshot = scan_snapshot(&ctx.runtime.project_root, &scan_options)?;
    let dst_snapshot = scan_snapshot(&ctx.workspace_root, &scan_options)?;

    let plan = plan_sync(
        &src_snapshot,
        &dst_snapshot,
        &ramws_core::sync::PlanOptions {
            delete_extra: project_config.sync.delete_extra,
            hash_mode: project_config.sync.hash_mode,
        },
    );

    let stats = apply_plan(
        &ctx.runtime.project_root,
        &ctx.workspace_root,
        &plan,
        ApplyOptions { set_mtime: true },
    )?;

    ctx.manifest.last_sync_in = Some(src_snapshot);
    ctx.manifest.last_sync_in_at = Some(unix_timestamp_secs());
    save_manifest(&ctx.workspace_root, &ctx.manifest)?;
    persist_workspace_record(
        &ctx.runtime.paths.workspace_state_dir,
        &ctx.workspace_root,
        &ctx.manifest,
    )?;

    Ok(SyncExecution { stats, skipped_conflicts: Vec::new() })
}

pub fn plan_sync_out(ctx: &WorkspaceContext) -> Result<SyncPlan> {
    let project_config = require_project_config(&ctx.runtime)?;
    let scan_options = build_scan_options(project_config);
    let workspace_snapshot = scan_snapshot(&ctx.workspace_root, &scan_options)?;
    let original_snapshot = scan_snapshot(&ctx.runtime.project_root, &scan_options)?;

    let mut plan = plan_sync(
        &workspace_snapshot,
        &original_snapshot,
        &ramws_core::sync::PlanOptions {
            delete_extra: project_config.sync.delete_extra,
            hash_mode: project_config.sync.hash_mode,
        },
    );
    plan.conflicts = detect_conflicts(
        ctx.manifest.last_sync_in.as_ref(),
        &original_snapshot,
        &workspace_snapshot,
        &plan,
    );
    Ok(plan)
}

pub fn sync_out(ctx: &mut WorkspaceContext) -> Result<SyncExecution> {
    let project_config = require_project_config(&ctx.runtime)?;

    let mut plan = plan_sync_out(ctx)?;

    let mut skipped_conflicts = Vec::new();
    match project_config.sync.conflict_policy {
        ConflictPolicy::Abort => {
            if !plan.conflicts.is_empty() {
                bail!(
                    "sync-out aborted due to {} conflict(s): {}",
                    plan.conflicts.len(),
                    summarize_conflicts(&plan.conflicts)
                );
            }
        }
        ConflictPolicy::PreferWorkspace => {}
        ConflictPolicy::PreferOriginal => {
            let conflict_paths: BTreeSet<&str> =
                plan.conflicts.iter().map(|item| item.path.as_str()).collect();
            plan.creates.retain(|item| !conflict_paths.contains(item.path.as_str()));
            plan.updates.retain(|item| !conflict_paths.contains(item.path.as_str()));
            skipped_conflicts =
                plan.conflicts.iter().map(|conflict| conflict.path.clone()).collect();
        }
    }

    let stats = apply_plan(
        &ctx.workspace_root,
        &ctx.runtime.project_root,
        &plan,
        ApplyOptions { set_mtime: true },
    )?;

    ctx.manifest.last_sync_out_at = Some(unix_timestamp_secs());
    save_manifest(&ctx.workspace_root, &ctx.manifest)?;
    persist_workspace_record(
        &ctx.runtime.paths.workspace_state_dir,
        &ctx.workspace_root,
        &ctx.manifest,
    )?;

    Ok(SyncExecution { stats, skipped_conflicts })
}

pub fn persist_workspace_record(
    workspace_state_dir: &Path,
    workspace_root: &Path,
    manifest: &Manifest,
) -> Result<()> {
    let record = WorkspaceRecord {
        workspace_id: manifest.workspace_id.clone(),
        project_root: manifest.project_root.clone(),
        pool_name: manifest.pool_name.clone(),
        workspace_root: workspace_root.to_string_lossy().to_string(),
        manifest_path: workspace_root
            .join(".ramws")
            .join("manifest.json")
            .to_string_lossy()
            .to_string(),
        created_at: manifest.created_at,
        last_sync_in_at: manifest.last_sync_in_at,
        last_sync_out_at: manifest.last_sync_out_at,
    };
    save_workspace_record(workspace_state_dir, &record)
}

pub fn list_pool_records(runtime: &Runtime, pool_name: &str) -> Result<Vec<WorkspaceRecord>> {
    let all = list_workspace_records(&runtime.paths.workspace_state_dir)?;
    Ok(all.into_iter().filter(|record| record.pool_name == pool_name).collect())
}

pub fn remove_workspace_record_by_id(runtime: &Runtime, workspace_id: &str) -> Result<()> {
    remove_workspace_record(&runtime.paths.workspace_state_dir, workspace_id)
}

fn summarize_conflicts(conflicts: &[ramws_core::sync::Conflict]) -> String {
    conflicts.iter().take(8).map(|item| item.path.as_str()).collect::<Vec<_>>().join(", ")
}

pub fn sync_on_exit_decision(sync_on_exit: SyncOnExit, has_changes: bool) -> &'static str {
    if !has_changes {
        return "skip";
    }

    match sync_on_exit {
        SyncOnExit::Ask => "ask",
        SyncOnExit::Always => "always",
        SyncOnExit::Never => "never",
    }
}
