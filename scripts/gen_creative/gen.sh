#!/bin/bash
# Gera mc-client/src/gen_creative.rs rodando o código original do forgeBin 1.8.9 (CreativeTabs).
# Uso: scripts/gen_creative/gen.sh
set -e
cd "$(dirname "$0")"
FB=$(ls ~/.gradle/caches/minecraft/net/minecraftforge/forge/1.8.9-*/stable/*/forgeBin-1.8.9-*.jar | head -1)
LIBS=$(find ~/.local/share/PrismLauncher/libraries -name '*.jar' | grep -vE 'minecraft-1.8.9-client|forge-1.8.9|natives' | tr '\n' ':')
CP="$FB:$LIBS"
OUT=$(mktemp -d)
trap "rm -rf $OUT" EXIT
javac -proc:none -nowarn -XDignore.symbol.file -cp "$CP" -d "$OUT" GenCreative.java 2>&1 | grep -v "^Note:" || true
java -cp "$OUT:$CP" GenCreative ../../mc-client/src/gen_creative.rs
