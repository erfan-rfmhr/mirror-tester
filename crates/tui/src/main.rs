//! Mirror Benchmark TUI entry point.

mod ui;

use std::io;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use mirror_core::app::App as CoreApp;
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::ui::Mode;

/// The TUI's application state: wraps the core [`CoreApp`] and adds UI-only
/// fields (input mode, current text input, cursor position, status text).
pub struct App {
    pub core: CoreApp,
    pub status: String,
    pub should_quit: bool,
    pub mode: Mode,
    pub input: String,
    pub cursor: usize,
}

impl App {
    fn new() -> Self {
        App {
            core: CoreApp::new(),
            status: "Press [Enter] to run a benchmark, [up/down] to switch package manager."
                .to_string(),
            should_quit: false,
            mode: Mode::Normal,
            input: String::new(),
            cursor: 0,
        }
    }

    fn enter_input(&mut self) {
        self.mode = Mode::Input;
        self.input.clear();
        self.cursor = 0;
        self.status =
            "Type a mirror URL, [Enter] to save, [Esc] to cancel.".to_string();
    }

    fn cancel_input(&mut self) {
        self.mode = Mode::Normal;
        self.cursor = 0;
        self.input.clear();
        self.status = "Press [Enter] to run a benchmark, [up/down] to switch package manager, [a] to add a mirror.".to_string();
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
        self.status = format!("Added mirror {url} to {}.", self.core.selection.to_package_manager().name());
        self.mode = Mode::Normal;
        Ok(())
    }

    /// Marks a benchmark run as starting, loading mirror config.
    fn start_benchmark(&mut self) {
        match self.core.start_benchmark() {
            Ok(()) => {
                self.status = "Benchmarking...".to_string();
            }
            Err(e) => {
                self.status = e;
            }
        }
    }

    /// Runs one benchmark step and updates the status text accordingly.
    async fn benchmark_step(&mut self) {
        let pm = self.core.selection.to_package_manager();
        let pending_status = match &self.core.config {
            Some(config) if self.core.benchmark_index < config.mirrors.len() => {
                Some(format!(
                    "Testing {}...",
                    config.mirrors[self.core.benchmark_index]
                ))
            }
            _ => None,
        };
        if let Some(s) = pending_status {
            self.status = s;
        }
        let done = self.core.benchmark_step().await;
        if done {
            self.status = "Benchmark complete.".to_string();
            let _ = pm;
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
                                app.status = e;
                            }
                        }
                        KeyCode::Backspace => app.backspace(),
                        KeyCode::Left => app.move_left(),
                        KeyCode::Right => app.move_right(),
                        KeyCode::Home => app.move_home(),
                        KeyCode::End => app.move_end(),
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char('q') => app.should_quit = true,
                        KeyCode::Up | KeyCode::Down => {
                            app.core.selection = app.core.selection.toggle()
                        }
                        KeyCode::Enter => {
                            app.start_benchmark();
                        }
                        KeyCode::Char('a') => app.enter_input(),
                        _ => {}
                    }
                }
            }
        }
    }

    Ok(())
}