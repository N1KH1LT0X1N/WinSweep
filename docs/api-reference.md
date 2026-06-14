# WinSweep API Reference

This document covers the public APIs exposed by `winsweep-core` and
`winsweep-common`.  These are the crates you would depend on when embedding
WinSweep's scanning or cleanup logic in another application.

---

## winsweep-common

### `Config`

```rust
pub struct Config {
    pub scan: ScanConfig,
    pub cleanup: CleanupConfig,
    pub ui: UiConfig,
    pub logging: LoggingConfig,
    pub auto_cleanup_enabled: bool,
    pub auto_cleanup_days: u32,
    pub notify_low_disk_space: bool,
    pub low_disk_threshold: u8,
    pub notify_cleanup_complete: bool,
}
```

#### Methods

| Method | Signature | Description |
|---|---|---|
| `load` | `() -> Result<Config>` | Load from `%AppData%\WinSweep\config.toml`, creating defaults if absent |
| `save` | `(&self) -> Result<()>` | Persist to the config file |
| `validate` | `(&mut self) -> Result<()>` | Clamp values to valid ranges |
| `config_path` | `() -> PathBuf` | Path to `config.toml` |
| `log_path` | `() -> PathBuf` | Path to the log directory |
| `cache_path` | `() -> PathBuf` | Path to the cache directory |

### `ScanConfig`

```rust
pub struct ScanConfig {
    pub default_paths: Vec<String>,
    pub include_hidden: bool,
    pub include_system: bool,
    pub min_file_size: u64,        // bytes
    pub exclude_patterns: Vec<String>,
    pub max_depth: Option<usize>,
    pub follow_symlinks: bool,
}
```

### `CleanupConfig`

```rust
pub struct CleanupConfig {
    pub clean_temp_files: bool,
    pub clean_recycle_bin: bool,
    pub clean_prefetch: bool,
    pub clean_browser_cache: bool,
    pub use_recycle_bin: bool,
    pub confirm_before_delete: bool,
    pub create_restore_point: bool,
}
```

### `NEVER_DELETE` (constant set)

```rust
/// Paths that must never be deleted, regardless of user input.
pub static NEVER_DELETE: Lazy<HashSet<PathBuf>>;
```

The set includes (but is not limited to):
`C:\Windows`, `C:\Windows\System32`, `C:\Program Files`,
`C:\Program Files (x86)`, `C:\Users`, `C:\ProgramData`.

---

## winsweep-core

### `Scanner`

```rust
pub struct Scanner;
```

#### `Scanner::start`

```rust
pub fn start(config: ScanConfig, paths: Vec<PathBuf>) -> ScannerHandle
```

Begins a parallel directory walk. Returns a `ScannerHandle` from which results
can be consumed asynchronously.

#### `ScannerHandle`

```rust
impl ScannerHandle {
    pub async fn next(&mut self) -> Option<CommonScanResult>;
    pub fn cancel(&self);
    pub fn is_finished(&self) -> bool;
    pub fn items_scanned(&self) -> u64;
    pub fn bytes_scanned(&self) -> u64;
}
```

#### `CommonScanResult`

```rust
pub struct CommonScanResult {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub modified: Option<SystemTime>,
    pub is_dir: bool,
    pub category: Option<String>,
}
```

---

### `CleanupManager`

```rust
pub struct CleanupManager;
```

#### `CleanupManager::delete_batch`

```rust
pub async fn delete_batch(
    paths: &[PathBuf],
    use_recycle_bin: bool,
) -> Result<CleanupResult>
```

Deletes `paths`, optionally moving them to the Recycle Bin.  Automatically
skips any path in `NEVER_DELETE`.

#### `CleanupResult`

```rust
pub struct CleanupResult {
    pub items_deleted: u64,
    pub items_failed: Vec<(PathBuf, String)>,   // (path, error message)
    pub space_freed_bytes: u64,
}
```

---

### `AuditLogger`

```rust
pub struct AuditLogger {
    pub log_path: PathBuf,
}
```

| Method | Description |
|---|---|
| `new(path: &Path) -> Result<Self>` | Open/create the log file |
| `log_scan_start(&self, paths: &[PathBuf]) -> Result<()>` | Record a scan-start event |
| `log_scan_complete(&self, result: &CleanupResult) -> Result<()>` | Record completion |
| `log_deletion(&self, path: &Path, size: u64) -> Result<()>` | Record a single deletion |

Log entries are written as JSON lines (NDJSON).

---

### `PackageManager` (trait)

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

#### `PackageCleanResult`

```rust
pub struct PackageCleanResult {
    pub package_manager: String,
    pub space_freed: u64,
    pub items_deleted: u64,
    pub errors: Vec<String>,
    pub duration_ms: u64,
}
```

#### `CacheInfo`

```rust
pub struct CacheInfo {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub description: String,
    pub can_delete: bool,
}
```

### `PackageManagerRegistry`

```rust
pub struct PackageManagerRegistry { /* opaque */ }

impl PackageManagerRegistry {
    pub async fn new() -> Self;
    pub fn get_managers(&self) -> &[Box<dyn PackageManager>];
    pub async fn get_installed(&self) -> Result<Vec<&dyn PackageManager>>;
    pub fn get_by_name(&self, name: &str) -> Option<&dyn PackageManager>;
    pub async fn clean_all(&self) -> Result<Vec<PackageCleanResult>>;
    pub async fn get_total_cache_size(&self) -> Result<u64>;
}
```

