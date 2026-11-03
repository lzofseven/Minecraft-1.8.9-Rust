//! Geometria do céu da 1.8.9: domo (`RenderGlobal.renderSky`/`generateSky`), estrelas
//! (`renderStars`), sol, lua, nascer/pôr do sol, céu do End e nuvens (rápidas e fancy).
//! Tudo em coordenadas relativas ao olho; cada `SkyDraw` leva a matriz do modelo.

use glam::{Mat4, Vec3};
use mc_world::jmath::{cos, sin, JRandom};

use crate::sky::PI;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SkyVertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SkyTex {
    White,
    Sun,
    Moon,
    Clouds,
    EndSky,
    Rain,
    Snow,
}

/// `tryBlendFuncSeparate`: Alpha = (770, 771, 1, 0), Additive = (770, 1, 1, 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkyBlend {
    Opaque,
    Alpha,
    Additive,
}

#[derive(Clone, Debug)]
pub struct SkyDraw {
    pub tex: SkyTex,
    pub blend: SkyBlend,
    pub fog: bool,
    pub depth_test: bool,
    pub depth_write: bool,
    pub color_write: bool,
    pub alpha_test: bool,
    /// Multiplicador do plano distante da projeção (`farPlaneDistance * mul`): céu 2, nuvens 4, resto √2.
    pub far_mul: f32,
    pub model: Mat4,
    /// Lista de triângulos (os quadrados do Java já vêm divididos em dois).
    pub verts: Vec<SkyVertex>,
}

impl SkyDraw {
    fn new(tex: SkyTex, blend: SkyBlend, model: Mat4) -> Self {
        Self { tex, blend, fog: false, depth_test: false, depth_write: false, color_write: true, alpha_test: false, far_mul: 2.0, model, verts: Vec::new() }
    }

    /// Quadrado na ordem de vértices do Java (GL_QUADS).
    pub fn quad(&mut self, v: [SkyVertex; 4]) {
        self.verts.extend_from_slice(&[v[0], v[1], v[2], v[0], v[2], v[3]]);
    }
}

fn v(pos: [f32; 3], uv: [f32; 2], color: [f32; 4]) -> SkyVertex {
    SkyVertex { pos, uv, color }
}

fn rot(deg: f32, axis: Vec3) -> Mat4 {
    Mat4::from_axis_angle(axis, deg.to_radians())
}

/// Vértices de `RenderGlobal.renderStars` (1500 tentativas, semente 10842).
pub fn star_vertices() -> Vec<[f32; 3]> {
    let mut rng = JRandom::new(10842);
    let mut out = Vec::new();
    for _ in 0..1500 {
        let mut d0 = (rng.next_float() * 2.0 - 1.0) as f64;
        let mut d1 = (rng.next_float() * 2.0 - 1.0) as f64;
        let mut d2 = (rng.next_float() * 2.0 - 1.0) as f64;
        let d3 = (0.15f32 + rng.next_float() * 0.1) as f64;
        let mut d4 = d0 * d0 + d1 * d1 + d2 * d2;
        if d4 < 1.0 && d4 > 0.01 {
            d4 = 1.0 / d4.sqrt();
            d0 *= d4;
            d1 *= d4;
            d2 *= d4;
            let (d5, d6, d7) = (d0 * 100.0, d1 * 100.0, d2 * 100.0);
            let d8 = d0.atan2(d2);
            let (d9, d10) = (d8.sin(), d8.cos());
            let d11 = (d0 * d0 + d2 * d2).sqrt().atan2(d1);
            let (d12, d13) = (d11.sin(), d11.cos());
            let d14 = rng.next_double() * std::f64::consts::PI * 2.0;
            let (d15, d16) = (d14.sin(), d14.cos());
            for j in 0..4i32 {
                let d18 = ((j & 2) - 1) as f64 * d3;
                let d19 = (((j + 1) & 2) - 1) as f64 * d3;
                let d21 = d18 * d16 - d19 * d15;
                let d22 = d19 * d16 + d18 * d15;
                let d23 = d21 * d12 + 0.0 * d13;
                let d24 = 0.0 * d12 - d21 * d13;
                let d25 = d24 * d9 - d22 * d10;
                let d26 = d22 * d9 + d24 * d10;
                out.push([(d5 + d25) as f32, (d6 + d23) as f32, (d7 + d26) as f32]);
            }
        }
    }
    out
}

