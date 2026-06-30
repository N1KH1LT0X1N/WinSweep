# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Live CLI backends** — every CLI page now performs real work instead of UI-only
  stubs: directory scanning (`Scanner`), WSL VHDX compaction, Docker cleanup,
  Windows Update cache cleanup, service start/stop, configuration persistence, and
  package-manager refresh/clean/info. The `--dry-run` flag is honored across all
  destructive handlers.
- **Service start-type & deletion APIs** — `ServiceManager::query_start_type`
  (via `QueryServiceConfigW`), `delete_service` (via `DeleteService`, guarded against
  critical services), and `re_enable_service`. `ServiceInfo::start_type` and
  `can_delete` are now populated accurately.
- **Executable version detection** — `ToolDetector` reads four-part file versions
  from the Win32 `VS_FIXEDFILEINFO` resource and parses the NuGet banner for its
  version.
- **WSL compaction resilience** — up to 5 compaction attempts with exponential
  backoff (2s·2ⁿ), a full `wsl --shutdown` between attempts to release VHDX locks,
  sparse-VHDX detection, and an `attempts` count on `WslCompactResult`.

### Changed

- `CleanupManager::cleanup` now accepts the originating `scan_id` so the returned
  `CleanupResult` is correlated with its scan instead of generating a new id.
- Hardened the cross-privilege IPC named-pipe DACL to least privilege
  (`SYSTEM`, `Administrators`, and the pipe owner) instead of all authenticated users.

### Fixed

- Closed leaked Windows file handles in `windows_api::get_final_path_name` and the
  junction detector by introducing a scoped-handle RAII guard.
- Added bounds checking to reparse-point parsing before pointer arithmetic.
- Removed an errant `cargo clean` that ran in the current working directory during
  Cargo cache cleanup.
- The scanner now reports an accurate `items_scanned` count, applies the
  `max_file_size` filter, and offloads synchronous directory-size walks to a
  blocking thread pool.
- **Docker "Prune System"** now prunes all four resource types (containers, images,
  volumes, networks) in a single background task. Previously, the `is_operation_running`
  guard caused only containers to be pruned while images, volumes, and networks were
  silently skipped.
- Installer `winsweep.nsi` corrected stale `PRODUCT_WEB_SITE` URL to
  `https://github.com/N1KH1LT0X1N/WinSweep`.
- `scripts/sign-build.ps1` removed erroneous `/f $CertificateThumbprint` signtool
  argument (expects a PFX file path, not a thumbprint); certificate selection now
  relies solely on `/sha1`.
- GitHub Actions workflows: replaced non-existent action versions (`checkout@v6` →
  `@v4`, `upload-artifact@v7` → `@v4`, `action-gh-release@v3` → `@v2`); added
  missing MinGW installation step to `release.yml` and the `docs` job so the
  `x86_64-pc-windows-gnu` default target can link correctly.

### Tests

- Workspace test suite expanded to **157 passing tests** (1 ignored), covering the
  new service start-type parsing, executable version detection, NuGet banner
  parsing, `items_scanned` reporting, and the `max_file_size` filter.

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
