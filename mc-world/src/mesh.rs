//! Malha de uma seção 16x16x16, gerada a partir de uma cópia com borda.
//!
//! A luz vai por vértice (céu e bloco separados) para o shader aplicar o ciclo
//! de dia e noite; as faces inteiras usam luz suave com oclusão de ambiente.

use bytemuck::{Pod, Zeroable};

use crate::blocks::{Aabb, BlockInfo, BlockTable, Layer, Model, UP};
use crate::model::{position_random, quad_bounds, BakedModel, BakedQuad, VERTEX_INFO};
use crate::biome;
use crate::states;
use crate::visgraph::{is_opaque, SetVisibility, VisGraph};

/// Atlas quadrado com 32x32 texturas de 16 px.
pub const ATLAS_COLS: u32 = 32;

const SHADE: [f32; 6] = [0.5, 1.0, 0.8, 0.8, 0.6, 0.6];
const DIRS: [[i32; 3]; 6] = [[0, -1, 0], [0, 1, 0], [0, 0, -1], [0, 0, 1], [-1, 0, 0], [1, 0, 0]];
/// Eixos tangentes de cada face (para a luz dos cantos).
const TANGENTS: [[usize; 2]; 6] = [[0, 2], [0, 2], [0, 1], [0, 1], [2, 1], [2, 1]];
/// Brilho por nível de oclusão (0 = canto fechado).
const AO: [u8; 4] = [130, 172, 214, 255];
/// Blocos de borda copiados em volta da seção (a luz suave olha até 3 de distância).
pub const BORDER: i32 = 3;
const PAD: i32 = 16 + 2 * BORDER;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    /// Tinta x sombreamento da face (sRGB).
    pub color: [u8; 4],
    /// Céu (0..255 = 0..15), bloco, oclusão, livre.
    pub light: [u8; 4],
}

