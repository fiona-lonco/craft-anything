<#
Builds the unofficial version as ONE file people download and open:
  dist\mod\CraftAnything-mod-<version>-windows-x64.exe   (+ .sha256)
craft-anything-cli.exe (the tool server coding agents run) is carried inside it and unpacked on the
first start into the settings folder. LEEME.txt and README.txt are copied beside it for the release.

The C runtime is linked in (+crt-static), as the official package does, so no Visual C++
redistributable is needed on the computer it runs on.

  powershell -File packaging\windows\mod\paquete.ps1 [-Jobs 6]
#>
param([int]$Jobs = 6, [string]$FontsDir = (Join-Path $env:USERPROFILE 'craft-fonts'))
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$Target = 'x86_64-pc-windows-msvc'

$Version = $null
$inPkg = $false
foreach ($line in Get-Content (Join-Path $Root 'Cargo.toml')) {
  if ($line -match '^\s*\[') { $inPkg = ($line.Trim() -eq '[workspace.package]'); continue }
  if ($inPkg -and $line -match '^\s*version\s*=\s*"([^"]+)"') { $Version = $Matches[1]; break }
}
if (-not $Version) { throw 'could not read [workspace.package] version from Cargo.toml' }
Write-Output "Craft Anything (unofficial) $Version for Windows x64, one file"

$env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS = '-C target-feature=+crt-static'
# The fonts for Chinese, Japanese, Korean, Arabic, Thai, Devanagari… (craft-fonts, the commit the
# official releases use). Required: without them those languages show boxes.
if (-not (Test-Path (Join-Path $FontsDir 'fonts\manifest.txt'))) { throw "craft-fonts isn't at $FontsDir (git clone https://github.com/storytold/craft-fonts and check out b076417)" }
$env:CRAFT_FONTS_DIR = (Resolve-Path $FontsDir).Path
$env:CRAFT_FONTS_REQUIRED = '1'
$Bin = Join-Path $Root "target\$Target\release"
Push-Location $Root
try {
  # The CLI first: the app carries it.
  $env:CRAFT_ANYTHING_EMBED_CLI = ''
  cargo build --release -j $Jobs -p craft-anything-cli --target $Target
  if ($LASTEXITCODE -ne 0) { throw "cargo build (cli) failed ($LASTEXITCODE)" }
  $Cli = Join-Path $Bin 'craft-anything-cli.exe'
  $env:CRAFT_ANYTHING_EMBED_CLI = $Cli
  cargo build --release -j $Jobs -p craft-anything --target $Target
  if ($LASTEXITCODE -ne 0) { throw "cargo build (app) failed ($LASTEXITCODE)" }
} finally {
  Remove-Item Env:CRAFT_ANYTHING_EMBED_CLI -ErrorAction SilentlyContinue
  Pop-Location
}

$App = Join-Path $Bin 'craft-anything.exe'
# Linked statically, the C runtime's DLL isn't named among its imports.
foreach ($exe in $App, $Cli) {
  if (Select-String -Path $exe -Pattern 'VCRUNTIME140.dll' -SimpleMatch -Quiet) { throw "$exe still needs VCRUNTIME140.dll" }
}
if ((Get-Item $App).Length -le (Get-Item $Cli).Length) { throw 'the app should carry the CLI inside it, but it is smaller than the CLI' }

$Dist = Join-Path $Root 'dist\mod'
New-Item -ItemType Directory -Force -Path $Dist | Out-Null
$Name = "CraftAnything-mod-$Version-windows-x64.exe"
$Out = Join-Path $Dist $Name
Copy-Item $App $Out -Force
Copy-Item (Join-Path $PSScriptRoot 'LEEME.txt'), (Join-Path $PSScriptRoot 'README.txt') $Dist -Force
# The licences of the fonts inside the program (OFL-1.1 asks for them to travel with it).
$Licences = Join-Path $Dist 'font-licences'
New-Item -ItemType Directory -Force -Path $Licences | Out-Null
Get-ChildItem -Path (Join-Path $Root 'assets\fonts') -Filter 'OFL-*.txt' | ForEach-Object { Copy-Item $_.FullName $Licences -Force }
Get-ChildItem -Path (Join-Path $env:CRAFT_FONTS_DIR 'fonts\*\OFL.txt') | ForEach-Object { Copy-Item $_.FullName (Join-Path $Licences "OFL-$($_.Directory.Name).txt") -Force }
$hash = (Get-FileHash $Out -Algorithm SHA256).Hash.ToLowerInvariant()
Set-Content -Path "$Out.sha256" -Value "$hash  $Name" -Encoding ascii
Write-Output ("ok {0} ({1:N0} MB)" -f $Out, ((Get-Item $Out).Length / 1MB))
Write-Output "sha256 $hash"
