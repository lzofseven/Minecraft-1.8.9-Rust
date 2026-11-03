#!/bin/bash
# Gera mc-client/src/gen_container_items.rs (rodando o código original do forgeBin 1.8.9) e
# mc-client/src/gen_containers.rs (layout dos slots lido dos construtores Container*.java).
# Uso: scripts/gen_containers/gen.sh [pasta-com-.local/src]
set -e
cd "$(dirname "$0")"
SRC="${1:-$(cd ../.. && pwd)/.local/src}"
[ -d "$SRC" ] || SRC=$(dirname "$0")/../../.local/src
FB=$(ls ~/.gradle/caches/minecraft/net/minecraftforge/forge/1.8.9-*/stable/*/forgeBin-1.8.9-*.jar | head -1)
LIBS=$(find ~/.local/share/PrismLauncher/libraries -name '*.jar' | grep -vE 'minecraft-1.8.9-client|forge-1.8.9|natives' | tr '\n' ':')
CP="$FB:$LIBS"
OUT=$(mktemp -d)
trap "rm -rf $OUT" EXIT
javac -proc:none -nowarn -XDignore.symbol.file -cp "$CP" -d "$OUT" GenContainers.java 2>&1 | grep -v "^Note:" || true
java -cp "$OUT:$CP" GenContainers ../../mc-client/src/gen_container_items.rs
python3 gen_layouts.py "$SRC" ../../mc-client/src/gen_containers.rs