/// `RenderGlobal.renderSky(builder, y, flip)`: grade de quadrados 64x64 de -384 a 384.
fn dome(y: f32, flip: bool, color: [f32; 4], d: &mut SkyDraw) {
    let mut k = -384;
    while k <= 384 {
        let mut l = -384;
        while l <= 384 {
            let (mut f, mut f1) = (k as f32, (k + 64) as f32);
            if flip {
                f1 = k as f32;
                f = (k + 64) as f32;
            }
            let (l0, l1) = (l as f32, (l + 64) as f32);
            d.quad([v([f, y, l0], [0.0; 2], color), v([f1, y, l0], [0.0; 2], color), v([f1, y, l1], [0.0; 2], color), v([f, y, l1], [0.0; 2], color)]);
            l += 64;
        }
        k += 64;
    }
}

/// Tudo que o céu consulta, já calculado pelo `sky.rs`.
pub struct SkyFrame {
    pub dimension: i32,
    pub surface: bool,
    pub sky_colored: bool,
    pub sky_color: [f32; 3],
    pub angle: f32,
    pub rain: f32,
    pub star_brightness: f32,
    pub moon_phase: i32,
    pub sunrise: Option<[f32; 4]>,
    /// `eyeY - world.getHorizon()`.
    pub eye_above_horizon: f64,
}

