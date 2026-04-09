# Fix Visual Studio Build Environment

Write-Host "Fixing Visual Studio build environment..." -ForegroundColor Cyan

# Find Visual Studio installation
$vsPath = "${env:ProgramFiles}\Microsoft Visual Studio\2022\Community"
if (-not (Test-Path $vsPath)) {
    $vsPath = "${env:ProgramFiles}\Microsoft Visual Studio\2022\Enterprise"
}
if (-not (Test-Path $vsPath)) {
    $vsPath = "${env:ProgramFiles}\Microsoft Visual Studio\2022\Professional"
}

$vcVarsAll = "$vsPath\VC\Auxiliary\Build\vcvarsall.bat"

if (-not (Test-Path $vcVarsAll)) {
    Write-Error "Visual Studio not found!"
    exit 1
}

# Find Windows SDK
$sdkPath = "${env:ProgramFiles(x86)}\Windows Kits\10"
if (Test-Path $sdkPath) {
    # Get latest SDK version
    $sdkVersions = Get-ChildItem $sdkPath\Lib | Where-Object { $_.Name -match '^\d+\.\d+\.\d+\.\d+$' } | Sort-Object Name -Descending
    if ($sdkVersions) {
        $latestSdk = $sdkVersions[0].Name
        Write-Host "Found Windows SDK: $latestSdk" -ForegroundColor Green
        
        # Set LIB environment variable
        $libPaths = @(
            "$vsPath\VC\Tools\MSVC\*\lib\x64",
            "$sdkPath\Lib\$latestSdk\um\x64",
            "$sdkPath\Lib\$latestSdk\ucrt\x64"
        )
        
        $expandedPaths = @()
        foreach ($path in $libPaths) {
            $resolved = Resolve-Path $path -ErrorAction SilentlyContinue
            if ($resolved) {
                $expandedPaths += $resolved.Path
            }
        }
        
        $env:LIB = $expandedPaths -join ";"
        Write-Host "LIB environment variable set" -ForegroundColor Green
        
        # Set INCLUDE environment variable
        $includePaths = @(
            "$vsPath\VC\Tools\MSVC\*\include",
            "$sdkPath\Include\$latestSdk\um",
            "$sdkPath\Include\$latestSdk\ucrt",
            "$sdkPath\Include\$latestSdk\shared"
        )
        
        $expandedIncludePaths = @()
        foreach ($path in $includePaths) {
            $resolved = Resolve-Path $path -ErrorAction SilentlyContinue
            if ($resolved) {
                $expandedIncludePaths += $resolved.Path
            }
        }
        
        $env:INCLUDE = $expandedIncludePaths -join ";"
        Write-Host "INCLUDE environment variable set" -ForegroundColor Green
    }
}

# Now try to build
Write-Host "`nAttempting to build..." -ForegroundColor Cyan
& cargo check --all-targets
