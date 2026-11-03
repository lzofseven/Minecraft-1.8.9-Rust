#!/bin/bash
# Gera mc-client/src/gen_recipes.rs rodando o código original do forgeBin 1.8.9 (receitas do CraftingManager).
# Uso: scripts/gen_item_flags/gen.sh
set -e
cd "$(dirname "$0")"
FB=$(ls ~/.gradle/caches/minecraft/net/minecraftforge/forge/1.8.9-*/stable/*/forgeBin-1.8.9-*.jar | head -1)
LIBS=$(find ~/.local/share/PrismLauncher/libraries -name '*.jar' | grep -vE 'minecraft-1.8.9-client|forge-1.8.9|natives' | tr '\n' ':')
CP="$FB:$LIBS"
OUT=$(mktemp -d)
trap "rm -rf $OUT" EXIT
javac -proc:none -nowarn -XDignore.symbol.file -cp "$CP" -d "$OUT" GenRecipes.java 2>&1 | grep -v "^Note:" || true
java -cp "$OUT:$CP" GenRecipes ../../mc-client/src/gen_recipes.rs
