//! Grafo de visibilidade de uma seção 16x16x16 (`VisGraph` e `SetVisibility` do jogo).
//!
//! Um caminho de ar liga duas faces da seção; o `RenderGlobal.setupTerrain` só
//! atravessa uma seção por pares de faces ligados, então seções escondidas atrás
//! de um bloco inteiro não entram na lista de desenho. Faces na ordem do `EnumFacing`.

use std::collections::VecDeque;

use crate::blocks::BlockTable;

pub const DOWN: usize = 0;
pub const UP: usize = 1;
pub const NORTH: usize = 2;
pub const SOUTH: usize = 3;
pub const WEST: usize = 4;
pub const EAST: usize = 5;

/// Face oposta: os pares são 0-1 (baixo, cima), 2-3 (norte, sul) e 4-5 (oeste, leste).
#[inline]
pub fn opposite(face: usize) -> usize {
    face ^ 1
}

/// Bloco que bloqueia a visão no grafo: cubo opaco inteiro (`isOpaqueCube`).
#[inline]
pub fn is_opaque(t: &BlockTable, state: u16) -> bool {
    t.get(state).full_opaque
}

/// Pares de faces ligados (6x6 bits, `SetVisibility`): o bit `a + b * 6` vale para o par (a, b).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SetVisibility(u64);

impl SetVisibility {
    /// Todos os pares ligados, ou nenhum (`setAllVisible`).
    pub fn all(visible: bool) -> Self {
        Self(if visible { (1 << 36) - 1 } else { 0 })
    }

    /// Liga ou desliga o par (a, b) nos dois sentidos.
    pub fn set(&mut self, a: usize, b: usize, visible: bool) {
        self.put(a + b * 6, visible);
        self.put(b + a * 6, visible);
    }

    fn put(&mut self, bit: usize, on: bool) {
        if on {
            self.0 |= 1 << bit;
        } else {
            self.0 &= !(1 << bit);
        }
    }

    /// Liga todos os pares de `faces` (máscara de bits), inclusive cada face com ela mesma (`setManyVisible`).
    pub fn set_many(&mut self, faces: u8) {
        for a in 0..6 {
            for b in 0..6 {
                if faces >> a & 1 != 0 && faces >> b & 1 != 0 {
                    self.set(a, b, true);
                }
            }
        }
    }

    pub fn is_visible(&self, a: usize, b: usize) -> bool {
        self.0 >> (a + b * 6) & 1 != 0
    }
}

/// Blocos opacos de uma seção e o flood fill pelo ar, como o `VisGraph` do jogo.
pub struct VisGraph {
    /// Opaco ou já visitado (o `BitSet` do jogo). Índice = x | z << 4 | y << 8.
    seen: Vec<bool>,
    opaque: usize,
}

/// Índice de um bloco local (x, y, z em 0..16), com o mesmo layout do jogo.
#[inline]
fn index(x: i32, y: i32, z: i32) -> usize {
    (x | z << 4 | y << 8) as usize
}

/// Faces da seção em que o bloco (índice) está na borda.
fn edge_faces(i: usize) -> u8 {
    let (x, y, z) = (i & 15, i >> 8 & 15, i >> 4 & 15);
    let mut f = 0;
    if x == 0 {
        f |= 1 << WEST;
    } else if x == 15 {
        f |= 1 << EAST;
    }
    if y == 0 {
        f |= 1 << DOWN;
    } else if y == 15 {
        f |= 1 << UP;
    }
    if z == 0 {
        f |= 1 << NORTH;
    } else if z == 15 {
        f |= 1 << SOUTH;
    }
    f
}

/// Vizinho do bloco pela face, dentro da seção (`None` na borda).
fn neighbor(i: usize, face: usize) -> Option<usize> {
    let (x, y, z) = (i & 15, i >> 8 & 15, i >> 4 & 15);
    match face {
        DOWN => (y > 0).then(|| i - 256),
        UP => (y < 15).then(|| i + 256),
        NORTH => (z > 0).then(|| i - 16),
        SOUTH => (z < 15).then(|| i + 16),
        WEST => (x > 0).then(|| i - 1),
        _ => (x < 15).then(|| i + 1),
    }
}

/// Índices dos blocos da borda, na ordem do jogo (x, depois y, depois z).
fn boundary() -> impl Iterator<Item = usize> {
    (0..16)
        .flat_map(|x| (0..16).flat_map(move |y| (0..16).map(move |z| (x, y, z))))
        .filter(|&(x, y, z)| [x, y, z].iter().any(|&c| c == 0 || c == 15))
        .map(|(x, y, z)| index(x, y, z))
}

