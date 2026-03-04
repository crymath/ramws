#![forbid(unsafe_code)]

use std::process;

use anyhow::Result;
use clap::Parser;

mod cli;
mod commands;

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err:#}");
        process::exit(1);
    }
}

fn run() -> Result<()> {
    init_logging();

    let cli = cli::Cli::parse();
    let code = match cli.command {
        cli::Command::Init { force } => commands::init::execute(force)?,
        cli::Command::Up { pool } => commands::up::execute(pool)?,
        cli::Command::Shell { pool } => commands::shell::execute(pool)?,
        cli::Command::Run { pool, cmd } => commands::run::execute(pool, cmd)?,
        cli::Command::Sync { direction, pool } => commands::sync::execute(direction, pool)?,
        cli::Command::Status { pool } => commands::status::execute(pool)?,
        cli::Command::Destroy { pool, all } => commands::destroy::execute(pool, all)?,
        cli::Command::Admin { command } => match command {
            cli::AdminCommand::Pool { command } => match command {
                cli::PoolCommand::Create { pool } => commands::admin::pool_create(pool)?,
                cli::PoolCommand::Destroy { pool, force } => {
                    commands::admin::pool_destroy(pool, force)?
                }
            },
        },
    };

    process::exit(code);
}

fn init_logging() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();
}
