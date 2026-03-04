use anyhow::Result;

use ramws_core::pool::PoolManager;

use crate::commands::load_runtime;

pub fn pool_create(pool: Option<String>) -> Result<i32> {
    let runtime = load_runtime(false)?;
    let pool_name = pool.unwrap_or_else(|| runtime.user_config.defaults.pool.clone());
    let instance = runtime.pool_manager.create_pool(&pool_name)?;

    println!("pool: {}", instance.name);
    println!("root: {}", instance.root.display());
    if let Some(managed) = instance.managed {
        println!("device: {}", managed.device);
        println!("mountpoint: {}", managed.mountpoint.display());
    }

    Ok(0)
}

pub fn pool_destroy(pool: Option<String>, force: bool) -> Result<i32> {
    let runtime = load_runtime(false)?;
    let pool_name = pool.unwrap_or_else(|| runtime.user_config.defaults.pool.clone());
    runtime.pool_manager.destroy_pool(&pool_name, force)?;
    println!("destroyed pool {}", pool_name);
    Ok(0)
}
