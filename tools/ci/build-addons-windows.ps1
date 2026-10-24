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
# Native add-ons: cargo package, DLL name, add-on id, version (zip name) and the menu the
# smoke test lists. Each zip carries one Windows x64 DLL beside its manifest.
$native = @(
    @{ crate = 'om9-addon-gem-on-curve'; dll = 'gem_on_curve.dll'; dir = 'gem_on_curve'; id = 'gem_on_curve' },
    @{ crate = 'om9-addon-curves'; dll = 'om9_curves.dll'; dir = 'curves'; id = 'om9_curves' },
    @{ crate = 'om9-addon-transform'; dll = 'om9_transform.dll'; dir = 'transform'; id = 'om9_transform' },
    @{ crate = 'om9-addon-gem-cutter'; dll = 'gem_cutter.dll'; dir = 'gem_cutter'; id = 'gem_cutter' }
)
Push-Location $source
try {
    foreach ($a in $native) {
        & cargo build --locked --release -p $a.crate --lib -j 2
        if ($LASTEXITCODE -ne 0) { throw "$($a.dir) add-on build failed" }
    }
    & cargo build --locked --release -p om9-addons --features native --bin om9-addon -j 2
    if ($LASTEXITCODE -ne 0) { throw 'om9-addon packaging tool build failed' }
} finally { Pop-Location }

$tool = Join-Path $release 'om9-addon.exe'
$catalog = Join-Path $source 'Resources/menu/MainMenu.ini'
$stage = Join-Path $OutDir 'stage'
Remove-Item -Recurse -Force $OutDir -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $stage | Out-Null

$packages = @()
foreach ($a in $native) {
    $src = Join-Path $source "addons/$($a.dir)/addon"
    $dst = Join-Path $stage $a.dir
    New-Item -ItemType Directory -Force $dst | Out-Null
    $text = (Get-Content -Raw -LiteralPath (Join-Path $src 'om9_addon.toml')).Replace('@PLATFORM@', 'windows-x64').Replace('@ENTRY@', $a.dll)
    [IO.File]::WriteAllText((Join-Path $dst 'om9_addon.toml'), $text, (New-Object Text.UTF8Encoding $false))
    Copy-Item (Join-Path $release $a.dll) (Join-Path $dst $a.dll)
    if (Test-Path (Join-Path $src 'icons')) {
        Copy-Item -Recurse (Join-Path $src 'icons') (Join-Path $dst 'icons')
    }
    $version = ([regex]::Match($text, '(?m)^version\s*=\s*"([^"]+)"')).Groups[1].Value
    if (-not $version) { throw "$($a.dir): no version in manifest" }
    $a.zip = "$($a.dir)-$version-windows-x64.zip"
    $packages += @{ src = $dst; out = $a.zip }
}
$packages += @{ src = Join-Path $source 'addons/ring_sizes'; out = 'ring_sizes-1.0.0.zip' }
foreach ($p in $packages) {
    $zip = Join-Path $OutDir $p.out
    & $tool --catalog $catalog build $p.src -o $zip
    if ($LASTEXITCODE -ne 0) { throw "Packaging $($p.out) failed" }
    & $tool --catalog $catalog validate $zip
    if ($LASTEXITCODE -ne 0) { throw "Validation of $($p.out) failed" }
}

# Smoke test in a throw-away config: install, enable (each native DLL really loads and
# answers its command list), list the Gems menu, uninstall.
$config = Join-Path $OutDir 'smoke-config'
foreach ($a in $native) {
    & $tool --catalog $catalog --config $config install (Join-Path $OutDir $a.zip) --enable --yes
    if ($LASTEXITCODE -ne 0) { throw "$($a.id) install/enable smoke test failed" }
}
& $tool --catalog $catalog --config $config menu Gems
if ($LASTEXITCODE -ne 0) { throw 'Gems command listing failed' }
foreach ($a in $native) {
    & $tool --catalog $catalog --config $config uninstall $a.id
    if ($LASTEXITCODE -ne 0) { throw "$($a.id) uninstall smoke test failed" }
}
Remove-Item -Recurse -Force $stage, $config

Push-Location $OutDir
try {
    Get-ChildItem -Filter *.zip | Sort-Object Name | ForEach-Object {
        '{0}  {1}' -f (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant(), $_.Name
    } | Set-Content -LiteralPath 'SHA256SUMS.txt' -Encoding ascii
} finally { Pop-Location }
Get-ChildItem $OutDir | Format-Table Name, Length
