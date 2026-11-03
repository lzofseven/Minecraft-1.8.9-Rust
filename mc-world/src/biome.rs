//! Biomas e cores por bioma: `BiomeGenBase`, `ColorizerGrass`/`ColorizerFoliage` e a média 3x3 de
//! `BiomeColorHelper`. A tabela de biomas é gerada por `scripts/gen_biomes/gen.sh`.

use std::sync::OnceLock;

use crate::gen_biomes::BIOMES;

/// `java.util.Random`.
pub struct JavaRandom(i64);

impl JavaRandom {
    pub fn new(seed: i64) -> Self {
        Self((seed ^ 0x5DEECE66D) & ((1 << 48) - 1))
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.0 = (self.0.wrapping_mul(0x5DEECE66D).wrapping_add(0xB)) & ((1 << 48) - 1);
        (self.0 >> (48 - bits)) as i32
    }

    pub fn next_int(&mut self, bound: i32) -> i32 {
        if bound & (bound - 1) == 0 {
            return ((bound as i64 * self.next(31) as i64) >> 31) as i32;
        }
        loop {
            let bits = self.next(31);
            let val = bits % bound;
            if bits.wrapping_sub(val).wrapping_add(bound - 1) >= 0 {
                return val;
            }
        }
    }

    pub fn next_double(&mut self) -> f64 {
        let hi = (self.next(26) as i64) << 27;
        (hi + self.next(27) as i64) as f64 * 1.110_223_024_625_156_5E-16
    }
}

const GRAD3: [[i32; 3]; 12] =
    [[1, 1, 0], [-1, 1, 0], [1, -1, 0], [-1, -1, 0], [1, 0, 1], [-1, 0, 1], [1, 0, -1], [-1, 0, -1], [0, 1, 1], [0, -1, 1], [0, 1, -1], [0, -1, -1]];

/// `NoiseGeneratorSimplex`.
pub struct Simplex {
    perm: [i32; 512],
}

impl Simplex {
    pub fn new(r: &mut JavaRandom) -> Self {
        // Os três deslocamentos são sorteados mesmo sem uso: consomem o gerador.
        r.next_double();
        r.next_double();
        r.next_double();
        let mut perm = [0i32; 512];
        for (i, p) in perm.iter_mut().take(256).enumerate() {
            *p = i as i32;
        }
        for i in 0..256 {
            let j = (r.next_int(256 - i as i32) + i as i32) as usize;
            perm.swap(i, j);
            perm[i + 256] = perm[i];
        }
        Self { perm }
    }

    fn fastfloor(v: f64) -> i32 {
        if v > 0.0 {
            v as i32
        } else {
            v as i32 - 1
        }
    }

    pub fn noise(&self, x: f64, y: f64) -> f64 {
        let f2 = 0.5 * (3f64.sqrt() - 1.0);
        let s = (x + y) * f2;
        let i = Self::fastfloor(x + s);
        let j = Self::fastfloor(y + s);
        let g2 = (3.0 - 3f64.sqrt()) / 6.0;
        let t = (i + j) as f64 * g2;
        let x0 = x - (i as f64 - t);
        let y0 = y - (j as f64 - t);
        let (i1, j1) = if x0 > y0 { (1, 0) } else { (0, 1) };
        let x1 = x0 - i1 as f64 + g2;
        let y1 = y0 - j1 as f64 + g2;
        let x2 = x0 - 1.0 + 2.0 * g2;
        let y2 = y0 - 1.0 + 2.0 * g2;
        let (ii, jj) = ((i & 255) as usize, (j & 255) as usize);
        let p = &self.perm;
        let gi0 = (p[ii + p[jj] as usize] % 12) as usize;
        let gi1 = (p[ii + i1 as usize + p[jj + j1 as usize] as usize] % 12) as usize;
        let gi2 = (p[ii + 1 + p[jj + 1] as usize] % 12) as usize;
        let corner = |t0: f64, g: usize, x: f64, y: f64| {
            if t0 < 0.0 {
                0.0
            } else {
                let t0 = t0 * t0;
                t0 * t0 * (GRAD3[g][0] as f64 * x + GRAD3[g][1] as f64 * y)
            }
        };
        let n0 = corner(0.5 - x0 * x0 - y0 * y0, gi0, x0, y0);
        let n1 = corner(0.5 - x1 * x1 - y1 * y1, gi1, x1, y1);
        let n2 = corner(0.5 - x2 * x2 - y2 * y2, gi2, x2, y2);
        70.0 * (n0 + n1 + n2)
    }
}

