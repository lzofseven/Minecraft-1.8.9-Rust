#!/usr/bin/env python3
"""Gera mc-client/src/gen_mob_models.rs a partir dos construtores dos ModelXxx do Java decompilado.

Nada é digitado à mão: cada construtor Java (`.local/src/net/minecraft/client/model/*.java`) é
traduzido linha a linha para Python e executado com um `ModelRenderer` simulado que registra
caixas (`addBox`), pontos de giro, rotações iniciais, espelho, tamanho de textura e filhos.
O resultado vira tabelas `GenModel` em Rust. Uso:  python3 scripts/gen_mob_models.py
"""
import ast, math, os, re, struct, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.environ.get("MC_SRC") or os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".local/src")
MODEL_DIR = os.path.join(SRC, "net/minecraft/client/model")
OUT = os.path.join(ROOT, "mc-client/src/gen_mob_models.rs")

# (nome Rust, classe Java, argumentos do construtor)
INSTANCES = [
    ("CREEPER", "ModelCreeper", [0.0]),
    ("CREEPER_CHARGED", "ModelCreeper", [2.0]),
    ("PIG", "ModelPig", [0.0]),
    ("PIG_SADDLE", "ModelPig", [0.5]),
    ("COW", "ModelCow", []),
    ("SHEEP_BODY", "ModelSheep2", []),
    ("SHEEP_WOOL", "ModelSheep1", []),
    ("CHICKEN", "ModelChicken", []),
    ("SPIDER", "ModelSpider", []),
    ("WOLF", "ModelWolf", []),
    ("OCELOT", "ModelOcelot", []),
    ("RABBIT", "ModelRabbit", []),
    ("SQUID", "ModelSquid", []),
    ("BAT", "ModelBat", []),
    ("SLIME_INNER", "ModelSlime", [16]),
    ("SLIME_OUTER", "ModelSlime", [0]),
    ("MAGMA_CUBE", "ModelMagmaCube", []),
    ("GHAST", "ModelGhast", []),
    ("BLAZE", "ModelBlaze", []),
    ("ENDERMAN", "ModelEnderman", [0.0]),
    ("ENDERMITE", "ModelEnderMite", []),
    ("SILVERFISH", "ModelSilverfish", []),
    ("WITCH", "ModelWitch", [0.0]),
    ("IRON_GOLEM", "ModelIronGolem", []),
    ("SNOWMAN", "ModelSnowMan", []),
    ("GUARDIAN", "ModelGuardian", []),
    ("WITHER", "ModelWither", [0.0]),
    ("WITHER_ARMOR", "ModelWither", [0.5]),
    ("DRAGON", "ModelDragon", [0.0]),
    ("HORSE", "ModelHorse", []),
    ("BOAT", "ModelBoat", []),
    ("MINECART", "ModelMinecart", []),
    ("ENDER_CRYSTAL", "ModelEnderCrystal", [0.0, True]),
    ("ENDER_CRYSTAL_NOBASE", "ModelEnderCrystal", [0.0, False]),
    ("ARMOR_STAND", "ModelArmorStand", []),
    ("LEASH_KNOT", "ModelLeashKnot", []),
    ("BOOK", "ModelBook", []),
]


def f32(x):
    return struct.unpack("f", struct.pack("f", float(x)))[0]


class MR:
    def __init__(self, model, a=None, b=None):
        self.model = model
        self.cubes = []
        self.children = []
        self.tw, self.th = model.textureWidth, model.textureHeight
        self.u, self.v = 0, 0
        self.rotationPointX = self.rotationPointY = self.rotationPointZ = 0.0
        self.rotateAngleX = self.rotateAngleY = self.rotateAngleZ = 0.0
        self.offsetX = self.offsetY = self.offsetZ = 0.0
        self.mirror = False
        self.showModel = True
        self.isHidden = False
        if isinstance(a, str):
            self.u, self.v = model.textureOffsets.get(a, (0, 0))
        elif a is not None:
            self.u, self.v = a, b
        model.all_renderers.append(self)

    def setTextureOffset(self, u, v):
        self.u, self.v = u, v
        return self

    def setTextureSize(self, w, h):
        self.tw, self.th = w, h
        return self

    def addChild(self, c):
        self.children.append(c)

    def setRotationPoint(self, x, y, z):
        self.rotationPointX, self.rotationPointY, self.rotationPointZ = x, y, z

    def addBox(self, *a):
        name = None
        if isinstance(a[0], str):
            name, a = a[0], a[1:]
            self.u, self.v = self.model.textureOffsets.get(name, (self.u, self.v))
        x, y, z, w, h, d = a[:6]
        infl = 0.0
        mir = self.mirror
        if len(a) == 7:
            if isinstance(a[6], bool):
                mir = a[6]
            else:
                infl = a[6]
        elif len(a) == 8:
            infl, mir = a[6], a[7]
        self.cubes.append((self.u, self.v, x, y, z, int(w), int(h), int(d), infl, mir))
        return self

    def render(self, *a): pass
    def renderWithRotation(self, *a): pass
    def postRender(self, *a): pass


