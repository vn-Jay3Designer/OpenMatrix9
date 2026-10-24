# SPDX-License-Identifier: LGPL-2.1-or-later
# Build the add-on packages shipped beside the standalone ZIP (installed from Tools > Add-ons).
# The DLL is compiled here and only ever lives in the build output, never in the repository.
param([string]$OutDir = (Join-Path $PSScriptRoot '../../build/ci/standalone/addons'))
$ErrorActionPreference = 'Stop'
$source = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$OutDir = [IO.Path]::GetFullPath($OutDir)
. (Join-Path $PSScriptRoot 'msvc.ps1')
Initialize-OM9Msvc
if (-not $env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR = Join-Path ([IO.Path]::GetTempPath()) 'om9-target' }
$env:CARGO_TARGET_DIR = [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)
$release = Join-Path $env:CARGO_TARGET_DIR 'release'
Push-Location $source
try {
    & cargo build --locked --release -p om9-addon-gem-on-curve --lib -j 2
    if ($LASTEXITCODE -ne 0) { throw 'gem_on_curve add-on build failed' }
    & cargo build --locked --release -p om9-addons --features native --bin om9-addon -j 2
    if ($LASTEXITCODE -ne 0) { throw 'om9-addon packaging tool build failed' }
} finally { Pop-Location }

$tool = Join-Path $release 'om9-addon.exe'
$catalog = Join-Path $source 'Resources/menu/MainMenu.ini'
$stage = Join-Path $OutDir 'stage'
Remove-Item -Recurse -Force $OutDir -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force (Join-Path $stage 'gem_on_curve/icons') | Out-Null

# gem_on_curve: native runtime, Windows x64 DLL.
$gem = Join-Path $source 'addons/gem_on_curve/addon'
$manifest = (Get-Content -Raw -LiteralPath (Join-Path $gem 'om9_addon.toml')).Replace('@PLATFORM@', 'windows-x64').Replace('@ENTRY@', 'gem_on_curve.dll')
[IO.File]::WriteAllText((Join-Path $stage 'gem_on_curve/om9_addon.toml'), $manifest, (New-Object Text.UTF8Encoding $false))
Copy-Item (Join-Path $release 'gem_on_curve.dll') (Join-Path $stage 'gem_on_curve/gem_on_curve.dll')
Copy-Item (Join-Path $gem 'icons/*') (Join-Path $stage 'gem_on_curve/icons')

$packages = @(
    @{ src = Join-Path $stage 'gem_on_curve'; out = 'gem_on_curve-0.4.0-windows-x64.zip' },
    @{ src = Join-Path $source 'addons/ring_sizes'; out = 'ring_sizes-1.0.0.zip' }
)
foreach ($p in $packages) {
    $zip = Join-Path $OutDir $p.out
    & $tool --catalog $catalog build $p.src -o $zip
    if ($LASTEXITCODE -ne 0) { throw "Packaging $($p.out) failed" }
    & $tool --catalog $catalog validate $zip
    if ($LASTEXITCODE -ne 0) { throw "Validation of $($p.out) failed" }
}

# Smoke test in a throw-away config: install, enable (native DLL really loads), list, uninstall.
$config = Join-Path $OutDir 'smoke-config'
& $tool --catalog $catalog --config $config install (Join-Path $OutDir 'gem_on_curve-0.4.0-windows-x64.zip') --enable --yes
if ($LASTEXITCODE -ne 0) { throw 'gem_on_curve install/enable smoke test failed' }
& $tool --catalog $catalog --config $config menu Gems
if ($LASTEXITCODE -ne 0) { throw 'gem_on_curve command listing failed' }
& $tool --catalog $catalog --config $config uninstall gem_on_curve
if ($LASTEXITCODE -ne 0) { throw 'gem_on_curve uninstall smoke test failed' }
Remove-Item -Recurse -Force $stage, $config

Push-Location $OutDir
try {
    Get-ChildItem -Filter *.zip | Sort-Object Name | ForEach-Object {
        '{0}  {1}' -f (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant(), $_.Name
    } | Set-Content -LiteralPath 'SHA256SUMS.txt' -Encoding ascii
} finally { Pop-Location }
Get-ChildItem $OutDir | Format-Table Name, Length
