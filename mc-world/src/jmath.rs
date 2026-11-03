//! Matemática do Java usada pelo céu: `java.util.Random`, `MathHelper` (tabela de seno,
//! `hsvToRGB`) e `NoiseGeneratorSimplex`/`NoiseGeneratorPerlin` (temperatura por altura).

use std::sync::OnceLock;

/// `java.util.Random` (LCG de 48 bits).
pub struct JRandom {
    seed: i64,
    next_gaussian: Option<f64>,
}

const MULT: i64 = 0x5DEECE66D;
const MASK: i64 = (1 << 48) - 1;

impl JRandom {
    pub fn new(seed: i64) -> Self {
        let mut r = Self { seed: 0, next_gaussian: None };
        r.set_seed(seed);
        r
    }

    pub fn set_seed(&mut self, seed: i64) {
        self.seed = (seed ^ MULT) & MASK;
        self.next_gaussian = None;
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.seed = (self.seed.wrapping_mul(MULT).wrapping_add(0xB)) & MASK;
        (self.seed >> (48 - bits)) as i32
    }

    pub fn next_int(&mut self, bound: i32) -> i32 {
        if bound & (-bound) == bound {
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

    pub fn next_float(&mut self) -> f32 {
        self.next(24) as f32 / (1 << 24) as f32
    }

    pub fn next_double(&mut self) -> f64 {
        (((self.next(26) as i64) << 27) + self.next(27) as i64) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    pub fn next_gaussian(&mut self) -> f64 {
        if let Some(g) = self.next_gaussian.take() {
            return g;
        }
        loop {
            let v1 = 2.0 * self.next_double() - 1.0;
            let v2 = 2.0 * self.next_double() - 1.0;
            let s = v1 * v1 + v2 * v2;
            if s < 1.0 && s != 0.0 {
                let m = (-2.0 * s.ln() / s).sqrt();
                self.next_gaussian = Some(v2 * m);
                return v1 * m;
            }
        }
    }
}

fn sin_table() -> &'static [f32; 65536] {
    static T: OnceLock<Box<[f32; 65536]>> = OnceLock::new();
    T.get_or_init(|| {
        let mut t = Box::new([0f32; 65536]);
        for (i, v) in t.iter_mut().enumerate() {
            *v = (i as f64 * std::f64::consts::PI * 2.0 / 65536.0).sin() as f32;
        }
        t
    })
}

/// `MathHelper.sin`.
pub fn sin(x: f32) -> f32 {
    sin_table()[((x * 10430.378f32) as i32 & 0xFFFF) as usize]
}

/// `MathHelper.cos`.
pub fn cos(x: f32) -> f32 {
    sin_table()[((x * 10430.378f32 + 16384.0f32) as i32 & 0xFFFF) as usize]
}

/// `MathHelper.clamp_float`.
pub fn clamp(v: f32, lo: f32, hi: f32) -> f32 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

/// `MathHelper.hsvToRGB` (0xRRGGBB).
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> i32 {
    let i = (h * 6.0) as i32 % 6;
    let f = h * 6.0 - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match i {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    let c = |x: f32| ((x * 255.0) as i32).clamp(0, 255);
    c(r) << 16 | c(g) << 8 | c(b)
}

const GRAD: [[i32; 3]; 12] = [[1, 1, 0], [-1, 1, 0], [1, -1, 0], [-1, -1, 0], [1, 0, 1], [-1, 0, 1], [1, 0, -1], [-1, 0, -1], [0, 1, 1], [0, -1, 1], [0, 1, -1], [0, -1, -1]];

/// `NoiseGeneratorSimplex` (só o ruído 2D).
pub struct Simplex {
    perm: [i32; 512],
}

impl Simplex {
    pub fn new(rng: &mut JRandom) -> Self {
        // Os três deslocamentos consomem o gerador, mas o 2D não os usa.
        rng.next_double();
        rng.next_double();
        rng.next_double();
        let mut perm = [0i32; 512];
        for (i, p) in perm.iter_mut().take(256).enumerate() {
            *p = i as i32;
        }
        for i in 0..256usize {
            let j = (rng.next_int(256 - i as i32) as usize) + i;
            perm.swap(i, j);
            perm[i + 256] = perm[i];
        }
        Self { perm }
    }

    fn floor(v: f64) -> i32 {
        if v > 0.0 {
            v as i32
        } else {
            v as i32 - 1
        }
    }

    fn dot(g: [i32; 3], x: f64, y: f64) -> f64 {
        g[0] as f64 * x + g[1] as f64 * y
    }

    pub fn noise(&self, xin: f64, yin: f64) -> f64 {
        let sqrt3 = 3.0f64.sqrt();
        let f2 = 0.5 * (sqrt3 - 1.0);
        let s = (xin + yin) * f2;
        let i = Self::floor(xin + s);
        let j = Self::floor(yin + s);
        let g2 = (3.0 - sqrt3) / 6.0;
        let t = (i + j) as f64 * g2;
        let x0 = xin - (i as f64 - t);
        let y0 = yin - (j as f64 - t);
        let (i1, j1) = if x0 > y0 { (1, 0) } else { (0, 1) };
        let x1 = x0 - i1 as f64 + g2;
        let y1 = y0 - j1 as f64 + g2;
        let x2 = x0 - 1.0 + 2.0 * g2;
        let y2 = y0 - 1.0 + 2.0 * g2;
        let ii = (i & 255) as usize;
        let jj = (j & 255) as usize;
        let p = &self.perm;
        let gi0 = (p[ii + p[jj] as usize] % 12) as usize;
        let gi1 = (p[ii + i1 as usize + p[jj + j1 as usize] as usize] % 12) as usize;
        let gi2 = (p[ii + 1 + p[jj + 1] as usize] % 12) as usize;
        let corner = |t0: f64, g: usize, x: f64, y: f64| {
            if t0 < 0.0 {
                0.0
            } else {
                let t0 = t0 * t0;
                t0 * t0 * Self::dot(GRAD[g], x, y)
            }
        };
        let n0 = corner(0.5 - x0 * x0 - y0 * y0, gi0, x0, y0);
        let n1 = corner(0.5 - x1 * x1 - y1 * y1, gi1, x1, y1);
        let n2 = corner(0.5 - x2 * x2 - y2 * y2, gi2, x2, y2);
        70.0 * (n0 + n1 + n2)
    }
}

/// `NoiseGeneratorPerlin(new Random(seed), 1)`, `func_151601_a`.
pub struct Perlin1(Simplex);

impl Perlin1 {
    pub fn new(seed: i64) -> Self {
        Self(Simplex::new(&mut JRandom::new(seed)))
    }

    pub fn noise(&self, x: f64, y: f64) -> f64 {
        0.0 + self.0.noise(x, y) / 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_matches_java() {
        // Valores de java.util.Random(42): nextInt() = -1170105035, depois 234785527.
        let mut r = JRandom::new(42);
        assert_eq!(r.next(32), -1170105035);
        assert_eq!(r.next(32), 234785527);
        let mut r = JRandom::new(0);
        assert_eq!(r.next_int(10), 0);
        assert_eq!(r.next_int(10), 8);
        let mut r = JRandom::new(0);
        assert!((r.next_gaussian() - 0.8025330637390305).abs() < 1e-12);
    }

    #[test]
    fn trig_table() {
        assert_eq!(sin(0.0), 0.0);
        assert!((cos(0.0) - 1.0).abs() < 1e-6);
        assert!((sin(std::f32::consts::FRAC_PI_2) - 1.0).abs() < 1e-3);
    }
}
