use anyhow::{Context, Result};
use directories::ProjectDirs;
use std::path::PathBuf;

const APP_NAME: &str = "pint";

pub fn data_dir() -> Result<PathBuf> {
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
