//! Mirror Benchmark TUI entry point.

mod ui;

use std::io;
use std::time::Duration;

use ayeneh_cli::{run_command, Cli};
use ayeneh_core::app::App as CoreApp;
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::ui::Mode;

const DEFAULT_STATUS: &str = "";

// Which section is currently active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveSection {
    Registries,
    Results,
}

impl ActiveSection {
    pub fn toggle(self) -> Self {
        match self {
            ActiveSection::Registries => ActiveSection::Results,
            ActiveSection::Results => ActiveSection::Registries,
        }
    }
}

/// The TUI's application state: wraps the core [`CoreApp`] and adds UI-only
/// fields (input mode, current text input, cursor position, status text).
pub struct App {
    pub core: CoreApp,
    pub status: String,
    pub should_quit: bool,
    pub mode: Mode,
    pub input: String,
    pub cursor: usize,
    pub error: Option<String>,
    pub active_section: ActiveSection,
    pub selected_mirror: usize,
}

impl App {
    fn new() -> Self {
        App {
            core: CoreApp::new(),
            status: DEFAULT_STATUS.to_string(),
            should_quit: false,
            mode: Mode::Normal,
            input: String::new(),
            cursor: 0,
            error: None,
            active_section: ActiveSection::Registries,
            selected_mirror: 0,
        }
    }

    fn enter_input(&mut self) {
        self.mode = Mode::Input;
        self.input.clear();
        self.cursor = 0;
        self.status = "Type a mirror URL, [Enter] to save, [Esc] to cancel.".to_string();
        self.error = None;
    }

    fn cancel_input(&mut self) {
        self.mode = Mode::Normal;
        self.cursor = 0;
        self.input.clear();
        self.status = DEFAULT_STATUS.to_string();
        self.error = None;
    }

    fn input_char(&mut self, c: char) {
        if !c.is_control() {
            self.input.insert(self.cursor, c);
            self.cursor += 1;
        }
    }

    fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.input.remove(self.cursor);
        }
    }

    fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    fn move_right(&mut self) {
        if self.cursor < self.input.len() {
            self.cursor += 1;
        }
    }

    fn move_home(&mut self) {
        self.cursor = 0;
    }

    fn move_end(&mut self) {
        self.cursor = self.input.len();
    }

    fn submit_input(&mut self) -> Result<(), String> {
        let url = self.input.trim().to_string();
        if url.is_empty() {
            return Err("Mirror URL cannot be empty.".to_string());
        }
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err("Mirror URL must start with http:// or https://.".to_string());
        }

        self.core.add_mirror(&url)?;
        self.status = format!("Added mirror {url} to {}.", self.core.selection.name());
        self.mode = Mode::Normal;
        self.error = None;
        Ok(())
    }

    fn mirror_count(&self) -> usize {
        self.core
            .data
            .get(self.core.selection.name())
            .map(|registry_data| registry_data.mirrors.len())
            .unwrap_or(0)
    }

    fn selected_mirror_url(&self) -> Option<String> {
        self.core
            .data
            .get(self.core.selection.name())?
            .mirrors
            .get(self.selected_mirror)
            .map(|entry| entry.url.clone())
    }

    fn move_mirror_selection(&mut self, down: bool) {
        let count = self.mirror_count();
        if count == 0 {
            self.selected_mirror = 0;
        } else if down {
            self.selected_mirror = (self.selected_mirror + 1).min(count - 1);
        } else {
            self.selected_mirror = self.selected_mirror.saturating_sub(1);
        }
    }

    fn start_selected_benchmark(&mut self) -> Result<(), String> {
        let Some(mirror) = self.selected_mirror_url() else {
            return Err("No mirror selected.".to_string());
        };
        self.core.start_single_benchmark(&mirror)?;
        self.error = None;
        self.status = format!("Benchmarking {mirror}...");
        Ok(())
    }

    fn remove_selected_mirror(&mut self) -> Result<(), String> {
        let Some(mirror) = self.selected_mirror_url() else {
            return Err("No mirror selected.".to_string());
        };
        self.core.remove_mirror(&mirror)?;
        self.selected_mirror = self
            .selected_mirror
            .min(self.mirror_count().saturating_sub(1));
        self.error = None;
        self.status = format!("Removed mirror {mirror}.");
        Ok(())
    }

    /// Marks a benchmark run as starting, loading mirror config.
    fn start_benchmark(&mut self) {
        self.error = None;
        match self.core.start_benchmark() {
            Ok(()) => {
                self.status = "Benchmarking started...".to_string();
            }
            Err(e) => {
                self.status = e;
            }
        }
    }

    /// Runs one benchmark step and updates the status text accordingly.
    async fn benchmark_step(&mut self) {
        if self.core.running {
            self.status = "Testing ...".to_string();
        }
        let done = self.core.benchmark_step().await;
        if done {
            self.status = "Benchmarking is completed.".to_string();
        }
    }

    fn submit_results(&mut self) {
        if let Err(e) = self.core.submit_results() {
            self.error = Some(e);
        } else {
            self.status = "Results submitted successfully.".to_string();
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    if cli.command.is_some() {
        if let Err(e) = run_command(cli).await {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }

    if let Err(e) = run_tui().await {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
    Ok(())
}

/// Launches the interactive terminal UI and runs its event loop.
async fn run_tui() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let res = run_app_loop(&mut terminal, &mut app).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    res
}

/// The main TUI event loop: draws the UI and handles keyboard input.
async fn run_app_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        if app.should_quit {
            break;
        }

        if app.core.running {
            app.benchmark_step().await;
            continue;
        }

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if app.mode == Mode::Input {
                    match key.code {
                        KeyCode::Char(c) => app.input_char(c),
                        KeyCode::Esc => app.cancel_input(),
                        KeyCode::Enter => {
                            if let Err(e) = app.submit_input() {
                                app.error = Some(e);
                            }
                        }
                        KeyCode::Backspace => app.backspace(),
                        KeyCode::Left => app.move_left(),
                        KeyCode::Right => app.move_right(),
                        KeyCode::Home => app.move_home(),
                        KeyCode::End => app.move_end(),
                        _ => {
                            app.error = Some("Unknown key".to_string());
                        }
                    }
                } else {
                    match key.code {
                        KeyCode::Char('q') => app.should_quit = true,
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            app.should_quit = true
                        }
                        KeyCode::Up | KeyCode::Down => match app.active_section {
                            ActiveSection::Registries => {
                                app.core.selection = app.core.selection.next();
                                app.selected_mirror = 0;
                            }
                            ActiveSection::Results => {
                                app.move_mirror_selection(key.code == KeyCode::Down);
                            }
                        },
                        KeyCode::Enter => match app.active_section {
                            ActiveSection::Registries => app.start_benchmark(),
                            ActiveSection::Results => {
                                if let Err(e) = app.start_selected_benchmark() {
                                    app.error = Some(e);
                                }
                            }
                        },
                        KeyCode::Char('d') if app.active_section == ActiveSection::Results => {
                            if let Err(e) = app.remove_selected_mirror() {
                                app.error = Some(e);
                            }
                        }
                        KeyCode::Char('a') => app.enter_input(),
                        KeyCode::BackTab => {
                            app.active_section = app.active_section.toggle();
                        }
                        KeyCode::Char('s') => app.submit_results(),
                        _ => {
                            app.error = Some("Unknown key".to_string());
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