class Model:
    def __init__(self):
        self.textureWidth = 64
        self.textureHeight = 32
        self.textureOffsets = {}
        self.all_renderers = []
        self.isChild = False
        self.boxList = []

    def setTextureOffset(self, name, u, v):
        self.textureOffsets[name] = (u, v)


def strip_comments(s):
    s = re.sub(r"/\*.*?\*/", "", s, flags=re.S)
    return re.sub(r"//[^\n]*", "", s)


def match_brace(s, i):
    d = 0
    while True:
        if s[i] == "{":
            d += 1
        elif s[i] == "}":
            d -= 1
            if d == 0:
                return i
        i += 1


class JavaClass:
    def __init__(self, name):
        self.name = name
        src = balanced_arrays(strip_comments(open(os.path.join(MODEL_DIR, name + ".java")).read()))
        m = re.search(r"class\s+%s(?:\s+extends\s+(\w+))?" % name, src)
        self.sup = m.group(1) if m else None
        self.ctors = []   # (params, body)
        self.methods = {}  # nome -> (params, body)
        i = src.index("{", m.end())
        end = match_brace(src, i)
        body = src[i + 1:end]
        pos = 0
        while pos < len(body):
            mm = re.compile(r"(?:public|private|protected)\s+(?:static\s+)?(?:(\w+(?:\[\])?)\s+)?(\w+)\s*\(([^)]*)\)\s*(?:throws\s+\w+\s*)?\{").search(body, pos)
            if not mm:
                break
            bs = mm.end() - 1
            be = match_brace(body, bs)
            params = [p.strip().split()[-1] for p in mm.group(3).split(",") if p.strip()]
            ptypes = [p.strip().split()[0] for p in mm.group(3).split(",") if p.strip()]
            if mm.group(2) == name and mm.group(1) is None:
                self.ctors.append((params, ptypes, body[bs + 1:be]))
            else:
                self.methods[mm.group(2)] = (params, body[bs + 1:be])
            pos = be + 1
        # inicializadores de campo (ex.: `public ModelRenderer head = new ModelRenderer(this, 0, 0);`)
        rest = body
        for (a, b) in sorted([(m_.start(), m_.end()) for m_ in []]):
            pass
        self.field_inits = []
        depth = 0
        out = []
        for ch in body:
            if ch == "{":
                depth += 1
            elif ch == "}":
                depth -= 1
            elif depth == 0:
                out.append(ch)
        top = "".join(out)
        for st in top.split(";"):
            mm = re.match(r"\s*(?:(?:public|protected|private)\s+)?(?:static\s+)?(?:final\s+)?[\w\[\]<>]+\s+(\w+)\s*=\s*(.+)$", st.strip(), re.S)
            if mm:
                self.field_inits.append((mm.group(1), mm.group(2)))


def balanced_arrays(src):
    """`new int[][]{{1,2},{3}}` -> `[[1,2],[3]]` (inicializadores de arrays primitivos)."""
    pat = re.compile(r"new\s+(?:int|float|double)(?:\[\])+\s*\{")
    while True:
        m = pat.search(src)
        if not m:
            return src
        i = m.end() - 1
        j = match_brace(src, i)
        inner = src[i:j + 1].replace("{", "[").replace("}", "]")
        src = src[:m.start()] + inner + src[j + 1:]


