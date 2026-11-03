//! Chunks no formato de rede da 1.8 e o mundo carregado no cliente.

use std::collections::{HashMap, HashSet};

use mc_protocol::{Reader, Result};

use crate::mesh::{Padded, BORDER};

/// Seção 16x16x16. Índice = y << 8 | z << 4 | x.
#[derive(Clone)]
pub struct Section {
    /// `id << 4 | meta`.
    pub blocks: [u16; 4096],
    pub block_light: [u8; 2048],
    pub sky_light: [u8; 2048],
}

#[inline]
fn nibble(arr: &[u8; 2048], i: usize) -> u8 {
    (arr[i >> 1] >> ((i & 1) * 4)) & 0xF
}

impl Section {
    pub fn empty(sky: bool) -> Self {
        Self { blocks: [0; 4096], block_light: [0; 2048], sky_light: [if sky { 0xFF } else { 0 }; 2048] }
    }

    #[inline]
    pub fn index(x: i32, y: i32, z: i32) -> usize {
        ((y << 8) | (z << 4) | x) as usize
    }

    /// Luz empacotada: céu << 4 | bloco.
    #[inline]
    pub fn light(&self, i: usize) -> u8 {
        (nibble(&self.sky_light, i) << 4) | nibble(&self.block_light, i)
    }
}

/// Coluna recebida da rede.
pub struct ChunkData {
    pub x: i32,
    pub z: i32,
    /// Ground-up continuous: a coluna inteira foi enviada.
    pub full: bool,
    pub mask: u16,
    pub sections: Vec<(usize, Box<Section>)>,
    /// Biomas da coluna (z << 4 | x), só nos pacotes com a coluna inteira.
    pub biomes: Option<Box<[u8; 256]>>,
}

/// Lê os dados de uma coluna: todos os blocos, depois toda a luz de bloco,
/// depois toda a luz do céu (se houver) e por fim os biomas (se `full`).
fn parse_column(x: i32, z: i32, full: bool, mask: u16, sky: bool, r: &mut Reader) -> Result<ChunkData> {
    let ys: Vec<usize> = (0..16).filter(|i| mask & (1 << i) != 0).collect();
    let mut secs: Vec<Box<Section>> = ys.iter().map(|_| Box::new(Section::empty(sky))).collect();
    for s in secs.iter_mut() {
        let b = r.bytes(8192)?;
        for i in 0..4096 {
            s.blocks[i] = u16::from_le_bytes([b[2 * i], b[2 * i + 1]]);
        }
    }
    for s in secs.iter_mut() {
        s.block_light.copy_from_slice(r.bytes(2048)?);
    }
    if sky {
        for s in secs.iter_mut() {
            s.sky_light.copy_from_slice(r.bytes(2048)?);
        }
    }
    let mut biomes = None;
    if full {
        let mut b = Box::new([0u8; 256]);
        b.copy_from_slice(r.bytes(256)?);
        biomes = Some(b);
    }
    Ok(ChunkData { x, z, full, mask, sections: ys.into_iter().zip(secs).collect(), biomes })
}

/// Chunk Data (0x21).
pub fn parse_chunk_data(body: &[u8]) -> Result<ChunkData> {
    let mut r = Reader::new(body);
    let x = r.i32()?;
    let z = r.i32()?;
    let full = r.bool()?;
    let mask = r.u16()?;
    let size = r.varint()?.max(0) as usize;
    let n = mask.count_ones() as usize;
    // O pacote não diz se há luz do céu; dá para deduzir pelo tamanho.
    let without_sky = n * (8192 + 2048) + if full { 256 } else { 0 };
    let data = r.bytes(size)?;
    parse_column(x, z, full, mask, size > without_sky, &mut Reader::new(data))
}

/// Map Chunk Bulk (0x26).
pub fn parse_chunk_bulk(body: &[u8]) -> Result<Vec<ChunkData>> {
    let mut r = Reader::new(body);
    let sky = r.bool()?;
    let count = r.varint()?.max(0) as usize;
    let mut metas = Vec::with_capacity(count);
    for _ in 0..count {
        metas.push((r.i32()?, r.i32()?, r.u16()?));
    }
    metas.into_iter().map(|(x, z, mask)| parse_column(x, z, true, mask, sky, &mut r)).collect()
}