#[derive(Default)]
pub struct Geometry {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl Geometry {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// Quadrilátero com a diagonal 0-2, como o `GL_QUADS` do jogo.
    pub fn quad_fixed(&mut self, v: [Vertex; 4]) {
        let base = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&v);
        self.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// Quadrilátero; a diagonal segue a oclusão para não criar listras.
    pub fn quad(&mut self, v: [Vertex; 4]) {
        let base = self.vertices.len() as u32;
        let flip = (v[0].light[2] as u32 + v[2].light[2] as u32) < (v[1].light[2] as u32 + v[3].light[2] as u32);
        self.vertices.extend_from_slice(&v);
        if flip {
            self.indices.extend_from_slice(&[base + 1, base + 2, base + 3, base + 1, base + 3, base]);
        } else {
            self.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
}

#[derive(Default)]
pub struct MeshData {
    /// SOLID: sem teste de alfa, desenhado de frente para trás.
    pub opaque: Geometry,
    /// CUTOUT_MIPPED: `alphaFunc(GREATER, 0.5)` com mipmap.
    pub cutout_mipped: Geometry,
    /// CUTOUT: `alphaFunc(GREATER, 0.5)` sem mipmap.
    pub cutout: Geometry,
    pub translucent: Geometry,
    /// Pares de faces ligados pelo ar dentro da seção (`culling` do cliente).
    pub visibility: SetVisibility,
}

/// Seção com `BORDER` blocos de borda em cada direção.
pub struct Padded {
    pub blocks: Vec<u16>,
    /// Céu << 4 | bloco.
    pub light: Vec<u8>,
    /// Biomas por coluna (255 = desconhecido); índice por `bidx`.
    pub biomes: Vec<u8>,
}

impl Padded {
    pub fn new(default_light: u8) -> Self {
        let n = (PAD * PAD * PAD) as usize;
        Self { blocks: vec![0; n], light: vec![default_light; n], biomes: vec![255; (PAD * PAD) as usize] }
    }

    #[inline]
    pub fn bidx(x: i32, z: i32) -> usize {
        ((z + BORDER) * PAD + (x + BORDER)) as usize
    }

    #[inline]
    pub fn biome(&self, x: i32, z: i32) -> u8 {
        self.biomes[Self::bidx(x, z)]
    }

    /// Coordenadas locais de -BORDER a 15 + BORDER.
    #[inline]
    pub fn idx(x: i32, y: i32, z: i32) -> usize {
        debug_assert!((-BORDER..16 + BORDER).contains(&x) && (-BORDER..16 + BORDER).contains(&y) && (-BORDER..16 + BORDER).contains(&z));
        (((y + BORDER) * PAD + (z + BORDER)) * PAD + (x + BORDER)) as usize
    }

    #[inline]
    pub fn block(&self, x: i32, y: i32, z: i32) -> u16 {
        self.blocks[Self::idx(x, y, z)]
    }

    #[inline]
    pub fn light(&self, x: i32, y: i32, z: i32) -> u8 {
        self.light[Self::idx(x, y, z)]
    }
}

/// Luz empacotada -> canal do vértice, sem oclusão.
pub(crate) fn flat(l: u8) -> [u8; 4] {
    [(l >> 4) * 17, (l & 15) * 17, 255, 0]
}

pub(crate) fn max_light(a: u8, b: u8) -> u8 {
    ((a >> 4).max(b >> 4) << 4) | (a & 15).max(b & 15)
}

pub fn tint_rgba(tint: u32, k: f32) -> [u8; 4] {
    let c = |shift: u32| (((tint >> shift) & 0xFF) as f32 * k).round().clamp(0.0, 255.0) as u8;
    [c(16), c(8), c(0), 255]
}

/// UV no atlas de blocos (u e v em pixels da textura, 0..16).
pub fn atlas_uv(tex: u16, u: f32, v: f32) -> [f32; 2] {
    let cols = ATLAS_COLS as f32;
    let c = (tex as u32 % ATLAS_COLS) as f32;
    let r = (tex as u32 / ATLAS_COLS) as f32;
    let u = u.clamp(0.01, 15.99);
    let v = v.clamp(0.01, 15.99);
    [(c + u / 16.0) / cols, (r + v / 16.0) / cols]
}

/// Uma face de uma caixa (coordenadas em blocos, relativas a `base`).
/// `light` recebe o canto (0..1 em cada eixo) e devolve a luz do vértice.
#[allow(clippy::too_many_arguments)]
pub fn face(geo: &mut Geometry, f: usize, lo: [f32; 3], hi: [f32; 3], base: [f32; 3], tex: u16, rgba: [u8; 4], light: &dyn Fn([f32; 3]) -> [u8; 4]) {
    let [x0, y0, z0] = lo;
    let [x1, y1, z1] = hi;
    let corners = match f {
        0 => [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]],
        1 => [[x0, y1, z0], [x1, y1, z0], [x1, y1, z1], [x0, y1, z1]],
        2 => [[x0, y0, z0], [x1, y0, z0], [x1, y1, z0], [x0, y1, z0]],
        3 => [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]],
        4 => [[x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]],
        _ => [[x1, y0, z0], [x1, y0, z1], [x1, y1, z1], [x1, y1, z0]],
    };
    geo.quad(corners.map(|c @ [x, y, z]| {
        let (u, v) = match f {
            0 | 1 => (x * 16.0, z * 16.0),
            2 | 3 => (x * 16.0, 16.0 - y * 16.0),
            _ => (z * 16.0, 16.0 - y * 16.0),
        };
        Vertex { pos: [base[0] + x, base[1] + y, base[2] + z], uv: atlas_uv(tex, u, v), color: rgba, light: light(c) }
    }));
}

fn cross(geo: &mut Geometry, base: [f32; 3], tex: u16, rgba: [u8; 4], light: [u8; 4]) {
    // Cada plano dos dois lados, como o block/cross.json (o terreno descarta as faces de costas).
    for (a, b) in [([0.0, 0.0], [1.0, 1.0]), ([0.0, 1.0], [1.0, 0.0]), ([1.0, 1.0], [0.0, 0.0]), ([1.0, 0.0], [0.0, 1.0])] {
        let corners = [(a, 0.0, 0.0), (b, 0.0, 16.0), (b, 1.0, 16.0), (a, 1.0, 0.0)];
        geo.quad(corners.map(|(p, y, u): ([f32; 2], f32, f32)| Vertex {
            pos: [base[0] + p[0], base[1] + y, base[2] + p[1]],
            uv: atlas_uv(tex, u, 16.0 - y * 16.0),
            color: rgba,
            light,
        }));
    }
}

fn tint_for(info: &BlockInfo, f: usize) -> u32 {
    if info.tint_top_only && f != UP {
        0xFFFFFF
    } else {
        info.tint
    }
}

/// Luz suave de um canto de face inteira: média das 4 células na frente do canto
/// e oclusão pelos vizinhos opacos.
fn smooth_corner(p: &Padded, t: &BlockTable, cell: [i32; 3], f: usize, c: [f32; 3]) -> [u8; 4] {
    let d = DIRS[f];
    let n = [cell[0] + d[0], cell[1] + d[1], cell[2] + d[2]];
    let [a1, a2] = TANGENTS[f];
    let mut e1 = [0; 3];
    let mut e2 = [0; 3];
    e1[a1] = if c[a1] > 0.5 { 1 } else { -1 };
    e2[a2] = if c[a2] > 0.5 { 1 } else { -1 };
    let at = |o: [i32; 3]| [n[0] + o[0], n[1] + o[1], n[2] + o[2]];
    let s1 = at(e1);
    let s2 = at(e2);
    let cc = at([e1[0] + e2[0], e1[1] + e2[1], e1[2] + e2[2]]);
    let opaque = |q: [i32; 3]| t.get(p.block(q[0], q[1], q[2])).full_opaque;
    let (o1, o2, oc) = (opaque(s1), opaque(s2), opaque(cc));
    let level = if o1 && o2 { 0 } else { 3 - (o1 as usize + o2 as usize + oc as usize) };

    let mut sky = 0u32;
    let mut blk = 0u32;
    let mut count = 0u32;
    for (q, skip) in [(n, false), (s1, o1), (s2, o2), (cc, oc || (o1 && o2))] {
        if skip {
            continue;
        }
        let l = p.light(q[0], q[1], q[2]);
        sky += (l >> 4) as u32;
        blk += (l & 15) as u32;
        count += 1;
    }
    [(sky * 17 / count) as u8, (blk * 17 / count) as u8, AO[level], 0]
}

/// Luz do jogo empacotada (`getCombinedLight`): céu << 20 | bloco << 4.
type Bright = u32;

/// `EnumNeighborInfo`: as quatro direções em volta de cada face.
const NEIGHBORS: [[usize; 4]; 6] = [
    [4, 5, 2, 3],
    [5, 4, 2, 3],
    [1, 0, 5, 4],
    [4, 5, 0, 1],
    [1, 0, 2, 3],
    [0, 1, 2, 3],
];

struct Ctx<'a> {
    p: &'a Padded,
    t: &'a BlockTable,
}

