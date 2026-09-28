//! A simple scheduler that repeatedly benchmarks all registries and saves
//! a report, sleeping for a configurable interval between runs.

use crate::benchmark::benchmark_all;
use crate::config::Config;
use crate::mirror::{load_mirrors, Registry};
use crate::report::Report;
use std::time::Duration;
use tokio::time::sleep;

/// Runs an infinite benchmark-and-report loop for every supported
/// registry, sleeping for `AYN_SCHEDULE_INTERVAL_SECS` (default one
/// hour) between cycles.
///
/// This never returns under normal operation; it is intended to be the
/// entire lifetime of the `schedule` subcommand.
pub async fn run() {
    let cfg = Config::from_env();
    let interval = Duration::from_secs(cfg.schedule_interval_secs);
    let registries = Registry::ALL;

    loop {
        for &registry in &registries {
            println!("Running scheduled benchmark for {}...", registry.name());

            match load_mirrors(registry) {
                Ok(config) => {
                    let results = benchmark_all(registry, &config.package, &config.mirrors).await;
                    let report = Report::new(registry.name(), &results);

                    match report.save() {
                        Ok(path) => println!("Report saved to {}", path.display()),
                        Err(e) => eprintln!("Failed to save report: {e}"),
                    }
                }
                Err(e) => eprintln!("Failed to load mirrors for {}: {e}", registry.name()),
            }
        }

        println!("Sleeping for {} seconds...", cfg.schedule_interval_secs);
        sleep(interval).await;
    }
}
