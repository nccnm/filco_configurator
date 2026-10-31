<#
.SYNOPSIS
  Run every gate a commit must pass: formatting, lints, type checks, unit and integration tests, text rules.

.DESCRIPTION
  Usage (PowerShell 7, repo root):  pwsh scripts/check.ps1 [-Quick]
  -Quick skips the release-mode exhaustive slot test.
  Exits non-zero on the first failing step.
#>
param([switch]$Quick)

$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
if ((Test-Path $cargoBin) -and -not ($env:Path -split ';' -contains $cargoBin)) { $env:Path = "$cargoBin;$env:Path" }

$steps = [System.Collections.Generic.List[object]]::new()
function Step([string]$name, [scriptblock]$body) {
  $sw = [Diagnostics.Stopwatch]::StartNew()
  Write-Host "==> $name" -ForegroundColor Cyan
  & $body
  if ($LASTEXITCODE -ne 0) { Write-Host "FAILED: $name" -ForegroundColor Red; exit 1 }
  $steps.Add([pscustomobject]@{ Step = $name; Seconds = [math]::Round($sw.Elapsed.TotalSeconds, 1) })
}

Step 'text rules (no em dash, no machine paths in fixtures)' { pwsh -NoProfile -File scripts/check-text.ps1 }
Step 'cargo fmt' { cargo fmt --all --check }
Step 'cargo clippy' { cargo clippy --workspace --all-targets --locked -- -D warnings }
Step 'cargo test' { cargo test --workspace --locked }
if (-not $Quick) {
  Step 'exhaustive slot round-trip (release)' { cargo test -p filco-core --release --locked -- --ignored exhaustive }
}
Step 'prettier' { npm run --silent format:check }
Step 'eslint' { npm run --silent lint }
Step 'svelte-check' { npm run --silent check }
Step 'vitest' { npm run --silent test }

$steps | Format-Table -AutoSize
Write-Host 'All checks passed.' -ForegroundColor Green
