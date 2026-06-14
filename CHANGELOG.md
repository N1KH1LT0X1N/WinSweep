# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-01-15

### Added

- **33+ package manager cleaners** — npm, pnpm, yarn, pip, poetry, cargo, go-modules, nuget, gradle, maven, flutter, bun, pixi, composer, vcpkg, conan, sbt, go-build, android-sdk, git-lfs, playwright, cypress, and more
- **Browser cache cleanup** — dedicated Chrome, Edge, and Firefox cache managers with full `PackageManager` trait implementation
- **Task Scheduler integration** — register automated cleanup tasks via `schtasks.exe` with daily/weekly/monthly frequencies
- **System tray support** — minimize to tray, tray-icon 0.14.3 API (GUI feature flag)
- **PowerShell notifications** — native Windows toast notifications with XML escaping fix
- **Full GUI (8 views)** — Dashboard, Scan, WSL, Docker, Package Managers, Windows Update, Services, Settings, and About
- **CLI NDJSON streaming** — machine-readable output for piping into PowerShell, jq, or other tools
- **Audit logger** — complete audit trail of all cleanup operations
- **NSIS installer** — `winsweep.nsi` for one-click Windows setup
- **WSL detection & cleanup** — detect WSL distros and reclaim disk space
- **Docker client integration** — prune images, containers, volumes, and build cache
- **Windows Service management** — scan and manage system services
- **Windows Update cache cleanup** — safely remove outdated update files
- **Junction / symlink detection** — proper handling of Windows reparse points
- **Restart Manager integration** — safely close applications holding file locks
- **Tool detection** — auto-detect installed dev tools and package managers
- **Home Edition compatibility layer** — gracefully degrade when Windows APIs are restricted
- **Windows edition detection** — probe SKU and adjust features accordingly
- **Cross-privilege IPC** — secure communication between UI and elevated components
- **Configuration management** — TOML-based config with per-feature flags
- **Localization framework** — English and Spanish locale files
- **Comprehensive test suite** — 85+ unit and integration tests

### Changed

- Migrated to Rust workspace with 4 crates (`winsweep-common`, `winsweep-core`, `winsweep-cli`, `winsweep-gui`)
- Default target triple set to `x86_64-pc-windows-gnu` via `.cargo/config.toml`
- Dashboard now shows per-drive mini bars with color-coded usage (green/amber/red)
- Auto-cleanup now respects individual `config.cleanup` flags (`temp_files`, `browser_cache`, `prefetch`) separately

### Fixed

- PowerShell notification XML escaping bug (`ps_escape` now handles `&`, `<`, `>`, `"`, `'`)
- `Cargo.lock` now properly tracked for reproducible builds (binary crate)
- All GitHub URLs normalized to `N1KH1LT0X1N/WinSweep`

### Security

- Added `SECURITY.md` vulnerability disclosure policy
- Added `CODE_OF_CONDUCT.md` (Contributor Covenant v2.1)
- Added Dependabot configuration for automated dependency updates
- Hardened CI with `cargo audit` and `cargo outdated` checks

[1.0.0]: https://github.com/N1KH1LT0X1N/WinSweep/releases/tag/v1.0.0
