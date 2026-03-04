use crate::config::HashMode;
use crate::sync::{EntryRecord, EntryType, PlanOptions, PlannedAction, Snapshot, SyncPlan};

pub fn plan_sync(src: &Snapshot, dst: &Snapshot, options: &PlanOptions) -> SyncPlan {
    let mut plan = SyncPlan::default();

    for (path, src_entry) in &src.entries {
        match dst.entries.get(path) {
            None => {
                plan.creates.push(PlannedAction { path: path.clone(), entry: src_entry.clone() })
            }
            Some(dst_entry) => {
                if needs_update(src_entry, dst_entry, options.hash_mode) {
                    plan.updates
                        .push(PlannedAction { path: path.clone(), entry: src_entry.clone() });
                }
            }
        }
    }

    if options.delete_extra {
        for path in dst.entries.keys() {
            if !src.entries.contains_key(path) && !path.starts_with(".ramws/") && path != ".ramws" {
                plan.deletes.push(path.clone());
            }
        }

        plan.deletes.sort_by_key(|path| std::cmp::Reverse(path_depth(path)));
    }

    plan
}

fn needs_update(src: &EntryRecord, dst: &EntryRecord, hash_mode: HashMode) -> bool {
    if src.entry_type != dst.entry_type {
        return true;
    }

    match src.entry_type {
        EntryType::File => {
            let Some(src_meta) = src.meta.as_ref() else {
                return true;
            };
            let Some(dst_meta) = dst.meta.as_ref() else {
                return true;
            };
            if src_meta.size != dst_meta.size
                || src_meta.mtime_secs != dst_meta.mtime_secs
                || src_meta.mtime_nanos != dst_meta.mtime_nanos
                || src_meta.mode != dst_meta.mode
            {
                return true;
            }

            if hash_mode == HashMode::Blake3 {
                return src_meta.blake3 != dst_meta.blake3;
            }

            false
        }
        EntryType::Symlink => src.link_target != dst.link_target,
        EntryType::Dir => src.meta != dst.meta,
    }
}

fn path_depth(path: &str) -> usize {
    path.chars().filter(|c| *c == '/').count()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::config::HashMode;
    use crate::sync::{EntryRecord, EntryType, FileMeta, PlanOptions, Snapshot};

    use super::plan_sync;

    fn file(meta_seed: u64) -> EntryRecord {
        EntryRecord {
            entry_type: EntryType::File,
            meta: Some(FileMeta {
                size: meta_seed,
                mtime_secs: meta_seed as i64,
                mtime_nanos: 0,
                mode: 0o100644,
                blake3: None,
            }),
            link_target: None,
        }
    }

    #[test]
    fn plan_creates_updates_and_deletes() {
        let src = Snapshot {
            entries: BTreeMap::from([
                ("new.txt".to_string(), file(1)),
                ("same.txt".to_string(), file(2)),
                ("changed.txt".to_string(), file(3)),
            ]),
        };

        let dst = Snapshot {
            entries: BTreeMap::from([
                ("same.txt".to_string(), file(2)),
                ("changed.txt".to_string(), file(10)),
                ("old.txt".to_string(), file(1)),
            ]),
        };

        let plan = plan_sync(
            &src,
            &dst,
            &PlanOptions { delete_extra: true, hash_mode: HashMode::Metadata },
        );

        assert_eq!(plan.creates.len(), 1);
        assert_eq!(plan.creates[0].path, "new.txt");
        assert_eq!(plan.updates.len(), 1);
        assert_eq!(plan.updates[0].path, "changed.txt");
        assert_eq!(plan.deletes.len(), 1);
        assert_eq!(plan.deletes[0], "old.txt");
    }
}
