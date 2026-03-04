use anyhow::Result;
use ramws_core::config::{ProjectConfig, write_project_config};

use crate::commands::load_runtime;

pub fn execute(force: bool) -> Result<i32> {
    let runtime = load_runtime(false)?;
    let config = ProjectConfig::default();
    write_project_config(&runtime.paths.project_config_path, &config, force)?;

    println!("initialized {}", runtime.paths.project_config_path.display());
    Ok(0)
}
