use std::os::unix::fs::MetadataExt;
use std::path::Path;

use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use tracing::warn;

use crate::config::HashMode;
use crate::sync::{EntryRecord, EntryType, FileMeta, ScanOptions, Snapshot};

pub fn scan_snapshot(root: &Path, options: &ScanOptions) -> Result<Snapshot> {
    let excludes = build_excludes(&options.exclude)?;
    let mut builder = WalkBuilder::new(root);
    builder.hidden(false);
    builder.follow_links(options.follow_symlinks);

    if !options.respect_gitignore {
        builder.git_ignore(false).git_exclude(false).git_global(false);
    }

    let mut snapshot = Snapshot::default();
    for entry in builder.build() {
        let entry = match entry {
            Ok(v) => v,
            Err(err) => {
                warn!("failed to walk entry: {}", err);
                continue;
            }
        };
        let path = entry.path();

        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        if relative.as_os_str().is_empty() {
            continue;
        }

        let relative_text = relative.to_string_lossy().replace('\\', "/");
        if is_excluded(&relative_text, &excludes) {
            continue;
        }

        let metadata = std::fs::symlink_metadata(path)
            .with_context(|| format!("failed to stat {}", path.display()))?;
        let file_type = metadata.file_type();

        let record = if file_type.is_dir() {
            EntryRecord {
                entry_type: EntryType::Dir,
                meta: Some(meta_from_metadata(&metadata, None)),
                link_target: None,
            }
        } else if file_type.is_file() {
            let hash = if options.hash_mode == HashMode::Blake3 {
                Some(
                    hash_file(path)
                        .with_context(|| format!("failed to hash {}", path.display()))?,
                )
            } else {
                None
            };
            EntryRecord {
                entry_type: EntryType::File,
                meta: Some(meta_from_metadata(&metadata, hash)),
                link_target: None,
            }
        } else if file_type.is_symlink() {
            let link_target = std::fs::read_link(path)
                .with_context(|| format!("failed to read symlink {}", path.display()))?;
            EntryRecord {
                entry_type: EntryType::Symlink,
                meta: Some(meta_from_metadata(&metadata, None)),
                link_target: Some(link_target.to_string_lossy().to_string()),
            }
        } else {
            warn!("skipping special file that is not dir/file/symlink: {}", path.display());
            continue;
        };

        snapshot.entries.insert(relative_text, record);
    }

    Ok(snapshot)
}

fn build_excludes(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder
            .add(Glob::new(pattern).with_context(|| format!("invalid exclude glob: {pattern}"))?);
    }
    builder.add(Glob::new(".ramws/**").expect("static glob is valid"));
    builder.build().context("failed building exclude glob set")
}

fn is_excluded(relative: &str, excludes: &GlobSet) -> bool {
    relative == ".ramws" || relative.starts_with(".ramws/") || excludes.is_match(relative)
}

fn meta_from_metadata(metadata: &std::fs::Metadata, blake3: Option<String>) -> FileMeta {
    FileMeta {
        size: metadata.len(),
        mtime_secs: metadata.mtime(),
        mtime_nanos: metadata.mtime_nsec(),
        mode: metadata.mode(),
        blake3,
    }
}

fn hash_file(path: &Path) -> Result<String> {
    let bytes =
        std::fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}
