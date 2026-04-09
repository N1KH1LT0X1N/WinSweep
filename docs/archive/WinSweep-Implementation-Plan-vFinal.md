# WinSweep — Final Implementation Plan v5.0
## Risk-Adjusted Roadmap with Critical Issue Mitigations

> This plan addresses 32 identified implementation issues across 7 categories.
> Timeline adjusted from 34 to 36 weeks to accommodate critical fixes and validation spikes.

---

## Revised Phase Structure

### Phase 0 — Foundation (Weeks 1-4)
**Core Deliverables:**
- Rust scanner core with parallel traversal (tokio)
- 34+ project type signatures detection
- **NEW: Cross-privilege named pipe IPC prototype**
- **NEW: Security descriptor implementation for pipe DACL**
- NEVER_DELETE list and audit logging
- **NEW: Junction vs symlink detection module**
- Binary signing infrastructure (EV cert acquisition)

**Critical Fixes Applied:**
- Implement `SECURITY_ATTRIBUTES` with proper DACL allowing unprivileged GUI to connect to elevated scanner
- Create `junction_detector.rs` using Windows API `GetFileAttributes` to detect `FILE_ATTRIBUTE_REPARSE_POINT`
- Add path resolution validation before all operations

**Risk Mitigation:**
- Week 4: Full IPC security testing on both Standard and Admin user accounts

### Phase 0.5 — Windows Home Compatibility Spike (Week 5)
**NEW VALIDATION PHASE**
- Set up Windows Home 10/11 VM test environment
- Validate `diskpart compact vdisk` workflow without Hyper-V module
- Test `wsl --manage` command availability on older builds
- Verify registry paths for WSL2 detection
- Document all Home edition limitations

**Deliverable:** Home Edition Compatibility Report with fallback implementations

### Phase 1 — CLI TUI (Weeks 6-9)
**Core Deliverables:**
- ratatui TUI with multi-select, search, vim bindings
- NDJSON output support
- Config management (single canonical TOML)
- **NEW: Tool availability detection module**
- **NEW: Service management wrapper**
- Restart Manager integration
- **NEW: Handle detection fallback using NtQuerySystemInformation**

**Critical Fixes Applied:**
- Implement `tool_detector.rs` to verify pnpm, go, Docker availability in PATH
- Create `service_manager.rs` with proper disable/stop/start/re-enable lifecycle
- Add hybrid locked file detection (Restart Manager + system handle query)

### Phase 2 — Package Manager Caches (Weeks 10-13)
**Core Deliverables:**
- All 18 cache scanners with runtime path resolution
- **NEW: Docker API version negotiation** (moved from Phase 3)
- pnpm store prune integration
- Yarn Classic vs Berry detection with zero-installs guard
- Cargo cache with prerequisite checking
- Go module/build cache via `go env` subprocess
- Poetry and uv cache cleanup
- **NEW: Multiple installation detection**

**Critical Fixes Applied:**
- Implement Docker API dynamic version detection (/v1.40/_ping → /version → endpoint selection)
- Add fallback paths for tools not in PATH (common installation directories)
- Create robust subprocess handling with timeout and error recovery

### Phase 3 — Windows-Specific Features (Weeks 14-21)
**Core Deliverables:**
- WSL2 VHDX compaction with retry logic
- Docker Engine API integration (version negotiation complete)
- Windows Update cache cleanup with service management
- IDE allowlist scanner
- Android SDK/AVD management
- Git LFS reporting
- Playwright/Cypress binary cache managers
- Infrastructure project types (Vagrant, Terraform, etc.)

**Critical Fixes Applied:**
- WSL2 compaction: 5-retry logic with 2-second exponential backoff
- Sparse VHD detection with fallback to manual compaction
- Service race condition prevention (disable before stop)
- Handle detection before VHDX operations

**Extended by 1 week** for comprehensive WSL2/Docker testing

### Phase 4 — GUI App (Weeks 22-30)
**Core Deliverables:**
- WinUI 3 interface with Fluent Design
- Dashboard with stacked bar chart
- **NEW: Elevated operation coordinator**
- System tray integration
- Disk pressure notifications
- OS feature-gating (Mica, Dev Drive wizard)
- WCAG 2.1 AA accessibility

**Critical Fixes Applied:**
- Implement elevated process coordinator for cross-privilege operations
- Add Windows Home edition UI adjustments (no Mica, solid brushes)
- Create comprehensive error handling for IPC failures

### Phase 5 — Automation & Polish (Weeks 31-36)
**Core Deliverables:**
- Task Scheduler wizard
- Per-rule prevention tips
- Verified self-update with WinVerifyTrust
- Opt-in Sentry crash reporting
- Localization scaffolding
- **NEW: Home edition feature parity documentation**

**Extended by 1 week** for additional testing on Windows Home

---

## Critical Implementation Details

### 1. Cross-Privilege Named Pipe IPC
```rust
// Security descriptor allowing unprivileged access
let mut sd = SecurityDescriptor::new()?;
sd.set_dacl_builder(|dacl| {
    dacl.add_allow_ace(Ace::new(
        Sid::from_known_sid(KnownSid::AuthenticatedUsers),
        AceFlags::OBJECT_INHERIT | AceFlags::CONTAINER_INHERIT,
        AccessRights::GENERIC_ALL,
        AceType::AccessAllowed,
    ))?;
    Ok(())
});
```

