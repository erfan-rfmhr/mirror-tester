//! Mirror Benchmark CLI entry point.

use clap::{Parser, Subcommand};
use mirror_core::benchmark::{benchmark_all, BenchmarkResult};
use mirror_core::mirror::{load_mirrors, PackageManager};
use mirror_core::report::Report;
use mirror_core::{pip, scheduler};

/// Mirror Benchmark: benchmark package registry mirrors and find the fastest one.
#[derive(Parser)]
#[command(name = "mirror-cli", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a one-off benchmark for a package manager ("pypi" or "npm") and print results.
    Run {
        /// Which package manager to benchmark: "pypi" or "npm".
        package_manager: String,
    },
    /// Run benchmarks for pypi and npm and write a JSON report to reports/.
    Report,
    /// Continuously benchmark and save reports every hour.
    Schedule,
    /// Install Python packages via PyPI mirrors, falling back to the next mirror on failure.
    Pip {
        #[command(subcommand)]
        command: PipCommand,
    },
}

#[derive(Subcommand)]
enum PipCommand {
    /// Install a package or a -r requirements file.
    Install {
        /// pip install arguments: package names, `-r FILE`, options.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Some(Commands::Run { package_manager }) => run_once(&package_manager).await,
        Some(Commands::Report) => run_report().await,
        Some(Commands::Schedule) => {
            scheduler::run().await;
            Ok(())
        }
        Some(Commands::Pip { command: PipCommand::Install { args } }) => {
            pip::install(&args).await
        }
        None => {
            eprintln!("No command provided. Run with --help for usage.");
            std::process::exit(2);
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
fn print_results(results: &[BenchmarkResult]) {
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