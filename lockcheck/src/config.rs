use anyhow::{Result, anyhow, Context};
use serde::Deserialize;

/// Identifies a lock type which will be checked
// TODO: don't require specifying lock method and constructor path
#[derive(Debug, Deserialize)]
pub struct LockCheckTarget {
    pub lock: String,
    pub guard: String,
    /// Path to lock constructor
    pub constructor: String,
    /// Path to lock method
    pub lock_method: String,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub locks: Vec<LockCheckTarget>,
}

/// Attempts to load config from the `lockcheck.toml` config file
/// 
/// This will search all parent directories that contain a `Cargo.toml` file, and try to load the `lockcheck.toml` from the same directory
pub fn load_config() -> Result<Config> {
    let current_dir = std::env::current_dir()?;

    for dir in current_dir.ancestors() {
        if dir.join("Cargo.toml").exists() {
            let lockcheck_config_path = dir.join("lockcheck.toml");
            if !lockcheck_config_path.exists() {
                continue;
            }

            let config_data = std::fs::read_to_string(lockcheck_config_path)?;
            let config: Config = toml::from_str(&config_data)
                .with_context(|| "invalid format of lockecheck config file")?;

            return Ok(config);
        }
    }

    Err(anyhow!("Could not find `lockcheck.toml` config file"))
}