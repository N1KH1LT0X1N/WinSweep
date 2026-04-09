# Setup Visual Studio Build Environment

Write-Host "Setting up Visual Studio environment..." -ForegroundColor Cyan

# Try different Visual Studio installations
$vsPaths = @(
    "${env:ProgramFiles}\Microsoft Visual Studio\2022\Enterprise\VC\Auxiliary\Build\vcvarsall.bat",
    "${env:ProgramFiles}\Microsoft Visual Studio\2022\Professional\VC\Auxiliary\Build\vcvarsall.bat",
    "${env:ProgramFiles}\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvarsall.bat",
    "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2019\Enterprise\VC\Auxiliary\Build\vcvarsall.bat",
    "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2019\Professional\VC\Auxiliary\Build\vcvarsall.bat",
    "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2019\Community\VC\Auxiliary\Build\vcvarsall.bat"
)

$vcvarsall = $null
foreach ($path in $vsPaths) {
    if (Test-Path $path) {
        $vcvarsall = $path
        Write-Host "Found Visual Studio at: $path" -ForegroundColor Green
        break
    }
}

if (-not $vcvarsall) {
    Write-Error "Visual Studio Build Tools not found!"
    Write-Host "Please install Visual Studio Build Tools with C++ support" -ForegroundColor Yellow
    exit 1
}

# Create a batch file to run vcvarsall and capture environment
$tempBat = "$env:TEMP\setup_vs_env.bat"
@"
@echo off
call "$vcvarsall" x64 >nul 2>&1
set > "$env:TEMP\vs_env.txt"
"@ | Out-File -FilePath $tempBat -Encoding ASCII

# Run the batch file
& $tempBat

# Read and apply environment variables
if (Test-Path "$env:TEMP\vs_env.txt") {
    Get-Content "$env:TEMP\vs_env.txt" | ForEach-Object {
        if ($_ -match '^(.+?)=(.*)$') {
            $name = $matches[1]
            $value = $matches[2]
            [Environment]::SetEnvironmentVariable($name, $value, "Process")
        }
    }
    Remove-Item "$env:TEMP\vs_env.txt"
}

# Clean up
Remove-Item $tempBat

# Verify the setup
$linker = Get-Command link.exe -ErrorAction SilentlyContinue
if ($linker) {
    Write-Host "Visual Studio environment configured successfully!" -ForegroundColor Green
    Write-Host "Linker found at: $($linker.Source)" -ForegroundColor Green
    
    # Now try cargo check
    Write-Host "`nRunning cargo check..." -ForegroundColor Cyan
    & cargo check --all-targets
} else {
    Write-Error "Failed to configure Visual Studio environment"
    Write-Host "You may need to restart your terminal after running this script" -ForegroundColor Yellow
}
