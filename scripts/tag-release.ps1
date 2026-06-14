#Requires -Version 5.1
<#
.SYNOPSIS
    Tag a new WinSweep release and push it to GitHub.

.DESCRIPTION
    Bumps the workspace version, commits, creates a signed tag,
    and optionally pushes to origin so the release.yml workflow triggers.

.PARAMETER Version
    New version string in semver format (e.g. 0.2.0).

.PARAMETER Push
    Push the tag to origin immediately.

.EXAMPLE
    .\scripts\tag-release.ps1 -Version 0.2.0 -Push
#>
[CmdletBinding(SupportsShouldProcess)]
param(
    [Parameter(Mandatory)]
    [string]$Version,

    [switch]$Push
)

$ErrorActionPreference = 'Stop'

# Validate semver-ish input
if ($Version -notmatch '^\d+\.\d+\.\d+') {
    throw "Version must follow semver (e.g. 0.2.0). Got: $Version"
}

$root = Split-Path -Parent $PSScriptRoot
$workspaceToml = Join-Path $root 'Cargo.toml'

# Read current version
$toml = Get-Content $workspaceToml -Raw
if ($toml -notmatch 'version\s*=\s*"([\d\.]+)"') {
    throw "Could not find existing version in $workspaceToml"
}
$oldVersion = $Matches[1]
Write-Host "Bumping workspace version $oldVersion -> $Version"

# Update workspace Cargo.toml
$toml = $toml -replace "version = `"$oldVersion`"", "version = `"$Version`""
Set-Content -Path $workspaceToml -Value $toml -NoNewline

# Stage and commit
Set-Location $root
& git add Cargo.toml
& git commit -m "release: bump version to $Version"

# Create annotated tag
$tag = "v$Version"
& git tag -a $tag -m "WinSweep $Version"
Write-Host "Created tag $tag"

if ($Push) {
    if ($PSCmdlet.ShouldProcess($tag, 'git push')) {
        & git push origin HEAD
        & git push origin $tag
        Write-Host "Pushed $tag to origin — release.yml will trigger automatically."
    }
} else {
    Write-Host "Tag created locally. Run: git push origin $tag"
}
