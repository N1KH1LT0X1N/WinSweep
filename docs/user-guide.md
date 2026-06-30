# WinSweep User Guide

## Table of Contents

1. [Installation](#installation)
2. [First Launch](#first-launch)
3. [Dashboard](#dashboard)
4. [System Scan](#system-scan)
5. [WSL Management](#wsl-management)
6. [Docker Cleanup](#docker-cleanup)
7. [Package Manager Caches](#package-manager-caches)
8. [Windows Update Cache](#windows-update-cache)
9. [Windows Services](#windows-services)
10. [Settings](#settings)
11. [System Tray](#system-tray)
12. [Command-Line Interface](#command-line-interface)
13. [Safety](#safety)
14. [Troubleshooting](#troubleshooting)

---

## Installation

### From Release (Recommended)

1. Download the latest `winsweep-<version>-setup.exe` from
   [GitHub Releases](https://github.com/N1KH1LT0X1N/WinSweep/releases).
2. Run the installer. It will place `winsweep-gui.exe` and `winsweep-cli.exe`
   in `%ProgramFiles%\WinSweep` and create Start Menu shortcuts.
3. Launch **WinSweep** from the Start Menu.

### From ZIP

Download the portable ZIP, extract it anywhere, and run `winsweep-gui.exe`
directly — no installation required.

### From Source

```powershell
git clone https://github.com/N1KH1LT0X1N/WinSweep.git
cd winsweep
cargo build --release -p winsweep-gui
cargo build --release -p winsweep-cli
```

---

## First Launch

On first launch WinSweep will:

- Create a default configuration file at `%AppData%\WinSweep\config.toml`.
- Populate the Dashboard with live system statistics (disk, memory, CPU).
- NOT delete or modify any files automatically.

---

## Dashboard

The Dashboard is the home screen. It shows:

| Section | Description |
|---|---|
| **System Overview** | OS version, RAM usage, CPU usage |
| **Quick Actions** | One-click buttons for the most common cleanups |
| **Drives** | All mounted volumes with colour-coded capacity bars |
| **Storage Gauge** | Primary-drive bar overlaid with reclaimable space (red) |
| **Reclaimable by Category** | Bar chart: Artifacts / Temp / Package Cache / Recycle Bin / Other |
| **Recent Activity** | Last 50 cleanup operations with timestamps and bytes freed |
| **System Health** | Traffic-light indicators for memory, CPU, and disk saturation |

### Quick Actions

| Button | Action |
|---|---|
| 🔍 Quick Scan | Navigate to the Scan view |
| 🧹 Clean Temp Files | Elevated cleanup of `%TEMP%` and `C:\Windows\Temp` |
| 📦 Clean Package Caches | Navigate to the Package Managers view |
| ♻️ Empty Recycle Bin | Immediately empty the Recycle Bin |
| 🌐 Clean Browser Caches | Delete Chrome, Edge, and Firefox disk caches |

### Drive Bars

Each drive shows:
- Mount point (e.g. `C:\`) and volume name
- File system (NTFS, exFAT, …)
- Free / Total size
- A capacity bar that turns **amber** above 80 % used and **red** above 95 %.

---

## System Scan

The Scan view lets you scan any folder and inspect what WinSweep finds.

### Starting a Scan

1. Enter or browse to a **Scan Location**.
2. Optionally enable **Include hidden files** or **Include system files**.
3. Set a **Min file size** (files smaller than this are ignored).
4. Click **🔍 Start Scan**.

### Working with Results

- Results appear in a sortable table. Click any column header to sort; click
  again to reverse.
- **Select / Deselect All** checkboxes for bulk operations.
- **Delete Selected** — removes only the checked rows.
- **Delete All** — removes every result in one operation.
- **Export CSV** — saves the full result list to a `.csv` file.

> Results that require confirmation (see Settings → Cleanup → "Confirm before
> deleting") show a confirmation dialog before any deletion occurs.

### Category Breakdown

After a scan completes, a collapsible breakdown shows how much space each
category occupies:

| Category | Typical contents |
|---|---|
| **Artifacts** | `node_modules`, `target`, `.gradle`, `build`, `__pycache__` |
| **Temp** | `%TEMP%`, `C:\Windows\Temp`, `Prefetch`, `INetCache` |
| **Package Cache** | `.npm`, `.pnpm-store`, NuGet, pip, Go module cache |
| **Recycle Bin** | `$Recycle.Bin` entries |
| **Other** | Everything else |

---

## WSL Management

Requires WSL 2 installed. The WSL view shows all registered Linux distributions.

### Actions

| Button | Effect |
|---|---|
| 🔄 Refresh | Re-query WSL for the current distribution list |
| ▶ Start | Start a stopped distribution |
| ⏹ Stop | Terminate a running distribution |
| 🗑️ Unregister | Permanently remove a distribution (data is lost) |
| 🗜️ Compact Disk | Compact the distribution's `.vhdx` file via `wsl --compact` |
| 📁 Open in Explorer | Open the distribution root in Windows Explorer |

> **Compact Disk** runs with elevated privileges via the ElevatedCoordinator.
> It temporarily stops the distribution, compacts the VHDX, then restarts it.

---

## Docker Cleanup

Requires Docker Desktop or Docker Engine running. The Docker view shows:

- **Daemon status** (running / stopped) and version
- **Containers** — list with status; buttons to Stop / Remove
- **Images** — list with size; button to Remove
- **Volumes** — list with size
- **Networks** — list

### Cleanup Buttons

| Button | Effect |
|---|---|
| 🗑️ Clean All | Remove stopped containers, dangling images, unused volumes/networks |
| 🧹 Prune System | `docker system prune -a --volumes` — reclaims maximum space |

---

## Package Manager Caches

WinSweep detects and cleans caches for 25+ package managers including browsers:

**Development tools:** npm, pnpm, yarn, pip, poetry, cargo, go, NuGet, Gradle,
Maven, Flutter/pub, Bun, Pixi, Composer, vcpkg, Conan, sbt, Go build cache,
Android SDK, Git LFS, Playwright, Cypress

**Browsers:** Google Chrome, Microsoft Edge, Mozilla Firefox

### Workflow

1. Click **🔄 Refresh** to detect installed managers and measure cache sizes.
2. Select a manager in the list to see its cache paths and size.
3. Click **Clean Selected** or **Clean All**.

> WinSweep uses the package manager's own clean command where available
> (e.g. `npm cache clean --force`, `pip cache purge`) so the tool correctly
> handles locked files and re-populates essential metadata.

---

## Windows Update Cache

The Windows Update view shows:

- **Service status** — whether Windows Update service is running
- **Last check** — when Windows last checked for updates
- **Pending updates** — list with category and severity

### Cleanup Options

| Checkbox | Files removed |
|---|---|
| Remove downloaded files | `C:\Windows\SoftwareDistribution\Download\*` |
| Compress backups | Runs `dism /online /cleanup-image /startcomponentcleanup` |
| Remove old versions | Runs `dism /online /cleanup-image /startcomponentcleanup /resetbase` |

> These operations require administrator privileges and are performed via the
> ElevatedCoordinator.

---

## Windows Services

The Services view lists all Windows services with filtering and management.

- **Search** — type to filter by name or description.
- **Running only** — show only active services.
- **▶ Start / ⏹ Stop / 🔄 Restart** — per-service controls.
- **⚙️ Properties** — shows start type and capability flags.

> Service management requires administrator privileges.

---

## Settings

Settings are split into five categories:

### General
- **Start with Windows** — writes a Run registry key for auto-start.
- **Minimize to tray** — closing the window hides it to the system tray.
- **Language / Theme** — UI customisation.

### Scan
- Hidden / system file inclusion.
- Default scan locations.
- Minimum file size filter.

### Cleanup
- **Confirm before deleting** — shows a modal before any destructive operation.
- **Move to Recycle Bin** — soft-delete; files can be recovered.
- **Automatic cleanup** — enable and configure cadence (days).
- **Windows Task Scheduler** — register a startup task so auto-cleanup runs
  even when the GUI is closed. Click **Register Startup Task** to create it,
  **Remove Task** to unregister.
- **What to clean** — checkboxes for temp files, Recycle Bin, Prefetch,
  browser cache.

### Notifications
- Enable / disable toast notifications for cleanup completion and low disk space.
- Low disk space threshold (1–50 %).
- Notification display duration.

### Advanced
- Debug / verbose logging.
- Max concurrent operations.
- Export / Import settings (`.toml`).
- **Reset All Settings** / **Clear All Data** — danger zone operations.

---

## System Tray

When the **system-tray** feature is compiled in, WinSweep minimises to the
system tray when the window is closed (if "Minimize to tray" is enabled).

Right-click the tray icon to access:

| Menu item | Action |
|---|---|
| Show WinSweep | Restore the main window |
| Quick Scan | Start a scan on the default path |
| Clean Temp Files | Immediate temp-file cleanup |
| Clean All | Full auto-cleanup run |
| Settings | Open the Settings view |
| About | Show version and licence info |
| Quit | Exit WinSweep |

---

## Command-Line Interface

`winsweep-cli` provides a fully scriptable interface.

```
Usage: winsweep [OPTIONS] [PATH]...

Arguments:
  [PATH]...  Paths to scan (default: current directory)

Options:
  -v, --verbose              Enable verbose logging
  -l, --log-file <LOG_FILE>  Log file path
      --mode <MODE>          Start mode [scan|wsl|docker|update|services|config]
      --older <DAYS>         Only report artifacts older than N days
      --output <FORMAT>      Output format [text|ndjson]
      --dry-run              Show what would be deleted without deleting
  -h, --help                 Print help
  -V, --version              Print version
```

### Examples

```powershell
# Interactive TUI scan
winsweep-cli C:\Users\$env:USERNAME

# Stream JSON lines — pipe into jq, PowerShell, etc.
winsweep-cli --output ndjson C:\ | ConvertFrom-Json | Where-Object { $_.size_bytes -gt 1GB }

# Find project artifacts older than 90 days
winsweep-cli --output ndjson --older 90 C:\Dev

# Dry run to preview what would be deleted
winsweep-cli --dry-run C:\Temp
```

---

## Safety

WinSweep has multiple layers of protection against accidental data loss:

1. **NEVER_DELETE list** — a hardcoded list of paths that can never be deleted,
   including `C:\Windows`, `C:\Program Files`, `C:\Users`, and the WinSweep
   binary itself.  This check is performed at two independent code points.

2. **Junction / symlink detection** — reparse points are never followed,
   preventing traversal into unexpected locations.

3. **Confirmation dialogs** — destructive operations show a modal listing the
   files to be deleted and total size, unless "Confirm before deleting" is
   disabled in Settings.

4. **Recycle Bin integration** — by default, files go to the Recycle Bin using
   the native `SHFileOperationW` API, allowing recovery via the shell.

5. **Audit log** — every operation (scan start/end, files deleted) is written to
   `%LocalAppData%\WinSweep\logs\winsweep.log` with timestamps and SHA-256
   hashes of deleted files.

6. **Dry-run mode** — pass `--dry-run` to the CLI to preview deletions without
   executing them.

7. **System Restore Point** — the Settings allow automatic restore-point creation
   before bulk cleanup operations.

---

## Troubleshooting

### "The operation requires elevation"

Some operations (clean system temp, Windows Update cache, service management)
need administrator privileges. WinSweep uses an ElevatedCoordinator that
launches a privileged helper process; you may see a UAC prompt.

### Scan finds nothing

- Check the **Min file size** setting — the default (1 KB) filters very small
  files.
- Try enabling **Include hidden files** and **Include system files**.
- Verify the scan path exists and is readable.

### Browser cache sizes show 0

The browser must have created its profile directory at least once. Make sure the
browser has been launched before running WinSweep.

### Docker view shows "Daemon not running"

Start Docker Desktop (or `dockerd`) before using the Docker view.

### Scheduled task doesn't run

- Check Task Scheduler (`taskschd.msc`) for the **WinSweep Auto Cleanup** task.
- Ensure the binary path in the task action is correct (it is set to the current
  exe at registration time).
- The task runs **ONLOGON** with a 1-minute delay; log off and back on to test.

### Notifications don't appear

WinSweep uses the Windows.UI.Notifications WinRT API via a hidden PowerShell
process. Make sure:
- PowerShell is not blocked by your system policy.
- Focus Assist / Do Not Disturb is not enabled in Windows Settings.
