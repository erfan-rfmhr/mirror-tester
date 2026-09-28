//! Application state and operations.

use std::collections::HashMap;

use crate::benchmark::benchmark_mirror;
use crate::mirror::{
    add_mirror, load_registry_data, remove_mirror, rewrite_mirrors, MirrorEntry, MirrorStats,
    Registry, RegistryData,
};

/// Core application state and operations.
pub struct App {
    pub selection: Registry,
    pub data: HashMap<String, RegistryData>,
    pub running: bool,
    pub benchmark_index: usize,
    /// Snapshot of mirror URLs being benchmarked in the current run, taken
    /// at `start_benchmark`/`start_single_benchmark` time so that mirrors
    /// added or removed mid-run don't disturb the in-flight iteration.
    benchmark_queue: Vec<String>,
}

impl App {
    /// Creates a fresh application state, loading both registries from disk.
    pub fn new() -> Self {
        let mut data = HashMap::new();
        for registry in Registry::ALL {
            if let Ok(registry_data) = load_registry_data(registry) {
                data.insert(registry.name().to_string(), registry_data);
            }
        }

        App {
            selection: Registry::PyPi,
            data,
            running: false,
            benchmark_index: 0,
            benchmark_queue: Vec::new(),
        }
    }

    /// Returns the fastest reachable mirror for the selected registry from
    /// the last benchmark run, if any.
    pub fn fastest(&self) -> Option<&str> {
        self.data
            .get(self.selection.name())?
            .mirrors
            .iter()
            .find(|entry| matches!(&entry.stats, Some(stats) if !stats.timed_out))
            .map(|entry| entry.url.as_str())
    }

    /// Returns the mirror currently being benchmarked, if a run is in
    /// progress. Used by frontends to highlight the in-flight mirror.
    pub fn pending_mirror(&self) -> Option<&str> {
        if !self.running {
            return None;
        }
        self.benchmark_queue
            .get(self.benchmark_index)
            .map(String::as_str)
    }

    /// Marks a benchmark run as starting: reloads the selected registry's
    /// mirror list from disk (resetting all stats), and queues every
    /// mirror for benchmarking. Call this and render a frame *before*
    /// calling [`App::benchmark_step`].
    pub fn start_benchmark(&mut self) -> Result<(), String> {
        let registry = self.selection;
        match load_registry_data(registry) {
            Ok(registry_data) => {
                self.benchmark_queue = registry_data.urls();
                self.data.insert(registry.name().to_string(), registry_data);
                self.benchmark_index = 0;
                self.running = true;
                Ok(())
            }
            Err(e) => {
                self.running = false;
                Err(format!("Failed to load mirrors: {e}"))
            }
        }
    }

    /// Starts a benchmark for one mirror without touching the rest of the
    /// registry's data.
    pub fn start_single_benchmark(&mut self, mirror: &str) -> Result<(), String> {
        let exists = self
            .data
            .get(self.selection.name())
            .is_some_and(|registry_data| registry_data.find(mirror).is_some());
        if !exists {
            return Err("Mirror is no longer available.".to_string());
        }

        self.benchmark_queue = vec![mirror.to_string()];
        self.benchmark_index = 0;
        self.running = true;
        Ok(())
    }

    /// Benchmarks the next pending mirror. Returns `true` when every queued
    /// mirror has been benchmarked. Call repeatedly with a redraw between
    /// each call for live progress.
    pub async fn benchmark_step(&mut self) -> bool {
        if self.benchmark_index >= self.benchmark_queue.len() {
            self.finish_benchmark();
            return true;
        }

        let registry = self.selection;
        let key = registry.name();
        let mirror = self.benchmark_queue[self.benchmark_index].clone();
        let package = self
            .data
            .get(key)
            .map(|registry_data| registry_data.package.clone())
            .unwrap_or_default();

        let result = benchmark_mirror(registry, &package, &mirror).await;
        if let Some(entry) = self
            .data
            .get_mut(key)
            .and_then(|registry_data| registry_data.find_mut(&mirror))
        {
            entry.stats = Some(MirrorStats {
                average_latency_ms: result.average_latency_ms,
                success_rate: result.success_rate,
                timed_out: result.timed_out,
            });
        }
        self.benchmark_index += 1;

        if self.benchmark_index >= self.benchmark_queue.len() {
            self.finish_benchmark();
            return true;
        }

        false
    }

    /// Ends the current benchmark run and sorts the selected registry's
    /// mirrors by result: fastest first, then not-yet-benchmarked, then
    /// timed out. Applies identically after full and single-mirror runs.
    fn finish_benchmark(&mut self) {
        self.running = false;
        self.benchmark_index = 0;
        self.benchmark_queue.clear();
        if let Some(registry_data) = self.data.get_mut(self.selection.name()) {
            registry_data.sort_by_results();
        }
    }

    /// Adds a mirror URL to the selected registry's mirror list.
    pub fn add_mirror(&mut self, url: &str) -> Result<(), String> {
        let registry = self.selection;
        add_mirror(registry, url).map_err(|e| format!("Failed to save mirror: {e}"))?;
        self.data
            .entry(registry.name().to_string())
            .or_insert_with(|| RegistryData {
                package: String::new(),
                mirrors: Vec::new(),
            })
            .mirrors
            .push(MirrorEntry {
                url: url.to_string(),
                stats: None,
            });
        Ok(())
    }

    /// Removes a mirror URL and its stale benchmark result.
    pub fn remove_mirror(&mut self, url: &str) -> Result<(), String> {
        let registry = self.selection;
        remove_mirror(registry, url).map_err(|e| format!("Failed to remove mirror: {e}"))?;
        if let Some(registry_data) = self.data.get_mut(registry.name()) {
            registry_data.mirrors.retain(|entry| entry.url != url);
        }
        Ok(())
    }

    /// Persists the selected registry's current mirror order to disk.
    /// Guards against submitting when nothing has been benchmarked yet;
    /// otherwise writes the mirror keys in their current order, which
    /// `benchmark_step` has already sorted (partially or fully).
    pub fn submit_results(&mut self) -> Result<(), String> {
        let registry = self.selection;
        let registry_data = self
            .data
            .get(registry.name())
            .filter(|registry_data| {
                registry_data
                    .mirrors
                    .iter()
                    .any(|entry| entry.stats.is_some())
            })
            .ok_or_else(|| "No benchmark results to submit.".to_string())?;

        rewrite_mirrors(registry, registry_data.urls())
            .map_err(|e| format!("Failed to submit results: {e}"))?;
        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
