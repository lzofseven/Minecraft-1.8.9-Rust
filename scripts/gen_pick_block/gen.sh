#!/bin/bash
# Gera mc-client/src/gen_pick_block.rs rodando o código original do forgeBin 1.8.9 (Block.getPickBlock de cada estado).
# Uso: scripts/gen_item_flags/gen.sh
set -e
cd "$(dirname "$0")"
FB=$(ls ~/.gradle/caches/minecraft/net/minecraftforge/forge/1.8.9-*/stable/*/forgeBin-1.8.9-*.jar | head -1)
LIBS=$(find ~/.local/share/PrismLauncher/libraries -name '*.jar' | grep -vE 'minecraft-1.8.9-client|forge-1.8.9|natives' | tr '\n' ':')
CP="$FB:$LIBS"
OUT=$(mktemp -d)
trap "rm -rf $OUT" EXIT
javac -proc:none -nowarn -XDignore.symbol.file -cp "$CP" -d "$OUT" GenPickBlock.java 2>&1 | grep -v "^Note:" || true
java -cp "$OUT:$CP" GenPickBlock ../../mc-client/src/gen_pick_block.rs