#[derive(Default)]
pub struct Column {
    pub sections: [Option<Box<Section>>; 16],
    /// Biomas (z << 4 | x); vazio até o servidor mandar a coluna inteira.
    pub biomes: Option<Box<[u8; 256]>>,
}

#[derive(Default)]
pub struct World {
    pub columns: HashMap<(i32, i32), Column>,
    pub has_sky: bool,
    /// Seções (cx, sy, cz) que precisam de malha nova.
    pub dirty: HashSet<(i32, i32, i32)>,
    /// Colunas descarregadas desde a última leitura.
    pub removed: Vec<(i32, i32)>,
    /// Muda quando os biomas de alguma coluna mudam (invalida caches de mistura de cor/céu).
    pub biome_rev: u32,
}

impl World {
    pub fn new(has_sky: bool) -> Self {
        Self { has_sky, ..Default::default() }
    }

    fn mark_present(&mut self, cx: i32, cz: i32) {
        if let Some(col) = self.columns.get(&(cx, cz)) {
            for (y, s) in col.sections.iter().enumerate() {
                if s.is_some() {
                    self.dirty.insert((cx, y as i32, cz));
                }
            }
        }
    }

    /// Recarga de recursos: toda seção presente precisa de malha nova.
    pub fn mark_all_dirty(&mut self) {
        let cols: Vec<(i32, i32)> = self.columns.keys().copied().collect();
        for (cx, cz) in cols {
            self.mark_present(cx, cz);
        }
    }

    /// `handleChunkData` (S21): coluna inteira sem nenhuma seção descarrega (`doPreChunk(x, z, false)`).
    pub fn apply(&mut self, c: ChunkData) {
        if c.full && c.mask == 0 {
            self.unload(c.x, c.z);
            return;
        }
        self.load(c);
    }