### 2. WSL2 Compaction with Retry Logic
```rust
for attempt in 1..=5 {
    match compact_vhd(&vhdx_path) {
        Ok(_) => break,
        Err(e) if e.contains("access denied") && attempt < 5 => {
            thread::sleep(Duration::from_millis(2000 * attempt));
            // Try to force close any remaining handles
            kill_wsl_processes()?;
        }
        Err(e) => return Err(e),
    }
}
```

### 3. Docker API Version Negotiation
```rust
let api_version = match client.get("/v1.40/_ping").await {
    Ok(_) => {
        let version: DockerVersion = client.get("/version").await?;
        version.api_version
    }
    Err(_) => {
        // Fallback to TCP socket
        try_tcp_socket().await?
    }
};
```

### 4. Service Management with Race Prevention
```powershell
# Disable service to prevent auto-restart
sc config wuauserv start= disabled
# Stop the service
net stop wuauserv
# Delete files
Remove-Item -Path "$env:WINDIR\SoftwareDistribution\Download\*" -Recurse -Force
# Re-enable service
sc config wuauserv start= auto
net start wuauserv
```

### 5. Tool Availability Detection
```rust
pub fn find_tool_executable(name: &str) -> Option<PathBuf> {
    // Check PATH first
    if let Some(path) = which::which(name).ok() {
        return Some(path);
    }
    
    // Check common installation locations
    let common_paths = match name {
        "pnpm" => vec![
            r"%LOCALAPPDATA%\pnpm\pnpm.exe",
            r"%APPDATA%\npm\pnpm.cmd",
        ],
        "go" => vec![
            r"%LOCALAPPDATA%\Programs\Go\bin\go.exe",
            r"C:\Go\bin\go.exe",
        ],
        // ... more tools
        _ => vec![],
    };
    
    common_paths.iter()
        .map(|p| PathBuf::from(p).expand_env())
        .find(|p| p.exists())
}
```

---

## Testing Strategy

### Continuous Integration
1. **Windows Matrix Testing:**
   - Windows 10 Pro (19044+)
   - Windows 11 Pro (22621+)
   - Windows 10 Home (19044+)
   - Windows 11 Home (22621+)

2. **Tool Version Matrix:**
   - Docker Desktop 2.x, 3.x, 4.x
   - Go 1.19, 1.20, 1.21
   - Node.js 16, 18, 20
   - pnpm 7, 8, 9

3. **Scenario Testing:**
   - Fresh machine with no tools
   - Machine with multiple tool versions
   - Machine with non-standard PATH configurations
   - Machine with locked files (IDEs running)

### Manual Validation Points
- Week 5: Windows Home VM validation
- Week 13: Package manager cache cleanup on real projects
- Week 20: WSL2 compaction on actual developer machines
- Week 29: GUI IPC stress testing with UAC prompts

---

## Risk Register

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Named pipe IPC security blocks communication | Medium | High | Phase 0 prototype, extensive testing |
| WSL2 handles remain locked after shutdown | High | Medium | Retry logic, handle detection, user prompt |
| Docker API version incompatibility | Medium | Medium | Dynamic negotiation, graceful degradation |
| Windows Home edition limitations | High | High | Phase 0.5 spike, diskpart fallback |
| Service restart race conditions | Medium | Medium | Disable before stop, atomic operations |
| Tool not in PATH | High | Low | Common path scanning, user guidance |
| Memory usage on large monorepos | Medium | Medium | Streaming results, bounded memory |

---

## Deliverables Summary

### Phase 0 (Weeks 1-4)
- `winsweep-core` crate with IPC prototype
- EV certificate acquired
- Junction detection module

### Phase 0.5 (Week 5)
- Windows Home compatibility report
- Validated fallback implementations

### Phase 1 (Weeks 6-9)
- Functional CLI with TUI
- Tool detection module
- Service management wrapper

### Phase 2 (Weeks 10-13)
- All 18 cache scanners operational
- Docker API negotiation working
- Multiple installation support

### Phase 3 (Weeks 14-21)
- WSL2 compaction reliable with retries
- Windows Update cleanup working
- All Windows-specific features complete

### Phase 4 (Weeks 22-30)
- Full GUI with elevated operation support
- Cross-privilege IPC production-ready
- OS feature-gating implemented

### Phase 5 (Weeks 31-36)
- Scheduled cleanup automation
- Verified self-update system
- Production-ready v1.0 release

---

## Success Metrics

1. **Reliability:** 99% of cleanup operations complete without error
2. **Compatibility:** Works on all Windows 10/11 editions (Pro/Home)
3. **Performance:** Full NVMe scan under 60 seconds, HDD under 5 minutes
4. **Safety:** Zero accidental deletions from NEVER_DELETE paths
5. **User Experience:** Clear error messages, no silent failures

---

*WinSweep Implementation Plan · Version 5.0 · 2026*
*Addresses all 32 identified issues with specific mitigations*
*Timeline: 36 weeks (2-week extension for critical fixes)*
