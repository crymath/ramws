use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "ramws", version, about = "RAM-backed project workspaces on macOS")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Init {
        #[arg(long)]
        force: bool,
    },
    Up {
        #[arg(long)]
        pool: Option<String>,
    },
    Shell {
        #[arg(long)]
        pool: Option<String>,
    },
    Run {
        #[arg(long)]
        pool: Option<String>,
        #[arg(required = true, trailing_var_arg = true, num_args = 1..)]
        cmd: Vec<String>,
    },
    Sync {
        #[command(subcommand)]
        direction: SyncDirection,
        #[arg(long)]
        pool: Option<String>,
    },
    Status {
        #[arg(long)]
        pool: Option<String>,
    },
    Destroy {
        #[arg(long)]
        pool: Option<String>,
        #[arg(long)]
        all: bool,
    },
    Admin {
        #[command(subcommand)]
        command: AdminCommand,
    },
}

#[derive(Debug, Clone, Copy, Subcommand)]
pub enum SyncDirection {
    In,
    Out,
}

#[derive(Debug, Subcommand)]
pub enum AdminCommand {
    Pool {
        #[command(subcommand)]
        command: PoolCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum PoolCommand {
    Create {
        #[arg(long)]
        pool: Option<String>,
    },
    Destroy {
        #[arg(long)]
        pool: Option<String>,
        #[arg(long)]
        force: bool,
    },
}
