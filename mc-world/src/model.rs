//! Modelos de bloco da 1.8: `models/block/*.json` com herança e `blockstates/*.json`
//! com variantes, assados em quadriláteros prontos (como o `ModelBakery`/`FaceBakery`).
//!
//! Posições ficam em blocos (0..1, relativas ao canto do bloco) e as UVs em pixels
//! da textura (0..16); quem desenha converte para o atlas.

use std::collections::HashMap;

use serde_json::Value;

/// Lê um arquivo do jar pelo caminho completo (`assets/minecraft/...`).
pub type Loader<'a> = dyn FnMut(&str) -> Option<Vec<u8>> + 'a;

const DOWN: usize = 0;
const UP: usize = 1;
const NORTH: usize = 2;
const SOUTH: usize = 3;
const WEST: usize = 4;
const EAST: usize = 5;

const FACE_NAMES: [&str; 6] = ["down", "up", "north", "south", "west", "east"];
const DIRS: [[f32; 3]; 6] = [[0., -1., 0.], [0., 1., 0.], [0., 0., -1.], [0., 0., 1.], [-1., 0., 0.], [1., 0., 0.]];

/// Brilho fixo de cada face (`FaceBakery.getFaceBrightness`).
pub const FACE_SHADE: [f32; 6] = [0.5, 1.0, 0.8, 0.8, 0.6, 0.6];

/// Ordem dos cantos de cada face (`EnumFaceDirection`): índices em
/// `[minY, maxY, minZ, maxZ, minX, maxX]` (o mesmo índice das faces).
pub const VERTEX_INFO: [[[usize; 3]; 4]; 6] = [
    [[WEST, DOWN, SOUTH], [WEST, DOWN, NORTH], [EAST, DOWN, NORTH], [EAST, DOWN, SOUTH]],
    [[WEST, UP, NORTH], [WEST, UP, SOUTH], [EAST, UP, SOUTH], [EAST, UP, NORTH]],
    [[EAST, UP, NORTH], [EAST, DOWN, NORTH], [WEST, DOWN, NORTH], [WEST, UP, NORTH]],
    [[WEST, UP, SOUTH], [WEST, DOWN, SOUTH], [EAST, DOWN, SOUTH], [EAST, UP, SOUTH]],
    [[WEST, UP, NORTH], [WEST, DOWN, NORTH], [WEST, DOWN, SOUTH], [WEST, UP, SOUTH]],
    [[EAST, UP, SOUTH], [EAST, DOWN, SOUTH], [EAST, DOWN, NORTH], [EAST, UP, NORTH]],
];

fn face_index(name: &str) -> Option<usize> {
    FACE_NAMES.iter().position(|n| *n == name)
}

#[derive(Clone, Debug, PartialEq)]
pub struct BakedQuad {
    pub pos: [[f32; 3]; 4],
    pub uv: [[f32; 2]; 4],
    pub tex: u16,
    /// `tintindex` (-1 = sem tinta).
    pub tint: i32,
    /// Face calculada pela normal (usada na luz).
    pub face: u8,
    /// Face que esconde o quad quando o vizinho tapa (`cullface` já girada).
    pub cull: Option<u8>,
    /// Brilho da face (1.0 com `shade: false`).
    pub shade: f32,
}

#[derive(Clone, Debug, Default)]
pub struct BakedModel {
    pub quads: Vec<BakedQuad>,
    pub ao: bool,
    /// Textura "particle" do modelo (`IBakedModel.getParticleTexture`), índice no atlas.
    pub particle: u16,
}

/// Variante da blockstate: uma ou mais opções com peso.
#[derive(Clone, Debug, Default)]
pub struct Weighted {
    pub models: Vec<(u32, BakedModel)>,
    pub total: u32,
}

/// `MathHelper.getPositionRandom`.
pub fn position_random(x: i32, y: i32, z: i32) -> i64 {
    let i = (x.wrapping_mul(3129871) as i64) ^ (z as i64).wrapping_mul(116129781) ^ (y as i64);
    i.wrapping_mul(i).wrapping_mul(42317861).wrapping_add(i.wrapping_mul(11))
}

