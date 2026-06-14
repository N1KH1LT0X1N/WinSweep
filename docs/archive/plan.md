# WinSweep — Development Plan

## Project Overview

WinSweep is a safe, high-performance disk cleaning tool for Windows 10/11, built as a
Rust workspace with four crates:

| Crate | Role |
|---|---|
| `winsweep-common` | Shared types, config, NEVER_DELETE list |
| `winsweep-core` | Scanner, cleanup, Docker, WSL, services, audit logger |
| `winsweep-cli` | CLI binary (`winsweep-cli.exe`) |
| `winsweep-gui` | GUI binary (`winsweep-gui.exe`) using egui 0.29 |

Target triple: `x86_64-pc-windows-gnu`. Config: `%APPDATA%\WinSweep\config.toml`.

---

## Current State (as of this session)

### What works / is complete
- Full egui GUI with 8 navigation views (Dashboard, Scan, WSL, Docker, Package Managers, Windows Update, Services, Settings)
- System tray integration (`tray-icon 0.14.3`, feature-gated `system-tray`)
- Live dashboard: sysinfo every 5s (CPU/memory/disk), segmented disk gauge, category bar chart, recent activity
- Parallel async scanner with NEVER_DELETE safety, project recognition, junction/symlink handling
- Cleanup manager supporting recycle bin, dry-run, system restore points, audit logging
- WSL VHDX compaction via elevated coordinator
- Docker client (containers, images, volumes, networks)
- Package manager cache detection and cleanup (npm, pip, cargo, and more)
- Windows Update cleanup via DISM + elevated coordinator
- Windows Service management (start/stop/restart)
- TOML config with load/save/validate, settings UI, registry-based "start with Windows"
- CI/CD: `ci.yml` (test + clippy + fmt + security audit) and `release.yml`
- 48+ passing tests (1 ignored — IPC requires admin)

### Bugs confirmed this session

| # | Location | Bug |
|---|---|---|
| B1 | `cleanup.rs::move_to_recycle_bin` | Uses broken PowerShell `SendKeys` + `InvokeVerb` — shows a dialog per file and may do nothing silently |
| B2 | `viewmodel/mod.rs::start_cleanup_task` | Hardcodes `use_recycle_bin: false` — ignores `config.cleanup.use_recycle_bin` |
| B3 | `views/dashboard.rs` "Clean Temp Files" button | Only calls `set_status_message` — never triggers an actual operation |
| B4 | `views/scan.rs` + `viewmodel/scan.rs` | `categorize_path` duplicated in two files with identical logic |
| B5 | 5 view files | `format_bytes` / `fmt_bytes` defined independently in `dashboard.rs`, `scan.rs`, `docker.rs`, `package_managers.rs`, `windows_update.rs` |
| B6 | `app.rs` layout | `ui.horizontal(sidebar + content)` — sidebar has no fixed width, content area does not fill remaining space, no independent scrolling |
| B7 | `viewmodel/scan.rs::update()` | `scan_options.min_file_size` never applied — all scan results kept regardless of the UI filter value |

---

## Changes Completed This Session

- [x] **`views/utils.rs`** — NEW: shared `pub fn format_bytes(u64) -> String` + 6 unit tests
- [x] **`views/mod.rs`** — added `pub mod utils`
- [x] **`viewmodel/scan.rs`**
  - `categorize_path` made `pub fn` (canonical single source of truth)
  - `compute_category_breakdown` refactored to call `categorize_path` instead of duplicating logic
  - `min_file_size` filter applied in `update()` when draining scanner results
  - 10 new unit tests: `test_categorize_*`, `test_compute_breakdown_*`
- [x] **`views/scan.rs`**
  - Removed local `format_bytes` and `categorize_path` — imports from canonical locations
  - Added **Select All** / **Deselect All** buttons
  - Added **total selected size** display next to delete button
  - `Delete Selected` and `Delete All` now route through confirmation check:
    - If `config.cleanup.cleanup_confirm_delete` → `set_pending_cleanup()`
    - Else → `start_cleanup_task()` directly
