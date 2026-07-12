//! Application state for the TUI.

use crate::benchmark::{benchmark_all, BenchmarkResult};
use crate::mirror::{load_mirrors, PackageManager};

/// Which package manager is currently selected in the menu.
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

    /// Converts this UI selection into the corresponding [`PackageManager`].
    pub fn to_package_manager(self) -> PackageManager {
        match self {
            Selection::PyPi => PackageManager::PyPi,
            Selection::Npm => PackageManager::Npm,
        }
    }
}

/// The overall state of the TUI application.
pub struct App {
    pub selection: Selection,
    pub results: Vec<BenchmarkResult>,
    pub status: String,
    pub running: bool,
    pub should_quit: bool,
}

impl App {
    /// Creates a fresh application state with no results yet.
    pub fn new() -> Self {
        App {
            selection: Selection::PyPi,
            results: Vec::new(),
            status: "Press [Enter] to run a benchmark, [up/down] to switch package manager.".to_string(),
            running: false,
            should_quit: false,
        }
    }

    /// Returns the fastest reachable mirror from the last benchmark run, if any.
    pub fn fastest(&self) -> Option<&str> {
        self.results
            .iter()
            .find(|r| !r.timed_out)
            .map(|r| r.name.as_str())
    }

    /// Marks a benchmark run as starting and sets the status message.
    ///
    /// Call this and render a frame *before* calling [`App::run_benchmark`],
    /// otherwise the "Benchmarking..." status will never be drawn since
    /// `run_benchmark` immediately awaits the (potentially slow) benchmark
    /// work without yielding back to the render loop.
    pub fn start_benchmark(&mut self) {
        self.running = true;
        let pm = self.selection.to_package_manager();
        self.status = format!("Benchmarking {}...", pm.name());
    }

    /// Runs a benchmark for the currently selected package manager and
    /// stores the results in the app state.
    pub async fn run_benchmark(&mut self) {
        let pm = self.selection.to_package_manager();

        match load_mirrors(pm) {
            Ok(config) => {
                self.results = benchmark_all(pm, &config.package, &config.mirrors).await;
                self.status = "Benchmark complete.".to_string();
            }
            Err(e) => {
                self.status = format!("Failed to load mirrors: {e}");
            }
        }

        self.running = false;
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
