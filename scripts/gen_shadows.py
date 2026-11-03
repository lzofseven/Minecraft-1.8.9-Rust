#!/usr/bin/env python3
"""Gera mc-client/src/gen_shadows.rs: tamanho e opacidade da sombra (`Render.shadowSize`/`shadowOpaque`)
de cada entidade, avaliando os construtores dos `Render*` do Java decompilado.

Nada digitado à mão: lê `EntityList` (id -> classe), `RenderManager` (classe -> renderizador e os
argumentos do construtor), o `handleSpawnObject` do `NetHandlerPlayClient` (tipo de objeto -> classe)
e percorre a cadeia de construtores `super(...)` dos renderizadores, aplicando `this.shadowSize = x`
e `*= x`. O que `doRender` troca em tempo de execução (lesma: `0.25 * tamanho`; aldeão filhote) fica
sinalizado em `RUNTIME_SHADOW`. Uso:  python3 scripts/gen_shadows.py
"""
import os, re

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.environ.get("MC_SRC") or os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".local/src")
MC = os.path.join(SRC, "net/minecraft")
OUT = os.path.join(ROOT, "mc-client/src/gen_shadows.rs")
RDIR = os.path.join(MC, "client/renderer/entity")
TDIR = os.path.join(MC, "client/renderer/tileentity")


def read(path):
    return open(path, encoding="utf-8").read()


def render_source(name):
    for d in (RDIR, TDIR):
        p = os.path.join(d, name + ".java")
        if os.path.exists(p):
            return read(p)
    return None


def split_args(s):
    """Divide argumentos no nível de topo (parênteses balanceados)."""
    out, depth, cur = [], 0, ""
    for ch in s:
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
        if ch == "," and depth == 0:
            out.append(cur.strip())
            cur = ""
        else:
            cur += ch
    if cur.strip():
        out.append(cur.strip())
    return out


def eval_expr(e, env):
    """Float literal, parâmetro, produto `a * b`; qualquer outra coisa (modelos, RenderManager) é None."""
    e = e.strip()
    m = re.fullmatch(r"([0-9]*\.?[0-9]+)[Ff]", e)
    if m:
        return float(m.group(1))
    if re.fullmatch(r"\w+", e):
        return env.get(e)
    if "*" in e and "(" not in e:
        parts = [eval_expr(p, env) for p in e.split("*")]
        if all(p is not None for p in parts):
            r = 1.0
            for p in parts:
                r *= p
            return r
    return None


def f32(x):
    import struct
    return struct.unpack("f", struct.pack("f", x))[0]


def shadow_of(cls, args, depth=0):
    """(shadowSize, shadowOpaque) do renderizador `cls` construído com `args` (valores avaliados)."""
    if cls == "Render":
        return 0.0, 1.0
    src = render_source(cls)
    if src is None:
        raise SystemExit(f"sem fonte para {cls}")
    parent = re.search(r"class\s+%s(?:<[^{]*?>)?\s+extends\s+(\w+)" % cls, src).group(1)
    # Construtor com a mesma quantidade de argumentos (RenderBiped tem dois, ligados por `this(...)`).
    ctors = re.findall(r"public\s+%s\s*\(([^)]*)\)\s*\{(.*?)\n   \}" % cls, src, re.S)
    chosen = None
    for plist, body in ctors:
        names = [p.split()[-1] for p in split_args(plist)] if plist.strip() else []
        if len(names) == len(args) or chosen is None:
            chosen = (names, body)
            if len(names) == len(args):
                break
    params, body = chosen if chosen else ([], "")
    env = {n: (args[i] if i < len(args) else None) for i, n in enumerate(params)}
    if cls == "RendererLivingEntity":
        size = env.get(params[2]) if len(params) > 2 else 0.0
        return (size or 0.0), 1.0
    sup = re.search(r"\b(super|this)\((.*)\);", body)
    sargs = [eval_expr(a, env) for a in split_args(sup.group(2))] if sup else []
    target = cls if sup and sup.group(1) == "this" else parent
    size, opaque = shadow_of(target, sargs, depth + 1)
    for op, val in re.findall(r"this\.shadowSize\s*(\*?=)\s*([0-9.]+)F;", body):
        v = float(val)
        size = f32(size * v) if op == "*=" else v
    for val in re.findall(r"this\.shadowOpaque\s*=\s*([0-9.]+)F;", body):
        opaque = float(val)
    return f32(size), opaque