fn add(a: [i32; 3], f: usize) -> [i32; 3] {
    let d = DIRS[f];
    [a[0] + d[0], a[1] + d[1], a[2] + d[2]]
}

/// Deslocamento aleatório das plantas (`Block.EnumOffsetType`).
fn plant_offset(id: u16, wpos: [i32; 3], ao: bool) -> [f32; 3] {
    let xyz = id == 31;
    if !(xyz || id == 37 || id == 38 || id == 175) {
        return [0.0; 3];
    }
    let [x, y, z] = wpos;
    let k = if ao {
        position_random(x, y, z)
    } else {
        let k = (x.wrapping_mul(3129871) as i64) ^ (z as i64).wrapping_mul(116129781);
        k.wrapping_mul(k).wrapping_mul(42317861).wrapping_add(k.wrapping_mul(11))
    };
    let part = |shift: u32| ((k >> shift) & 15) as f32 / 15.0;
    let dy = if xyz { (part(20) - 1.0) * 0.2 } else { 0.0 };
    [(part(16) - 0.5) * 0.5, dy, (part(24) - 0.5) * 0.5]
}

impl Ctx<'_> {
    #[inline]
    fn block(&self, q: [i32; 3]) -> u16 {
        self.p.block(q[0], q[1], q[2])
    }

    /// `ChunkCache.getCombinedLight`, com a luz dos vizinhos para escadas, lajes etc.
    fn combined(&self, q: [i32; 3]) -> Bright {
        let st = self.block(q);
        let (mut sky, mut blk) = if self.t.neighbor_light(st) {
            let mut s = 0;
            let mut b = 0;
            for f in 0..6 {
                let n = add(q, f);
                let l = self.p.light(n[0], n[1], n[2]);
                s = s.max(l >> 4);
                b = b.max(l & 15);
            }
            (s, b)
        } else {
            let l = self.p.light(q[0], q[1], q[2]);
            (l >> 4, l & 15)
        };
        blk = blk.max(self.t.emission(st));
        sky = sky.min(15);
        (sky as u32) << 20 | (blk as u32) << 4
    }

    /// `Block.getMixedBrightnessForBlock`.
    fn mixed(&self, q: [i32; 3]) -> Bright {
        let c = self.combined(q);
        if c == 0 && states::is_slab(self.block(q) >> 4) {
            return self.combined(add(q, 0));
        }
        c
    }

    fn ao_value(&self, q: [i32; 3]) -> f32 {
        if self.t.normal_cube(self.block(q)) {
            0.2
        } else {
            1.0
        }
    }

    /// `shouldSideBeRendered` do bloco `st` para a face `f` (vizinho `n`).
    fn side_visible(&self, st: u16, n: u16, f: usize) -> bool {
        let (id, nid) = (st >> 4, n >> 4);
        match id {
            // Vidro: só esconde contra o mesmo estado; qualquer outro vizinho desenha.
            20 | 95 => return n != st,
            79 | 165 | 90 => {
                if nid == id {
                    return false;
                }
            }
            101 | 102 | 160 => {
                if nid == id {
                    return false;
                }
            }
            _ if states::is_fence(id) || states::is_gate(id) => return true,
            139 => {
                if f != 0 {
                    return true;
                }
            }
            78 | 171 => {
                if f == UP {
                    return true;
                }
            }
            _ => {}
        }
        // isOpaqueCube do vizinho; as folhas mudam com os gráficos (BlockTable::set_fancy_leaves).
        !self.t.get(n).full_opaque
    }

    /// `Block.colorMultiplier(world, pos)`: cor por bioma quando as colormaps do jar estão carregadas.
    fn tint_at(&self, st: u16, pos: [i32; 3], wpos: [i32; 3]) -> u32 {
        let Some(maps) = &self.t.colormaps else { return self.t.tint_color(st) };
        let (id, m) = (st >> 4, st & 15);
        let origin = [wpos[0] - pos[0], wpos[2] - pos[2]];
        // Coordenadas do mundo -> locais da seção para achar o bioma.
        let biome_at = |x: i32, z: i32| self.p.biome(x - origin[0], z - origin[1]);
        let y = wpos[1];
        let single = |f: &dyn Fn(&biome::Biome, i32, i32, i32) -> u32| f(&biome::biome(biome_at(wpos[0], wpos[2])), wpos[0], y, wpos[2]);
        let grass3 = || biome::average_3x3(&biome_at, wpos[0], wpos[2], &|b, x, z| maps.grass_at(b, x, y, z));
        let foliage3 = || biome::average_3x3(&biome_at, wpos[0], wpos[2], &|b, x, z| maps.foliage_at(b, x, y, z));
        match id {
            2 | 175 => grass3(),
            31 | 83 => single(&|b, x, y, z| maps.grass_at(b, x, y, z)),
            18 => match m & 3 {
                1 => 0x619961,
                2 => 0x80A755,
                _ => foliage3(),
            },
            161 => foliage3(),
            106 => single(&|b, x, y, z| maps.foliage_at(b, x, y, z)),
            _ => self.t.tint_color(st),
        }
    }

    fn render_model(&self, geo: &mut Geometry, st: u16, pos: [i32; 3], wpos: [i32; 3], model: &BakedModel) {
        let id = st >> 4;
        let ao = model.ao && self.t.emission(st) == 0;
        let off = plant_offset(id, wpos, ao);
        let base = [wpos[0] as f32 + off[0], wpos[1] as f32 + off[1], wpos[2] as f32 + off[2]];
        let tint = self.tint_at(st, pos, wpos);
        let full = self.t.full_cube(st);
        for q in &model.quads {
            if let Some(c) = q.cull {
                let n = add(pos, c as usize);
                if !self.side_visible(st, self.block(n), c as usize) {
                    continue;
                }
            }
            let (bright, mult) = if ao {
                self.ao_quad(pos, q, full)
            } else {
                let b = match q.cull {
                    Some(c) => self.mixed(add(pos, c as usize)),
                    None => {
                        let (_, flush) = quad_flags(q, full);
                        self.mixed(if flush { add(pos, q.face as usize) } else { pos })
                    }
                };
                ([b; 4], [1.0; 4])
            };
            let rgb = if q.tint >= 0 { tint } else { 0xFFFFFF };
            let mut v = [Vertex { pos: [0.0; 3], uv: [0.0; 2], color: [0; 4], light: [0; 4] }; 4];
            for i in 0..4 {
                let p = q.pos[i];
                let to8 = |b: u32| ((b & 0xFF) * 255 / 240).min(255) as u8;
                v[i] = Vertex {
                    pos: [base[0] + p[0], base[1] + p[1], base[2] + p[2]],
                    uv: atlas_uv(q.tex, q.uv[i][0], q.uv[i][1]),
                    color: tint_rgba(rgb, q.shade),
                    light: [to8(bright[i] >> 16), to8(bright[i]), (mult[i] * 255.0).round().clamp(0.0, 255.0) as u8, 0],
                };
            }
            geo.quad_fixed(v);
        }
    }

    /// `AmbientOcclusionFace.updateVertexBrightness`: luz e sombra de cada canto do quad.
    fn ao_quad(&self, pos: [i32; 3], q: &BakedQuad, full: bool) -> ([Bright; 4], [f32; 4]) {
        let face = q.face as usize;
        let (partial, flush) = quad_flags(q, full);
        let base = if flush { add(pos, face) } else { pos };
        let nd = NEIGHBORS[face];
        let b1 = add(base, nd[0]);
        let b2 = add(base, nd[1]);
        let b3 = add(base, nd[2]);
        let b4 = add(base, nd[3]);
        let (i, j, k, l) = (self.mixed(b1), self.mixed(b2), self.mixed(b3), self.mixed(b4));
        let (f, f1, f2, f3) = (self.ao_value(b1), self.ao_value(b2), self.ao_value(b3), self.ao_value(b4));
        let tr = |b: [i32; 3]| self.t.translucent(self.block(add(b, face)));
        let (t1, t2, t3, t4) = (tr(b1), tr(b2), tr(b3), tr(b4));
        // Canto diagonal: só olha quando algum dos lados deixa a luz passar.
        let corner = |side: [i32; 3], d: usize| -> (f32, Bright) {
            let q = add(side, d);
            (self.ao_value(q), self.mixed(q))
        };
        let (f4, i1) = if !t3 && !t1 { (f, i) } else { corner(b1, nd[2]) };
        let (f5, j1) = if !t4 && !t1 { (f, i) } else { corner(b1, nd[3]) };
        let (f6, k1) = if !t3 && !t2 { (f1, j) } else { corner(b2, nd[2]) };
        let (f7, l1) = if !t4 && !t2 { (f1, j) } else { corner(b2, nd[3]) };
        let i3 = if flush || !self.t.get(self.block(add(pos, face))).full_opaque { self.mixed(add(pos, face)) } else { self.mixed(pos) };
        let f8 = if flush { self.ao_value(base) } else { self.ao_value(pos) };
        // Cantos: (d3,d0), (d2,d0), (d2,d1), (d3,d1).
        let ao = [(f3 + f + f5 + f8) * 0.25, (f2 + f + f4 + f8) * 0.25, (f2 + f1 + f6 + f8) * 0.25, (f3 + f1 + f7 + f8) * 0.25];
        let br = [ao_bright(l, i, j1, i3), ao_bright(k, i, i1, i3), ao_bright(k, j, k1, i3), ao_bright(l, j, l1, i3)];
        // Cada vértice usa a posição padrão do seu índice dentro dos limites do quad
        // e mistura os quatro cantos (o jogo faz assim com `Orientation`).
        let b = quad_bounds(&q.pos);
        let mut out_b = [0; 4];
        let mut out_a = [0.0; 4];
        for s in 0..4 {
            let vi = VERTEX_INFO[face][s];
            let mut cp = [0.0f32; 3];
            for (axis, &bi) in vi.iter().enumerate() {
                cp[axis] = b[bi];
            }
            if !partial {
                cp = cp.map(|c| c.round());
            }
            let w = |d: usize| -> f32 {
                let v = match d {
                    0 | 1 => cp[1],
                    2 | 3 => cp[2],
                    _ => cp[0],
                };
                if d % 2 == 1 { v } else { 1.0 - v }
            };
            let wt = [w(nd[0]) * w(nd[3]), w(nd[0]) * w(nd[2]), w(nd[1]) * w(nd[2]), w(nd[1]) * w(nd[3])];
            out_a[s] = (0..4).map(|c| ao[c] * wt[c]).sum();
            if partial {
                let mix = |shift: u32| -> u32 {
                    ((0..4).map(|c| ((br[c] >> shift) & 0xFF) as f32 * wt[c]).sum::<f32>() as u32) & 0xFF
                };
                out_b[s] = mix(16) << 16 | mix(0);
            } else {
                out_b[s] = (0..4).map(|c| if wt[c] > 0.5 { br[c] } else { 0 }).sum();
            }
        }
        (out_b, out_a)
    }
}

