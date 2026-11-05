//! Nível de detalhe (opções "LOD" e "Distância LOD" das Opções de vídeo; não existe no jogo original).
//!
//! `mesh_section_lod`: a seção vira uma grade de células de `step` blocos (2 ou 4). A célula é sólida quando
//! pelo menos metade dos blocos dela é sólida e vira um cubo grande com a textura do bloco mais alto dela;
//! plantas, tochas e afins somem, a água fica só com a superfície. Luz plana do bloco à frente de cada face.
//!
//! `FarColumn` e `mesh_far_tile`: resumo de uma coluna (para cada x, z os trechos sólidos, a água e a copa) e o
//! terreno de longe feito desses resumos, em células de `step` blocos.

use crate::biome;
use crate::blocks::{BlockTable, Model};
use crate::chunk::{Column, Section};
use crate::mesh::{atlas_uv, flat, section_visibility, tint_for, Ctx, Geometry, MeshData, Padded, Vertex, BORDER, DIRS, SHADE};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Air,
    Solid,
    Liquid,
}

fn kind(t: &BlockTable, st: u16) -> Kind {
    if st == 0 || (t.models.is_some() && !t.valid(st)) {
        return Kind::Air;
    }
    match t.get(st).model {
        Model::Air | Model::Invisible | Model::Unsupported | Model::Cross | Model::DoublePlant => Kind::Air,
        Model::Liquid { .. } => Kind::Liquid,
        _ => Kind::Solid,
    }
}

/// Textura da face `f` do bloco e se ela leva a cor do bioma (o maior quad virado para `f` do modelo).
fn face_look(t: &BlockTable, st: u16, f: usize) -> (u16, bool) {
    if let Some(models) = &t.models {
        if let Some(w) = models.has(st).then(|| models.get(st, 0)).flatten() {
            let m = w.pick(0, 0, 0);
            return match m.quads.iter().find(|q| q.face as usize == f) {
                Some(q) => (q.tex, q.tint >= 0),
                None => (m.particle, false),
            };
        }
    }
    let info = t.get(st);
    (info.tex[f], tint_for(info, f) != 0xFFFFFF)
}

/// Tipo da célula (`lo` e `size` em coordenadas locais do `Padded`) e o bloco mais alto do tipo dela.
/// `strict` (vizinha fora da seção, vista só em parte): sólida só se todos os blocos vistos forem sólidos.
fn classify(p: &Padded, t: &BlockTable, lo: [i32; 3], size: [i32; 3], strict: bool) -> (Kind, u16) {
    let (mut solid, mut liquid, mut n) = (0u32, 0u32, 0u32);
    let mut top: [Option<u16>; 2] = [None, None];
    for y in (lo[1]..lo[1] + size[1]).rev() {
        for z in lo[2]..lo[2] + size[2] {
            for x in lo[0]..lo[0] + size[0] {
                let st = p.block(x, y, z);
                n += 1;
                match kind(t, st) {
                    Kind::Solid => {
                        solid += 1;
                        top[0].get_or_insert(st);
                    }
                    Kind::Liquid => {
                        liquid += 1;
                        top[1].get_or_insert(st);
                    }
                    Kind::Air => {}
                }
            }
        }
    }
    let need = if strict { n } else { n.div_ceil(2) };
    if n == 0 {
        (Kind::Air, 0)
    } else if solid >= need {
        (Kind::Solid, top[0].unwrap_or(0))
    } else if solid + liquid >= need && liquid > 0 {
        (Kind::Liquid, top[1].unwrap_or(0))
    } else {
        (Kind::Air, 0)
    }
}

