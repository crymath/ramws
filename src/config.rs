use crate::util::{expand_placeholders, project_slug};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkspaceSection {
    pub root: Option<String>,
}

impl Default for WorkspaceSection {
    fn default() -> Self {
        Self { root: None }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BuildDirType {
    Scratch,
    Cache,
}

impl Default for BuildDirType {
    fn default() -> Self {
        BuildDirType::Scratch
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SourceSpec {
    pub path: PathBuf,
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BuildDirSpec {
    pub path: PathBuf,
    #[serde(default)]
    pub r#type: BuildDirType,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncOnExit {
    Ask,
    Auto,
    Never,
}

impl Default for SyncOnExit {
    fn default() -> Self {
        SyncOnExit::Ask
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SyncConfig {
    #[serde(default)]
    pub on_exit: SyncOnExit,
    #[serde(default = "default_delete")]
    pub delete: bool,
}

fn default_delete() -> bool {
    true
}

impl Default for SyncConfig {
    fn default() -> Self {
        SyncConfig {
            on_exit: SyncOnExit::Ask,
            delete: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct GitConfig {
    #[serde(default)]
    pub require_clean: bool,
    #[serde(default)]
    pub auto_stage_synced: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub workspace: WorkspaceSection,
    #[serde(default = "default_sources")]
    pub sources: Vec<SourceSpec>,
    #[serde(default)]
    pub build_dirs: Vec<BuildDirSpec>,
    #[serde(default)]
    pub sync: SyncConfig,
    #[serde(default)]
    pub git: GitConfig,
}

fn default_sources() -> Vec<SourceSpec> {
    vec![SourceSpec {
        path: PathBuf::from("."),
        include: vec![],
        exclude: vec![
            ".git/**".to_string(),
            "build/**".to_string(),
            "target/**".to_string(),
            "node_modules/**".to_string(),
        ],
    }]
}

impl Default for Config {
    fn default() -> Self {
        Config {
            workspace: WorkspaceSection::default(),
            sources: default_sources(),
            build_dirs: vec![],
            sync: SyncConfig::default(),
            git: GitConfig::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub config_path: PathBuf,
    pub orig_root: PathBuf,
    pub workspace_root: PathBuf,
    pub project_slug: String,
    pub raw: Config,
}

impl Config {
    pub fn load_from_file(path: &Path, orig_root: PathBuf) -> Result<ResolvedConfig> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("failed to read config {}", path.display()))?;
        let config: Config = serde_yaml::from_str(&text)
            .with_context(|| format!("failed to parse yaml at {}", path.display()))?;
        let project_slug = project_slug(&orig_root)?;
        let ws_root_str = config
            .workspace
            .root
            .clone()
            .unwrap_or_else(|| format!("/dev/shm/ramws-${{USER}}/{project_slug}"));
        let expanded = expand_placeholders(&ws_root_str, &project_slug);
        let workspace_root = PathBuf::from(expanded);
        Ok(ResolvedConfig {
            config_path: path.to_path_buf(),
            orig_root,
            workspace_root,
            project_slug,
            raw: config,
        })
    }
}
