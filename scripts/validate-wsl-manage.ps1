#!/usr/bin/env pwsh
<#
.SYNOPSIS
    Validate wsl --manage command availability on Windows editions
.DESCRIPTION
    This script tests wsl --manage command availability on different Windows builds,
    particularly for Windows Home edition compatibility validation.
.PARAMETER TestDistribution
    Name of distribution to test with (default: auto-detect)
.EXAMPLE
    .\validate-wsl-manage.ps1 -TestDistribution "Ubuntu"
#>

param(
    [Parameter()]
    [string]$TestDistribution = ""
)

Write-Host "=== WinSweep WSL --manage Validation ===" -ForegroundColor Cyan
Write-Host "Windows Edition: $((Get-WmiObject -class Win32_OperatingSystem).Caption)"
Write-Host "Build Number: $((Get-WmiObject -class Win32_OperatingSystem).BuildNumber)"
Write-Host "PowerShell Version: $($PSVersionTable.PSVersion)"
Write-Host ""

# Test 1: Check if WSL is installed
Write-Host "Test 1: Checking WSL installation..." -ForegroundColor Yellow
$wslExe = Get-Command wsl.exe -ErrorAction SilentlyContinue

if ($wslExe) {
    Write-Host "✓ wsl.exe found at $($wslExe.Source)" -ForegroundColor Green
    $wslVersion = & wsl.exe --version 2>&1
    Write-Host "WSL Version: $wslVersion" -ForegroundColor Cyan
} else {
    Write-Host "✗ wsl.exe not found in PATH" -ForegroundColor Red
    Write-Host "WSL is not installed or not in PATH" -ForegroundColor Red
    exit 1
}

# Test 2: Check basic WSL functionality
Write-Host "Test 2: Checking basic WSL functionality..." -ForegroundColor Yellow
try {
    $wslStatus = & wsl.exe --status 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Host "✓ wsl --status works" -ForegroundColor Green
        Write-Host $wslStatus -ForegroundColor Cyan
    } else {
        Write-Host "✗ wsl --status failed" -ForegroundColor Red
    }
} catch {
    Write-Host "✗ Failed to run wsl --status: $_" -ForegroundColor Red
}

# Test 3: Check if --manage command is available
Write-Host "Test 3: Checking wsl --manage availability..." -ForegroundColor Yellow
try {
    $manageHelp = & wsl.exe --manage --help 2>&1
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "✓ wsl --manage command is available" -ForegroundColor Green
        Write-Host "Manage help output:" -ForegroundColor Cyan
        Write-Host $manageHelp -ForegroundColor Gray
        $hasManage = $true
    } else {
        Write-Host "✗ wsl --manage command not available" -ForegroundColor Red
        Write-Host "Error: $manageHelp" -ForegroundColor Red
        $hasManage = $false
    }
} catch {
    Write-Host "✗ Failed to run wsl --manage: $_" -ForegroundColor Red
    $hasManage = $false
}

# Test 4: Check Windows build for --manage support
Write-Host "Test 4: Checking Windows build requirements..." -ForegroundColor Yellow
$buildNumber = [int](Get-WmiObject -class Win32_OperatingSystem).BuildNumber
$requiredBuild = 21364

if ($buildNumber -ge $requiredBuild) {
    Write-Host "✓ Windows build $buildNumber meets requirement (>= $requiredBuild)" -ForegroundColor Green
    $buildSupportsManage = $true
} else {
    Write-Host "✗ Windows build $buildNumber is too old (requires >= $requiredBuild)" -ForegroundColor Red
    $buildSupportsManage = $false
}

# Test 5: List available distributions
Write-Host "Test 5: Listing available distributions..." -ForegroundColor Yellow
try {
    $wslList = & wsl.exe -l -v 2>&1
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "✓ wsl -l -v works" -ForegroundColor Green
        Write-Host $wslList -ForegroundColor Cyan
        
        # Parse distributions
        $distributions = @()
        $lines = $wslList -split "`n"
        foreach ($line in $lines[1..($lines.Length - 1)]) { # Skip header
            if ($line.Trim() -ne "") {
                $parts = $line -split "\s+"
                if ($parts.Length -ge 3) {
                    $distributions += @{
                        Name = $parts[0]
                        State = $parts[1]
                        Version = $parts[2]
                        Default = if ($parts.Length -gt 3 -and $parts[3] -eq "*") { $true } else { $false }
                    }
                }
            }
        }
        
        Write-Host "Found $($distributions.Length) distributions" -ForegroundColor Cyan
    } else {
        Write-Host "✗ wsl -l -v failed" -ForegroundColor Red
    }
} catch {
    Write-Host "✗ Failed to list distributions: $_" -ForegroundColor Red
}

# Test 6: Test --manage functionality if available
if ($hasManage) {
    Write-Host "Test 6: Testing wsl --manage functionality..." -ForegroundColor Yellow
    
    # Test 6a: List distributions with --manage
    try {
        $manageList = & wsl.exe --manage --list 2>&1
        
        if ($LASTEXITCODE -eq 0) {
            Write-Host "✓ wsl --manage --list works" -ForegroundColor Green
            Write-Host $manageList -ForegroundColor Cyan
        } else {
            Write-Host "✗ wsl --manage --list failed" -ForegroundColor Red
        }
    } catch {
        Write-Host "✗ Failed to run wsl --manage --list: $_" -ForegroundColor Red
    }
    
    # Test 6b: Check --manage options
    try {
        $manageOptions = & wsl.exe --manage --help 2>&1
        $options = @()
        
        if ($manageOptions -match "--shutdown") { $options += "shutdown" }
        if ($manageOptions -match "--move") { $options += "move" }
        if ($manageOptions -match "--set-version") { $options += "set-version" }
        if ($manageOptions -match "--set-default") { $options += "set-default" }
        
        Write-Host "Available --manage options: $($options -join ', ')" -ForegroundColor Cyan
    } catch {
        Write-Host "⚠ Could not parse --manage options" -ForegroundColor Yellow
    }
}