entity_list = read(os.path.join(MC, "entity/EntityList.java"))
id_class = {}
for cls, num in re.findall(r"addMapping\((\w+)\.class,\s*[^,]+,\s*(\d+)", entity_list):
    id_class[int(num)] = cls

manager = read(os.path.join(RDIR, "RenderManager.java"))
class_render = {}
for ecls, rcls, rest in re.findall(r"entityRenderMap\.put\((\w+)\.class,\s*new\s+(\w+)(?:<[^>]*>)?\(this(.*?)\)\);", manager):
    args = [eval_expr(a, {}) for a in split_args(rest.lstrip(", "))] if rest.strip() else []
    class_render[ecls] = (rcls, [None] + args)  # primeiro argumento = RenderManager

net = read(os.path.join(MC, "client/network/NetHandlerPlayClient.java"))
obj_class = {}
spawn = net[net.index("public void handleSpawnObject"):net.index("if(entity != null) {", net.index("public void handleSpawnObject"))]
# o corpo das ramificações varia (peixe, moldura...): pega o primeiro `new EntityXxx` depois de cada `getType() == N`
for m in re.finditer(r"getType\(\)\s*==\s*(\d+)\)\s*\{(.*?)\n      (?:\}|\} else)", spawn, re.S):
    t, body = int(m.group(1)), m.group(2)
    c = re.search(r"new\s+(Entity\w+)\(", body)
    obj_class.setdefault(t, c.group(1) if c else "EntityMinecart")

# minecart (tipo 10): todos os subtipos usam renderizadores com sombra 0.5 (RenderMinecart / RenderTntMinecart / spawner)
lookup = {}


def classes_shadow(ecls):
    if ecls in lookup:
        return lookup[ecls]
    # `EntityMinecartChest` etc. caem no `Entity.class`? Não: o mapa tem EntityMinecart(Empty/TNT/Spawner).
    key = ecls
    if key not in class_render and ecls.startswith("EntityMinecart"):
        key = "EntityMinecart"
    if key not in class_render:
        lookup[ecls] = None
        return None
    r, a = class_render[key]
    lookup[ecls] = shadow_of(r, a)
    return lookup[ecls]


mob_rows = []
for i, ecls in sorted(id_class.items()):
    s = classes_shadow(ecls)
    if s is not None:
        mob_rows.append((i, ecls, s))
obj_rows = []
for t, ecls in sorted(obj_class.items()):
    s = classes_shadow(ecls)
    if s is not None:
        obj_rows.append((t, ecls, s))

# Tamanhos que `doRender` troca a cada quadro.
runtime = []
for name in ("RenderSlime", "RenderVillager"):
    runtime.append(name)

lines = ["// GERADO por scripts/gen_shadows.py avaliando os construtores dos Render* do Java 1.8.9. Não editar.", ""]
lines.append("/// `Render.shadowSize` e `shadowOpaque` das entidades do `EntityList` (id do mob -> classe Java -> sombra).")
lines.append("pub const MOB_SHADOW: &[(u8, &str, f32, f32)] = &[")
for i, ecls, (sz, op) in mob_rows:
    lines.append(f'    ({i}, "{ecls}", {sz!r}, {op!r}),')
lines.append("];")
lines.append("")
lines.append("/// Mesmo, por tipo do `S0EPacketSpawnObject` (via `handleSpawnObject`).")
lines.append("pub const OBJECT_SHADOW: &[(u8, &str, f32, f32)] = &[")
for t, ecls, (sz, op) in obj_rows:
    lines.append(f'    ({t}, "{ecls}", {sz!r}, {op!r}),')
lines.append("];")
lines.append("")
xp = classes_shadow("EntityXPOrb")
lines.append("/// `RenderXPOrb` (orbe de XP, `S11`).")
lines.append(f"pub const XP_ORB_SHADOW: (f32, f32) = ({xp[0]!r}, {xp[1]!r});")
lines.append("")
pl = shadow_of("RenderPlayer", [None, None])
lines.append("/// `RenderPlayer` (jogadores, S0C).")
lines.append(f"pub const PLAYER_SHADOW: (f32, f32) = ({pl[0]!r}, {pl[1]!r});")
lines.append("")
lines.append("/// Renderizadores que trocam `shadowSize` dentro do `doRender` (ver `entities.rs::shadow_size`).")
lines.append("pub const RUNTIME_SHADOW: &[&str] = &[" + ", ".join(f'"{r}"' for r in runtime) + "];")
open(OUT, "w", encoding="utf-8").write("\n".join(lines) + "\n")
print(f"{OUT}: {len(mob_rows)} mobs, {len(obj_rows)} objetos")
