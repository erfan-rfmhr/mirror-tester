//! A simple scheduler that repeatedly benchmarks all package managers and
//! saves a report, sleeping for an hour between runs.

use crate::benchmark::benchmark_all;
use crate::mirror::{load_mirrors, PackageManager};
use crate::report::Report;
use std::time::Duration;
use tokio::time::sleep;

const INTERVAL: Duration = Duration::from_secs(60 * 60);

/// Runs an infinite benchmark-and-report loop for every supported package
/// manager, sleeping for one hour between cycles.
///
/// This never returns under normal operation; it is intended to be the
/// entire lifetime of the `schedule` subcommand.
pub async fn run() {
    let package_managers = [PackageManager::PyPi, PackageManager::Npm];

    loop {
        for &pm in &package_managers {
            println!("Running scheduled benchmark for {}...", pm.name());

            match load_mirrors(pm) {
                Ok(config) => {
                    let results = benchmark_all(pm, &config.package, &config.mirrors).await;
                    let report = Report::new(pm.name(), &results);

                    match report.save() {
                        Ok(path) => println!("Report saved to {}", path.display()),
                        Err(e) => eprintln!("Failed to save report: {e}"),
                    }
                }
                Err(e) => eprintln!("Failed to load mirrors for {}: {e}", pm.name()),
            }
        }

        println!("Sleeping for 1 hour...");
        sleep(INTERVAL).await;
    }
}