/// `RenderGlobal.renderSky(pt, pass)`: todas as chamadas de desenho do céu, em ordem.
pub fn sky_draws(f: &SkyFrame) -> Vec<SkyDraw> {
    let mut out = Vec::new();
    if f.dimension == 1 {
        // renderSkyEnd: seis faces com a textura do End, cor 40/255.
        let c = [40.0 / 255.0, 40.0 / 255.0, 40.0 / 255.0, 1.0];
        for i in 0..6 {
            let m = match i {
                1 => rot(90.0, Vec3::X),
                2 => rot(-90.0, Vec3::X),
                3 => rot(180.0, Vec3::X),
                4 => rot(90.0, Vec3::Z),
                5 => rot(-90.0, Vec3::Z),
                _ => Mat4::IDENTITY,
            };
            let mut d = SkyDraw::new(SkyTex::EndSky, SkyBlend::Alpha, m);
            d.quad([
                v([-100.0, -100.0, -100.0], [0.0, 0.0], c),
                v([-100.0, -100.0, 100.0], [0.0, 16.0], c),
                v([100.0, -100.0, 100.0], [16.0, 16.0], c),
                v([100.0, -100.0, -100.0], [16.0, 0.0], c),
            ]);
            out.push(d);
        }
        return out;
    }
    if !f.surface {
        return out;
    }
    let [r, g, b] = f.sky_color;
    // Domo do céu, com neblina.
    let mut d = SkyDraw::new(SkyTex::White, SkyBlend::Opaque, Mat4::IDENTITY);
    d.fog = true;
    dome(16.0, false, [r, g, b, 1.0], &mut d);
    out.push(d);

    // Nascer/pôr do sol: leque colorido.
    if let Some(a) = f.sunrise {
        let sgn = if sin(f.angle * PI * 2.0) < 0.0 { 180.0 } else { 0.0 };
        let m = rot(90.0, Vec3::X) * rot(sgn, Vec3::Z) * rot(90.0, Vec3::Z);
        let mut d = SkyDraw::new(SkyTex::White, SkyBlend::Alpha, m);
        let center = v([0.0, 100.0, 0.0], [0.0; 2], [a[0], a[1], a[2], a[3]]);
        let rim = |l: i32| {
            let f21 = l as f32 * PI * 2.0 / 16.0;
            let (f12, f13) = (sin(f21), cos(f21));
            v([f12 * 120.0, f13 * 120.0, -f13 * 40.0 * a[3]], [0.0; 2], [a[0], a[1], a[2], 0.0])
        };
        for l in 0..16 {
            d.verts.extend_from_slice(&[center, rim(l), rim(l + 1)]);
        }
        out.push(d);
    }

    // Sol, lua e estrelas (mesma rotação), aditivos.
    let m = rot(-90.0, Vec3::Y) * rot(f.angle * 360.0, Vec3::X);
    let a = 1.0 - f.rain;
    let mut sun = SkyDraw::new(SkyTex::Sun, SkyBlend::Additive, m);
    let s = 30.0;
    let w = [1.0, 1.0, 1.0, a];
    sun.quad([v([-s, 100.0, -s], [0.0, 0.0], w), v([s, 100.0, -s], [1.0, 0.0], w), v([s, 100.0, s], [1.0, 1.0], w), v([-s, 100.0, s], [0.0, 1.0], w)]);
    out.push(sun);
    let mut moon = SkyDraw::new(SkyTex::Moon, SkyBlend::Additive, m);
    let s = 20.0;
    let (k, i1) = (f.moon_phase % 4, f.moon_phase / 4 % 2);
    let (u0, v0) = (k as f32 / 4.0, i1 as f32 / 2.0);
    let (u1, v1) = ((k + 1) as f32 / 4.0, (i1 + 1) as f32 / 2.0);
    moon.quad([v([-s, -100.0, s], [u1, v1], w), v([s, -100.0, s], [u0, v1], w), v([s, -100.0, -s], [u0, v0], w), v([-s, -100.0, -s], [u1, v0], w)]);
    out.push(moon);
    let sb = f.star_brightness * a;
    if sb > 0.0 {
        let c = [sb, sb, sb, sb];
        let mut st = SkyDraw::new(SkyTex::White, SkyBlend::Additive, m);
        for q in star_vertices().chunks(4) {
            st.quad([v(q[0], [0.0; 2], c), v(q[1], [0.0; 2], c), v(q[2], [0.0; 2], c), v(q[3], [0.0; 2], c)]);
        }
        out.push(st);
    }

    // Abaixo do horizonte: plano preto e caixa escura quando o olho está abaixo de 63.
    let d0 = f.eye_above_horizon;
    let black = [0.0, 0.0, 0.0, 1.0];
    if d0 < 0.0 {
        let mut d = SkyDraw::new(SkyTex::White, SkyBlend::Opaque, Mat4::from_translation(Vec3::new(0.0, 12.0, 0.0)));
        d.fog = true;
        dome(-16.0, true, black, &mut d);
        out.push(d);
        let f19 = -((d0 + 65.0) as f32);
        let mut d = SkyDraw::new(SkyTex::White, SkyBlend::Opaque, Mat4::IDENTITY);
        d.fog = true;
        let p = |x: f32, y: f32, z: f32| v([x, y, z], [0.0; 2], black);
        d.quad([p(-1.0, f19, 1.0), p(1.0, f19, 1.0), p(1.0, -1.0, 1.0), p(-1.0, -1.0, 1.0)]);
        d.quad([p(-1.0, -1.0, -1.0), p(1.0, -1.0, -1.0), p(1.0, f19, -1.0), p(-1.0, f19, -1.0)]);
        d.quad([p(1.0, -1.0, -1.0), p(1.0, -1.0, 1.0), p(1.0, f19, 1.0), p(1.0, f19, -1.0)]);
        d.quad([p(-1.0, f19, -1.0), p(-1.0, f19, 1.0), p(-1.0, -1.0, 1.0), p(-1.0, -1.0, -1.0)]);
        d.quad([p(-1.0, -1.0, -1.0), p(-1.0, -1.0, 1.0), p(1.0, -1.0, 1.0), p(1.0, -1.0, -1.0)]);
        out.push(d);
    }
    let c = if f.sky_colored { [r * 0.2 + 0.04, g * 0.2 + 0.04, b * 0.6 + 0.1, 1.0] } else { [r, g, b, 1.0] };
    let mut d = SkyDraw::new(SkyTex::White, SkyBlend::Opaque, Mat4::from_translation(Vec3::new(0.0, -((d0 - 16.0) as f32), 0.0)));
    d.fog = true;
    dome(-16.0, true, c, &mut d);
    out.push(d);
    out
}

