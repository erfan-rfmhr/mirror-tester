# Mirror CLI

A terminal tool that benchmarks package registry mirrors, install packages from mirrors, reporting, all with both simple TUI and CLI.

## Features

- Benchmarks mirrors and ranks them by dowload speed
- Install packages from mirrors. Switch to a different mirror if fails.
- Interactive keyboard-only TUI built with `ratatui`

## Installation

Pickup desired binary from the [releases](https://github.com/erfan-rfmhr/mirror-tester/releases)
Or build from source:

- Make sure you have Rust (stable) installed via [rustup](https://rustup.rs)

```bash
git clone <https://github.com/erfan-rfmhr/mirror-tester>
cd mirror-tester
make
# or
cargo build
```

## Usage

Get help:

```bash
mirror COMMAND help
mirror pip help
```

Launch the TUI:

```bash
mirror
```

Install python packages from mirrors:

```bash
mirror pip install <package>
```
or install from requirements file:

```bash
mirror pip install -r requirements.txt
```

Run a one-off benchmark from the command line:

```bash
mirror run pypi
mirror run npm
```

Generate a JSON report for both package managers:

```bash
mirror report
```

Run the hourly scheduler (keeps benchmarking forever):

```bash
mirror schedule
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
