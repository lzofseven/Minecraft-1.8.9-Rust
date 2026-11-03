//! Texturas animadas: `TextureAtlasSprite.updateAnimation` com `AnimationMetadataSection`
//! (`.mcmeta` do jar), mais `TextureCompass` e `TextureClock`, que dependem do mundo.

use image::RgbaImage;
use mc_world::mesh::ATLAS_COLS;

use crate::assets::Jar;

/// `AnimationMetadataSection`: quadros (índice, tempo ou -1) e tempo padrão.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimMeta {
    pub frame_time: i32,
    pub interpolate: bool,
    pub frames: Vec<(usize, i32)>,
}

impl AnimMeta {
    /// `AnimationMetadataSectionSerializer.deserialize`; sem lista de quadros usa todos, em ordem.
    pub fn parse(json: &[u8], frame_count: usize) -> Option<Self> {
        let v: serde_json::Value = serde_json::from_slice(json).ok()?;
        let a = v.get("animation")?;
        let frame_time = a.get("frametime").and_then(|f| f.as_i64()).unwrap_or(1) as i32;
        let interpolate = a.get("interpolate").and_then(|f| f.as_bool()).unwrap_or(false);
        let mut frames: Vec<(usize, i32)> = a
            .get("frames")
            .and_then(|f| f.as_array())
            .map(|list| {
                list.iter()
                    .filter_map(|e| match e {
                        serde_json::Value::Number(n) => Some((n.as_u64()? as usize, -1)),
                        o => Some((o.get("index")?.as_u64()? as usize, o.get("time").and_then(|t| t.as_i64()).unwrap_or(-1) as i32)),
                    })
                    .collect()
            })
            .unwrap_or_default();
        if frames.is_empty() {
            frames = (0..frame_count).map(|i| (i, -1)).collect();
        }
        Some(Self { frame_time, interpolate, frames })
    }

    /// `getFrameTimeSingle`.
    fn time_of(&self, i: usize) -> i32 {
        let t = self.frames[i].1;
        if t == -1 {
            self.frame_time
        } else {
            t
        }
    }
}

/// Mipmaps por média 2x2 (16, 8, 4, 2, 1 px), como o atlas.
pub fn mip_chain(base: Vec<u8>, size: u32) -> Vec<Vec<u8>> {
    let mut levels = vec![base];
    let mut s = size;
    while s > 1 {
        let prev = levels.last().unwrap();
        let n = s / 2;
        let mut next = vec![0u8; (n * n * 4) as usize];
        for y in 0..n {
            for x in 0..n {
                for c in 0..4 {
                    let at = |dx: u32, dy: u32| prev[(((2 * y + dy) * s + 2 * x + dx) * 4 + c) as usize] as u32;
                    next[((y * n + x) * 4 + c) as usize] = ((at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1)) / 4) as u8;
                }
            }
        }
        levels.push(next);
        s = n;
    }
    levels
}

/// Quadro `i` da tira vertical, levado a `px` x `px` como `block_tile` (o atlas pode ser HD).
fn frame_rgba(img: &RgbaImage, i: u32, px: u32) -> Vec<u8> {
    let w = img.width().max(1);
    let mut out = Vec::with_capacity((px * px * 4) as usize);
    for y in 0..px {
        for x in 0..px {
            let sy = (i * w + y * w / px).min(img.height() - 1);
            out.extend_from_slice(&img.get_pixel(x * w / px, sy).0);
        }
    }
    out
}

fn strip_frames(img: &RgbaImage, px: u32) -> Vec<Vec<u8>> {
    (0..(img.height() / img.width().max(1)).max(1)).map(|i| frame_rgba(img, i, px)).collect()
}

/// Uma textura animada do atlas de blocos.
pub struct Sprite {
    pub tile: u16,
    frames: Vec<Vec<Vec<u8>>>,
    meta: AnimMeta,
    frame_counter: usize,
    tick_counter: i32,
    blended: Vec<Vec<u8>>,
}

impl Sprite {
    pub fn new(tile: u16, frames: Vec<Vec<u8>>, meta: AnimMeta) -> Self {
        let frames = frames.into_iter().map(|f| mip_chain(f.clone(), ((f.len() / 4) as f64).sqrt() as u32)).collect();
        Self { tile, frames, meta, frame_counter: 0, tick_counter: 0, blended: Vec::new() }
    }

    /// Índice do quadro mostrado agora (`getFrameIndex(frameCounter)`).
    pub fn shown_frame(&self) -> usize {
        self.meta.frames[self.frame_counter].0
    }