- [x] **`viewmodel/mod.rs`**
  - `NavigationView::About` variant added
  - `PendingCleanup { items, description, total_size }` struct added
  - `pending_cleanup: Option<PendingCleanup>` field on `WinSweepViewModel`
  - `set_pending_cleanup()` method
  - `start_cleanup_task` reads `self.config.cleanup.use_recycle_bin` (was hardcoded `false`)
  - 4 unit tests for `should_auto_clean`: None, recent, old, invalid timestamp
- [x] **`app.rs`**
  - Layout replaced: `egui::SidePanel::left("nav_panel", 195px) + egui::CentralPanel + ScrollArea`
  - About nav item pinned to bottom of sidebar via `Layout::bottom_up`
  - Confirmation modal (`egui::Window`) rendered when `pending_cleanup.is_some()`
  - `NavigationView::About` arm dispatches to `show_about(ui)`

---

## Remaining Work

### P1 — Must fix before build passes

- [ ] **`app.rs`** — Remove unused `about_open: bool` field from `WinSweepApp` struct; add `fn show_about(ui: &mut egui::Ui)` function at bottom of file
- [ ] **`views/dashboard.rs`**
  - Replace `fn fmt_bytes` with `use crate::views::utils::format_bytes`
  - Fix "Clean Temp Files" button: call `viewmodel.start_elevated_task(ElevatedOperation::CleanSystemTemp {...}, ...)` instead of `set_status_message`
  - Add **Total Reclaimable KPI**: a prominent `"{X} reclaimable"` headline above the category chart
- [ ] **`views/docker.rs`** — Remove local `fn format_bytes`; import from `crate::views::utils`
- [ ] **`views/package_managers.rs`** — Remove local `fn format_bytes`; import from `crate::views::utils`
- [ ] **`views/windows_update.rs`** — Remove local `fn format_bytes`; import from `crate::views::utils`
- [ ] **`crates/winsweep-core/Cargo.toml`** — Add `windows-sys = { version = "0.59", features = ["Win32_Foundation", "Win32_UI_Shell"] }`
- [ ] **`crates/winsweep-core/src/cleanup.rs`** — Replace `move_to_recycle_bin` PowerShell implementation with `SHFileOperationW` (flags: `FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT`, double-null-terminated wide path, `tokio::task::spawn_blocking`)

### P2 — Polish & completeness

- [ ] Verify `cargo test --workspace --all-features` passes with new tests (target: 60+ tests)
- [ ] Verify `cargo build -p winsweep-gui` and `cargo build -p winsweep-gui --features system-tray` compile warning-free
- [ ] Verify `cargo clippy --all-targets --all-features -- -D warnings` clean
- [ ] Persist `dashboard.recent_operations` to disk (currently lost on restart)
- [ ] Wire scan results into `dashboard.category_breakdown` when scan completes (already done in `viewmodel/mod.rs` via `pending_category_breakdown`; verify it flows through)
- [ ] Multi-drive disk info on dashboard (currently shows only primary drive)
- [ ] Windows toast notifications via native API instead of PowerShell spawn

### P3 — Future enhancements

- [ ] Browser cache cleanup (Chrome, Edge, Firefox `%LOCALAPPDATA%` caches)
- [ ] Scheduled cleanup with configurable interval (config skeleton exists: `auto_cleanup_enabled`, `auto_cleanup_days`)
- [ ] Installer polish: `installer/winsweep.nsi` exists — wire into `release.yml`
- [ ] `docs/` directory content (User Guide, Developer Guide)
- [ ] README placeholder images (`via.placeholder.com`) → real screenshots

---

## Key File Map