impl Weighted {
    /// Escolha por posição como `WeightedBakedModel.getAlternativeModel`.
    pub fn pick(&self, x: i32, y: i32, z: i32) -> &BakedModel {
        if self.models.len() == 1 || self.total == 0 {
            return &self.models[0].1;
        }
        let seed = position_random(x, y, z);
        let mut w = ((seed as i32) >> 16).wrapping_abs() % self.total as i32;
        for (weight, m) in &self.models {
            w -= *weight as i32;
            if w < 0 {
                return m;
            }
        }
        &self.models[self.models.len() - 1].1
    }
}

#[derive(Clone, Debug)]
struct PartRot {
    origin: [f32; 3],
    axis: usize,
    angle: f32,
    rescale: bool,
}

#[derive(Clone, Debug)]
struct Face {
    uv: Option<[f32; 4]>,
    texture: String,
    cull: Option<usize>,
    rotation: i32,
    tint: i32,
}

#[derive(Clone, Debug)]
struct Element {
    from: [f32; 3],
    to: [f32; 3],
    rot: Option<PartRot>,
    shade: bool,
    faces: Vec<(usize, Face)>,
}

#[derive(Clone, Debug)]
struct RawModel {
    parent: Option<String>,
    ao: bool,
    textures: HashMap<String, String>,
    elements: Vec<Element>,
    /// `display.gui` (rotation, translation, scale), se o modelo define.
    gui: Option<[[f32; 3]; 3]>,
    /// Demais visões de `display` (`firstperson`, `thirdperson`, ...), no mesmo formato.
    display: HashMap<String, [[f32; 3]; 3]>,
}

fn vec3(v: &Value) -> Option<[f32; 3]> {
    let a = v.as_array()?;
    Some([a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32])
}

fn parse_model(v: &Value) -> RawModel {
    let parent = v.get("parent").and_then(Value::as_str).map(str::to_string);
    let ao = v.get("ambientocclusion").and_then(Value::as_bool).unwrap_or(true);
    let textures = v
        .get("textures")
        .and_then(Value::as_object)
        .map(|o| o.iter().filter_map(|(k, t)| Some((k.clone(), t.as_str()?.to_string()))).collect())
        .unwrap_or_default();
    let mut elements = Vec::new();
    for e in v.get("elements").and_then(Value::as_array).into_iter().flatten() {
        let (Some(from), Some(to)) = (e.get("from").and_then(vec3), e.get("to").and_then(vec3)) else { continue };
        let rot = e.get("rotation").and_then(|r| {
            let axis = ["x", "y", "z"].iter().position(|a| Some(*a) == r.get("axis").and_then(Value::as_str))?;
            Some(PartRot {
                origin: r.get("origin").and_then(vec3)?.map(|c| c / 16.0),
                axis,
                angle: r.get("angle").and_then(Value::as_f64).unwrap_or(0.0) as f32,
                rescale: r.get("rescale").and_then(Value::as_bool).unwrap_or(false),
            })
        });
        let shade = e.get("shade").and_then(Value::as_bool).unwrap_or(true);
        // As faces seguem a ordem do EnumFacing (EnumMap no jogo).
        let mut faces = Vec::new();
        if let Some(fs) = e.get("faces").and_then(Value::as_object) {
            for (i, name) in FACE_NAMES.iter().enumerate() {
                let Some(f) = fs.get(*name) else { continue };
                let uv = f.get("uv").and_then(Value::as_array).and_then(|a| {
                    (a.len() == 4).then(|| [0, 1, 2, 3].map(|k| a[k].as_f64().unwrap_or(0.0) as f32))
                });
                faces.push((
                    i,
                    Face {
                        uv,
                        texture: f.get("texture").and_then(Value::as_str).unwrap_or("").to_string(),
                        cull: f.get("cullface").and_then(Value::as_str).and_then(face_index),
                        rotation: f.get("rotation").and_then(Value::as_i64).unwrap_or(0) as i32,
                        tint: f.get("tintindex").and_then(Value::as_i64).unwrap_or(-1) as i32,
                    },
                ));
            }
        }
        elements.push(Element { from, to, rot, shade, faces });
    }
    // ItemTransformVec3f: ausentes -> rotação 0, translação 0, escala 1; a translação é dividida por 16 e limitada a +-1,5; escala limitada a +-4.
    let transform = |g: &Value| {
        let rot = g.get("rotation").and_then(vec3).unwrap_or([0.0; 3]);
        let tr = g.get("translation").and_then(vec3).unwrap_or([0.0; 3]).map(|c| (c / 16.0).clamp(-1.5, 1.5));
        let sc = g.get("scale").and_then(vec3).unwrap_or([1.0; 3]).map(|c| c.clamp(-4.0, 4.0));
        [rot, tr, sc]
    };
    let gui = v.get("display").and_then(|d| d.get("gui")).map(transform);
    let display = v
        .get("display")
        .and_then(Value::as_object)
        .map(|o| o.iter().map(|(k, g)| (k.clone(), transform(g))).collect())
        .unwrap_or_default();
    RawModel { parent, ao, textures, elements, gui, display }
}

