#!/usr/bin/env pwsh
<#
.SYNOPSIS
    Signs WinSweep binaries with EV certificate
.DESCRIPTION
    This script signs WinSweep executables and DLLs with an EV certificate
    for distribution on Windows platforms.
.PARAMETER BuildPath
    Path to the build output directory
.PARAMETER CertificateThumbprint
    Thumbprint of the EV certificate to use for signing
.PARAMETER TimestampServer
    Timestamp server URL (default: http://timestamp.digicert.com)
.EXAMPLE
    .\sign-build.ps1 -BuildPath "target\release" -CertificateThumbprint "1234567890ABCDEF1234567890ABCDEF12345678"
#>

param(
    [Parameter(Mandatory=$true)]
    [string]$BuildPath,
    
    [Parameter(Mandatory=$true)]
    [string]$CertificateThumbprint,
    
    [Parameter()]
    [string]$TimestampServer = "http://timestamp.digicert.com"
)

# Ensure we're running on Windows
if ($IsLinux -or $IsMacOS) {
    Write-Error "This script must be run on Windows"
    exit 1
}

# Check if signtool is available
$signtool = Get-Command "signtool.exe" -ErrorAction SilentlyContinue
if (-not $signtool) {
    # Try common SDK paths
    $sdkPaths = @(
        "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe",
        "${env:ProgramFiles}\Windows Kits\10\bin\*\x64\signtool.exe"
    )
    
    foreach ($path in $sdkPaths) {
        $signtool = Get-ChildItem -Path $path -ErrorAction SilentlyContinue | Sort-Object Name -Descending | Select-Object -First 1
        if ($signtool) {
            break
        }
    }
    
    if (-not $signtool) {
        Write-Error "signtool.exe not found. Please install Windows SDK."
        exit 1
    }
}

Write-Host "Using signtool at: $($signtool.FullName)"

# Get all binaries to sign
$binaries = Get-ChildItem -Path $BuildPath -Include "*.exe", "*.dll" -Recurse

if ($binaries.Count -eq 0) {
    Write-Warning "No binaries found to sign in $BuildPath"
    exit 0
}

Write-Host "Found $($binaries.Count) binaries to sign"

# Sign each binary
foreach ($binary in $binaries) {
    Write-Host "Signing: $($binary.FullName)"
    
    $arguments = @(
        "sign",
        "/sha1", $CertificateThumbprint,
        "/tr", $TimestampServer,
        "/td", "sha256",
        "/fd", "sha256",
        "/a",
        $binary.FullName
    )
    
    $process = Start-Process -FilePath $signtool.FullName -ArgumentList $arguments -Wait -PassThru -NoNewWindow
    
    if ($process.ExitCode -ne 0) {
        Write-Error "Failed to sign $($binary.FullName). Exit code: $($process.ExitCode)"
        exit 1
    }
    
    # Verify the signature
    $verifyArguments = @(
        "verify",
        "/pa",
        $binary.FullName
    )
    
    $verifyProcess = Start-Process -FilePath $signtool.FullName -ArgumentList $verifyArguments -Wait -PassThru -NoNewWindow
    
    if ($verifyProcess.ExitCode -ne 0) {
        Write-Error "Signature verification failed for $($binary.FullName)"
        exit 1
    }
    
    Write-Host "Successfully signed and verified: $($binary.Name)" -ForegroundColor Green
}

Write-Host "All binaries signed successfully!" -ForegroundColor Green
