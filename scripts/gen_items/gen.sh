#!/bin/bash
# Gera mc-world/src/gen_items.rs rodando o código original (forgeBin 1.8.9): copia o corpo de
# RenderItem.registerItems de .local/src, executa-o contra um "mesher" que só anota, e consulta
# Item.getColorFromItemStack, EntityList.entityEggs e PotionHelper.getLiquidColor.
# Também grava mc-world/src/gen_tools.rs (dureza e ferramentas).
# Uso: scripts/gen_items/gen.sh [pasta-com-.local/src]
set -e
cd "$(dirname "$0")"
SRC="${1:-$(cd ../.. && pwd)/.local/src}"
[ -d "$SRC" ] || SRC=$(dirname "$0")/../../.local/src
trap "rm -rf logs" EXIT
FB=$(ls ~/.gradle/caches/minecraft/net/minecraftforge/forge/1.8.9-*/stable/*/forgeBin-1.8.9-*.jar | head -1)
LIBS=$(find ~/.local/share/PrismLauncher/libraries -name '*.jar' | grep -vE 'minecraft-1.8.9-client|forge-1.8.9|natives' | tr '\n' ':')
JAR=~/.local/share/PrismLauncher/libraries/com/mojang/minecraft/1.8.9/minecraft-1.8.9-client.jar
CP="$FB:$LIBS"
OUT=$(mktemp -d)
python3 - "$SRC/net/minecraft/client/renderer/entity/RenderItem.java" "$OUT/GenItems.java" <<'PY'
import sys, re
src = open(sys.argv[1]).read().split("\n")
imports = [l for l in src if l.startswith("import net.minecraft") or l.startswith("import java.util")]
start = next(i for i, l in enumerate(src) if "private void registerItems() {" in l)
end = next(i for i in range(start + 1, len(src)) if src[i] == "   }")
body = "\n".join(l for l in src[start + 1:end] if "ModelLoader.onRegisterItems" not in l)  # gancho do Forge, irrelevante
tpl = open("GenItems.java.in").read()
open(sys.argv[2], "w").write(tpl.replace("//IMPORTS", "\n".join(imports)).replace("//BODY", body))
PY
javac -proc:none -nowarn -XDignore.symbol.file -cp "$CP" -d "$OUT" "$OUT/GenItems.java" 2>&1 | grep -v "^Note:" || true
java -cp "$OUT:$CP" GenItems "$JAR" ../../mc-world/src/gen_items.rs ../../mc-world/src/gen_tools.rs
rm -rf "$OUT"