/// Quadrilátero de uma face de caixa com a textura inteira esticada (coordenadas do mundo).
fn big_face(geo: &mut Geometry, f: usize, lo: [f32; 3], hi: [f32; 3], tex: u16, rgba: [u8; 4], light: [u8; 4]) {
    let [x0, y0, z0] = lo;
    let [x1, y1, z1] = hi;
    let mut corners = match f {
        0 | 1 => {
            let y = if f == 0 { y0 } else { y1 };
            [[x0, y, z0], [x1, y, z0], [x1, y, z1], [x0, y, z1]]
        }
        2 | 3 => {
            let z = if f == 2 { z0 } else { z1 };
            [[x0, y0, z], [x1, y0, z], [x1, y1, z], [x0, y1, z]]
        }
        _ => {
            let x = if f == 4 { x0 } else { x1 };
            [[x, y0, z0], [x, y0, z1], [x, y1, z1], [x, y1, z0]]
        }
    };
    // Sentido anti-horário visto de fora (o terreno descarta as faces de costas).
    let e1 = [corners[1][0] - corners[0][0], corners[1][1] - corners[0][1], corners[1][2] - corners[0][2]];
    let e2 = [corners[2][0] - corners[0][0], corners[2][1] - corners[0][1], corners[2][2] - corners[0][2]];
    let nrm = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
    let d = DIRS[f];
    if nrm[0] * (d[0] as f32) + nrm[1] * (d[1] as f32) + nrm[2] * (d[2] as f32) < 0.0 {
        corners.reverse();
    }
    let rel = |v: f32, a: f32, b: f32| if b > a { (v - a) / (b - a) * 16.0 } else { 0.0 };
    geo.quad_fixed(corners.map(|[x, y, z]| {
        let (u, v) = match f {
            0 | 1 => (rel(x, x0, x1), rel(z, z0, z1)),
            2 | 3 => (rel(x, x0, x1), 16.0 - rel(y, y0, y1)),
            _ => (rel(z, z0, z1), 16.0 - rel(y, y0, y1)),
        };
        Vertex { pos: [x, y, z], uv: atlas_uv(tex, u, v), color: rgba, light }
    }));
}

/// Malha simplificada da seção (`level` 1 = células de 2 blocos, 2 = de 4). A visibilidade é a mesma da malha
/// cheia, então o `culling` não muda.
pub fn mesh_section_lod(p: &Padded, origin: [i32; 3], t: &BlockTable, level: u8) -> MeshData {
    let step = 1i32 << level.clamp(1, 2);
    let n = 16 / step;
    let idx = |c: [i32; 3]| ((c[1] * n + c[2]) * n + c[0]) as usize;
    let mut cells = vec![(Kind::Air, 0u16); (n * n * n) as usize];
    for cy in 0..n {
        for cz in 0..n {
            for cx in 0..n {
                cells[idx([cx, cy, cz])] = classify(p, t, [cx * step, cy * step, cz * step], [step; 3], false);
            }
        }
    }
    // Vizinho fora da seção: os blocos da borda do `Padded` (até `BORDER` de fundo). A seção vizinha decide
    // sozinha, com a célula inteira; aqui, na dúvida, a face fica (escondida dentro do cubo dela), nunca um buraco.
    let at = |c: [i32; 3]| -> Kind {
        if c.iter().all(|v| (0..n).contains(v)) {
            return cells[idx(c)].0;
        }
        let mut lo = [0; 3];
        let mut size = [0; 3];
        for a in 0..3 {
            let a0 = (c[a] * step).max(-BORDER);
            let a1 = (c[a] * step + step).min(16 + BORDER);
            lo[a] = a0;
            size[a] = (a1 - a0).max(0);
        }
        classify(p, t, lo, size, true).0
    };
    let mut m = MeshData::default();
    let ctx = Ctx { p, t };
    let half = step / 2;
    for cy in 0..n {
        for cz in 0..n {
            for cx in 0..n {
                let (k, st) = cells[idx([cx, cy, cz])];
                if k == Kind::Air {
                    continue;
                }
                let lo = [cx * step, cy * step, cz * step];
                let lava = matches!(st >> 4, 10 | 11);
                for (f, d) in DIRS.iter().enumerate() {
                    let nb = at([cx + d[0], cy + d[1], cz + d[2]]);
                    let show = match k {
                        Kind::Solid => nb != Kind::Solid,
                        _ => nb == Kind::Air && f != 0,
                    };
                    if !show {
                        continue;
                    }
                    // Luz do bloco à frente do meio da face.
                    let mut q = [lo[0] + half, lo[1] + half, lo[2] + half];
                    for a in 0..3 {
                        if d[a] < 0 {
                            q[a] = lo[a] - 1;
                        } else if d[a] > 0 {
                            q[a] = lo[a] + step;
                        }
                        q[a] = q[a].clamp(-BORDER, 15 + BORDER);
                    }
                    let light = flat(p.light(q[0], q[1], q[2]));
                    let wlo = [(origin[0] + lo[0]) as f32, (origin[1] + lo[1]) as f32, (origin[2] + lo[2]) as f32];
                    let mut whi = wlo.map(|v| v + step as f32);
                    let wpos = [origin[0] + lo[0], origin[1] + lo[1] + step - 1, origin[2] + lo[2]];
                    if k == Kind::Liquid {
                        let (tex, rgb) = if lava {
                            (t.fluid_tex[1][0], 0xFFFFFF)
                        } else {
                            (t.fluid_tex[0][0], biome::biome(p.biome(lo[0] + half, lo[2] + half)).water_color)
                        };
                        whi[1] -= 0.125;
                        let geo = if lava { &mut m.opaque } else { &mut m.translucent };
                        big_face(geo, f, wlo, whi, tex, crate::mesh::tint_rgba(rgb, SHADE[f]), light);
                    } else {
                        let (tex, tinted) = face_look(t, st, f);
                        let rgb = if tinted { ctx.tint_at(st, [lo[0] + half, lo[1] + step - 1, lo[2] + half], wpos) } else { 0xFFFFFF };
                        big_face(&mut m.opaque, f, wlo, whi, tex, crate::mesh::tint_rgba(rgb, SHADE[f]), light);
                    }
                }
            }
        }
    }
    m.visibility = section_visibility(p, t);
    m
}

