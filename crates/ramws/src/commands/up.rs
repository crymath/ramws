use anyhow::Result;

use crate::commands::{load_runtime, resolve_workspace_context, sync_in};

pub fn execute(pool: Option<String>) -> Result<i32> {
    let runtime = load_runtime(true)?;
    let mut ctx = resolve_workspace_context(runtime, pool.as_deref())?;
    let run = sync_in(&mut ctx)?;

    eprintln!("sync-in: +{} ~{} -{}", run.stats.created, run.stats.updated, run.stats.deleted);
    println!("{}", ctx.workspace_root.display());

    Ok(0)
}