class JRandom:
    """java.util.Random (LCG de 48 bits), para os modelos que sorteiam com semente fixa."""
    def __init__(self, seed):
        self.seed = (seed ^ 0x5DEECE66D) & ((1 << 48) - 1)

    def _next(self, bits):
        self.seed = (self.seed * 0x5DEECE66D + 0xB) & ((1 << 48) - 1)
        r = self.seed >> (48 - bits)
        if r >= 1 << (bits - 1):
            r -= 1 << bits
        return r

    def nextInt(self, bound):
        if bound & -bound == bound:
            return (bound * (self._next(31) & 0x7FFFFFFF)) >> 31
        while True:
            bits = self._next(31) & 0x7FFFFFFF
            val = bits % bound
            if bits - val + (bound - 1) < (1 << 31):
                return val

    def nextFloat(self):
        return (self._next(24) & 0xFFFFFF) / float(1 << 24)


class JMath:
    abs = staticmethod(abs)
    sqrt = staticmethod(math.sqrt)
    sin = staticmethod(math.sin)
    cos = staticmethod(math.cos)
    signum = staticmethod(lambda x: (x > 0) - (x < 0) + 0.0)
    PI = math.pi


def jdiv(a, b):
    if isinstance(a, int) and isinstance(b, int) and not isinstance(a, bool):
        q = abs(a) // abs(b)
        return q if (a >= 0) == (b >= 0) else -q
    return a / b


def jmod(a, b):
    if isinstance(a, int) and isinstance(b, int):
        return a - b * jdiv(a, b)
    return math.fmod(a, b)


class _Ops(ast.NodeTransformer):
    def visit_BinOp(self, n):
        self.generic_visit(n)
        if isinstance(n.op, ast.Div):
            return ast.copy_location(ast.Call(ast.Name("jdiv", ast.Load()), [n.left, n.right], []), n)
        if isinstance(n.op, ast.Mod):
            return ast.copy_location(ast.Call(ast.Name("jmod", ast.Load()), [n.left, n.right], []), n)
        return n


def compile_java_py(code):
    tree = _Ops().visit(ast.parse(code))
    ast.fix_missing_locations(tree)
    return compile(tree, "<modelo>", "exec")


def cast_args(e):
    """`(float)x`, `(float)(expr)`, `(int)f(x)` -> `float(x)` etc., com um nível de parênteses."""
    operand = r"(\w+(?:\.\w+)*(?:\[[^\]]*\])*(?:\((?:[^()]|\([^()]*\))*\))?|\((?:[^()]|\((?:[^()]|\([^()]*\))*\))*\))"
    pat = re.compile(r"\(\s*(float|int|double|byte|long)\s*\)\s*" + operand)
    prev = None
    while prev != e:
        prev = e
        e = pat.sub(lambda m: ("int" if m.group(1) in ("int", "byte", "long") else "float") + "(" + m.group(2) + ")", e)
    return e


def java_expr(e, methods=()):
    e = re.sub(r"(?<![\w.])(\d+\.?\d*(?:[eE][-+]?\d+)?)[fFdDL]\b", r"\1", e)
    e = cast_args(e)
    e = e.replace("new ModelRenderer(this", "MR(self").replace("new Random(", "JRandom(")
    e = re.sub(r"new ModelRenderer\[(\w+)\]", r"[None]*(\1)", e)
    e = re.sub(r"new (?:float|int|double)\[(\w+)\]", r"[0.0]*(\1)", e)
    for mname in methods:
        e = e.replace("this." + mname + "(", mname + "(")
    e = e.replace("this.", "self.")
    e = re.sub(r"(self\.\w+|\b\w+)\.length\b", r"len(\1)", e)
    e = e.replace("Math.PI", "math.pi").replace("MathHelper.", "mh_").replace("Math.", "JMath.")
    e = re.sub(r"\btrue\b", "True", e)
    e = re.sub(r"\bfalse\b", "False", e)
    e = e.replace("&&", " and ").replace("||", " or ")
    e = re.sub(r"!(?!=)", " not ", e)
    return e


def rebuild(body, cls):
    """Tradução por varredura com contagem de chaves e parênteses (trata `for(;;)`)."""
    out = []
    ind = 0
    buf = ""
    par = 0
    for ch in body.replace("\n", " "):
        if ch == "(":
            par += 1
        elif ch == ")":
            par -= 1
        if par == 0 and ch == ";":
            out.append(("    " * ind) + stmt_py(buf.strip(), cls))
            buf = ""
        elif par == 0 and ch == "{":
            out.append(("    " * ind) + head_py(buf.strip(), cls))
            buf = ""
            ind += 1
        elif par == 0 and ch == "}":
            if buf.strip():
                out.append(("    " * ind) + stmt_py(buf.strip(), cls))
                buf = ""
            ind -= 1
        else:
            buf += ch
    return "\n".join(x for x in out if x.strip()) + "\n"