/// Bloco visto pelo terreno de longe: chão (vira trecho sólido), água, folhas (camada da copa) ou nada.
/// Troncos, cercas, portas, tochas, plantas, trilhos e afins não contam: não viram pilar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FarKind {
    Air,
    Ground,
    Water,
    Leaves,
}

fn far_kind(t: &BlockTable, st: u16) -> FarKind {
    match st >> 4 {
        0 => return FarKind::Air,
        8 | 9 => return FarKind::Water,
        18 | 161 => return FarKind::Leaves,
        17 | 162 | 26 | 27 | 28 | 30 | 39 | 40 | 50 | 51 | 55 | 63 | 64 | 65 | 66 | 68 | 69 | 70 | 71 | 72 | 75 | 76 | 77 | 78 | 90 | 93 | 94
        | 96 | 106 | 111 | 119 | 131 | 132 | 143 | 144 | 147 | 148 | 149 | 150 | 157 | 167 | 171 | 176 | 177 | 193..=197 => {
            return FarKind::Air
        }
        _ => {}
    }
    if t.models.is_some() && !t.valid(st) {
        return FarKind::Air;
    }
    match t.get(st).model {
        Model::Air | Model::Invisible | Model::Unsupported | Model::Cross | Model::DoublePlant | Model::Connected(_) | Model::Door { .. } => FarKind::Air,
        _ => FarKind::Ground,
    }
}

/// Trecho sólido de uma coluna: do bloco `bottom` até `top - 1`; `top_state` é o bloco de cima e `fill_state`
/// o de 3 blocos abaixo dele (a lateral: grama em cima, terra ou pedra embaixo).
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Span {
    pub bottom: u8,
    pub top: u8,
    pub top_state: u16,
    pub fill_state: u16,
}

/// Trechos guardados por coluna, de cima para baixo; o resto vai junto no último (cavernas fundas somem).
pub const FAR_SPANS: usize = 3;

/// Uma coluna x, z do resumo: os trechos sólidos (sem fundo falso: ilhas no vazio continuam flutuando), a
/// superfície da água acima do chão (`water` = y + 1, 0 = sem), a copa das árvores acima do chão (`canopy` = y + 1
/// do topo das folhas, com `canopy_depth` de espessura) e o bioma.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct FarCell {
    pub n: u8,
    pub spans: [Span; FAR_SPANS],
    pub water: u8,
    pub canopy: u8,
    pub canopy_depth: u8,
    pub canopy_state: u16,
    pub biome: u8,
}

impl FarCell {
    fn spans(&self) -> &[Span] {
        &self.spans[..self.n as usize]
    }

