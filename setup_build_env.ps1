# Setup Windows SDK for Rust compilation

Write-Host "Windows SDK Setup for WinSweep" -ForegroundColor Cyan
Write-Host "=============================" -ForegroundColor Cyan

# Check if Windows SDK is installed
$sdkPath = "${env:ProgramFiles(x86)}\Windows Kits\10"
if (-not (Test-Path $sdkPath)) {
    Write-Error "Windows SDK 10 not found at $sdkPath"
    Write-Host "Please install Windows 10/11 SDK from:" -ForegroundColor Yellow
    Write-Host "https://developer.microsoft.com/en-us/windows/downloads/windows-sdk/" -ForegroundColor Yellow
    Write-Host "Or run Visual Studio Installer and modify your installation to include Windows 10/11 SDK" -ForegroundColor Yellow
    exit 1
}

Write-Host "Found Windows SDK at: $sdkPath" -ForegroundColor Green

# Find the latest SDK version
$sdkVersions = Get-ChildItem $sdkPath\Lib | Where-Object { $_.Name -match '^\d+\.\d+\.\d+\.\d+$' } | Sort-Object Name -Descending
if (-not $sdkVersions) {
    Write-Error "No valid SDK versions found"
    exit 1
}

$latestSdk = $sdkVersions[0].Name
Write-Host "Latest SDK version: $latestSdk" -ForegroundColor Green

# Find Visual Studio installation
$vsPath = "${env:ProgramFiles}\Microsoft Visual Studio\2022\Community"
if (-not (Test-Path $vsPath)) {
    $vsPath = "${env:ProgramFiles}\Microsoft Visual Studio\2022\Enterprise"
}
if (-not (Test-Path $vsPath)) {
    $vsPath = "${env:ProgramFiles}\Microsoft Visual Studio\2022\Professional"
}

if (-not (Test-Path $vsPath)) {
    Write-Error "Visual Studio 2022 not found"
    exit 1
}

Write-Host "Found Visual Studio at: $vsPath" -ForegroundColor Green

# Set up environment variables for this session
$vcToolsPath = Get-ChildItem "$vsPath\VC\Tools\MSVC" | Sort-Object Name -Descending | Select-Object -First 1
$vcVersion = $vcToolsPath.Name

Write-Host "VC Tools version: $vcVersion" -ForegroundColor Green

# Set LIB path
$libPaths = @(
    "$vcToolsPath\lib\x64",
    "$sdkPath\Lib\$latestSdk\um\x64",
    "$sdkPath\Lib\$latestSdk\ucrt\x64"
)
$env:LIB = $libPaths -join ";"

# Set INCLUDE path
$includePaths = @(
    "$vcToolsPath\include",
    "$sdkPath\Include\$latestSdk\um",
    "$sdkPath\Include\$latestSdk\ucrt",
    "$sdkPath\Include\$latestSdk\shared"
)
$env:INCLUDE = $includePaths -join ";"

# Set PATH for tools
$env:PATH = "$vcToolsPath\bin\HostX64\x64;$env:PATH"

Write-Host "Environment variables set" -ForegroundColor Green

# Test compilation
Write-Host "Testing compilation..." -ForegroundColor Cyan
& cargo check --all-targets

if ($LASTEXITCODE -eq 0) {
    Write-Host "Success! WinSweep compiles correctly." -ForegroundColor Green
    Write-Host "You can now run:" -ForegroundColor Cyan
    Write-Host "  cargo build --release" -ForegroundColor Gray
    Write-Host "  cargo test" -ForegroundColor Gray
} else {
    Write-Host "Compilation failed. You may need to:" -ForegroundColor Red
    Write-Host "  1. Restart your terminal" -ForegroundColor Yellow
    Write-Host "  2. Run this script again" -ForegroundColor Yellow
    Write-Host "  3. Use Developer Command Prompt for VS 2022 from Start Menu" -ForegroundColor Yellow
}
