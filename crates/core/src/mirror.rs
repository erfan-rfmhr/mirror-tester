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

/// Supported registries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Registry {
    PyPi,
    Npm,
    Gradle,
}

impl Registry {
    /// Every supported registry, in display order.
    pub const ALL: [Registry; 3] = [Registry::PyPi, Registry::Npm, Registry::Gradle];

    /// Parses a registry name from a string (case-insensitive).
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "pypi" | "pip" => Some(Registry::PyPi),
            "npm" => Some(Registry::Npm),
            "gradle" | "maven" => Some(Registry::Gradle),
            _ => None,
        }
    }

    /// Returns the next registry in [`Registry::ALL`], wrapping around.
    pub fn next(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|entry| *entry == self)
            .unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    /// Returns the relative file path for this registry's mirror list
    /// within the configured data directory.
    pub fn data_file_name(&self) -> &'static str {
        match self {
            Registry::PyPi => "pypi.json",
            Registry::Npm => "npm.json",
            Registry::Gradle => "gradle.json",
        }
    }

    /// Returns the on-disk file path for this registry's mirror list,
    /// resolved from the configured data directory.
    pub fn data_file(&self) -> PathBuf {
        data_dir().join(self.data_file_name())
    }

    /// Returns the registry's identifier, used for display and as its key
    /// in [`crate::app::App::data`].
    pub fn name(&self) -> &'static str {
        match self {
            Registry::PyPi => "pypi",
            Registry::Npm => "npm",
            Registry::Gradle => "gradle",
        }
    }

    /// Human-readable registry name, used for display (e.g. the TUI menu).
    pub fn label(&self) -> &'static str {
        match self {
            Registry::PyPi => "PyPI",
            Registry::Npm => "npm",
            Registry::Gradle => "Gradle",
        }
    }
}

/// A sample package to download during benchmarking, together with the
/// list of mirror URLs to test it against. This is the on-disk shape of
/// `pypi.json` / `npm.json` / `gradle.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorConfig {
    /// Name of the package that will actually be downloaded from each
    /// mirror when benchmarking (e.g. `"requests"` for PyPI, `"lodash"`
    /// for npm, `"com.google.guava:guava"` for Gradle).
    pub package: String,
    /// Mirror base URLs to benchmark.
    pub mirrors: Vec<String>,
}

/// Benchmark outcome for a single mirror, stored inline in [`RegistryData`].
#[derive(Debug, Clone)]
pub struct MirrorStats {
    pub average_latency_ms: u128,
    pub success_rate: f32,
    pub timed_out: bool,
}

/// A mirror URL paired with its latest benchmark outcome. `stats` is `None`
/// until the mirror has been benchmarked at least once in this session.
#[derive(Debug, Clone)]
pub struct MirrorEntry {
    pub url: String,
    pub stats: Option<MirrorStats>,
}

/// Package + ordered mirror list (with results) for one registry. This is
/// the single in-memory source of truth used by [`crate::app::App`].
#[derive(Debug, Clone)]
pub struct RegistryData {
    pub package: String,
    pub mirrors: Vec<MirrorEntry>,
}

impl RegistryData {
    /// Finds the entry for a given mirror URL.
    pub fn find(&self, url: &str) -> Option<&MirrorEntry> {
        self.mirrors.iter().find(|entry| entry.url == url)
    }

    /// Finds the entry for a given mirror URL, mutably.
    pub fn find_mut(&mut self, url: &str) -> Option<&mut MirrorEntry> {
        self.mirrors.iter_mut().find(|entry| entry.url == url)
    }

    /// Returns the mirror URLs in their current order.
    pub fn urls(&self) -> Vec<String> {
        self.mirrors.iter().map(|entry| entry.url.clone()).collect()
    }

    /// Sorts mirrors after a benchmark run: successful results fastest
    /// first, then not-yet-benchmarked mirrors, then confirmed timeouts
    /// last. Safe to call after both full and single-mirror benchmarks.
    pub fn sort_by_results(&mut self) {
        fn rank(entry: &MirrorEntry) -> u8 {
            match &entry.stats {
                Some(stats) if !stats.timed_out => 0,
                None => 1,
                Some(_) => 2,
            }
        }

        self.mirrors.sort_by(|a, b| {
            rank(a)
                .cmp(&rank(b))
                .then_with(|| match (&a.stats, &b.stats) {
                    (Some(x), Some(y)) => x.average_latency_ms.cmp(&y.average_latency_ms),
                    _ => std::cmp::Ordering::Equal,
                })
        });
    }
}