#### Registered manager names

`npm`, `pnpm`, `yarn`, `pip`, `poetry`, `cargo`, `go-modules`, `nuget`,
`gradle`, `maven`, `flutter`, `bun`, `pixi`, `composer`, `vcpkg`, `conan`,
`sbt`, `go-build`, `android-sdk`, `git-lfs`, `playwright`, `cypress`,
`chrome`, `edge`, `firefox`

---

### `DockerClient`

```rust
pub struct DockerClient { /* opaque */ }

impl DockerClient {
    pub async fn new() -> Result<Self>;
    pub async fn is_running(&self) -> bool;
    pub async fn list_containers(&self) -> Result<Vec<ContainerInfo>>;
    pub async fn list_images(&self) -> Result<Vec<ImageInfo>>;
    pub async fn list_volumes(&self) -> Result<Vec<VolumeInfo>>;
    pub async fn list_networks(&self) -> Result<Vec<NetworkInfo>>;
    pub async fn cleanup(&self, options: CleanupOptions) -> Result<DockerCleanupResult>;
}
```

---

### `WslDetector`

```rust
pub struct WslDetector;

impl WslDetector {
    pub fn new() -> Self;
    pub async fn detect_distributions(&self) -> Result<Vec<WslDistribution>>;
    pub async fn stop_distribution(&self, name: &str) -> Result<()>;
    pub async fn unregister_distribution(&self, name: &str) -> Result<()>;
}
```

#### `WslDistribution`

```rust
pub struct WslDistribution {
    pub name: String,
    pub state: WslState,
    pub version: WslVersion,
    pub vhd_path: Option<PathBuf>,
    pub disk_size_bytes: Option<u64>,
}

pub enum WslState  { Running, Stopped, Unknown }
pub enum WslVersion { V1, V2 }
```

---

### `ServiceManager`

```rust
pub struct ServiceManager;

impl ServiceManager {
    pub fn new() -> Result<Self>;
    pub fn list_services(&self) -> Result<Vec<ServiceStatus>>;
    pub fn start_service(&self, name: &str) -> Result<()>;
    pub fn stop_service(&self, name: &str) -> Result<()>;
    pub fn restart_service(&self, name: &str) -> Result<()>;
}
```

#### `ServiceStatus`

```rust
pub struct ServiceStatus {
    pub name: String,
    pub display_name: String,
    pub state: ServiceState,
    pub start_type: String,
    pub description: Option<String>,
}

pub enum ServiceState { Running, Stopped, StartPending, StopPending, Paused, Unknown }
```

---

### `WindowsEditionDetector`

```rust
pub struct WindowsEditionDetector;

impl WindowsEditionDetector {
    pub fn new() -> Self;
    pub fn detect() -> Result<WindowsCompatibilityReport>;
}
```

#### `WindowsCompatibilityReport`

```rust
pub struct WindowsCompatibilityReport {
    pub edition: WindowsEdition,
    pub version: String,
    pub build_number: u32,
    pub features: WindowsFeatures,
}

pub struct WindowsFeatures {
    pub wsl_available: bool,
    pub hyper_v_available: bool,
    pub sandbox_available: bool,
    pub containers_available: bool,
}
```

---

### `JunctionDetector`

```rust
pub struct JunctionDetector;

impl JunctionDetector {
    pub fn new() -> Self;
    pub fn is_junction(path: &Path) -> bool;
    pub fn is_symlink(path: &Path) -> bool;
    pub fn is_reparse_point(path: &Path) -> bool;
}
```

---

### `IpcClient` / `IpcServer`

Used internally by the ElevatedCoordinator. Not intended for direct use.

```rust
pub struct IpcClient;
impl IpcClient {
    pub async fn connect(pipe_name: &str) -> Result<Self>;
    pub async fn send(&mut self, op: &ElevatedOperation) -> Result<ElevatedOperationResult>;
}

pub struct IpcServer;
impl IpcServer {
    pub async fn listen(pipe_name: &str) -> Result<Self>;
    pub async fn next_request(&mut self) -> Result<ElevatedOperation>;
    pub async fn send_result(&mut self, result: &ElevatedOperationResult) -> Result<()>;
}
```

---

### Utility Functions

```rust
/// Calculate total size of a directory tree.
pub async fn calculate_directory_size(path: &PathBuf) -> Result<u64>;

/// Delete a directory, first shutting down any processes that have it locked.
pub async fn safe_delete_directory(path: &PathBuf) -> Result<u64>;

/// Format bytes as a human-readable string (B / KB / MB / GB / TB).
pub fn format_bytes(bytes: u64) -> String;
```

---

## CLI Output Format (ndjson)

When run with `--output ndjson`, each discovered file is emitted as one JSON
object per line:

```json
{"path":"C:\\Users\\alice\\AppData\\Local\\npm-cache\\...","size_bytes":12345678,"modified":"2024-01-15T10:30:00Z","category":"Package Cache"}
```

Fields:

| Field | Type | Description |
|---|---|---|
| `path` | string | Absolute path |
| `size_bytes` | u64 | File size in bytes |
| `modified` | string (RFC3339) or null | Last modification time |
| `category` | string or null | Classification |

The stream terminates with a summary line:

```json
{"summary":true,"total_files":1234,"total_bytes":9876543210,"duration_ms":4500}
```
