use crate::sync::{Conflict, EntryRecord, Snapshot, SyncPlan};

pub fn detect_conflicts(
    baseline: Option<&Snapshot>,
    current_orig: &Snapshot,
    current_workspace: &Snapshot,
    plan: &SyncPlan,
) -> Vec<Conflict> {
    let mut conflicts = Vec::new();

    for path in plan.creates.iter().chain(plan.updates.iter()).map(|action| action.path.as_str()) {
        let baseline_entry = baseline.and_then(|snapshot| snapshot.entries.get(path));
        let orig_entry = current_orig.entries.get(path);
        let ws_entry = current_workspace.entries.get(path);

        let orig_changed = !same_entry(orig_entry, baseline_entry);
        let ws_changed = !same_entry(ws_entry, baseline_entry);

        if orig_changed && ws_changed && !same_entry(orig_entry, ws_entry) {
            conflicts.push(Conflict {
                path: path.to_string(),
                reason: "both original and workspace changed since last sync-in".to_string(),
            });
        }
    }

    conflicts
}

fn same_entry(a: Option<&EntryRecord>, b: Option<&EntryRecord>) -> bool {
    a == b
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::sync::{EntryRecord, EntryType, FileMeta, PlannedAction, Snapshot, SyncPlan};

    use super::detect_conflicts;

    fn file(seed: u64) -> EntryRecord {
        EntryRecord {
            entry_type: EntryType::File,
            meta: Some(FileMeta {
                size: seed,
                mtime_secs: seed as i64,
                mtime_nanos: 0,
                mode: 0o100644,
                blake3: None,
            }),
            link_target: None,
        }
    }

    #[test]
    fn detect_two_sided_change_conflict() {
        let baseline = Snapshot { entries: BTreeMap::from([("a.txt".to_string(), file(1))]) };
        let orig = Snapshot { entries: BTreeMap::from([("a.txt".to_string(), file(2))]) };
        let workspace = Snapshot { entries: BTreeMap::from([("a.txt".to_string(), file(3))]) };
        let plan = SyncPlan {
            creates: Vec::new(),
            updates: vec![PlannedAction { path: "a.txt".to_string(), entry: file(3) }],
            deletes: Vec::new(),
            conflicts: Vec::new(),
        };

        let conflicts = detect_conflicts(Some(&baseline), &orig, &workspace, &plan);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].path, "a.txt");
    }
}