/// `getAoBrightness`: média dos quatro, trocando os zeros pelo centro.
fn ao_bright(a: Bright, b: Bright, c: Bright, center: Bright) -> Bright {
    let a = if a == 0 { center } else { a };
    let b = if b == 0 { center } else { b };
    let c = if c == 0 { center } else { c };
    ((a + b + c + center) >> 2) & 0x00FF_00FF
}

/// `fillQuadBounds`: (quad menor que a face, quad encostado na face do bloco).
fn quad_flags(q: &BakedQuad, full: bool) -> (bool, bool) {
    let b = quad_bounds(&q.pos);
    let (min_y, max_y, min_z, max_z, min_x, max_x) = (b[0], b[1], b[2], b[3], b[4], b[5]);
    const LO: f32 = 1.0e-4;
    const HI: f32 = 0.9999;
    match q.face {
        0 | 1 => {
            let partial = min_x >= LO || min_z >= LO || max_x <= HI || max_z <= HI;
            let edge = if q.face == 0 { min_y < LO } else { max_y > HI };
            (partial, (edge || full) && min_y == max_y)
        }
        2 | 3 => {
            let partial = min_x >= LO || min_y >= LO || max_x <= HI || max_y <= HI;
            let edge = if q.face == 2 { min_z < LO } else { max_z > HI };
            (partial, (edge || full) && min_z == max_z)
        }
        _ => {
            let partial = min_y >= LO || min_z >= LO || max_y <= HI || max_z <= HI;
            let edge = if q.face == 4 { min_x < LO } else { max_x > HI };
            (partial, (edge || full) && min_x == max_x)
        }
    }
}