# Test 7: Check for alternative management methods
Write-Host "Test 7: Checking alternative WSL management methods..." -ForegroundColor Yellow

# Check for wslconfig.exe
$wslconfig = Get-Command wslconfig.exe -ErrorAction SilentlyContinue
if ($wslconfig) {
    Write-Host "✓ wslconfig.exe found at $($wslconfig.Source)" -ForegroundColor Green
    try {
        $configHelp = & wslconfig.exe /? 2>&1
        Write-Host "wslconfig options available" -ForegroundColor Cyan
    } catch {
        Write-Host "⚠ wslconfig.exe found but failed to run" -ForegroundColor Yellow
    }
} else {
    Write-Host "✗ wslconfig.exe not found" -ForegroundColor Red
}

# Check for PowerShell WSL module
$wslModule = Get-Module -ListAvailable -Name "WSL" -ErrorAction SilentlyContinue
if ($wslModule) {
    Write-Host "✓ PowerShell WSL module found" -ForegroundColor Green
} else {
    Write-Host "⚠ PowerShell WSL module not found" -ForegroundColor Yellow
}

# Test 8: Registry check for WSL management features
Write-Host "Test 8: Checking registry for WSL management features..." -ForegroundColor Yellow
try {
    $registryPaths = @(
        "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Lxss",
        "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\AppModel\StateRepository"
    )
    
    foreach ($path in $registryPaths) {
        if (Test-Path $path) {
            Write-Host "✓ Registry key found: $path" -ForegroundColor Green
            
            # Check for management-related values
            $properties = Get-ItemProperty $path -ErrorAction SilentlyContinue
            if ($properties) {
                $mgmtProps = $properties.PSObject.Properties | Where-Object { 
                    $_.Name -like "*Manage*" -or 
                    $_.Name -like "*Config*" -or 
                    $_.Name -like "*Control*"
                }
                
                if ($mgmtProps) {
                    Write-Host "  Management-related properties found:" -ForegroundColor Cyan
                    foreach ($prop in $mgmtProps) {
                        Write-Host "    - $($prop.Name)" -ForegroundColor Gray
                    }
                }
            }
        } else {
            Write-Host "✗ Registry key not found: $path" -ForegroundColor Red
        }
    }
} catch {
    Write-Host "✗ Registry check failed: $_" -ForegroundColor Red
}

# Generate compatibility report
Write-Host ""
Write-Host "=== Compatibility Report ===" -ForegroundColor Cyan

Write-Host "Windows Build: $buildNumber"
Write-Host "WSL Available: Yes"
Write-Host "wsl --manage Available: $hasManage"
Write-Host "Build Supports --manage: $buildSupportsManage"

# Determine compatibility status
Write-Host ""
Write-Host "=== Compatibility Assessment ===" -ForegroundColor Cyan

if ($hasManage -and $buildSupportsManage) {
    Write-Host "✓ Full WSL management support available" -ForegroundColor Green
    Write-Host "  - Use wsl --manage for advanced operations" -ForegroundColor Cyan
    $compatibility = "Full"
} elseif ($buildSupportsManage -and !$hasManage) {
    Write-Host "⚠ WSL --manage should be available but not working" -ForegroundColor Yellow
    Write-Host "  - May need Windows update or WSL update" -ForegroundColor Cyan
    $compatibility = "Partial"
} elseif (!$buildSupportsManage) {
    Write-Host "⚠ Windows build does not support wsl --manage" -ForegroundColor Yellow
    Write-Host "  - Use alternative methods (wslconfig.exe, direct commands)" -ForegroundColor Cyan
    $compatibility = "Limited"
} else {
    Write-Host "✗ WSL management not available" -ForegroundColor Red
    $compatibility = "None"
}

# Home edition specific notes
$osCaption = (Get-WmiObject -class Win32_OperatingSystem).Caption
if ($osCaption -like "*Home*") {
    Write-Host ""
    Write-Host "Windows Home Edition Notes:" -ForegroundColor Yellow
    if ($compatibility -eq "Full") {
        Write-Host "- wsl --manage is available on this build" -ForegroundColor Green
    } else {
        Write-Host "- wsl --manage may not be available on Home edition" -ForegroundColor Cyan
        Write-Host "- Use wslconfig.exe for basic management" -ForegroundColor Cyan
        Write-Host "- Consider upgrading to Windows 11 for better WSL support" -ForegroundColor Cyan
    }
}

Write-Host ""
Write-Host "=== Validation Results ===" -ForegroundColor Cyan
Write-Host "Compatibility Status: $compatibility"
Write-Host "Recommended Management Method: $(if ($hasManage) { 'wsl --manage' } elseif ($wslconfig) { 'wslconfig.exe' } else { 'Direct wsl commands' })"

# Exit with appropriate code
if ($compatibility -eq "Full") {
    Write-Host ""
    Write-Host "=== Validation PASSED ===" -ForegroundColor Green
    exit 0
} elseif ($compatibility -eq "Partial" -or $compatibility -eq "Limited") {
    Write-Host ""
    Write-Host "=== Validation PASSED with Limitations ===" -ForegroundColor Yellow
    exit 0
} else {
    Write-Host ""
    Write-Host "=== Validation FAILED ===" -ForegroundColor Red
    exit 1
}
