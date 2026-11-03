#!/usr/bin/env python3
"""Gera mc-client/src/block_physics.rs a partir do código decompilado da 1.8.9.

Lê `Block.registerBlocks` (id -> classe e argumentos), os construtores das classes
(material), `Material.java` (flags) e as sobrescritas de `slipperiness`,
`isFullCube`, `isOpaqueCube` e `canProvidePower`. Nada é escrito à mão: se o
script não conseguir resolver algo, ele para com erro.

Uso: python3 scripts/gen_block_physics.py <.local/src> > mc-client/src/block_physics.rs
"""
import re
import sys
from pathlib import Path

SRC = Path(sys.argv[1]) / "net/minecraft/block"


def read(name):
    p = SRC / f"{name}.java"
    return p.read_text() if p.exists() else None


# --- Materiais -----------------------------------------------------------------
mat_src = (SRC / "material/Material.java").read_text()
MATERIALS = {}
for m in re.finditer(r"public static final Material (\w+) = (.*?);\n", mat_src, re.S):
    name, expr = m.group(1), m.group(2)
    kind = re.search(r"new (Material\w*)\(", expr).group(1)
    translucent = "setTranslucent()" in expr
    liquid = kind == "MaterialLiquid"
    solid = kind == "Material"
    blocks = kind == "Material" and "blocksMovement" not in expr
    MATERIALS[name] = dict(liquid=liquid, solid=solid, blocks=blocks, opaque=(not translucent) and blocks)
MAT_NAMES = list(MATERIALS)

# --- Registro dos blocos ---------------------------------------------------------
blk = (SRC / "Block.java").read_text()
body = blk[blk.index("public static void registerBlocks()"):]
body = body[: body.index("\n   }\n")]
vars_ = {}
for m in re.finditer(r"Block (\w+) = (.*?);\n", body):
    vars_[m.group(1)] = m.group(2)
regs = {}
for m in re.finditer(r"registerBlock\((\d+), \([\w]+\)[\w\"]+?, (.*?)\);\n", body):
    expr = m.group(2)
    regs[int(m.group(1))] = vars_.get(expr.strip(), expr)

extends = {}


def class_src(cls):
    s = read(cls)
    if s is None:
        raise SystemExit(f"classe não encontrada: {cls}")
    e = re.search(r"class \w+(?:<[^>]*>)? extends (\w+)", s)
    extends[cls] = e.group(1) if e else None
    return s


def chain(cls):
    out = []
    while cls and cls != "Object":
        out.append(cls)
        class_src(cls)
        cls = extends.get(cls)
    return out


def ctor_material(cls, args):
    """Material de `new cls(args)`."""
    m = re.search(r"Material\.(\w+)", args)
    if m and m.group(1) in MATERIALS:
        return m.group(1)
    m = re.match(r"(\w+)(?:\.getDefaultState\(\).*)?$", args.strip())
    if m and m.group(1) in vars_:  # escadas e muros: material do bloco de origem
        return expr_material(vars_[m.group(1)])
    for c in chain(cls):
        s = class_src(c)
        for cm in re.finditer(rf"(?:public |protected |private )?{c}\((.*?)\) \{{(.*?)\n   \}}", s, re.S):
            mm = re.search(r"(?:super|this)\((.*?)\);", cm.group(2))
            if mm:
                found = re.search(r"Material\.(\w+)", mm.group(1))
                if found and found.group(1) in MATERIALS:
                    return found.group(1)
    raise SystemExit(f"material não resolvido: {cls}({args})")


def new_args(expr):
    """Classe e argumentos (parênteses balanceados) do primeiro `new X(...)`."""
    m = re.search(r"new (\w+)(?:\.\w+)?\(", expr)
    i, depth = m.end(), 1
    while depth:
        depth += {"(": 1, ")": -1}.get(expr[i], 0)
        i += 1
    return m.group(1), expr[m.end(): i - 1]


