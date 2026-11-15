# Builds PeaceControl release artifacts: binaries, portable zip and installer.
#
#   powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1 -Version 1.0.0
#
# Requires: Rust (MSVC toolchain). NSIS 3 is optional; without it the installer
# step is skipped and only the portable package is produced.

param(
    [string]$Version = "1.0.1",
    [switch]$SkipTests
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

function Write-Step($text) {
    Write-Host ""
    Write-Host "== $text" -ForegroundColor Cyan
}

Write-Step "Building PeaceControl $Version"

if (-not $SkipTests) {
    Write-Step "Running tests"
    cargo test --workspace
    if ($LASTEXITCODE -ne 0) { throw "tests failed" }
}

Write-Step "Building release binaries"
cargo build --release --workspace
if ($LASTEXITCODE -ne 0) { throw "release build failed" }

$exe = Join-Path $root "target\release\peacecontrol.exe"
$cli = Join-Path $root "target\release\peacecontrol-cli.exe"
if (-not (Test-Path $exe)) { throw "missing $exe" }

$dist = Join-Path $root "dist"
$portable = Join-Path $dist "portable"
New-Item -ItemType Directory -Force -Path $dist, $portable | Out-Null
Get-ChildItem -Path $portable -File | Remove-Item -Force -ErrorAction SilentlyContinue

Write-Step "Preparing portable package"
Copy-Item $exe (Join-Path $portable "PeaceControl.exe") -Force
if (Test-Path $cli) { Copy-Item $cli (Join-Path $portable "peacecontrol-cli.exe") -Force }
New-Item -ItemType File -Force -Path (Join-Path $portable "peacecontrol.portable") | Out-Null
Copy-Item (Join-Path $root "README.md") $portable -Force
Copy-Item (Join-Path $root "LICENSE") $portable -Force
if (Test-Path (Join-Path $root "CHANGELOG.md")) {
    Copy-Item (Join-Path $root "CHANGELOG.md") $portable -Force
}

$zip = Join-Path $dist "PeaceControl-$Version-portable.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $portable "*") -DestinationPath $zip -Force
Write-Host "portable: $zip"

Write-Step "Building installer"
$makensis = $null
foreach ($candidate in @(
    "${env:ProgramFiles(x86)}\NSIS\makensis.exe",
    "$env:ProgramFiles\NSIS\makensis.exe",
    "$env:LOCALAPPDATA\Programs\NSIS\makensis.exe"
)) {
    if ($candidate -and (Test-Path $candidate)) { $makensis = $candidate; break }
}
if (-not $makensis) {
    $command = Get-Command makensis -ErrorAction SilentlyContinue
    if ($command) { $makensis = $command.Source }
}

if ($makensis) {
    & $makensis "/DAPP_VERSION=$Version" (Join-Path $root "installer\PeaceControl.nsi")
    if ($LASTEXITCODE -ne 0) { throw "installer build failed" }
    Write-Host "installer: $(Join-Path $dist "PeaceControl-Setup-$Version.exe")"
} else {
    Write-Warning "NSIS was not found; skipping the installer. Install NSIS 3 and rerun."
}

Write-Step "Done"
Get-ChildItem $dist -File | Format-Table Name, Length, LastWriteTime
