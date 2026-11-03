//! `MathHelper` da 1.8.9: seno e cosseno por tabela de 65536 posições, como o
//! jogo usa na física, na câmera e nos modelos (o resultado difere do `sin` exato).

use std::sync::OnceLock;

fn table() -> &'static [f32; 65536] {
    static T: OnceLock<Box<[f32; 65536]>> = OnceLock::new();
    T.get_or_init(|| {
        let mut t = Box::new([0.0f32; 65536]);
        for (i, v) in t.iter_mut().enumerate() {
            *v = (i as f64 * std::f64::consts::PI * 2.0 / 65536.0).sin() as f32;
        }
        t
    })
}

/// `MathHelper.sin`.
pub fn sin(x: f32) -> f32 {
    table()[((x * 10430.378) as i32 & 0xffff) as usize]
}

/// `MathHelper.cos`.
pub fn cos(x: f32) -> f32 {
    table()[((x * 10430.378 + 16384.0) as i32 & 0xffff) as usize]
}

/// `MathHelper.wrapAngleTo180_float`.
pub fn wrap_degrees(mut a: f32) -> f32 {
    a %= 360.0;
    if a >= 180.0 {
        a -= 360.0;
    }
    if a < -180.0 {
        a += 360.0;
    }
    a
}

/// `MathHelper.floor_double`.
pub fn floor(v: f64) -> i32 {
    v.floor() as i32
}

/// `(float)Math.PI`.
pub const PI: f32 = std::f32::consts::PI;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_matches_java_samples() {
        assert_eq!(sin(0.0), 0.0);
        assert_eq!(cos(0.0), 1.0);
        // Valores conferidos com a tabela do Java: índice (int)(x*10430.378).
        assert_eq!(sin(PI / 2.0), table()[16383]);
        assert!((sin(1.0) - 1f32.sin()).abs() < 1e-4);
        assert_eq!(wrap_degrees(190.0), -170.0);
        assert_eq!(wrap_degrees(-540.0), -180.0);
    }
}
