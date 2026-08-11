//! Application state and operations.

use crate::benchmark::{benchmark_mirror, BenchmarkResult};
use crate::mirror::{add_mirror, load_mirrors, MirrorConfig, PackageManager};

/// Which package manager is currently selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    PyPi,
    Npm,
}

impl Selection {
    /// Toggles between the two available selections.
    pub fn toggle(self) -> Self {
        match self {
            Selection::PyPi => Selection::Npm,
            Selection::Npm => Selection::PyPi,
        }
    }

    /// Converts this selection into the corresponding [`PackageManager`].
    pub fn to_package_manager(self) -> PackageManager {
        match self {
            Selection::PyPi => PackageManager::PyPi,
            Selection::Npm => PackageManager::Npm,
        }
    }
}

/// Core application state and operations.
pub struct App {
    pub selection: Selection,
    pub results: Vec<BenchmarkResult>,
    pub running: bool,
    pub config: Option<MirrorConfig>,
    pub benchmark_index: usize,
}

impl App {
    /// Creates a fresh application state with no results yet.
    pub fn new() -> Self {
        App {
            selection: Selection::PyPi,
            results: Vec::new(),
            running: false,
            config: None,
            benchmark_index: 0,
        }
    }

    /// Returns the fastest reachable mirror from the last benchmark run, if any.
    pub fn fastest(&self) -> Option<&str> {
        self.results
            .iter()
            .find(|r| !r.timed_out)
            .map(|r| r.name.as_str())
    }

    /// Marks a benchmark run as starting, loads mirror config, sets status.
    /// Call this and render a frame *before* calling [`App::benchmark_step`].
    pub fn start_benchmark(&mut self) -> Result<(), String> {
        self.running = true;
        self.results.clear();
        self.benchmark_index = 0;
        let pm = self.selection.to_package_manager();
        match load_mirrors(pm) {
            Ok(config) => {
                self.config = Some(config);
                Ok(())
            }
            Err(e) => {
                self.running = false;
                self.config = None;
                Err(format!("Failed to load mirrors: {e}"))
            }
        }
    }

    /// Benchmarks the next pending mirror. Returns `true` when all mirrors
    /// have been benchmarked (or no config loaded). Call repeatedly with a
    /// redraw between each call for live progress.
    pub async fn benchmark_step(&mut self) -> bool {
        let config = match &self.config {
            Some(c) => c.clone(),
            None => return true,
        };

        if self.benchmark_index >= config.mirrors.len() {
            self.running = false;
            self.config = None;
            self.benchmark_index = 0;
            self.results.sort_by(|a, b| match (a.timed_out, b.timed_out) {
                (true, true) => std::cmp::Ordering::Equal,
                (true, false) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                (false, false) => a.average_latency_ms.cmp(&b.average_latency_ms),
            });
            return true;
        }

        let pm = self.selection.to_package_manager();
        let mirror = &config.mirrors[self.benchmark_index];

        let result = benchmark_mirror(pm, &config.package, mirror).await;
        self.results.push(result);
        self.benchmark_index += 1;

        if self.benchmark_index >= config.mirrors.len() {
            self.running = false;
            self.config = None;
            self.benchmark_index = 0;
            self.results.sort_by(|a, b| match (a.timed_out, b.timed_out) {
                (true, true) => std::cmp::Ordering::Equal,
                (true, false) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                (false, false) => a.average_latency_ms.cmp(&b.average_latency_ms),
            });
            return true;
        }

        false
    }

    /// Adds a mirror URL to the selected package manager's mirror list.
    pub fn add_mirror(&mut self, url: &str) -> Result<(), String> {
        let pm = self.selection.to_package_manager();
        add_mirror(pm, url).map_err(|e| format!("Failed to save mirror: {e}"))?;
        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}