/// `NoiseGeneratorPerlin` (camadas de simplex).
pub struct Perlin(Vec<Simplex>);

impl Perlin {
    pub fn new(r: &mut JavaRandom, octaves: usize) -> Self {
        Self((0..octaves).map(|_| Simplex::new(r)).collect())
    }

    pub fn noise(&self, x: f64, y: f64) -> f64 {
        let (mut sum, mut f) = (0.0, 1.0);
        for s in &self.0 {
            sum += s.noise(x * f, y * f) / f;
            f /= 2.0;
        }
        sum
    }
}

/// Ruídos estáticos de `BiomeGenBase`: `temperatureNoise` (seed 1234) e `GRASS_COLOR_NOISE` (2345).
fn noises() -> &'static (Perlin, Perlin) {
    static N: OnceLock<(Perlin, Perlin)> = OnceLock::new();
    N.get_or_init(|| (Perlin::new(&mut JavaRandom::new(1234), 1), Perlin::new(&mut JavaRandom::new(2345), 1)))
}

pub const PLAINS: u8 = 1;

pub struct Biome {
    pub id: u8,
    pub name: &'static str,
    pub temperature: f32,
    pub rainfall: f32,
    pub water_color: u32,
    /// 0 padrão, 1 pântano, 2 mesa, 3 floresta escura.
    pub kind: u8,
    /// Bioma-base de `BiomeGenMutated` (as cores vêm dele).
    pub delegate: Option<u8>,
}

/// `BiomeGenBase.getBiome`; id sem bioma vira planície (como `Chunk.getBiome`).
pub fn biome(id: u8) -> Biome {
    let find = |id: u8| BIOMES.iter().find(|b| b.0 == id);
    let b = find(id).or_else(|| find(PLAINS)).unwrap();
    Biome { id: b.0, name: b.1, temperature: b.2, rainfall: b.3, water_color: b.4, kind: b.5, delegate: (b.6 >= 0).then_some(b.6 as u8) }
}

/// `BiomeGenBase.getFloatTemperature(pos)`.
pub fn float_temperature(b: &Biome, x: i32, y: i32, z: i32) -> f32 {
    if y > 64 {
        let f = (noises().0.noise(x as f64 / 8.0, z as f64 / 8.0) * 4.0) as f32;
        b.temperature - (f + y as f32 - 64.0) * 0.05 / 30.0
    } else {
        b.temperature
    }
}

/// `colormap/grass.png` e `colormap/foliage.png` (256x256, RGB).
#[derive(Clone)]
pub struct Colormaps {
    pub grass: Vec<u32>,
    pub foliage: Vec<u32>,
}

fn colormap_png(bytes: &[u8]) -> Option<Vec<u32>> {
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok()?.to_rgba8();
    (img.width() == 256 && img.height() == 256).then(|| img.pixels().map(|p| (p[0] as u32) << 16 | (p[1] as u32) << 8 | p[2] as u32).collect())
}

/// `ColorizerGrass.getGrassColor` / `ColorizerFoliage.getFoliageColor`.
fn colorize(buf: &[u32], t: f64, r: f64) -> u32 {
    let r = r * t;
    let x = ((1.0 - t) * 255.0) as usize;
    let y = ((1.0 - r) * 255.0) as usize;
    buf.get(y << 8 | x).copied().unwrap_or(0xFFFFFF)
}

impl Colormaps {
    pub fn load(load: &mut dyn FnMut(&str) -> Option<Vec<u8>>) -> Option<Self> {
        let grass = colormap_png(&load("assets/minecraft/textures/colormap/grass.png")?)?;
        let foliage = colormap_png(&load("assets/minecraft/textures/colormap/foliage.png")?)?;
        Some(Self { grass, foliage })
    }

    fn clamped(b: &Biome, x: i32, y: i32, z: i32) -> (f64, f64) {
        (float_temperature(b, x, y, z).clamp(0.0, 1.0) as f64, b.rainfall.clamp(0.0, 1.0) as f64)
    }

    /// `BiomeGenBase.getGrassColorAtPos` (RGB).
    pub fn grass_at(&self, b: &Biome, x: i32, y: i32, z: i32) -> u32 {
        let b = &biome(b.delegate.unwrap_or(b.id));
        match b.kind {
            1 => {
                if noises().1.noise(x as f64 * 0.0225, z as f64 * 0.0225) < -0.1 {
                    5011004
                } else {
                    6975545
                }
            }
            2 => 9470285,
            k => {
                let (t, r) = Self::clamped(b, x, y, z);
                let c = colorize(&self.grass, t, r);
                if k == 3 {
                    ((c & 16711422) + 2634762) >> 1
                } else {
                    c
                }
            }
        }
    }

