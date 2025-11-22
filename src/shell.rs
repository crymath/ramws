use crate::workspace::Workspace;
use anyhow::{Context, Result};
use std::env;
use std::process::Command;
use tracing::info;

pub struct ShellOptions {
    pub shell: Option<String>,
    pub no_prompt: bool,
    pub noninteractive: bool,
    pub command: Vec<String>,
}

pub fn run_shell(workspace: &Workspace, opts: ShellOptions) -> Result<i32> {
    workspace.ensure(false)?;
    let ws_root = workspace.config.workspace_root.clone();
    let shell_bin = opts
        .shell
        .clone()
        .or_else(|| env::var("SHELL").ok())
        .unwrap_or_else(|| "/bin/bash".to_string());
    let mut cmd = Command::new(shell_bin);
    if opts.command.is_empty() {
        cmd.arg("-i");
    } else {
        let combined = opts.command.join(" ");
        cmd.arg("-lc").arg(combined);
    }
    cmd.current_dir(&ws_root);
    cmd.env("RAMWS_ACTIVE", "1");
    cmd.env(
        "RAMWS_LEVEL",
        env::var("RAMWS_LEVEL")
            .map(|l| format!("{}", l.parse::<u32>().unwrap_or(0) + 1))
            .unwrap_or_else(|_| "1".to_string()),
    );
    cmd.env("RAMWS_ORIG_ROOT", &workspace.config.orig_root);
    cmd.env("RAMWS_WS_ROOT", &workspace.config.workspace_root);
    cmd.env("RAMWS_CONFIG", &workspace.config.config_path);
    if !opts.no_prompt && opts.command.is_empty() {
        let prefix = "(ramws)";
        if let Ok(ps1) = env::var("PS1") {
            cmd.env("PS1", format!("{} {}", prefix, ps1));
        } else {
            cmd.env("PS1", format!("{} \\\\u$ ", prefix));
        }
    }
    info!("launching shell in {}", ws_root.display());
    let status = cmd.status().context("failed to launch shell")?;
    Ok(status.code().unwrap_or(1))
}
