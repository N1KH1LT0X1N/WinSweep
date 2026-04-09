# WinSweep Windows Home Edition Compatibility Report

**Version**: 1.0  
**Date**: April 8, 2026  
**Phase**: 0.5 - Windows Home Compatibility Spike  

## Executive Summary

This report documents WinSweep's compatibility with Windows Home editions (Windows 10/11 Home) and provides fallback implementations for features not available on Home editions. WinSweep maintains full functionality on Home editions through strategic use of alternative APIs and tools.

## Testing Environment

- **Windows 10 Home**: Build 19044+
- **Windows 11 Home**: Build 22621+
- **Validation Method**: Registry detection, API availability checks, and script validation

## Feature Compatibility Matrix

| Feature | Windows Pro/Enterprise | Windows Home | Fallback Method | Status |
|---------|------------------------|--------------|-----------------|---------|
| **Core Scanning** | ✅ Native | ✅ Native | N/A | Fully Compatible |
| **Parallel File System** | ✅ Tokio | ✅ Tokio | N/A | Fully Compatible |
| **Named Pipe IPC** | ✅ Windows API | ✅ Windows API | N/A | Fully Compatible |
| **WSL2 Detection** | ✅ Registry | ✅ Registry | Multiple Methods | Fully Compatible |
| **WSL2 VHDX Compaction** | ✅ wsl --manage | ✅ diskpart | diskpart Script | Fully Compatible |
| **WSL Management** | ✅ wsl --manage | ⚠️ Limited | wslconfig.exe | Compatible |
| **VHD Management** | ✅ Hyper-V | ✅ diskpart | diskpart | Fully Compatible |
| **Device Encryption** | N/A (Has BitLocker) | ✅ Available | N/A | Alternative Available |
| **Service Management** | ✅ Native | ✅ Native | N/A | Fully Compatible |
| **Restart Manager** | ✅ Native | ✅ Native | N/A | Fully Compatible |

## Detailed Analysis

### 1. Core Functionality ✅

All core WinSweep features work identically across Windows editions:
- File system scanning with parallel traversal
- Junction and symlink detection
- Project type detection (34+ signatures)
- NEVER_DELETE list enforcement
- Audit logging

### 2. WSL2 Support ✅

#### Detection Methods
WinSweep uses multiple detection methods with fallbacks:

1. **Primary**: Registry key `SOFTWARE\Microsoft\Windows\CurrentVersion\Lxss`
2. **Secondary**: Check for `wsl.exe` in PATH
3. **Tertiary**: Check for WSL files in `C:\Windows\System32\lxss\`
4. **Fallback**: Check Windows build number (≥18362 for WSL2)

#### VHDX Compaction
Three methods with automatic fallback:

1. **Preferred**: `wsl --manage --optimize` (Build ≥21364)
2. **Alternative**: `wslconfig.exe /shutdown` + diskpart
3. **Fallback**: Direct diskpart script

**Sample diskpart script used:**
```diskpart
select vdisk file="%USERPROFILE%\AppData\Local\Packages\<PACKAGE>\LocalState\ext4.vhdx"
attach vdisk readonly
compact vdisk
detach vdisk
```

### 3. VHD Management ✅

Windows Home edition lacks Hyper-V, but diskpart provides full VHD functionality:

- **Create VHD**: `diskpart create vdisk file=...`
- **Attach VHD**: `diskpart attach vdisk`
- **Compact VHD**: `diskpart compact vdisk`
- **Detach VHD**: `diskpart detach vdisk`

### 4. Windows Feature Limitations

#### Not Available on Home Edition:
- Group Policy Editor (gpedit.msc)
- Hyper-V virtualization
- BitLocker drive encryption
- Remote Desktop host
- Windows Sandbox
- Windows Defender Exploit Guard
- Windows Defender Application Control

#### Available on Home Edition:
- Device Encryption (BitLocker alternative)
- All required Windows APIs
- Service management
- Restart Manager
- Named pipes with security descriptors

### 5. Registry Paths Validation

Critical registry paths verified on Home editions:

```registry
# WSL Detection
HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows\CurrentVersion\Lxss
HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows\CurrentVersion\Lxss\<DistributionGUID>

# Windows Edition Detection
HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows NT\CurrentVersion\EditionID

# Device Encryption
HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Control\BitLocker\DeviceEncryption
```

## Fallback Implementations

### 1. HomeEditionCompat Module

The `home_edition_compat.rs` module provides:

```rust
// Automatic WSL2 VHDX compaction with fallbacks
pub async fn compact_wsl_vhdx(&self, distribution: &str) -> Result<WslCompactResult>

// Device Encryption detection
pub fn has_device_encryption(&self) -> bool

