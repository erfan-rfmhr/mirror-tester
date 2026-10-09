![Ayeneh — install packages through mirrors, mirror testing and benchmarking](./docs/banner.webp)

# Ayeneh

A terminal tool that installs packages from mirrors, benchmarks, and reports results, all with both simple TUI and CLI.

## Supported Package Managers


|Registry        |Package Manager        |
|----------------|-----------------------|
|PyPi|pip, uv|
|npm |npm|
|Gradle/Maven|gradle, mvn (Maven repositories)|


## Features

- Install packages from mirrors. Switch to a different mirror if fails.
- Benchmarks mirrors and ranks them by dowload speed.
- Interactive keyboard-only TUI built with `ratatui`.

## Installation

Pickup desired binary from the [releases](https://github.com/pasta-engineers/ayeneh/releases)
Or build from source:

- Make sure you have Rust (stable) installed via [rustup](https://rustup.rs)

```bash
git clone https://github.com/pasta-engineers/ayeneh.git
cd ayeneh
make
# or
cargo build
```

## Usage

The project ships two binaries:

* `ayeneh-cli` — non-interactive CLI, suitable for CI pipelines, Docker
  images, and shell scripting.
* `ayeneh-tui` — interactive Ratatui application.

Get help:

```bash
ayeneh-cli --help
ayeneh-cli pip --help
ayeneh-cli npm --help
ayeneh-cli gradle --help
```

### Environment Variables

Runtime behavior can be tuned without rebuilding:

| Variable                          | Meaning                                | Default                   |
|-----------------------------------|----------------------------------------|---------------------------|
| `AYN_DATA_DIR`                    | Directory holding the registry JSON files | `./data` |
| `AYN_REPORTS_DIR`                 | Directory for generated reports        | auto-detected `reports/`  |
| `AYN_TIMEOUT`                | Per-attempt HTTP timeout (seconds)     | `15`                      |
| `AYN_ATTEMPTS`                    | Benchmark attempts per mirror          | `3`                       |
| `AYN_SCHEDULE_INTERVAL_SECS`      | Delay between scheduler cycles (seconds) | `3600`                    |

The numeric variables fall back to their default if unset or given a
non-numeric, zero, or negative value.

Launch the TUI:
```bash
ayeneh-tui
```
![TUI screenshot](./docs/tui.png)

In TUI, you can:
- Run benchmarks
- Reorder mirrors based on benchmark results, so faster mirrors are listed first in installation processes
- Add/remove mirrors manually
CLI features are available in TUI as well.

Install python packages from mirrors:

```bash
ayeneh-cli pip install <package>
```
or install from requirements file:

```bash
ayeneh-cli pip install -r requirements.txt
```

Install npm packages from mirrors:

```bash
ayeneh-cli npm install <package>
```

Run Gradle dependency-resolving project commands through the configured Maven
mirrors. The command runs in the current Gradle project directory and retries
with the next mirror if dependency resolution fails:

```bash
ayeneh-cli gradle build
ayeneh-cli gradle sync
ayeneh-cli gradle test --refresh-dependencies
```

`sync` is an Ayeneh command that resolves every resolvable Gradle
configuration. It is useful from a terminal or CI; IDE sync itself is not a
portable Gradle CLI task. Any other Gradle task is forwarded as well.

Run Maven lifecycle phases and plugin goals through the same mirror list:

```bash
ayeneh-cli maven compile
ayeneh-cli maven test
ayeneh-cli maven package
ayeneh-cli maven verify
ayeneh-cli maven install
ayeneh-cli maven sync
```

`sync` maps to Maven's `dependency:resolve`. Maven commands also retry with the
next configured mirror and do not modify `pom.xml` or other project files.

For direct coordinate downloads, the existing Gradle shortcut remains
available. The version may be omitted, in which case Gradle resolves the
newest available one:

```bash
ayeneh-cli gradle install com.google.guava:guava:34.0.0-jre
ayeneh-cli gradle install com.google.guava:guava
```

Gradle artifacts are downloaded into Gradle's own dependency cache, so a later
build that uses the same coordinates can reuse them.

Run a one-off benchmark from the command line:

```bash
ayeneh-cli run pypi
ayeneh-cli run npm
ayeneh-cli run gradle
```

Generate a JSON report for all package managers:

```bash
ayeneh-cli report
```

Run the hourly scheduler (keeps benchmarking forever):

```bash
ayeneh-cli schedule
```

## Example Report

`reports/2026-07-12_14-30.json`:

```json
{
  "package_manager": "pypi",
  "generated_at": "2026-07-12T14:30:00+00:00",
  "results": [
    { "mirror": "https://pypi.org/simple/", "latency": 120, "success": 100.0 },
    { "mirror": "https://mirror.example1/simple/", "latency": 180, "success": 100.0 }
  ],
  "best": "https://pypi.org/simple/"
}
```

## Mirror Lists

`pypi.json`, `npm.json`, and `gradle.json` each define the sample `package` to
download during benchmarking and the list of `mirrors` to test it against. By
default, the program reads them from `./data`:

```json
{
    "package": "requests",
    "mirrors": [
        "https://pypi.org/simple/",
        "https://pypi.devneeds.ir/simple/",
        "https://package-mirror.liara.ir/repository/pypi/"
    ]
}
```

Set `AYN_DATA_DIR` when the registry files are stored elsewhere:

```bash
AYN_DATA_DIR=/opt/mirror/registries ayeneh-cli run pypi
```

Edit these JSON files to add or remove mirrors, or to change the package used
for benchmarking. Mirror lists are configured statically — no network
scraping is performed to discover them.

In `gradle.json` the `package` is a pair of Maven coordinates
(`group:artifact`, e.g. `com.google.guava:guava`) and each mirror is a Maven
repository root, so the same list can be used as a `maven { url ... }`
repository in Gradle or Maven.

### Docker

A Python-based image is provided that ships the prebuilt `ayeneh-cli` binary
alongside the mirror registry config, so it can be used directly as a base for
installing packages or benchmarking. Build it from the repository root (a
release binary must already exist):

```bash
cargo build --release -p ayeneh-cli
docker build -f docker/Dockerfile.python -t ayeneh-cli .
```

Run it, or extend it in your own Dockerfile to install packages from mirrors:

```bash
docker run --rm ayeneh-cli ayeneh-cli pip install requests
```

```dockerfile
FROM ayeneh-cli
RUN ayeneh-cli pip install -r requirements.txt
```

A Node.js-based image is provided for npm. Build it the same way (a release
binary must already exist):

```bash
docker build -f docker/Dockerfile.npm -t ayeneh-cli-npm .
```

Run it, or extend it in your own Dockerfile to install packages from npm
mirrors:

```bash
docker run --rm ayeneh-cli-npm ayeneh-cli npm install lodash
```

```dockerfile
FROM ayeneh-cli-npm
RUN ayeneh-cli npm install lodash express
```
