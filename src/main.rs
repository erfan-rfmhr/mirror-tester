//! Mirror Benchmark CLI entry point.

mod app;
mod benchmark;
mod mirror;
mod report;
mod scheduler;
mod ui;

use app::{App, Mode};
use benchmark::benchmark_all;
use clap::{Parser, Subcommand};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind}, execute, terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use mirror::{load_mirrors, PackageManager};
use ratatui::{backend::CrosstermBackend, Terminal};
use report::Report;
use std::io;
use std::time::Duration;

/// Mirror Benchmark: benchmark package registry mirrors and find the fastest one.
#[derive(Parser)]
#[command(name = "mirror-benchmark", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a one-off benchmark for a package manager ("pypi" or "npm") and print results.
    Run {
        /// Which package manager to benchmark: "pypi" or "npm".
        package_manager: String,
    },
    /// Launch the interactive terminal UI.
    Tui,
    /// Run benchmarks for pypi and npm and write a JSON report to reports/.
    Report,
    /// Continuously benchmark and save reports every hour.
    Schedule,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Run { package_manager } => run_once(&package_manager).await,
        Commands::Tui => run_tui().await,
        Commands::Report => run_report().await,
        Commands::Schedule => {
            scheduler::run().await;
            Ok(())
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

/// Runs a single benchmark for the given package manager name and prints
/// the results to stdout.
async fn run_once(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let pm = PackageManager::from_str(name)
        .ok_or_else(|| format!("Unknown package manager '{name}'. Use 'pypi' or 'npm'."))?;

    let config = load_mirrors(pm)?;
    println!(
        "Benchmarking {} mirrors for {} (package: {})...",
        config.mirrors.len(),
        pm.name(),
        config.package
    );

    let results = benchmark_all(pm, &config.package, &config.mirrors).await;
    print_results(&results);

    Ok(())
}

/// Prints benchmark results as a simple table to stdout.
fn print_results(results: &[benchmark::BenchmarkResult]) {
    println!("{:<40} {:>10} {:>10}", "Mirror", "Avg(ms)", "Success");
    for r in results {
        let latency = if r.timed_out {
            "timeout".to_string()
        } else {
            r.average_latency_ms.to_string()
        };
        let success = format!("{:.0}%", r.success_rate);
        println!("{:<40} {:>10} {:>10}", r.name, latency, success);
    }

    if let Some(best) = results.iter().find(|r| !r.timed_out) {
        println!("\nFastest mirror: {}", best.name);
    } else {
        println!("\nNo mirrors were reachable.");
    }
}

/// Runs benchmarks for both pypi and npm and saves a JSON report for each.
async fn run_report() -> Result<(), Box<dyn std::error::Error>> {
    for pm in [PackageManager::PyPi, PackageManager::Npm] {
        let config = load_mirrors(pm)?;
        println!("Benchmarking {}...", pm.name());

        let results = benchmark_all(pm, &config.package, &config.mirrors).await;
        let report = Report::new(pm.name(), &results);
        let path = report.save()?;

        println!("Report saved to {}", path.display());
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
                            app.submit_input().ok();
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
                        KeyCode::Up | KeyCode::Down => app.selection = app.selection.toggle(),
                        KeyCode::Enter => {
                            app.start_benchmark();
                            terminal.draw(|f| ui::draw(f, app))?;
                            app.run_benchmark().await;
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