def head_py(h, cls):
    h = h.strip()
    ms = list(cls.methods)
    m = re.match(r"(?:\}\s*)?else\s+if\s*\((.*)\)\s*$", h, re.S)
    if m:
        return "elif " + java_expr(m.group(1), ms) + ":"
    if re.match(r"(?:\}\s*)?else$", h):
        return "else:"
    m = re.match(r"if\s*\((.*)\)\s*$", h, re.S)
    if m:
        return "if " + java_expr(m.group(1), ms) + ":"
    m = re.match(r"for\s*\(\s*(?:int|float)\s+(\w+)\s*=\s*(.+?)\s*;\s*\1\s*(<|<=)\s*(.+?)\s*;\s*(?:\+\+\1|\1\+\+)\s*\)\s*$", h, re.S)
    if m:
        hi = java_expr(m.group(4), ms) + ("+1" if m.group(3) == "<=" else "")
        return f"for {m.group(1)} in range(int({java_expr(m.group(2), ms)}), int({hi})):"
    raise RuntimeError("cabeçalho não suportado: " + h)


def stmt_py(s, cls):
    s = s.strip()
    if not s:
        return ""
    ms = list(cls.methods)
    if s.startswith("super("):
        return f"_super_call(self, {java_expr(s[len('super('):-1], ms)})"
    if s.startswith("this(") and not s.startswith("this."):
        return f"_this_call(self, {java_expr(s[len('this('):-1], ms)})"
    s = re.sub(r"^(?:final\s+)?(?:int|float|double|boolean|Random|ModelRenderer(?:\[\])?)\s+(\w+)\s*=", r"\1 =", s)
    s = re.sub(r"^(?:final\s+)?(?:int|float|double|boolean)\s+(\w+)\s*$", r"\1 = 0", s)
    if re.fullmatch(r"\+\+\w+", s):
        s = s[2:] + " += 1"
    return java_expr(s, ms)


def instantiate(cls_name, args):
    obj = Model()
    run_chain(JavaClass(cls_name), args, obj)
    return obj


def run_chain(cls, args, obj):
    if cls.name == "ModelBase":
        return
    cand = [c for c in cls.ctors if len(c[0]) == len(args)]
    if not cand:
        raise RuntimeError(f"{cls.name}: sem construtor de {len(args)} args")
    params, ptypes, body = cand[0]
    env = {"MR": MR, "math": math, "self": obj, "mh_cos": math.cos, "mh_sin": math.sin,
           "mh_abs": abs, "mh_sqrt_float": math.sqrt, "JRandom": JRandom, "JMath": JMath,
           "jdiv": jdiv, "jmod": jmod}
    for p, a in zip(params, args):
        env[p] = a

    def _super_call(o, *a):
        run_chain(JavaClass(cls.sup), list(a), obj)

    def _this_call(o, *a):
        run_chain(cls, list(a), obj)

    env["_super_call"] = _super_call
    env["_this_call"] = _this_call
    for mname, (mparams, mbody) in cls.methods.items():
        if mname in ("render", "setRotationAngles", "setLivingAnimations"):
            continue
        try:
            src = "def %s(%s):\n" % (mname, ", ".join(mparams)) + "\n".join("    " + l for l in rebuild(mbody, cls).splitlines()) + "\n"
            exec(compile_java_py(src), env)
        except Exception:
            pass
    first = body.strip()
    if cls.sup and os.path.exists(os.path.join(MODEL_DIR, cls.sup + ".java")) and not first.startswith("super(") and not first.startswith("this("):
        run_chain(JavaClass(cls.sup), [], obj)
    if not first.startswith("this("):
        for fname, fexpr in cls.field_inits:
            v = eval(compile_java_py("_v = " + java_expr(fexpr, list(cls.methods)) + "\n"), env) if False else None
            exec(compile_java_py("_v = " + java_expr(fexpr, list(cls.methods)) + "\n"), env)
            setattr(obj, fname, env["_v"])
            env[fname] = env["_v"]
    exec(compile_java_py(rebuild(body, cls)), env)


# ---------- saída Rust ----------

def num(x):
    x = f32(x)
    if x == int(x) and abs(x) < 1e9:
        return "%d.0" % int(x)
    return repr(x) if len(repr(x)) <= 12 else ("%.9g" % x)