/// Gera a malha da seção cuja origem (canto mínimo, em blocos) é `origin`.
pub fn mesh_section(p: &Padded, origin: [i32; 3], t: &BlockTable) -> MeshData {
    let mut m = MeshData::default();
    let mut boxes: Vec<Aabb> = Vec::new();
    for y in 0..16 {
        for z in 0..16 {
            for x in 0..16 {
                mesh_cell(p, origin, t, x, y, z, &mut m, &mut boxes);
            }
        }
    }
    m.visibility = section_visibility(p, t);
    m
}

/// Só o bloco da célula local (x, y, z), com os vizinhos do `Padded` para a oclusão, a luz suave e o corte de
/// faces (`BlockRendererDispatcher.renderBlock` de um bloco, como o `RenderFallingBlock` faz). Sem visibilidade.
pub fn mesh_one(p: &Padded, origin: [i32; 3], t: &BlockTable, cell: [i32; 3]) -> MeshData {
    let mut m = MeshData::default();
    mesh_cell(p, origin, t, cell[0], cell[1], cell[2], &mut m, &mut Vec::new());
    m
}

#[allow(clippy::too_many_arguments)]
fn mesh_cell(p: &Padded, origin: [i32; 3], t: &BlockTable, x: i32, y: i32, z: i32, m: &mut MeshData, boxes: &mut Vec<Aabb>) {
    let st = p.block(x, y, z);
    if st == 0 {
        return;
    }
    let info = t.get(st);
    let geo = match info.layer {
        Layer::Opaque => &mut m.opaque,
        Layer::CutoutMipped => &mut m.cutout_mipped,
        Layer::Cutout => &mut m.cutout,
        Layer::Translucent => &mut m.translucent,
    };
    if let Some(models) = &t.models {
        // Meta que não existe no jogo vira ar (BLOCK_STATE_IDS não tem o estado).
        if !t.valid(st) {
            return;
        }
        if models.has(st) {
            let wpos = [origin[0] + x, origin[1] + y, origin[2] + z];
            let nb = |dx: i32, dy: i32, dz: i32| p.block(x + dx, y + dy, z + dz);
            let bits = if states::dyn_count(st >> 4) > 1 { states::actual_bits(t, st, wpos, &nb) } else { 0 };
            if let Some(w) = models.get(st, bits) {
                let model = w.pick(wpos[0], wpos[1], wpos[2]);
                let ctx = Ctx { p, t };
                ctx.render_model(geo, st, [x, y, z], wpos, model);
                return;
            }
        }
    }
    let base = [(origin[0] + x) as f32, (origin[1] + y) as f32, (origin[2] + z) as f32];
    let own = max_light(p.light(x, y, z), p.light(x, y + 1, z));
    match &info.model {
        Model::Air | Model::Invisible | Model::Unsupported => {}
        Model::Cross => cross(geo, base, info.tex[0], tint_rgba(info.tint, 1.0), flat(own)),
        Model::DoublePlant => {
            let upper = st & 8 != 0;
            let kind = if upper { p.block(x, y - 1, z) & 7 } else { st & 7 } as usize;
            let tex = t.double_plant[kind.min(5)][upper as usize];
            let tint = if kind == 2 || kind == 3 { 0x91BD59 } else { 0xFFFFFF };
            cross(geo, base, tex, tint_rgba(tint, 1.0), flat(own));
        }
        Model::Liquid { lava } => {
            crate::fluid::render_fluid(geo, p, t, [x, y, z], [origin[0] + x, origin[1] + y, origin[2] + z], *lava);
        }
        _ => {
            boxes.clear();
            t.boxes(st, &|dx, dy, dz| p.block(x + dx, y + dy, z + dz), boxes);
            let tex = match &info.model {
                Model::Door { upper } if st & 8 != 0 => [*upper; 6],
                _ => info.tex,
            };
            // Luz suave só para blocos opacos e inteiros (como o jogo faz).
            let smooth = info.full_opaque || (info.layer != Layer::Translucent && boxes.len() == 1 && boxes[0] == Aabb::FULL);
            for b in boxes.iter() {
                let lo = b.min.map(|v| v as f32 / 16.0);
                let hi = b.max.map(|v| v.min(16) as f32 / 16.0);
                for (f, d) in DIRS.iter().enumerate() {
                    let on_edge = match f {
                        0 => b.min[1] == 0,
                        1 => b.max[1] >= 16,
                        2 => b.min[2] == 0,
                        3 => b.max[2] >= 16,
                        4 => b.min[0] == 0,
                        _ => b.max[0] >= 16,
                    };
                    let (nx, ny, nz) = (x + d[0], y + d[1], z + d[2]);
                    if on_edge {
                        let n = p.block(nx, ny, nz);
                        if t.get(n).full_opaque || (info.cull_same && n >> 4 == st >> 4) {
                            continue;
                        }
                    }
                    let rgba = tint_rgba(tint_for(info, f), SHADE[f]);
                    if smooth && on_edge {
                        face(geo, f, lo, hi, base, tex[f], rgba, &|c| smooth_corner(p, t, [x, y, z], f, c));
                    } else {
                        let mut l = p.light(nx, ny, nz);
                        if !on_edge {
                            l = max_light(l, own);
                        }
                        let l = flat(l);
                        face(geo, f, lo, hi, base, tex[f], rgba, &|_| l);
                    }
                }
            }
        }
    }
}

