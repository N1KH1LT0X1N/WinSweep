# WinSweep Build Script
# 
# This script builds all WinSweep components and creates release packages

param(
    [switch]$Release = $false,
    [switch]$Test = $false,
    [switch]$Package = $false,
    [switch]$Clean = $false,
    [switch]$InstallDeps = $false,
    [string]$Target = "all"
)

$ErrorActionPreference = "Stop"

# Color output functions
function Write-ColorOutput($ForegroundColor) {
    $fc = $host.UI.RawUI.ForegroundColor
    $host.UI.RawUI.ForegroundColor = $ForegroundColor
    if ($args) {
        Write-Output $args
    } else {
        $input | Write-Output
    }
    $host.UI.RawUI.ForegroundColor = $fc
}

function Write-Success($message) {
    Write-ColorOutput Green "✓ $message"
}

function Write-Error($message) {
    Write-ColorOutput Red "✗ $message"
}

function Write-Warning($message) {
    Write-ColorOutput Yellow "⚠ $message"
}

function Write-Info($message) {
    Write-ColorOutput Cyan "ℹ $message"
}

# Check prerequisites
function Test-Prerequisites {
    Write-Info "Checking prerequisites..."
    
    # Check Rust installation
    try {
        $rustVersion = rustc --version 2>$null
        Write-Success "Rust installed: $rustVersion"
    }
    catch {
        Write-Error "Rust not found. Please install Rust from https://rustup.rs/"
        exit 1
    }
    
    # Check Visual Studio Build Tools
    try {
        $linker = where.exe link.exe 2>$null
        if ($linker) {
            Write-Success "Visual Studio Build Tools found"
        }
        else {
            Write-Warning "Visual Studio Build Tools not found in PATH"
            Write-Info "Attempting to locate Visual Studio..."
            
            # Try to find VS installation
            $vsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
            if (Test-Path $vsWhere) {
                $vsPath = & $vsWhere -latest -property installationPath
                if ($vsPath) {
                    $vcVarsAll = "$vsPath\VC\Auxiliary\Build\vcvarsall.bat"
                    if (Test-Path $vcVarsAll) {
                        Write-Success "Found Visual Studio at: $vsPath"
                        Write-Info "You may need to run: `"$vcVarsAll`" x64"
                        return $vcVarsAll
                    }
                }
            }
            
            Write-Error "Visual Studio Build Tools with C++ tools required"
            Write-Info "Download from: https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022"
            exit 1
        }
    }
    catch {
        Write-Error "Failed to check Visual Studio Build Tools"
        exit 1
    }
    
    return $null
}

# Install dependencies
function Install-Dependencies {
    Write-Info "Installing Rust dependencies..."
    
    if ($Target -eq "all" -or $Target -eq "cli") {
        cargo fetch --manifest-path crates/winsweep-cli/Cargo.toml
    }
    
    if ($Target -eq "all" -or $Target -eq "gui") {
        cargo fetch --manifest-path crates/winsweep-gui/Cargo.toml
    }
    
    if ($Target -eq "all" -or $Target -eq "core") {
        cargo fetch --manifest-path crates/winsweep-core/Cargo.toml
    }
    
    Write-Success "Dependencies installed"
}

# Clean build artifacts
function Clean-Build {
    Write-Info "Cleaning build artifacts..."
    
    cargo clean
    if (Test-Path "dist") {
        Remove-Item -Recurse -Force "dist"
    }
    
    Write-Success "Clean completed"
}

# Run tests
function Run-Tests {
    Write-Info "Running tests..."
    
    $envRustBacktrace = "1"
    
    # Run unit tests for all crates
    $testTargets = @()
    if ($Target -eq "all" -or $Target -eq "core") { $testTargets += "winsweep-core" }
    if ($Target -eq "all" -or $Target -eq "cli") { $testTargets += "winsweep-cli" }
    if ($Target -eq "all" -or $Target -eq "gui") { $testTargets += "winsweep-gui" }
    
    foreach ($testTarget in $testTargets) {
        Write-Info "Testing $testTarget..."
        cargo test -p $testTarget --all-features
        if ($LASTEXITCODE -ne 0) {
            Write-Error "Tests failed for $testTarget"
            exit 1
        }
    }
    
    # Run integration tests
    Write-Info "Running integration tests..."
    cargo test --test integration_tests --all-features
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Integration tests failed"
        exit 1
    }
    
    Write-Success "All tests passed"
}

# Build binaries
function Build-Binaries {
    Write-Info "Building binaries..."
    
    $buildArgs = @()
    if ($Release) { $buildArgs += "--release" }
    
    # Build CLI
    if ($Target -eq "all" -or $Target -eq "cli") {
        Write-Info "Building CLI..."
        cargo build $buildArgs -p winsweep-cli
        if ($LASTEXITCODE -ne 0) {
            Write-Error "CLI build failed"
            exit 1
        }
    }
    
    # Build GUI
    if ($Target -eq "all" -or $Target -eq "gui") {
        Write-Info "Building GUI..."
        cargo build $buildArgs -p winsweep-gui
        if ($LASTEXITCODE -ne 0) {
            Write-Error "GUI build failed"
            exit 1
        }
    }
    
    # Build Core library
    if ($Target -eq "all" -or $Target -eq "core") {
        Write-Info "Building Core library..."
        cargo build $buildArgs -p winsweep-core
        if ($LASTEXITCODE -ne 0) {
            Write-Error "Core build failed"
            exit 1
        }
    }
    
    Write-Success "Build completed"
}

# Create release packages
function New-Packages {
    Write-Info "Creating release packages..."
    
    if (-not $Release) {
        Write-Warning "Package creation requires -Release flag"
        return
    }
    
    $version = (cargo metadata --no-deps --format-version 1 | ConvertFrom-Json).packages[0].version
    $distDir = "dist\winsweep-$version"
    
    if (Test-Path $distDir) {
        Remove-Item -Recurse -Force $distDir
    }
    New-Item -ItemType Directory -Path $distDir -Force | Out-Null
    
    # Copy CLI binary
    if ($Target -eq "all" -or $Target -eq "cli") {
        $cliPath = "target\release\winsweep-cli.exe"
        if (Test-Path $cliPath) {
            Copy-Item $cliPath "$distDir\winsweep-cli.exe"
            Write-Success "CLI binary packaged"
        }
    }
    
    # Copy GUI binary
    if ($Target -eq "all" -or $Target -eq "gui") {
        $guiPath = "target\release\winsweep-gui.exe"
        if (Test-Path $guiPath) {
            Copy-Item $guiPath "$distDir\winsweep-gui.exe"
            Write-Success "GUI binary packaged"
        }
    }
    
    # Copy documentation
    Copy-Item README.md "$distDir\"
    Copy-Item LICENSE "$distDir\" -ErrorAction SilentlyContinue
    if (Test-Path "docs") {
        Copy-Item "docs\*" "$distDir\docs\" -Recurse -ErrorAction SilentlyContinue
    }
    
    # Create ZIP archive
    $zipPath = "dist\winsweep-$version-windows-x64.zip"
    if (Test-Path $zipPath) {
        Remove-Item $zipPath
    }
    Compress-Archive -Path "$distDir\*" -DestinationPath $zipPath
    
    Write-Success "Package created: $zipPath"
    
    # Calculate checksums
    $checksum = Get-FileHash -Path $zipPath -Algorithm SHA256
    $checksum | Out-File "$zipPath.sha256"
    Write-Info "SHA256: $($checksum.Hash)"
}

# Main execution
function Main {
    Write-Info "WinSweep Build Script"
    Write-Info "===================="
    
    Push-Location $PSScriptRoot
    
    try {
        # Check prerequisites
        $vcVarsAll = Test-Prerequisites
        
        # Setup Visual Studio environment if needed
        if ($vcVarsAll -and (Get-Command link.exe -ErrorAction SilentlyContinue) -eq $null) {
            Write-Info "Setting up Visual Studio environment..."
            cmd /c "`"$vcVarsAll`" x64 && set" | ForEach-Object {
                if ($_ -match "^(.+?)=(.*)$") {
                    [Environment]::SetEnvironmentVariable($matches[1], $matches[2])
                }
            }
        }
        
        # Install dependencies if requested
        if ($InstallDeps) {
            Install-Dependencies
        }
        
        # Clean if requested
        if ($Clean) {
            Clean-Build
        }
        
        # Build
        Build-Binaries
        
        # Run tests if requested
        if ($Test) {
            Run-Tests
        }
        
        # Create packages if requested
        if ($Package) {
            New-Packages
        }
        
        Write-Success "Build script completed successfully!"
    }
    catch {
        Write-Error "Build failed: $($_.Exception.Message)"
        exit 1
    }
    finally {
        Pop-Location
    }
}

# Run main function
Main
