//! Nível de detalhe (opções "LOD" e "Distância LOD" das Opções de vídeo; não existe no jogo original).
//!
//! `mesh_section_lod`: a seção vira uma grade de células de `step` blocos (2 ou 4). A célula é sólida quando
//! pelo menos metade dos blocos dela é sólida e vira um cubo grande com a textura do bloco mais alto dela;
//! plantas, tochas e afins somem, a água fica só com a superfície. Luz plana do bloco à frente de cada face.
//!
//! `FarColumn` e `mesh_far_tile`: resumo de uma coluna (altura, bloco do topo e bioma de cada x, z) e o terreno
//! de longe feito desses resumos, um mapa de alturas em células de `step` blocos com paredes entre as alturas.

use crate::biome;
use crate::blocks::{BlockTable, Model};
use crate::chunk::{Column, Section};
use crate::mesh::{atlas_uv, flat, section_visibility, tint_for, Ctx, Geometry, MeshData, Padded, Vertex, BORDER, DIRS, SHADE};

/// Escurecimento extra das laterais de superfície (que usam a textura do topo).
const SURFACE_SIDE: f32 = 0.72;

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
                        // Lateral de célula de superfície: de longe, um morro de degraus de 1 bloco mostra quase só o
                        // topo dos blocos; a lateral esticada (grama em cima, terra embaixo) deixaria o morro marrom.
                        let surface = f >= 2 && at([cx, cy + 1, cz]) != Kind::Solid;
                        let (tex, tinted) = face_look(t, st, if surface { 1 } else { f });
                        let rgb = if tinted { ctx.tint_at(st, [lo[0] + half, lo[1] + step - 1, lo[2] + half], wpos) } else { 0xFFFFFF };
                        // A lateral de verdade tem terra e sombra da oclusão: o topo usado nela vem mais escuro.
                        let k = if surface { SHADE[f] * SURFACE_SIDE } else { SHADE[f] };
                        big_face(&mut m.opaque, f, wlo, whi, tex, crate::mesh::tint_rgba(rgb, k), light);
                    }
                }
            }
        }
    }
    m.visibility = section_visibility(p, t);
    m
}

/// Resumo de uma coluna para o terreno de longe: para cada x, z (índice z * 16 + x) a altura do topo
/// (y + 1 do bloco mais alto que não é ar nem planta; 0 = vazio), o bloco do topo e o bioma.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FarColumn {
    pub height: [u8; 256],
    pub state: [u16; 256],
    pub biome: [u8; 256],
}

impl Default for FarColumn {
    fn default() -> Self {
        Self { height: [0; 256], state: [0; 256], biome: [1; 256] }
    }
}

/// Bytes por coluna em `FarColumn::to_bytes`.
pub const FAR_COLUMN_BYTES: usize = 256 * 4;

impl FarColumn {
    pub fn summarize(col: &Column, t: &BlockTable) -> Self {
        let mut out = Self::default();
        let top = col.sections.iter().rposition(|s| s.is_some());
        for z in 0..16 {
            for x in 0..16 {
                let i = (z * 16 + x) as usize;
                if let Some(b) = &col.biomes {
                    out.biome[i] = if b[i] == 255 { 1 } else { b[i] };
                }
                let Some(top) = top else { continue };
                'col: for sy in (0..=top).rev() {
                    let Some(s) = &col.sections[sy] else { continue };
                    for y in (0..16).rev() {
                        let st = s.blocks[Section::index(x, y, z)];
                        if kind(t, st) != Kind::Air {
                            out.height[i] = (sy as i32 * 16 + y + 1).min(255) as u8;
                            out.state[i] = st;
                            break 'col;
                        }
                    }
                }
            }
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.height.iter().all(|&h| h == 0)
    }

    pub fn to_bytes(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.height);
        for s in &self.state {
            out.extend_from_slice(&s.to_le_bytes());
        }
        out.extend_from_slice(&self.biome);
    }

    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        if b.len() < FAR_COLUMN_BYTES {
            return None;
        }
        let mut c = Self::default();
        c.height.copy_from_slice(&b[..256]);
        for i in 0..256 {
            c.state[i] = u16::from_le_bytes([b[256 + i * 2], b[257 + i * 2]]);
        }
        c.biome.copy_from_slice(&b[768..1024]);
        Some(c)
    }
}

