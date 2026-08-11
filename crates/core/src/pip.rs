//! Python package installation through a PyPI mirror with fallback.

use crate::mirror::{load_mirrors, PackageManager};
use std::process::Stdio;
use tokio::process::Command;

/// Installs `args` (a package name, `-r FILE`, etc.) via the first
/// configured PyPI mirror that succeeds. Pip's output streams straight to
/// the terminal; a failed mirror falls through to the next one.
pub async fn install(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_mirrors(PackageManager::PyPi)?;
    let interpreter = resolve_interpreter().await?;

    let mut last_status = String::from("no mirrors configured");
    for mirror in &config.mirrors {
        println!("Installing via {mirror}...");

        let status = Command::new(interpreter)
            .arg("-m")
            .arg("pip")
            .args(["install", "--index-url"])
            .arg(mirror)
            .arg("--disable-pip-version-check")
            .args(args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await?;

        if status.success() {
            println!("Installed successfully via {mirror}.");
            return Ok(());
        }

        last_status = format!("mirror {mirror} failed (exit code {:?})", status.code());
        eprintln!("{last_status}");
    }

    Err(format!("All mirrors failed, last error: {last_status}").into())
}

/// Resolves the Python interpreter to use: `python` if present, else
/// `python3`. Errors out if neither exists.
async fn resolve_interpreter() -> Result<&'static str, Box<dyn std::error::Error>> {
    if Command::new("python").arg("--version").output().await.is_ok() {
        return Ok("python");
    }
    if Command::new("python3").arg("--version").output().await.is_ok() {
        return Ok("python3");
    }
    Err("Neither 'python' nor 'python3' found on PATH; install Python to use pip install.".into())
}