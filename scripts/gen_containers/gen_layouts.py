#!/usr/bin/env python3
"""Lê os construtores de net/minecraft/inventory/Container*.java e grava o layout dos slots
(tipo, x, y na ordem de addSlotToContainer) em mc-client/src/gen_containers.rs.
Uso: gen_layouts.py <pasta .local/src> <saida.rs>"""
import re, sys

src, out = sys.argv[1], sys.argv[2]
INV = src + "/net/minecraft/inventory/"

# (arquivo, nome Rust, variantes de parâmetros)
TARGETS = [
    ("ContainerPlayer", "PLAYER", [{}]),
    ("ContainerWorkbench", "WORKBENCH", [{}]),
    ("ContainerFurnace", "FURNACE", [{}]),
    ("ContainerDispenser", "DISPENSER", [{}]),
    ("ContainerHopper", "HOPPER", [{"size": 5}]),
    ("ContainerEnchantment", "ENCHANTMENT", [{}]),
    ("ContainerBrewingStand", "BREWING_STAND", [{}]),
    ("ContainerRepair", "REPAIR", [{}]),
    ("ContainerBeacon", "BEACON", [{}]),
    ("ContainerMerchant", "MERCHANT", [{}]),
    ("ContainerHorseInventory", "HORSE", [{"chested": False}, {"chested": True}]),
    ("ContainerChest", "CHEST", [{"numRows": r} for r in range(1, 7)]),
]
KINDS = {
    "Slot": "Plain", "SlotCrafting": "Crafting", "SlotFurnaceFuel": "FurnaceFuel", "SlotFurnaceOutput": "FurnaceOutput",
    "ContainerBrewingStand.Potion": "BrewPotion", "ContainerBrewingStand.Ingredient": "BrewIngredient",
    "ContainerBeacon.BeaconSlot": "Beacon", "SlotMerchantResult": "MerchantResult",
}


def split_args(s):
    args, depth, cur = [], 0, ""
    for ch in s:
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
        if ch == "," and depth == 0:
            args.append(cur.strip())
            cur = ""
        else:
            cur += ch
    args.append(cur.strip())
    return args


def ev(expr, scope):
    e = expr.replace("this.", "")
    e = re.sub(r"\w+\.getSizeInventory\(\)", str(scope.get("size", 0)), e)
    return int(eval(e, {}, dict(scope)))


def constructor_body(text, cls):
    """Corpo do construtor com mais chamadas addSlotToContainer."""
    best = ""
    for m in re.finditer(r"public %s\(" % cls, text):
        i = text.index("{", m.start())
        depth, j = 0, i
        while True:
            if text[j] == "{":
                depth += 1
            elif text[j] == "}":
                depth -= 1
                if depth == 0:
                    break
            j += 1
        body = text[i + 1:j]
        if body.count("addSlotToContainer") > best.count("addSlotToContainer"):
            best = body
    return best


