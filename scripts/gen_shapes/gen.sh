#!/bin/bash
# Gera mc-world/src/gen_shapes.rs e gen_variants.txt rodando o código original (forgeBin 1.8.9, nomes MCP).
# Precisa do jar do ForgeGradle no cache do gradle e das bibliotecas do Prism.
set -e
cd "$(dirname "$0")"
trap "rm -rf logs" EXIT  # o log4j do jogo cria logs/ aqui
FB=$(ls ~/.gradle/caches/minecraft/net/minecraftforge/forge/1.8.9-*/stable/*/forgeBin-1.8.9-*.jar | head -1)
LIBS=$(find ~/.local/share/PrismLauncher/libraries -name '*.jar' | grep -vE 'minecraft-1.8.9-client|forge-1.8.9|natives' | tr '\n' ':')
CP="$FB:$LIBS"
OUT=$(mktemp -d)
javac -proc:none -nowarn -XDignore.symbol.file -cp "$CP" -d "$OUT" GenShapes.java 2>&1 | grep -v "^Note:" || true
java -cp "$OUT:$CP" GenShapes ../../mc-world/src/gen_shapes.rs ../../mc-world/src/gen_variants.txt
rm -rf "$OUT"