/// Dados das nuvens (`renderClouds`).
pub struct CloudFrame {
    /// 1 = rápidas, 2 = fancy.
    pub mode: u8,
    pub color: [f32; 3],
    /// Altura do olho interpolada.
    pub eye_y: f32,
    pub cloud_height: f32,
    /// `cloudTickCounter + pt`.
    pub ticks: f64,
    /// Posição X/Z do olho interpolada.
    pub x: f64,
    pub z: f64,
}

pub fn cloud_draws(c: &CloudFrame) -> Vec<SkyDraw> {
    let [r, g, b] = c.color;
    let mut out = Vec::new();
    let f3 = c.cloud_height - c.eye_y + 0.33;
    if c.mode != 2 {
        let f10 = 4.8828125E-4f32;
        let mut d0 = c.x + c.ticks * 0.029999999329447746;
        let mut d1 = c.z;
        let k = (d0 / 2048.0).floor() as i32;
        let l = (d1 / 2048.0).floor() as i32;
        d0 -= (k * 2048) as f64;
        d1 -= (l * 2048) as f64;
        let f8 = (d0 * 4.8828125E-4) as f32;
        let f9 = (d1 * 4.8828125E-4) as f32;
        let mut d = SkyDraw::new(SkyTex::Clouds, SkyBlend::Alpha, Mat4::IDENTITY);
        d.fog = true;
        d.depth_test = true;
        d.depth_write = true;
        d.alpha_test = true;
        d.far_mul = 4.0;
        let col = [r, g, b, 0.8];
        let mut i1 = -256;
        while i1 < 256 {
            let mut j1 = -256;
            while j1 < 256 {
                let (a0, a1, b0, b1) = (i1 as f32, (i1 + 32) as f32, j1 as f32, (j1 + 32) as f32);
                d.quad([
                    v([a0, f3, b1], [a0 * f10 + f8, b1 * f10 + f9], col),
                    v([a1, f3, b1], [a1 * f10 + f8, b1 * f10 + f9], col),
                    v([a1, f3, b0], [a1 * f10 + f8, b0 * f10 + f9], col),
                    v([a0, f3, b0], [a0 * f10 + f8, b0 * f10 + f9], col),
                ]);
                j1 += 32;
            }
            i1 += 32;
        }
        out.push(d);
        return out;
    }
    // Fancy: caixas de 12x4x12 blocos; duas passagens (profundidade e cor).
    let d0 = (c.x + c.ticks * 0.029999999329447746) / 12.0;
    let d2 = c.z / 12.0 + 0.33000001311302185;
    let i = (d0 / 2048.0).floor() as i32;
    let j = (d2 / 2048.0).floor() as i32;
    let d1 = d0 - (i * 2048) as f64;
    let d2 = d2 - (j * 2048) as f64;
    let (f4, f5, f6) = (r, g, b);
    let (f26, f27, f28) = (f4 * 0.9, f5 * 0.9, f6 * 0.9);
    let (f10, f11, f12) = (f4 * 0.7, f5 * 0.7, f6 * 0.7);
    let (f13, f14, f15) = (f4 * 0.8, f5 * 0.8, f6 * 0.8);
    let f16 = 0.00390625f32;
    let f17 = d1.floor() as f32 * f16;
    let f18 = d2.floor() as f32 * f16;
    let f19 = (d1 - d1.floor()) as f32;
    let f20 = (d2 - d2.floor()) as f32;
    let eps = 9.765625E-4f32;
    let mut verts: Vec<SkyVertex> = Vec::new();
    // As posições já levam a escala (12, 1, 12) do `GlStateManager.scale`.
    let mut q = |p: [[f32; 3]; 4], uv: [[f32; 2]; 4], col: [f32; 3]| {
        let c4 = [col[0], col[1], col[2], 0.8];
        let vs = [v([p[0][0] * 12.0, p[0][1], p[0][2] * 12.0], uv[0], c4), v([p[1][0] * 12.0, p[1][1], p[1][2] * 12.0], uv[1], c4), v([p[2][0] * 12.0, p[2][1], p[2][2] * 12.0], uv[2], c4), v([p[3][0] * 12.0, p[3][1], p[3][2] * 12.0], uv[3], c4)];
        verts.extend_from_slice(&[vs[0], vs[1], vs[2], vs[0], vs[2], vs[3]]);
    };
    for j1 in -3..=4i32 {
        for k1 in -3..=4i32 {
            let f22 = (j1 * 8) as f32;
            let f23 = (k1 * 8) as f32;
            let f24 = f22 - f19;
            let f25 = f23 - f20;
            let u = |x: f32| x * f16 + f17;
            let w = |z: f32| z * f16 + f18;
            if f3 > -5.0 {
                let y = f3;
                q(
                    [[f24, y, f25 + 8.0], [f24 + 8.0, y, f25 + 8.0], [f24 + 8.0, y, f25], [f24, y, f25]],
                    [[u(f22), w(f23 + 8.0)], [u(f22 + 8.0), w(f23 + 8.0)], [u(f22 + 8.0), w(f23)], [u(f22), w(f23)]],
                    [f10, f11, f12],
                );
            }
            if f3 <= 5.0 {
                let y = f3 + 4.0 - eps;
                q(
                    [[f24, y, f25 + 8.0], [f24 + 8.0, y, f25 + 8.0], [f24 + 8.0, y, f25], [f24, y, f25]],
                    [[u(f22), w(f23 + 8.0)], [u(f22 + 8.0), w(f23 + 8.0)], [u(f22 + 8.0), w(f23)], [u(f22), w(f23)]],
                    [f4, f5, f6],
                );
            }
            if j1 > -1 {
                for l1 in 0..8 {
                    let l = l1 as f32;
                    let x = f24 + l;
                    let uu = u(f22 + l + 0.5);
                    q(
                        [[x, f3, f25 + 8.0], [x, f3 + 4.0, f25 + 8.0], [x, f3 + 4.0, f25], [x, f3, f25]],
                        [[uu, w(f23 + 8.0)], [uu, w(f23 + 8.0)], [uu, w(f23)], [uu, w(f23)]],
                        [f26, f27, f28],
                    );
                }
            }
            if j1 <= 1 {
                for i2 in 0..8 {
                    let l = i2 as f32;
                    let x = f24 + l + 1.0 - eps;
                    let uu = u(f22 + l + 0.5);
                    q(
                        [[x, f3, f25 + 8.0], [x, f3 + 4.0, f25 + 8.0], [x, f3 + 4.0, f25], [x, f3, f25]],
                        [[uu, w(f23 + 8.0)], [uu, w(f23 + 8.0)], [uu, w(f23)], [uu, w(f23)]],
                        [f26, f27, f28],
                    );
                }
            }
            if k1 > -1 {
                for j2 in 0..8 {
                    let l = j2 as f32;
                    let z = f25 + l;
                    let ww = w(f23 + l + 0.5);
                    q(
                        [[f24, f3 + 4.0, z], [f24 + 8.0, f3 + 4.0, z], [f24 + 8.0, f3, z], [f24, f3, z]],
                        [[u(f22), ww], [u(f22 + 8.0), ww], [u(f22 + 8.0), ww], [u(f22), ww]],
                        [f13, f14, f15],
                    );
                }
            }
            if k1 <= 1 {
                for k2 in 0..8 {
                    let l = k2 as f32;
                    let z = f25 + l + 1.0 - eps;
                    let ww = w(f23 + l + 0.5);
                    q(
                        [[f24, f3 + 4.0, z], [f24 + 8.0, f3 + 4.0, z], [f24 + 8.0, f3, z], [f24, f3, z]],
                        [[u(f22), ww], [u(f22 + 8.0), ww], [u(f22 + 8.0), ww], [u(f22), ww]],
                        [f13, f14, f15],
                    );
                }
            }
        }
    }
    for pass in 0..2 {
        let mut d = SkyDraw::new(SkyTex::Clouds, SkyBlend::Alpha, Mat4::IDENTITY);
        d.fog = true;
        d.depth_test = true;
        d.depth_write = true;
        d.color_write = pass == 1;
        d.alpha_test = true;
        d.far_mul = 4.0;
        d.verts = verts.clone();
        out.push(d);
    }
    out
}