def layout(cls, params):
    body = constructor_body(open(INV + cls + ".java").read(), cls)
    slots = []
    scope = dict(params)
    stack = []  # (variável do for, valores restantes, linhas) simplificado: executa por recursão
    lines = [l.strip() for l in body.split("\n") if l.strip()]

    def run(lines, scope):
        i = 0
        while i < len(lines):
            l = lines[i]
            m = re.match(r"for\((?:final )?int (\w+) = ([^;]+); \1 < ([^;]+); \+\+\1\) \{", l)
            if m:
                depth, j = 1, i + 1
                while depth:
                    depth += lines[j].count("{") - lines[j].count("}")
                    j += 1
                inner = lines[i + 1:j - 1]
                v, lo, hi = m.group(1), ev(m.group(2), scope), ev(m.group(3), scope)
                for k in range(lo, hi):
                    s2 = dict(scope)
                    s2[v] = k
                    run(inner, s2)
                i = j
                continue
            m = re.match(r"if\(\w+\.isChested\(\)\) \{", l)
            if m:
                depth, j = 1, i + 1
                while depth:
                    depth += lines[j].count("{") - lines[j].count("}")
                    j += 1
                if scope.get("chested"):
                    run(lines[i + 1:j - 1], scope)
                i = j
                continue
            m = re.match(r"int (\w+) = (.+);$", l)
            if m and "new " not in l:
                try:
                    scope[m.group(1)] = ev(m.group(2), scope)
                except Exception:
                    pass
                i += 1
                continue
            if "addSlotToContainer(" in l:
                m = re.search(r"new ([\w.]+)\((.*)$", l)
                name = m.group(1)
                # junta o resto da chamada até fechar os parênteses
                text, k = m.group(2), i
                depth = 1 + sum(1 if c == "(" else -1 if c == ")" else 0 for c in "")
                depth = 1
                buf = ""
                for ch in text:
                    if ch == "(":
                        depth += 1
                    elif ch == ")":
                        depth -= 1
                        if depth == 0:
                            break
                    buf += ch
                args = split_args(buf)
                x, y = ev(args[-2], scope), ev(args[-1], scope)
                kind = KINDS.get(name)
                anon = l.rstrip().endswith("{")
                if kind is None or (anon and name == "Slot"):
                    kind = "Plain"
                slots.append((kind, x, y, anon))
                if anon:  # pula o corpo da classe anônima
                    depth, j = 1, i + 1
                    while depth:
                        depth += lines[j].count("{") - lines[j].count("}")
                        j += 1
                    i = j
                    continue
            i += 1

    run(lines, scope)
    return slots


with open(out, "w") as f:
    f.write("//! Gerado por scripts/gen_containers/gen.sh a partir dos construtores de net.minecraft.inventory.Container*.\n")
    f.write("//! Layout dos slots do container (sem o inventário do jogador, que vem depois quando o Java o adiciona).\n")
    f.write("//! `anon` marca slots de classe anônima (isItemValid/limite próprios, tratados em containers.rs). Não editar.\n\n")
    f.write("#[derive(Clone, Copy, Debug, PartialEq, Eq)]\n")
    f.write("pub enum Kind {\n    Plain,\n    Crafting,\n    FurnaceFuel,\n    FurnaceOutput,\n    BrewPotion,\n    BrewIngredient,\n    Beacon,\n    MerchantResult,\n}\n\n")
    f.write("/// (tipo, x, y, classe anônima).\npub type Def = (Kind, i32, i32, bool);\n\n")
    for cls, name, variants in TARGETS:
        res = [layout(cls, dict(p)) for p in variants]
        if len(variants) == 1:
            f.write("/// %s\npub const %s: &[Def] = &[\n" % (cls, name))
            for k, x, y, a in res[0]:
                f.write("    (Kind::%s, %d, %d, %s),\n" % (k, x, y, str(a).lower()))
            f.write("];\n\n")
        else:
            for p, r in zip(variants, res):
                tag = "_".join(str(v).upper() for v in p.values())
                f.write("/// %s %s\npub const %s_%s: &[Def] = &[\n" % (cls, p, name, tag))
                for k, x, y, a in r:
                    f.write("    (Kind::%s, %d, %d, %s),\n" % (k, x, y, str(a).lower()))
                f.write("];\n\n")
    names = re.search(r'namePartsArray = "([^"]*)"', open(src + "/net/minecraft/util/EnchantmentNameParts.java").read()).group(1).split()
    f.write("/// `EnchantmentNameParts.namePartsArray`.\npub const ENCHANT_NAME_PARTS: &[&str] = &[%s];\n\n" % ", ".join('"%s"' % n for n in names))
    f.write("pub const CHEST_ROWS: [&[Def]; 6] = [CHEST_1, CHEST_2, CHEST_3, CHEST_4, CHEST_5, CHEST_6];\n")
