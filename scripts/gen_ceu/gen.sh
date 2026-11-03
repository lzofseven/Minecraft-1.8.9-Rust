#!/bin/bash
# Gera mc-render/src/sky_golden.txt rodando o código original
# (forgeBin 1.8.9, nomes MCP). A tabela de biomas vem de scripts/gen_biomes/gen.sh. Precisa do jar do ForgeGradle no cache do gradle e das
# bibliotecas do Prism.
set -e
cd "$(dirname "$0")"
trap "rm -rf logs" EXIT
FB=$(ls ~/.gradle/caches/minecraft/net/minecraftforge/forge/1.8.9-*/stable/*/forgeBin-1.8.9-*.jar | head -1)
LIBS=$(find ~/.local/share/PrismLauncher/libraries -name '*.jar' | grep -vE 'minecraft-1.8.9-client|forge-1.8.9|natives|org/lwjgl/lwjgl[^/]*/3\.' | tr '\n' ':')
CP="$FB:$LIBS"
OUT=$(mktemp -d)
javac -proc:none -nowarn -XDignore.symbol.file -cp "$CP" -d "$OUT" GenCeu.java 2>&1 | grep -v "^Note:" || true
java -cp "$OUT:$CP" GenCeu ../../mc-render/src/sky_golden.txt
rm -rf "$OUT"
