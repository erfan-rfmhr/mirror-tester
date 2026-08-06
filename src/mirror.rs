//! Mirror list loading utilities.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

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

    /// Returns the on-disk file path for this package manager's mirror list.
    pub fn data_file(&self) -> &'static str {
        match self {
            PackageManager::PyPi => "data/pypi.json",
            PackageManager::Npm => "data/npm.json",
        }
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

/// Loads the mirror configuration (sample package + mirror URLs) for the
/// given package manager.
pub fn load_mirrors(pm: PackageManager) -> Result<MirrorConfig, MirrorError> {
    load_mirrors_from(Path::new(pm.data_file()))
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
    let mut config = load_mirrors(pm)?;
    config.mirrors.push(mirror.to_string());
    let content = serde_json::to_string_pretty(&config)?;
    fs::write(pm.data_file(), content)?;
    Ok(())
}
