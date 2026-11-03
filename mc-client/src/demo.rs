//! Mundo local de demonstração (`MINE_RS_DEMO=1`), para prints sem entrar em servidor:
//! gramado, piscina com fluxo, lava, folhas e dois biomas lado a lado (planície e pântano).

use mc_world::chunk::{ChunkData, Section, World};

fn put(w: &mut World, x: i32, y: i32, z: i32, id: u16, meta: u16) {
    w.set_block(x, y, z, id << 4 | meta);
}

pub fn world() -> World {
    let mut w = World::new(true);
    for cz in -2..2 {
        for cx in -2..2 {
            let mut secs = Vec::new();
            for sy in 0..4 {
                let mut s = Section::empty(true);
                for y in 0..16 {
                    let wy = sy * 16 + y;
                    // 1 = pedra, 3 = terra, 2 = grama.
                    let id: u16 = match wy {
                        0..=58 => 1,
                        59..=62 => 3,
                        63 => 2,
                        _ => 0,
                    };
                    for z in 0..16 {
                        for x in 0..16 {
                            s.blocks[Section::index(x, y, z)] = id << 4;
                        }
                    }
                }
                secs.push((sy as usize, Box::new(s)));
            }
            // x < 0 planície (1), x >= 0 pântano (6).
            let biome = if cx < 0 { 1 } else { 6 };
            w.apply(ChunkData { x: cx, z: cz, full: true, mask: 0b1111, sections: secs, biomes: Some(Box::new([biome; 256])) });
        }
    }
    // Piscina de água (afunda 2 blocos) nos dois biomas, com uma cascata de níveis de fluxo.
    for x in -12..12 {
        for z in -3..4 {
            for y in 61..64 {
                put(&mut w, x, y, z, 0, 0);
            }
            put(&mut w, x, 61, z, 9, 0);
            put(&mut w, x, 62, z, 9, 0);
        }
    }
    // Água fluindo: níveis 1..7 numa borda, caindo da plataforma elevada.
    for i in 0..8 {
        put(&mut w, -3 + i, 63, 8, 9, 0);
    }
    for i in 0..7 {
        put(&mut w, -3 + i, 63, 9, 9, (i + 1) as u16);
        put(&mut w, -3 + i, 63, 10, 0, 0);
    }
    // Lava e pedra por cima.
    for x in 4..8 {
        for z in -8..-4 {
            put(&mut w, x, 63, z, 11, 0);
        }
    }
    // Árvore de folhas (carvalho, abeto, bétula) e capim.
    for (i, meta) in [0u16, 1, 2].into_iter().enumerate() {
        let x = -8 + i as i32 * 5;
        for y in 64..68 {
            put(&mut w, x, y, -8, 17, meta);
        }
        for dx in -1..=1 {
            for dz in -1..=1 {
                put(&mut w, x + dx, 68, -8 + dz, 18, meta);
            }
        }
    }
    for x in -10..10 {
        put(&mut w, x, 64, 6, 31, 1);
    }
    w
}

#[cfg(test)]
mod tests {
    #[test]
    fn demo_world_has_pool_and_cascade() {
        let w = super::world();
        assert_eq!(w.block(0, 62, 0), 9 << 4);
        assert_eq!(w.block(-3, 63, 8), 9 << 4);
        assert_eq!(w.block(-2, 63, 9), (9 << 4) | 2);
        assert_eq!(w.block(-3, 63, 10), 0);
    }
}