/// O que `EntityRenderer.renderRainSnow` pergunta ao mundo.
pub trait WeatherWorld {
    /// `World.getBiomeGenForCoords`: id do bioma da coluna.
    fn biome(&self, x: i32, z: i32) -> u8;
    /// `World.getPrecipitationHeight(pos).getY()`.
    fn precipitation_height(&self, x: i32, z: i32) -> i32;
    /// (céu, bloco) de `getCombinedLight(pos, 0)`.
    fn light(&self, x: i32, y: i32, z: i32) -> (u8, u8);
}

/// Parâmetros de um quadro de chuva/neve.
pub struct WeatherFrame<'a> {
    /// `world.getRainStrength(pt)`.
    pub strength: f32,
    pub fancy: bool,
    /// `rendererUpdateCount`.
    pub ticks: i32,
    pub pt: f32,
    /// `entity.posX/Y/Z` (posição do tick, sem interpolar).
    pub pos: [f64; 3],
    /// Posição interpolada dos pés (`lastTickPos + (pos - lastTickPos) * pt`).
    pub feet: [f64; 3],
    /// `EntityRenderer.lightmapColors` (céu * 16 + bloco), em gama.
    pub lightmap: &'a [[u8; 3]; 256],
}

/// `rainXCoords`/`rainYCoords` do construtor do `EntityRenderer` (32x32).
fn rain_coords() -> &'static [(f32, f32); 1024] {
    static T: std::sync::OnceLock<Box<[(f32, f32); 1024]>> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let mut t = Box::new([(0f32, 0f32); 1024]);
        for i in 0..32usize {
            for j in 0..32usize {
                let f = (j as i32 - 16) as f32;
                let f1 = (i as i32 - 16) as f32;
                let f2 = ((f * f + f1 * f1) as f64).sqrt() as f32;
                t[i << 5 | j] = (-f1 / f2, f / f2);
            }
        }
        t
    })
}