    /// `BiomeGenBase.getFoliageColorAtPos` (RGB).
    pub fn foliage_at(&self, b: &Biome, x: i32, y: i32, z: i32) -> u32 {
        let b = &biome(b.delegate.unwrap_or(b.id));
        match b.kind {
            1 => 6975545,
            2 => 10387789,
            _ => {
                let (t, r) = Self::clamped(b, x, y, z);
                colorize(&self.foliage, t, r)
            }
        }
    }
}

/// `BiomeColorHelper.getColorAtPos`: média 3x3 (em x e z) da cor de cada bioma.
pub fn average_3x3(biome_at: &dyn Fn(i32, i32) -> u8, x: i32, z: i32, color: &dyn Fn(&Biome, i32, i32) -> u32) -> u32 {
    let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
    for dz in -1..=1 {
        for dx in -1..=1 {
            let (px, pz) = (x + dx, z + dz);
            let c = color(&biome(biome_at(px, pz)), px, pz);
            r += (c & 0xFF0000) >> 16;
            g += (c & 0xFF00) >> 8;
            b += c & 0xFF;
        }
    }
    (r / 9 & 255) << 16 | (g / 9 & 255) << 8 | b / 9 & 255
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::jar_tests::jar_loader;

    #[test]
    fn java_random_matches_known_values() {
        // Valores de `new Random(42)` medidos no Java 21: 30 63 48, nextInt(256) = 12, nextDouble = 0.30871945533265976.
        let mut r = JavaRandom::new(42);
        assert_eq!([r.next_int(100), r.next_int(100), r.next_int(100), r.next_int(256)], [30, 63, 48, 12]);
        assert_eq!(r.next_double(), 0.30871945533265976);
    }

    #[test]
    fn colors_match_java_vectors() {
        let Some(mut load) = jar_loader() else { return };
        let maps = Colormaps::load(&mut load).expect("colormaps do jar");
        let (mut n, mut bad) = (0, Vec::new());
        for line in include_str!("gen_biome_vectors.txt").lines().filter(|l| !l.starts_with('#')) {
            let v: Vec<i64> = line.split(' ').map(|s| s.parse().unwrap()).collect();
            let b = biome(v[0] as u8);
            let (x, y, z) = (v[1] as i32, v[2] as i32, v[3] as i32);
            let (g, f) = (maps.grass_at(&b, x, y, z), maps.foliage_at(&b, x, y, z));
            n += 1;
            if g as i64 != v[4] || f as i64 != v[5] {
                bad.push(format!("{} {x} {y} {z}: grama {g} (java {}), folhagem {f} (java {})", b.name, v[4], v[5]));
            }
        }
        println!("{n} vetores do Java conferidos, {} divergentes", bad.len());
        assert!(n > 700 && bad.is_empty(), "{:?}", &bad[..bad.len().min(5)]);
    }

    #[test]
    fn known_biomes() {
        let Some(mut load) = jar_loader() else { return };
        let maps = Colormaps::load(&mut load).unwrap();
        // Planície ao nível do mar: 0.8/0.4 -> 0x91BD59 (a cor "fixa" que o port usava).
        assert_eq!(maps.grass_at(&biome(1), 0, 63, 0), 0x91BD59);
        assert_eq!(maps.foliage_at(&biome(1), 0, 63, 0), 0x77AB2F);
        assert_eq!(biome(6).water_color, 14745518);
        assert_eq!(biome(255).name, "Plains");
    }

    #[test]
    fn average_of_uniform_biome_is_that_color() {
        let c = average_3x3(&|_, _| 1, 10, 10, &|_, _, _| 0x91BD59);
        assert_eq!(c, 0x91BD59);
        // Em x = 10 a coluna 9 é planície e as colunas 10 e 11 são o outro bioma: (0x10 * 3 + 0x20 * 6) / 9 = 0x1A.
        let c = average_3x3(&|x, _| if x < 10 { 1 } else { 2 }, 10, 10, &|b, _, _| if b.id == 1 { 0x101010 } else { 0x202020 });
        assert_eq!(c, 0x1A1A1A);
    }
}
