#!/bin/bash
# Gera mc-client/src/gen_debug.rs rodando o código original do forgeBin 1.8.9 (nomes e propriedades dos blocos para o F3).
# Uso: scripts/gen_debug/gen.sh
set -e
cd "$(dirname "$0")"
FB=$(ls ~/.gradle/caches/minecraft/net/minecraftforge/forge/1.8.9-*/stable/*/forgeBin-1.8.9-*.jar | head -1)
LIBS=$(find ~/.local/share/PrismLauncher/libraries -name '*.jar' | grep -vE 'minecraft-1.8.9-client|forge-1.8.9|natives' | tr '\n' ':')
CP="$FB:$LIBS"
OUT=$(mktemp -d)
trap "rm -rf $OUT" EXIT
javac -proc:none -nowarn -XDignore.symbol.file -cp "$CP" -d "$OUT" GenDebug.java 2>&1 | grep -v "^Note:" || true
java -cp "$OUT:$CP" GenDebug ../../mc-client/src/gen_debug.rs
