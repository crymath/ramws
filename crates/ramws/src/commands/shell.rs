use std::io::{self, IsTerminal, Write};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::commands::{
    load_runtime, plan_sync_out, require_project_config, resolve_workspace_context,
    sync_on_exit_decision, sync_out,
};

pub fn execute(pool: Option<String>) -> Result<i32> {
    let runtime = load_runtime(true)?;
    let mut ctx = resolve_workspace_context(runtime, pool.as_deref())?;
    crate::commands::sync_in(&mut ctx)?;

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let status = Command::new(&shell)
        .arg("-i")
        .current_dir(&ctx.workspace_root)
        .env("RAMWS_ACTIVE", "1")
        .env("RAMWS_ORIG_ROOT", ctx.runtime.project_root.to_string_lossy().to_string())
        .env("RAMWS_WS_ROOT", ctx.workspace_root.to_string_lossy().to_string())
        .env("RAMWS_POOL", &ctx.pool_name)
        .status()
        .with_context(|| format!("failed to launch shell {}", shell))?;

    let shell_code = status.code().unwrap_or(1);
    maybe_sync_back(&mut ctx)?;
    Ok(shell_code)
}

fn maybe_sync_back(ctx: &mut crate::commands::WorkspaceContext) -> Result<()> {
    let project_config = require_project_config(&ctx.runtime)?;
    let plan = plan_sync_out(ctx)?;
    let decision =
        sync_on_exit_decision(project_config.lifecycle.sync_on_exit, plan.total_changes() > 0);

    match decision {
        "skip" => Ok(()),
        "never" => {
            eprintln!("workspace changed; run 'ramws sync out' to sync back");
            Ok(())
        }
        "always" => {
            let run = sync_out(ctx)?;
            eprintln!(
                "sync-out: +{} ~{} -{}{}",
                run.stats.created,
                run.stats.updated,
                run.stats.deleted,
                if run.skipped_conflicts.is_empty() {
                    String::new()
                } else {
                    format!(", skipped_conflicts={}", run.skipped_conflicts.len())
                }
            );
            Ok(())
        }
        "ask" => {
            if !io::stdin().is_terminal() {
                bail!(
                    "sync_on_exit=ask requires interactive stdin; use sync_on_exit=always|never or run 'ramws sync out' manually"
                );
            }

            eprint!(
                "Sync back? [y/N] changes: +{} ~{} -{} conflicts={} ",
                plan.creates.len(),
                plan.updates.len(),
                plan.deletes.len(),
                plan.conflicts.len()
            );
            io::stderr().flush().context("failed to flush prompt")?;

            let mut line = String::new();
            io::stdin().read_line(&mut line).context("failed to read prompt response")?;
            let normalized = line.trim().to_ascii_lowercase();
            if normalized == "y" || normalized == "yes" {
                let run = sync_out(ctx)?;
                eprintln!(
                    "sync-out: +{} ~{} -{}{}",
                    run.stats.created,
                    run.stats.updated,
                    run.stats.deleted,
                    if run.skipped_conflicts.is_empty() {
                        String::new()
                    } else {
                        format!(", skipped_conflicts={}", run.skipped_conflicts.len())
                    }
                );
            } else {
                eprintln!("skipped sync-back; run 'ramws sync out' later");
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
