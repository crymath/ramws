use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail};

use crate::config::{AppPaths, PoolConfig, UserConfig};
use crate::pool::model::{ManagedPoolState, PoolInstance, PoolKindTag};
use crate::util::lock::with_exclusive_lock;

#[cfg(target_os = "macos")]
use crate::pool::macos_ramdisk;

pub trait PoolManager {
    fn ensure_pool(&self, name: &str) -> Result<PoolInstance>;
    fn create_pool(&self, name: &str) -> Result<PoolInstance>;
    fn destroy_pool(&self, name: &str, force: bool) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct DefaultPoolManager {
    pub app_paths: AppPaths,
    pub user_config: UserConfig,
}

impl DefaultPoolManager {
    pub fn new(app_paths: AppPaths, user_config: UserConfig) -> Self {
        Self { app_paths, user_config }
    }

    pub fn pool_state_path(&self, name: &str) -> PathBuf {
        self.app_paths.pool_state_dir.join(format!("{name}.toml"))
    }

    fn lock_path(&self, name: &str) -> PathBuf {
        self.app_paths.lock_dir.join(format!("pool-{name}.lock"))
    }

    fn pool_cfg(&self, name: &str) -> Result<&PoolConfig> {
        self.user_config
            .pools
            .get(name)
            .ok_or_else(|| anyhow!("pool '{}' is not defined in user config", name))
    }

    fn load_managed_state(&self, name: &str) -> Result<Option<ManagedPoolState>> {
        let state_path = self.pool_state_path(name);
        if !state_path.exists() {
            return Ok(None);
        }

        #[cfg(target_os = "macos")]
        {
            Ok(Some(macos_ramdisk::load_managed_state(&state_path)?))
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = name;
            bail!("managed ramdisk pools are only supported on macOS")
        }
    }

    fn managed_to_instance(name: &str, state: ManagedPoolState) -> PoolInstance {
        PoolInstance {
            name: name.to_string(),
            kind: PoolKindTag::ManagedRamdisk,
            root: state.mountpoint.clone(),
            managed: Some(state),
        }
    }

    pub fn inspect_pool(&self, name: &str) -> Result<Option<PoolInstance>> {
        let cfg = self.pool_cfg(name)?.clone();
        match cfg {
            PoolConfig::ExternalDir(external) => Ok(Some(PoolInstance {
                name: name.to_string(),
                kind: PoolKindTag::ExternalDir,
                root: external.path,
                managed: None,
            })),
            PoolConfig::ManagedRamdisk(_) => {
                let state = self.load_managed_state(name)?;
                Ok(state.map(|s| Self::managed_to_instance(name, s)))
            }
        }
    }
}

impl PoolManager for DefaultPoolManager {
    fn ensure_pool(&self, name: &str) -> Result<PoolInstance> {
        let cfg = self.pool_cfg(name)?.clone();

        match cfg {
            PoolConfig::ExternalDir(external) => {
                std::fs::create_dir_all(&external.path).with_context(|| {
                    format!("failed to create external pool directory {}", external.path.display())
                })?;
                Ok(PoolInstance {
                    name: name.to_string(),
                    kind: PoolKindTag::ExternalDir,
                    root: external.path,
                    managed: None,
                })
            }
            PoolConfig::ManagedRamdisk(managed) => {
                let lock = self.lock_path(name);
                let state_path = self.pool_state_path(name);
                with_exclusive_lock(&lock, || {
                    if let Some(state) = self.load_managed_state(name)?
                        && state.mountpoint.exists()
                    {
                        return Ok(Self::managed_to_instance(name, state));
                    }

                    #[cfg(target_os = "macos")]
                    {
                        let created = macos_ramdisk::create_pool(name, &managed, &state_path)?;
                        Ok(Self::managed_to_instance(name, created))
                    }

                    #[cfg(not(target_os = "macos"))]
                    {
                        let _ = managed;
                        bail!("managed ramdisk pools are only supported on macOS")
                    }
                })
            }
        }
    }

    fn create_pool(&self, name: &str) -> Result<PoolInstance> {
        let cfg = self.pool_cfg(name)?.clone();
        match cfg {
            PoolConfig::ExternalDir(external) => {
                std::fs::create_dir_all(&external.path).with_context(|| {
                    format!("failed to create external pool directory {}", external.path.display())
                })?;
                Ok(PoolInstance {
                    name: name.to_string(),
                    kind: PoolKindTag::ExternalDir,
                    root: external.path,
                    managed: None,
                })
            }
            PoolConfig::ManagedRamdisk(managed) => {
                let lock = self.lock_path(name);
                let state_path = self.pool_state_path(name);

                with_exclusive_lock(&lock, || {
                    if let Some(state) = self.load_managed_state(name)?
                        && state.mountpoint.exists()
                    {
                        return Ok(Self::managed_to_instance(name, state));
                    }

                    #[cfg(target_os = "macos")]
                    {
                        let created = macos_ramdisk::create_pool(name, &managed, &state_path)?;
                        Ok(Self::managed_to_instance(name, created))
                    }

                    #[cfg(not(target_os = "macos"))]
                    {
                        let _ = managed;
                        bail!("managed ramdisk pools are only supported on macOS")
                    }
                })
            }
        }
    }

    fn destroy_pool(&self, name: &str, force: bool) -> Result<()> {
        let cfg = self.pool_cfg(name)?.clone();
        match cfg {
            PoolConfig::ExternalDir(_) => {
                bail!(
                    "pool '{}' is external_dir; admin pool destroy only supports managed_ramdisk",
                    name
                );
            }
            PoolConfig::ManagedRamdisk(_) => {
                let lock = self.lock_path(name);
                let state_path = self.pool_state_path(name);

                with_exclusive_lock(&lock, || {
                    let Some(state) = self.load_managed_state(name)? else {
                        return Ok(());
                    };

                    #[cfg(target_os = "macos")]
                    {
                        macos_ramdisk::destroy_pool(&state, force)?;
                        if state_path.exists() {
                            std::fs::remove_file(&state_path).with_context(|| {
                                format!("failed to remove pool state {}", state_path.display())
                            })?;
                        }
                        Ok(())
                    }

                    #[cfg(not(target_os = "macos"))]
                    {
                        let _ = state;
                        bail!("managed ramdisk pools are only supported on macOS")
                    }
                })
            }
        }
    }
}

pub fn resolve_pool_name(
    explicit: Option<&str>,
    project_pool: &str,
    user_config: &UserConfig,
) -> String {
    if let Some(name) = explicit {
        return name.to_string();
    }

    if !project_pool.is_empty() {
        return project_pool.to_string();
    }

    user_config.defaults.pool.clone()
}