/// Cor do bioma para o topo de longe (o `colorMultiplier` sem a média 3x3).
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

/// Terreno de longe do bloco de `FAR_TILE` x `FAR_TILE` chunks que começa no chunk `(cx0, cz0)`, em células de
/// `step` blocos (divisor de 16). `get` devolve o resumo de uma coluna (também as vizinhas, para as paredes).
/// Tudo opaco, com luz do céu cheia.
pub fn mesh_far_tile<'c>(cx0: i32, cz0: i32, step: i32, t: &BlockTable, get: &dyn Fn(i32, i32) -> Option<&'c FarColumn>) -> Geometry {
    let mut geo = Geometry::default();
    let step = step.clamp(1, 16);
    let per = 16 / step;
    // Altura e bloco de uma célula: o ponto mais alto dela (mantém morros e árvores); e o mais baixo, até onde
    // descem as paredes das vizinhas (na borda com o terreno de perto, a encosta de verdade fica coberta).
    let cell = |gx: i32, gz: i32| -> Option<(i32, u16, u8, i32)> {
        let (cx, cz) = (gx.div_euclid(per), gz.div_euclid(per));
        let c = get(cx, cz)?;
        let (lx, lz) = (gx.rem_euclid(per) * step, gz.rem_euclid(per) * step);
        let mut best: Option<(i32, u16, u8)> = None;
        let mut low = i32::MAX;
        for z in lz..lz + step {
            for x in lx..lx + step {
                let i = (z * 16 + x) as usize;
                let h = c.height[i] as i32;
                if h > 0 && best.map_or(true, |b| h > b.0) {
                    best = Some((h, c.state[i], c.biome[i]));
                }
                if h > 0 {
                    low = low.min(h);
                }
            }
        }
        let (h, st, b) = best.unwrap_or((0, 0, 1));
        Some((h, st, b, if low == i32::MAX { 0 } else { low }))
    };
    let light = flat(0xF0);
    let s = step as f32;
    for gz in cz0 * per..(cz0 + FAR_TILE) * per {
        for gx in cx0 * per..(cx0 + FAR_TILE) * per {
            let Some((h, st, b, _)) = cell(gx, gz) else { continue };
            if h == 0 {
                continue;
            }
            let (x0, z0) = ((gx * step) as f32, (gz * step) as f32);
            let wpos = [gx * step, h - 1, gz * step];
            let water = matches!(st >> 4, 8 | 9);
            let rgb_of = |f: usize| -> (u16, u32) {
                if water {
                    return (t.fluid_tex[0][0], biome::biome(b).water_color);
                }
                if matches!(st >> 4, 10 | 11) {
                    return (t.fluid_tex[1][0], 0xFFFFFF);
                }
                let (tex, tinted) = face_look(t, st, f);
                (tex, if tinted { far_tint(t, st, b, wpos) } else { 0xFFFFFF })
            };
            let top = if water { h as f32 - 0.125 } else { h as f32 };
            let (tex, rgb) = rgb_of(1);
            big_face(&mut geo, 1, [x0, top, z0], [x0 + s, top, z0 + s], tex, crate::mesh::tint_rgba(rgb, SHADE[1]), light);
            // Paredes até a célula vizinha mais baixa (sem dados: um degrau curto, para não abrir buracos).
            for (f, (dx, dz)) in [(2usize, (0, -1)), (3, (0, 1)), (4, (-1, 0)), (5, (1, 0))] {
                let edge = match f {
                    2 => gz.rem_euclid(per) == 0,
                    3 => gz.rem_euclid(per) == per - 1,
                    4 => gx.rem_euclid(per) == 0,
                    _ => gx.rem_euclid(per) == per - 1,
                };
                let (hn, low) = match cell(gx + dx, gz + dz) {
                    Some((hn, _, _, low)) => (hn as f32, low as f32),
                    None => {
                        let d = (h - 2 * step).max(0) as f32;
                        (d, d)
                    }
                };
                // Na divisa de chunks (onde pode estar a borda com o terreno de perto) a parede desce até o ponto
                // mais baixo da vizinha; dentro do chunk, até o mais alto.
                let floor = if edge { low.min(hn) } else { hn };
                if floor >= top {
                    // Água encostada em água na divisa de chunks: uma cortina de 12 blocos para baixo, dentro desta
                    // célula. Quando a vizinha é do terreno de perto, quem olha pela água dela (de cima, inclinado)
                    // vê a cortina em vez do fundo do céu.
                    if water && edge {
                        let (lo, hi) = match f {
                            2 => ([x0, top - 12.0, z0 + 0.02], [x0 + s, top, z0 + 0.02]),
                            3 => ([x0, top - 12.0, z0 + s - 0.02], [x0 + s, top, z0 + s - 0.02]),
                            4 => ([x0 + 0.02, top - 12.0, z0], [x0 + 0.02, top, z0 + s]),
                            _ => ([x0 + s - 0.02, top - 12.0, z0], [x0 + s - 0.02, top, z0 + s]),
                        };
                        let (tex, rgb) = rgb_of(f);
                        big_face(&mut geo, f, lo, hi, tex, crate::mesh::tint_rgba(rgb, SHADE[f]), light);
                    }
                    continue;
                }
                // Degrau baixo (encosta) com a textura do topo; penhasco com a lateral.
                let slope = top - hn <= (2 * step) as f32;
                let hn = floor;
                let (tex, rgb) = rgb_of(if slope { 1 } else { f });
                let k = if slope { SHADE[f] * SURFACE_SIDE } else { SHADE[f] };
                let rgba = crate::mesh::tint_rgba(rgb, k);
                // Na divisa de chunks a parede sai duas vezes, 0,02 bloco para cada lado: o recorte do terreno de
                // perto (por coluna) mantém a do lado de longe, e a borda perto/longe não abre fresta em nenhum
                // sentido. Dentro do chunk, uma só.
                let offsets: &[f32] = if edge { &[-0.02, 0.02] } else { &[0.0] };
                for &o in offsets {
                    let (lo, hi) = match f {
                        2 => ([x0, hn, z0 + o], [x0 + s, top, z0 + o]),
                        3 => ([x0, hn, z0 + s + o], [x0 + s, top, z0 + s + o]),
                        4 => ([x0 + o, hn, z0], [x0 + o, top, z0 + s]),
                        _ => ([x0 + s + o, hn, z0], [x0 + s + o, top, z0 + s]),
                    };
                    big_face(&mut geo, f, lo, hi, tex, rgba, light);
                }
            }
        }
    }
    geo
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

    #[test]
    fn resumo_ida_e_volta() {
        let mut c = FarColumn::default();
        c.height[5] = 70;
        c.state[5] = 2 << 4;
        c.biome[5] = 4;
        let mut b = Vec::new();
        c.to_bytes(&mut b);
        assert_eq!(b.len(), FAR_COLUMN_BYTES);
        assert_eq!(FarColumn::from_bytes(&b), Some(c));
    }

    #[test]
    fn terreno_de_longe_tem_topo_e_paredes() {
        let t = BlockTable::new();
        let mut c = FarColumn::default();
        for i in 0..256 {
            c.height[i] = 64;
            c.state[i] = 2 << 4;
        }
        // Um degrau de 4 blocos na metade x >= 8 da coluna.
        for z in 0..16 {
            for x in 8..16 {
                c.height[z * 16 + x] = 68;
            }
        }
        let get = |cx: i32, cz: i32| (cx == 0 && cz == 0).then_some(&c);
        let g = mesh_far_tile(0, 0, 4, &t, &get);
        let tops = g.vertices.chunks(4).filter(|q| q.iter().all(|v| v.pos[1] == q[0].pos[1])).count();
        assert!(tops >= 16);
        assert!(g.vertices.iter().any(|v| v.pos[1] == 68.0) && g.vertices.iter().any(|v| v.pos[1] == 64.0));
    }
}