impl VisGraph {
    pub fn new() -> Self {
        Self { seen: vec![false; 4096], opaque: 0 }
    }

    /// Marca o bloco local (x, y, z) como opaco (`func_178606_a`).
    pub fn set_opaque(&mut self, x: i32, y: i32, z: i32) {
        let i = index(x, y, z);
        if !self.seen[i] {
            self.seen[i] = true;
            self.opaque += 1;
        }
    }

    /// Visibilidade entre as faces (`computeVisibility`). Com menos de 256 blocos opacos, tudo fica visível.
    pub fn compute_visibility(&mut self) -> SetVisibility {
        if self.opaque < 256 {
            return SetVisibility::all(true);
        }
        if self.opaque == 4096 {
            return SetVisibility::all(false);
        }
        let mut out = SetVisibility::default();
        for i in boundary() {
            if !self.seen[i] {
                let faces = self.flood(i);
                out.set_many(faces);
            }
        }
        out
    }

    /// Faces que o ar a partir do bloco (x, y, z) alcança (`func_178609_b`), mesmo que o bloco seja opaco.
    pub fn facings_from(&mut self, x: i32, y: i32, z: i32) -> u8 {
        self.flood(index(x, y, z))
    }

    /// Flood fill em largura a partir de `start`, pelos blocos não visitados (`func_178604_a`).
    fn flood(&mut self, start: usize) -> u8 {
        let mut faces = 0u8;
        let mut queue = VecDeque::from([start]);
        self.seen[start] = true;
        while let Some(i) = queue.pop_front() {
            faces |= edge_faces(i);
            for face in 0..6 {
                if let Some(n) = neighbor(i, face) {
                    if !self.seen[n] {
                        self.seen[n] = true;
                        queue.push_back(n);
                    }
                }
            }
        }
        faces
    }
}

impl Default for VisGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parede de 256 blocos opacos no plano x = 8: separa a seção em um lado oeste e um leste.
    fn wall() -> VisGraph {
        let mut g = VisGraph::new();
        for y in 0..16 {
            for z in 0..16 {
                g.set_opaque(8, y, z);
            }
        }
        g
    }

    #[test]
    fn parede_separa_o_lado_oeste_do_leste() {
        let v = wall().compute_visibility();
        // Cada lado liga as suas faces (inclusive com baixo, cima, norte e sul, que os dois tocam).
        assert!(v.is_visible(WEST, WEST) && v.is_visible(WEST, UP) && v.is_visible(WEST, SOUTH));
        assert!(v.is_visible(EAST, EAST) && v.is_visible(EAST, NORTH) && v.is_visible(EAST, DOWN));
        assert!(!v.is_visible(WEST, EAST), "a parede corta o caminho entre oeste e leste");
    }

    #[test]
    fn faces_a_partir_do_olho() {
        let mut g = wall();
        let oeste = 1 << WEST | 1 << UP | 1 << DOWN | 1 << NORTH | 1 << SOUTH;
        assert_eq!(g.facings_from(2, 5, 5), oeste);
        let mut g = wall();
        let leste = 1 << EAST | 1 << UP | 1 << DOWN | 1 << NORTH | 1 << SOUTH;
        assert_eq!(g.facings_from(12, 5, 5), leste);
    }

    #[test]
    fn poucos_opacos_deixam_tudo_visivel() {
        let mut g = VisGraph::new();
        g.set_opaque(3, 3, 3);
        assert_eq!(g.compute_visibility(), SetVisibility::all(true));
    }

    #[test]
    fn bolsao_fechado_nao_liga_faces() {
        // Tudo opaco menos dois blocos no meio: o ar não chega à borda.
        let mut g = VisGraph::new();
        for i in 0..4096 {
            let (x, y, z) = (i & 15, i >> 8 & 15, i >> 4 & 15);
            if !((x == 5 || x == 6) && y == 5 && z == 5) {
                g.set_opaque(x, y, z);
            }
        }
        assert_eq!(g.compute_visibility(), SetVisibility::default());
    }

    #[test]
    fn cheia_de_opacos_nao_liga_nada() {
        let mut g = VisGraph::new();
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    g.set_opaque(x, y, z);
                }
            }
        }
        assert_eq!(g.compute_visibility(), SetVisibility::all(false));
    }
}
