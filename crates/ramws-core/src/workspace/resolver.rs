use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::util::fs::{canonicalize_best_effort, normalize_component, stable_hash_str};
use crate::workspace::manifest::manifest_path;

pub fn resolve_workspace_path(
    pool_root: &Path,
    project_root: &Path,
    layout: &str,
) -> Result<PathBuf> {
    let canonical_project_root = canonicalize_best_effort(project_root)?;
    let project_root_text = canonical_project_root.to_string_lossy();

    let user = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
    let project =
        canonical_project_root.file_name().and_then(|value| value.to_str()).unwrap_or("project");
    let hash = stable_hash_str(&project_root_text);

    let rendered = render_layout(layout, &user, project, &hash);
    let candidate = pool_root.join(rendered);

    if !is_collision(&candidate, &project_root_text)? {
        return Ok(candidate);
    }

    let suffix = &hash[..8.min(hash.len())];
    let file_name = candidate.file_name().and_then(|value| value.to_str()).unwrap_or("workspace");
    let parent = candidate.parent().unwrap_or(pool_root);
    Ok(parent.join(format!("{file_name}-{suffix}")))
}

pub fn render_layout(layout: &str, user: &str, project: &str, hash: &str) -> PathBuf {
    let text = layout
        .replace("${user}", &normalize_component(user))
        .replace("${project}", &normalize_component(project))
        .replace("${hash}", hash);

    let mut output = PathBuf::new();
    for component in text.split('/') {
        if component.is_empty() {
            continue;
        }
        output.push(component);
    }

    if output.as_os_str().is_empty() {
        output.push(hash);
    }

    output
}

fn is_collision(candidate: &Path, canonical_project_root_text: &str) -> Result<bool> {
    let manifest = manifest_path(candidate);
    if !manifest.exists() {
        return Ok(false);
    }

    let text = std::fs::read_to_string(&manifest)
        .with_context(|| format!("failed to read manifest {}", manifest.display()))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .with_context(|| format!("invalid manifest {}", manifest.display()))?;
    let existing =
        value.get("project_root").and_then(serde_json::Value::as_str).unwrap_or_default();

    Ok(existing != canonical_project_root_text)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn render_tokens() {
        let path = render_layout("${user}/${project}/${hash}", "alice", "repo", "abcd");
        assert_eq!(path, PathBuf::from("alice/repo/abcd"));
    }

    #[test]
    fn collision_appends_suffix() {
        let tmp = TempDir::new().expect("tmp");
        let pool = tmp.path().join("pool");
        let p1 = tmp.path().join("projects/repo-a");
        let p2 = tmp.path().join("projects/repo-a-copy");
        fs::create_dir_all(&pool).expect("pool");
        fs::create_dir_all(&p1).expect("p1");
        fs::create_dir_all(&p2).expect("p2");

        let candidate = pool.join("user/repo-a");
        fs::create_dir_all(candidate.join(".ramws")).expect("manifest dir");
        fs::write(candidate.join(".ramws/manifest.json"), r#"{"project_root":"/other/path"}"#)
            .expect("manifest");

        let resolved =
            resolve_workspace_path(&pool, &p1, "user/repo-a").expect("resolved workspace path");
        assert_ne!(resolved, candidate);
    }
}
