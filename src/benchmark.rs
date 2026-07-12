//! Mirror benchmarking logic.
//!
//! Benchmarking works by actually downloading a real package from each
//! mirror (never installing it) and timing the whole round trip: looking up
//! where the package's file lives on that mirror, downloading it, and then
//! deleting the downloaded file. This is a much more realistic measure of
//! mirror speed than a plain HTTP ping, since it exercises the same
//! metadata + file-transfer path that `pip install` / `npm install` would
//! use.

use crate::mirror::PackageManager;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

const TIMEOUT_SECS: u64 = 15;
const ATTEMPTS: u32 = 3;

/// Any error that can occur while resolving or downloading a package file.
/// Benchmarking never propagates these; they simply mark an attempt as
/// failed.
type DownloadError = Box<dyn std::error::Error + Send + Sync>;

/// The result of benchmarking a single mirror.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub name: String,
    pub average_latency_ms: u128,
    pub success_rate: f32,
    pub timed_out: bool,
}

/// Benchmarks a single mirror by downloading `package` from it `ATTEMPTS`
/// times, averaging the latency of the successful downloads.
///
/// Always returns a [`BenchmarkResult`]; failures and timeouts are recorded
/// on the result rather than propagated as an error, so that a single bad
/// mirror never stops the overall benchmark run.
pub async fn benchmark_mirror(pm: PackageManager, package: &str, mirror: &str) -> BenchmarkResult {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .build()
    {
        Ok(c) => c,
        Err(_) => {
            return BenchmarkResult {
                name: mirror.to_string(),
                average_latency_ms: 0,
                success_rate: 0.0,
                timed_out: true,
            }
        }
    };

    let mut successes: u32 = 0;
    let mut total_latency: u128 = 0;

    for _ in 0..ATTEMPTS {
        let start = Instant::now();
        match download_and_discard_package(&client, pm, package, mirror).await {
            Ok(()) => {
                total_latency += start.elapsed().as_millis();
                successes += 1;
            }
            Err(_) => {
                // Metadata lookup failure, download failure, or timeout - skip this attempt.
            }
        }
    }

    let success_rate = (successes as f32 / ATTEMPTS as f32) * 100.0;
    let average_latency_ms = if successes > 0 {
        total_latency / successes as u128
    } else {
        0
    };

    BenchmarkResult {
        name: mirror.to_string(),
        average_latency_ms,
        success_rate,
        timed_out: successes == 0,
    }
}

/// Benchmarks a list of mirrors sequentially and returns results sorted by
/// average latency (fastest first). Mirrors that time out completely are
/// sorted to the end.
pub async fn benchmark_all(
    pm: PackageManager,
    package: &str,
    mirrors: &[String],
) -> Vec<BenchmarkResult> {
    let mut results = Vec::with_capacity(mirrors.len());

    for mirror in mirrors {
        results.push(benchmark_mirror(pm, package, mirror).await);
    }

    results.sort_by(|a, b| match (a.timed_out, b.timed_out) {
        (true, true) => std::cmp::Ordering::Equal,
        (true, false) => std::cmp::Ordering::Greater,
        (false, true) => std::cmp::Ordering::Less,
        (false, false) => a.average_latency_ms.cmp(&b.average_latency_ms),
    });

    results
}

/// Downloads `package` from `mirror` to a temporary file and immediately
/// deletes it. The package is only ever downloaded, never installed/run.
async fn download_and_discard_package(
    client: &reqwest::Client,
    pm: PackageManager,
    package: &str,
    mirror: &str,
) -> Result<(), DownloadError> {
    let download_url = resolve_download_url(client, pm, package, mirror).await?;

    let bytes = client
        .get(&download_url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;

    let file_name = download_url
        .rsplit('/')
        .find(|segment| !segment.is_empty())
        .unwrap_or("mirror-benchmark-download.tmp");
    let path = std::env::temp_dir().join(format!(
        "mirror-benchmark-{}-{}",
        std::process::id(),
        file_name
    ));

    tokio::fs::write(&path, &bytes).await?;
    tokio::fs::remove_file(&path).await?;

    Ok(())
}

/// Resolves the direct download URL for `package`'s archive file on
/// `mirror`, dispatching to the package-manager-specific lookup.
async fn resolve_download_url(
    client: &reqwest::Client,
    pm: PackageManager,
    package: &str,
    mirror: &str,
) -> Result<String, DownloadError> {
    match pm {
        PackageManager::PyPi => resolve_pypi_download_url(client, package, mirror).await,
        PackageManager::Npm => resolve_npm_download_url(client, package, mirror).await,
    }
}

/// Resolves a package's download URL from a PEP 503 "simple" index page,
/// e.g. `https://pypi.org/simple/requests/`.
async fn resolve_pypi_download_url(
    client: &reqwest::Client,
    package: &str,
    mirror: &str,
) -> Result<String, DownloadError> {
    let index_url = format!("{}{}/", ensure_trailing_slash(mirror), package);
    let body = client
        .get(&index_url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    let href = extract_hrefs(&body)
        .into_iter()
        .find(|href| is_package_archive(href))
        .ok_or("no downloadable release found in simple index")?;

    let base = reqwest::Url::parse(&index_url)?;
    let resolved = base.join(&href)?;
    Ok(resolved.to_string())
}

/// Resolves a package's tarball URL via the npm registry's `<package>/latest`
/// shortcut, e.g. `https://registry.npmjs.org/lodash/latest`.
async fn resolve_npm_download_url(
    client: &reqwest::Client,
    package: &str,
    mirror: &str,
) -> Result<String, DownloadError> {
    let metadata_url = format!("{}{}/latest", ensure_trailing_slash(mirror), package);
    let body = client
        .get(&metadata_url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    let metadata: serde_json::Value = serde_json::from_str(&body)?;
    let tarball = metadata["dist"]["tarball"]
        .as_str()
        .ok_or("npm metadata is missing dist.tarball")?;

    Ok(tarball.to_string())
}

/// Ensures a mirror base URL ends with a trailing slash so it can be safely
/// joined with a package name.
fn ensure_trailing_slash(url: &str) -> String {
    if url.ends_with('/') {
        url.to_string()
    } else {
        format!("{url}/")
    }
}

/// Extracts every `href="..."` attribute value from an HTML page, in order.
fn extract_hrefs(html: &str) -> Vec<String> {
    let mut hrefs = Vec::new();
    let mut rest = html;

    while let Some(start) = rest.find("href=\"") {
        rest = &rest[start + "href=\"".len()..];
        match rest.find('"') {
            Some(end) => {
                hrefs.push(rest[..end].to_string());
                rest = &rest[end + 1..];
            }
            None => break,
        }
    }

    hrefs
}

/// Returns true if `href` looks like a downloadable Python package archive.
fn is_package_archive(href: &str) -> bool {
    let path = href.split('#').next().unwrap_or(href);
    [".whl", ".tar.gz", ".zip", ".egg"]
        .iter()
        .any(|ext| path.ends_with(ext))
}
