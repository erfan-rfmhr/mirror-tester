//! Maven project commands through configured Maven repository mirrors.
//!
//! Maven's dependency-installing lifecycle phases (`compile`, `test`,
//! `package`, `verify`, and `install`) are run with a temporary settings file
//! that points every repository at one Ayeneh mirror. A failed invocation is
//! retried with the next mirror.

use crate::mirror::{load_mirrors, Registry};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::process::Command;

/// Runs a Maven goal or lifecycle phase through the configured mirrors.
///
/// The first item in `args` is the Maven goal/phase and the remaining items
/// are passed through unchanged. `sync` is an Ayeneh alias for
/// `dependency:resolve`, since Maven has no portable sync command.
pub async fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (goal, goal_args) = args
        .split_first()
        .ok_or("No Maven command given; try 'install' or 'sync'.")?;
    if has_settings_option(goal_args) {
        return Err(
            "Do not pass a Maven settings option; ayeneh supplies one for each mirror attempt."
                .into(),
        );
    }

    let goal = if goal == "sync" {
        "dependency:resolve"
    } else {
        goal
    };
    let config = load_mirrors(Registry::Gradle)?;
    let maven = resolve_maven().await?;
    let mut last_status = String::from("no mirrors configured");

    for mirror in &config.mirrors {
        println!("Running Maven {goal} via {mirror}...");

        let settings = temp_settings_path();
        write_settings(&settings, mirror)?;
        let command_args = [
            "--settings".to_string(),
            settings.to_string_lossy().into_owned(),
            goal.to_string(),
        ];

        let status = Command::new(&maven)
            .args(command_args)
            .args(goal_args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await;

        let _ = fs::remove_file(&settings);
        let status = status?;

        if status.success() {
            println!("Maven {goal} completed successfully via {mirror}.");
            return Ok(());
        }

        last_status = format!("mirror {mirror} failed (exit code {:?})", status.code());
        eprintln!("{last_status}");
    }

    Err(format!("All mirrors failed, last error: {last_status}").into())
}

async fn resolve_maven() -> Result<String, Box<dyn std::error::Error>> {
    for candidate in ["./mvnw", "mvn"] {
        if Command::new(candidate)
            .arg("--version")
            .output()
            .await
            .map(|output| output.status.success())
            .unwrap_or(false)
        {
            return Ok(candidate.to_string());
        }
    }

    Err("Neither './mvnw' nor 'mvn' was found on PATH.".into())
}

fn temp_settings_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "ayeneh-maven-settings-{}-{}.xml",
        std::process::id(),
        unique_suffix()
    ))
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn write_settings(path: &Path, mirror: &str) -> std::io::Result<()> {
    fs::write(path, build_settings(mirror))
}

fn build_settings(mirror: &str) -> String {
    let mirror = escape_xml(mirror);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<settings xmlns="http://maven.apache.org/SETTINGS/1.0.0"
          xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
          xsi:schemaLocation="http://maven.apache.org/SETTINGS/1.0.0 https://maven.apache.org/xsd/settings-1.0.0.xsd">
  <mirrors>
    <mirror>
      <id>ayeneh-mirror</id>
      <name>Mirror selected by ayeneh</name>
      <url>{mirror}</url>
      <mirrorOf>*</mirrorOf>
    </mirror>
  </mirrors>
</settings>
"#
    )
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn has_settings_option(args: &[String]) -> bool {
    args.iter().any(|arg| {
        arg == "-s"
            || arg == "--settings"
            || arg.starts_with("--settings=")
            || (arg.starts_with("-s") && arg.len() > 2)
    })
}

#[cfg(test)]
mod tests {
    use super::{build_settings, escape_xml, has_settings_option};

    #[test]
    fn settings_mirror_allows_maven_to_use_one_repository() {
        let settings = build_settings("https://repo.example/maven/");

        assert!(settings.contains("<url>https://repo.example/maven/</url>"));
        assert!(settings.contains("<mirrorOf>*</mirrorOf>"));
    }

    #[test]
    fn settings_escape_xml_values() {
        assert_eq!(
            escape_xml("https://example.test/a?x=1&y=2"),
            "https://example.test/a?x=1&amp;y=2"
        );
    }

    #[test]
    fn settings_options_are_rejected() {
        assert!(has_settings_option(&[
            "--settings=/tmp/custom.xml".to_string()
        ]));
        assert!(has_settings_option(&[
            "-s".to_string(),
            "custom.xml".to_string()
        ]));
        assert!(!has_settings_option(&["-DskipTests".to_string()]));
    }
}
