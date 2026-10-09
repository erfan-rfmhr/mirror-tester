//! Mirror Benchmark CLI shared logic, exposed as a library so other crates
//! (e.g. the TUI) can reuse the same command handlers.

use ayeneh_core::benchmark::{benchmark_all, BenchmarkResult};
use ayeneh_core::mirror::{load_mirrors, Registry};
use ayeneh_core::report::Report;
use ayeneh_core::{gradle, maven, npm, pip, scheduler, uv};
use clap::{Parser, Subcommand};

/// Mirror Benchmark: benchmark package registry mirrors and find the fastest one.
#[derive(Parser)]
#[command(name = "ayeneh-cli", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run a one-off benchmark for a registry ("pypi", "npm", "gradle", or "maven") and print results.
    Run {
        /// Which registry to benchmark: "pypi", "npm", "gradle", or "maven".
        registry: String,
    },
    /// Run benchmarks for every registry and write a JSON report to reports/.
    Report,
    /// Continuously benchmark and save reports every hour.
    Schedule,
    /// Install Python packages via PyPI mirrors, falling back to the next mirror on failure.
    Pip {
        #[command(subcommand)]
        command: PipCommand,
    },
    /// Install npm packages via npm mirrors, falling back to the next mirror on failure.
    Npm {
        #[command(subcommand)]
        command: NpmCommand,
    },
    /// Install uv tools.
    UV {
        #[command(subcommand)]
        command: UVCommand,
    },
    /// Run Gradle dependency-resolving commands through configured Maven mirrors.
    #[command(
        after_help = "Common tasks: build, sync, assemble, check, test, dependencies. Other Gradle tasks are forwarded."
    )]
    Gradle {
        #[command(subcommand)]
        command: GradleCommand,
    },
    /// Run Maven dependency-resolving goals through configured Maven mirrors.
    #[command(
        alias = "mvn",
        after_help = "Common goals: compile, test, package, verify, install, sync. Other Maven goals are forwarded."
    )]
    Maven {
        #[command(subcommand)]
        command: MavenCommand,
    },
}

#[derive(Subcommand)]
pub enum PipCommand {
    /// Install a package or a -r requirements file.
    Install {
        /// pip install arguments: package names, `-r FILE`, options.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

#[derive(Subcommand)]
pub enum NpmCommand {
    /// Install one or more packages.
    Install {
        /// npm install arguments: package names, options.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

#[derive(Subcommand)]
pub enum UVCommand {
    /// Mirrors `uv add` command.
    Add {
        /// uv add arguments: package names.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

#[derive(Subcommand)]
pub enum GradleCommand {
    /// Download Maven coordinates (group:artifact:version) into the Gradle cache.
    Install {
        /// Maven coordinates, e.g. `com.google.guava:guava:34.0.0-jre`. The version may be omitted.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Forward any other Gradle task, such as build, sync, test, or dependencies.
    #[command(external_subcommand)]
    External(Vec<String>),
}

#[derive(Subcommand)]
pub enum MavenCommand {
    /// Forward a Maven goal or lifecycle phase, such as compile, test, package, verify, or install.
    #[command(external_subcommand)]
    External(Vec<String>),
}

fn is_maven_coordinate(value: &str) -> bool {
    !value.starts_with('-') && value.matches(':').count() >= 1 && !value.contains('/')
}

/// Runs a single benchmark for the given package manager name and prints
/// the results to stdout.
pub async fn run_once(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let registry = Registry::from_str(name).ok_or_else(|| {
        format!("Unknown registry '{name}'. Use 'pypi', 'npm', 'gradle', or 'maven'.")
    })?;

    let config = load_mirrors(registry)?;
    println!(
        "Benchmarking {} mirrors for {} (package: {})...",
        config.mirrors.len(),
        registry.name(),
        config.package
    );

    let results = benchmark_all(registry, &config.package, &config.mirrors).await;
    print_results(&results);

    Ok(())
}

/// Prints benchmark results as a simple table to stdout.
pub fn print_results(results: &[BenchmarkResult]) {
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

/// Runs benchmarks for every registry and saves a JSON report for each.
pub async fn run_report() -> Result<(), Box<dyn std::error::Error>> {
    for registry in Registry::ALL {
        let config = load_mirrors(registry)?;
        println!("Benchmarking {}...", registry.name());

        let results = benchmark_all(registry, &config.package, &config.mirrors).await;
        let report = Report::new(registry.name(), &results);
        let path = report.save()?;

        println!("Report saved to {}", path.display());
    }

    Ok(())
}

/// Executes a parsed [`Cli`] subcommand, printing results to stdout. Returns
/// `Ok(())` when a subcommand was handled and `Err(e)` on failure. Calling this
/// with `None` is a no-op, letting the caller decide what to do when no command
/// was provided.
pub async fn run_command(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        Some(Commands::Run { registry }) => run_once(&registry).await,
        Some(Commands::Report) => run_report().await,
        Some(Commands::Schedule) => {
            scheduler::run().await;
            Ok(())
        }
        Some(Commands::Pip {
            command: PipCommand::Install { args },
        }) => pip::install(&args).await,
        Some(Commands::Npm {
            command: NpmCommand::Install { args },
        }) => npm::install(&args).await,
        Some(Commands::UV {
            command: UVCommand::Add { args },
        }) => uv::add(&args).await,
        Some(Commands::Gradle {
            command: GradleCommand::Install { args },
        }) => {
            if args.is_empty() || !args.iter().all(|arg| is_maven_coordinate(arg)) {
                let mut command = vec!["install".to_string()];
                command.extend(args);
                gradle::run(&command).await
            } else {
                gradle::install(&args).await
            }
        }
        Some(Commands::Gradle {
            command: GradleCommand::External(args),
        }) => gradle::run(&args).await,
        Some(Commands::Maven {
            command: MavenCommand::External(args),
        }) => maven::run(&args).await,
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::{Cli, Commands, GradleCommand, MavenCommand};
    use clap::Parser;

    #[test]
    fn parses_gradle_project_commands() {
        let cli = Cli::try_parse_from(["ayeneh-cli", "gradle", "build", "--refresh-dependencies"])
            .expect("Gradle task should parse");

        match cli.command {
            Some(Commands::Gradle {
                command: GradleCommand::External(args),
            }) => assert_eq!(args, ["build", "--refresh-dependencies"]),
            _ => panic!("expected an external Gradle command"),
        }
    }

    #[test]
    fn parses_maven_goals() {
        let cli = Cli::try_parse_from(["ayeneh-cli", "mvn", "install", "-DskipTests"])
            .expect("Maven goal should parse");

        match cli.command {
            Some(Commands::Maven {
                command: MavenCommand::External(args),
            }) => assert_eq!(args, ["install", "-DskipTests"]),
            _ => panic!("expected an external Maven command"),
        }
    }
}
