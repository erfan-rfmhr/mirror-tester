//! JSON report generation.

use crate::benchmark::BenchmarkResult;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;

/// A single mirror entry within a report.
#[derive(Debug, Serialize, Deserialize)]
pub struct ReportEntry {
    pub mirror: String,
    pub latency: u128,
    pub success: f32,
}

/// A full benchmark report for a package manager.
#[derive(Debug, Serialize, Deserialize)]
pub struct Report {
    pub package_manager: String,
    pub generated_at: String,
    pub results: Vec<ReportEntry>,
    pub best: Option<String>,
}

impl Report {
    /// Builds a report from a set of benchmark results.
    pub fn new(package_manager: &str, results: &[BenchmarkResult]) -> Self {
        let entries = results
            .iter()
            .map(|r| ReportEntry {
                mirror: r.name.clone(),
                latency: r.average_latency_ms,
                success: r.success_rate,
            })
            .collect();

        let best = results
            .iter()
            .find(|r| !r.timed_out)
            .map(|r| r.name.clone());

        Report {
            package_manager: package_manager.to_string(),
            generated_at: Local::now().to_rfc3339(),
            results: entries,
            best,
        }
    }

    /// Saves the report as JSON to `reports/YYYY-MM-DD_HH-MM.json`,
    /// creating the `reports/` directory if needed. Returns the path
    /// the report was written to.
    pub fn save(&self) -> io::Result<PathBuf> {
        fs::create_dir_all("reports")?;

        let filename = format!("{}.json", Local::now().format("%Y-%m-%d_%H-%M"));
        let path = PathBuf::from("reports").join(filename);

        let json = serde_json::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        fs::write(&path, json)?;

        Ok(path)
    }
}