/// Visibilidade da seção, como `RenderChunk.rebuildChunk`: os blocos opacos vão para o `VisGraph`,
/// que liga as faces por onde o ar passa.
pub fn section_visibility(p: &Padded, t: &BlockTable) -> SetVisibility {
    let mut g = VisGraph::new();
    for y in 0..16 {
        for z in 0..16 {
            for x in 0..16 {
                if is_opaque(t, p.block(x, y, z)) {
                    g.set_opaque(x, y, z);
                }
            }
        }
    }
    g.compute_visibility()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ar_liga_todas_as_faces_e_pedra_so_fecha() {
        let t = BlockTable::new();
        assert_eq!(section_visibility(&Padded::new(0xF0), &t), SetVisibility::all(true), "seção de ar");
        let mut p = Padded::new(0xF0);
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    p.blocks[Padded::idx(x, y, z)] = 1 << 4;
                }
            }
        }
        assert_eq!(section_visibility(&p, &t), SetVisibility::default(), "seção de pedra");
    }

    #[test]
    fn single_stone_has_six_faces_and_buried_none() {
        let t = BlockTable::new();
        let mut p = Padded::new(0xF0);
        p.blocks[Padded::idx(5, 5, 5)] = 1 << 4;
        let m = mesh_section(&p, [0, 0, 0], &t);
        assert_eq!(m.opaque.indices.len(), 6 * 6);

        // Cercado de pedra por todos os lados: só as faces de fora dos vizinhos.
        for (dx, dy, dz) in [(-1, 0, 0), (1, 0, 0), (0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1)] {
            p.blocks[Padded::idx(5 + dx, 5 + dy, 5 + dz)] = 1 << 4;
        }
        let m = mesh_section(&p, [0, 0, 0], &t);
        assert_eq!(m.opaque.indices.len(), 6 * 5 * 6);
    }

    #[test]
    fn corner_next_to_wall_is_darker() {
        let t = BlockTable::new();
        let mut p = Padded::new(0xF0);
        // Chão de pedra com uma parede ao norte do bloco do meio.
        for x in 0..16 {
            for z in 0..16 {
                p.blocks[Padded::idx(x, 0, z)] = 1 << 4;
            }
        }
        p.blocks[Padded::idx(5, 1, 4)] = 1 << 4;
        let near = smooth_corner(&p, &t, [5, 0, 5], UP, [0.5, 1.0, 0.0]);
        let open = smooth_corner(&p, &t, [5, 0, 5], UP, [0.5, 1.0, 1.0]);
        assert!(near[2] < open[2], "AO perto da parede: {near:?} vs {open:?}");
    }

    #[test]
    fn water_goes_to_translucent() {
        let t = BlockTable::new();
        let mut p = Padded::new(0xF0);
        p.blocks[Padded::idx(0, 0, 0)] = 9 << 4;
        let m = mesh_section(&p, [0, 0, 0], &t);
        assert!(m.opaque.is_empty());
        assert!(!m.translucent.is_empty());
    }
}