```
c:\Dev\WinSweep\
├── Cargo.toml                          workspace root
├── crates/
│   ├── winsweep-common/src/
│   │   ├── config.rs                   Config struct, load/save/validate
│   │   ├── never_delete.rs             NEVER_DELETE_PATHS + pattern list
│   │   └── types.rs                    ScanResult, CleanupResult, ScanConfig, IpcMessage
│   ├── winsweep-core/src/
│   │   ├── scanner.rs                  Parallel scanner, ScannerHandle
│   │   ├── cleanup.rs                  CleanupManager, move_to_recycle_bin ← BUG B1
│   │   ├── audit_logger.rs             Append-only audit log
│   │   ├── docker.rs                   DockerClient (HTTP to socket)
│   │   ├── wsl_detector.rs             WslDetector, VHDX path, state
│   │   ├── service_manager.rs          ServiceManager (windows-sys SC API)
│   │   └── package_managers/           PackageManagerRegistry + per-manager cleaners
│   ├── winsweep-gui/src/
│   │   ├── main.rs                     eframe entry point, tokio runtime init
│   │   ├── app.rs                      WinSweepApp, layout, tray events, confirmation modal
│   │   ├── tray.rs                     TrayManager (tray-icon 0.14.3)
│   │   ├── notifications.rs            show_toast via PowerShell
│   │   ├── elevated_coordinator.rs     ElevatedCoordinator, ElevatedOperation enum
│   │   ├── util.rs                     recycle_bin_size / empty_recycle_bin (windows-sys)
│   │   ├── viewmodel/
│   │   │   ├── mod.rs                  WinSweepViewModel, NavigationView, PendingCleanup
│   │   │   ├── dashboard.rs            SystemInfo, CategoryBreakdown, recent_operations
│   │   │   ├── scan.rs                 ScanViewModel, categorize_path (canonical), tests
│   │   │   ├── settings.rs             SettingsViewModel, registry "start with Windows"
│   │   │   ├── wsl.rs / docker.rs / package_managers.rs / services.rs / windows_update.rs
│   │   └── views/
│   │       ├── utils.rs                format_bytes (canonical), tests  ← NEW
│   │       ├── dashboard.rs            ← needs fmt_bytes fix + KPI + Clean Temp fix
│   │       ├── scan.rs                 Select All, total selected, confirmation-aware delete
│   │       ├── docker.rs               ← needs format_bytes fix
│   │       ├── package_managers.rs     ← needs format_bytes fix
│   │       └── windows_update.rs       ← needs format_bytes fix
│   └── winsweep-cli/src/
│       ├── main.rs                     tokio entry, subcommand dispatch
│       └── app.rs                      clap: scan/clean/config/wsl/docker/services/pkg-mgrs
├── tests/integration_tests.rs          48 integration tests
└── .github/workflows/
    ├── ci.yml                          test + clippy + fmt + security audit + docs
    └── release.yml                     build release binaries + package ZIP
```

---

## Architecture Invariants

- **Views never hold state** — all state lives in `viewmodel/`
- **Viewmodel never imports views** — only views import viewmodel (no circular deps)
- **`categorize_path`** is canonical in `viewmodel/scan.rs`; views import it from there
- **`format_bytes`** is canonical in `views/utils.rs`; all views import it from there
- **Destructive operations** check `config.cleanup.cleanup_confirm_delete` before executing; if true they call `set_pending_cleanup()` and the confirmation modal in `app.rs` handles the actual dispatch
- **Elevated operations** always go through `ElevatedCoordinator` — never directly spawned from views
- **NEVER_DELETE** checks happen in both `scanner.rs` and `cleanup.rs` — belt and suspenders

---

## Build Commands

```powershell
# Run all tests
cargo test --workspace --all-features

# Build GUI (no tray)
cargo build -p winsweep-gui

# Build GUI with system tray
cargo build -p winsweep-gui --features system-tray

# Clippy (CI-strict)
cargo clippy --all-targets --all-features -- -D warnings

# Format check
cargo fmt --all -- --check
```
