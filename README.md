# WinSweep

<div align="center">

![WinSweep Logo](https://via.placeholder.com/200x80/1e1e1e/ffffff?text=WinSweep)

**A safe, high-performance disk cleaning tool for Windows 10/11**

[![Crates.io](https://img.shields.io/crates/v/winsweep)](https://crates.io/crates/winsweep)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Build Status](https://github.com/winsweep/winsweep/workflows/CI/badge.svg)](https://github.com/winsweep/winsweep/actions)

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
- Windows 10 or Windows 11
- [Visual Studio C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) (for building from source)

### From Release (Recommended)
1. Download the latest release from [GitHub Releases](https://github.com/winsweep/winsweep/releases)
2. Run the installer
3. Launch WinSweep from the Start menu

### From Source
```powershell
# Clone the repository
git clone https://github.com/winsweep/winsweep.git
cd winsweep

# Build release binaries
.\build.ps1 --release

# Run the application
.\target\release\winsweep.exe
```

### Using Cargo
```powershell
# Install from crates.io (coming soon)
cargo install winsweep

# Or build from source
cargo install --path .
```

## Quick Start

### GUI Mode
```powershell
# Launch the graphical interface
winsweep gui
```

### Command Line Interface
```powershell
# Scan for junk files (dry run)
winsweep scan --path C:\

# Clean with confirmation
winsweep clean --path C:\Users\%USERNAME% --interactive

# Clean specific categories
winsweep clean --categories temp,cache,logs --auto
```

### PowerShell Module
```powershell
# Import the module
Import-Module WinSweep

# Clean with PowerShell pipeline
Get-ChildItem C:\Temp | Remove-WinSweepJunk

# Schedule regular cleaning
Register-WinSweepScheduledTask -Daily -At 3am
```

## Usage

### Graphical Interface
The WinSweep GUI provides an intuitive way to clean your system:

1. **Select Drive/Folder** - Choose what to scan
2. **Review Results** - See categorized junk files
3. **Customize** - Include/exclude specific file types
4. **Clean** - Click "Clean" to remove selected files

### Command Line Options
```bash
# Basic usage
winsweep [OPTIONS] <SUBCOMMAND>

# Subcommands
scan        Scan for junk files without deleting
clean       Clean up junk files
config      Manage configuration
gui         Launch graphical interface

# Options
-p, --path <PATH>         Path to scan (default: system drives)
-c, --categories <CATS>   Specific categories to clean
--dry-run                 Show what would be deleted
--interactive             Prompt before each deletion
--admin                   Request administrator privileges
--config <FILE>           Use custom config file
```

### Configuration
WinSweep creates a configuration file at:
```
%APPDATA%\WinSweep\config.toml
```

Example configuration:
```toml
[categories]
temp = true
cache = true
logs = true
recycle_bin = false

[protection]
protect_projects = true
protect_system = true
min_file_age = "7d"
max_file_size = "1GB"

[logging]
level = "info"
file = "%APPDATA%\WinSweep\winsweep.log"
```

## Documentation

- [User Guide](docs/user-guide.md) - Detailed usage instructions
- [Developer Documentation](docs/developer-guide.md) - Architecture and contributing
- [API Reference](docs/api-reference.md) - Programmatic usage
- [FAQ](docs/faq.md) - Common questions and issues

## Screenshots

<div align="center">
  <img src="https://via.placeholder.com/800x450/2d2d2d/ffffff?text=Main+Interface" alt="Main Interface" width="45%">
  <img src="https://via.placeholder.com/800x450/2d2d2d/ffffff?text=Scan+Results" alt="Scan Results" width="45%">
</div>

## Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

### Development Setup
```powershell
# Clone the repository
git clone https://github.com/winsweep/winsweep.git
cd winsweep

# Install development dependencies
cargo install cargo-watch cargo-nextest

# Run tests
cargo nextest run --workspace

# Run with auto-reload
cargo watch -x 'run -- gui'
```

### Project Structure
```
WinSweep/
├── crates/
│   ├── winsweep-common/     # Shared types and utilities
│   ├── winsweep-core/       # Core scanning and cleaning logic
│   ├── winsweep-cli/        # Command-line interface
│   └── winsweep-gui/        # Graphical user interface
├── scripts/                 # Build and utility scripts
├── docs/                    # Documentation
└── tests/                   # Integration tests
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

- **Windows 10** - Version 1903 and later
- **Windows 11** - All versions
- **Windows Server** - 2016 and later
- **Architecture** - x64, ARM64

## License

WinSweep is released under the [MIT License](LICENSE).

## Support

- **Documentation** - [docs/](docs/)
- **Issue Tracker** - [GitHub Issues](https://github.com/winsweep/winsweep/issues)
- **Discussions** - [GitHub Discussions](https://github.com/winsweep/winsweep/discussions)
- **Email Support** - [support@winsweep.io](mailto:support@winsweep.io)

## Acknowledgments

- **Built with Rust** for safety and performance
- **UI powered by egui**
- **Thanks to all contributors**

---

<div align="center">
  <sub>Built with ❤️ for the Windows community</sub>
</div>