/// Modelo com a herança já resolvida.
#[derive(Clone, Debug)]
pub struct ResolvedModel {
    textures: HashMap<String, String>,
    elements: Vec<Element>,
    pub ao: bool,
    /// `display.gui` herdado do pai quando o filho não define (`ModelBlock.getTransform`).
    pub gui: Option<[[f32; 3]; 3]>,
    /// `display.<visão>` herdado do pai quando o filho não define (rotation, translation/16, scale).
    pub display: HashMap<String, [[f32; 3]; 3]>,
    /// Último pai `builtin/...` da cadeia (`builtin/generated`, `builtin/entity`, ...).
    pub builtin: Option<String>,
}

impl ResolvedModel {
    /// `#ref` -> nome final da textura ("blocks/stone"), ou "missingno".
    pub fn texture(&self, name: &str) -> String {
        let mut cur = if name.starts_with('#') { name.to_string() } else { format!("#{name}") };
        for _ in 0..32 {
            match cur.strip_prefix('#') {
                Some(key) => match self.textures.get(key) {
                    Some(v) => cur = v.clone(),
                    None => return "missingno".to_string(),
                },
                None => return cur,
            }
        }
        "missingno".to_string()
    }

    pub fn element_count(&self) -> usize {
        self.elements.len()
    }
}

/// Biblioteca de modelos lidos do jar, com cache.
#[derive(Default)]
pub struct ModelLib {
    raw: HashMap<String, Option<RawModel>>,
}

impl ModelLib {
    fn raw(&mut self, name: &str, load: &mut Loader) -> Option<&RawModel> {
        if !self.raw.contains_key(name) {
            let m = load(&format!("assets/minecraft/models/{name}.json"))
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                .map(|v| parse_model(&v));
            self.raw.insert(name.to_string(), m);
        }
        self.raw.get(name)?.as_ref()
    }

    /// Resolve `block/xxx`: texturas do filho sobrepõem as do pai; elementos e
    /// oclusão vêm do modelo mais alto da cadeia (como `ModelBlock`).
    pub fn resolve(&mut self, name: &str, load: &mut Loader) -> Option<ResolvedModel> {
        let mut chain = Vec::new();
        let mut cur = name.to_string();
        for _ in 0..32 {
            let m = self.raw(&cur, load)?.clone();
            let parent = m.parent.clone();
            chain.push(m);
            match parent {
                Some(p) if !p.starts_with("builtin/") => cur = p,
                _ => break,
            }
        }
        let root = chain.last()?;
        let (ao, elements) = (root.ao, root.elements.clone());
        let gui = chain.iter().find_map(|m| m.gui);
        let mut display = HashMap::new();
        for m in chain.iter().rev() {
            display.extend(m.display.iter().map(|(k, v)| (k.clone(), *v)));
        }
        let builtin = root.parent.clone().filter(|p| p.starts_with("builtin/"));
        let mut textures = HashMap::new();
        for m in chain.iter().rev() {
            for (k, v) in &m.textures {
                textures.insert(k.clone(), v.clone());
            }
        }
        Some(ResolvedModel { textures, elements, ao, gui, display, builtin })
    }
}

/// Rotação da variante em quartos de volta (`ModelRotation`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rot {
    pub qx: u8,
    pub qy: u8,
}

/// `EnumFacing.rotateAround(X)`.
fn rot_x_face(f: usize) -> usize {
    match f {
        NORTH => DOWN,
        DOWN => SOUTH,
        SOUTH => UP,
        UP => NORTH,
        x => x,
    }
}

/// `EnumFacing.rotateAround(Y)` (horário visto de cima).
fn rot_y_face(f: usize) -> usize {
    match f {
        NORTH => EAST,
        EAST => SOUTH,
        SOUTH => WEST,
        WEST => NORTH,
        x => x,
    }
}

impl Rot {
    pub fn new(x: i64, y: i64) -> Self {
        Self { qx: (x.rem_euclid(360) / 90) as u8, qy: (y.rem_euclid(360) / 90) as u8 }
    }

