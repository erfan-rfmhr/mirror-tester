# Mirror Benchmark

A terminal tool that benchmarks package registry mirrors (PyPI and npm)
and recommends the fastest one.

## Features

- Benchmarks PyPI and npm mirrors defined in `data/pypi.json` and `data/npm.json`
- Measures real-world speed by actually downloading a sample package from
  each mirror (never installing it) and deleting it immediately afterward
- Interactive keyboard-only TUI built with `ratatui`
- One-off CLI benchmarks (`run pypi`, `run npm`)
- JSON report generation
- Simple hourly scheduler
- Never crashes on an unreachable mirror — failed mirrors are marked as timeouts

## Requirements

- Rust (stable), installed via [rustup](https://rustup.rs)
- Internet access to reach the mirrors listed in `data/`

## Installation

```bash
git clone <this-repo>
cd mirror-benchmark
```

## Build

```bash
make
# or
cargo build
```

## Run

Launch the TUI:

```bash
make run
```

Run a one-off benchmark from the command line:

```bash
cargo run -- run pypi
cargo run -- run npm
```

Generate a JSON report for both package managers:

```bash
cargo run -- report
```

Run the hourly scheduler (keeps benchmarking forever):

```bash
cargo run -- schedule
```

## Example TUI

```
+------------------------------------------------+
| Mirror Benchmark                                |
+------------------------------------------------+

Package Manager:

> PyPI
  npm

--------------------------------------------
Results

Mirror                     Avg(ms)   Success
pypi.org                   120       100%
mirror1                    180       100%
mirror2                    timeout   0%
--------------------------------------------

Fastest Mirror:
https://pypi.org/simple/

[q] Quit   [Enter] Run Benchmark   [up/down] Switch
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

`data/pypi.json` and `data/npm.json` each define the sample `package` to
download during benchmarking and the list of `mirrors` to test it against:

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

Edit these files to add or remove mirrors, or to change the package used
for benchmarking. Mirror lists are configured statically — no network
scraping is performed to discover them.