/// Returns the directory containing the registry configuration files.
///
/// Resolution order:
/// 1. `AYN_DATA_DIR` environment variable (if set).
/// 2. `./data`.
fn data_dir() -> PathBuf {
    std::env::var("AYN_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("./data"))
}

#[cfg(test)]
mod tests {
    use super::data_dir;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct DataDirEnv(Option<std::ffi::OsString>);

    impl DataDirEnv {
        fn set(value: impl Into<std::ffi::OsString>) -> Self {
            let previous = std::env::var_os("AYN_DATA_DIR");
            let value = value.into();
            unsafe { std::env::set_var("AYN_DATA_DIR", value) };
            Self(previous)
        }

        fn remove() -> Self {
            let previous = std::env::var_os("AYN_DATA_DIR");
            unsafe { std::env::remove_var("AYN_DATA_DIR") };
            Self(previous)
        }
    }

    impl Drop for DataDirEnv {
        fn drop(&mut self) {
            match self.0.take() {
                Some(value) => unsafe { std::env::set_var("AYN_DATA_DIR", value) },
                None => unsafe { std::env::remove_var("AYN_DATA_DIR") },
            }
        }
    }

    #[test]
    fn data_dir_defaults_to_system_registry_directory() {
        let _lock = env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _env = DataDirEnv::remove();

        assert_eq!(data_dir(), PathBuf::from("./data"));
    }

    #[test]
    fn data_dir_uses_ayn_data_dir_when_set() {
        let _lock = env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _env = DataDirEnv::set("/custom/registries");

        assert_eq!(data_dir(), PathBuf::from("/custom/registries"));
    }
}

/// Loads the mirror configuration (sample package + mirror URLs) for the
/// given registry.
pub fn load_mirrors(registry: Registry) -> Result<MirrorConfig, MirrorError> {
    let path = registry.data_file();
    load_mirrors_from(&path)
}

/// Loads a mirror configuration from an arbitrary JSON file path.
pub fn load_mirrors_from(path: &Path) -> Result<MirrorConfig, MirrorError> {
    let content = fs::read_to_string(path)?;
    let config: MirrorConfig = serde_json::from_str(&content)?;
    Ok(config)
}

/// Loads a registry's mirror list from disk and wraps it as [`RegistryData`],
/// with every mirror's stats initialized to `None`.
pub fn load_registry_data(registry: Registry) -> Result<RegistryData, MirrorError> {
    let config = load_mirrors(registry)?;
    Ok(RegistryData {
        package: config.package,
        mirrors: config
            .mirrors
            .into_iter()
            .map(|url| MirrorEntry { url, stats: None })
            .collect(),
    })
}

/// Appends a mirror URL to the given registry's mirror list and persists
/// the updated configuration back to its data file.
pub fn add_mirror(registry: Registry, mirror: &str) -> Result<(), MirrorError> {
    let path = registry.data_file();
    let mut config = load_mirrors_from(&path)?;
    config.mirrors.push(mirror.to_string());
    let content = serde_json::to_string_pretty(&config)?;
    fs::write(&path, content)?;
    Ok(())
}

/// Removes a mirror URL from the given registry's mirror list.
pub fn remove_mirror(registry: Registry, mirror: &str) -> Result<(), MirrorError> {
    let path = registry.data_file();
    let mut config = load_mirrors_from(&path)?;
    config.mirrors.retain(|candidate| candidate != mirror);
    let content = serde_json::to_string_pretty(&config)?;
    fs::write(&path, content)?;
    Ok(())
}

/// Rewrites the on-disk mirror order for a registry, preserving the package
/// field. Used to persist the sorted order after benchmarking.
pub fn rewrite_mirrors(registry: Registry, urls: Vec<String>) -> Result<(), MirrorError> {
    let path = registry.data_file();
    let mut config = load_mirrors_from(&path)?;
    config.mirrors = urls;
    let content = serde_json::to_string_pretty(&config)?;
    fs::write(&path, content)?;
    Ok(())
}
