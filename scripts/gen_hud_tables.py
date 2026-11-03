#!/usr/bin/env python3
"""Gera mc-client/src/gen_hud.rs a partir do Java decompilado (.local/src):
poções (Potion.java) e valores de armadura (Item.java + ItemArmor.ArmorMaterial)."""
import re, sys, pathlib

SRC = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else str(pathlib.Path(__file__).resolve().parents[1] / ".local/src"))
OUT = pathlib.Path(__file__).resolve().parent.parent / "mc-client/src/gen_hud.rs"
mc = SRC / "net/minecraft"

pot = (mc / "potion/Potion.java").read_text()
rows = []
for m in re.finditer(r'public static final Potion \w+ = \(new \w+\((\d+), new ResourceLocation\("(\w+)"\), (true|false), (\d+)\)\)\.setPotionName\("([\w.]+)"\)(.*?);\n', pot):
    pid, rl, bad, color, name, rest = m.groups()
    ic = re.search(r'setIconIndex\((\d+), (\d+)\)', rest)
    icon = int(ic.group(1)) + int(ic.group(2)) * 8 if ic else -1
    eff = re.search(r'setEffectiveness\(([\d.]+)D\)', rest)
    rows.append((int(pid), rl, bad == "true", int(color), name, icon))
rows.sort()

item = (mc / "item/Item.java").read_text()
arm = (mc / "item/ItemArmor.java").read_text()
mats = {m.group(1): [int(x) for x in m.group(2).split(",")] for m in re.finditer(r'(\w+)\("\w+", \d+, new int\[\]\{([\d, ]+)\}, \d+\)', arm)}
armor = []
for m in re.finditer(r'registerItem\((\d+), \(String\)"(\w+)", \(new ItemArmor\(ItemArmor\.ArmorMaterial\.(\w+), \d+, (\d)\)\)', item):
    iid, name, mat, kind = m.groups()
    armor.append((int(iid), name, mats[mat][int(kind)], int(kind)))

o = ["// Gerado por scripts/gen_hud_tables.py a partir de Potion.java, Item.java e ItemArmor.java. Não editar.", ""]
o.append("/// (id, nome do recurso, efeito ruim, cor do líquido, chave do .lang, índice do ícone em inventory.png ou -1)")
o.append("pub const POTIONS: &[(u8, &str, bool, u32, &str, i32)] = &[")
for pid, rl, bad, color, name, icon in rows:
    o.append(f'    ({pid}, "{rl}", {str(bad).lower()}, {color}, "{name}", {icon}),')
o.append("];")
o.append("")
o.append("/// (id do item, nome, `damageReduceAmount`, tipo 0 capacete .. 3 botas)")
o.append("pub const ARMOR: &[(i16, &str, i32, u8)] = &[")
for iid, name, red, kind in armor:
    o.append(f'    ({iid}, "{name}", {red}, {kind}),')
o.append("];")
OUT.write_text("\n".join(o) + "\n")
print(len(rows), "poções,", len(armor), "armaduras")
