use crate::config::{BuildDirType, ResolvedConfig};
use crate::util::{path_with_trailing_slash, prompt_confirm};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncDirection {
    OrigToWorkspace,
    WorkspaceToOrig,
}

#[derive(Debug, Clone)]
pub struct SyncOptions {
    pub delete: bool,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub itemize: bool,
    pub dry_run: bool,
}

#[derive(Debug, Default, Clone)]
pub struct DiffSummary {
    pub changed: usize,
    pub added: usize,
    pub deleted: usize,
}

fn build_rsync_command(
    source: &Path,
    dest: &Path,
    direction: SyncDirection,
    opts: &SyncOptions,
) -> Command {
    let mut cmd = Command::new("rsync");
    cmd.arg("-a");
    if opts.delete {
        cmd.arg("--delete");
    }
    if opts.dry_run {
        cmd.arg("--dry-run");
    }
    if opts.itemize {
        cmd.arg("--itemize-changes");
    }
    for inc in &opts.include {
        cmd.arg(format!("--include={inc}"));
    }
    for exc in &opts.exclude {
        cmd.arg(format!("--exclude={exc}"));
    }
    let mut src = source.to_path_buf();
    let dst = dest.to_path_buf();
    match direction {
        SyncDirection::OrigToWorkspace => {
            src = PathBuf::from(path_with_trailing_slash(&src));
        }
        SyncDirection::WorkspaceToOrig => {
            src = PathBuf::from(path_with_trailing_slash(&src));
        }
    }
    cmd.arg(src);
    cmd.arg(dst);
    cmd
}

pub fn sync_path(
    source: &Path,
    dest: &Path,
    direction: SyncDirection,
    opts: SyncOptions,
) -> Result<()> {
    let mut cmd = build_rsync_command(source, dest, direction, &opts);
    let output = cmd.output().context("failed to run rsync")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("rsync failed: {stderr}");
    }
    Ok(())
}

pub fn diff_path(source: &Path, dest: &Path, opts: SyncOptions) -> Result<DiffSummary> {
    let mut cmd = build_rsync_command(source, dest, SyncDirection::WorkspaceToOrig, &opts);
    let output = cmd.output().context("failed to run rsync")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("rsync failed: {stderr}");
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut summary = DiffSummary::default();
    for line in stdout.lines() {
        if line.starts_with(">f") || line.starts_with(".f") || line.starts_with("cD") {
            summary.changed += 1;
        } else if line.starts_with(">f+++++++++") {
            summary.added += 1;
        } else if line.starts_with("*deleting") {
            summary.deleted += 1;
        }
    }
    Ok(summary)
}

pub fn sync_back(cfg: &ResolvedConfig, paths: &[PathBuf], noninteractive: bool) -> Result<()> {
    let staging = cfg.orig_root.join(".ramws-staging");
    if staging.exists() {
        std::fs::remove_dir_all(&staging).context("failed to clean staging directory")?;
    }
    std::fs::create_dir_all(&staging).context("failed to create staging directory")?;
    let delete = cfg.raw.sync.delete;
    let mut total_synced = 0usize;
    for rel in paths {
        let ws_path = cfg.workspace_root.join(rel);
        let stage_path = staging.join(rel);
        if let Some(parent) = stage_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let opts = SyncOptions {
            delete,
            include: vec![],
            exclude: vec![],
            itemize: false,
            dry_run: false,
        };
        sync_path(
            &ws_path,
            &stage_path,
            SyncDirection::WorkspaceToOrig,
            opts.clone(),
        )?;
        let opts2 = SyncOptions {
            dry_run: false,
            ..opts
        };
        let dest = cfg.orig_root.join(rel);
        sync_path(&stage_path, &dest, SyncDirection::OrigToWorkspace, opts2)?;
        total_synced += 1;
    }
    if !noninteractive {
        info!("synced {} paths back to disk", total_synced);
    }
    std::fs::remove_dir_all(&staging).ok();
    Ok(())
}

pub fn refresh_from_orig(cfg: &ResolvedConfig, paths: &[PathBuf]) -> Result<()> {
    let delete = cfg.raw.sync.delete;
    for rel in paths {
        let src = cfg.orig_root.join(rel);
        let dest = cfg.workspace_root.join(rel);
        let opts = SyncOptions {
            delete,
            include: vec![],
            exclude: vec![],
            itemize: false,
            dry_run: false,
        };
        sync_path(&src, &dest, SyncDirection::OrigToWorkspace, opts)?;
    }
    Ok(())
}

pub fn paths_from_roles(cfg: &ResolvedConfig, roles: &[BuildDirType]) -> Vec<PathBuf> {
    let mut result: Vec<PathBuf> = vec![];
    if roles.is_empty()
        || roles.contains(&BuildDirType::Scratch)
        || roles.contains(&BuildDirType::Cache)
    {
        for s in &cfg.raw.sources {
            result.push(s.path.clone());
        }
    }
    for build in &cfg.raw.build_dirs {
        if roles.contains(&build.r#type) {
            result.push(build.path.clone());
        }
    }
    result
}

pub fn confirm_if_needed(message: &str, noninteractive: bool) -> Result<bool> {
    if noninteractive {
        Ok(true)
    } else {
        prompt_confirm(message, true)
    }
}
