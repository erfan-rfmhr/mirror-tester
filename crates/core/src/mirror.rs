//! Mirror list loading utilities.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Errors that can occur while loading mirror lists.
#[derive(Debug)]
pub enum MirrorError {
    Io(std::io::Error),
    Parse(serde_json::Error),
}

impl std::fmt::Display for MirrorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MirrorError::Io(e) => write!(f, "IO error: {e}"),
            MirrorError::Parse(e) => write!(f, "Parse error: {e}"),
        }
    }
}

impl std::error::Error for MirrorError {}

impl From<std::io::Error> for MirrorError {
    fn from(e: std::io::Error) -> Self {
        MirrorError::Io(e)
    }
}

impl From<serde_json::Error> for MirrorError {
    fn from(e: serde_json::Error) -> Self {
        MirrorError::Parse(e)
    }
}

/// Supported package managers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    PyPi,
    Npm,
}

impl PackageManager {
    /// Parses a package manager name from a string (case-insensitive).
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "pypi" | "pip" => Some(PackageManager::PyPi),
            "npm" => Some(PackageManager::Npm),
            _ => None,
        }
    }

    /// Returns the relative file path for this package manager's mirror list
    /// within the `data/` directory.
    pub fn data_file_name(&self) -> &'static str {
        match self {
            PackageManager::PyPi => "pypi.json",
            PackageManager::Npm => "npm.json",
        }
    }

    /// Returns the on-disk file path for this package manager's mirror list,
    /// resolved relative to the workspace root.
    pub fn data_file(&self) -> PathBuf {
        data_dir().join(self.data_file_name())
    }

    /// Returns the display name of the package manager.
    pub fn name(&self) -> &'static str {
        match self {
            PackageManager::PyPi => "pypi",
            PackageManager::Npm => "npm",
        }
    }
}

/// A sample package to download during benchmarking, together with the
/// list of mirror URLs to test it against.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorConfig {
    /// Name of the package that will actually be downloaded from each
    /// mirror when benchmarking (e.g. `"requests"` for PyPI, `"lodash"`
    /// for npm).
    pub package: String,
    /// Mirror base URLs to benchmark.
    pub mirrors: Vec<String>,
}

/// Returns the path to the `data/` directory, resolved relative to the
/// workspace root.
///
/// Resolution order:
/// 1. `MIRROR_DATA_DIR` environment variable (if set).
/// 2. Walk up from the current executable looking for a `data/` directory
///    containing both `pypi.json` and `npm.json`.
/// 3. Walk up from the current working directory as a fallback.
fn data_dir() -> PathBuf {
    if let Ok(custom) = std::env::var("MIRROR_DATA_DIR") {
        return PathBuf::from(custom);
    }

    if let Some(dir) = find_data_dir_from_exe() {
        return dir;
    }

    if let Some(dir) = find_data_dir_from_cwd() {
        return dir;
    }

    PathBuf::from("data")
}

/// Walks up from the current executable looking for a `data/` directory
/// containing both expected data files.
fn find_data_dir_from_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let mut dir = exe.parent()?;
    loop {
        let candidate = dir.join("data");
        if is_data_dir(&candidate) {
            return Some(candidate);
        }
        dir = dir.parent()?;
    }
}

/// Walks up from the current working directory looking for a `data/`
/// directory.
fn find_data_dir_from_cwd() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let candidate = dir.join("data");
        if is_data_dir(&candidate) {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Returns true if `path` looks like the project's `data/` directory
/// (contains both expected mirror files).
fn is_data_dir(path: &Path) -> bool {
    path.join("pypi.json").is_file() && path.join("npm.json").is_file()
}

/// Loads the mirror configuration (sample package + mirror URLs) for the
/// given package manager.
pub fn load_mirrors(pm: PackageManager) -> Result<MirrorConfig, MirrorError> {
    let path = pm.data_file();
    load_mirrors_from(&path)
}

/// Loads a mirror configuration from an arbitrary JSON file path.
pub fn load_mirrors_from(path: &Path) -> Result<MirrorConfig, MirrorError> {
    let content = fs::read_to_string(path)?;
    let config: MirrorConfig = serde_json::from_str(&content)?;
    Ok(config)
}

/// Appends a mirror URL to the given package manager's mirror list and
/// persists the updated configuration back to its data file.
pub fn add_mirror(pm: PackageManager, mirror: &str) -> Result<(), MirrorError> {
    let path = pm.data_file();
    let mut config = load_mirrors_from(&path)?;
    config.mirrors.push(mirror.to_string());
    let content = serde_json::to_string_pretty(&config)?;
    fs::write(&path, content)?;
    Ok(())
}