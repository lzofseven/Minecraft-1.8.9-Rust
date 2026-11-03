//! Quais seções desenhar, como o `RenderGlobal.setupTerrain` do jogo: uma BFS a
//! partir da seção do olho que só passa por faces ligadas pelo ar
//! (`SetVisibility`), dentro do frustum e do alcance de desenho.
//!
//! Seção sem entrada em `vis` (ainda sem malha, ou sem blocos) conta como toda
//! visível, então nada some enquanto a malha não chega. O `setupTerrain` do jogo
//! trata seções sem malha da mesma forma quando são de ar.

use std::collections::{HashMap, VecDeque};

use glam::{Vec3, Vec4};
use mc_render::{aabb_visible, Camera, SectionSet};
use mc_world::blocks::BlockTable;
use mc_world::visgraph::{is_opaque, opposite, SetVisibility, VisGraph, NORTH};
use mc_world::World;

type Key = (i32, i32, i32);

/// Passo de cada face, na ordem do `EnumFacing` (baixo, cima, norte, sul, oeste, leste).
const STEP: [[i32; 3]; 6] = [[0, -1, 0], [0, 1, 0], [0, 0, -1], [0, 0, 1], [-1, 0, 0], [1, 0, 0]];

/// O que a BFS lê da cena do cliente.
pub struct Scene<'a> {
    pub world: &'a World,
    /// Visibilidade das seções com malha (`MeshData::visibility`).
    pub vis: &'a HashMap<Key, SetVisibility>,
    pub table: &'a BlockTable,
    pub cam: &'a Camera,
    /// Planos do frustum da câmera (`Renderer::cam_planes`).
    pub planes: [Vec4; 5],
    /// `thirdPersonView == 2`: o jogo inverte o olhar ao escolher a face que sai da lista.
    pub rear_view: bool,
    /// Espectador: com o olho dentro de bloco opaco, a BFS deixa de usar a visibilidade.
    pub spectator: bool,
}

/// Item da fila: a seção, a face por onde entrou (`None` na semente) e as faces já usadas no caminho.
struct Entry {
    key: Key,
    from: Option<usize>,
    path: u8,
}

fn center(k: Key) -> Vec3 {
    Vec3::new(k.0 as f32 * 16.0 + 8.0, k.1 as f32 * 16.0 + 8.0, k.2 as f32 * 16.0 + 8.0)
}

/// `EnumFacing.getFacingFromVector`: a face de maior produto escalar (empate fica com a primeira).
fn facing_from(v: Vec3) -> usize {
    let mut best = NORTH;
    let mut best_dot = f32::from_bits(1);
    for (face, d) in STEP.iter().enumerate() {
        let dot = v.x * d[0] as f32 + v.y * d[1] as f32 + v.z * d[2] as f32;
        if dot > best_dot {
            best_dot = dot;
            best = face;
        }
    }
    best
}

/// Grafo de visibilidade da seção, lido do mundo (`RenderChunk.rebuildChunk`).
/// Lê a seção direto (sem procurar a coluna a cada bloco); seção ausente é só ar.
fn section_graph(sc: &Scene, key: Key) -> VisGraph {
    let mut g = VisGraph::new();
    let Some(sec) = sc.world.columns.get(&(key.0, key.2)).and_then(|c| c.sections.get(key.1 as usize)).and_then(|s| s.as_ref()) else { return g };
    for y in 0..16 {
        for z in 0..16 {
            for x in 0..16 {
                if is_opaque(sc.table, sec.blocks[mc_world::chunk::Section::index(x, y, z)]) {
                    g.set_opaque(x, y, z);
                }
            }
        }
    }
    g
}

