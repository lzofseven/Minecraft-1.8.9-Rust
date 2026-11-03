#!/usr/bin/env python3
"""Gera mc-world/src/item_keys.rs (id -> chave de tradução) a partir do código decompilado em .local/src."""
import re, sys, pathlib

src = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ".local/src/net/minecraft")
out = pathlib.Path("mc-world/src/item_keys.rs")

items = {}
for m in re.finditer(r'registerItem\((\d+), \(String\)"\w+", .*?setUnlocalizedName\("([^"]+)"\)', (src / "item/Item.java").read_text()):
    items[int(m.group(1))] = "item." + m.group(2)

block_src = (src / "block/Block.java").read_text()
vars_ = {m.group(1): m.group(2) for m in re.finditer(r'Block (\w+) = .*?setUnlocalizedName\("([^"]+)"\)', block_src)}
blocks = {}
for m in re.finditer(r'registerBlock\((\d+), \(String\)"\w+", (.*?)\);\n', block_src):
    key = re.search(r'setUnlocalizedName\("([^"]+)"\)', m.group(2))
    name = key.group(1) if key else vars_.get(m.group(2).strip())
    if name:
        blocks[int(m.group(1))] = "tile." + name

rows = sorted(blocks.items()) + sorted(items.items())
body = "\n".join(f'    ({i}, "{k}"),' for i, k in rows)
out.write_text(
    "//! Gerado por scripts/gen_item_keys.py a partir do jogo do próprio jogador. Não editar.\n\n"
    "/// (id, chave de tradução sem o sufixo `.name`).\n"
    f"pub const ITEM_KEYS: &[(u16, &str)] = &[\n{body}\n];\n"
)
print(f"{len(blocks)} blocos, {len(items)} itens -> {out}")
