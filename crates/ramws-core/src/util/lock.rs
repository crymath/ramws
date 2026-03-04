use std::fs::OpenOptions;
use std::path::Path;

use anyhow::{Context, Result};
use fd_lock::RwLock;

pub fn with_exclusive_lock<T, F>(path: &Path, f: F) -> Result<T>
where
    F: FnOnce() -> Result<T>,
{
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create lock dir: {}", parent.display()))?;
    }

    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(path)
        .with_context(|| format!("failed to open lock file: {}", path.display()))?;

    let mut lock = RwLock::new(file);
    let _guard =
        lock.write().with_context(|| format!("failed to acquire lock: {}", path.display()))?;

    f()
}
