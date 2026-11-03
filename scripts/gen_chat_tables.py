#!/usr/bin/env python3
"""Gera mc-client/src/gen_chat_tables.rs a partir do Java decompilado (.local/src).

Tabelas que o chat precisa para os eventos de hover (`GuiScreen.handleComponentHover`):
- `ITEM_NAMES`: registro de itens (`Item.itemRegistry`): id -> nome, para `Item.getByNameOrId`;
- `ACHIEVEMENTS`: `AchievementList` (id da estatística, é especial?);
- `GENERAL_STATS`: `StatBasic` de `StatList` registrados (`getOneShotStat`);
- `ENTITY_IDS`: `EntityList.addMapping` (nome -> id), para `getIDFromString`.
"""
import re, sys, pathlib

src = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ".local/src/net/minecraft")
out = pathlib.Path(sys.argv[2] if len(sys.argv) > 2 else "mc-client/src/gen_chat_tables.rs")

block_src = (src / "block/Block.java").read_text()
blocks = {}
for m in re.finditer(r'registerBlock\((\d+), (?:\(String\)"(\w+)"|\(ResourceLocation\)AIR_ID)', block_src):
    blocks[m.group(2) or "air"] = int(m.group(1))

fields = {m.group(1): m.group(2) for m in re.finditer(r'(\w+) = (?:\(\w+\))?getRegisteredBlock\("(\w+)"\)', (src / "init/Blocks.java").read_text())}

item_src = (src / "item/Item.java").read_text()
items = {}
for m in re.finditer(r'registerItemBlock\(Blocks\.(\w+)', item_src):
    name = fields[m.group(1)]
    items[blocks[name]] = name
for m in re.finditer(r'registerItem\((\d+), \(String\)"(\w+)"', item_src):
    items[int(m.group(1))] = m.group(2)

ach_src = (src / "stats/AchievementList.java").read_text()
achievements = [(m.group(1), ".setSpecial()" in m.group(0)) for m in re.finditer(r'new Achievement\("(achievement\.\w+)".*', ach_src)]

stat_src = (src / "stats/StatList.java").read_text()
stats = [m.group(1) for m in re.finditer(r'new StatBasic\("(stat\.\w+)".*registerStat\(\)', stat_src)]

ent_src = (src / "entity/EntityList.java").read_text()
entities = [(m.group(1), int(m.group(2))) for m in re.finditer(r'addMapping\(\w+\.class, "(\w+)", (\d+)', ent_src)]

lines = ["//! Gerado por scripts/gen_chat_tables.py a partir do Java decompilado. Não editar.", ""]
lines.append("/// `Item.itemRegistry`: (id, nome sem o `minecraft:`), em ordem de id.")
lines.append("pub const ITEM_NAMES: &[(u16, &str)] = &[")
lines += [f'    ({i}, "{n}"),' for i, n in sorted(items.items())]
lines += ["];", "", "/// `AchievementList`: (id da estatística, `setSpecial`).", "pub const ACHIEVEMENTS: &[(&str, bool)] = &["]
lines += [f'    ("{n}", {str(s).lower()}),' for n, s in achievements]
lines += ["];", "", "/// `StatBasic` registrados em `StatList` (ids como `stat.leaveGame`).", "pub const GENERAL_STATS: &[&str] = &["]
lines += [f'    "{n}",' for n in stats]
lines += ["];", "", "/// `EntityList.addMapping`: (nome, id).", "pub const ENTITY_IDS: &[(&str, u16)] = &["]
lines += [f'    ("{n}", {i}),' for n, i in entities]
lines += ["];", ""]
out.write_text("\n".join(lines))
print(f"{len(items)} itens, {len(achievements)} conquistas, {len(stats)} estatísticas, {len(entities)} entidades -> {out}")
