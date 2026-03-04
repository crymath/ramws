use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

pub mod project;
pub mod user;

pub use project::{LifecycleConfig, ProjectConfig, SyncConfig, WorkspaceConfig};
pub use user::{
    DefaultConfig, ExternalDirPoolConfig, ManagedRamdiskPoolConfig, PoolConfig, UserConfig,
};

pub const PROJECT_CONFIG_FILE: &str = ".ramws.toml";
pub const USER_CONFIG_FILE: &str = "config.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConflictPolicy {
    #[default]
    Abort,
    PreferWorkspace,
    PreferOriginal,
}

impl Display for ConflictPolicy {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Abort => write!(f, "abort"),
            Self::PreferWorkspace => write!(f, "prefer_workspace"),
            Self::PreferOriginal => write!(f, "prefer_original"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum HashMode {
    #[default]
    Metadata,
    Blake3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SyncOnExit {
    #[default]
    Ask,
    Always,
    Never,
}

impl Display for SyncOnExit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ask => write!(f, "ask"),
            Self::Always => write!(f, "always"),
            Self::Never => write!(f, "never"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub project_root: PathBuf,
    pub project_config_path: PathBuf,
    pub user_config_path: PathBuf,
    pub state_dir: PathBuf,
    pub pool_state_dir: PathBuf,
    pub workspace_state_dir: PathBuf,
    pub lock_dir: PathBuf,
}

pub fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("dev", "ramws", "ramws")
        .ok_or_else(|| anyhow!("failed to determine platform project directories"))
}

pub fn app_paths(start: &Path) -> Result<AppPaths> {
    let dirs = project_dirs()?;
    let project_root = discover_project_root(start)?;
    let project_config_path = project_root.join(PROJECT_CONFIG_FILE);
    let user_config_path = dirs.config_dir().join(USER_CONFIG_FILE);
    let state_dir = dirs.state_dir().unwrap_or(dirs.data_local_dir()).to_path_buf();

    Ok(AppPaths {
        project_root,
        project_config_path,
        user_config_path,
        pool_state_dir: state_dir.join("pools"),
        workspace_state_dir: state_dir.join("workspaces"),
        lock_dir: state_dir.join("locks"),
        state_dir,
    })
}

pub fn ensure_app_dirs(paths: &AppPaths) -> Result<()> {
    fs::create_dir_all(&paths.state_dir)
        .with_context(|| format!("failed to create state dir {}", paths.state_dir.display()))?;
    fs::create_dir_all(&paths.pool_state_dir).with_context(|| {
        format!("failed to create pool state dir {}", paths.pool_state_dir.display())
    })?;
    fs::create_dir_all(&paths.workspace_state_dir).with_context(|| {
        format!("failed to create workspace state dir {}", paths.workspace_state_dir.display())
    })?;
    fs::create_dir_all(&paths.lock_dir)
        .with_context(|| format!("failed to create lock dir {}", paths.lock_dir.display()))?;
    if let Some(parent) = paths.user_config_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create config dir {}", parent.display()))?;
    }
    Ok(())
}

pub fn discover_project_root(start: &Path) -> Result<PathBuf> {
    let start = if start.is_absolute() {
        start.to_path_buf()
    } else {
        std::env::current_dir().context("failed to read current working directory")?.join(start)
    };

    let mut current = if start.is_file() {
        start
            .parent()
            .ok_or_else(|| anyhow!("path has no parent: {}", start.display()))?
            .to_path_buf()
    } else {
        start
    };

    loop {
        if current.join(PROJECT_CONFIG_FILE).exists() || current.join(".git").exists() {
            return Ok(current);
        }

        if !current.pop() {
            break;
        }
    }

    std::env::current_dir().context("failed to read current working directory")
}

pub fn load_project_config(project_config_path: &Path) -> Result<ProjectConfig> {
    let data = fs::read_to_string(project_config_path)
        .with_context(|| format!("failed to read {}", project_config_path.display()))?;
    toml::from_str::<ProjectConfig>(&data)
        .with_context(|| format!("failed to parse {}", project_config_path.display()))
}

pub fn load_project_config_or_default(project_config_path: &Path) -> Result<ProjectConfig> {
    if project_config_path.exists() {
        load_project_config(project_config_path)
    } else {
        Ok(ProjectConfig::default())
    }
}

pub fn write_project_config(path: &Path, config: &ProjectConfig, force: bool) -> Result<()> {
    if path.exists() && !force {
        bail!("{} already exists; pass --force to overwrite", path.display());
    }
    let text = toml::to_string_pretty(config).context("failed to render project config")?;
    fs::write(path, text).with_context(|| format!("failed to write {}", path.display()))
}

pub fn load_user_config(path: &Path) -> Result<UserConfig> {
    if !path.exists() {
        return Ok(UserConfig::default());
    }
    let data = fs::read_to_string(path)
        .with_context(|| format!("failed to read user config {}", path.display()))?;
    toml::from_str::<UserConfig>(&data)
        .with_context(|| format!("failed to parse user config {}", path.display()))
}

pub fn load_or_initialize_user_config(path: &Path) -> Result<UserConfig> {
    if !path.exists() {
        let default = UserConfig::default();
        let text =
            toml::to_string_pretty(&default).context("failed to render default user config")?;
        fs::write(path, text)
            .with_context(|| format!("failed to write default user config {}", path.display()))?;
        return Ok(default);
    }

    load_user_config(path)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn project_defaults_parse() {
        let cfg = ProjectConfig::default();
        assert_eq!(cfg.version, 1);
        assert_eq!(cfg.workspace.pool, "main");
        assert!(cfg.sync.respect_gitignore);
        assert_eq!(cfg.lifecycle.sync_on_exit, SyncOnExit::Ask);
    }

    #[test]
    fn discover_root_prefers_git_or_config() {
        let tmp = TempDir::new().expect("tmp");
        let root = tmp.path().join("repo");
        let nested = root.join("a/b/c");
        fs::create_dir_all(&nested).expect("mkdirs");
        fs::write(root.join(".git"), "x").expect("write git marker");

        let found = discover_project_root(&nested).expect("discover root");
        assert_eq!(found, root);
    }

    #[test]
    fn user_default_roundtrip() {
        let tmp = TempDir::new().expect("tmp");
        let path = tmp.path().join("config.toml");
        let cfg = load_or_initialize_user_config(&path).expect("init user config");
        assert!(path.exists());
        assert!(cfg.pools.contains_key("main"));
    }
}
