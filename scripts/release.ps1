<#
.SYNOPSIS
  Builds the Windows installer and checks it against the budgets: installer size, JS + CSS size, SHA-256.

.DESCRIPTION
  Usage (PowerShell 7, repo root):  pwsh scripts/release.ps1 [-InstallTest]
  Output: target\release\bundle\nsis\<name>_<version>_x64-setup.exe and a .sha256 file next to it.
  -InstallTest  installs it silently for the current user, checks the files, then uninstalls it.
#>
param([switch]$InstallTest)

$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
if ((Test-Path $cargoBin) -and -not ($env:Path -split ';' -contains $cargoBin)) { $env:Path = "$cargoBin;$env:Path" }

cargo deny check
if ($LASTEXITCODE -ne 0) { exit 1 }
npx tauri build
if ($LASTEXITCODE -ne 0) { exit 1 }

$conf = Get-Content src-tauri/tauri.conf.json -Raw | ConvertFrom-Json
$setup = Get-ChildItem target/release/bundle/nsis -Filter "*_$($conf.version)_x64-setup.exe" | Select-Object -First 1
if (-not $setup) { throw 'installer not found' }
$hash = (Get-FileHash $setup.FullName -Algorithm SHA256).Hash.ToLower()
"$hash  $($setup.Name)" | Set-Content -NoNewline -Encoding utf8 "$($setup.FullName).sha256"

# JS + CSS of the UI, gzip-compressed (fonts not counted).
$gz = 0
foreach ($f in Get-ChildItem ui/dist/assets -File | Where-Object { $_.Extension -in '.js', '.css' }) {
  $ms = [IO.MemoryStream]::new()
  $z = [IO.Compression.GZipStream]::new($ms, [IO.Compression.CompressionLevel]::Optimal)
  $bytes = [IO.File]::ReadAllBytes($f.FullName)
  $z.Write($bytes, 0, $bytes.Length); $z.Dispose()
  $gz += $ms.ToArray().Length
}

$installerMb = [math]::Round($setup.Length / 1MB, 2)
$jsCssKb = [math]::Round($gz / 1KB, 1)
[pscustomobject]@{
  Installer = $setup.FullName
  'Size (MB)' = $installerMb
  'Budget (MB)' = 6
  'JS+CSS gzip (KB)' = $jsCssKb
  'Budget (KB)' = 80
  SHA256 = $hash
} | Format-List

$ok = $true
if ($installerMb -gt 6) { Write-Host 'Installer over budget' -ForegroundColor Red; $ok = $false }
if ($jsCssKb -gt 80) { Write-Host 'JS + CSS over budget' -ForegroundColor Red; $ok = $false }

if ($InstallTest) {
  $dir = Join-Path $env:LOCALAPPDATA $conf.productName
  Write-Host "==> silent install to $dir" -ForegroundColor Cyan
  Start-Process $setup.FullName -ArgumentList '/S' -Wait
  $exe = Join-Path $dir 'keyboard-layer-editor.exe'
  if (-not (Test-Path $exe)) { Write-Host "not installed: $exe" -ForegroundColor Red; exit 1 }
  # The bundler patches a few bytes (bundle type marker) into the copy it installs, so compare size and version.
  $built = Get-Item target/release/keyboard-layer-editor.exe
  $installed = Get-Item $exe
  if ($installed.Length -ne $built.Length -or
      $installed.VersionInfo.ProductVersion -ne $built.VersionInfo.ProductVersion) {
    Write-Host 'installed exe is not this build' -ForegroundColor Red; exit 1
  }
  Write-Host '==> silent uninstall' -ForegroundColor Cyan
  Start-Process (Join-Path $dir 'uninstall.exe') -ArgumentList '/S' -Wait
  # The NSIS uninstaller copies itself to %TEMP% and returns at once; wait for the files to go.
  for ($i = 0; $i -lt 50 -and (Test-Path $exe); $i++) { Start-Sleep -Milliseconds 200 }
  if (Test-Path $exe) { Write-Host 'uninstall left the app behind' -ForegroundColor Red; exit 1 }
  Write-Host 'Install and uninstall OK.' -ForegroundColor Green
}

if (-not $ok) { exit 1 }
