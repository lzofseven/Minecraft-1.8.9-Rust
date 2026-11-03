//! `net.minecraft.util.MathHelper`: seno/cosseno por tabela e `atan2` aproximado, como no jogo.

use std::sync::OnceLock;

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

/// Tabelas de `atan2`: `asin(i / 256)` e seu cosseno.
fn atan_tables() -> &'static ([f64; 257], [f64; 257]) {
    static T: OnceLock<([f64; 257], [f64; 257])> = OnceLock::new();
    T.get_or_init(|| {
        let (mut asin, mut cos) = ([0.0; 257], [0.0; 257]);
        for i in 0..257 {
            let a = (i as f64 / 256.0).asin();
            asin[i] = a;
            cos[i] = a.cos();
        }
        (asin, cos)
    })
}

fn fast_inv_sqrt(x: f64) -> f64 {
    let half = 0.5 * x;
    let bits = x.to_bits() as i64;
    let mut y = f64::from_bits((6910469410427058090i64 - (bits >> 1)) as u64);
    y *= 1.5 - half * y * y;
    y
}

/// `MathHelper.atan2`.
pub fn atan2(mut y: f64, mut x: f64) -> f64 {
    let d = x * x + y * y;
    if d.is_nan() {
        return f64::NAN;
    }
    let neg_y = y < 0.0;
    if neg_y {
        y = -y;
    }
    let neg_x = x < 0.0;
    if neg_x {
        x = -x;
    }
    let swap = y > x;
    if swap {
        std::mem::swap(&mut x, &mut y);
    }
    let inv = fast_inv_sqrt(d);
    x *= inv;
    y *= inv;
    let magic = f64::from_bits(4805340802404319232u64);
    let t = magic + y;
    let idx = t.to_bits() as i32 as usize;
    let (asin, cos) = atan_tables();
    let (a, c) = (asin[idx], cos[idx]);
    let e = t - magic;
    let f = y * c - x * e;
    let g = (6.0 + f * f) * f * 0.16666666666666666;
    let mut r = a + g;
    if swap {
        r = std::f64::consts::FRAC_PI_2 - r;
    }
    if neg_x {
        r = std::f64::consts::PI - r;
    }
    if neg_y {
        r = -r;
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_sin_cos_close_to_exact() {
        for i in 0..64 {
            let a = i as f32 * 0.1;
            assert!((sin(a) - a.sin()).abs() < 1e-3 && (cos(a) - a.cos()).abs() < 1e-3);
        }
        assert_eq!(sin(0.0), 0.0);
        assert_eq!(cos(0.0), 1.0);
    }

    #[test]
    fn atan2_close_to_exact_all_quadrants() {
        for (y, x) in [(1.0, 1.0), (1.0, -1.0), (-1.0, -1.0), (-1.0, 1.0), (0.3, 5.0), (7.0, 0.1), (0.0, -2.0), (-2.0, 0.0)] {
            assert!((atan2(y, x) - f64::atan2(y, x)).abs() < 1e-5, "{y} {x}");
        }
    }
}
