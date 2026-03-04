use anyhow::Result;

use crate::cli::SyncDirection;
use crate::commands::{load_runtime, resolve_workspace_context, sync_in, sync_out};

pub fn execute(direction: SyncDirection, pool: Option<String>) -> Result<i32> {
    let runtime = load_runtime(true)?;
    let mut ctx = resolve_workspace_context(runtime, pool.as_deref())?;

    let run = match direction {
        SyncDirection::In => sync_in(&mut ctx)?,
        SyncDirection::Out => sync_out(&mut ctx)?,
    };

    println!(
        "sync {:?}: +{} ~{} -{}{}",
        direction,
        run.stats.created,
        run.stats.updated,
        run.stats.deleted,
        if run.skipped_conflicts.is_empty() {
            String::new()
        } else {
            format!(", skipped_conflicts={}", run.skipped_conflicts.len())
        }
    );

    Ok(0)
}