    /// `updateAnimation`: devolve os mipmaps a gravar no atlas, se este tick muda a imagem.
    pub fn update(&mut self) -> Option<&[Vec<u8>]> {
        self.tick_counter += 1;
        let count = self.meta.frames.len();
        if self.tick_counter >= self.meta.time_of(self.frame_counter) {
            let i = self.meta.frames[self.frame_counter].0;
            self.frame_counter = (self.frame_counter + 1) % count;
            self.tick_counter = 0;
            let k = self.meta.frames[self.frame_counter].0;
            if i != k && k < self.frames.len() {
                return Some(&self.frames[k]);
            }
        } else if self.meta.interpolate {
            return self.interpolated();
        }
        None
    }

    /// `updateAnimationInterpolated`.
    fn interpolated(&mut self) -> Option<&[Vec<u8>]> {
        let d0 = 1.0 - self.tick_counter as f64 / self.meta.time_of(self.frame_counter) as f64;
        let i = self.meta.frames[self.frame_counter].0;
        let k = self.meta.frames[(self.frame_counter + 1) % self.meta.frames.len()].0;
        if i == k || k >= self.frames.len() {
            return None;
        }
        let (a, b) = (&self.frames[i], &self.frames[k]);
        self.blended = a
            .iter()
            .zip(b)
            .map(|(la, lb)| {
                la.chunks_exact(4)
                    .zip(lb.chunks_exact(4))
                    .flat_map(|(p, q)| {
                        let m = |c: usize| (p[c] as f64 * d0 + q[c] as f64 * (1.0 - d0)) as u8;
                        [m(0), m(1), m(2), p[3]]
                    })
                    .collect()
            })
            .collect();
        Some(&self.blended)
    }
}

/// `TextureCompass`.
pub struct Compass {
    frames: Vec<Vec<u8>>,
    current_angle: f64,
    angle_delta: f64,
    frame: usize,
}

/// Dados do mundo que a bússola lê (`Minecraft.theWorld` e `thePlayer`).
pub struct CompassWorld {
    pub spawn: (i32, i32),
    pub x: f64,
    pub z: f64,
    pub yaw: f64,
    pub surface: bool,
}

impl Compass {
    pub fn new(frames: Vec<Vec<u8>>) -> Self {
        Self { frames, current_angle: 0.0, angle_delta: 0.0, frame: 0 }
    }

    /// `updateCompass(world, x, z, yaw, noWorld, snap)`; `random` é o `Math.random()` do nether/end.
    /// Devolve o quadro novo quando ele muda.
    pub fn update(&mut self, w: Option<CompassWorld>, random: f64) -> Option<usize> {
        let n = self.frames.len() as i32;
        if n == 0 {
            return None;
        }
        let mut target = 0.0;
        if let Some(w) = w {
            let dx = w.spawn.0 as f64 - w.x;
            let dz = w.spawn.1 as f64 - w.z;
            let yaw = w.yaw % 360.0;
            target = -((yaw - 90.0) * std::f64::consts::PI / 180.0 - dz.atan2(dx));
            if !w.surface {
                target = random * 3.1415927410125732 * 2.0;
            }
        }
        let mut d = target - self.current_angle;
        while d < -std::f64::consts::PI {
            d += std::f64::consts::TAU;
        }
        while d >= std::f64::consts::PI {
            d -= std::f64::consts::TAU;
        }
        d = d.clamp(-1.0, 1.0);
        self.angle_delta += d * 0.1;
        self.angle_delta *= 0.8;
        self.current_angle += self.angle_delta;
        let mut f = ((self.current_angle / std::f64::consts::TAU + 1.0) * n as f64) as i32 % n;
        while f < 0 {
            f = (f + n) % n;
        }
        self.set_frame(f as usize)
    }

    fn set_frame(&mut self, f: usize) -> Option<usize> {
        (f != self.frame).then(|| {
            self.frame = f;
            f
        })
    }

    pub fn frame_rgba(&self, i: usize) -> &[u8] {
        &self.frames[i]
    }
}

/// `TextureClock`.
pub struct Clock {
    frames: Vec<Vec<u8>>,
    current_angle: f64,
    angle_delta: f64,
    frame: usize,
}

impl Clock {
    pub fn new(frames: Vec<Vec<u8>>) -> Self {
        Self { frames, current_angle: 0.0, angle_delta: 0.0, frame: 0 }
    }

    /// `updateAnimation`: `celestial` é `getCelestialAngle(1.0F)` (None sem mundo); `random` vale
    /// no nether/end.
    pub fn update(&mut self, celestial: Option<f64>, surface: bool, random: f64) -> Option<usize> {
        let n = self.frames.len() as i32;
        if n == 0 {
            return None;
        }
        let mut target = 0.0;
        if let Some(c) = celestial {
            target = if surface { c } else { random };
        }
        let mut d = target - self.current_angle;
        while d < -0.5 {
            d += 1.0;
        }
        while d >= 0.5 {
            d -= 1.0;
        }
        d = d.clamp(-1.0, 1.0);
        self.angle_delta += d * 0.1;
        self.angle_delta *= 0.8;
        self.current_angle += self.angle_delta;
        let mut f = ((self.current_angle + 1.0) * n as f64) as i32 % n;
        while f < 0 {
            f = (f + n) % n;
        }
        let f = f as usize;
        (f != self.frame).then(|| {
            self.frame = f;
            f
        })
    }