    fn top(&self) -> i32 {
        self.spans().first().map_or(0, |s| s.top as i32)
    }

    fn push(&mut self, s: Span) {
        if (self.n as usize) < FAR_SPANS {
            self.spans[self.n as usize] = s;
            self.n += 1;
        } else {
            self.spans[FAR_SPANS - 1].bottom = s.bottom;
        }
    }
}

const CELL_BYTES: usize = 1 + FAR_SPANS * 6 + 6;
/// Bytes por coluna em `FarColumn::to_bytes`.
pub const FAR_COLUMN_BYTES: usize = 256 * CELL_BYTES;

/// Resumo de uma coluna de chunk para o terreno de longe (índice z * 16 + x).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FarColumn {
    pub cells: Box<[FarCell; 256]>,
}

impl Default for FarColumn {
    fn default() -> Self {
        Self { cells: Box::new([FarCell { biome: 1, ..Default::default() }; 256]) }
    }
}

impl FarColumn {
    pub fn summarize(col: &Column, t: &BlockTable) -> Self {
        let mut out = Self::default();
        let Some(top_sec) = col.sections.iter().rposition(|s| s.is_some()) else { return out };
        for z in 0..16 {
            for x in 0..16 {
                let i = (z * 16 + x) as usize;
                let c = &mut out.cells[i];
                if let Some(b) = &col.biomes {
                    c.biome = if b[i] == 255 { 1 } else { b[i] };
                }
                let mut run: Option<Span> = None;
                let mut ground = false;
                let mut prev_leaf = false;
                for y in (0..(top_sec as i32 + 1) * 16).rev() {
                    let st = col.sections[(y >> 4) as usize].as_ref().map_or(0, |s| s.blocks[Section::index(x, y & 15, z)]);
                    let k = far_kind(t, st);
                    if k == FarKind::Ground {
                        ground = true;
                        prev_leaf = false;
                        match &mut run {
                            None => run = Some(Span { bottom: y as u8, top: (y + 1).min(255) as u8, top_state: st, fill_state: st }),
                            Some(s) => {
                                s.bottom = y as u8;
                                if s.top as i32 - y <= 4 {
                                    s.fill_state = st;
                                }
                            }
                        }
                        continue;
                    }
                    if let Some(s) = run.take() {
                        c.push(s);
                    }
                    if !ground {
                        match k {
                            FarKind::Water if c.water == 0 => c.water = (y + 1).min(255) as u8,
                            FarKind::Leaves if c.canopy == 0 => {
                                c.canopy = (y + 1).min(255) as u8;
                                c.canopy_state = st;
                                c.canopy_depth = 1;
                            }
                            FarKind::Leaves if prev_leaf && c.canopy_depth < 8 => c.canopy_depth += 1,
                            _ => {}
                        }
                    }
                    prev_leaf = k == FarKind::Leaves;
                }
                if let Some(s) = run {
                    c.push(s);
                }
            }
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.cells.iter().all(|c| c.n == 0 && c.water == 0 && c.canopy == 0)
    }

    pub fn to_bytes(&self, out: &mut Vec<u8>) {
        for c in self.cells.iter() {
            out.push(c.n);
            for s in &c.spans {
                out.extend_from_slice(&[s.bottom, s.top]);
                out.extend_from_slice(&s.top_state.to_le_bytes());
                out.extend_from_slice(&s.fill_state.to_le_bytes());
            }
            out.extend_from_slice(&[c.water, c.canopy, c.canopy_depth]);
            out.extend_from_slice(&c.canopy_state.to_le_bytes());
            out.push(c.biome);
        }
    }

    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        if b.len() < FAR_COLUMN_BYTES {
            return None;
        }
        let mut col = Self::default();
        let u16_at = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
        for (k, c) in col.cells.iter_mut().enumerate() {
            let o = k * CELL_BYTES;
            c.n = b[o].min(FAR_SPANS as u8);
            for (j, s) in c.spans.iter_mut().enumerate() {
                let p = o + 1 + j * 6;
                *s = Span { bottom: b[p], top: b[p + 1], top_state: u16_at(p + 2), fill_state: u16_at(p + 4) };
            }
            let p = o + 1 + FAR_SPANS * 6;
            (c.water, c.canopy, c.canopy_depth, c.canopy_state, c.biome) = (b[p], b[p + 1], b[p + 2], u16_at(p + 3), b[p + 5]);
        }
        Some(col)
    }
}

