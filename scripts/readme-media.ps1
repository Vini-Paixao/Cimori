# The README's demo and clips, drawn by Cimori's own renderer on a made-up desktop with made-up data
# (crates/isle-render/examples/demo_reel.rs). Needs ffmpeg on PATH.
#   .\scripts\readme-media.ps1            -> docs\media\demo.mp4, demo.webp and one .webp per chapter
#   .\scripts\readme-media.ps1 -Lang en-US -Out docs\media\en  -> the same, in English
param([string]$Out = "docs\media", [string]$Lang = "pt-BR")
$env:CIMORI_LANG = $Lang

$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root
New-Item -ItemType Directory -Force $Out | Out-Null

cargo run --release -q -p isle-render --example demo_reel -- "$Out\demo.mp4" "$Out\chapters.txt"
if ($LASTEXITCODE -ne 0) { throw "demo_reel failed" }

function Convert-Webp([string[]]$Cut, [string]$Filter, [string]$Path) {
    & ffmpeg -v error -y @Cut -i "$Out\demo.mp4" -vf $Filter -c:v libwebp_anim -q:v 72 -compression_level 6 -loop 0 -an $Path
    if ($LASTEXITCODE -ne 0) { throw "ffmpeg failed on $Path" }
}

# The whole demo, the desktop and all.
Convert-Webp @() "fps=20,scale=1280:-1:flags=lanczos" "$Out\demo.webp"

# Each chapter: the top of the screen around the island (its canvas is 880 wide, centred).
foreach ($line in Get-Content "$Out\chapters.txt") {
    $name, $from, $to = $line -split " "
    $length = [double]$to - [double]$from
    Convert-Webp @("-ss", $from, "-t", "$length") "crop=1040:560:440:0,fps=24,scale=780:-1:flags=lanczos" "$Out\$name.webp"
}
Remove-Item "$Out\chapters.txt"

# The settings window, page after page (the PNGs demo_reel wrote next to the video).
& ffmpeg -v error -y -framerate 1/2.2 -i "$Out\settings-%02d.png" -vf "fps=10,scale=980:-1:flags=lanczos" -c:v libwebp_anim -q:v 80 -loop 0 "$Out\settings.webp"
if ($LASTEXITCODE -ne 0) { throw "ffmpeg failed on settings.webp" }
Remove-Item "$Out\settings-*.png"
Get-ChildItem $Out | Select-Object Name, @{ n = "KB"; e = { [math]::Round($_.Length / 1KB) } }