    pub fn frame_rgba(&self, i: usize) -> &[u8] {
        &self.frames[i]
    }
}

/// `WorldProvider.calculateCelestialAngle(time, 1.0F)` para o relógio.
pub fn celestial_angle(world_time: i64) -> f64 {
    let i = (world_time.unsigned_abs() % 24000) as i32;
    let mut f = (i as f32 + 1.0) / 24000.0 - 0.25;
    if f < 0.0 {
        f += 1.0;
    }
    if f > 1.0 {
        f -= 1.0;
    }
    let f1 = f;
    let f = 1.0 - (((f as f64 * std::f64::consts::PI).cos() + 1.0) / 2.0) as f32;
    (f1 + (f - f1) / 3.0) as f64
}

/// Todas as animações do atlas de blocos e os ícones vivos de bússola e relógio.
pub struct Animations {
    pub sprites: Vec<Sprite>,
    pub compass: Option<Compass>,
    pub clock: Option<Clock>,
}

impl Animations {
    /// Procura `.mcmeta` para cada textura do atlas (`names` na ordem do atlas) e para os itens.
    pub fn load(jar: &mut Jar, names: &[String], tile: u32) -> Self {
        let mut sprites = Vec::new();
        for (i, name) in names.iter().enumerate() {
            if name.starts_with('@') || name.starts_with('~') {
                continue;
            }
            let Some(meta_bytes) = jar.texture_mcmeta(&format!("blocks/{name}")) else { continue };
            let Some(img) = jar.texture(&format!("blocks/{name}")) else { continue };
            let frames = strip_frames(&img, tile);
            if let Some(meta) = AnimMeta::parse(&meta_bytes, frames.len()) {
                sprites.push(Sprite::new(i as u16, frames, meta));
            }
        }
        // Quadros na resolução da textura (16 no jar, 32+ em pacotes HD), empilhados na vertical.
        let item = |jar: &mut Jar, name: &str| jar.texture(&format!("items/{name}")).map(|img| strip_frames(&img, img.width().max(1)));
        Self { sprites, compass: item(jar, "compass").map(Compass::new), clock: item(jar, "clock").map(Clock::new) }
    }

    /// Um tick do jogo: tiles do atlas que mudam, com todos os mipmaps.
    pub fn tick_blocks(&mut self) -> Vec<(u16, &[Vec<u8>])> {
        let mut out = Vec::new();
        for s in &mut self.sprites {
            let tile = s.tile;
            if let Some(levels) = s.update() {
                out.push((tile, levels));
            }
        }
        out
    }
}

