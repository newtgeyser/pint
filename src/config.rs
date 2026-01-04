use anyhow::{Context, Result};
use directories::ProjectDirs;
use std::path::PathBuf;
use std::sync::OnceLock;

const APP_NAME: &str = "pint";
const ENV_DATA_DIR: &str = "PINT_DATA_DIR";

static DATA_DIR_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

/// Set a custom data directory (call early in main, before any data access)
pub fn set_data_dir(path: PathBuf) {
    let _ = DATA_DIR_OVERRIDE.set(path);
}

pub fn data_dir() -> Result<PathBuf> {
    // 1. Check for programmatic override (from --data-dir flag)
    if let Some(path) = DATA_DIR_OVERRIDE.get() {
        return Ok(path.clone());
    }

    // 2. Check environment variable
    if let Ok(path) = std::env::var(ENV_DATA_DIR) {
        return Ok(PathBuf::from(path));
    }

    // 3. Default to XDG/platform standard
    let proj_dirs = ProjectDirs::from("", "", APP_NAME)
        .context("Could not determine data directory for your platform")?;

    Ok(proj_dirs.data_dir().to_path_buf())
}

pub fn db_path() -> Result<PathBuf> {
    let mut path = data_dir()?;
    path.push("pint.db");
    Ok(path)
}

pub fn ensure_data_dir() -> Result<PathBuf> {
    let dir = data_dir()?;
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create data directory: {}", dir.display()))?;
    Ok(dir)
}
