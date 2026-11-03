#!/bin/bash
# Gera mc-world/src/gen_biomes.rs e gen_biome_vectors.txt rodando o código original (forgeBin 1.8.9, nomes MCP).
set -e
cd "$(dirname "$0")"
trap "rm -rf logs" EXIT
FB=$(ls ~/.gradle/caches/minecraft/net/minecraftforge/forge/1.8.9-*/stable/*/forgeBin-1.8.9-*.jar | head -1)
LIBS=$(find ~/.local/share/PrismLauncher/libraries -name '*.jar' | grep -vE 'minecraft-1.8.9-client|forge-1.8.9|natives' | tr '\n' ':')
JAR=~/.local/share/PrismLauncher/libraries/com/mojang/minecraft/1.8.9/minecraft-1.8.9-client.jar
CP="$FB:$LIBS"
OUT=$(mktemp -d)
javac -proc:none -nowarn -XDignore.symbol.file -cp "$CP" -d "$OUT" GenBiomes.java 2>&1 | grep -v "^Note:" || true
java -cp "$OUT:$CP" GenBiomes ../../mc-world/src/gen_biomes.rs ../../mc-world/src/gen_biome_vectors.txt "$JAR"
rm -rf "$OUT"
