use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub fn canonicalize_best_effort(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        path.canonicalize()
            .with_context(|| format!("failed to canonicalize path: {}", path.display()))
    } else {
        Ok(path.to_path_buf())
    }
}

pub fn stable_hash_str(input: &str) -> String {
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

pub fn normalize_component(input: &str) -> String {
    input
        .chars()
        .map(
            |c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '-' }
            },
        )
        .collect::<String>()
}
