<#
.SYNOPSIS
  Builds the e2e-sim app and runs the end-to-end suites against it.

.DESCRIPTION
  Usage (PowerShell 7, repo root):  pwsh scripts/e2e.ps1 [-Runs 3] [-Perf] [-Screenshots] [-NoBuild]
  Needs tauri-driver (cargo install tauri-driver --locked) and an msedgedriver of the installed WebView2 major
  version (MSEDGEDRIVER, or %LOCALAPPDATA%\kle-tools\edgedriver-*\msedgedriver.exe); see README.
  -Runs N        run the functional and visual suites N times (flakiness check)
  -Perf          also measure the performance budgets (tests/e2e/out/perf.json)
  -Screenshots   also capture every screen in light and dark into docs/screenshots/
#>
param([int]$Runs = 1, [switch]$Perf, [switch]$Screenshots, [switch]$NoBuild)

$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
if ((Test-Path $cargoBin) -and -not ($env:Path -split ';' -contains $cargoBin)) { $env:Path = "$cargoBin;$env:Path" }

if (-not $NoBuild) {
  # A separate target folder, so the e2e build never replaces the real release build.
  $env:CARGO_TARGET_DIR = Join-Path (Get-Location) 'target\e2e'
  npx tauri build --no-bundle --features e2e-sim
  if ($LASTEXITCODE -ne 0) { exit 1 }
  Remove-Item Env:CARGO_TARGET_DIR
}

for ($i = 1; $i -le $Runs; $i++) {
  Write-Host "==> run $i of $Runs" -ForegroundColor Cyan
  node --test --test-concurrency=1 tests/e2e/app.test.mjs tests/e2e/visual.test.mjs
  if ($LASTEXITCODE -ne 0) { Write-Host "FAILED in run $i" -ForegroundColor Red; exit 1 }
}
if ($Perf) { node tests/e2e/perf.mjs; if ($LASTEXITCODE -ne 0) { exit 1 } }
if ($Screenshots) { node tests/e2e/screenshots.mjs; if ($LASTEXITCODE -ne 0) { exit 1 } }
Write-Host 'E2E passed.' -ForegroundColor Green
