#!/usr/bin/env python3
"""Gera mc-client/src/item_use.rs: ação e duração de uso de cada item da 1.8.9.

Lê `Item.registerItems` (id -> classe, `setAlwaysEdible`) e, na cadeia de classes,
`getItemUseAction` e `getMaxItemUseDuration`. Itens sem ação (NONE) ficam de fora.

Uso: python3 scripts/gen_item_use.py <.local/src> > mc-client/src/item_use.rs
"""
import re
import sys
from pathlib import Path

SRC = Path(sys.argv[1]) / "net/minecraft/item"
item_src = (SRC / "Item.java").read_text()
body = item_src[item_src.index("public static void registerItems()"):]


def chain(cls):
    while cls and (SRC / f"{cls}.java").exists():
        s = (SRC / f"{cls}.java").read_text()
        yield cls, s
        e = re.search(r"class \w+ extends (\w+)", s)
        cls = e.group(1) if e else None


def method(cls, name):
    for c, s in chain(cls):
        m = re.search(rf"public \w+ {name}\(ItemStack \w+\) \{{\s*return (.*?);", s)
        if m:
            return m.group(1)
    return None


rows = []
for m in re.finditer(r"registerItem\((\d+), \(String\)\"(\w+)\", \(new (\w+)\(.*?\);\n", body):
    id_, name, cls = int(m.group(1)), m.group(2), m.group(3)
    action = method(cls, "getItemUseAction")
    if action in (None, "EnumAction.NONE"):
        continue
    dur = method(cls, "getMaxItemUseDuration")
    if dur == "this.itemUseDuration":
        dur = re.search(r"this\.itemUseDuration = (\d+);", (SRC / "ItemFood.java").read_text()).group(1)
    if not dur or not dur.isdigit():
        raise SystemExit(f"duração não literal em {cls}: {dur}")
    always = ".setAlwaysEdible()" in m.group(0)
    rows.append((id_, name, action.split(".")[1], int(dur), always, cls))

print("//! GERADO por scripts/gen_item_use.py a partir do código decompilado da 1.8.9. Não editar.\n")
print("use crate::physics::UseAction;\n")
print("/// (id, ação, duração em ticks, `alwaysEdible`).")
print("pub const ITEM_USE: &[(i16, UseAction, i32, bool)] = &[")
for id_, name, action, dur, always, cls in rows:
    a = {"EAT": "Eat", "DRINK": "Drink", "BLOCK": "Block", "BOW": "Bow"}[action]
    print(f"    ({id_}, UseAction::{a}, {dur}, {str(always).lower()}), // {name} ({cls})")
print("];")

# Armaduras: (id, material de textura, armorType 0 capacete .. 3 botas) e a cor padrão do couro.
armor_src = (SRC / "ItemArmor.java").read_text()
mats = dict(re.findall(r"^\s+(\w+)\(\"(\w+)\", \d+", armor_src, re.M))
print("\n/// (id, nome do material em models/armor, armorType: 0 capacete, 1 peito, 2 calça, 3 botas).")
print("pub const ARMOR: &[(i16, &str, u8)] = &[")
for m in re.finditer(r"registerItem\((\d+), \(String\)\"\w+\", \(new ItemArmor\(ItemArmor\.ArmorMaterial\.(\w+), \d+, (\d)\)", body):
    print(f"    ({m.group(1)}, \"{mats[m.group(2)]}\", {m.group(3)}),")
print("];")
leather = re.search(r"return (\d+);", armor_src[armor_src.index("public int getColor(ItemStack"):]).group(1)
print(f"\n/// `ItemArmor.getColor` do couro sem tinta.\npub const LEATHER_DEFAULT_COLOR: u32 = {leather};")
