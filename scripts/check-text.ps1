<#
.SYNOPSIS
  Text rules for every tracked or new (not ignored) file:
  - no em dash (U+2014) anywhere;
  - fixtures contain no machine-specific paths or user names.
#>
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)

$binary = '\.(bin|png|ico|icns|woff2?|ttf|exe|zip|gz|zst)$'
$files = git ls-files -co --exclude-standard | Where-Object { $_ -notmatch $binary -and (Test-Path $_ -PathType Leaf) }
$emDash = [string][char]0x2014
$bad = @()
# docs/BRIEF.md is a verbatim copy of the assignment, which quotes the forbidden character in its own rule.
foreach ($f in ($files | Where-Object { $_ -ne 'docs/BRIEF.md' })) {
  $text = [IO.File]::ReadAllText((Resolve-Path $f))
  if ($text.Contains($emDash)) { $bad += "em dash: $f" }
}

# Fixtures are shared publicly: no device instance paths, drive paths or the local user name.
$userName = [Environment]::UserName
$fixturePatterns = @('\\\\\?\\', '[A-Za-z]:\\Users', '[A-Za-z]:/Users', 'hid#vid_')
if ($userName) { $fixturePatterns += "\b$([regex]::Escape($userName))\b" }
foreach ($f in ($files | Where-Object { $_ -like 'tests/fixtures/*' })) {
  $text = [IO.File]::ReadAllText((Resolve-Path $f))
  foreach ($p in $fixturePatterns) {
    if ($text -cmatch $p) { $bad += "machine-specific text /$p/ in $f" }
  }
}

if ($bad.Count) { $bad | ForEach-Object { Write-Host $_ -ForegroundColor Red }; exit 1 }
Write-Host "text rules ok ($($files.Count) files)"
exit 0