/// Origem e tamanho (px) do tile `tile` no nível de mipmap `level`, num atlas de tiles de `px` px.
pub fn tile_rect(tile: u16, level: u32, px: u32) -> (u32, u32, u32) {
    let (cx, cy) = ((tile as u32 % ATLAS_COLS) * px, (tile as u32 / ATLAS_COLS) * px);
    (cx >> level, cy >> level, px >> level)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::TILE;

    fn solid(v: u8) -> Vec<u8> {
        vec![v; (TILE * TILE * 4) as usize]
    }

    fn sprite(frames: &[u8], meta: AnimMeta) -> Sprite {
        Sprite::new(0, frames.iter().map(|&v| solid(v)).collect(), meta)
    }

    #[test]
    fn mcmeta_defaults_and_explicit_frames() {
        let m = AnimMeta::parse(br#"{"animation": {}}"#, 3).unwrap();
        assert_eq!(m, AnimMeta { frame_time: 1, interpolate: false, frames: vec![(0, -1), (1, -1), (2, -1)] });
        let m = AnimMeta::parse(br#"{"animation":{"frametime":300,"interpolate":true,"frames":[0,{"index":2,"time":5}]}}"#, 4).unwrap();
        assert_eq!(m, AnimMeta { frame_time: 300, interpolate: true, frames: vec![(0, -1), (2, 5)] });
        assert_eq!((m.time_of(0), m.time_of(1)), (300, 5));
    }

    #[test]
    fn frames_change_exactly_every_frametime_ticks() {
        let mut s = sprite(&[10, 20, 30], AnimMeta { frame_time: 3, interpolate: false, frames: vec![(0, -1), (1, -1), (2, -1)] });
        let mut changes = Vec::new();
        for tick in 1..=12 {
            if let Some(l) = s.update() {
                changes.push((tick, l[0][0]));
            }
        }
        // Troca no 3º, 6º, 9º e 12º tick: 1, 2, 0, 1.
        assert_eq!(changes, vec![(3, 20), (6, 30), (9, 10), (12, 20)]);
    }

    #[test]
    fn per_frame_time_overrides_frametime() {
        let mut s = sprite(&[1, 2], AnimMeta { frame_time: 10, interpolate: false, frames: vec![(0, 2), (1, 4)] });
        let mut at = Vec::new();
        for t in 1..=12 {
            if s.update().is_some() {
                at.push(t);
            }
        }
        assert_eq!(at, vec![2, 6, 8, 12]);
    }

    #[test]
    fn same_frame_index_twice_does_not_upload() {
        // Lista 0,0,1: a passagem do primeiro 0 para o segundo 0 não regrava a textura.
        let mut s = sprite(&[1, 2], AnimMeta { frame_time: 1, interpolate: false, frames: vec![(0, -1), (0, -1), (1, -1)] });
        let got: Vec<bool> = (0..4).map(|_| s.update().is_some()).collect();
        assert_eq!(got, vec![false, true, true, false]);
    }

    #[test]
    fn interpolation_blends_between_frames_each_tick() {
        let mut s = sprite(&[0, 200], AnimMeta { frame_time: 4, interpolate: true, frames: vec![(0, -1), (1, -1)] });
        let vals: Vec<u8> = (0..4).map(|_| s.update().map(|l| l[0][0]).unwrap_or(255)).collect();
        // d0 = 1 - t/4: mistura 0 * 0.75 + 200 * 0.25 = 50, depois 100, 150 e a troca (quadro 1 = 200).
        assert_eq!(vals, vec![50, 100, 150, 200]);
    }

    #[test]
    fn mipmaps_follow_every_frame() {
        let s = sprite(&[100], AnimMeta { frame_time: 1, interpolate: false, frames: vec![(0, -1)] });
        assert_eq!(s.frames[0].len(), 5);
        assert_eq!(s.frames[0][4], vec![100; 4]);
    }

    fn compass() -> Compass {
        Compass::new((0..32).map(|i| solid(i as u8)).collect())
    }

    #[test]
    fn compass_points_to_spawn_and_settles() {
        let mut c = compass();
        // Jogador em (0, 0) olhando para o sul (yaw 0), spawn ao sul (z positivo): ângulo alvo 0 -> quadro 0 (mod 32).
        let w = || CompassWorld { spawn: (0, 100), x: 0.0, z: 0.0, yaw: 0.0, surface: true };
        for _ in 0..300 {
            c.update(Some(w()), 0.0);
        }
        let target = -((0.0 - 90.0) * std::f64::consts::PI / 180.0 - 100f64.atan2(0.0));
        // O ângulo converge para o alvo a menos de voltas inteiras (a diferença é normalizada).
        let diff = (c.current_angle - target).rem_euclid(std::f64::consts::TAU);
        assert!(diff.min(std::f64::consts::TAU - diff) < 1e-3, "{} vs {}", c.current_angle, target);
        let want = (((target / std::f64::consts::TAU).rem_euclid(1.0)) * 32.0) as i32;
        let got = c.frame as i32;
        assert!(((got - want).rem_euclid(32)).min((want - got).rem_euclid(32)) <= 1, "quadro {got}, esperado ~{want}");
    }

    #[test]
    fn compass_turns_gradually() {
        let mut c = compass();
        let w = CompassWorld { spawn: (100, 0), x: 0.0, z: 0.0, yaw: 0.0, surface: true };
        c.update(Some(w), 0.0);
        // Um tick só move delta = clamp(d,-1,1) * 0.1 * 0.8 = no máximo 0.08 rad.
        assert!(c.current_angle.abs() <= 0.08 + 1e-12 && c.current_angle != 0.0);
    }

    #[test]
    fn clock_follows_celestial_angle() {
        // Meio-dia (6000): ângulo celeste 0.0 -> quadro 0; meia-noite (18000): 0.5 -> quadro 32 de 64.
        assert!(celestial_angle(6000).abs() < 1e-3);
        assert!((celestial_angle(18000) - 0.5).abs() < 1e-3);
        let mut k = Clock::new((0..64).map(|i| solid(i as u8)).collect());
        for _ in 0..400 {
            k.update(Some(celestial_angle(18000)), true, 0.0);
        }
        assert_eq!(k.frame, ((k.current_angle + 1.0) * 64.0) as usize % 64);
        assert!((31..=33).contains(&k.frame), "{}", k.frame);
    }

    #[test]
    fn tile_rect_halves_each_level() {
        assert_eq!(tile_rect(33, 0, 16), (16, 16, 16));
        assert_eq!(tile_rect(33, 2, 16), (4, 4, 4));
    }
}
