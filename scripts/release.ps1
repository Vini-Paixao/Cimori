# Builds Cimori.exe and packs dist\Cimori-<version>-win-x64.zip (exe, Windhawk bridge, README, LICENSE)
# and, with Inno Setup 6 installed, dist\Cimori-<version>-setup.exe. Writes dist\SHA256SUMS.txt.
# Uploads nothing (the release workflow does, from a tag).
# Uso: powershell -ExecutionPolicy Bypass -File scripts\release.ps1
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
Write-Host "==> Cimori $version"

cargo build --release
if ($LASTEXITCODE -ne 0) { throw "cargo build falhou" }

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
$iscc = @("${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe", "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe") | Where-Object { Test-Path $_ } | Select-Object -First 1
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