def collect(obj):
    names = {}
    for k, v in obj.__dict__.items():
        if k in ("all_renderers", "boxList"):
            continue
        if isinstance(v, MR):
            names[id(v)] = k
        elif isinstance(v, list):
            for i, e in enumerate(v):
                if isinstance(e, MR):
                    names[id(e)] = f"{k}_{i}"
    return names


def emit_model(rname, obj):
    names = collect(obj)
    parts = [r for r in obj.all_renderers if id(r) in names]
    # filhos anônimos (não nomeados) viram nomes derivados
    seen = {id(p) for p in parts}
    # renderers sobrescritos pelo construtor da subclasse (ex.: ModelCow refaz head/body/legs do
    # ModelQuadruped) ficam órfãos e o Java nunca os desenha: só entram os alcançáveis por filhos.
    reach = set(seen)
    stack = list(parts)
    while stack:
        for c in stack.pop().children:
            if id(c) not in reach:
                reach.add(id(c))
                stack.append(c)
    extra = [r for r in obj.all_renderers if id(r) not in seen and id(r) in reach]
    for k, r in enumerate(extra):
        names[id(r)] = f"anon_{k}"
    parts = parts + extra
    index = {id(p): i for i, p in enumerate(parts)}
    lines = []
    lines.append(f"pub const {rname}: GenModel = GenModel {{ tex: ({obj.textureWidth}, {obj.textureHeight}), parts: &[")
    for p in parts:
        cubes = ", ".join(
            "GenCube { uv: (%d, %d), from: [%s, %s, %s], size: [%d, %d, %d], inflate: %s, mirror: %s }"
            % (c[0], c[1], num(c[2]), num(c[3]), num(c[4]), c[5], c[6], c[7], num(c[8]), "true" if c[9] else "false")
            for c in p.cubes)
        kids = ", ".join(str(index[id(c)]) for c in p.children if id(c) in index)
        lines.append(
            "    GenPart { name: \"%s\", pivot: [%s, %s, %s], rot: [%s, %s, %s], tex: (%d, %d), cubes: &[%s], children: &[%s] },"
            % (names[id(p)], num(p.rotationPointX), num(p.rotationPointY), num(p.rotationPointZ),
               num(p.rotateAngleX), num(p.rotateAngleY), num(p.rotateAngleZ), p.tw, p.th, cubes, kids))
    lines.append("] };")
    return "\n".join(lines)


def sheep_colors():
    """EntitySheep.DYE_TO_RGB indexado pela metadata de EnumDyeColor."""
    dye = open(os.path.join(SRC, "net/minecraft/item/EnumDyeColor.java")).read()
    meta = {m.group(1): int(m.group(2)) for m in re.finditer(r"^\s*(\w+)\((\d+), \d+, \"", dye, re.M)}
    sh = open(os.path.join(SRC, "net/minecraft/entity/passive/EntitySheep.java")).read()
    rows = {}
    for m in re.finditer(r"DYE_TO_RGB\.put\(EnumDyeColor\.(\w+), new float\[\]\{([^}]*)\}", sh):
        rows[meta[m.group(1)]] = [x.strip().rstrip("Ff") for x in m.group(2).split(",")]
    assert sorted(rows) == list(range(16))
    body = ", ".join("[" + ", ".join(num(float(v)) for v in rows[i]) + "]" for i in range(16))
    return "\n/// `EntitySheep.DYE_TO_RGB` pela metadata do `EnumDyeColor`.\npub const SHEEP_COLORS: [[f32; 3]; 16] = [" + body + "];"


def main():
    out = ["// GERADO por scripts/gen_mob_models.py a partir dos construtores dos ModelXxx do Java 1.8.9. Não editar.",
           "use crate::mob_rig::{GenCube, GenModel, GenPart};", ""]
    ok = bad = 0
    for rname, cname, args in INSTANCES:
        try:
            obj = instantiate(cname, args)
            out.append(emit_model(rname, obj))
            ok += 1
        except Exception as e:
            sys.stderr.write(f"FALHOU {rname} ({cname}): {type(e).__name__}: {e}\n")
            bad += 1
    out.append(sheep_colors())
    open(OUT, "w").write("\n".join(out) + "\n")
    print(f"{ok} modelos gerados, {bad} falhas -> {OUT}")


if __name__ == "__main__":
    main()
