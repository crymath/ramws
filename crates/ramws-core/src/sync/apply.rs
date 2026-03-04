use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use filetime::{FileTime, set_file_times};

use crate::sync::{ApplyOptions, ApplyStats, EntryType, PlannedAction, SyncPlan};

pub fn apply_plan(
    src_root: &Path,
    dst_root: &Path,
    plan: &SyncPlan,
    options: ApplyOptions,
) -> Result<ApplyStats> {
    let mut stats = ApplyStats::default();

    for relative in &plan.deletes {
        let destination = dst_root.join(relative);
        if destination.exists() || is_symlink_path(&destination) {
            remove_path(&destination)?;
            stats.deleted += 1;
        }
    }

    let mut actions: Vec<&PlannedAction> = plan.creates.iter().collect();
    actions.extend(plan.updates.iter());
    actions.sort_by_key(|action| action.path.matches('/').count());

    for action in actions {
        let source = src_root.join(&action.path);
        let destination = dst_root.join(&action.path);

        prepare_destination_for_entry(&destination, action.entry.entry_type)?;

        match action.entry.entry_type {
            EntryType::Dir => {
                std::fs::create_dir_all(&destination)
                    .with_context(|| format!("failed to create dir {}", destination.display()))?;
                if let Some(meta) = action.entry.meta.as_ref() {
                    let perms = std::fs::Permissions::from_mode(meta.mode);
                    let _ = std::fs::set_permissions(&destination, perms);
                }
            }
            EntryType::File => {
                let parent = destination.parent().ok_or_else(|| {
                    anyhow!("destination has no parent: {}", destination.display())
                })?;
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create parent {}", parent.display()))?;

                let tmp = tempfile::NamedTempFile::new_in(parent).with_context(|| {
                    format!("failed to create temp file in {}", parent.display())
                })?;
                std::fs::copy(&source, tmp.path()).with_context(|| {
                    format!("failed to copy file {} -> {}", source.display(), tmp.path().display())
                })?;

                if let Some(meta) = action.entry.meta.as_ref() {
                    let perms = std::fs::Permissions::from_mode(meta.mode);
                    std::fs::set_permissions(tmp.path(), perms).with_context(|| {
                        format!("failed to set permissions on {}", tmp.path().display())
                    })?;
                }

                tmp.persist(&destination).with_context(|| {
                    format!("failed to rename file into place {}", destination.display())
                })?;

                if options.set_mtime
                    && let Some(meta) = action.entry.meta.as_ref()
                {
                    let mtime = FileTime::from_unix_time(meta.mtime_secs, meta.mtime_nanos as u32);
                    set_file_times(&destination, mtime, mtime).with_context(|| {
                        format!("failed to set mtime on {}", destination.display())
                    })?;
                }
            }
            EntryType::Symlink => {
                let parent = destination.parent().ok_or_else(|| {
                    anyhow!("destination has no parent: {}", destination.display())
                })?;
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create parent {}", parent.display()))?;

                let target = action.entry.link_target.as_ref().ok_or_else(|| {
                    anyhow!("symlink action without link_target: {}", action.path)
                })?;
                symlink(target, &destination).with_context(|| {
                    format!("failed to create symlink {}", destination.display())
                })?;
            }
        }

        if plan.creates.iter().any(|item| item.path == action.path) {
            stats.created += 1;
        } else {
            stats.updated += 1;
        }
    }

    Ok(stats)
}

fn prepare_destination_for_entry(destination: &Path, entry_type: EntryType) -> Result<()> {
    if !(destination.exists() || is_symlink_path(destination)) {
        return Ok(());
    }

    let metadata = std::fs::symlink_metadata(destination)
        .with_context(|| format!("failed to stat {}", destination.display()))?;
    let file_type = metadata.file_type();

    let is_compatible = match entry_type {
        EntryType::Dir => file_type.is_dir(),
        EntryType::File => file_type.is_file(),
        EntryType::Symlink => file_type.is_symlink(),
    };

    if !is_compatible {
        remove_path(destination)?;
    }

    Ok(())
}

fn remove_path(path: &Path) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path)
        .with_context(|| format!("failed to stat {}", path.display()))?;

    if metadata.file_type().is_dir() {
        std::fs::remove_dir_all(path)
            .with_context(|| format!("failed to remove directory {}", path.display()))
    } else {
        std::fs::remove_file(path)
            .with_context(|| format!("failed to remove path {}", path.display()))
    }
}

fn is_symlink_path(path: &Path) -> bool {
    std::fs::symlink_metadata(path).map(|meta| meta.file_type().is_symlink()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::symlink;

    use tempfile::TempDir;

    use crate::sync::{EntryRecord, EntryType, FileMeta, PlannedAction, SyncPlan};

    use super::{ApplyOptions, apply_plan};

    #[test]
    fn apply_creates_files_and_symlinks() {
        let src = TempDir::new().expect("src tempdir");
        let dst = TempDir::new().expect("dst tempdir");

        fs::create_dir_all(src.path().join("dir")).expect("mkdir");
        fs::write(src.path().join("dir/file.txt"), b"hello").expect("write");
        symlink("file.txt", src.path().join("dir/link")).expect("symlink");

        let file_action = PlannedAction {
            path: "dir/file.txt".to_string(),
            entry: EntryRecord {
                entry_type: EntryType::File,
                meta: Some(FileMeta {
                    size: 5,
                    mtime_secs: 1,
                    mtime_nanos: 0,
                    mode: 0o100644,
                    blake3: None,
                }),
                link_target: None,
            },
        };
        let link_action = PlannedAction {
            path: "dir/link".to_string(),
            entry: EntryRecord {
                entry_type: EntryType::Symlink,
                meta: None,
                link_target: Some("file.txt".to_string()),
            },
        };

        let plan = SyncPlan {
            creates: vec![file_action, link_action],
            updates: Vec::new(),
            deletes: Vec::new(),
            conflicts: Vec::new(),
        };

        let stats = apply_plan(src.path(), dst.path(), &plan, ApplyOptions { set_mtime: false })
            .expect("apply");
        assert_eq!(stats.created, 2);
        assert_eq!(fs::read(dst.path().join("dir/file.txt")).expect("read"), b"hello".to_vec());
        assert!(
            fs::symlink_metadata(dst.path().join("dir/link"))
                .expect("link metadata")
                .file_type()
                .is_symlink()
        );
    }
}
