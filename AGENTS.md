# Agent Guide

## Project Overview

`ayeneh`: Rust 2021 Cargo workspace. Benchmarks package registry mirrors, reports performance, installs packages with mirror fallback. Supports PyPI, npm, and Gradle (Maven repositories). Scripted CLI + interactive terminal UI.

Runtime networked: benchmark resolves + downloads real package archive from each mirror, writes briefly to OS temp dir, deletes, records latency. Never installs or executes benchmark package.

## Repository Layout

```text
crates/
  core/       Shared library
  cli/        `ayeneh-cli` command parsing and command handlers
  tui/        `ayeneh-tui` Ratatui/Crossterm interface
data/         Static mirror configuration
.github/      CI workflows
ARCHITECTURE.md
Cargo.toml
Makefile
README.md
```

Read `ARCHITECTURE.md` for data flow + benchmark semantics.

## Crate Boundaries

- `ayeneh-core`: reusable domain + app logic. Modules: `app`, `benchmark`, `config`, `mirror`, `pip`, `report`, `scheduler`.
- `ayeneh-cli`: `clap` parsing, command dispatch, stdout tables, CLI error/exit behavior. Handlers exposed from `src/lib.rs` for TUI reuse.
- `ayeneh-tui`: terminal setup, keyboard events, UI state, rendering. Wraps `ayeneh_core::app::App`; can dispatch CLI commands when args supplied.

Dependency rules:

- Keep `ayeneh-core` independent of `ratatui` + `crossterm`.
- Keep `ayeneh-cli` independent of `ratatui` + `crossterm`.
- Shared behavior in `ayeneh-core`, not frontends.
- TUI-only state + rendering in `crates/tui`.

## Important Runtime Details

### Mirror Configuration

`data/pypi.json`, `data/npm.json`, and `data/gradle.json` each contain:

- `package`: sample package downloaded during benchmarks (`group:artifact` for Gradle)
- `mirrors`: ordered list of mirror base URLs

Mirror discovery static; no scraping or auto-discovery. TUI add-mirror appends directly to selected JSON config.

Data dir: `MIRROR_DATA_DIR` if set, else `./data`. Must contain `pypi.json` + `npm.json`.

Reports: `MIRROR_REPORTS_DIR` first, else `reports/` dir near executable or working dir.

### Benchmarking

- 3 sequential attempts per mirror by default (`MIRROR_ATTEMPTS`).
- 15s `reqwest` client timeout per attempt by default (`MIRROR_TIMEOUT_SECS`).
- PyPI: resolve archive from PEP 503 simple index.
- npm: resolve `dist.tarball` from registry `/latest` metadata.
- Gradle/Maven: resolve the `<release>` version from the artifact's `maven-metadata.xml`, then download the matching `{artifact}-{version}.jar`.
- Avg successful attempt latencies; failed attempts lower success rate, do not abort run.
- Zero-success mirrors marked timed out, sort last.
- Else sort by avg latency ascending.

Benchmarks use real network; failures may stem from DNS, connectivity, mirror metadata, rate limits, or download host. Check failure details before calling network-dependent test failure a code regression.

### Reports and Generated Files

`ayeneh-cli report` + `ayeneh-cli schedule` write JSON to `reports/YYYY-MM-DD_HH-MM.json`. Report JSON git-ignored; do not force generated reports into commits unless asked.

## Common Commands

Example commands in the [Makefile](Makefile).

## Development Workflow

1. Inspect relevant crate + module boundaries before changes.
2. Shared behavior: implement + test in `crates/core` first; keep frontend changes to presentation + command wiring.
3. Format + validate Rust changes:

   ```bash
   cargo fmt --all -- --check
   cargo check --workspace
   cargo test --workspace
   ```

## CI and Releases

- `.github/workflows/rust.yml` builds and tests on pushes and pull requests to `main`.
- `.github/workflows/release.yml` builds release binaries for Ubuntu and Windows when a GitHub release is published.
- Release artifacts are `ayeneh-cli` and `ayeneh-tui` (with `.exe` on Windows).

Keep workspace dependency versions in the root `Cargo.toml` and use workspace dependencies from individual crate manifests where possible.

## Change Guidance

- Preserve the CLI's non-interactive behavior and meaningful exit codes.
- Preserve TUI terminal cleanup, including raw-mode and alternate-screen restoration on normal exit.
- Do not move network, filesystem, or subprocess logic into UI rendering code.
- When changing the benchmark algorithm, update `ARCHITECTURE.md` and any user-facing README behavior that has changed.
- When changing the JSON shape of mirror configuration or reports, update the sample files/documentation and consider compatibility with existing files.

## graphify

This project has a knowledge graph at graphify-out/ with god nodes, community structure, and cross-file relationships.

When the user types `/graphify`, use the installed graphify skill or instructions before doing anything else.

Rules:
- For codebase questions, first run `graphify query "<question>"` when graphify-out/graph.json exists. Use `graphify path "<A>" "<B>"` for relationships and `graphify explain "<concept>"` for focused concepts. These return a scoped subgraph, usually much smaller than GRAPH_REPORT.md or raw grep output.
- Dirty graphify-out/ files are expected after hooks or incremental updates; dirty graph files are not a reason to skip graphify. Only skip graphify if the task is about stale or incorrect graph output, or the user explicitly says not to use it.
- If graphify-out/wiki/index.md exists, use it for broad navigation instead of raw source browsing.
- Read graphify-out/GRAPH_REPORT.md only for broad architecture review or when query/path/explain do not surface enough context.
- After modifying code, run `graphify update .` to keep the graph current (AST-only, no API cost).
