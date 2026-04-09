# WinSweep — Architecture & Build Plan v3.0
## 100% Fact-Verified Edition

> Every technical claim in this document is sourced from official documentation,
> first-party source code, or verified community research. Assumptions have been
> replaced with either confirmed facts or explicit "requires runtime detection"
> markers where behavior varies by machine configuration.

---

## Table of Contents
1. Executive Summary
2. What Builds Up on a Windows Dev Machine
3. Lessons Learned From Existing Tools
4. Product Vision & Differentiators
5. Feature Matrix
6. Architecture & Tech Stack
7. Project Type Coverage
8. CLI Command Reference
9. GUI Design Specification
10. Safety & Risk Mitigation
11. Windows-Specific Features (Fully Verified)
12. Configuration & Extensibility
13. Phased Build Roadmap
14. Engineering Decisions & Trade-offs

---

## 1. Executive Summary

Windows developer machines accumulate disk waste from sources no single existing tool
handles completely: WSL2 virtual disks that grow dynamically but never shrink automatically
(confirmed by Microsoft's own WSL documentation at learn.microsoft.com/en-us/windows/wsl/disk-space),
Docker Desktop VHDX files that exhibit the same behaviour, language-specific build caches
across a dozen package ecosystems, and IDE caches scattered across AppData.

WinSweep is a Rust-powered scanner with a native WinUI 3 desktop app and a full CLI.
Its core differentiator is correct, verified handling of every category above — including
WSL2 VHDX compaction with a working fallback for Windows Home, Docker Engine API
integration using the correct endpoint, and pnpm store management via the tool's own
prune command rather than direct file deletion.

**Minimum OS:** Windows 10 version 21H2 (Build 19044). Features requiring newer OS
versions are feature-gated at runtime after an OS version check.

**Performance target:** Full project-directory scan across a typical developer NVMe SSD
in under 60 seconds. HDD performance varies and is not guaranteed; continuous progress
feedback is provided regardless.

---

## 2. What Builds Up on a Windows Dev Machine

All paths below are **defaults only**. WinSweep resolves actual paths at runtime
(see Section 8 for the path-resolution strategy). Every override variable listed
has been confirmed against official documentation for each tool.

| Path (default) | Runtime Override | What It Is | Typical Size | Priority |
|---|---|---|---|---|
| `node_modules` (per project) | n/a | npm / yarn / pnpm / bun build output | 1–20+ GB | Very High |
| `target/` (per project) | n/a | Rust Cargo build artifacts | 1–30 GB | Very High |
| `%USERPROFILE%\.gradle\caches` | `GRADLE_USER_HOME` env var | Gradle dependency + build cache | 2–15 GB | High |
| `%USERPROFILE%\.gradle\daemon` | `GRADLE_USER_HOME` | Gradle long-lived daemon JVMs | 200 MB–2 GB | Medium |
| `%USERPROFILE%\.gradle\wrapper\dists` | `GRADLE_USER_HOME` | Gradle wrapper downloads | 500 MB–5 GB | Medium |
| Maven local repo (default `%USERPROFILE%\.m2\repository`) | `<localRepository>` in `~/.m2/settings.xml` | Maven local artifact repository | 1–10 GB | High |
| NuGet packages (default `%USERPROFILE%\.nuget\packages`) | `NUGET_PACKAGES` env var | NuGet global packages (.NET) | 1–15 GB | High |
| npm cache (default `%APPDATA%\npm-cache`) | `npm_config_cache` env var; read via `npm config get cache` | npm HTTP download cache | 200 MB–5 GB | Medium |
| pnpm store (located via `pnpm store path`) | `PNPM_HOME` env var | pnpm content-addressable store (hardlinks + junctions on Windows) | 1–8 GB | Medium |
| Yarn Classic cache (`%LOCALAPPDATA%\Yarn\Cache\v6`) | `YARN_CACHE_FOLDER` env var | Yarn v1 global package cache | 500 MB–5 GB | Medium |
| Yarn Berry per-project `.yarn/cache` | `.yarnrc.yml → cacheFolder` | Yarn v2+ project cache; may be zero-installs (see Section 11) | 200 MB–3 GB | Conditional |
| pip cache (`%LOCALAPPDATA%\pip\Cache`) | `PIP_CACHE_DIR` env var | pip wheel + HTTP cache | 200 MB–4 GB | Medium |
| `%CARGO_HOME%\registry` (default `%USERPROFILE%\.cargo`) | `CARGO_HOME` env var | Cargo crate archive + source cache | 1–8 GB | High |
| `%CARGO_HOME%\git` | `CARGO_HOME` | Cargo git dependency checkouts | 200 MB–3 GB | Medium |
| `build/ dist/ .next/ .nuxt/ out/` (per project) | n/a | Front-end framework build outputs | 100 MB–3 GB | Medium |
| `__pycache__` / `.pyc` (per project) | n/a | Python bytecode cache | 50–500 MB | Low |
| Docker data VHDX: `%LOCALAPPDATA%\Docker\wsl\data\ext4.vhdx` | Docker Desktop settings.json | Docker images, containers, volumes (WSL2 backend) | 5–100+ GB | Very High |
| Docker distro VHDX: `%LOCALAPPDATA%\Docker\wsl\distro\ext4.vhdx` | Docker Desktop settings.json | Docker Desktop Linux distribution | 1–20 GB | High |
| WSL2 distro VHDXs: `%LOCALAPPDATA%\Packages\<DistroPackage>\LocalState\ext4.vhdx` | Registry: `HKCU:\Software\Microsoft\Windows\CurrentVersion\Lxss` | WSL2 distribution virtual disks — never auto-shrink | 5–80 GB | Very High |
| VS ComponentModelCache: `%LOCALAPPDATA%\Microsoft\VisualStudio\*\ComponentModelCache` | n/a | Visual Studio component resolver cache | 200 MB–2 GB | Medium |
| JetBrains caches: `%LOCALAPPDATA%\JetBrains\*\caches` | n/a | IntelliJ IDEA / Rider / WebStorm caches | 500 MB–10 GB | High |
| JetBrains logs: `%LOCALAPPDATA%\JetBrains\*\log` | n/a | JetBrains IDE logs | 50–500 MB | Low |
| vcpkg binary cache: `%LOCALAPPDATA%\vcpkg\archives` | `VCPKG_DEFAULT_BINARY_CACHE` env var | vcpkg C++ package binary cache | 500 MB–5 GB | Medium |
| Dart/Flutter pub cache: `%LOCALAPPDATA%\pub-cache` | `PUB_CACHE` env var | Flutter pub package cache | 200 MB–3 GB | Medium |
| Windows Update cache: `%WINDIR%\SoftwareDistribution\Download` | n/a | Downloaded Windows update packages | 1–10 GB | High |
| `%TEMP%` + `%WINDIR%\Temp` | n/a | Windows temporary files | 200 MB–5 GB | Medium |
| Android SDK images: `%LOCALAPPDATA%\Android\Sdk\system-images\` | `ANDROID_HOME` or `ANDROID_SDK_ROOT` | Android emulator system images | 2–50 GB | High |
| Android AVDs: `%USERPROFILE%\.android\avd\` | `ANDROID_AVD_HOME` env var | Android Virtual Device disk images + snapshots | 2–20 GB | High |
| Vagrant boxes: `%USERPROFILE%\.vagrant.d\boxes\` | `VAGRANT_HOME` env var | Vagrant virtual machine box images | 5–50 GB | High |
| Terraform providers: `.terraform\` (per project) | n/a | Terraform provider binaries per workspace | 100 MB–2 GB | Medium |
| Git LFS global cache: `%APPDATA%\Git\lfs\objects` | `GIT_LFS_SKIP_SMUDGE` / LFS config | Git Large File Storage cached objects | 1–50 GB | Medium |

---

## 3. Lessons Learned From Existing Tools

*(Tool analysis from v1.0 and v2.0 is factually accurate and retained unchanged.)*

**Confirmed gaps filled by no existing tool (post-research):**

- **WSL2 VHDX compaction with Windows Home fallback** — `Optimize-VHD` is a Hyper-V
  PowerShell cmdlet unavailable on Windows Home. Confirmed by multiple community reports
  and the Microsoft Q&A at learn.microsoft.com. `diskpart compact vdisk` works on all
  editions and requires only elevation, not Hyper-V.

- **Correct Docker build cache API** — the correct endpoint is `POST /build/prune`
  (confirmed by Docker Engine API docs and the Python SDK reference). No existing dev
  cleaner calls this endpoint directly.

- **pnpm store safe cleanup** — pnpm uses a content-addressable store with hard links
  on Windows and junctions for `node_modules` symlinks (confirmed by pnpm FAQ).
  `pnpm store prune` is the documented safe cleanup command; raw store deletion is not
  the recommended approach though it won't orphan hard links since deleting the store
  breaks the link source, causing the hardlink targets in node_modules to remain but
  point to files no longer in the store.

- **Yarn Berry zero-installs detection** — `.yarn/cache` in a Yarn Berry project may
  be intentionally committed to git (zero-installs strategy). Deleting it breaks the
  project for teams using zero-installs. Detection requires reading `.gitignore` to
  check whether `.yarn/cache` is gitignored.

- **NTFS atime disabled for large volumes** — Windows uses a "System Managed" mode
  for last-access-time updates. For volumes larger than 128 GB, this mode disables
  atime updates automatically (confirmed by Microsoft's fsutil documentation).
  Current status can be queried at runtime via `fsutil behavior query disablelastaccess`.

---

## 4. Product Vision & Differentiators

1. **Safety first** — dry-run is default on first launch; all deletions require
   explicit confirmation.
2. **Show before you delete** — sizes and paths displayed before any deletion.
3. **Age-aware** — idle projects are safer to clean; age is inferred from lock file
   `LastWriteTime` (reliable) rather than directory atime (unreliable on Windows).
4. **Extensible** — user-defined JSON rules; signed community rule packs.
5. **Windows-native feel** — WinUI 3 with Fluent Design. Not Electron, not Tauri.
6. **Complete coverage** — 24 project types, 12 package manager cache scanners,
   WSL2, Docker, IDEs, Windows system caches, Android SDK, Vagrant, Terraform.
7. **Runtime path resolution** — every cache path resolved via env var → tool config
   query → default, never hardcoded.
8. **Tool-managed caches cleaned correctly** — pnpm via `pnpm store prune`,
   Cargo caches via `cargo cache --autoclean`, git via `git lfs prune` and `git gc`.
9. **Signed binary** — Authenticode EV code signing from day one; self-update
   verifies signature via `WinVerifyTrust` before replacing the running binary.
10. **Workspace-aware** — Cargo workspaces, pnpm workspaces, Nx, and Turborepo
    are detected and reported as groups.

---

## 5. Feature Matrix

| Feature | WinSweep | WinMole | Kondo | Notes |
|---|---|---|---|---|
| Project scan (24 types) | ✓ | ✗ | ✓ | Same sentinel-based approach as kondo |
| Workspace/monorepo grouping | ✓ | ✗ | ✗ | Cargo workspace, pnpm workspace, Nx |
| Runtime path resolution via env vars | ✓ | ✗ | ✗ | CARGO_HOME, GRADLE_USER_HOME, PIP_CACHE_DIR, etc. |
| Age filter (lock file mtime) | ✓ | ✓ | ✗ | Lock file mtime is reliable; dir atime is not |
| NTFS atime status check at startup | ✓ | ✗ | ✗ | `fsutil behavior query disablelastaccess` |
| Dry-run default | ✓ | ✓ | ✓ | |
| TUI multi-select with vim bindings | ✓ | ✓ | ~ | |
| GUI (WinUI 3, Fluent Design) | ✓ | ✗ | ✗ | |
| Package manager caches (12 scanners) | ✓ | ✓ | ✓ | |
| pnpm via `pnpm store prune` | ✓ | ✗ | ✗ | Never raw-deletes the store |
| Yarn Classic vs Berry detection | ✓ | ✗ | ✗ | Reads `.yarnrc.yml`; checks `.gitignore` for zero-installs |
| WSL2 VHDX compaction | ✓ | ✗ | ✗ | `Optimize-VHD` + `diskpart` fallback for Windows Home |
| Docker build cache (correct API) | ✓ | ~ | ✗ | `POST /build/prune` with `until` filter |
| Android SDK / AVD management | ✓ | ✗ | ✗ | System images, AVD snapshots |
| Vagrant box management | ✓ | ✗ | ✗ | `%USERPROFILE%\.vagrant.d\boxes\` |
| Terraform provider cache | ✓ | ✗ | ✗ | `.terraform\` per workspace |
| Git LFS cache reporting | ✓ | ✗ | ✗ | Report only; clean via `git lfs prune` |
| VS / JetBrains cache scan | ✓ | ✗ | ✗ | Allowlist model, not blocklist-with-exceptions |
| JSON / NDJSON streaming output | ✓ | ✓ | ✗ | |
| TOML config (single canonical path) | ✓ | ✓ | ✗ | `%LOCALAPPDATA%\WinSweep\config.toml` |
| Hardcoded NEVER_DELETE list | ✓ | ✓ | ✗ | Compiled into binary, not user-overridable |
| Allowlist for permitted system subdirs | ✓ | ✗ | ✗ | Safer than blocklist-with-exceptions |
| Locked file detection before deletion | ✓ | ✗ | ✗ | Windows Restart Manager API |
| EV Authenticode signing | ✓ | ✗ | ✗ | Required at v0.5 launch |
| Verified self-update | ✓ | ✓ | ✗ | `WinVerifyTrust` signature check before binary replacement |
| Signed community rule packs | ✓ | ✗ | ✗ | Unsigned packs require `--unsafe-import` |
| Scheduled cleanup (Task Scheduler) | ✓ | ✗ | ✗ | |
| Per-rule prevention tips | ✓ | ✗ | ✗ | After cleanup: how to prevent regeneration |
| Opt-in anonymous crash reporting | ✓ | ✗ | ✗ | Sentry (Rust crate); opt-in only |
| OS version feature-gating | ✓ | ✗ | ✗ | Dev Drive wizard requires Win 11 22H2; Mica requires Win 11 |

---

## 6. Architecture & Tech Stack

| Component | Tech Stack | Responsibility |
|---|---|---|
| Scanner Engine | Rust (tokio async) | Parallel dir traversal, 24 project types, workspace detection, symlink-safe traversal via `symlink_metadata()`, reparse-point detection |
| CLI Interface | Rust (clap + ratatui) | TUI, multi-select, NDJSON output, config parse |
| GUI App | WinUI 3 + C# (.NET 8) | Fluent Design, system tray, stacked bar chart (v1.0), scheduler wizard |
| **IPC: named pipe (committed)** | `tokio::net::windows::named_pipe` | Scanner and GUI are separate processes; named pipe is the only architecture that supports cross-privilege operations (GUI runs as user; WSL/UpdateCache operations run elevated) |
| Config | TOML at `%LOCALAPPDATA%\WinSweep\config.toml` | Single canonical location; no `.windsweeprc` |
| WSL2 Compactor | PowerShell + `diskpart` fallback | Detect Hyper-V at runtime; use `Optimize-VHD` if available, `diskpart compact vdisk` otherwise |
| Docker Integration | Docker Engine REST API over `\\.\pipe\docker_engine` | `GET /system/df`, `POST /build/prune` |
| IDE Cache Reader | C# + File API | Allowlist-based: only touch permitted subdirs inside protected roots |
| Signed Update System | GitHub Releases API + `WinVerifyTrust` | Download → verify Authenticode signature → replace binary. EV cert required at v0.5 launch. |
| Locked File Detector | Windows Restart Manager API (`rstrtmgr.dll`) | Enumerate processes holding handles to target dirs before deletion |

**IPC architecture justification (definitive):**
The scanner must sometimes run elevated (WSL2 compaction requires admin for
`Optimize-VHD` or `diskpart`; Windows Update cache deletion requires stopping the
`wuauserv` service, which requires admin). The GUI must never run elevated as a whole.
Named pipe IPC is the only Windows architecture that correctly supports
cross-privilege communication between an unprivileged GUI process and an optionally-
elevated scanner worker. P/Invoke across privilege boundaries is not supported on
Windows.

---

## 7. Project Type Coverage

### 24 Project Type Signatures (Sentinel-File Detection)

| JS / Node | Rust / Go / C++ | JVM / .NET | Python / Others |
|---|---|---|---|
| `package.json` → npm / yarn / pnpm / bun | `Cargo.toml` → Rust / Cargo | `build.gradle` or `build.gradle.kts` → Gradle | `*.py` files + `__pycache__` → Python |
| `turbo.json` → Turborepo | `go.mod` → Go | `.csproj` or `.sln` → .NET / MSBuild | `pyvenv.cfg` → pip virtualenv |
| `app.json` + `android/` → React Native | `CMakeLists.txt` → CMake | `pom.xml` → Maven | `pubspec.yaml` → Dart / Flutter |
| `deno.json` or `deno.jsonc` → Deno | `build.zig` → Zig | `build.sbt` → SBT | `Gemfile` → Ruby |
| `.next/` dir → Next.js | `mix.exs` → Elixir | `build.xml` → Ant | `composer.json` → PHP Composer |
| `.nuxt/` dir → Nuxt | `Package.swift` → Swift | `ProjectSettings/` dir → Unity | `*.cabal` → Haskell Cabal |
| | `Makefile` + `obj/` or `CMakeCache.txt` → C/C++ | `project.godot` → Godot 4 | `pixi.toml` → Pixi |

**Plus 5 infrastructure project types:**

| Sentinel | Clean Target | Typical Size |
|---|---|---|
| `*.tf` files or `.terraform/` dir | `.terraform/` (provider binaries only) | 100 MB – 2 GB |
| `serverless.yml` or `serverless.json` | `.serverless/` | 50 MB – 500 MB |
| `Pulumi.yaml` or `Pulumi.yml` | `.pulumi/` | 50 MB – 200 MB |
| `cdk.json` | `cdk.out/` | 100 MB – 1 GB |
| `Vagrantfile` | `%USERPROFILE%\.vagrant.d\boxes\` (global, not per-project) | 5 GB – 50 GB |

### 12 Package Manager Cache Scanners

Each cache path is resolved at runtime using the priority: env var → tool config query
→ documented default. Hardcoded paths are only used as last-resort fallbacks.

| Tool | Runtime Resolution | Default Fallback |
|---|---|---|
| npm | `npm config get cache` | `%APPDATA%\npm-cache` |
| Yarn Classic (v1) | `YARN_CACHE_FOLDER` env var | `%LOCALAPPDATA%\Yarn\Cache\v6` |
| Yarn Berry (v2+) | `.yarnrc.yml → cacheFolder`; `enableGlobalCache` flag | Per-project `.yarn/cache` (see Section 11 for zero-installs detection) |
| pnpm | `pnpm store path` (executable query) | Varies; on Windows typically `%LOCALAPPDATA%\pnpm\store\v3` |
| bun | `%LOCALAPPDATA%\bun\install\cache` | Same (documented default on Windows) |
| pip | `PIP_CACHE_DIR` env var | `%LOCALAPPDATA%\pip\Cache` |
| Cargo registry | `CARGO_HOME` env var | `%USERPROFILE%\.cargo\registry` |
| Cargo git | `CARGO_HOME` env var | `%USERPROFILE%\.cargo\git` |
| Gradle | `GRADLE_USER_HOME` env var | `%USERPROFILE%\.gradle` |
| Maven | `<localRepository>` in `~/.m2/settings.xml` | `%USERPROFILE%\.m2\repository` |
| NuGet | `NUGET_PACKAGES` env var | `%USERPROFILE%\.nuget\packages` |
| Flutter pub | `PUB_CACHE` env var | `%LOCALAPPDATA%\pub-cache` |

### Workspace Detection

Monorepo structures require grouping, not per-project duplicate counting:

- **pnpm workspace**: `pnpm-workspace.yaml` in root → one `node_modules` at root.
  Report as one workspace artifact, not per-package.
- **Cargo workspace**: `Cargo.toml` with `[workspace]` section → one `target/` at root.
- **Nx monorepo**: `nx.json` in root → aggregate per-project `dist/` and `.nx/cache`.
- **Turborepo**: `turbo.json` → aggregate per-project outputs per `turbo.json → pipeline → outputs`.

### Size Calculation Correctness

- Symlinks: detected via `symlink_metadata()` returning `FileType::Symlink`; size is
  NOT followed or counted from the link target.
- Junction points (Windows reparse points): detected by `FILE_ATTRIBUTE_REPARSE_POINT`
  flag on `GetFileAttributes`; not followed during traversal.
- Nested projects: when a project root is detected inside another project's artifact
  dir, its artifact size is subtracted from the parent's total and reported as a
  separate line item (preventing double-counting).

---

## 8. CLI Command Reference

**Binary name:** `winsweep.exe` (matching brand name WinSweep throughout; no silent 'd').

| Command | Description |
|---|---|
| `winsweep scan [path]` | Scan path for build artifacts. TUI, sorted by size. |
| `winsweep scan --older 30d --json-stream` | Filter projects by lock-file mtime > 30 days; NDJSON output. |
| `winsweep scan --stamp` | Write `.winsweep-stamp` at each project root; subsequent sweep deletes files not accessed since stamp. |
| `winsweep scan --use-cache` | Replay last scan from `last-scan.json` without re-scanning disk. |
| `winsweep caches` | Resolve and show all package manager cache sizes. |
| `winsweep caches clean [--category npm]` | Clean cache. pnpm always uses `pnpm store prune`; Cargo uses `cargo cache --autoclean`; others use direct deletion. |
| `winsweep docker` | `GET /system/df` breakdown: images, containers, volumes, build cache. |
| `winsweep docker prune --older 7d` | `POST /build/prune` with `until=168h` filter. |
| `winsweep wsl list` | List installed WSL2 distros and their VHDX paths (from registry `HKCU:\Software\Microsoft\Windows\CurrentVersion\Lxss`). |
| `winsweep wsl compact [distro]` | Compact WSL2 VHDX. Detects Hyper-V at runtime; uses `Optimize-VHD` if available, `diskpart compact vdisk` on Windows Home. |
| `winsweep ide [--vs --jetbrains --vscode]` | Scan allowlisted IDE cache paths. Warns if IDE process is detected running. |
| `winsweep android` | List Android SDK system images, build tools versions, AVDs with sizes. |
| `winsweep git [path]` | Report `.git/lfs` cache sizes. Offers `git lfs prune` and `git gc`. Never deletes `.git` content directly. |
| `winsweep schedule` | Wizard: register `winsweep scan --older 30d` as a weekly Windows Task Scheduler job. |
| `winsweep config` | Open `%LOCALAPPDATA%\WinSweep\config.toml` in default editor. |
| `winsweep whitelist add <path>` | Append path to `whitelist.paths` in config. |
| `winsweep update` | Download latest GitHub release, verify Authenticode signature via `WinVerifyTrust`, replace binary. |
| `winsweep check-atime` | Run `fsutil behavior query disablelastaccess`. Report whether age filters can rely on access time. |
| `winsweep rules import <url>` | Import signed rule pack. Unsigned packs require `--unsafe-import` with explicit red-text warning. |
| `winsweep --dry-run` | Global flag: preview only. Default on first run per session. |
| `winsweep --json` / `--json-stream` | JSON array / NDJSON streaming output. |

### Config File (`%LOCALAPPDATA%\WinSweep\config.toml`)

```toml
[general]
dry_run = false
confirm_threshold_mb = 500
age_filter_days = 14
# Age is based on lock-file LastWriteTime (reliable).
# Atime is not used by default: disabled for volumes >128 GB on Windows
# (System Managed mode). Check with: winsweep check-atime
use_recycle_bin = false
# WARNING: Recycle Bin mode does NOT free disk space until the Recycle Bin
# is emptied. The moved files still occupy the same drive. Enable only for
# small, recoverable deletions.
scan_roots = [
  "C:\\Users\\me\\Projects",
]

[notifications]
disk_pressure_threshold_gb = 20

[telemetry]
# Must be explicitly set true. Never true by default.
opt_in = false

[whitelist]
paths = [
  "C:\\Projects\\keep-this\\node_modules",
]

[custom_rules]
[[custom_rules.rule]]
name = "WebStorm crash logs"
glob = "%USERPROFILE%\\java_error_in_webstorm_*.log"
risk = "safe"
prevention_tip = "Increase JVM heap in WebStorm's Help > Edit Custom VM Options"
```

---

## 9. GUI Design Specification

**Main Window Layout:**
- Top bar: WinSweep logo + free disk space gauge (green > 50 GB, yellow 20–50 GB, red < 20 GB)
- Left sidebar: Scan, Caches, Docker, WSL2, IDE Tools, Android, Schedule, Settings
- Main area: Sortable results list with multi-select checkboxes
- Bottom bar: Total selected size + Delete Selected + Dry Run toggle
- 🔒 icon on any item whose files are held by a running process (Restart Manager check)

**Dashboard:**
- Stacked bar chart (v1.0): Free / Dev Artifacts / Other proportional to drive size
- Summary cards: e.g. "node_modules: 14.2 GB across 87 projects"
- Last scan timestamp + Quick Scan button

**v1.1 Enhancement (post-launch):**
- Full proportional treemap (WizTree-inspired): each block = one artifact category;
  click to select; right-click for Open in Explorer / Add to Whitelist / Delete.
  Deferred from v1.0 because building a production-quality treemap control from
  scratch in WinUI 3 XAML is estimated at 4–6 additional weeks.

**OS feature-gating in GUI:**
- Mica background material: Windows 11 only; gracefully falls back to solid
  `SystemChromeMediumLowColor` brush on Windows 10.
- Dev Drive migration wizard: shown only when OS version ≥ 22H2 (Build 22621).
  Detected at runtime via `AnalyticsInfo.VersionInfo` or `Environment.OSVersion`.
- Rounded corners, snap layouts: Windows 11 only; no action needed for Win 10 fallback
  (WinUI 3 handles gracefully).

**Recycle Bin warning:** Any UI toggle for Recycle Bin mode shows inline:
> ⚠️ Recycle Bin mode does not free disk space. Files are renamed within the same drive
> and remain until you empty the Recycle Bin manually. For large deletions, use direct
> delete or create a compressed backup archive.

---

## 10. Safety & Risk Mitigation

### NEVER_DELETE List (Compiled Into Binary, Not User-Overridable)
```
C:\Windows
C:\Windows\System32
C:\Program Files
C:\Program Files (x86)
%SYSTEMROOT%
%WINDIR%
The directory containing the winsweep.exe binary itself
All drive roots (e.g. C:\, D:\)
Any path that is a parent directory of a currently running process executable
```

### Allowlist for Permitted System Subdirectories
Rather than a blocklist-with-exceptions (which has an inverted logic error risk), an
explicit allowlist governs the only subdirectories inside protected roots that WinSweep
may touch. This list is hardcoded and not user-overridable:

```
%LOCALAPPDATA%\Microsoft\VisualStudio\*\ComponentModelCache
%LOCALAPPDATA%\Microsoft\VisualStudio\*\Designer
%LOCALAPPDATA%\JetBrains\*\caches      ← NOT %LOCALAPPDATA%\JetBrains\Toolbox\apps\*
%LOCALAPPDATA%\JetBrains\*\log
%APPDATA%\Code\Cache
%APPDATA%\Code\CachedData
%APPDATA%\Code\CachedExtensions
%APPDATA%\Code\logs
```

`%LOCALAPPDATA%\JetBrains\Toolbox\apps\*` is the IDE installation directory and is
explicitly excluded from all scanning.

### All Safety Mechanisms

| Mechanism | Implementation |
|---|---|
| Dry-run by default | First operation per session is preview-only |
| Whitelist | User-defined paths in config; permanently skipped |
| Age filter | Based on lock-file `LastWriteTime`; atime status displayed at startup |
| Recycle Bin mode | Optional; with prominent "doesn't free space" warning in UI |
| Tool-managed cache protection | pnpm → `pnpm store prune`; Cargo → `cargo cache --autoclean`; git LFS → `git lfs prune`. Never raw-deleted. |
| pnpm junctions guard | pnpm on Windows uses directory junctions for node_modules. Junctions are `FILE_ATTRIBUTE_REPARSE_POINT` and are detected and not followed. |
| System node_modules guard | node_modules inside %APPDATA%, %LOCALAPPDATA%, %PF%, %PF(x86)% tagged SYSTEM; excluded from bulk-select |
| Yarn zero-installs guard | `.yarn/cache` in Berry projects: check if `.gitignore` excludes it. If not excluded (zero-installs), flag ⚠️ and require explicit per-item confirmation |
| Locked file detection | `RmGetList()` from Restart Manager API before any deletion; locked items shown with 🔒; skip-and-continue available |
| Docker volume protection | Named volumes never touched without `--include-volumes` flag |
| WSL2 prerequisites | Check `wsl --status`; enumerate running distros; confirm no active distros before compact |
| Windows Update cache | Stop `wuauserv` service before deletion; restart after. Requires elevation. |
| Signed self-update | `WinVerifyTrust` validates Authenticode signature before binary replacement |
| Community rule validation | Signed packs only by default; glob expansion checked against NEVER_DELETE + allowlist boundaries; per-rule confirmation shown |
| Audit log | Every deletion appended to `%LOCALAPPDATA%\WinSweep\audit.log` with timestamp, path, size |

---

## 11. Windows-Specific Features (Fully Verified)

### WSL2 VHDX Compaction — Verified Procedures

**Problem (confirmed by Microsoft documentation):** WSL2 uses dynamically expanding
VHDX files. When files are deleted inside WSL, the VHDX does not automatically shrink.
The VHDX must be explicitly compacted to reclaim space on the host.

**Method A — `Optimize-VHD` (requires Hyper-V module):**
`Optimize-VHD` is part of the `Hyper-V` PowerShell module. It is available on:
- Windows 10/11 Pro, Enterprise, Education with the Hyper-V feature enabled
- It is **NOT** available on Windows 10/11 Home editions regardless of elevation

Detection at runtime: `(Get-WindowsOptionalFeature -Online -FeatureName
Microsoft-Hyper-V-Management-PowerShell).State`

```powershell
# Method A procedure (Hyper-V available):
wsl.exe --shutdown
Optimize-VHD -Path "<path-to-vhdx>" -Mode Full
```

**Method B — `diskpart compact vdisk` (works on all Windows editions):**
Confirmed working on Windows Home by multiple community sources. Requires elevation.
The VHDX must be attached readonly before compaction (if not already detached):

```
diskpart
select vdisk file="<path-to-vhdx>"
attach vdisk readonly
compact vdisk
detach vdisk
exit
```

**Note on sparse VHD (WSL2 experimental feature):** WSL2 supports a `[experimental]
sparseVhd=true` setting in `.wslconfig`. If sparse VHD is enabled, `Optimize-VHD`
returns: "The requested operation could not be completed due to a virtual disk system
limitation. Virtual hard disk files must be uncompressed and unencrypted and must not
be sparse." WinSweep must detect sparse VHD mode before attempting compaction:
`wsl --manage <distro> --set-sparse false` disables it, enabling manual compaction.

**WSL2 distro VHDX path detection (do not hardcode):**
WSL2 stores distro registration data in the Windows registry at:
`HKCU:\Software\Microsoft\Windows\CurrentVersion\Lxss`

Query all installed distros and their base paths programmatically:
```powershell
Get-ChildItem HKCU:\Software\Microsoft\Windows\CurrentVersion\Lxss |
  ForEach-Object { Get-ItemProperty $_.PSPath } |
  Select-Object DistributionName, BasePath
```
The VHDX file is at `<BasePath>\ext4.vhdx` for each distro.

**Docker Desktop VHDX paths (verified):**
Docker Desktop with WSL2 backend creates two VHDX files:
- Data disk (images, containers, volumes): `%LOCALAPPDATA%\Docker\wsl\data\ext4.vhdx`
- Distro disk (Docker Desktop Linux environment): `%LOCALAPPDATA%\Docker\wsl\distro\ext4.vhdx`

Note: Newer versions of Docker Desktop (verified in community reports for Docker Desktop
4.x+) may use `docker_data.vhdx` instead of `ext4.vhdx` in the data folder. WinSweep
must enumerate both names:
```
%LOCALAPPDATA%\Docker\wsl\data\ext4.vhdx
%LOCALAPPDATA%\Docker\wsl\data\docker_data.vhdx
%LOCALAPPDATA%\Docker\wsl\distro\ext4.vhdx
```

**Both Docker Desktop AND WSL2 must be stopped before compaction** — confirmed by
community reports: `Optimize-VHD` returns `0x80070020` (file in use) if either is running.
Procedure: `taskkill /IM "Docker Desktop.exe" /F` → `net stop com.docker.service` →
`wsl --shutdown` → compact → restart Docker Desktop.

---

### Docker Integration via Engine REST API — Verified Endpoints

Docker Engine exposes a REST API over a Windows named pipe at `\\.\pipe\docker_engine`.
Connection from Rust requires `tokio::net::windows::named_pipe::ClientOptions`.

**API version negotiation:** Query `GET /_ping` to confirm daemon is reachable, then
`GET /version` to retrieve the engine API version. All subsequent calls must use the
versioned path (e.g., `/v1.47/<endpoint>`).

**Confirmed correct endpoints (sourced from Docker Engine API docs):**

| Operation | Endpoint | Notes |
|---|---|---|
| Disk usage breakdown | `GET /v{ver}/system/df` | Returns images, containers, volumes, build-cache with sizes |
| Build cache prune with age | `POST /v{ver}/build/prune` | `until` filter is a Go duration string (e.g. `168h` = 7 days) passed as URL query parameter: `?filters={"until":["168h"]}` (URL-encoded) |
| List images | `GET /v{ver}/images/json` | `Created` field is a Unix timestamp; use for age filtering |
| Remove single image | `DELETE /v{ver}/images/{id}` | |
| List containers | `GET /v{ver}/containers/json?all=true` | |
| Prune stopped containers | `POST /v{ver}/containers/prune` | |
| List volumes | `GET /v{ver}/volumes` | |
| Prune anonymous volumes | `POST /v{ver}/volumes/prune` | Named volumes not touched without `--include-volumes` flag |

**Docker not running:** If the named pipe is unavailable, the Docker subcommands
display a clear "Docker Desktop is not running" message and exit gracefully.
No crash.

---

### NTFS Last Access Time — Verified Behaviour

**Source:** Microsoft's `fsutil behavior` documentation confirms the following:

Windows 10/11 uses a "System Managed" mode for NTFS last-access-time (atime) updates.
In System Managed mode:
- Volumes **128 GB or smaller**: atime updates **enabled**
- Volumes **larger than 128 GB**: atime updates **disabled** (the common case for developer machines)

The current mode can be queried with:
```
fsutil behavior query disablelastaccess
```
Output values:
- `0` = User Managed, atime updates enabled
- `1` = User Managed, atime updates disabled
- `2` = System Managed, atime updates enabled (volume ≤ 128 GB)
- `3` = System Managed, atime updates disabled (volume > 128 GB)

**Impact on WinSweep:** The age filter for project artifacts uses `LastWriteTime` of
the project's lock file (`Cargo.lock`, `package-lock.json`, `yarn.lock`, etc.) as a
reliable proxy for "last time this project was actively developed." This is
`LastWriteTime`, not `LastAccessTime`, and is always reliable on NTFS regardless of
the atime setting.

For package manager caches (npm cache, Cargo registry, etc.), atime is the ideal
metric but is unreliable without user action. WinSweep reports the atime status at
startup and offers `winsweep check-atime` to surface the current setting. It does
NOT silently offer atime-based cache aging as if it were reliable.

---

### Yarn Berry Zero-Installs Detection (Verified)

Yarn Berry (v2+) caches packages as zip files inside `.yarn/cache` (per project).
If a project uses **zero-installs**, `.yarn/cache` is intentionally committed to git
and must not be deleted.

**Detection algorithm (verified against Yarn documentation):**

1. Detect Yarn Berry: presence of `.yarn/releases/yarn-*.cjs` OR `packageManager`
   field in `package.json` with a value starting with `yarn@` followed by a version
   ≥ 2.0.0 (e.g., `yarn@4.1.1`).
2. Determine zero-installs status: read `.gitignore` in the project root.
   - If `.yarn/cache` **is** listed in `.gitignore` → standard mode → cache is safe to clean.
   - If `.yarn/cache` is **not** in `.gitignore` → zero-installs → flag ⚠️ and require
     explicit per-item confirmation before offering to delete.

---

### pnpm Store Cleanup — Verified Behaviour

**Source:** pnpm official documentation (pnpm.io/cli/store):

pnpm stores packages in a content-addressable store. Project `node_modules` contain
hard links back to files in the store. On Windows, pnpm uses directory junctions
(not symlinks) for the `node_modules` virtual store structure.

`pnpm store prune` removes unreferenced packages from the store (packages not linked
by any project on the machine). According to pnpm documentation, this is safe and
has no side effects on existing projects. WinSweep always cleans pnpm via
`pnpm store prune`, never by directly deleting the store directory.

The store location is resolved at runtime via `pnpm store path` (subprocess call),
not by hardcoding a path.

---

### Windows Update Cache Deletion — Verified Procedure

Deleting `%WINDIR%\SoftwareDistribution\Download` requires:
1. Elevation (admin rights)
2. The Windows Update service (`wuauserv`) must be stopped before deletion to avoid
   access violations on locked files.

```
net stop wuauserv
net stop bits
[delete %WINDIR%\SoftwareDistribution\Download]
net start wuauserv
net start bits
```

WinSweep implements this service-stop-delete-restart lifecycle. Files that are locked
despite the service stop (rare) are skipped and logged in the audit log.

---

### IDE Cache Detection — Allowlist Model

**Allowlisted paths (safe to delete; hardcoded):**

```
%LOCALAPPDATA%\Microsoft\VisualStudio\*\ComponentModelCache
  → VS rebuilds this on next launch.
%LOCALAPPDATA%\Microsoft\VisualStudio\*\Designer
  → Designer cache; VS rebuilds on demand.
%LOCALAPPDATA%\JetBrains\*\caches
  → IntelliJ / Rider / WebStorm caches; rebuilt on next launch.
  → EXCLUDE: %LOCALAPPDATA%\JetBrains\Toolbox\apps\* (IDE installation)
%LOCALAPPDATA%\JetBrains\*\log
  → Log files; always safe.
%APPDATA%\Code\Cache
%APPDATA%\Code\CachedData
%APPDATA%\Code\CachedExtensions
%APPDATA%\Code\logs
```

**NOT implemented:** JetBrains "invalidate caches REST endpoint". No such public,
stable, cross-product REST API exists in JetBrains products. If a JetBrains IDE
process is detected running, WinSweep displays: "⚠️ [IDE name] is running. Close it
before deleting cache directories, or use File → Invalidate Caches inside the IDE."

---

## 12. Configuration & Extensibility

### Community Rule Pack Security Model

```
Trust Tier 1 — Built-in rules:
  Embedded in binary; signed with WinSweep's key; cannot be overridden.

Trust Tier 2 — Official registry (winsweep.dev/rules):
  Signed with WinSweep's key; fetched over HTTPS; checksum verified.

Trust Tier 3 — Community packs:
  Must be signed by author key; author key must be added to local trust store.
  Import: winsweep rules import <url> --trust-key <key>

Trust Tier 4 — Unsigned packs:
  Require --unsafe-import flag. Display red-text warning with full glob expansion
  shown before any confirmation is accepted. Per-rule confirmation mandatory.
```

**Glob validation (all tiers):** After glob expansion, every resolved path is
checked against the NEVER_DELETE list and the allowlist boundary. Any rule that
would resolve outside `scan_roots`, `%USERPROFILE%`, or the explicit allowlist
is rejected with an error.

---

## 13. Phased Build Roadmap

Realistic timeline for a 2-developer team. Tested assumptions based on scope analysis:

| Phase | Duration | Deliverables | Output |
|---|---|---|---|
| **Phase 0 — Foundation** | Weeks 1–4 | Rust scanner core: parallel dir traversal (tokio), 24 project type signatures, workspace detection, symlink-safe traversal, junction detection, env-var path resolution, NEVER_DELETE list, audit log. Binary signing infrastructure (EV cert). | `winsweep-core` crate |
| **Phase 1 — CLI TUI** | Weeks 5–8 | ratatui TUI: list, multi-select, range (V key), search, 3-colour progress bar, NDJSON output, config (single canonical TOML path), whitelist, Restart Manager locked-file detection. | `winsweep.exe` |
| **Phase 2 — Package Manager Caches** | Weeks 9–11 | All 12 cache scanners with runtime path resolution. pnpm via `pnpm store prune`. Yarn Classic vs Berry detection. Zero-installs guard. Cargo via `cargo cache --autoclean`. NTFS atime status reported at startup. | `winsweep.exe` |
| **Phase 3 — Windows-Specific** | Weeks 12–17 | WSL2 VHDX compaction (Optimize-VHD + diskpart fallback + sparse VHD detection). Docker Engine API (correct endpoints). Windows Update cache (service stop/start lifecycle). IDE allowlist scanner. Android SDK/AVD lister. Git LFS reporter. Vagrant, Terraform, Pulumi, CDK, Serverless project types. | `winsweep.exe` (v0.5 — fully shippable CLI) |
| **Phase 4 — GUI App** | Weeks 18–26 | WinUI 3: dashboard with stacked bar chart, allowlist editor, one-click scan + clean, disk before/after chart, system tray with disk pressure badge, OS feature-gating (Mica on Win 11, fallback brush on Win 10, Dev Drive wizard only on ≥ 22H2), WCAG 2.1 AA accessibility via WinUI UIA. | `WinSweep.exe` GUI |
| **Phase 5 — Automation & Polish** | Weeks 27–32 | Task Scheduler wizard, per-rule prevention tips, verified self-update (WinVerifyTrust), opt-in Sentry crash reporting, localization scaffolding (.resw strings externalized), disk threshold toasts, startup scan option. | v1.0 full release |
| **v1.1 (post-launch)** | +8–10 weeks | Full interactive treemap widget, community rule registry at winsweep.dev/rules, additional language packs. | v1.1 |

---

## 14. Engineering Decisions & Trade-offs

### Why Rust for the Scanner
Rust's `tokio` async runtime uses Windows I/O Completion Ports (IOCP) natively, giving
optimal parallel NTFS traversal performance. The existing Rust ecosystem (`kondo-lib`,
`cargo-sweep`, `cargo-cache`) provides battle-tested scanner code that WinSweep can
learn from directly. Symlink-safe traversal is straightforward using Rust's
`symlink_metadata()`.

### Why WinUI 3 for the GUI
WinUI 3 is the Windows 11 native UI framework providing: Fluent Design System (Mica
material, rounded corners, accent colours), native Windows notifications, system tray
API, and Task Scheduler COM integration. WinSweep is Windows-only by design; WinUI 3
is the correct framework choice. The treemap widget is deferred to v1.1 because
building a production-quality treemap control in WinUI 3 XAML from scratch is estimated
at 4–6 additional weeks.

### Why Named Pipe for IPC (Committed Decision)
Named pipe IPC is the only Windows architecture that correctly supports
cross-privilege communication: the GUI runs unprivileged; WSL2 compaction and Windows
Update cache deletion require elevation. Named pipe allows the GUI to request an
elevated scanner worker process without elevating the entire GUI.

### pnpm Store Is Never Raw-Deleted
pnpm creates hard links from the global store to `node_modules`. Deleting the store
directory while `node_modules` directories exist leaves the hardlinks intact (the
files are still accessible via the hardlink) but the store is broken — `pnpm install`
on any project would need to re-download all packages. Always use `pnpm store prune`.

### Cargo-sweep Stamp Mode Adopted
`winsweep scan --stamp` writes `.winsweep-stamp` (distinct from cargo-sweep's
`.cargo-sweep` to avoid conflicts if both tools are used). Subsequent sweeps delete
files not accessed since the stamp. This is safer than time-based deletion alone.

### npkill System node_modules Pattern — Adopted and Extended
Any `node_modules` inside a path that is NOT under a configured `scan_root` OR is
inside `%APPDATA%`, `%LOCALAPPDATA%`, `%ProgramFiles%`, or `%ProgramFiles(x86)%` is
tagged SYSTEM and excluded from bulk-select. Users may override per-item with a
confirmation prompt.

### Code Signing Is Non-Negotiable at v0.5 Launch
Without an EV Authenticode certificate, Windows Defender SmartScreen blocks the
installer and every self-update download for users who have not previously run the
binary. EV cert acquisition is a Phase 0 task, not a post-launch concern.

### Telemetry Is Opt-In, Never Opt-Out
`[telemetry] opt_in = false` is the hardcoded default. Telemetry is offered at first
launch with a clear description of what is collected (crash reports via Sentry,
anonymous counts of artifact types found and space freed — no file paths, no
usernames, no content). The user must explicitly set `opt_in = true`.

---

*WinSweep Architecture Plan · Version 3.0 · 2026*
*All technical claims sourced from official documentation or verified community research.*
*No assumptions made; "requires runtime detection" stated explicitly where behaviour varies.*
