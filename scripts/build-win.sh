#!/usr/bin/env bash
set -euo pipefail

export PATH="$HOME/.cargo/bin:$PATH"

echo "==> [1/3] Compiling native Fedora binary (Cimori / --fedora-bridge)..."
cargo build --release --offline

echo "==> [2/3] Cross-compiling Windows 11 binary (Cimori.exe)..."
cargo build --release --target x86_64-pc-windows-msvc --offline

echo "==> [3/3] Packaging dist/ bundle for Windows 11..."
mkdir -p dist
cp target/x86_64-pc-windows-msvc/release/Cimori.exe dist/Cimori.exe
if [ -f "Mac Island & Dock 2026-11-10.txt" ]; then
    cp "Mac Island & Dock 2026-11-10.txt" "dist/Mac Island & Dock 2026-11-10.txt"
fi

ls -lh dist/Cimori.exe target/release/Cimori
