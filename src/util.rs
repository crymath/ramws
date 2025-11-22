use anyhow::{Context, Result};
use nix::libc;
use nix::sys::statfs::{statfs, Statfs};
use sha1::Digest;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FsStatus {
    pub fs_type: String,
    pub total: u64,
    pub available: u64,
    pub used: u64,
}

pub fn find_project_root(start: &Path) -> Result<PathBuf> {
    let mut current = start
        .canonicalize()
        .context("failed to canonicalize start path")?;
    loop {
        if current.join(".git").is_dir() {
            return Ok(current);
        }
        if let Some(parent) = current.parent() {
            current = parent.to_path_buf();
        } else {
            return Ok(start.canonicalize()?);
        }
    }
}

pub fn project_slug(path: &Path) -> Result<String> {
    let canonical = path.canonicalize()?;
    let name = canonical
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project");
    let digest = sha1::Sha1::digest(canonical.to_string_lossy().as_bytes());
    let hex = format!("{:x}", digest);
    let short = &hex[..7];
    Ok(format!("{}-{}", name, short))
}

pub fn expand_placeholders(template: &str, project_slug: &str) -> String {
    let mut value = template.replace("${PROJECT}", project_slug);
    if let Ok(user) = env::var("USER") {
        value = value.replace("${USER}", &user);
    }
    value
}

pub fn ensure_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)
        .with_context(|| format!("failed to create directory {}", path.display()))?;
    Ok(())
}

pub fn fs_status(path: &Path) -> Result<FsStatus> {
    let stat: Statfs =
        statfs(path).with_context(|| format!("statfs failed for {}", path.display()))?;
    let block_size = stat.block_size() as u64;
    let blocks = stat.blocks() as u64;
    let bfree = stat.blocks_free() as u64;
    let bavail = stat.blocks_available() as u64;
    let total = blocks * block_size;
    let available = bavail * block_size;
    let used = total - (bfree * block_size);
    let fs_type = match stat.filesystem_type().0 as i64 {
        libc::TMPFS_MAGIC => "tmpfs".to_string(),
        other => format!("0x{:x}", other),
    };
    Ok(FsStatus {
        fs_type,
        total,
        available,
        used,
    })
}

pub fn is_tmpfs(path: &Path) -> Result<bool> {
    let stat: Statfs =
        statfs(path).with_context(|| format!("statfs failed for {}", path.display()))?;
    Ok(stat.filesystem_type().0 as i64 == libc::TMPFS_MAGIC)
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.2} {}", UNITS[unit])
}

pub fn prompt_confirm(message: &str, default: bool) -> Result<bool> {
    use dialoguer::Confirm;
    let response = Confirm::new()
        .with_prompt(message)
        .default(default)
        .interact()?;
    Ok(response)
}

pub fn path_with_trailing_slash(path: &Path) -> String {
    let mut s = path.to_string_lossy().into_owned();
    if !s.ends_with('/') {
        s.push('/');
    }
    s
}

pub fn ensure_within_root(root: &Path, candidate: &Path) -> Result<()> {
    let root = root.canonicalize()?;
    let candidate = candidate.canonicalize()?;
    if candidate.starts_with(&root) {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "path {} escapes project root {}",
            candidate.display(),
            root.display()
        ))
    }
}
