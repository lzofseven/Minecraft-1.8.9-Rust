//! Mirar e interagir com blocos (porte de MovingObjectPosition, PlayerControllerMP e Block.getPlayerRelativeBlockHardness).

use glam::DVec3;
use mc_world::blocks::BlockTable;
use mc_world::gen_tools::{BLOCK_HARDNESS, TOOL_HARVEST, TOOL_STRENGTH};
use mc_world::mesh::{atlas_uv, Geometry, Vertex};
use mc_world::World;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub pos: (i32, i32, i32),
    /// 0 baixo, 1 cima, 2 norte, 3 sul, 4 oeste, 5 leste (igual ao protocolo).
    pub face: u8,
    pub point: DVec3,
}

/// Caixa de seleção do bloco (`getSelectedBoundingBox` gerado do jogo).
fn selection_boxes(t: &BlockTable, w: &World, x: i32, y: i32, z: i32, out: &mut Vec<[f64; 6]>) {
    let st = w.block(x, y, z);
    if let Some(b) = t.selection(st, &|dx, dy, dz| w.block(x + dx, y + dy, z + dz)) {
        out.push(b);
    }
}

/// Entrada do raio numa caixa: (distância, face).
pub fn ray_box(from: DVec3, dir: DVec3, min: DVec3, max: DVec3) -> Option<(f64, u8)> {
    let mut t0 = f64::NEG_INFINITY;
    let mut t1 = f64::INFINITY;
    let mut face = 0u8;
    for axis in 0..3 {
        let (o, d, lo, hi) = (from[axis], dir[axis], min[axis], max[axis]);
        if d.abs() < 1e-12 {
            if o < lo || o > hi {
                return None;
            }
            continue;
        }
        let (mut a, mut b) = ((lo - o) / d, (hi - o) / d);
        let mut entry_face = match axis {
            0 => 4,
            1 => 0,
            _ => 2,
        };
        if a > b {
            std::mem::swap(&mut a, &mut b);
            entry_face += 1;
        }
        if a > t0 {
            t0 = a;
            face = entry_face;
        }
        t1 = t1.min(b);
        if t0 > t1 {
            return None;
        }
    }
    (t0 >= 0.0).then_some((t0, face))
}