    /// `handleMapChunkBulk` (S26): sempre `doPreChunk(x, z, true)`, até a coluna só de ar (máscara 0) fica
    /// carregada; é nela que entram os blocos das pontes sobre o void.
    pub fn load(&mut self, c: ChunkData) {
        let col = self.columns.entry((c.x, c.z)).or_default();
        if c.full {
            col.sections = Default::default();
        }
        if c.biomes.is_some() {
            col.biomes = c.biomes;
            self.biome_rev = self.biome_rev.wrapping_add(1);
        }
        for (y, s) in c.sections {
            col.sections[y] = Some(s);
        }
        // A coluna inteira (seções vazias perdem a malha) e as bordas dos vizinhos.
        for y in 0..16 {
            if c.full || c.mask & (1 << y) != 0 {
                self.dirty.insert((c.x, y, c.z));
            }
        }
        for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            self.mark_present(c.x + dx, c.z + dz);
        }
    }

    /// `Chunk.getBiome` sem gerador: bioma da coluna, ou planície (1) enquanto o servidor não mandou
    /// a coluna inteira (e para o id 255).
    pub fn biome_at(&self, x: i32, z: i32) -> u8 {
        match self.columns.get(&(x >> 4, z >> 4)).and_then(|c| c.biomes.as_ref()) {
            Some(b) => match b[((z & 15) * 16 + (x & 15)) as usize] {
                255 => 1,
                id => id,
            },
            None => 1,
        }
    }

    /// `Chunk.getPrecipitationHeight`: `y + 1` do primeiro bloco, de cima para baixo, cujo material
    /// bloqueia movimento ou é líquido (`F_SOLID` do gerador é `blocksMovement`); -1 se não houver.
    /// Coluna não carregada vira -1 (o `EmptyChunk` do Java só tem ar).
    pub fn precipitation_height(&self, x: i32, z: i32) -> i32 {
        use crate::gen_shapes::{F_LIQUID, F_SOLID, FLAGS};
        let Some(col) = self.columns.get(&(x >> 4, z >> 4)) else { return -1 };
        // `getTopFilledSegment() + 15`: topo da última seção não vazia.
        let top = col.sections.iter().rposition(|s| s.is_some()).map_or(0, |i| i as i32 * 16);
        let mut y = top + 15;
        while y > 0 {
            let id = (self.block(x, y, z) >> 4) as usize;
            if id < FLAGS.len() && FLAGS[id] & (F_SOLID | F_LIQUID) != 0 {
                return y + 1;
            }
            y -= 1;
        }
        -1
    }

    pub fn unload(&mut self, cx: i32, cz: i32) {
        if self.columns.remove(&(cx, cz)).is_some() {
            self.removed.push((cx, cz));
            self.biome_rev = self.biome_rev.wrapping_add(1);
            self.dirty.retain(|k| (k.0, k.2) != (cx, cz));
            for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                self.mark_present(cx + dx, cz + dz);
            }
        }
    }

    pub fn is_loaded(&self, x: i32, z: i32) -> bool {
        self.columns.contains_key(&(x >> 4, z >> 4))
    }

    pub fn block(&self, x: i32, y: i32, z: i32) -> u16 {
        if !(0..256).contains(&y) {
            return 0;
        }
        self.columns
            .get(&(x >> 4, z >> 4))
            .and_then(|c| c.sections[(y >> 4) as usize].as_ref())
            .map_or(0, |s| s.blocks[Section::index(x & 15, y & 15, z & 15)])
    }

    pub fn set_block(&mut self, x: i32, y: i32, z: i32, state: u16) {
        if !(0..256).contains(&y) {
            return;
        }
        let sky = self.has_sky;
        let Some(col) = self.columns.get_mut(&(x >> 4, z >> 4)) else { return };
        let sec = &mut col.sections[(y >> 4) as usize];
        if sec.is_none() {
            if state == 0 {
                return;
            }
            *sec = Some(Box::new(Section::empty(sky)));
        }
        sec.as_mut().unwrap().blocks[Section::index(x & 15, y & 15, z & 15)] = state;

        let (cx, sy, cz) = (x >> 4, y >> 4, z >> 4);
        self.dirty.insert((cx, sy, cz));
        let (lx, ly, lz) = (x & 15, y & 15, z & 15);
        if lx == 0 { self.dirty.insert((cx - 1, sy, cz)); }
        if lx == 15 { self.dirty.insert((cx + 1, sy, cz)); }
        if lz == 0 { self.dirty.insert((cx, sy, cz - 1)); }
        if lz == 15 { self.dirty.insert((cx, sy, cz + 1)); }
        if ly == 0 && sy > 0 { self.dirty.insert((cx, sy - 1, cz)); }
        if ly == 15 && sy < 15 { self.dirty.insert((cx, sy + 1, cz)); }
    }

    /// Cópia da seção com `BORDER` blocos de borda para gerar malha fora da thread principal.
    /// `None` quando a seção não existe (não há nada para desenhar).
    pub fn snapshot(&self, cx: i32, sy: i32, cz: i32) -> Option<Padded> {
        if !(0..16).contains(&sy) {
            return None;
        }
        self.columns.get(&(cx, cz))?.sections[sy as usize].as_ref()?;
        let mut cols: [[Option<&Column>; 3]; 3] = Default::default();
        for dz in 0..3 {
            for dx in 0..3 {
                cols[dz][dx] = self.columns.get(&(cx + dx as i32 - 1, cz + dz as i32 - 1));
            }
        }
        let default_light = if self.has_sky { 0xF0 } else { 0 };
        let mut p = Padded::new(default_light);
        let split = |v: i32| -> (usize, i32) {
            if v < 0 {
                (0, v + 16)
            } else if v > 15 {
                (2, v - 16)
            } else {
                (1, v)
            }
        };
        for ly in -BORDER..16 + BORDER {
            let wy = sy * 16 + ly;
            if !(0..256).contains(&wy) {
                continue;
            }
            let (si, iy) = ((wy >> 4) as usize, wy & 15);
            for lz in -BORDER..16 + BORDER {
                let (cz_i, iz) = split(lz);
                for lx in -BORDER..16 + BORDER {
                    let (cx_i, ix) = split(lx);
                    let Some(col) = cols[cz_i][cx_i] else { continue };
                    if ly == 0 {
                        if let Some(b) = &col.biomes {
                            p.biomes[Padded::bidx(lx, lz)] = b[(iz << 4 | ix) as usize];
                        }
                    }
                    let Some(sec) = &col.sections[si] else { continue };
                    let i = Section::index(ix, iy, iz);
                    let pi = Padded::idx(lx, ly, lz);
                    p.blocks[pi] = sec.blocks[i];
                    p.light[pi] = sec.light(i);
                }
            }
        }
        Some(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_protocol::WriteExt;

    fn column_bytes(sections: usize, sky: bool, full: bool, block: u16) -> Vec<u8> {
        let mut d = Vec::new();
        for _ in 0..sections {
            for _ in 0..4096 {
                d.extend_from_slice(&block.to_le_bytes());
            }
        }
        d.extend(std::iter::repeat(0x77).take(sections * 2048));
        if sky {
            d.extend(std::iter::repeat(0xFF).take(sections * 2048));
        }
        if full {
            d.extend(std::iter::repeat(1).take(256));
        }
        d
    }

    #[test]
    fn chunk_data_with_and_without_sky() {
        for sky in [true, false] {
            let data = column_bytes(2, sky, true, 1 << 4);
            let mut p = Vec::new();
            p.extend_from_slice(&3i32.to_be_bytes());
            p.extend_from_slice(&(-2i32).to_be_bytes());
            p.put_bool(true);
            p.put_u16(0b101);
            p.put_varint(data.len() as i32);
            p.extend_from_slice(&data);
            let c = parse_chunk_data(&p).unwrap();
            assert_eq!((c.x, c.z, c.sections.len()), (3, -2, 2));
            assert_eq!(c.sections[1].0, 2);
            assert_eq!(c.sections[0].1.blocks[123], 1 << 4);
            let expected = if sky { 0xF7 } else { 0x07 };
            assert_eq!(c.sections[0].1.light(5), expected);
        }
    }

    #[test]
    fn bulk_and_world_lookup() {
        let mut p = Vec::new();
        p.put_bool(true);
        p.put_varint(2);
        for (x, z) in [(0i32, 0i32), (-1, 0)] {
            p.extend_from_slice(&x.to_be_bytes());
            p.extend_from_slice(&z.to_be_bytes());
            p.put_u16(1);
        }
        p.extend(column_bytes(1, true, true, 2 << 4));
        p.extend(column_bytes(1, true, true, 3 << 4));
        let chunks = parse_chunk_bulk(&p).unwrap();
        let mut w = World::new(true);
        for c in chunks {
            w.load(c);
        }
        assert_eq!(w.block(5, 3, 5) >> 4, 2);
        assert_eq!(w.block(-5, 3, 5) >> 4, 3);
        assert_eq!(w.block(5, 20, 5), 0);
        w.set_block(5, 20, 5, 7 << 4);
        assert_eq!(w.block(5, 20, 5) >> 4, 7);
        let snap = w.snapshot(0, 0, 0).unwrap();
        assert_eq!(snap.block(-1, 0, 0) >> 4, 3);
        assert_eq!(snap.block(16, 0, 0), 0);
    }

    #[test]
    fn empty_bulk_column_stays_loaded() {
        // S26 com máscara 0 (coluna só de ar, como o void do bedwars): carregada, e a ponte colocada nela fica.
        let mut p = Vec::new();
        p.put_bool(true);
        p.put_varint(1);
        p.extend_from_slice(&3i32.to_be_bytes());
        p.extend_from_slice(&0i32.to_be_bytes());
        p.put_u16(0);
        p.extend(std::iter::repeat_n(1u8, 256));
        let mut w = World::new(true);
        for c in parse_chunk_bulk(&p).unwrap() {
            w.load(c);
        }
        assert!(w.is_loaded(50, 5));
        w.set_block(50, 64, 5, 35 << 4);
        assert_eq!(w.block(50, 64, 5) >> 4, 35);
        // S21 inteiro com máscara 0 descarrega.
        w.apply(ChunkData { x: 3, z: 0, full: true, mask: 0, sections: vec![], biomes: None });
        assert!(!w.is_loaded(50, 5));
    }

    #[test]
    fn biomes_reach_the_padded_snapshot() {
        let mut w = World::new(true);
        for (cx, id) in [(0, 6u8), (1, 2u8)] {
            let mut b = Box::new([0u8; 256]);
            for z in 0..16 {
                for x in 0..16 {
                    b[z << 4 | x] = id + if x == 3 { 30 } else { 0 };
                }
            }
            w.apply(ChunkData { x: cx, z: 0, full: true, mask: 1, sections: vec![(0, Box::new(Section::empty(true)))], biomes: Some(b) });
        }
        let p = w.snapshot(0, 0, 0).unwrap();
        assert_eq!(p.biome(0, 0), 6);
        assert_eq!(p.biome(3, 5), 36);
        // Coluna vizinha (x = 16..18 da seção) e coluna ausente (z = -1).
        assert_eq!(p.biome(16, 0), 2);
        assert_eq!(p.biome(0, -1), 255);
    }
}