    pub fn is_identity(&self) -> bool {
        self.qx == 0 && self.qy == 0
    }

    pub fn face(&self, f: usize) -> usize {
        let mut f = f;
        for _ in 0..self.qx {
            f = rot_x_face(f);
        }
        if f > UP {
            for _ in 0..self.qy {
                f = rot_y_face(f);
            }
        }
        f
    }

    /// Novo índice do vértice (`ModelRotation.rotateVertex`).
    pub fn vertex(&self, f: usize, i: usize) -> usize {
        let mut i = i;
        if f == WEST || f == EAST {
            i = (i + self.qx as usize) % 4;
        }
        let mut g = f;
        for _ in 0..self.qx {
            g = rot_x_face(g);
        }
        if g <= UP {
            i = (i + self.qy as usize) % 4;
        }
        i
    }

    /// Gira o ponto em volta do centro: -x em X e depois -y em Y.
    pub fn apply(&self, p: [f32; 3]) -> [f32; 3] {
        let [mut x, mut y, mut z] = [p[0] - 0.5, p[1] - 0.5, p[2] - 0.5];
        for _ in 0..self.qx {
            // Rx(-90): y' = z, z' = -y.
            let (ny, nz) = (z, -y);
            y = ny;
            z = nz;
        }
        for _ in 0..self.qy {
            // Ry(-90): x' = -z, z' = x.
            let (nx, nz) = (-z, x);
            x = nx;
            z = nz;
        }
        [x + 0.5, y + 0.5, z + 0.5]
    }
}

/// `BlockFaceUV`: UV do vértice `i` girada por `rotation`.
struct FaceUv {
    uvs: [f32; 4],
    rotation: i32,
}

impl FaceUv {
    fn shifted(&self, i: usize) -> usize {
        (i + (self.rotation / 90) as usize) % 4
    }
    fn u(&self, i: usize) -> f32 {
        let k = self.shifted(i);
        if k == 0 || k == 1 {
            self.uvs[0]
        } else {
            self.uvs[2]
        }
    }
    fn v(&self, i: usize) -> f32 {
        let k = self.shifted(i);
        if k == 0 || k == 3 {
            self.uvs[1]
        } else {
            self.uvs[3]
        }
    }
    fn lock_index(&self, i: usize) -> usize {
        (i + 4 - (self.rotation / 90) as usize) % 4
    }
}

/// UV padrão quando o JSON não traz `uv` (`BlockPart.getFaceUvs`).
fn default_uv(face: usize, from: [f32; 3], to: [f32; 3]) -> [f32; 4] {
    match face {
        DOWN | UP => [from[0], from[2], to[0], to[2]],
        NORTH | SOUTH => [from[0], 16.0 - to[1], to[0], 16.0 - from[1]],
        _ => [from[2], 16.0 - to[1], to[2], 16.0 - from[1]],
    }
}

fn rotate_part(p: [f32; 3], r: &PartRot) -> [f32; 3] {
    let a = r.angle.to_radians();
    let (s, c) = a.sin_cos();
    let v = [p[0] - r.origin[0], p[1] - r.origin[1], p[2] - r.origin[2]];
    let (mut out, mut scale) = match r.axis {
        0 => ([v[0], v[1] * c - v[2] * s, v[1] * s + v[2] * c], [0.0, 1.0, 1.0]),
        1 => ([v[0] * c + v[2] * s, v[1], -v[0] * s + v[2] * c], [1.0, 0.0, 1.0]),
        _ => ([v[0] * c - v[1] * s, v[0] * s + v[1] * c, v[2]], [1.0, 1.0, 0.0]),
    };
    if r.rescale {
        let k = if r.angle.abs() == 22.5 {
            1.0 / 22.5f32.to_radians().cos() - 1.0
        } else {
            1.0 / 45f32.to_radians().cos() - 1.0
        };
        scale = scale.map(|c: f32| c * k + 1.0);
    } else {
        scale = [1.0; 3];
    }
    for i in 0..3 {
        out[i] = out[i] * scale[i] + r.origin[i];
    }
    out
}

