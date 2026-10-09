//! Gradle project commands and coordinate installation through Maven mirrors.
//!
//! Gradle resolves dependencies while running project tasks rather than through a
//! separate package-install command. Ayeneh runs those tasks with a temporary
//! init script that replaces project, plugin, and buildscript repositories with
//! one configured mirror, then retries the task against the next mirror when it
//! fails.

use crate::mirror::{load_mirrors, Registry};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::process::Command;

/// Downloads `args` (Maven coordinates, e.g.
/// `com.google.guava:guava:34.0.0-jre`) via the first configured Gradle
/// mirror that succeeds. Gradle's output streams straight to the terminal; a
/// failed mirror falls through to the next one.
pub async fn install(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        return Err(
            "No packages given; pass Maven coordinates like group:artifact:version.".into(),
        );
    }

    let config = load_mirrors(Registry::Gradle)?;
    let gradle = resolve_gradle().await?;
    let coordinates: Vec<String> = args.iter().map(|arg| with_version(arg)).collect();

    let mut last_status = String::from("no mirrors configured");
    for mirror in &config.mirrors {
        println!("Installing via {mirror}...");

        let project = temp_project_dir();
        write_project(&project, mirror, &coordinates)?;

        let status = Command::new(&gradle)
            .args(["resolveMirror", "--console=plain", "--no-daemon"])
            .current_dir(&project)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await;

        let _ = fs::remove_dir_all(&project);
        let status = status?;

        if status.success() {
            println!("Installed successfully via {mirror}.");
            return Ok(());
        }

        last_status = format!("mirror {mirror} failed (exit code {:?})", status.code());
        eprintln!("{last_status}");
    }

    Err(format!("All mirrors failed, last error: {last_status}").into())
}

/// Runs a normal Gradle project command through the configured mirrors.
///
/// The first item in `args` is the Gradle task or command and the remaining
/// items are passed through unchanged. `sync` invokes a generated task that
/// resolves every resolvable configuration because dependency sync is an IDE
/// operation, not a portable Gradle CLI task.
pub async fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (task, task_args) = args
        .split_first()
        .ok_or("No Gradle command given; try 'build' or 'sync'.")?;
    let task = if task == "sync" { "ayenehSync" } else { task };

    let config = load_mirrors(Registry::Gradle)?;
    let gradle = resolve_gradle().await?;
    let mut last_status = String::from("no mirrors configured");

    for mirror in &config.mirrors {
        println!("Running Gradle {task} via {mirror}...");

        let init_script = temp_init_script_path();
        write_init_script(&init_script, mirror)?;

        let mut command_args = vec![
            "--init-script".to_string(),
            init_script.to_string_lossy().into_owned(),
            task.to_string(),
        ];
        command_args.extend(task_args.iter().cloned());
        if !has_option(task_args, "--console") {
            command_args.push("--console=plain".to_string());
        }
        if !has_option(task_args, "--daemon") && !has_option(task_args, "--no-daemon") {
            command_args.push("--no-daemon".to_string());
        }

        let status = Command::new(&gradle)
            .args(&command_args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await;

        let _ = fs::remove_file(&init_script);
        let status = status?;

        if status.success() {
            println!("Gradle {task} completed successfully via {mirror}.");
            return Ok(());
        }

        last_status = format!("mirror {mirror} failed (exit code {:?})", status.code());
        eprintln!("{last_status}");
    }

    Err(format!("All mirrors failed, last error: {last_status}").into())
}

/// Resolves the Gradle launcher, preferring a project wrapper when available.
async fn resolve_gradle() -> Result<String, Box<dyn std::error::Error>> {
    for candidate in ["./gradlew", "gradle"] {
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

    Err("Neither './gradlew' nor 'gradle' was found on PATH.".into())
}

/// Coordinates without a version (`group:artifact`) resolve to the newest
/// available version.
fn with_version(coordinate: &str) -> String {
    if coordinate.matches(':').count() == 1 {
        format!("{coordinate}:+")
    } else {
        coordinate.to_string()
    }
}

/// Path of the throwaway project used for one coordinate-install attempt.
fn temp_project_dir() -> PathBuf {
    std::env::temp_dir().join(format!(
        "ayeneh-gradle-{}-{}",
        std::process::id(),
        unique_suffix()
    ))
}

fn temp_init_script_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "ayeneh-gradle-init-{}-{}.gradle",
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

/// Writes the throwaway project that resolves `coordinates` from `mirror`.
fn write_project(dir: &Path, mirror: &str, coordinates: &[String]) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    fs::write(
        dir.join("settings.gradle"),
        "rootProject.name = 'ayeneh-mirror-install'\n",
    )?;
    fs::write(dir.join("build.gradle"), build_script(mirror, coordinates))?;
    Ok(())
}

/// Writes the init script used to make one Gradle invocation use one mirror.
fn write_init_script(path: &Path, mirror: &str) -> std::io::Result<()> {
    fs::write(path, build_init_script(mirror))
}