/// Cor do bioma para o terreno de longe (o `colorMultiplier` sem a média 3x3).
fn far_tint(t: &BlockTable, st: u16, biome_id: u8, wpos: [i32; 3]) -> u32 {
    let b = biome::biome(biome_id);
    let Some(maps) = &t.colormaps else { return t.tint_color(st) };
    let [x, y, z] = wpos;
    match st >> 4 {
        2 | 31 | 175 => maps.grass_at(&b, x, y, z),
        18 => match st & 3 {
            1 => 0x619961,
            2 => 0x80A755,
            _ => maps.foliage_at(&b, x, y, z),
        },
        161 | 106 => maps.foliage_at(&b, x, y, z),
        _ => t.tint_color(st),
    }
}

/// Lado do bloco de terreno de longe, em chunks.
pub const FAR_TILE: i32 = 4;

/// Terreno de longe de um bloco: a parte opaca e a água (translúcida, como a de perto).
#[derive(Default)]
pub struct FarMesh {
    pub opaque: Geometry,
    pub translucent: Geometry,
}

/// `[lo, hi)` menos os trechos `cover`.
fn uncovered(lo: f32, hi: f32, cover: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let mut out = vec![(lo, hi)];
    for &(a, b) in cover {
        out = out
            .into_iter()
            .flat_map(|(l, h)| {
                let mut v = Vec::new();
                if a > l {
                    v.push((l, a.min(h)));
                }
                if b < h {
                    v.push((b.max(l), h));
                }
                v
            })
            .filter(|(l, h)| h - l > 0.001)
            .collect();
    }
    out
}

