#!/usr/bin/env pwsh
<#
.SYNOPSIS
    Validate diskpart compact vdisk functionality on Windows Home editions
.DESCRIPTION
    This script tests the diskpart compact vdisk workflow without Hyper-V,
    specifically for Windows Home edition compatibility validation.
.PARAMETER TestVhdPath
    Path to create test VHD/VHDX file (default: temp directory)
.PARAMETER TestSizeGB
    Size of test VHD in GB (default: 1)
.PARAMETER CleanupAfter
    Whether to clean up test files after validation (default: true)
.EXAMPLE
    .\validate-diskpart-compact.ps1 -TestVhdPath "C:\Temp\test.vhdx" -TestSizeGB 2
#>

param(
    [Parameter()]
    [string]$TestVhdPath = "",
    
    [Parameter()]
    [int]$TestSizeGB = 1,
    
    [Parameter()]
    [bool]$CleanupAfter = $true
)

# Ensure running as administrator
if (-NOT ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole] "Administrator")) {
    Write-Error "This script must be run as Administrator"
    exit 1
}

Write-Host "=== WinSweep Diskpart Compact VDisk Validation ===" -ForegroundColor Cyan
Write-Host "Windows Edition: $((Get-WmiObject -class Win32_OperatingSystem).Caption)"
Write-Host "Build Number: $((Get-WmiObject -class Win32_OperatingSystem).BuildNumber)"
Write-Host ""

# Initialize test environment
if ([string]::IsNullOrEmpty($TestVhdPath)) {
    $tempDir = Join-Path $env:TEMP "WinSweep-Diskpart-Test"
    New-Item -ItemType Directory -Force -Path $tempDir | Out-Null
    $TestVhdPath = Join-Path $tempDir "test-disk.vhdx"
}

Write-Host "Test VHD Path: $TestVhdPath"
Write-Host "Test Size: $TestSizeGB GB"
Write-Host ""

# Test 1: Check diskpart availability
Write-Host "Test 1: Checking diskpart availability..." -ForegroundColor Yellow
try {
    $diskpartVersion = & diskpart.exe /? 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Host "✓ diskpart.exe is available" -ForegroundColor Green
    } else {
        Write-Host "✗ diskpart.exe not available" -ForegroundColor Red
        exit 1
    }
} catch {
    Write-Host "✗ Failed to run diskpart.exe: $_" -ForegroundColor Red
    exit 1
}

# Test 2: Create test VHD
Write-Host "Test 2: Creating test VHD..." -ForegroundColor Yellow
try {
    $diskpartScript = @"
create vdisk file="$TestVhdPath" maximum=$TestSizeGB type=expandable
select vdisk file="$TestVhdPath"
attach vdisk
create partition primary
format fs=ntfs quick assign
exit
"@
    
    $diskpartScript | Out-File -FilePath "$env:TEMP\create-vhd.txt" -Encoding ASCII
    & diskpart.exe /s "$env:TEMP\create-vhd.txt" | Out-Null
    
    if ($LASTEXITCODE -eq 0 -and (Test-Path $TestVhdPath)) {
        $vhdSize = (Get-Item $TestVhdPath).Length / 1GB
        Write-Host "✓ VHD created successfully (Size: $([math]::Round($vhdSize, 2)) GB)" -ForegroundColor Green
    } else {
        Write-Host "✗ Failed to create VHD" -ForegroundColor Red
        exit 1
    }
} catch {
    Write-Host "✗ Failed to create VHD: $_" -ForegroundColor Red
    exit 1
}

# Test 3: Add test data to VHD
Write-Host "Test 3: Adding test data to VHD..." -ForegroundColor Yellow
try {
    $driveLetter = (Get-Volume | Where-Object { $_.FileSystemLabel -like "*test*" -or $_.Size -eq ($TestSizeGB * 1GB) }).DriveLetter
    
    if ([string]::IsNullOrEmpty($driveLetter)) {
        # Try to find the newly attached volume
        $driveLetter = (Get-Volume | Sort-Object -Property DriveLetter | Select-Object -Last 1).DriveLetter
    }
    
    if ($driveLetter) {
        $testDataPath = "${driveLetter}:\winsweep-test-data"
        New-Item -ItemType Directory -Force -Path $testDataPath | Out-Null
        
        # Create test files
        for ($i = 1; $i -le 100; $i++) {
            "Test data file $i - $(Get-Date)" | Out-File -FilePath "$testDataPath\test$i.txt"
        }
        
        Write-Host "✓ Test data added to VHD (Drive $driveLetter)" -ForegroundColor Green
    } else {
        Write-Host "⚠ Could not determine VHD drive letter, skipping data test" -ForegroundColor Yellow
    }
} catch {
    Write-Host "✗ Failed to add test data: $_" -ForegroundColor Red
}

# Test 4: Detach VHD
Write-Host "Test 4: Detaching VHD..." -ForegroundColor Yellow
try {
    $diskpartScript = @"
select vdisk file="$TestVhdPath"
detach vdisk
exit
"@
    
    $diskpartScript | Out-File -FilePath "$env:TEMP\detach-vhd.txt" -Encoding ASCII
    & diskpart.exe /s "$env:TEMP\detach-vhd.txt" | Out-Null
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "✓ VHD detached successfully" -ForegroundColor Green
    } else {
        Write-Host "✗ Failed to detach VHD" -ForegroundColor Red
        exit 1
    }
} catch {
    Write-Host "✗ Failed to detach VHD: $_" -ForegroundColor Red
    exit 1
}