/// Builds the Gradle script used to download `coordinates` from `mirror`:
/// one repository, the requested dependencies, and a `resolveMirror` task
/// that forces artifact resolution (and therefore the download).
fn build_script(mirror: &str, coordinates: &[String]) -> String {
    let dependencies: String = coordinates
        .iter()
        .map(|coordinate| format!("    mirrorDeps '{coordinate}'\n"))
        .collect();

    format!(
        r#"// Generated by ayeneh-cli; deleted after the install attempt.
repositories {{
    maven {{ url = uri('{mirror}') }}
}}

configurations {{
    mirrorDeps {{
        // Java runtime classpath attributes. Without these Gradle cannot
        // select a variant of modules that publish Gradle Module Metadata
        // (e.g. Guava) and resolution fails with "no matching variant".
        canBeConsumed = false
        canBeResolved = true
        attributes {{
            attribute(Usage.USAGE_ATTRIBUTE, objects.named(Usage, Usage.JAVA_RUNTIME))
            attribute(Category.CATEGORY_ATTRIBUTE, objects.named(Category, Category.LIBRARY))
            attribute(LibraryElements.LIBRARY_ELEMENTS_ATTRIBUTE, objects.named(LibraryElements, LibraryElements.JAR))
            attribute(Bundling.BUNDLING_ATTRIBUTE, objects.named(Bundling, Bundling.EXTERNAL))
            attribute(TargetJvmEnvironment.TARGET_JVM_ENVIRONMENT_ATTRIBUTE, objects.named(TargetJvmEnvironment, TargetJvmEnvironment.STANDARD_JVM))
        }}
    }}
}}

dependencies {{
{dependencies}}}

tasks.register('resolveMirror') {{
    def mirrorDeps = configurations.mirrorDeps
    doLast {{
        mirrorDeps.resolve().each {{ file ->
            println "Resolved: ${{file}}"
        }}
    }}
}}
"#
    )
}

/// Builds an init script that replaces repositories declared by the project,
/// settings, plugins, and buildscript with exactly one Ayeneh mirror.
fn build_init_script(mirror: &str) -> String {
    let mirror = escape_groovy_string(mirror);
    format!(
        r#"// Generated by ayeneh-cli; deleted after this Gradle invocation.
def ayenehMirror = '{mirror}'
def ayenehUseMirror = {{ repositories ->
    repositories.clear()
    repositories.maven {{ url = uri(ayenehMirror) }}
}}

settingsEvaluated {{ settings ->
    ayenehUseMirror(settings.pluginManagement.repositories)
    try {{
        ayenehUseMirror(settings.dependencyResolutionManagement.repositories)
    }} catch (Exception ignored) {{
        // Older Gradle versions do not expose dependencyResolutionManagement.
    }}
}}

gradle.beforeProject {{ project ->
    ayenehUseMirror(project.buildscript.repositories)
    ayenehUseMirror(project.repositories)
    project.afterEvaluate {{
        ayenehUseMirror(project.repositories)
    }}
}}

gradle.projectsLoaded {{
    gradle.rootProject.tasks.register('ayenehSync') {{
        doLast {{
            gradle.rootProject.allprojects.each {{ project ->
                project.configurations.findAll {{ configuration ->
                    configuration.canBeResolved
                }}.each {{ configuration ->
                    println "Resolving: ${{project.path}}:${{configuration.name}}"
                    configuration.resolve()
                }}
            }}
        }}
    }}
}}

gradle.projectsEvaluated {{
    gradle.rootProject.allprojects.each {{ project ->
        ayenehUseMirror(project.repositories)
    }}
}}
"#
    )
}

fn escape_groovy_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "\\'")
}

fn has_option(args: &[String], option: &str) -> bool {
    args.iter()
        .any(|arg| arg == option || arg.starts_with(&format!("{option}=")))
}

#[cfg(test)]
mod tests {
    use super::{build_init_script, build_script, with_version};

    #[test]
    fn script_targets_mirror_and_coordinates() {
        let script = build_script(
            "https://repo1.maven.org/maven2/",
            &["com.google.guava:guava:34.0.0-jre".to_string()],
        );

        assert!(script.contains("maven { url = uri('https://repo1.maven.org/maven2/') }"));
        assert!(script.contains("mirrorDeps 'com.google.guava:guava:34.0.0-jre'"));
        assert!(script.contains("attribute(Usage.USAGE_ATTRIBUTE"));
        assert!(script.contains("tasks.register('resolveMirror')"));
    }

    #[test]
    fn coordinates_without_version_resolve_latest() {
        assert_eq!(
            with_version("com.google.guava:guava"),
            "com.google.guava:guava:+"
        );
        assert_eq!(
            with_version("com.google.guava:guava:34.0.0-jre"),
            "com.google.guava:guava:34.0.0-jre"
        );
    }

    #[test]
    fn init_script_replaces_project_and_plugin_repositories() {
        let script = build_init_script("https://mirror.example/maven/");

        assert!(script.contains("settings.pluginManagement.repositories"));
        assert!(script.contains("settings.dependencyResolutionManagement.repositories"));
        assert!(script.contains("project.buildscript.repositories"));
        assert!(script.contains("project.repositories"));
        assert!(script.contains("tasks.register('ayenehSync')"));
        assert!(script.contains("uri(ayenehMirror)"));
    }

    #[test]
    fn init_script_escapes_groovy_string_literals() {
        let script = build_init_script("https://mirror.example/a'b/");

        assert!(script.contains("https://mirror.example/a\\'b/"));
    }
}
