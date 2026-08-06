//! Application state for the TUI.

use crate::benchmark::{benchmark_all, BenchmarkResult};
use crate::mirror::{add_mirror, load_mirrors, PackageManager};

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

/// The interaction mode of the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Navigating menus and running benchmarks.
    Normal,
    /// Typing a new mirror URL.
    Input,
}

/// The overall state of the TUI application.
pub struct App {
    pub selection: Selection,
    pub results: Vec<BenchmarkResult>,
    pub status: String,
    pub running: bool,
    pub should_quit: bool,
    pub mode: Mode,
    pub input: String,
    pub cursor: usize,
}

impl App {
    /// Creates a fresh application state with no results yet.
    pub fn new() -> Self {
        App {
            selection: Selection::PyPi,
            results: Vec::new(),
            status: "Press [Enter] to run a benchmark, [up/down] to switch package manager."
                .to_string(),
            running: false,
            should_quit: false,
            mode: Mode::Normal,
            input: String::new(),
            cursor: 0,
        }
    }

    /// Enters input mode so the user can type a new mirror URL.
    pub fn enter_input(&mut self) {
        self.mode = Mode::Input;
        self.input.clear();
        self.status = "Type a mirror URL, [Enter] to save, [Esc] to cancel.".to_string();
    }

    /// Leaves input mode without saving anything.
    pub fn cancel_input(&mut self) {
        self.mode = Mode::Normal;
        self.cursor = 0;
        self.input.clear();
        self.status = "Press [Enter] to run a benchmark, [up/down] to switch package manager, [a] to add a mirror.".to_string();
    }

    /// Appends a character to the input buffer, ignoring control characters.
    pub fn input_char(&mut self, c: char) {
        if !c.is_control() {
            self.input.insert(self.cursor, c);
            self.cursor += 1;
        }
    }

    /// Removes the last character from the input buffer.
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.input.remove(self.cursor);
        }
    }

    /// Validates the typed URL and saves it to the selected package
    /// manager's mirror list. On success, returns to normal mode.
    pub fn submit_input(&mut self) -> Result<(), String> {
        let url = self.input.trim().to_string();
        if url.is_empty() {
            return Err("Mirror URL cannot be empty.".to_string());
        }
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err("Mirror URL must start with http:// or https://.".to_string());
        }

        let pm = self.selection.to_package_manager();
        add_mirror(pm, &url).map_err(|e| format!("Failed to save mirror: {e}"))?;

        self.status = format!("Added mirror {url} to {}.", pm.name());
        self.mode = Mode::Normal;
        Ok(())
    }

    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_right(&mut self) {
        if self.cursor < self.input.len() {
            self.cursor += 1;
        }
    }

    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor = self.input.len();
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