/// `FaceBakery.getFacingFromVertexData`.
fn facing_from(pos: &[[f32; 3]; 4]) -> usize {
    let sub = |a: [f32; 3], b: [f32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let a = sub(pos[0], pos[1]);
    let b = sub(pos[2], pos[1]);
    let mut n = [b[1] * a[2] - b[2] * a[1], b[2] * a[0] - b[0] * a[2], b[0] * a[1] - b[1] * a[0]];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len > 0.0 {
        n = n.map(|c| c / len);
    }
    let mut best = None;
    let mut best_dot = 0.0;
    for (f, d) in DIRS.iter().enumerate() {
        let dot = n[0] * d[0] + n[1] * d[1] + n[2] * d[2];
        if dot >= 0.0 && dot > best_dot {
            best_dot = dot;
            best = Some(f);
        }
    }
    best.unwrap_or(UP)
}

fn eps_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < 1.0e-5
}

/// Limites de um quad indexados como as faces: [minY, maxY, minZ, maxZ, minX, maxX].
pub fn quad_bounds(pos: &[[f32; 3]; 4]) -> [f32; 6] {
    let mut b: [f32; 6] = [32.0, -32.0, 32.0, -32.0, 32.0, -32.0];
    for p in pos {
        b[WEST] = b[WEST].min(p[0]);
        b[EAST] = b[EAST].max(p[0]);
        b[DOWN] = b[DOWN].min(p[1]);
        b[UP] = b[UP].max(p[1]);
        b[NORTH] = b[NORTH].min(p[2]);
        b[SOUTH] = b[SOUTH].max(p[2]);
    }
    b
}

/// Um quad de um elemento (port do `FaceBakery.makeBakedQuad`).
#[allow(clippy::too_many_arguments)]
fn bake_quad(el: &Element, face: usize, f: &Face, tex: u16, rot: Rot, uvlock: bool) -> BakedQuad {
    let from = el.from;
    let to = el.to;
    let positions = [from[1] / 16.0, to[1] / 16.0, from[2] / 16.0, to[2] / 16.0, from[0] / 16.0, to[0] / 16.0];
    let fuv = FaceUv { uvs: f.uv.unwrap_or_else(|| default_uv(face, from, to)), rotation: f.rotation };
    let mut pos = [[0.0f32; 3]; 4];
    let mut uv = [[0.0f32; 2]; 4];
    for i in 0..4 {
        let vi = VERTEX_INFO[face][i];
        let mut p = [positions[vi[0]], positions[vi[1]], positions[vi[2]]];
        if let Some(r) = &el.rot {
            p = rotate_part(p, r);
        }
        let j = if rot.is_identity() {
            i
        } else {
            p = rot.apply(p);
            rot.vertex(face, i)
        };
        pos[j] = p;
        uv[j] = [fuv.u(i), fuv.v(i)];
    }
    let facing = facing_from(&pos);
    if uvlock {
        for i in 0..4 {
            let mut c = pos[i];
            for v in c.iter_mut() {
                if *v < -0.1 || *v >= 1.1 {
                    *v -= v.floor();
                }
            }
            let [x, y, z] = c;
            let (u, v) = match facing {
                DOWN => (x * 16.0, (1.0 - z) * 16.0),
                UP => (x * 16.0, z * 16.0),
                NORTH => ((1.0 - x) * 16.0, (1.0 - y) * 16.0),
                SOUTH => (x * 16.0, (1.0 - y) * 16.0),
                WEST => (z * 16.0, (1.0 - y) * 16.0),
                _ => ((1.0 - z) * 16.0, (1.0 - y) * 16.0),
            };
            uv[fuv.lock_index(i)] = [u, v];
        }
    }
    if el.rot.is_none() {
        // `applyFacing`: reordena os cantos na ordem padrão da face e leva a UV junto.
        let b = quad_bounds(&pos);
        let (old_pos, old_uv) = (pos, uv);
        for i in 0..4 {
            let vi = VERTEX_INFO[facing][i];
            let p = [b[vi[0]], b[vi[1]], b[vi[2]]];
            pos[i] = p;
            for k in 0..4 {
                let o = old_pos[k];
                if eps_eq(p[0], o[0]) && eps_eq(p[1], o[1]) && eps_eq(p[2], o[2]) {
                    uv[i] = old_uv[k];
                }
            }
        }
    }
    let shade = if el.shade { FACE_SHADE[rot.face(face)] } else { 1.0 };
    BakedQuad {
        pos,
        uv,
        tex,
        tint: f.tint,
        face: facing as u8,
        cull: f.cull.map(|c| rot.face(c) as u8),
        shade,
    }
}

