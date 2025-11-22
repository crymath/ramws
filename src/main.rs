mod config;
mod shell;
mod status;
mod syncer;
mod util;
mod workspace;

use crate::config::{BuildDirType, Config, ResolvedConfig, SyncOnExit};
use crate::shell::{run_shell, ShellOptions};
use crate::status::collect_status;
use crate::syncer::{refresh_from_orig, sync_back};
use crate::util::find_project_root;
use crate::workspace::Workspace;
use anyhow::{bail, Context, Result};
use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
#[command(
    author,
    version,
    about = "ramws – per-project RAM workspace orchestrator"
)]
struct Cli {
    #[arg(short = 'C', long)]
    chdir: Option<PathBuf>,
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long)]
    json: bool,
    #[arg(short, long, action = ArgAction::Count)]
    verbose: u8,
    #[arg(short = 'q', long, action = ArgAction::Count)]
    quiet: u8,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init {
        #[arg(long)]
        force: bool,
        #[arg(long)]
        template: Option<String>,
    },
    Start {
        #[arg(long)]
        noninteractive: bool,
        #[arg(long)]
        refresh_sources_only: bool,
    },
    Shell {
        #[arg(long)]
        shell: Option<String>,
        #[arg(long)]
        no_prompt: bool,
        #[arg(long)]
        noninteractive: bool,
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
    },
    Sync {
        #[arg(long, conflicts_with = "from")]
        back: bool,
        #[arg(long)]
        from: bool,
        #[arg(long = "only", value_name = "PATH", num_args = 1..)]
        only: Vec<PathBuf>,
        #[arg(long = "role", value_name = "ROLE", value_enum, num_args = 1..)]
        roles: Vec<Role>,
        #[arg(long)]
        noninteractive: bool,
    },
    Status {},
    Destroy {
        #[arg(long)]
        force: bool,
        #[arg(long)]
        noninteractive: bool,
    },
}

#[derive(Copy, Clone, Debug, ValueEnum, PartialEq, Eq)]
enum Role {
    Source,
    Cache,
    Scratch,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let level = match cli.verbose.saturating_sub(cli.quiet) {
        0 => Level::INFO,
        1 => Level::DEBUG,
        _ => Level::TRACE,
    };
    let subscriber = FmtSubscriber::builder()
        .with_max_level(level)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    match &cli.command {
        Commands::Init { force, template } => init_command(&cli, *force, template.as_deref()),
        Commands::Start {
            noninteractive,
            refresh_sources_only,
        } => start_command(&cli, *noninteractive, *refresh_sources_only),
        Commands::Shell {
            shell,
            no_prompt,
            noninteractive,
            command,
        } => shell_command(
            &cli,
            shell.clone(),
            *no_prompt,
            *noninteractive,
            command.clone(),
        ),
        Commands::Sync {
            back: _,
            from,
            only,
            roles,
            noninteractive,
        } => sync_command(&cli, !from, only.clone(), roles.clone(), *noninteractive),
        Commands::Status {} => status_command(&cli),
        Commands::Destroy {
            force,
            noninteractive,
        } => destroy_command(&cli, *force, *noninteractive),
    }
}

fn init_command(cli: &Cli, force: bool, template: Option<&str>) -> Result<()> {
    let start = cli
        .chdir
        .clone()
        .unwrap_or(env::current_dir().context("failed to read current dir")?);
    let project_root = find_project_root(&start)?;
    let config_path = project_root.join(".ramws.yml");
    if config_path.exists() && !force {
        bail!(
            "{} already exists; use --force to overwrite",
            config_path.display()
        );
    }
    let cfg = Config::default();
    let yaml = serde_yaml::to_string(&cfg)?;
    fs::write(&config_path, yaml)?;
    println!("created {}", config_path.display());
    if let Some(tpl) = template {
        println!("template hint: {tpl} (no template logic implemented, adjust config manually)");
    }
    Ok(())
}

fn load_resolved_config(cli: &Cli) -> Result<ResolvedConfig> {
    let base = cli
        .chdir
        .clone()
        .unwrap_or(env::current_dir().context("failed to read cwd")?);
    let orig_root = find_project_root(&base)?;
    let cfg_path = if let Some(p) = &cli.config {
        p.clone()
    } else {
        discover_config(&orig_root)?
    };
    Config::load_from_file(&cfg_path, orig_root)
}

fn discover_config(root: &Path) -> Result<PathBuf> {
    let mut current = root.to_path_buf();
    loop {
        let candidate = current.join(".ramws.yml");
        if candidate.exists() {
            return Ok(candidate);
        }
        if let Some(parent) = current.parent() {
            current = parent.to_path_buf();
        } else {
            bail!(".ramws.yml not found; run ramws init");
        }
    }
}

fn start_command(cli: &Cli, _noninteractive: bool, refresh_sources_only: bool) -> Result<()> {
    let cfg = load_resolved_config(cli)?;
    let workspace = Workspace::new(cfg);
    workspace.ensure(refresh_sources_only)?;
    println!(
        "workspace ready at {}",
        workspace.config.workspace_root.display()
    );
    Ok(())
}