#[cfg(test)]
mod model_tests {
    use super::*;
    use crate::blocks::jar_tests::table;

    fn quads(m: &MeshData) -> usize {
        (m.opaque.indices.len() + m.cutout_mipped.indices.len() + m.cutout.indices.len() + m.translucent.indices.len()) / 6
    }

    #[test]
    fn baked_quad_counts() {
        let Some(t) = table() else { return };
        let mut p = Padded::new(0xF0);
        p.blocks[Padded::idx(5, 5, 5)] = 1 << 4;
        assert_eq!(quads(&mesh_section(&p, [0, 0, 0], &t)), 6, "pedra sozinha");
        p.blocks[Padded::idx(5, 6, 5)] = 1 << 4;
        assert_eq!(quads(&mesh_section(&p, [0, 0, 0], &t)), 10, "duas pedras empilhadas");

        // Escada solta: base com 6 faces + degrau com 6 (o degrau também tem face de baixo).
        let mut p = Padded::new(0xF0);
        p.blocks[Padded::idx(5, 5, 5)] = 53 << 4;
        assert_eq!(quads(&mesh_section(&p, [0, 0, 0], &t)), 12);
        p.blocks[Padded::idx(5, 4, 5)] = 1 << 4;
        assert_eq!(quads(&mesh_section(&p, [0, 0, 0], &t)), 6 + 10, "pedra embaixo esconde as duas faces de baixo");
    }