/// Assa um modelo resolvido com a rotação da variante.
pub fn bake(m: &ResolvedModel, rot: Rot, uvlock: bool, tex: &mut dyn FnMut(&str) -> u16) -> BakedModel {
    let mut quads = Vec::new();
    for el in &m.elements {
        for (face, f) in &el.faces {
            let t = tex(&m.texture(&f.texture));
            quads.push(bake_quad(el, *face, f, t, rot, uvlock));
        }
    }
    let particle = tex(&m.texture("particle"));
    BakedModel { quads, ao: m.ao, particle }
}

/// Lê `blockstates/<name>.json` e assa todas as variantes.
pub fn load_blockstate(
    name: &str,
    lib: &mut ModelLib,
    load: &mut Loader,
    tex: &mut dyn FnMut(&str) -> u16,
) -> Option<HashMap<String, Weighted>> {
    let bytes = load(&format!("assets/minecraft/blockstates/{name}.json"))?;
    let v: Value = serde_json::from_slice(&bytes).ok()?;
    let variants = v.get("variants")?.as_object()?;
    let mut out = HashMap::new();
    for (key, val) in variants {
        let list: Vec<&Value> = match val {
            Value::Array(a) => a.iter().collect(),
            other => vec![other],
        };
        let mut w = Weighted::default();
        for item in list {
            let Some(model) = item.get("model").and_then(Value::as_str) else { continue };
            let Some(resolved) = lib.resolve(&format!("block/{model}"), load) else { continue };
            let rot = Rot::new(
                item.get("x").and_then(Value::as_i64).unwrap_or(0),
                item.get("y").and_then(Value::as_i64).unwrap_or(0),
            );
            let uvlock = item.get("uvlock").and_then(Value::as_bool).unwrap_or(false);
            let weight = item.get("weight").and_then(Value::as_u64).unwrap_or(1) as u32;
            w.models.push((weight, bake(&resolved, rot, uvlock, tex)));
        }
        if w.models.is_empty() {
            continue;
        }
        // Peso maior primeiro e, no empate, menos quads (ordem do WeightedBakedModel).
        w.models.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.quads.len().cmp(&b.1.quads.len())));
        w.total = w.models.iter().map(|m| m.0).sum();
        out.insert(key.clone(), w);
    }
    Some(out)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Carregador a partir de arquivos em memória.
    pub fn mem_loader(files: &[(&str, &str)]) -> impl FnMut(&str) -> Option<Vec<u8>> {
        let map: HashMap<String, Vec<u8>> = files.iter().map(|(k, v)| (k.to_string(), v.as_bytes().to_vec())).collect();
        move |p: &str| map.get(p).cloned()
    }

    fn cube_files() -> Vec<(&'static str, &'static str)> {
        vec![
            (
                "assets/minecraft/models/block/cube.json",
                r##"{"elements":[{"from":[0,0,0],"to":[16,16,16],"faces":{
                    "down":{"texture":"#down","cullface":"down"},"up":{"texture":"#up","cullface":"up"},
                    "north":{"texture":"#north","cullface":"north"},"south":{"texture":"#south","cullface":"south"},
                    "west":{"texture":"#west","cullface":"west"},"east":{"texture":"#east","cullface":"east"}}}]}"##,
            ),
            (
                "assets/minecraft/models/block/cube_all.json",
                r##"{"parent":"block/cube","textures":{"particle":"#all","down":"#all","up":"#all","north":"#all","east":"#all","south":"#all","west":"#all"}}"##,
            ),
            ("assets/minecraft/models/block/stone.json", r##"{"parent":"block/cube_all","textures":{"all":"blocks/stone"}}"##),
        ]
    }

    #[test]
    fn inheritance_resolves_textures() {
        let files = cube_files();
        let mut load = mem_loader(&files);
        let mut lib = ModelLib::default();
        let m = lib.resolve("block/stone", &mut load).unwrap();
        assert_eq!(m.texture("#north"), "blocks/stone");
        assert_eq!(m.texture("particle"), "blocks/stone");
        assert_eq!(m.texture("#nada"), "missingno");
        assert!(m.ao);
        assert_eq!(m.element_count(), 1);
        let mut names = Vec::new();
        let b = bake(&m, Rot::default(), false, &mut |n| {
            names.push(n.to_string());
            0
        });
        assert_eq!(b.quads.len(), 6);
        assert!(names.iter().all(|n| n == "blocks/stone"));
        // A face de cima fica no topo, com a ordem de cantos do jogo.
        let up = &b.quads[1];
        assert_eq!(up.face as usize, UP);
        assert_eq!(up.cull, Some(UP as u8));
        assert_eq!(up.pos, [[0., 1., 0.], [0., 1., 1.], [1., 1., 1.], [1., 1., 0.]]);
        assert_eq!(up.uv, [[0., 0.], [0., 16.], [16., 16.], [16., 0.]]);
    }

    #[test]
    fn variant_rotation_turns_faces() {
        let r = Rot::new(0, 90);
        assert_eq!(r.face(NORTH), EAST);
        assert_eq!(r.face(UP), UP);
        let r = Rot::new(90, 0);
        assert_eq!(r.face(NORTH), DOWN);
        assert_eq!(r.face(UP), NORTH);
        let p = Rot::new(0, 90).apply([0.5, 0.5, 0.0]);
        assert!((p[0] - 1.0).abs() < 1e-6 && (p[2] - 0.5).abs() < 1e-6, "{p:?}");
    }

    #[test]
    fn element_rotation_and_rescale() {
        // Cruz de planta: 45 graus em Y com rescale estica até os cantos do bloco.
        let r = PartRot { origin: [0.5, 0.5, 0.5], axis: 1, angle: 45.0, rescale: true };
        let p = rotate_part([0.8, 0.0, 0.5], &r);
        let d = ((p[0] - 0.5).powi(2) + (p[2] - 0.5).powi(2)).sqrt();
        assert!((d - 0.3 * 2f32.sqrt()).abs() < 1e-4, "{p:?}");
        let r = PartRot { rescale: false, ..r };
        let p = rotate_part([0.8, 0.0, 0.5], &r);
        let d = ((p[0] - 0.5).powi(2) + (p[2] - 0.5).powi(2)).sqrt();
        assert!((d - 0.3).abs() < 1e-5);
    }

    #[test]
    fn uv_rotation_and_uvlock() {
        let fuv = FaceUv { uvs: [0.0, 0.0, 16.0, 8.0], rotation: 90 };
        // Rotação 90: o vértice 0 pega o canto que antes era do vértice 1.
        assert_eq!((fuv.u(0), fuv.v(0)), (0.0, 8.0));
        assert_eq!((fuv.u(3), fuv.v(3)), (0.0, 0.0));
        assert_eq!(fuv.lock_index(0), 3);

        // Laje de baixo girada 90° com uvlock: a UV de cima continua presa ao mundo.
        let el = Element {
            from: [0.0, 0.0, 0.0],
            to: [16.0, 8.0, 8.0],
            rot: None,
            shade: true,
            faces: vec![],
        };
        let f = Face { uv: None, texture: String::new(), cull: None, rotation: 0, tint: -1 };
        let q = bake_quad(&el, UP, &f, 0, Rot::new(0, 90), true);
        for (p, uv) in q.pos.iter().zip(q.uv.iter()) {
            assert!((uv[0] - p[0] * 16.0).abs() < 1e-4 && (uv[1] - p[2] * 16.0).abs() < 1e-4, "{:?} {:?}", q.pos, q.uv);
        }
        let q = bake_quad(&el, UP, &f, 0, Rot::new(0, 90), false);
        assert_eq!(q.face as usize, UP);
        // Sem uvlock a textura gira junto: o canto (1,·,0.5)->... não segue x*16.
        assert!(q.pos.iter().zip(q.uv.iter()).any(|(p, uv)| (uv[0] - p[0] * 16.0).abs() > 1e-3));
    }

    #[test]
    fn weighted_pick_is_stable() {
        let m = |n: usize| BakedModel { quads: vec![BakedQuad { pos: [[0.0; 3]; 4], uv: [[0.0; 2]; 4], tex: n as u16, tint: -1, face: 0, cull: None, shade: 1.0 }], ao: true, particle: 0 };
        let w = Weighted { models: vec![(1, m(0)), (1, m(1)), (1, m(2)), (1, m(3))], total: 4 };
        let mut seen = [false; 4];
        for x in 0..32 {
            for z in 0..32 {
                seen[w.pick(x, 64, z).quads[0].tex as usize] = true;
            }
        }
        assert!(seen.iter().all(|s| *s));
        assert_eq!(w.pick(10, 70, -3).quads[0].tex, w.pick(10, 70, -3).quads[0].tex);
    }
}