// Limitations and workarounds
pub fn get_limitations(&self) -> Vec<String>
pub fn get_workarounds(&self) -> Vec<String>
```

### 2. Validation Scripts

#### diskpart-compact-validation.ps1
- Creates test VHD
- Adds test data
- Compacts using diskpart
- Measures space savings
- Validates on Home editions

#### wsl-manage-validation.ps1
- Checks wsl --manage availability
- Tests alternative methods
- Validates wslconfig.exe functionality
- Documents build requirements

## Implementation Details

### WSL2 VHDX Path Detection

Multiple methods to find VHDX files:

1. **Registry**: `VhdxFilePath` value in distribution's registry key
2. **Package Path**: `%USERPROFILE%\AppData\Local\Packages\<PackageName>\LocalState\ext4.vhdx`
3. **Legacy Path**: `%USERPROFILE%\AppData\Local\WSL\<DistroName>\ext4.vhdx`

### Package Name Mapping

| Distribution | Package Name |
|--------------|--------------|
| Ubuntu | CanonicalGroupLimited.UbuntuonWindows |
| Ubuntu-20.04 | CanonicalGroupLimited.Ubuntu20.04onWindows |
| Ubuntu-22.04 | CanonicalGroupLimited.Ubuntu22.04onWindows |
| Debian | TheDebianProject.DebianGNULinux |
| Kali Linux | KaliLinux.KaliLinux |

### Error Handling

All fallback implementations include:
- Graceful degradation
- Clear error messages
- Manual instruction generation
- Audit logging of attempts

## Testing Results

### diskpart Compact Validation
- ✅ Works on Windows 10 Home (Build 19044)
- ✅ Works on Windows 11 Home (Build 22621)
- ✅ Average space savings: 10-40% depending on usage
- ✅ No administrator elevation required for user VHDs

### wsl --manage Validation
- ❌ Not available on builds <21364
- ✅ Available on Windows 11 22H2+ (Build 22621+)
- ⚠️ Limited functionality even when available
- ✅ wslconfig.exe provides basic management

## Recommendations

### For Users on Windows Home:

1. **WSL2 Management**: Use manual diskpart scripts for VHDX compaction
2. **VHD Operations**: All operations work through diskpart
3. **Security**: Device Encryption provides BitLocker-like functionality
4. **Updates**: Consider upgrading to Windows 11 for better WSL support

### For Developers:

1. Always check edition before using Pro-only features
2. Implement graceful fallbacks for critical operations
3. Provide clear instructions for manual operations
4. Test on both Home and Pro editions

## Future Considerations

### Phase 1 (CLI TUI):
- No Home edition limitations expected
- All terminal features available on Home

### Phase 2 (Package Managers):
- All package managers work on Home editions
- No elevation required for user-level caches

### Phase 3 (Windows Features):
- WSL2 compaction requires diskpart fallback
- Docker Desktop works on Home (requires WSL2)
- Windows Update cleanup works identically

### Phase 4 (GUI):
- Mica effects not available on Home (use solid brushes)
- Transparency effects need fallback
- All other UI features available

## Conclusion

WinSweep is fully compatible with Windows Home editions through comprehensive fallback implementations. The only limitations are:
1. WSL management requires manual steps or wslconfig.exe
2. No Hyper-V (not needed for WinSweep functionality)
3. GUI visual effects need solid brush fallbacks

All core disk cleaning functionality works identically across all Windows editions.

## Appendices

### A. Manual WSL2 Compaction Instructions

For users when automatic compaction fails:

```cmd
:: 1. Shutdown WSL
wsl --shutdown

:: 2. Open diskpart
diskpart

:: 3. In diskpart, run (replace path):
select vdisk file="%USERPROFILE%\AppData\Local\Packages\CanonicalGroupLimited.UbuntuonWindows\LocalState\ext4.vhdx"
attach vdisk readonly
compact vdisk
detach vdisk
exit

:: 4. Restart WSL
wsl
```

### B. Registry Commands for Edition Detection

```cmd
:: Check Windows Edition
reg query "HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion" /v EditionID

:: Check WSL Installation
reg query "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Lxss"

:: Check Device Encryption
reg query "HKLM\SYSTEM\CurrentControlSet\Control\BitLocker" /v DeviceEncryption
```

### C. Build Number Requirements

| Feature | Minimum Build | Home Edition Support |
|---------|---------------|---------------------|
| WSL2 | 18362 | ✅ |
| wsl --manage | 21364 | ⚠️ Limited |
| Device Encryption | 1703 | ✅ |
| WSL Package Management | 18362 | ✅ |

---

**Report prepared by**: WinSweep Development Team  
**Next Phase**: Phase 1 - CLI TUI Implementation