fn shell_command(
    cli: &Cli,
    shell: Option<String>,
    no_prompt: bool,
    noninteractive: bool,
    command: Vec<String>,
) -> Result<()> {
    let cfg = load_resolved_config(cli)?;
    let workspace = Workspace::new(cfg.clone());
    let code = run_shell(
        &workspace,
        ShellOptions {
            shell,
            no_prompt,
            noninteractive,
            command,
        },
    )?;
    handle_on_exit(&cfg, noninteractive)?;
    std::process::exit(code);
}

fn handle_on_exit(cfg: &ResolvedConfig, noninteractive: bool) -> Result<()> {
    match cfg.raw.sync.on_exit {
        SyncOnExit::Never => Ok(()),
        SyncOnExit::Auto => {
            let paths: Vec<PathBuf> = cfg.raw.sources.iter().map(|s| s.path.clone()).collect();
            sync_back(cfg, &paths, true)
        }
        SyncOnExit::Ask => {
            let paths: Vec<PathBuf> = cfg.raw.sources.iter().map(|s| s.path.clone()).collect();
            let mut pending = false;
            for rel in &paths {
                let ws = cfg.workspace_root.join(rel);
                let orig = cfg.orig_root.join(rel);
                let opts = crate::syncer::SyncOptions {
                    delete: cfg.raw.sync.delete,
                    include: vec![],
                    exclude: vec![],
                    itemize: true,
                    dry_run: true,
                };
                let diff = crate::syncer::diff_path(&ws, &orig, opts)?;
                if diff.added + diff.changed + diff.deleted > 0 {
                    pending = true;
                    break;
                }
            }
            if pending {
                if crate::syncer::confirm_if_needed("Sync changes back to disk?", noninteractive)? {
                    sync_back(cfg, &paths, noninteractive)
                } else {
                    Ok(())
                }
            } else {
                Ok(())
            }
        }
    }
}

fn sync_command(
    cli: &Cli,
    back: bool,
    only: Vec<PathBuf>,
    roles: Vec<Role>,
    noninteractive: bool,
) -> Result<()> {
    let cfg = load_resolved_config(cli)?;
    let include_sources = roles.is_empty() || roles.contains(&Role::Source);
    let mut selected: Vec<PathBuf> = if !only.is_empty() {
        only
    } else {
        let mut paths: Vec<PathBuf> = Vec::new();
        if include_sources {
            paths.extend(cfg.raw.sources.iter().map(|s| s.path.clone()));
        }
        if roles.contains(&Role::Cache) {
            for b in &cfg.raw.build_dirs {
                if b.r#type == BuildDirType::Cache {
                    paths.push(b.path.clone());
                }
            }
        }
        if roles.contains(&Role::Scratch) {
            for b in &cfg.raw.build_dirs {
                if b.r#type == BuildDirType::Scratch {
                    paths.push(b.path.clone());
                }
            }
        }
        paths.sort();
        paths.dedup();
        paths
    };
    if selected.is_empty() {
        selected = cfg.raw.sources.iter().map(|s| s.path.clone()).collect();
    }
    if back {
        sync_back(&cfg, &selected, noninteractive)
    } else {
        refresh_from_orig(&cfg, &selected)
    }
}

fn status_command(cli: &Cli) -> Result<()> {
    let cfg = load_resolved_config(cli)?;
    let report = collect_status(&cfg)?;
    if cli.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("Workspace: {}", report.workspace_root);
        println!("Exists: {}", report.workspace_exists);
        if let Some(fs) = report.fs_type {
            println!("Filesystem: {fs}");
        }
        if let Some(total) = report.total {
            println!(
                "Capacity: total {total}, used {}",
                report.used.unwrap_or_else(|| "n/a".to_string())
            );
            println!(
                "Available: {}",
                report.available.unwrap_or_else(|| "n/a".to_string())
            );
        }
        println!(
            "Diff summary: changed {}, added {}, deleted {}",
            report.diff_changed, report.diff_added, report.diff_deleted
        );
        println!("Sync on exit: {:?}", report.sync_policy);
    }
    Ok(())
}

fn destroy_command(cli: &Cli, force: bool, noninteractive: bool) -> Result<()> {
    let cfg = load_resolved_config(cli)?;
    let workspace = Workspace::new(cfg.clone());
    if !workspace.exists() {
        println!(
            "workspace not found at {}",
            workspace.config.workspace_root.display()
        );
        return Ok(());
    }
    if !force {
        let report = collect_status(&cfg)?;
        if report.diff_added + report.diff_changed + report.diff_deleted > 0 {
            if !crate::syncer::confirm_if_needed(
                "Unsynced changes detected. Delete workspace?",
                noninteractive,
            )? {
                return Ok(());
            }
        }
    }
    workspace.delete()
}