    #[test]
    fn grass_layer_section() {
        let Some(t) = table() else { return };
        // Camada de grama 16x16 solta no ar: topo e base de todos, lados (com a sobreposição) só na borda.
        let mut p = Padded::new(0xF0);
        for x in 0..16 {
            for z in 0..16 {
                p.blocks[Padded::idx(x, 0, z)] = 2 << 4;
            }
        }
        let m = mesh_section(&p, [0, 0, 0], &t);
        assert_eq!(quads(&m), 256 * 2 + 64 * 2);
        // Topo tingido de verde; vértices de cima totalmente iluminados pelo céu. Grama é CUTOUT_MIPPED.
        assert!(m.opaque.is_empty());
        let top = m.cutout_mipped.vertices.iter().find(|v| v.pos[1] == 1.0 && v.color[1] > v.color[0]).unwrap();
        assert_eq!(top.light[0], 255);
    }

    #[test]
    fn ao_darkens_inner_corner() {
        let Some(t) = table() else { return };
        let mut p = Padded::new(0xF0);
        for x in 0..16 {
            for z in 0..16 {
                p.blocks[Padded::idx(x, 0, z)] = 1 << 4;
            }
        }
        p.blocks[Padded::idx(5, 1, 4)] = 1 << 4;
        let m = mesh_section(&p, [0, 0, 0], &t);
        // Vértices de faces de cima (brilho 1.0) do bloco (5,0,5) no canto colado na parede (z = 5) e do lado aberto (z = 6).
        let ao_at = |x: f32, z: f32| m.opaque.vertices.iter().filter(|v| v.pos == [x, 1.0, z] && v.color[0] == 255).map(|v| v.light[2]).min().unwrap();
        assert!(ao_at(5.0, 5.0) < ao_at(5.0, 6.0), "{} vs {}", ao_at(5.0, 5.0), ao_at(5.0, 6.0));
        // Um vizinho opaco no canto: (0.2 + 1 + 1 + 1) / 4 = 0.8; aberto = 1.0.
        assert_eq!(ao_at(5.0, 5.0), 204);
        assert_eq!(ao_at(5.0, 6.0), 255);
    }
}
