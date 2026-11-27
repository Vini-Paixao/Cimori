# Builds Cimori.exe and packs dist\Cimori-<version>-win-x64.zip (exe, Windhawk bridge, README, LICENSE)
# and, with Inno Setup 6 installed, dist\Cimori-<version>-setup.exe. Writes dist\SHA256SUMS.txt.
# Pass -Version to reproduce a published release using its tag's version.
# Uso: powershell -ExecutionPolicy Bypass -File scripts\release.ps1
param(
    [ValidatePattern('^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$')] [string]$Version,
    [switch]$RequireInstaller
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$manifest = Join-Path $root 'Cargo.toml'
$lockfile = Join-Path $root 'Cargo.lock'
$versionLine = Select-String -LiteralPath $manifest -Pattern '^version = "(.+)"' | Select-Object -First 1
$sourceVersion = $versionLine.Matches[0].Groups[1].Value
if (-not $Version) { $Version = $sourceVersion }
$iscc = @("${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe", "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe") | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
if ($RequireInstaller -and -not $iscc) { throw 'Inno Setup 6 is required' }
Write-Host "==> Cimori $version"

if ($Version -ne $sourceVersion) {
    $metadataJson = cargo metadata --locked --no-deps --format-version 1
    if ($LASTEXITCODE -ne 0) { throw 'Could not read the Cargo workspace packages' }
    $packages = ($metadataJson | ConvertFrom-Json).packages | Where-Object { $_.version -eq $sourceVersion }
    $packageNames = ($packages.name | ForEach-Object { [regex]::Escape($_) }) -join '|'
    $lockPattern = '(?m)^(\[\[package\]\]\r?\nname = "(?:' + $packageNames + ')"\r?\nversion = ")' + [regex]::Escape($sourceVersion) + '"(?=\r?$)'
    $lockText = [IO.File]::ReadAllText($lockfile)
    if ([regex]::Matches($lockText, $lockPattern).Count -ne @($packages).Count) { throw 'Cargo.lock does not match the Cargo workspace packages' }
    $originalManifest = [IO.File]::ReadAllBytes($manifest)
    $originalLockfile = [IO.File]::ReadAllBytes($lockfile)
}

try {
    if ($originalManifest) {
        $workspaceVersion = [regex]'(?m)(^\[workspace\.package\]\r?\nversion = ")[^"]+"'
        $replacement = '${1}' + $Version + '"'
        [IO.File]::WriteAllText($manifest, $workspaceVersion.Replace([IO.File]::ReadAllText($manifest), $replacement, 1))
        [IO.File]::WriteAllText($lockfile, [regex]::Replace($lockText, $lockPattern, $replacement))
    }
    cargo build --release --locked
    if ($LASTEXITCODE -ne 0) { throw 'cargo build falhou' }
} finally {
    if ($originalManifest) {
        [IO.File]::WriteAllBytes($manifest, $originalManifest)
        [IO.File]::WriteAllBytes($lockfile, $originalLockfile)
    }
}

$name = "Cimori-$version-win-x64"
$stage = Join-Path $root "dist\$name"
$zip = Join-Path $root "dist\$name.zip"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
if (Test-Path $zip) { Remove-Item -Force $zip }
New-Item -ItemType Directory -Force $stage | Out-Null

Copy-Item target\release\Cimori.exe $stage
Copy-Item windhawk\cimori-tray-bridge.wh.cpp $stage
Copy-Item README.md, LICENSE, CHANGELOG.md $stage

Compress-Archive -Path "$stage\*" -DestinationPath $zip
Remove-Item -Recurse -Force $stage

$outputs = @($zip)
if ($iscc) {
    & $iscc /Q "/DVersion=$version" installer\cimori.iss
    if ($LASTEXITCODE -ne 0) { throw "Inno Setup falhou" }
    $outputs += Join-Path $root "dist\Cimori-$version-setup.exe"
} else {
    Write-Host "(Inno Setup 6 não encontrado: só o zip; o instalador sai no workflow de release)"
}

$sums = foreach ($file in $outputs) {
    $hash = (Get-FileHash -Algorithm SHA256 $file).Hash.ToLower()
    Write-Host "==> $file"
    Write-Host "SHA-256: $hash"
    "$hash  $(Split-Path -Leaf $file)"
}
Set-Content -Encoding ascii -Path (Join-Path $root 'dist\SHA256SUMS.txt') -Value $sums