/// Bloco mirado até `reach` blocos (DDA pelas células, depois as caixas de cada uma).
pub fn raycast(w: &World, t: &BlockTable, from: DVec3, dir: DVec3, reach: f64) -> Option<Hit> {
    let dir = dir.normalize();
    let mut cell = from.floor().as_ivec3();
    let step = dir.signum().as_ivec3();
    let next = |c: f64, d: f64| if d > 0.0 { (c.floor() + 1.0 - c) / d } else if d < 0.0 { (c - c.floor()) / -d } else { f64::INFINITY };
    let mut t_max = DVec3::new(next(from.x, dir.x), next(from.y, dir.y), next(from.z, dir.z));
    let t_delta = DVec3::new(1.0 / dir.x.abs(), 1.0 / dir.y.abs(), 1.0 / dir.z.abs());
    let mut boxes = Vec::new();
    for _ in 0..((reach * 3.0) as usize + 3) {
        boxes.clear();
        selection_boxes(t, w, cell.x, cell.y, cell.z, &mut boxes);
        let o = cell.as_dvec3();
        let best = boxes
            .iter()
            .filter_map(|b| {
                ray_box(from, dir, o + DVec3::new(b[0], b[1], b[2]), o + DVec3::new(b[3], b[4], b[5]))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((d, face)) = best {
            if d <= reach {
                return Some(Hit { pos: (cell.x, cell.y, cell.z), face, point: from + dir * d });
            }
            return None;
        }
        let axis = if t_max.x < t_max.y && t_max.x < t_max.z { 0 } else if t_max.y < t_max.z { 1 } else { 2 };
        if t_max[axis] > reach {
            return None;
        }
        cell[axis] += step[axis];
        t_max[axis] += t_delta[axis];
    }
    None
}

/// Dureza (`Block.blockHardness`, gerada de `Block.registerBlocks`); negativa = inquebrável.
pub fn hardness(id: u16) -> f32 {
    BLOCK_HARDNESS.get(id as usize).map_or(0.0, |b| b.0)
}

/// `Item.getStrVsBlock` da ferramenta na mão (1.0 sem item ou sem efeito).
fn str_vs_block(held: Option<i16>, block: u16) -> f32 {
    let Some(item) = held.filter(|i| *i >= 256) else { return 1.0 };
    TOOL_STRENGTH
        .binary_search_by(|(i, b, _)| (*i, *b as u16).cmp(&(item as u16, block)))
        .map_or(1.0, |k| TOOL_STRENGTH[k].2)
}

/// `Block.canHarvestBlock`: material que dispensa ferramenta, ou `Item.canHarvestBlock` da mão.
fn can_harvest(held: Option<i16>, block: u16) -> bool {
    if BLOCK_HARDNESS.get(block as usize).is_some_and(|b| b.1) {
        return true;
    }
    held.filter(|i| *i >= 256)
        .is_some_and(|item| TOOL_HARVEST.binary_search(&(item as u16, block as u8)).is_ok())
}

/// Progresso de quebra por tick (`Block.getPlayerRelativeBlockHardness` com `EntityPlayer.getBreakSpeed`,
/// sem encantamento de eficiência nem efeitos de poção, que dependem do estado do jogador).
/// Modificadores do `EntityPlayer.getBreakSpeed` além do item: Eficiência, Pressa e Fadiga de Mineração (amplificador).
#[derive(Clone, Copy, Default)]
pub struct DigMods {
    pub efficiency: i32,
    pub haste: Option<i8>,
    pub fatigue: Option<i8>,
}

/// `ForgeHooks.blockStrength` com o `EntityPlayer.getBreakSpeed`; `in_water` = olhos na água sem Afinidade Aquática.
pub fn break_speed(block: u16, held: Option<i16>, on_ground: bool, in_water: bool, m: DigMods) -> f32 {
    let h = hardness(block);
    if h < 0.0 {
        return 0.0;
    }
    let mut speed = str_vs_block(held, block);
    if speed > 1.0 && m.efficiency > 0 && held.is_some() {
        speed += (m.efficiency * m.efficiency + 1) as f32;
    }
    if let Some(a) = m.haste {
        speed *= 1.0 + (a as f32 + 1.0) * 0.2;
    }
    if let Some(a) = m.fatigue {
        speed *= match a {
            0 => 0.3,
            1 => 0.09,
            2 => 0.0027,
            _ => 8.1e-4,
        };
    }
    if in_water {
        speed /= 5.0;
    }
    if !on_ground {
        speed /= 5.0;
    }
    if h == 0.0 {
        return 1.0; // divisão por zero no original = quebra instantânea
    }
    speed / h / if can_harvest(held, block) { 30.0 } else { 100.0 }
}

const FULL: [u8; 4] = [255, 255, 255, 0];

fn cuboid(geo: &mut Geometry, min: DVec3, max: DVec3, uv: impl Fn(usize, f32, f32) -> [f32; 2], color: [u8; 4]) {
    let (a, b) = (min.as_vec3(), max.as_vec3());
    let faces: [[[f32; 3]; 4]; 6] = [
        [[a.x, a.y, a.z], [b.x, a.y, a.z], [b.x, a.y, b.z], [a.x, a.y, b.z]],
        [[a.x, b.y, a.z], [b.x, b.y, a.z], [b.x, b.y, b.z], [a.x, b.y, b.z]],
        [[a.x, a.y, a.z], [b.x, a.y, a.z], [b.x, b.y, a.z], [a.x, b.y, a.z]],
        [[a.x, a.y, b.z], [b.x, a.y, b.z], [b.x, b.y, b.z], [a.x, b.y, b.z]],
        [[a.x, a.y, a.z], [a.x, a.y, b.z], [a.x, b.y, b.z], [a.x, b.y, a.z]],
        [[b.x, a.y, a.z], [b.x, a.y, b.z], [b.x, b.y, b.z], [b.x, b.y, a.z]],
    ];
    let corners = [(0.0, 16.0), (16.0, 16.0), (16.0, 0.0), (0.0, 0.0)];
    for (f, q) in faces.iter().enumerate() {
        let mut i = 0;
        geo.quad(q.map(|pos| {
            let (u, v) = corners[i];
            i += 1;
            Vertex { pos, uv: uv(f, u, v), color, light: FULL }
        }));
    }
}

/// Contorno do bloco mirado (caixas de seleção): preto 40% com `glLineWidth(2)`, em linhas finas.
pub fn outline(w: &World, t: &BlockTable, hit: &Hit, white: [f32; 2]) -> Geometry {
    let (color, width) = ([0, 0, 0, 102], 2.0);
    let mut geo = Geometry::default();
    let mut boxes = Vec::new();
    selection_boxes(t, w, hit.pos.0, hit.pos.1, hit.pos.2, &mut boxes);
    let o = DVec3::new(hit.pos.0 as f64, hit.pos.1 as f64, hit.pos.2 as f64);
    let e = 0.002;
    for b in boxes {
        let min = o + DVec3::new(b[0], b[1], b[2]) - e;
        let max = o + DVec3::new(b[3], b[4], b[5]) + e;
        box_edges_w(&mut geo, min, max, white, color, LINE * width / 2.0);
    }
    geo
}

/// Meia espessura das linhas desenhadas como caixas finas.
const LINE: f64 = 0.006;

/// `RenderGlobal.drawOutlinedBoundingBox`: as 12 arestas como caixas finas.
fn box_edges(geo: &mut Geometry, min: DVec3, max: DVec3, white: [f32; 2], color: [u8; 4]) {
    box_edges_w(geo, min, max, white, color, LINE);
}

fn box_edges_w(geo: &mut Geometry, min: DVec3, max: DVec3, white: [f32; 2], color: [u8; 4], line: f64) {
    for axis in 0..3 {
        for (s1, s2) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
            let (a1, a2) = ((axis + 1) % 3, (axis + 2) % 3);
            let mut lo = min;
            let mut hi = max;
            let p1 = if s1 == 0 { min[a1] } else { max[a1] };
            let p2 = if s2 == 0 { min[a2] } else { max[a2] };
            lo[a1] = p1 - line;
            hi[a1] = p1 + line;
            lo[a2] = p2 - line;
            hi[a2] = p2 + line;
            cuboid(geo, lo, hi, |_, _, _| white, color);
        }
    }
}

/// Segmento como prisma fino (o `GL_LINE_STRIP` do vetor de olhar).
pub fn segment(geo: &mut Geometry, a: DVec3, b: DVec3, white: [f32; 2], color: [u8; 4]) {
    let d = (b - a).normalize_or_zero();
    let side = if d.y.abs() < 0.99 { d.cross(DVec3::Y) } else { d.cross(DVec3::X) }.normalize() * LINE;
    let up = d.cross(side).normalize() * LINE;
    let v = |p: DVec3| Vertex { pos: p.as_vec3().to_array(), uv: white, color, light: FULL };
    for (s, u) in [(side, up), (up, -side), (-side, -up), (-up, side)] {
        geo.quad([v(a + s + u), v(b + s + u), v(b + s - u), v(a + s - u)]);
    }
}

/// `RenderManager.renderDebugBoundingBox` (F3+B): caixa branca; nos vivos, a faixa vermelha na altura dos
/// olhos e o olhar em azul com 2 blocos. `pos` são os pés, `look` o `getLook(partial)`.
pub fn debug_box(geo: &mut Geometry, pos: DVec3, width: f64, height: f64, living: Option<(f64, DVec3)>, white: [f32; 2]) {
    let f = width / 2.0;
    box_edges(geo, pos - DVec3::new(f, 0.0, f), pos + DVec3::new(f, height, f), white, [255, 255, 255, 255]);
    if let Some((eye, look)) = living {
        let y = pos.y + eye;
        box_edges(geo, DVec3::new(pos.x - f, y - 0.01, pos.z - f), DVec3::new(pos.x + f, y + 0.01, pos.z + f), white, [255, 0, 0, 255]);
        let from = DVec3::new(pos.x, y, pos.z);
        segment(geo, from, from + look * 2.0, white, [0, 0, 255, 255]);
    }
}

/// `RenderFish`: a linha do anzol em 16 pedaços da ponta da vara (`rod`) até a boia (`hook`, já com +0,25),
/// caindo em `(f² + f) / 2`; `dy` é o ajuste de altura do pescador (olhos na 1ª pessoa, agachado na 3ª).
pub fn fishing_line(geo: &mut Geometry, rod: DVec3, hook: DVec3, dy: f64, white: [f32; 2]) {
    // Os deltas passam por float no Java.
    let d = DVec3::new((rod.x - hook.x) as f32 as f64, (rod.y - hook.y) as f32 as f64 + dy, (rod.z - hook.z) as f32 as f64);
    let at = |f: f64| DVec3::new(hook.x + d.x * f, hook.y + d.y * (f * f + f) * 0.5, hook.z + d.z * f);
    for l in 0..16 {
        let (f0, f1) = (l as f64 / 16.0, (l + 1) as f64 / 16.0);
        segment(geo, at(f0), at(f1), white, [0, 0, 0, 255]);
    }
}

/// `Vec3.rotatePitch` e `Vec3.rotateYaw` com o seno/cosseno de tabela do `MathHelper`.
pub fn rotate_pitch(v: DVec3, a: f32) -> DVec3 {
    let (c, s) = (crate::mth::cos(a) as f64, crate::mth::sin(a) as f64);
    DVec3::new(v.x, v.y * c + v.z * s, v.z * c - v.y * s)
}

pub fn rotate_yaw(v: DVec3, a: f32) -> DVec3 {
    let (c, s) = (crate::mth::cos(a) as f64, crate::mth::sin(a) as f64);
    DVec3::new(v.x * c + v.z * s, v.y, v.z * c - v.x * s)
}

/// `Entity.getVectorForRotation`.
pub fn look_vector(pitch: f32, yaw: f32) -> DVec3 {
    let f = (-yaw * 0.017453292 - std::f32::consts::PI).cos();
    let f1 = (-yaw * 0.017453292 - std::f32::consts::PI).sin();
    let f2 = -(-pitch * 0.017453292).cos();
    let f3 = (-pitch * 0.017453292).sin();
    DVec3::new((f1 * f2) as f64, f3 as f64, (f * f2) as f64)
}

/// Rachaduras sobre o bloco sendo quebrado (estágio 0..9).
pub fn cracks(t: &BlockTable, pos: (i32, i32, i32), stage: usize) -> Geometry {
    let mut geo = Geometry::default();
    let o = DVec3::new(pos.0 as f64, pos.1 as f64, pos.2 as f64);
    let tex = t.destroy[stage.min(9)];
    cuboid(&mut geo, o - 0.003, o + 1.003, |_, u, v| atlas_uv(tex, u, v), [255, 255, 255, 255]);
    geo
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_world::{ChunkData, Section};

    fn world_with_block(x: i32, y: i32, z: i32, state: u16) -> World {
        let mut w = World::new(true);
        w.apply(ChunkData { x: 0, z: 0, full: true, mask: 1, sections: vec![(0, Box::new(Section::empty(true)))], biomes: None });
        w.set_block(x, y, z, state);
        w
    }

    #[test]
    fn ray_hits_front_face_of_stone() {
        let t = BlockTable::new();
        let w = world_with_block(5, 4, 8, 1 << 4);
        let hit = raycast(&w, &t, DVec3::new(5.5, 4.5, 5.0), DVec3::new(0.0, 0.0, 1.0), 4.5).unwrap();
        assert_eq!(hit.pos, (5, 4, 8));
        assert_eq!(hit.face, 2, "entra pela face norte");
        assert!((hit.point.z - 8.0).abs() < 1e-9);
        assert!(raycast(&w, &t, DVec3::new(5.5, 4.5, 5.0), DVec3::new(0.0, 0.0, -1.0), 4.5).is_none());
        assert!(raycast(&w, &t, DVec3::new(5.5, 4.5, 2.0), DVec3::new(0.0, 0.0, 1.0), 4.5).is_none(), "fora do alcance");
    }

    #[test]
    fn ray_from_above_hits_slab_top() {
        let t = BlockTable::new();
        let w = world_with_block(3, 2, 3, 44 << 4);
        let hit = raycast(&w, &t, DVec3::new(3.5, 5.0, 3.5), DVec3::new(0.0, -1.0, 0.0), 4.5).unwrap();
        assert_eq!((hit.pos, hit.face), ((3, 2, 3), 1));
        assert!((hit.point.y - 2.5).abs() < 1e-9);
    }

    #[test]
    fn break_times_match_vanilla() {
        // Pedra com picareta de madeira: 2/1.5/30 por tick -> 23 ticks (1,15 s).
        let s = break_speed(1 << 4 >> 4, Some(270), true, false, DigMods::default());
        assert!(((1.0 / s).ceil() - 23.0).abs() < 1e-6, "{}", 1.0 / s);
        // Pedra na mão: 1/1.5/100 -> 150 ticks (7,5 s).
        assert!(((1.0 / break_speed(1, None, true, false, DigMods::default())) - 150.0).abs() < 1e-3);
        assert_eq!(break_speed(31, None, true, false, DigMods::default()), 1.0);
        assert_eq!(break_speed(7, Some(278), true, false, DigMods::default()), 0.0);
        // Pedra: dureza 1,5 sem ferramenta adequada (/100) e com picareta de diamante (força 8, /30).
        assert_eq!(hardness(1), 1.5);
        assert!((break_speed(1, Some(278), true, false, DigMods::default()) - 8.0 / 1.5 / 30.0).abs() < 1e-6);
        // Terra com pá de ferro (6) e obsidiana sem picareta de diamante (/100); no ar e na água, 1/5.
        assert!((break_speed(3, Some(256), true, false, DigMods::default()) - 6.0 / 0.5 / 30.0).abs() < 1e-6);
        assert!((break_speed(49, Some(257), true, false, DigMods::default()) - 6.0 / 50.0 / 100.0).abs() < 1e-6);
        assert!((break_speed(49, Some(278), true, false, DigMods::default()) - 8.0 / 50.0 / 30.0).abs() < 1e-6);
        assert!((break_speed(3, None, false, true, DigMods::default()) - 1.0 / 25.0 / 0.5 / 30.0).abs() < 1e-6);
        // Teia: espada (15) e tesoura (15) colhem; mão não.
        assert!((break_speed(30, Some(267), true, false, DigMods::default()) - 15.0 / 4.0 / 30.0).abs() < 1e-6);
        assert!((break_speed(30, None, true, false, DigMods::default()) - 1.0 / 4.0 / 100.0).abs() < 1e-6);
        // Lã com tesoura: força 5.
        assert!((break_speed(35, Some(359), true, false, DigMods::default()) - 5.0 / 0.8 / 30.0).abs() < 1e-6);
    }
}
