# WinSweep

<div align="center">

**A safe, high-performance disk cleaning tool for Windows 10/11**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Build Status](https://github.com/N1KH1LT0X1N/WinSweep/workflows/CI/badge.svg)](https://github.com/N1KH1LT0X1N/WinSweep/actions)
[![Rust 1.75+](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)

[Download](#installation) • [Features](#features) • [Usage](#usage) • [Documentation](#documentation)

</div>

## Overview

WinSweep is a powerful yet safe disk cleaning utility designed specifically for Windows. It intelligently identifies and removes unnecessary files while protecting your critical system data and important project files.

### Why WinSweep?

- **Safe by Design** - Never deletes critical system files or important project data
- **Lightning Fast** - Parallel scanning engine processes millions of files in seconds
- **Smart Detection** - Recognizes 34+ project types and protects them from accidental deletion
- **Administrator-Free** - Works without admin rights for user directories
- **Detailed Reporting** - See exactly what's being cleaned before it happens

## Features

### Core Cleaning Capabilities
- **System Junk Removal** - Clean temp files, cache, recycle bin, and more
- **Application Cleanup** - Remove leftover data from uninstalled applications
- **Browser Cleaning** - Clear cache, cookies, and history from all major browsers
- **Windows Update Cache** - Safely remove outdated Windows update files
- **Large File Detection** - Find and remove space-hogging files you no longer need

### Safety Features
- **NEVER_DELETE List** - Comprehensive protection for system-critical files
- **Project Recognition** - Automatically detects and protects development projects
- **Junction/Symlink Handling** - Properly handles Windows reparse points
- **Audit Logging** - Complete audit trail of all operations
- **Recycle Bin Support** - Deleted files go to recycle bin when possible

### Advanced Features
- **Parallel Processing** - Multi-threaded scanning for maximum performance
- **Cross-Privilege IPC** - Secure communication between UI and elevated components
- **Configuration Management** - Customize what gets cleaned and what gets protected
- **Real-time Preview** - See what will be deleted before committing

## Installation

### Prerequisites
- Windows 10 (1903+) or Windows 11
- For building from source: Rust 1.75+, `x86_64-pc-windows-gnu` target, MinGW-w64

### From Release (Recommended)
1. Download `winsweep-<version>-setup.exe` from [GitHub Releases](https://github.com/N1KH1LT0X1N/WinSweep/releases)
2. Run the installer (requires admin for `%ProgramFiles%`)
3. Launch **WinSweep** from the Start Menu

### Portable ZIP
Download and extract anywhere; run `winsweep-gui.exe` directly — no installation required.

### From Source
```powershell
# Install the target toolchain (once)
rustup target add x86_64-pc-windows-gnu

git clone https://github.com/N1KH1LT0X1N/WinSweep.git
cd winsweep

# GUI with system tray
cargo build --release -p winsweep-gui --features system-tray

# CLI
cargo build --release -p winsweep-cli

# Binaries land in:
# target\x86_64-pc-windows-gnu\release\winsweep-gui.exe
# target\x86_64-pc-windows-gnu\release\winsweep-cli.exe
```

## Quick Start

### GUI

Double-click `winsweep-gui.exe`. The Dashboard opens with live system stats.

Common workflow:
1. **Dashboard → 🔍 Quick Scan** or navigate to **Scan**.
2. Set the scan path and click **Start Scan**.
3. Review results, check the items to delete.
4. Click **Delete Selected** (or **Delete All**).

### CLI

```powershell
# Interactive TUI
winsweep-cli C:\Users\$env:USERNAME

# Stream NDJSON — pipe into PowerShell
winsweep-cli --output ndjson C:\ |
    ForEach-Object { $_ | ConvertFrom-Json } |
    Where-Object { $_.size_bytes -gt 100MB }

# Files older than 90 days — preview only
winsweep-cli --output ndjson --older 90 --dry-run C:\Dev

# All options
winsweep-cli --help
```

## Usage

### CLI Reference

```
Usage: winsweep-cli [OPTIONS] [PATH]...

Arguments:
  [PATH]...  Paths to scan (default: current directory)

Options:
  -v, --verbose              Enable verbose logging
  -l, --log-file <FILE>      Log file path
      --mode <MODE>          Start view [scan|wsl|docker|update|services|config]
      --older <DAYS>         Only report files older than N days
      --output <FORMAT>      Output format: text (default) | ndjson
      --dry-run              Preview only — do not delete anything
  -h, --help                 Print help
  -V, --version              Print version
```

### Configuration

Configuration is stored at `%AppData%\WinSweep\config.toml`.

```toml
auto_cleanup_enabled = false
auto_cleanup_days = 7
notify_low_disk_space = true
low_disk_threshold = 10   # percent free before warning

[scan]
include_hidden = false
include_system = false
min_file_size = 1024      # bytes

[cleanup]
use_recycle_bin = true
confirm_before_delete = true
clean_temp_files = true
clean_recycle_bin = true
clean_prefetch = false
clean_browser_cache = false

[ui]
show_notifications = true
minimize_to_tray = false

[logging]
log_level = "info"
```

## Documentation

- [User Guide](docs/user-guide.md) - Detailed usage instructions
- [Developer Documentation](docs/developer-guide.md) - Architecture and contributing
- [API Reference](docs/api-reference.md) - Programmatic usage
- [FAQ](docs/faq.md) - Common questions and issues

## Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

```powershell
# Quick dev-loop check before opening a PR
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace     # 105 tests
```

### Project Structure
```
WinSweep/
├── Cargo.toml                 workspace manifest
├── .cargo/config.toml         default target: x86_64-pc-windows-gnu
├── crates/
│   ├── winsweep-common/       Config, types, NEVER_DELETE
│   ├── winsweep-core/         Scanner, CleanupManager, 33 package managers
│   ├── winsweep-cli/          TUI + ndjson streaming CLI
│   └── winsweep-gui/          egui GUI + system tray
├── tests/
│   └── integration_tests.rs   105 total tests
├── installer/
│   └── winsweep.nsi           NSIS installer script
└── docs/                      user / developer / api docs
```

## Security

WinSweep takes security seriously:

- **All operations are logged for full auditability**
- **Critical system paths are protected by default**
- **No network access required**
- **Open source for full transparency**
- **Regular security audits**

## Performance

WinSweep is optimized for speed:

- **Scans 1M+ files in under 30 seconds**
- **Uses parallel processing for multi-core CPUs**
- **Memory-efficient streaming algorithms**
- **Minimal system impact during operation**

## Compatibility

| | Supported |
|---|---|
| Windows 10 (1903+) | ✔ |
| Windows 11 | ✔ |
| Windows Server 2016+ | ✔ |
| Architecture | x64 |

## License

WinSweep is released under the [MIT License](LICENSE).

## Support

- **Documentation** — [docs/](docs/)
- **Issue Tracker** — [GitHub Issues](https://github.com/N1KH1LT0X1N/WinSweep/issues)
- **Discussions** — [GitHub Discussions](https://github.com/N1KH1LT0X1N/WinSweep/discussions)

## Acknowledgments

- **[egui](https://github.com/emilk/egui)** — immediate-mode GUI framework
- **[sysinfo](https://github.com/GuillaumeGomez/sysinfo)** — cross-platform system info
- **[tokio](https://tokio.rs/)** — async runtime
- **[windows-rs](https://github.com/microsoft/windows-rs)** — Windows API bindings
- All contributors

---

<div align="center">
  <sub>Built with ❤️ for the Windows community</sub>
</div>