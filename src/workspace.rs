use crate::config::{BuildDirType, ResolvedConfig};
use crate::syncer::{sync_path, SyncDirection, SyncOptions};
use crate::util::{ensure_dir, is_tmpfs};
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use tracing::{info, warn};

pub struct Workspace {
    pub config: ResolvedConfig,
}

impl Workspace {
    pub fn new(config: ResolvedConfig) -> Self {
        Self { config }
    }

    pub fn ensure(&self, refresh_sources_only: bool) -> Result<()> {
        ensure_dir(&self.config.workspace_root)?;
        if !is_tmpfs(&self.config.workspace_root)? {
            warn!(
                "workspace {} is not on tmpfs; performance may be lower",
                self.config.workspace_root.display()
            );
        }
        if !refresh_sources_only {
            for build in &self.config.raw.build_dirs {
                let path = self.config.workspace_root.join(&build.path);
                ensure_dir(&path)?;
            }
        }
        // populate sources via rsync
        for source in &self.config.raw.sources {
            let src_path = self.config.orig_root.join(&source.path);
            let dest_path = self.config.workspace_root.join(&source.path);
            ensure_dir(&dest_path)?;
            let opts = SyncOptions {
                delete: self.config.raw.sync.delete,
                include: source.include.clone(),
                exclude: source.exclude.clone(),
                itemize: false,
                dry_run: false,
            };
            sync_path(&src_path, &dest_path, SyncDirection::OrigToWorkspace, opts)?;
        }
        Ok(())
    }

    pub fn exists(&self) -> bool {
        self.config.workspace_root.exists()
    }

    pub fn delete(&self) -> Result<()> {
        if self.exists() {
            info!(
                "removing workspace {}",
                self.config.workspace_root.display()
            );
            fs::remove_dir_all(&self.config.workspace_root).with_context(|| {
                format!(
                    "failed to remove workspace {}",
                    self.config.workspace_root.display()
                )
            })?;
        }
        Ok(())
    }

    pub fn build_paths_by_role(&self, role: BuildDirType) -> Vec<PathBuf> {
        self.config
            .raw
            .build_dirs
            .iter()
            .filter(|b| b.r#type == role)
            .map(|b| b.path.clone())
            .collect()
    }
}