/// Terreno de longe do bloco de `FAR_TILE` x `FAR_TILE` chunks que começa no chunk `(cx0, cz0)`, em células de
/// `step` blocos (divisor de 16). Cada célula usa a coluna mais alta dela; paredes só onde o lado fica exposto
/// (até o trecho da vizinha, ou o fundo do trecho: nada de pilar até y = 0), com a lateral do bloco de cima no
/// primeiro bloco e a do bloco de dentro no resto. Copa como placa, água translúcida. Luz do céu cheia.
pub fn mesh_far_tile<'c>(cx0: i32, cz0: i32, step: i32, t: &BlockTable, get: &dyn Fn(i32, i32) -> Option<&'c FarColumn>) -> FarMesh {
    let mut m = FarMesh::default();
    let step = step.clamp(1, 16);
    let per = 16 / step;
    // Célula: a coluna de chão mais alta (ou, com `low`, a mais baixa: a parede da borda com o terreno de perto
    // desce até o chão de verdade da coluna vizinha), com a água e a copa mais altas da célula.
    let cell = |gx: i32, gz: i32, low: bool| -> Option<FarCell> {
        let col = get(gx.div_euclid(per), gz.div_euclid(per))?;
        let (lx, lz) = (gx.rem_euclid(per) * step, gz.rem_euclid(per) * step);
        let mut best: Option<FarCell> = None;
        let (mut water, mut canopy) = (0u8, None::<FarCell>);
        for z in lz..lz + step {
            for x in lx..lx + step {
                let c = col.cells[(z * 16 + x) as usize];
                let better = match best {
                    None => true,
                    Some(b) if low => c.top() < b.top(),
                    Some(b) => c.top() > b.top(),
                };
                if better {
                    best = Some(c);
                }
                water = water.max(c.water);
                if c.canopy > canopy.map_or(0, |k| k.canopy) {
                    canopy = Some(c);
                }
            }
        }
        let mut c = best?;
        c.water = water;
        if let Some(k) = canopy {
            (c.canopy, c.canopy_depth, c.canopy_state) = (k.canopy, k.canopy_depth, k.canopy_state);
        }
        Some(c)
    };
    let light = flat(0xF0);
    let s = step as f32;
    let look = |st: u16, f: usize, b: u8, wpos: [i32; 3]| -> (u16, u32) {
        if matches!(st >> 4, 10 | 11) {
            return (t.fluid_tex[1][0], 0xFFFFFF);
        }
        let (tex, tinted) = face_look(t, st, f);
        (tex, if tinted { far_tint(t, st, b, wpos) } else { 0xFFFFFF })
    };
    for gz in cz0 * per..(cz0 + FAR_TILE) * per {
        for gx in cx0 * per..(cx0 + FAR_TILE) * per {
            let Some(c) = cell(gx, gz, false) else { continue };
            let (x0, z0) = ((gx * step) as f32, (gz * step) as f32);
            let wpos = |y: i32| [gx * step, y, gz * step];
            let sides = [(2usize, (0, -1)), (3, (0, 1)), (4, (-1, 0)), (5, (1, 0))];
            let edge_of = |f: usize| match f {
                2 => gz.rem_euclid(per) == 0,
                3 => gz.rem_euclid(per) == per - 1,
                4 => gx.rem_euclid(per) == 0,
                _ => gx.rem_euclid(per) == per - 1,
            };
            // Parede vertical de `lo` a `hi` no lado `f`. Na divisa de chunks sai duas vezes, 0,02 bloco para cada
            // lado: o recorte do terreno de perto (por coluna) mantém a do lado de longe.
            let wall = |geo: &mut Geometry, f: usize, lo: f32, hi: f32, tex: u16, rgba: [u8; 4]| {
                let offsets: &[f32] = if edge_of(f) { &[-0.02, 0.02] } else { &[0.0] };
                for &o in offsets {
                    let (a, b) = match f {
                        2 => ([x0, lo, z0 + o], [x0 + s, hi, z0 + o]),
                        3 => ([x0, lo, z0 + s + o], [x0 + s, hi, z0 + s + o]),
                        4 => ([x0 + o, lo, z0], [x0 + o, hi, z0 + s]),
                        _ => ([x0 + s + o, lo, z0], [x0 + s + o, hi, z0 + s]),
                    };
                    big_face(geo, f, a, b, tex, rgba, light);
                }
            };
            for sp in c.spans() {
                let (bot, top) = (sp.bottom as f32, sp.top as f32);
                let (tex, rgb) = look(sp.top_state, 1, c.biome, wpos(sp.top as i32 - 1));
                big_face(&mut m.opaque, 1, [x0, top, z0], [x0 + s, top, z0 + s], tex, crate::mesh::tint_rgba(rgb, SHADE[1]), light);
                // Fundo só onde dá para ver: com vão embaixo na própria coluna (ilha sobre o chão) ou acima do chão de
                // alguma vizinha (borda de mapa no vazio). O fundo de um chão normal fica escondido e não entra.
                let gap_below = c.spans().iter().any(|q| q.top <= sp.bottom);
                let open_below = gap_below || sides.iter().any(|&(_, (dx, dz))| cell(gx + dx, gz + dz, false).is_some_and(|n| n.top() <= sp.bottom as i32));
                if sp.bottom > 0 && open_below {
                    let (tex, rgb) = look(sp.fill_state, 0, c.biome, wpos(sp.bottom as i32));
                    big_face(&mut m.opaque, 0, [x0, bot, z0], [x0 + s, bot, z0 + s], tex, crate::mesh::tint_rgba(rgb, SHADE[0]), light);
                }
                for (f, (dx, dz)) in sides {
                    let cover: Vec<(f32, f32)> = match cell(gx + dx, gz + dz, edge_of(f)) {
                        Some(n) => n.spans().iter().map(|q| (q.bottom as f32, q.top as f32)).collect(),
                        // Sem dados ao lado: um degrau curto, sem parede até o fundo.
                        None => vec![(bot, (top - 2.0 * s).max(bot))],
                    };
                    for (lo, hi) in uncovered(bot, top, &cover) {
                        // Bloco de cima com a lateral dele; o resto com a lateral do bloco de dentro.
                        let cut = (top - 1.0).clamp(lo, hi);
                        if hi > cut {
                            let (tex, rgb) = look(sp.top_state, f, c.biome, wpos(sp.top as i32 - 1));
                            wall(&mut m.opaque, f, cut, hi, tex, crate::mesh::tint_rgba(rgb, SHADE[f]));
                        }
                        if cut > lo {
                            let (tex, rgb) = look(sp.fill_state, f, c.biome, wpos(sp.top as i32 - 4));
                            wall(&mut m.opaque, f, lo, cut, tex, crate::mesh::tint_rgba(rgb, SHADE[f]));
                        }
                    }
                }
            }
            // Copa: placa da espessura das folhas, sem nada embaixo.
            if c.canopy > 0 && c.canopy as i32 > c.top() {
                let top = c.canopy as f32;
                let bot = (top - c.canopy_depth.max(1) as f32).max(c.top() as f32);
                let st = c.canopy_state;
                let (tex, rgb) = look(st, 1, c.biome, wpos(c.canopy as i32 - 1));
                big_face(&mut m.opaque, 1, [x0, top, z0], [x0 + s, top, z0 + s], tex, crate::mesh::tint_rgba(rgb, SHADE[1]), light);
                if bot > c.top() as f32 {
                    big_face(&mut m.opaque, 0, [x0, bot, z0], [x0 + s, bot, z0 + s], tex, crate::mesh::tint_rgba(rgb, SHADE[0]), light);
                }
                for (f, (dx, dz)) in sides {
                    let cover: Vec<(f32, f32)> = match cell(gx + dx, gz + dz, false) {
                        Some(n) if n.canopy as i32 > n.top() => vec![((n.canopy as f32 - n.canopy_depth.max(1) as f32), n.canopy as f32), (0.0, n.top() as f32)],
                        Some(n) => vec![(0.0, n.top() as f32)],
                        None => Vec::new(),
                    };
                    for (lo, hi) in uncovered(bot, top, &cover) {
                        wall(&mut m.opaque, f, lo, hi, tex, crate::mesh::tint_rgba(rgb, SHADE[f]));
                    }
                }
            }
            // Água: só a superfície, translúcida e com a cor do bioma (o fundo aparece embaixo).
            if c.water > 0 && c.water as i32 > c.top() {
                let y = c.water as f32 - 0.125;
                let rgb = biome::biome(c.biome).water_color;
                big_face(&mut m.translucent, 1, [x0, y, z0], [x0 + s, y, z0 + s], t.fluid_tex[0][0], crate::mesh::tint_rgba(rgb, 1.0), light);
            }
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::mesh_section;

    fn padded_with(f: impl Fn(i32, i32, i32) -> u16) -> Padded {
        let mut p = Padded::new(0xF0);
        for y in -BORDER..16 + BORDER {
            for z in -BORDER..16 + BORDER {
                for x in -BORDER..16 + BORDER {
                    p.blocks[Padded::idx(x, y, z)] = f(x, y, z);
                }
            }
        }
        p
    }

    #[test]
    fn chao_plano_vira_poucas_faces() {
        let t = BlockTable::new();
        // Pedra até y = 7 local, ar em cima: o topo fica no mesmo plano (y = 8).
        let p = padded_with(|_, y, _| if y < 8 { 1 << 4 } else { 0 });
        let full = mesh_section(&p, [0, 0, 0], &t);
        for level in [1u8, 2] {
            let m = mesh_section_lod(&p, [0, 0, 0], &t, level);
            let step = 1 << level;
            let quads = m.opaque.indices.len() / 6;
            assert_eq!(quads, (16 / step) * (16 / step), "nível {level}");
            assert!(m.opaque.vertices.iter().all(|v| v.pos[1] <= 8.0));
            assert!(m.opaque.vertices.iter().any(|v| v.pos[1] == 8.0));
            assert!(quads < full.opaque.indices.len() / 6);
            assert_eq!(m.visibility, full.visibility);
        }
    }

    #[test]
    fn plantas_somem_e_secao_vazia_fica_vazia() {
        let t = BlockTable::new();
        let p = padded_with(|_, y, _| if y == 0 { 31 << 4 | 1 } else { 0 });
        let m = mesh_section_lod(&p, [0, 0, 0], &t, 1);
        assert!(m.opaque.is_empty() && m.translucent.is_empty());
    }

    /// Coluna com chão até y = 63 (grama no topo), uma árvore em (3, 5) (tronco até 69, copa 68 a 71), um lago
    /// em (8, 8) (fundo 60, água até 62) e uma ilha flutuante de 90 a 94 em (12, 12).
    fn coluna() -> Column {
        let mut col = Column::default();
        let mut put = |x: i32, y: i32, z: i32, st: u16| {
            let s = col.sections[(y >> 4) as usize].get_or_insert_with(|| Box::new(Section::empty(true)));
            s.blocks[Section::index(x, y & 15, z)] = st;
        };
        for z in 0..16 {
            for x in 0..16 {
                let top = if (x, z) == (8, 8) { 60 } else { 63 };
                for y in 40..=top {
                    put(x, y, z, if y == top { 2 << 4 } else if y > top - 3 { 3 << 4 } else { 1 << 4 });
                }
            }
        }
        for y in 61..=62 {
            put(8, y, 8, 9 << 4);
        }
        for y in 64..70 {
            put(3, y, 5, 17 << 4);
        }
        for y in 68..72 {
            put(3, y, 5, 18 << 4);
            put(4, y, 5, 18 << 4);
        }
        for y in 90..95 {
            put(12, y, 12, 3 << 4);
        }
        col
    }

    #[test]
    fn resumo_tem_trechos_copa_e_agua() {
        let t = BlockTable::new();
        let c = FarColumn::summarize(&coluna(), &t);
        let at = |x: usize, z: usize| c.cells[z * 16 + x];
        let chao = at(0, 0);
        assert_eq!((chao.n, chao.spans[0].bottom, chao.spans[0].top, chao.spans[0].top_state, chao.spans[0].fill_state), (1, 40, 64, 2 << 4, 1 << 4));
        // Tronco não vira pilar: a árvore é chão até 64 e copa de 72 com 4 de espessura.
        let arvore = at(3, 5);
        assert_eq!((arvore.top(), arvore.canopy, arvore.canopy_depth), (64, 72, 4));
        let lago = at(8, 8);
        assert_eq!((lago.top(), lago.water), (61, 63));
        let ilha = at(12, 12);
        assert_eq!((ilha.n, ilha.spans[0].bottom, ilha.spans[0].top, ilha.spans[1].top), (2, 90, 95, 64));
        let mut b = Vec::new();
        c.to_bytes(&mut b);
        assert_eq!(b.len(), FAR_COLUMN_BYTES);
        assert_eq!(FarColumn::from_bytes(&b), Some(c));
    }

    #[test]
    fn terreno_de_longe_sem_pilar() {
        let t = BlockTable::new();
        let c = FarColumn::summarize(&coluna(), &t);
        let get = |cx: i32, cz: i32| (cx == 0 && cz == 0).then_some(&c);
        let m = mesh_far_tile(0, 0, 1, &t, &get);
        let quads: Vec<&[Vertex]> = m.opaque.vertices.chunks(4).collect();
        // Nada da ilha (x 12..13, z 12..13) desce abaixo do fundo dela, e nada sai da copa até o chão.
        let ilha = |q: &&&[Vertex]| q.iter().all(|v| (12.0..=13.0).contains(&v.pos[0]) && (12.0..=13.0).contains(&v.pos[2]) && v.pos[1] > 64.5);
        assert!(quads.iter().filter(ilha).count() >= 6);
        assert!(quads.iter().filter(ilha).all(|q| q.iter().all(|v| v.pos[1] >= 90.0)));
        assert!(quads.iter().filter(ilha).any(|q| q.iter().all(|v| v.pos[1] == 90.0)), "ilha sem fundo");
        // O fundo do chão (y = 40) fica escondido: nenhuma face lá.
        assert!(!quads.iter().any(|q| q.iter().all(|v| v.pos[1] == 40.0)));
        let copa = |q: &&&[Vertex]| q.iter().all(|v| (3.0..=5.0).contains(&v.pos[0]) && (5.0..=6.0).contains(&v.pos[2]) && v.pos[1] > 64.5);
        assert!(quads.iter().filter(copa).count() >= 2);
        assert!(quads.iter().filter(copa).all(|q| q.iter().all(|v| v.pos[1] >= 68.0)));
        // Água só translúcida, na superfície.
        assert!(!m.translucent.is_empty() && m.translucent.vertices.iter().all(|v| v.pos[1] == 62.875));
    }
}