def expr_material(expr):
    return ctor_material(*new_args(expr))


def expr_class(expr):
    if "new " not in expr:
        raise SystemExit(f"registro sem new: {expr}")
    # Classe interna (BlockBanner.BlockBannerStanding): as regras vêm da externa.
    return re.search(r"new (\w+)[.(]", expr).group(1)


def method_return(cls, name):
    """Valor literal de `name()` na cadeia de classes (None = expressão)."""
    for c in chain(cls):
        s = class_src(c)
        m = re.search(rf"public (?:boolean|float|int) {name}\(\) \{{\s*return (.*?);\s*\}}", s)
        if m:
            return c, m.group(1)
    return None, None


def slipperiness(cls):
    for c in chain(cls):
        m = re.search(r"this\.slipperiness = ([\d.]+)F;", class_src(c))
        if m and c != "Block":
            return m.group(1)
    return re.search(r"this\.slipperiness = ([\d.]+)F;", blk).group(1)


def full_cube(id_, cls):
    c, v = method_return(cls, "isFullCube")
    if v in ("true", "false"):
        return v == "true"
    if v == "this.isDouble()":
        # BlockSlab: a classe "Double" de cada laje devolve true.
        return "Double" in regs[id_]
    raise SystemExit(f"isFullCube não literal em {c}: {v}")


def provides_power(cls):
    c, v = method_return(cls, "canProvidePower")
    if v in ("true", "false"):
        return v == "true"
    raise SystemExit(f"canProvidePower não literal em {c}: {v}")


rows = []
for id_ in range(256):
    if id_ not in regs:
        rows.append(None)
        continue
    expr = regs[id_]
    cls = expr_class(expr)
    mat = expr_material(expr)
    m = MATERIALS[mat]
    normal = m["opaque"] and full_cube(id_, cls) and not provides_power(cls)
    rows.append((mat, slipperiness(cls), normal, cls))

print("//! GERADO por scripts/gen_block_physics.py a partir do código decompilado da 1.8.9.")
print("//! Não editar à mão.\n")
print("#![allow(dead_code)]\n")
print("/// Flags de `Material`: líquido, sólido, bloqueia movimento, opaco.")
print("#[derive(Clone, Copy, Debug, PartialEq, Eq)]")
print("pub struct Material {\n    pub name: &'static str,\n    pub liquid: bool,\n    pub solid: bool,\n    pub blocks_movement: bool,\n    pub opaque: bool,\n}\n")
print("pub const MATERIALS: [Material; %d] = [" % len(MAT_NAMES))
for n in MAT_NAMES:
    f = MATERIALS[n]
    print(f"    Material {{ name: \"{n}\", liquid: {str(f['liquid']).lower()}, solid: {str(f['solid']).lower()}, blocks_movement: {str(f['blocks']).lower()}, opaque: {str(f['opaque']).lower()} }},")
print("];\n")
for i, n in enumerate(MAT_NAMES):
    print(f"pub const M_{n.upper()}: u8 = {i};")
print()
print("/// Por id de bloco: (material, `slipperiness`, `isNormalCube`).")
print("pub const BLOCKS: [(u8, f32, bool); 256] = [")
for id_, r in enumerate(rows):
    if r is None:
        print(f"    ({MAT_NAMES.index('air')}, 0.6, false), // {id_}: sem bloco")
    else:
        mat, sl, normal, cls = r
        print(f"    ({MAT_NAMES.index(mat)}, {sl}, {str(normal).lower()}), // {id_}: {cls} ({mat})")
print("];")

# Blocos com `getRenderType() == 2` (baú e afins, desenhados por TileEntity): o LayerHeldItem os ajusta.
rt2 = [i for i, r in enumerate(rows) if r and method_return(r[3], "getRenderType")[1] == "2"]
print("\n/// Ids com `getRenderType() == 2`.")
print(f"pub const RENDER_TYPE_2: &[u16] = &{rt2};")
