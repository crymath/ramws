use crate::config::{ResolvedConfig, SyncOnExit};
use crate::syncer::{diff_path, SyncOptions};
use crate::util::{format_bytes, fs_status};
use anyhow::Result;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct StatusReport {
    pub workspace_exists: bool,
    pub workspace_root: String,
    pub fs_type: Option<String>,
    pub total: Option<String>,
    pub available: Option<String>,
    pub used: Option<String>,
    pub diff_changed: usize,
    pub diff_added: usize,
    pub diff_deleted: usize,
    pub sync_policy: SyncOnExit,
    pub config_path: String,
}

pub fn collect_status(cfg: &ResolvedConfig) -> Result<StatusReport> {
    let exists = cfg.workspace_root.exists();
    let mut fs_type = None;
    let mut total = None;
    let mut available = None;
    let mut used = None;
    if exists {
        if let Ok(stat) = fs_status(&cfg.workspace_root) {
            fs_type = Some(stat.fs_type);
            total = Some(format_bytes(stat.total));
            available = Some(format_bytes(stat.available));
            used = Some(format_bytes(stat.used));
        }
    }
    let mut diff_changed = 0usize;
    let mut diff_added = 0usize;
    let mut diff_deleted = 0usize;
    if exists {
        for source in &cfg.raw.sources {
            let ws_path = cfg.workspace_root.join(&source.path);
            let orig_path = cfg.orig_root.join(&source.path);
            let opts = SyncOptions {
                delete: cfg.raw.sync.delete,
                include: source.include.clone(),
                exclude: source.exclude.clone(),
                itemize: true,
                dry_run: true,
            };
            if let Ok(summary) = diff_path(&ws_path, &orig_path, opts) {
                diff_changed += summary.changed;
                diff_added += summary.added;
                diff_deleted += summary.deleted;
            }
        }
    }
    Ok(StatusReport {
        workspace_exists: exists,
        workspace_root: cfg.workspace_root.display().to_string(),
        fs_type,
        total,
        available,
        used,
        diff_changed,
        diff_added,
        diff_deleted,
        sync_policy: cfg.raw.sync.on_exit.clone(),
        config_path: cfg.config_path.display().to_string(),
    })
}
