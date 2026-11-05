#!/bin/bash
# Gera o executável e o instalador do Windows (rodar num Linux com rustup, mingw-w64 e nsis):
#   rustup target add x86_64-pc-windows-gnu; apt install mingw-w64 nsis
# Saída: target/windows/Minecraft-1.8.9-Rust-Instalador.exe (e o mine-rs.exe solto). O nome é fixo para o
# link `releases/latest/download/...` do README.
set -euo pipefail
cd "$(dirname "$0")/.."
versao=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
cargo build --release -p mc-client --target x86_64-pc-windows-gnu
mkdir -p target/windows
cp target/x86_64-pc-windows-gnu/release/mc-client.exe target/windows/mine-rs.exe
x86_64-w64-mingw32-strip target/windows/mine-rs.exe
makensis -V2 -DVERSAO="$versao" -DEXE="$PWD/target/windows/mine-rs.exe" \
  -DSAIDA="$PWD/target/windows/Minecraft-1.8.9-Rust-Instalador.exe" windows/instalador.nsi
ls -l target/windows