/// Seções a desenhar (`renderInfos` do `setupTerrain`), já com a checagem de frustum e alcance.
/// Tudo numa grade em volta da câmera (sem hash): as colunas carregadas são lidas uma vez por quadro.
pub fn visible_sections(sc: &Scene) -> SectionSet {
    let cam = sc.cam;
    // O mesmo alcance que `render` usa para desenhar: a BFS nunca fica mais curta que o desenho.
    let max_d2 = (cam.view_distance + 24.0).powi(2);
    let in_range = |k: Key| {
        let d = center(k) - cam.eye;
        d.x * d.x + d.z * d.z <= max_d2
    };
    let in_frustum = |k: Key| aabb_visible(&sc.planes, center(k) - Vec3::splat(8.0), center(k) + Vec3::splat(8.0));
    let vis_of = |k: Key| sc.vis.get(&k).copied().unwrap_or(SetVisibility::all(true));

    let eye = cam.eye.floor();
    let (ex, ey, ez) = (eye.x as i32, eye.y as i32, eye.z as i32);
    let start: Key = (ex >> 4, ey >> 4, ez >> 4);
    // A grade cobre todo o alcance (centro da seção a até view + 24 do olho).
    let r = ((cam.view_distance + 24.0) / 16.0).ceil() as i32 + 2;
    let mut out = SectionSet::new(start.0, start.2, r);
    let mut marked = SectionSet::new(start.0, start.2, r);
    let (x0, z0, w) = out.bounds();
    let mut columns = vec![false; (w * w) as usize];
    for dx in 0..w {
        for dz in 0..w {
            columns[(dx * w + dz) as usize] = sc.world.columns.contains_key(&(x0 + dx, z0 + dz));
        }
    }
    let loaded = |k: Key| (0..16).contains(&k.1) && out.column(k.0, k.2).is_some_and(|c| columns[c]);
    let mut queue: VecDeque<Entry> = VecDeque::new();
    // `renderChunksMany` vem ligado por padrão no jogo.
    let mut many = true;

    if loaded(start) {
        let mut g = section_graph(sc, start);
        let mut faces = g.facings_from(ex & 15, ey & 15, ez & 15);
        if faces.count_ones() == 1 {
            let look = if sc.rear_view { -cam.forward() } else { cam.forward() };
            faces &= !(1 << opposite(facing_from(look)));
        }
        if faces == 0 && !sc.spectator {
            // Olho numa caverna fechada: só a própria seção.
            let mut only = out;
            only.insert(start);
            return only;
        }
        if sc.spectator && is_opaque(sc.table, sc.world.block(ex, ey, ez)) {
            many = false;
        }
        marked.insert(start);
        queue.push_back(Entry { key: start, from: None, path: 0 });
    } else {
        // Seção do olho sem dados: semente em todas as colunas na linha de cima (ou de baixo), como o jogo.
        let row = if ey > 0 { 15 } else { 0 };
        let mut seeds: Vec<Key> = sc.world.columns.keys().map(|&(cx, cz)| (cx, row, cz)).filter(|&k| in_range(k) && in_frustum(k) && loaded(k)).collect();
        seeds.sort();
        for k in seeds {
            marked.insert(k);
            queue.push_back(Entry { key: k, from: None, path: 0 });
        }
    }

    let mut found = Vec::new();
    while let Some(e) = queue.pop_front() {
        found.push(e.key);
        // A visibilidade da seção é lida uma vez, não por face.
        let vis = (many && e.from.is_some()).then(|| vis_of(e.key));
        for face in 0..6 {
            // Sem voltar pelo caminho: a face oposta a uma já usada não entra.
            if many && e.path & (1 << opposite(face)) != 0 {
                continue;
            }
            // Só sai pela face que a seção liga à de entrada.
            if let (Some(v), Some(from)) = (vis, e.from) {
                if !v.is_visible(opposite(from), face) {
                    continue;
                }
            }
            let s = STEP[face];
            let n: Key = (e.key.0 + s[0], e.key.1 + s[1], e.key.2 + s[2]);
            if !loaded(n) || !in_range(n) || !marked.insert(n) || !in_frustum(n) {
                continue;
            }
            queue.push_back(Entry { key: n, from: Some(face), path: e.path | 1 << face });
        }
    }
    for k in found {
        out.insert(k);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_world::chunk::Section;
    use mc_world::ChunkData;

    fn stone(pocket: bool) -> Box<Section> {
        let mut s = Box::new(Section::empty(true));
        for b in s.blocks.iter_mut() {
            *b = 1 << 4;
        }
        if pocket {
            // Bolsão de dois blocos de ar no meio: não chega à borda da seção.
            s.blocks[Section::index(5, 5, 5)] = 0;
            s.blocks[Section::index(6, 5, 5)] = 0;
        }
        s
    }

    /// Coluna carregada só de ar: uma seção vazia (sem nenhuma, `apply` descarregaria a coluna).
    fn air_column(w: &mut World, cx: i32, cz: i32) {
        w.apply(ChunkData { x: cx, z: cz, full: true, mask: 1, sections: vec![(0, Box::new(Section::empty(true)))], biomes: None });
    }

    fn cam(eye: Vec3) -> Camera {
        Camera { eye, yaw: 0.0, pitch: 0.0, fov_deg: 70.0, view_distance: 64.0, view: None }
    }

    /// Colunas (0,0) e (2,0) só de ar; (1,0) inteira de pedra. Sem o resto do mundo carregado,
    /// a parede de x = 1 é o único caminho.
    fn wall_world() -> (World, HashMap<Key, SetVisibility>) {
        let mut w = World::new(true);
        air_column(&mut w, 0, 0);
        air_column(&mut w, 2, 0);
        let sections = (0..16).map(|y| (y, stone(false))).collect();
        w.apply(ChunkData { x: 1, z: 0, full: true, mask: 0xFFFF, sections, biomes: None });
        // Seções de pedra têm visibilidade nula; as de ar ficam sem entrada (tudo visível).
        let vis = (0..16).map(|y| ((1, y, 0), SetVisibility::default())).collect();
        (w, vis)
    }

    #[test]
    fn parede_de_pedra_bloqueia_a_passagem() {
        let (w, vis) = wall_world();
        let table = BlockTable::new();
        let c = cam(Vec3::new(4.5, 20.5, 8.5));
        let sc = Scene { world: &w, vis: &vis, table: &table, cam: &c, planes: [Vec4::W; 5], rear_view: false, spectator: false };
        let got = visible_sections(&sc);
        assert!(got.contains(&(0, 1, 0)), "a seção do olho sempre entra");
        assert!(got.contains(&(1, 1, 0)), "a parede entra na lista, mas não deixa passar");
        assert!(!got.contains(&(2, 1, 0)), "nada atrás da parede");
    }

    #[test]
    fn olho_em_bolsao_fechado_so_desenha_a_propria_secao() {
        let mut w = World::new(true);
        w.apply(ChunkData { x: 0, z: 0, full: true, mask: 1 << 1, sections: vec![(1, stone(true))], biomes: None });
        air_column(&mut w, 1, 0);
        let table = BlockTable::new();
        let vis = HashMap::new();
        // Olho no bolsão (bloco 5,5,5 da seção (0,1,0)): nenhuma face alcançada, só a seção.
        let c = cam(Vec3::new(5.5, 21.5, 5.5));
        let sc = Scene { world: &w, vis: &vis, table: &table, cam: &c, planes: [Vec4::W; 5], rear_view: false, spectator: false };
        let got = visible_sections(&sc);
        assert!(got.len() == 1 && got.contains(&(0, 1, 0)));
    }

    #[test]
    fn seccao_sem_dados_e_toda_visivel() {
        // Coluna carregada sem nenhuma seção (ar) e sem entrada em `vis`: a BFS atravessa.
        let mut w = World::new(true);
        air_column(&mut w, 0, 0);
        air_column(&mut w, 1, 0);
        let table = BlockTable::new();
        let vis = HashMap::new();
        let c = cam(Vec3::new(4.5, 20.5, 8.5));
        let sc = Scene { world: &w, vis: &vis, table: &table, cam: &c, planes: [Vec4::W; 5], rear_view: false, spectator: false };
        let got = visible_sections(&sc);
        assert!(got.contains(&(1, 1, 0)));
        assert!(got.contains(&(1, 0, 0)));
    }
}