/// `EntityRenderer.renderRainSnow`: quadrados de chuva e neve ao redor do jogador. A geometria sai
/// relativa aos pés (o `setTranslation(-d0, -d1, -d2)` do Java); o chamador aplica o deslocamento da câmera.
/// Devolve no máximo dois desenhos (chuva e neve), nessa ordem.
pub fn weather_draws(f: &WeatherFrame, world: &dyn WeatherWorld) -> Vec<SkyDraw> {
    let strength = f.strength;
    if strength <= 0.0 {
        return Vec::new();
    }
    let (i, j, k) = (f.pos[0].floor() as i32, f.pos[1].floor() as i32, f.pos[2].floor() as i32);
    let l = f.feet[1].floor() as i32;
    let i1 = if f.fancy { 10 } else { 5 };
    let f1 = f.ticks as f32 + f.pt;
    let coords = rain_coords();
    let mut rng = JRandom::new(0);
    let mut rain = SkyDraw::new(SkyTex::Rain, SkyBlend::Alpha, Mat4::IDENTITY);
    let mut snow = SkyDraw::new(SkyTex::Snow, SkyBlend::Alpha, Mat4::IDENTITY);
    for d in [&mut rain, &mut snow] {
        d.fog = true;
        d.depth_test = true;
        d.alpha_test = true;
        d.far_mul = std::f32::consts::SQRT_2;
    }
    let rel = |x: f64, y: f64, z: f64| [(x - f.feet[0]) as f32, (y - f.feet[1]) as f32, (z - f.feet[2]) as f32];
    let lm = |sky: u8, blk: u8| {
        let c = f.lightmap[(sky as usize & 15) * 16 + (blk as usize & 15)];
        [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0]
    };
    for k1 in (k - i1)..=(k + i1) {
        for l1 in (i - i1)..=(i + i1) {
            let i2 = ((k1 - k + 16) * 32 + l1 - i + 16) as usize;
            let (cx, cy) = coords[i2];
            // O índice central (0/0) é NaN no Java: o quadrado some na rasterização.
            if cx.is_nan() || cy.is_nan() {
                continue;
            }
            let (d3, d4) = (cx as f64 * 0.5, cy as f64 * 0.5);
            let biome = world.biome(l1, k1);
            if !crate::sky::biome_precipitates(biome) {
                continue;
            }
            let j2 = world.precipitation_height(l1, k1);
            let mut k2 = j - i1;
            let mut l2 = j + i1;
            if k2 < j2 {
                k2 = j2;
            }
            if l2 < j2 {
                l2 = j2;
            }
            let mut i3 = j2;
            if j2 < l {
                i3 = l;
            }
            if k2 == l2 {
                continue;
            }
            let seed = (l1.wrapping_mul(l1).wrapping_mul(3121).wrapping_add(l1.wrapping_mul(45238971)))
                ^ (k1.wrapping_mul(k1).wrapping_mul(418711).wrapping_add(k1.wrapping_mul(13761)));
            rng.set_seed(seed as i64);
            let t = crate::sky::float_temperature(biome, l1, k2, k1);
            let (x0, x1) = (l1 as f64 - d3 + 0.5, l1 as f64 + d3 + 0.5);
            let (z0, z1) = (k1 as f64 - d4 + 0.5, k1 as f64 + d4 + 0.5);
            let (yb, yt) = (k2 as f64, l2 as f64);
            let d6 = (l1 as f32 + 0.5) as f64 - f.pos[0];
            let d7 = (k1 as f32 + 0.5) as f64 - f.pos[2];
            let dist = ((d6 * d6 + d7 * d7).sqrt() as f32) / i1 as f32;
            let (sky, blk) = world.light(l1, i3, k1);
            if t >= 0.15 {
                let sum = f.ticks.wrapping_add(l1.wrapping_mul(l1).wrapping_mul(3121)).wrapping_add(l1.wrapping_mul(45238971)).wrapping_add(k1.wrapping_mul(k1).wrapping_mul(418711)).wrapping_add(k1.wrapping_mul(13761));
                let d5 = ((sum & 31) as f64 + f.pt as f64) / 32.0 * (3.0 + rng.next_double());
                let a = ((1.0 - dist * dist) * 0.5 + 0.5) * strength;
                let [r, g, b] = lm(sky, blk);
                let c = [r, g, b, a];
                rain.quad([
                    v(rel(x0, yb, z0), [0.0, (yb * 0.25 + d5) as f32], c),
                    v(rel(x1, yb, z1), [1.0, (yb * 0.25 + d5) as f32], c),
                    v(rel(x1, yt, z1), [1.0, (yt * 0.25 + d5) as f32], c),
                    v(rel(x0, yt, z0), [0.0, (yt * 0.25 + d5) as f32], c),
                ]);
            } else {
                let d8 = ((((f.ticks & 511) as f32) + f.pt) / 512.0) as f64;
                let d9 = rng.next_double() + f1 as f64 * 0.01 * (rng.next_gaussian() as f32) as f64;
                let d10 = rng.next_double() + (f1 * (rng.next_gaussian() as f32)) as f64 * 0.001;
                let a = ((1.0 - dist * dist) * 0.3 + 0.5) * strength;
                // `(getCombinedLight * 3 + 15728880) / 4`: clareia a luz da neve.
                let packed = ((sky as i32) << 20 | (blk as i32) << 4) * 3 + 15728880;
                let i4 = packed / 4;
                let (s4, b4) = (((i4 >> 16) & 0xFFFF) >> 4, (i4 & 0xFFFF) >> 4);
                let [r, g, b] = lm(s4 as u8, b4 as u8);
                let c = [r, g, b, a];
                let vv = |y: f64| (y * 0.25 + d8 + d10) as f32;
                snow.quad([
                    v(rel(x0, yb, z0), [d9 as f32, vv(yb)], c),
                    v(rel(x1, yb, z1), [(1.0 + d9) as f32, vv(yb)], c),
                    v(rel(x1, yt, z1), [(1.0 + d9) as f32, vv(yt)], c),
                    v(rel(x0, yt, z0), [d9 as f32, vv(yt)], c),
                ]);
            }
        }
    }
    [rain, snow].into_iter().filter(|d| !d.verts.is_empty()).collect()
}
