# WinSweep - Windows Disk Cleaning Tool

A high-performance, safe disk cleaning tool for Windows 10/11 that intelligently identifies and removes unnecessary files while protecting critical system data.

## Phase 0 Implementation (Foundation)

### Overview
Phase 0 implements the foundational components of WinSweep, providing the core infrastructure for all subsequent phases.

### Architecture

```
WinSweep/
├── crates/
│   ├── winsweep-common/     # Shared types, configuration, and utilities
│   ├── winsweep-core/       # Core scanning and cleanup logic
│   ├── winsweep-cli/        # Command-line interface (Phase 1)
│   └── winsweep-gui/        # GUI application (Phase 4)
├── scripts/
│   └── sign-build.ps1       # Binary signing script
└── docs/                    # Documentation
```

### Implemented Features

#### 1. Parallel File System Scanner
- **Location**: `crates/winsweep-core/src/scanner.rs`
- **Features**:
  - Tokio-based parallel traversal
  - Configurable parallelism
  - Streaming results for memory efficiency
  - Junction and symlink detection
  - Hidden file handling

#### 2. Cross-Privilege Named Pipe IPC
- **Location**: `crates/winsweep-core/src/ipc.rs`
- **Features**:
  - Secure communication between GUI and elevated scanner
  - SDDL-based security descriptor (Authenticated Users access)
  - Async message framing with length prefixes
  - Ping/pong health checks

#### 3. Project Type Detection
- **Location**: `crates/winsweep-common/src/project_signatures.rs`
- **Features**:
  - 34+ project type signatures
  - Confidence-based detection
  - Content pattern matching
  - Support for: Node.js, Rust, Python, Java, Go, Docker, etc.

#### 4. NEVER_DELETE List
- **Location**: `crates/winsweep-common/src/never_delete.rs`
- **Features**:
  - Comprehensive system path protection
  - Pattern-based file extension protection
  - Context-aware safety checks
  - Prevents accidental system file deletion

#### 5. Junction vs Symlink Detection
- **Location**: `crates/winsweep-core/src/junction_detector.rs`
- **Features**:
  - Windows API-based detection
  - Target resolution
  - Circular reference detection
  - Proper handling of reparse points

#### 6. Audit Logging
- **Location**: `crates/winsweep-core/src/audit_logger.rs`
- **Features**:
  - JSON-formatted audit trail
  - Operation tracking
  - Security violation logging
  - Log rotation support

#### 7. Windows API Wrapper
- **Location**: `crates/winsweep-core/src/windows_api.rs`
- **Features**:
  - Safe wrappers around Windows APIs
  - Process enumeration
  - Disk space queries
  - Registry access
  - File lock detection

#### 8. Cleanup Manager
- **Location**: `crates/winsweep-core/src/cleanup.rs`
- **Features**:
  - Safe file/directory deletion
  - Recycle bin support
  - Verification of deletions
  - Restore point creation

### Build Requirements

1. **Rust 1.75+** - Install from [rustup.rs](https://rustup.rs/)
2. **Visual Studio C++ Build Tools** - Required for Windows dependencies
3. **Windows 10/11** - Target platform

### Building

```powershell
# Clone the repository
git clone https://github.com/winsweep/winsweep.git
cd winsweep

# Build all crates
cargo build --workspace

# Run tests
cargo test --workspace

# Build release binaries
cargo build --workspace --release
```

### Testing

The implementation includes comprehensive unit tests for all major components. Tests cover:
- Scanner functionality
- IPC communication
- Project type detection
- Junction detection
- Cleanup operations
- Windows API wrappers

### Security Considerations

1. **Elevation Requirements**: Scanner component requires administrator privileges for full system access
2. **IPC Security**: Named pipe uses restrictive DACL allowing only Authenticated Users
3. **Path Validation**: All paths are validated against NEVER_DELETE list before operations
4. **Audit Trail**: All operations are logged for security and compliance

### Next Steps

Phase 0 is complete and ready for Phase 1 implementation:
- CLI TUI with ratatui
- Configuration management
- Tool availability detection
- Service management integration

### Configuration

Default configuration is created automatically at:
```
%APPDATA%\WinSweep\config.toml
```

Audit logs are stored at:
```
%PROGRAMDATA%\WinSweep\audit.log
```

## License

MIT License - see LICENSE file for details.