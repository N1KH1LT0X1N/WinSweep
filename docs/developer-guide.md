# WinSweep Developer Guide

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Workspace Layout](#workspace-layout)
3. [Building](#building)
4. [Testing](#testing)
5. [Core Crate Deep Dive](#core-crate-deep-dive)
6. [GUI Crate Deep Dive](#gui-crate-deep-dive)
7. [Adding a Package Manager](#adding-a-package-manager)
8. [Adding a New View](#adding-a-new-view)
9. [Elevated Operations](#elevated-operations)
10. [Configuration System](#configuration-system)
11. [CI/CD Pipeline](#cicd-pipeline)
12. [Release Process](#release-process)
13. [Contributing](#contributing)
14. [Code Style](#code-style)

---

## Architecture Overview

```
winsweep-common   ──► shared types, Config, ScanConfig, NEVER_DELETE
       │
winsweep-core     ──► scanner, cleanup, all detectors, IPC, audit log
       │
winsweep-cli      ──► TUI (ratatui) + ndjson streaming mode
winsweep-gui      ──► egui desktop GUI, system tray (feature-gated)
```

### Key Design Decisions

| Decision | Rationale |
|---|---|
| Tokio async runtime leaked as `'static` | eframe's `update()` is sync; we `block_on` short async operations inside it. |
| ElevatedCoordinator + IPC | Privileged operations run in a child process; avoids requiring the whole app to be elevated. |
| ViewModel pattern | Each view owns its data; views are pure rendering functions that read/write the VM. |
| `#[serde(skip)]` for runtime fields | eframe's persistence serialises the VM to disk; runtime handles must be excluded. |
| NEVER_DELETE checked in two places | Scanner and CleanupManager both enforce it; defence-in-depth. |
| sysinfo polled every 5 s | Frequent enough for live indicators without hammering the OS. |

---

## Workspace Layout

```
WinSweep/
├── Cargo.toml                  workspace manifest + shared deps
├── .cargo/config.toml          target = x86_64-pc-windows-gnu
├── crates/
│   ├── winsweep-common/
│   │   └── src/
│   │       ├── config.rs       Config struct + load/save
│   │       ├── types.rs        ScanResult, ScanConfig, …
│   │       └── never_delete.rs NEVER_DELETE path set
│   ├── winsweep-core/
│   │   ├── src/
│   │   │   ├── scanner.rs      parallel file walker + result streaming
│   │   │   ├── cleanup.rs      CleanupManager (SHFileOperationW / direct delete)
│   │   │   ├── audit_logger.rs structured log of every operation
│   │   │   ├── package_manager/
│   │   │   │   ├── mod.rs      PackageManager trait + registry
│   │   │   │   ├── browser.rs  Chrome / Edge / Firefox cache
│   │   │   │   ├── npm.rs
│   │   │   │   └── … (25 total)
│   │   │   ├── docker.rs       Docker SDK wrapper
│   │   │   ├── wsl_detector.rs sysinfo + wsl.exe parsing
│   │   │   ├── service_manager.rs Windows SCM wrapper
│   │   │   ├── ipc.rs          named-pipe IPC for elevated helper
│   │   │   ├── elevated_coordinator.rs operation enum + result types
│   │   │   └── …
│   │   └── tests/
│   │       └── scanner_tests.rs
│   ├── winsweep-cli/
│   │   └── src/
│   │       └── main.rs         clap CLI, ratatui TUI, ndjson mode
│   └── winsweep-gui/
│       └── src/
│           ├── main.rs         eframe entry point
│           ├── app.rs          WinSweepApp (eframe::App impl), tray events
│           ├── viewmodel/
│           │   ├── mod.rs      WinSweepViewModel + background tasks
│           │   ├── scan.rs     ScanViewModel, categorize_path
│           │   ├── dashboard.rs DashboardViewModel, DriveInfo, sysinfo polling
│           │   ├── wsl.rs
│           │   ├── docker.rs
│           │   ├── package_managers.rs
│           │   ├── windows_update.rs
│           │   ├── services.rs
│           │   └── settings.rs
│           ├── views/
│           │   ├── mod.rs      re-exports all view functions
│           │   ├── dashboard.rs
│           │   ├── scan.rs
│           │   ├── wsl.rs
│           │   ├── docker.rs
│           │   ├── package_managers.rs
│           │   ├── windows_update.rs
│           │   ├── services.rs
│           │   ├── settings.rs
│           │   └── utils.rs    format_bytes (canonical)
│           ├── elevated_coordinator.rs IPC client + ElevatedOperation enum
│           ├── notifications.rs toast via hidden PowerShell
│           ├── scheduler.rs    schtasks.exe wrapper
│           ├── tray.rs         tray-icon 0.14 integration (system-tray feature)
│           └── util.rs         SHQueryRecycleBinW / SHEmptyRecycleBinW
├── tests/
│   └── integration_tests.rs   workspace-level integration tests
├── installer/
│   └── winsweep.nsi            NSIS installer script
├── docs/
│   ├── user-guide.md
│   ├── developer-guide.md      (this file)
│   ├── faq.md
│   └── api-reference.md
└── .github/
    └── workflows/
        ├── ci.yml
        └── release.yml
```

---

## Building

### Prerequisites

- Rust 1.75+ (edition 2021)
- Target: `x86_64-pc-windows-gnu` — install with:
  ```powershell
  rustup target add x86_64-pc-windows-gnu
  ```
- MinGW-w64 toolchain (for the linker)
- Windows 10/11 (tests rely on Windows APIs)

### Commands

```powershell
# Debug build (both crates)
cargo build --workspace

# GUI only (without system tray)
cargo build -p winsweep-gui

# GUI with system tray icon
cargo build -p winsweep-gui --features system-tray

# Release builds
cargo build --release -p winsweep-gui --features system-tray
cargo build --release -p winsweep-cli

# Run clippy (treat warnings as errors in CI)
cargo clippy --all-targets --all-features -- -D warnings

# Format check
cargo fmt --all -- --check
```

---

## Testing

```powershell
# All workspace tests
cargo test --workspace

# Specific crate
cargo test -p winsweep-core
cargo test -p winsweep-gui

# Integration tests only
cargo test --test integration_tests

# Single test
cargo test test_browser_cache_paths_detection
```

### Test Counts (baseline)

| Crate / suite | Tests |
|---|---|
| `winsweep-core` unit | 31 |
| `winsweep-core` scanner_tests | 7 |
| `winsweep-gui` unit | 27 |
| `integration_tests` | 12 |
| **Total** | **77** |

### Writing Tests

- **Unit tests** live in `#[cfg(test)] mod tests { … }` at the bottom of each
  source file.
- **Integration tests** go in `tests/integration_tests.rs` at the workspace root.
- Use `tempfile::TempDir` for any test that touches the filesystem.
- Use `#[ignore]` for tests that require admin rights or external services
  (Docker, WSL).
- Async tests use `#[tokio::test]`.

---

## Core Crate Deep Dive

### Scanner

`winsweep_core::Scanner` walks a directory tree in parallel using a configurable
thread pool (`num_cpus` threads by default).  Results are streamed through a
`tokio::sync::mpsc` channel.

Key types:
- `ScannerHandle` — returned by `Scanner::start()`; call `.next().await` in a
  loop to receive `CommonScanResult` items.
- `ScanConfig` (from `winsweep-common`) — configures include/exclude patterns,
  min size, older-than filter, follow-symlinks toggle.

### CleanupManager

`CleanupManager::delete_batch(&paths, use_recycle_bin)`:

1. Checks every path against `NEVER_DELETE`.
2. If `use_recycle_bin` → calls `SHFileOperationW` with `FO_DELETE` and
   `FOF_ALLOWUNDO | FOF_NO_UI | FOF_NOCONFIRMATION`.
3. Otherwise → `std::fs::remove_file` / `remove_dir_all`.
4. Records each deletion in the `AuditLogger`.

### ElevatedCoordinator (core)

`ElevatedOperation` and `ElevatedOperationResult` are the wire types serialised
over a named pipe between the GUI and the elevated helper process.

The helper is the same binary with a special env-var flag: when
`WINSWEEP_ELEVATED_MODE=1` the binary runs the pipe server instead of the GUI.

### PackageManager Trait

```rust
#[async_trait]
pub trait PackageManager: Send + Sync {
    fn name(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    async fn is_installed(&self) -> bool;
    async fn get_version(&self) -> Result<Option<String>>;
    async fn get_cache_paths(&self) -> Result<Vec<PathBuf>>;
    async fn calculate_cache_size(&self) -> Result<u64>;
    async fn clean_all_caches(&self) -> Result<PackageCleanResult>;
    async fn clean_paths(&self, paths: &[PathBuf]) -> Result<PackageCleanResult>;
    async fn get_cache_info(&self) -> Result<Vec<CacheInfo>>;
}
```

Implementations live in `crates/winsweep-core/src/package_manager/<name>.rs`.

---

## GUI Crate Deep Dive

### Event Loop

```
eframe::run_native()
  └─ WinSweepApp::update() [called every frame ~60 fps]
       ├─ poll tray events  (if system-tray feature)
       ├─ viewmodel.update()
       │    ├─ dashboard.update()     (sysinfo every 5 s)
       │    ├─ auto-cleanup check
       │    ├─ low-disk notification
       │    ├─ scan.update()
       │    ├─ poll background_handle (JoinHandle<Result<BackgroundResult>>)
       │    └─ … sub-viewmodel updates
       ├─ show_side_panel()   navigation rail
       └─ show_central_panel()
            └─ match current_view { Dashboard → show_dashboard(), … }
```

### Background Tasks

Long-running async work is offloaded to a single `background_handle:
Option<JoinHandle<Result<BackgroundResult>>>`. Only one task runs at a time.

```rust
pub enum BackgroundResult {
    Cleanup(CleanupResult),
    Elevated(ElevatedOperationResult),
    DockerRefresh(…),
    PackageManagerClean(Result<PackageCleanResult, String>),
    ServiceAction(Result<String, String>),
    …
}
```

When the task finishes, `poll_background_handle()` in `viewmodel/mod.rs`
dispatches to the appropriate handler.

### ViewModel Persistence

`WinSweepApp::save()` calls:
```rust
eframe::set_value(storage, eframe::APP_KEY, &self.viewmodel);
```

`WinSweepViewModel` derives `Serialize`/`Deserialize`.  Fields that must not be
persisted are annotated `#[serde(skip)]`:

- `runtime: Option<&'static Runtime>`
- `background_handle`
- `wsl_detector`, `docker_client`, `windows_detector`, `home_edition_compat`
- `package_manager_registry`
- `elevated_coordinator`
- Sysinfo `sys: System`
- UI timing fields (`last_refresh: Option<Instant>`)

---

## Adding a Package Manager

1. Create `crates/winsweep-core/src/package_manager/<name>.rs`.
2. Implement the `PackageManager` trait (all methods are `async`).
3. Include unit tests with `#[tokio::test]` inside `#[cfg(test)] mod tests`.
4. Add `pub mod <name>;` to the module list at the bottom of
   `crates/winsweep-core/src/package_manager.rs`.
5. Register in `PackageManagerRegistry::new()`:
   ```rust
   if let Ok(manager) = crate::package_manager::<name>::<Name>Manager::new().await {
       managers.push(Box::new(manager));
   }
   ```
6. The Package Managers view picks it up automatically.

---

## Adding a New View

1. Create `crates/winsweep-gui/src/views/<name>.rs`.
   - Export a `pub fn show_<name>(ui: &mut egui::Ui, viewmodel: &mut WinSweepViewModel)`.
2. Create `crates/winsweep-gui/src/viewmodel/<name>.rs` if needed.
   - Derive `Serialize, Deserialize` and add `#[serde(skip)]` for runtime fields.
3. Add `pub mod <name>;` to:
   - `views/mod.rs`
   - `viewmodel/mod.rs` (if you added a VM file)
4. Add a `NavigationView::<Name>` variant to the enum in `viewmodel/mod.rs`.
5. Wire it into `app.rs`:
   - Add a nav button in `show_side_panel()`.
   - Add a `NavigationView::<Name> => views::show_<name>(ui, &mut self.viewmodel)` arm.

---

## Elevated Operations

To add a new privileged operation:

1. Add a variant to `ElevatedOperation` in
   `crates/winsweep-gui/src/elevated_coordinator.rs`.
2. Add the corresponding `ElevatedOperationResult` fields if needed.
3. Handle the variant in the elevated helper's dispatch match arm
   (`ElevatedCoordinator::dispatch_operation()`).
4. Call `viewmodel.start_elevated_task(ElevatedOperation::<NewVariant> { … }, desc)`.

The IPC layer (`winsweep_core::ipc`) serialises/deserialises over a named pipe
using `serde_json`.

---

## Configuration System

`winsweep_common::Config` is the top-level config type:

```
Config
├── ScanConfig
│   ├── default_paths: Vec<String>
│   ├── include_hidden: bool
│   ├── min_file_size: u64
│   └── …
├── CleanupConfig
│   ├── clean_temp_files: bool
│   ├── clean_recycle_bin: bool
│   ├── clean_prefetch: bool
│   ├── clean_browser_cache: bool
│   ├── use_recycle_bin: bool
│   └── confirm_before_delete: bool
├── UiConfig
│   ├── show_notifications: bool
│   ├── minimize_to_tray: bool
│   └── …
└── LoggingConfig
    ├── log_level: String
    └── log_file: Option<String>

Config (top-level)
    ├── auto_cleanup_enabled: bool
    ├── auto_cleanup_days: u32
    ├── notify_low_disk_space: bool
    ├── low_disk_threshold: u8
    └── notify_cleanup_complete: bool
```

Config is stored at `%AppData%\WinSweep\config.toml` and loaded on startup.
It is also persisted via eframe's persistence layer (local storage JSON).

---

## CI/CD Pipeline

`.github/workflows/ci.yml` runs on every push and PR:

| Job | Steps |
|---|---|
| **test** | fmt check → clippy → unit tests → integration tests |
| **build** | Release build of both binaries, creates ZIP artefact |
| **security-audit** | `cargo audit` + `cargo outdated` |
| **docs** | `cargo doc`, deploy to GitHub Pages |

`.github/workflows/release.yml` triggers on `v*` tags:

1. Run tests
2. Build release binaries (`--features system-tray` for GUI)
3. Package into ZIP
4. Build NSIS installer (`makensis`)
5. Calculate SHA256 checksums
6. Create GitHub Release with all artefacts

---

## Release Process

```powershell
# 1. Bump version in workspace Cargo.toml
# 2. Update CHANGELOG.md
# 3. Commit and tag
git add -A
git commit -m "Release v0.2.0"
git tag -a v0.2.0 -m "Release v0.2.0"
git push origin main --tags
# The release workflow kicks off automatically
```

---

## Contributing

See [CONTRIBUTING.md](../CONTRIBUTING.md) for the full contribution guide.

Quick checklist:
- [ ] `cargo fmt --all` — no formatting diffs
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` — no warnings
- [ ] All existing tests pass: `cargo test --workspace`
- [ ] New code has tests
- [ ] NEVER add paths to `NEVER_DELETE` without a strong justification
- [ ] Destructive operations go through `CleanupManager` (not `fs::remove_file` directly)

---

## Code Style

- **Error handling**: use `anyhow::Result` in binary/GUI code, `thiserror` in
  library code.
- **Logging**: `tracing::{debug, info, warn, error}` — never `println!` in
  library code.
- **Async**: prefer `async fn` + `.await`; use `tokio::spawn` for CPU-heavy work.
- **Clippy**: CI enforces `-D warnings`; fix all lints, including `clippy::pedantic`
  if activated.
- **No unsafe without justification**: the only `unsafe` blocks are for
  Windows API calls that have no safe wrapper.
- **Doc comments** on every public item in library crates.