# Test 5: Get initial VHD size
Write-Host "Test 5: Measuring initial VHD size..." -ForegroundColor Yellow
$initialSize = (Get-Item $TestVhdPath).Length
Write-Host "Initial VHD size: $([math]::Round($initialSize / 1MB, 2)) MB" -ForegroundColor Cyan

# Test 6: Compact VHD
Write-Host "Test 6: Compacting VHD..." -ForegroundColor Yellow
try {
    $diskpartScript = @"
select vdisk file="$TestVhdPath"
attach vdisk readonly
compact vdisk
detach vdisk
exit
"@
    
    $diskpartScript | Out-File -FilePath "$env:TEMP\compact-vhd.txt" -Encoding ASCII
    $compactOutput = & diskpart.exe /s "$env:TEMP\compact-vhd.txt" 2>&1
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "✓ VHD compact command executed successfully" -ForegroundColor Green
        Write-Host "Compact output: $compactOutput" -ForegroundColor Cyan
    } else {
        Write-Host "✗ VHD compact command failed" -ForegroundColor Red
        Write-Host "Error output: $compactOutput" -ForegroundColor Red
        exit 1
    }
} catch {
    Write-Host "✗ Failed to compact VHD: $_" -ForegroundColor Red
    exit 1
}

# Test 7: Measure final VHD size
Write-Host "Test 7: Measuring final VHD size..." -ForegroundColor Yellow
$finalSize = (Get-Item $TestVhdPath).Length
$spaceSaved = $initialSize - $finalSize
$percentSaved = if ($initialSize -gt 0) { ($spaceSaved / $initialSize) * 100 } else { 0 }

Write-Host "Final VHD size: $([math]::Round($finalSize / 1MB, 2)) MB" -ForegroundColor Cyan
Write-Host "Space saved: $([math]::Round($spaceSaved / 1MB, 2)) MB ($([math]::Round($percentSaved, 2))%)" -ForegroundColor Green

# Test 8: Test alternative compact method (for Home edition)
Write-Host "Test 8: Testing alternative compact method..." -ForegroundColor Yellow
try {
    # PowerShell method for Home edition (if available)
    $psVersion = $PSVersionTable.PSVersion.Major
    if ($psVersion -ge 5) {
        Write-Host "PowerShell $psVersion detected, testing Optimize-VHD..." -ForegroundColor Cyan
        
        # This might not work on Home edition, but worth testing
        try {
            $optimizeResult = Optimize-VHD -Path $TestVhdPath -Mode Full -ErrorAction Stop
            Write-Host "✓ Optimize-VHD succeeded (PowerShell method available)" -ForegroundColor Green
        } catch {
            Write-Host "⚠ Optimize-VHD not available: $_" -ForegroundColor Yellow
            Write-Host "This is expected on Windows Home editions" -ForegroundColor Cyan
        }
    }
} catch {
    Write-Host "⚠ PowerShell optimization test failed: $_" -ForegroundColor Yellow
}

# Generate report
Write-Host ""
Write-Host "=== Validation Report ===" -ForegroundColor Cyan
Write-Host "Windows Edition: $((Get-WmiObject -class Win32_OperatingSystem).Caption)"
Write-Host "PowerShell Version: $($PSVersionTable.PSVersion)"
Write-Host "Initial VHD Size: $([math]::Round($initialSize / 1MB, 2)) MB"
Write-Host "Final VHD Size: $([math]::Round($finalSize / 1MB, 2)) MB"
Write-Host "Space Saved: $([math]::Round($spaceSaved / 1MB, 2)) MB ($([math]::Round($percentSaved, 2))%)"

# Determine compatibility
Write-Host ""
Write-Host "=== Compatibility Assessment ===" -ForegroundColor Cyan

if ($spaceSaved -gt 0) {
    Write-Host "✓ diskpart compact vdisk is functional" -ForegroundColor Green
    $isCompatible = $true
} else {
    Write-Host "✗ diskpart compact vdisk did not save space" -ForegroundColor Red
    $isCompatible = $false
}

# Check for Home edition specific limitations
$osCaption = (Get-WmiObject -class Win32_OperatingSystem).Caption
if ($osCaption -like "*Home*") {
    Write-Host ""
    Write-Host "Windows Home Edition Notes:" -ForegroundColor Yellow
    Write-Host "- diskpart compact vdisk is available but may require manual steps" -ForegroundColor Cyan
    Write-Host "- Optimize-VHD cmdlet is not available on Home edition" -ForegroundColor Cyan
    Write-Host "- Consider using diskpart script-based approach for automation" -ForegroundColor Cyan
}

# Cleanup
if ($CleanupAfter) {
    Write-Host ""
    Write-Host "Cleaning up test files..." -ForegroundColor Yellow
    try {
        Remove-Item -Path $TestVhdPath -Force -ErrorAction SilentlyContinue
        Remove-Item -Path "$env:TEMP\create-vhd.txt" -Force -ErrorAction SilentlyContinue
        Remove-Item -Path "$env:TEMP\detach-vhd.txt" -Force -ErrorAction SilentlyContinue
        Remove-Item -Path "$env:TEMP\compact-vhd.txt" -Force -ErrorAction SilentlyContinue
        Remove-Item -Path $tempDir -Recurse -Force -ErrorAction SilentlyContinue
        Write-Host "✓ Cleanup completed" -ForegroundColor Green
    } catch {
        Write-Host "⚠ Some cleanup failed: $_" -ForegroundColor Yellow
    }
}

# Exit with appropriate code
if ($isCompatible) {
    Write-Host ""
    Write-Host "=== Validation PASSED ===" -ForegroundColor Green
    exit 0
} else {
    Write-Host ""
    Write-Host "=== Validation FAILED ===" -ForegroundColor Red
    exit 1
}
