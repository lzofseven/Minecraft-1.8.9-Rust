//! Céu, neblina, nuvens e mapa de luz da 1.8.9, portados de `WorldProvider`, `World`,
//! `EntityRenderer` (updateFogColor, setupFog, updateLightmap) e `BiomeGenBase`.
//! Toda a aritmética segue o Java em `f32`/`f64` na mesma ordem; as cores saem em espaço
//! gama (0..1), como o OpenGL fixo do jogo original.

use std::sync::OnceLock;

use mc_world::gen_biomes::BIOMES;
use mc_world::jmath::{clamp, cos, hsv_to_rgb, sin, Perlin1};

pub const PI: f32 = 3.1415927;

/// Estado do mundo que o céu consulta (`World`/`WorldInfo` do cliente).
#[derive(Clone, Copy, Debug, Default)]
pub struct SkyWorld {
    pub dimension: i32,
    /// `WorldInfo.getWorldTime()` (sempre positivo; o sinal só marca doDaylightCycle).
    pub time: i64,
    /// Mundo superplano (`WorldType.FLAT`): horizonte em 0 e neblina de vazio total.
    pub flat: bool,
    /// `rainingStrength` (S2B 1, 2 e 7).
    pub rain: f32,
    /// `thunderingStrength` bruto (S2B 8); a força real é `thunder * rain`.
    pub thunder: f32,
    /// `lastLightningBolt`.
    pub lightning: i32,
}

impl SkyWorld {
    pub fn rain_strength(&self) -> f32 {
        self.rain
    }

    /// `World.getThunderStrength`.
    pub fn thunder_strength(&self) -> f32 {
        self.thunder * self.rain
    }

    /// `WorldProvider.calculateCelestialAngle` de cada dimensão.
    pub fn celestial_angle(&self, pt: f32) -> f32 {
        match self.dimension {
            -1 => 0.5,
            1 => 0.0,
            _ => {
                let i = (self.time % 24000) as i32;
                let mut f = (i as f32 + pt) / 24000.0 - 0.25;
                if f < 0.0 {
                    f += 1.0;
                }
                if f > 1.0 {
                    f -= 1.0;
                }
                let f1 = 1.0 - ((((f as f64) * std::f64::consts::PI).cos() + 1.0) / 2.0) as f32;
                f + (f1 - f) / 3.0
            }
        }
    }

    /// `WorldProvider.getMoonPhase`.
    pub fn moon_phase(&self) -> i32 {
        ((self.time / 24000 % 8 + 8) % 8) as i32
    }

    /// `WorldProvider.isSurfaceWorld`.
    pub fn is_surface(&self) -> bool {
        self.dimension != -1 && self.dimension != 1
    }

    /// `WorldProvider.isSkyColored`.
    pub fn is_sky_colored(&self) -> bool {
        self.dimension != 1
    }

    /// `WorldProvider.getCloudHeight`.
    pub fn cloud_height(&self) -> f32 {
        if self.dimension == 1 {
            8.0
        } else {
            128.0
        }
    }

    /// `WorldType.getHorizon`.
    pub fn horizon(&self) -> f64 {
        if self.flat {
            0.0
        } else {
            63.0
        }
    }

    /// `WorldType.voidFadeMagnitude`.
    pub fn void_fade(&self) -> f64 {
        if self.flat {
            1.0
        } else {
            0.03125
        }
    }

    /// `WorldProvider.doesXZShowFog` (neblina do Nether).
    pub fn xz_shows_fog(&self) -> bool {
        self.dimension == -1
    }

    /// `WorldProvider.calcSunriseSunsetColors`.
    pub fn sunrise_colors(&self, angle: f32) -> Option<[f32; 4]> {
        if self.dimension == 1 {
            return None;
        }
        let f = 0.4f32;
        let f1 = cos(angle * PI * 2.0) - 0.0;
        let f2 = -0.0f32;
        if f1 >= f2 - f && f1 <= f2 + f {
            let f3 = (f1 - f2) / f * 0.5 + 0.5;
            let mut f4 = 1.0 - (1.0 - sin(f3 * PI)) * 0.99;
            f4 *= f4;
            Some([f3 * 0.3 + 0.7, f3 * f3 * 0.7 + 0.2, f3 * f3 * 0.0 + 0.2, f4])
        } else {
            None
        }
    }

    /// `WorldProvider.getFogColor(angle, pt)`.
    pub fn fog_color(&self, angle: f32) -> [f32; 3] {
        match self.dimension {
            -1 => [0.20000000298023224f64 as f32, 0.029999999329447746f64 as f32, 0.029999999329447746f64 as f32],
            1 => {
                let c = 10518688i32;
                let mut f = cos(angle * PI * 2.0) * 2.0 + 0.5;
                f = clamp(f, 0.0, 1.0);
                let r = ((c >> 16) & 255) as f32 / 255.0;
                let g = ((c >> 8) & 255) as f32 / 255.0;
                let b = (c & 255) as f32 / 255.0;
                [r * (f * 0.0 + 0.15), g * (f * 0.0 + 0.15), b * (f * 0.0 + 0.15)]
            }
            _ => {
                let f = clamp(cos(angle * PI * 2.0) * 2.0 + 0.5, 0.0, 1.0);
                [0.7529412f32 * (f * 0.94 + 0.06), 0.84705883f32 * (f * 0.94 + 0.06), 1.0f32 * (f * 0.91 + 0.09)]
            }
        }
    }

    /// `World.getSkyColorBody` com a cor do bioma (`getSkyBlendColour`) já calculada (0xRRGGBB).
    pub fn sky_color(&self, angle: f32, biome_rgb: i32, pt: f32) -> [f32; 3] {
        let f1 = clamp(cos(angle * PI * 2.0) * 2.0 + 0.5, 0.0, 1.0);
        let mut f3 = ((biome_rgb >> 16) & 255) as f32 / 255.0;
        let mut f4 = ((biome_rgb >> 8) & 255) as f32 / 255.0;
        let mut f5 = (biome_rgb & 255) as f32 / 255.0;
        f3 *= f1;
        f4 *= f1;
        f5 *= f1;
        let f6 = self.rain_strength();
        if f6 > 0.0 {
            let f7 = (f3 * 0.3 + f4 * 0.59 + f5 * 0.11) * 0.6;
            let f8 = 1.0 - f6 * 0.75;
            f3 = f3 * f8 + f7 * (1.0 - f8);
            f4 = f4 * f8 + f7 * (1.0 - f8);
            f5 = f5 * f8 + f7 * (1.0 - f8);
        }
        let f10 = self.thunder_strength();
        if f10 > 0.0 {
            let f11 = (f3 * 0.3 + f4 * 0.59 + f5 * 0.11) * 0.2;
            let f9 = 1.0 - f10 * 0.75;
            f3 = f3 * f9 + f11 * (1.0 - f9);
            f4 = f4 * f9 + f11 * (1.0 - f9);
            f5 = f5 * f9 + f11 * (1.0 - f9);
        }
        if self.lightning > 0 {
            let mut f12 = self.lightning as f32 - pt;
            if f12 > 1.0 {
                f12 = 1.0;
            }
            f12 *= 0.45;
            f3 = f3 * (1.0 - f12) + 0.8 * f12;
            f4 = f4 * (1.0 - f12) + 0.8 * f12;
            f5 = f5 * (1.0 - f12) + 1.0 * f12;
        }
        [f3, f4, f5]
    }

    /// `World.drawCloudsBody` (cor das nuvens; `cloudColour` fixo em 0xFFFFFF).
    pub fn cloud_color(&self, angle: f32) -> [f32; 3] {
        let f1 = clamp(cos(angle * PI * 2.0) * 2.0 + 0.5, 0.0, 1.0);
        let (mut f2, mut f3, mut f4) = (1.0f32, 1.0f32, 1.0f32);
        let f5 = self.rain_strength();
        if f5 > 0.0 {
            let f6 = (f2 * 0.3 + f3 * 0.59 + f4 * 0.11) * 0.6;
            let f7 = 1.0 - f5 * 0.95;
            f2 = f2 * f7 + f6 * (1.0 - f7);
            f3 = f3 * f7 + f6 * (1.0 - f7);
            f4 = f4 * f7 + f6 * (1.0 - f7);
        }
        f2 *= f1 * 0.9 + 0.1;
        f3 *= f1 * 0.9 + 0.1;
        f4 *= f1 * 0.85 + 0.15;
        let f9 = self.thunder_strength();
        if f9 > 0.0 {
            let f10 = (f2 * 0.3 + f3 * 0.59 + f4 * 0.11) * 0.2;
            let f8 = 1.0 - f9 * 0.95;
            f2 = f2 * f8 + f10 * (1.0 - f8);
            f3 = f3 * f8 + f10 * (1.0 - f8);
            f4 = f4 * f8 + f10 * (1.0 - f8);
        }
        [f2, f3, f4]
    }

    /// `World.getStarBrightnessBody`.
    pub fn star_brightness(&self, angle: f32) -> f32 {
        let f1 = clamp(1.0 - (cos(angle * PI * 2.0) * 2.0 + 0.25), 0.0, 1.0);
        f1 * f1 * 0.5
    }

    /// `World.getSunBrightnessBody`.
    pub fn sun_brightness(&self, angle: f32) -> f32 {
        let mut f1 = clamp(1.0 - (cos(angle * PI * 2.0) * 2.0 + 0.2), 0.0, 1.0);
        f1 = 1.0 - f1;
        f1 = (f1 as f64 * (1.0 - (self.rain_strength() * 5.0) as f64 / 16.0)) as f32;
        f1 = (f1 as f64 * (1.0 - (self.thunder_strength() * 5.0) as f64 / 16.0)) as f32;
        f1 * 0.8 + 0.2
    }

    /// `World.getSunBrightnessFactor`.
    pub fn sun_brightness_factor(&self, angle: f32) -> f32 {
        let mut f1 = clamp(1.0 - (cos(angle * PI * 2.0) * 2.0 + 0.5), 0.0, 1.0);
        f1 = 1.0 - f1;
        f1 = (f1 as f64 * (1.0 - (self.rain_strength() * 5.0) as f64 / 16.0)) as f32;
        f1 = (f1 as f64 * (1.0 - (self.thunder_strength() * 5.0) as f64 / 16.0)) as f32;
        f1
    }

    /// `World.calculateSkylightSubtracted`.
    pub fn skylight_subtracted(&self, pt: f32) -> i32 {
        let f = 1.0 - self.sun_brightness_factor(self.celestial_angle(pt));
        (f * 11.0) as i32
    }

    /// `WorldProvider.lightBrightnessTable`.
    pub fn light_table(&self) -> [f32; 16] {
        let base = if self.dimension == -1 { 0.1f32 } else { 0.0 };
        let mut t = [0f32; 16];
        for (i, v) in t.iter_mut().enumerate() {
            let f1 = 1.0 - i as f32 / 15.0;
            *v = (1.0 - f1) / (f1 * 3.0 + 1.0) * (1.0 - base) + base;
        }
        t
    }
}

fn temperature_noise() -> &'static Perlin1 {
    static N: OnceLock<Perlin1> = OnceLock::new();
    N.get_or_init(|| Perlin1::new(1234))
}

/// Bioma do id (desconhecido vira planície, como `Chunk.getBiome`).
fn biome(id: u8) -> (f32, f32, bool, bool, bool) {
    // (temperatura, chuva, pode_chover, neve, céu_zero) por id, da tabela única `gen_biomes::BIOMES`.
    static T: OnceLock<[Option<(f32, f32, bool, bool, bool)>; 256]> = OnceLock::new();
    let t = T.get_or_init(|| {
        let mut t = [None; 256];
        for b in BIOMES {
            t[b.0 as usize] = Some((b.2, b.3, b.7, b.8, b.9));
        }
        t
    });
    t[id as usize].unwrap_or_else(|| t[1].expect("planície"))
}

/// `BiomeGenBase.getFloatTemperature(BlockPos)`.
pub fn float_temperature(id: u8, x: i32, y: i32, z: i32) -> f32 {
    let t = biome(id).0;
    if y > 64 {
        let f = (temperature_noise().noise(x as f64 * 1.0 / 8.0, z as f64 * 1.0 / 8.0) * 4.0) as f32;
        t - (f + y as f32 - 64.0) * 0.05 / 30.0
    } else {
        t
    }
}

/// `BiomeGenBase.getSkyColorByTemp`.
pub fn biome_sky_color(id: u8, t: f32) -> i32 {
    // BiomeGenEnd.getSkyColorByTemp devolve 0 (única sobrescrita).
    if biome(id).4 {
        0
    } else {
        sky_color_by_temp(t)
    }
}

/// `BiomeGenBase.getSkyColorByTemp`.
pub fn sky_color_by_temp(t: f32) -> i32 {
    let t = clamp(t / 3.0, -1.0, 1.0);
    hsv_to_rgb(0.62222224 - t * 0.05, 0.5 + t * 0.1, 1.0)
}

/// `BiomeGenBase.canRain()`.
pub fn biome_can_rain(id: u8) -> bool {
    biome(id).2
}

/// `BiomeGenBase.canRain() || getEnableSnow()` e se neva (temperatura < 0.15).
pub fn biome_precipitates(id: u8) -> bool {
    let b = biome(id);
    b.2 || b.3
}

/// Raio do `ForgeHooksClient.getSkyBlendColour` (`ForgeModContainer.blendRanges`).
pub fn sky_blend_radius(render_distance_chunks: i32, fancy: bool) -> i32 {
    const RANGES: [i32; 17] = [2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30, 32, 34];
    if fancy && (0..17).contains(&render_distance_chunks) {
        RANGES[render_distance_chunks as usize]
    } else {
        0
    }
}

/// `ForgeHooksClient.getSkyBlendColour`: média da cor do céu dos biomas ao redor.
pub fn sky_blend_color(x: i32, y: i32, z: i32, radius: i32, biome_at: &dyn Fn(i32, i32) -> u8) -> i32 {
    let (mut r, mut g, mut b, mut n) = (0i32, 0i32, 0i32, 0i32);
    for dx in -radius..=radius {
        for dz in -radius..=radius {
            let (px, pz) = (x + dx, z + dz);
            let id = biome_at(px, pz);
            let c = biome_sky_color(id, float_temperature(id, px, y, pz));
            r += (c & 0xFF0000) >> 16;
            g += (c & 0xFF00) >> 8;
            b += c & 255;
            n += 1;
        }
    }
    ((r / n) & 255) << 16 | ((g / n) & 255) << 8 | (b / n) & 255
}

/// Material em que fica o olho (`ActiveRenderInfo.getBlockAtEntityViewpoint`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Medium {
    #[default]
    Air,
    Water,
    Lava,
}

/// `GlStateManager.setFog*`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FogMode {
    Linear { start: f32, end: f32 },
    Exp { density: f32 },
}

/// Efeitos e equipamento que mexem com a neblina e a luz.
#[derive(Clone, Copy, Debug, Default)]
pub struct Effects {
    /// Ticks restantes da cegueira.
    pub blindness: Option<i32>,
    /// Ticks restantes da visão noturna.
    pub night_vision: Option<i32>,
    pub water_breathing: bool,
    /// Nível de Respiração do capacete.
    pub respiration: i32,
}

/// `EntityRenderer.setupFog` (tipo -1 = céu, 0 = mundo).
pub fn setup_fog(kind: i32, far: f32, w: &SkyWorld, medium: Medium, fx: &Effects, eye_x: i32, eye_z: i32) -> FogMode {
    let _ = (eye_x, eye_z);
    if let Some(i) = fx.blindness {
        let mut f1 = 5.0f32;
        if i < 20 {
            f1 = 5.0 + (far - 5.0) * (1.0 - i as f32 / 20.0);
        }
        return if kind == -1 { FogMode::Linear { start: 0.0, end: f1 * 0.8 } } else { FogMode::Linear { start: f1 * 0.25, end: f1 } };
    }
    match medium {
        Medium::Water => {
            if fx.water_breathing {
                FogMode::Exp { density: 0.01 }
            } else {
                FogMode::Exp { density: 0.1 - fx.respiration as f32 * 0.03 }
            }
        }
        Medium::Lava => FogMode::Exp { density: 2.0 },
        Medium::Air => {
            let (mut start, mut end) = if kind == -1 { (0.0, far) } else { (far * 0.75, far) };
            if w.xz_shows_fog() {
                start = far * 0.05;
                end = far.min(192.0) * 0.5;
            }
            FogMode::Linear { start, end }
        }
    }
}

/// Entrada de `EntityRenderer.updateFogColor`.
pub struct FogIn<'a> {
    pub w: &'a SkyWorld,
    pub pt: f32,
    pub render_distance_chunks: i32,
    /// `entity.getLook(pt)`.
    pub look: [f32; 3],
    /// Cor do céu (`world.getSkyColor`).
    pub sky: [f32; 3],
    pub medium: Medium,
    pub fx: Effects,
    /// `fogColor2 + (fogColor1 - fogColor2) * pt`: brilho suavizado do local.
    pub smooth_brightness: f32,
    /// `entity.lastTickPosY + (posY - lastTickPosY) * pt`.
    pub eye_y: f64,
}

/// Valor de `getNightVisionBrightness`.
pub fn night_vision_brightness(ticks: i32, pt: f32) -> f32 {
    if ticks > 200 {
        1.0
    } else {
        0.7 + sin((ticks as f32 - pt) * PI * 0.2) * 0.3
    }
}

/// `EntityRenderer.updateFogColor`: cor de fundo e da neblina.
pub fn update_fog_color(i: &FogIn) -> [f32; 3] {
    let w = i.w;
    let pt = i.pt;
    let rd = i.render_distance_chunks as f32;
    let mut f = 0.25 + 0.75 * rd / 32.0;
    f = 1.0 - (f as f64).powf(0.25) as f32;
    let angle = w.celestial_angle(pt);
    let [f1, f2, f3] = i.sky;
    let [mut r, mut g, mut b] = w.fog_color(angle);
    if i.render_distance_chunks >= 4 {
        let d0 = if sin(angle * PI * 2.0) > 0.0 { -1.0f64 } else { 1.0 };
        let mut f5 = (i.look[0] as f64 * d0) as f32;
        if f5 < 0.0 {
            f5 = 0.0;
        }
        if f5 > 0.0 {
            if let Some(a) = w.sunrise_colors(angle) {
                f5 *= a[3];
                r = r * (1.0 - f5) + a[0] * f5;
                g = g * (1.0 - f5) + a[1] * f5;
                b = b * (1.0 - f5) + a[2] * f5;
            }
        }
    }
    r += (f1 - r) * f;
    g += (f2 - g) * f;
    b += (f3 - b) * f;
    let f8 = w.rain_strength();
    if f8 > 0.0 {
        let f4 = 1.0 - f8 * 0.5;
        let f10 = 1.0 - f8 * 0.4;
        r *= f4;
        g *= f4;
        b *= f10;
    }
    let f9 = w.thunder_strength();
    if f9 > 0.0 {
        let f11 = 1.0 - f9 * 0.5;
        r *= f11;
        g *= f11;
        b *= f11;
    }
    match i.medium {
        Medium::Water => {
            let mut f12 = i.fx.respiration as f32 * 0.2;
            if i.fx.water_breathing {
                f12 = f12 * 0.3 + 0.6;
            }
            r = 0.02 + f12;
            g = 0.02 + f12;
            b = 0.2 + f12;
        }
        Medium::Lava => {
            r = 0.6;
            g = 0.1;
            b = 0.0;
        }
        Medium::Air => {}
    }
    let f13 = i.smooth_brightness;
    r *= f13;
    g *= f13;
    b *= f13;
    let mut d1 = i.eye_y * w.void_fade();
    if let Some(t) = i.fx.blindness {
        if t < 20 {
            d1 *= (1.0 - t as f32 / 20.0) as f64;
        } else {
            d1 = 0.0;
        }
    }
    if d1 < 1.0 {
        if d1 < 0.0 {
            d1 = 0.0;
        }
        d1 *= d1;
        r = (r as f64 * d1) as f32;
        g = (g as f64 * d1) as f32;
        b = (b as f64 * d1) as f32;
    }
    if let Some(t) = i.fx.night_vision {
        let f15 = night_vision_brightness(t, pt);
        let mut f6 = 1.0 / r;
        if f6 > 1.0 / g {
            f6 = 1.0 / g;
        }
        if f6 > 1.0 / b {
            f6 = 1.0 / b;
        }
        r = r * (1.0 - f15) + r * f6 * f15;
        g = g * (1.0 - f15) + g * f6 * f15;
        b = b * (1.0 - f15) + b * f6 * f15;
    }
    [r, g, b]
}

/// `EntityRenderer.updateLightmap`: 256 cores (índice = céu * 16 + bloco), cada canal em
/// 0..=255 como no `lightmapColors` do jogo.
pub fn lightmap(w: &SkyWorld, flicker: f32, gamma: f32, night_vision: Option<i32>, pt: f32) -> [[u8; 3]; 256] {
    let sun = w.sun_brightness(w.celestial_angle(1.0));
    lightmap_core(w, sun, flicker, gamma, night_vision, pt)
}

/// Núcleo do mapa de luz para um brilho de sol dado (`world.getSunBrightness(1.0)`).
pub fn lightmap_core(w: &SkyWorld, sun: f32, flicker: f32, gamma: f32, night_vision: Option<i32>, pt: f32) -> [[u8; 3]; 256] {
    let table = w.light_table();
    let f1 = sun * 0.95 + 0.05;
    let mut out = [[0u8; 3]; 256];
    for (i, px) in out.iter_mut().enumerate() {
        let mut f2 = table[i / 16] * f1;
        let f3 = table[i % 16] * (flicker * 0.1 + 1.5);
        if w.lightning > 0 {
            f2 = table[i / 16];
        }
        let f4 = f2 * (sun * 0.65 + 0.35);
        let f5 = f2 * (sun * 0.65 + 0.35);
        let f6 = f3 * ((f3 * 0.6 + 0.4) * 0.6 + 0.4);
        let f7 = f3 * (f3 * f3 * 0.6 + 0.4);
        let mut f8 = f4 + f3;
        let mut f9 = f5 + f6;
        let mut f10 = f2 + f7;
        f8 = f8 * 0.96 + 0.03;
        f9 = f9 * 0.96 + 0.03;
        f10 = f10 * 0.96 + 0.03;
        if w.dimension == 1 {
            f8 = 0.22 + f3 * 0.75;
            f9 = 0.28 + f6 * 0.75;
            f10 = 0.25 + f7 * 0.75;
        }
        if let Some(t) = night_vision {
            let f15 = night_vision_brightness(t, pt);
            let mut f12 = 1.0 / f8;
            if f12 > 1.0 / f9 {
                f12 = 1.0 / f9;
            }
            if f12 > 1.0 / f10 {
                f12 = 1.0 / f10;
            }
            f8 = f8 * (1.0 - f15) + f8 * f12 * f15;
            f9 = f9 * (1.0 - f15) + f9 * f12 * f15;
            f10 = f10 * (1.0 - f15) + f10 * f12 * f15;
        }
        f8 = f8.min(1.0);
        f9 = f9.min(1.0);
        f10 = f10.min(1.0);
        let f17 = 1.0 - f8;
        let f13 = 1.0 - f9;
        let f14 = 1.0 - f10;
        let f17 = 1.0 - f17 * f17 * f17 * f17;
        let f13 = 1.0 - f13 * f13 * f13 * f13;
        let f14 = 1.0 - f14 * f14 * f14 * f14;
        f8 = f8 * (1.0 - gamma) + f17 * gamma;
        f9 = f9 * (1.0 - gamma) + f13 * gamma;
        f10 = f10 * (1.0 - gamma) + f14 * gamma;
        f8 = f8 * 0.96 + 0.03;
        f9 = f9 * 0.96 + 0.03;
        f10 = f10 * 0.96 + 0.03;
        let c = |v: f32| (v.clamp(0.0, 1.0) * 255.0) as u8;
        *px = [c(f8), c(f9), c(f10)];
    }
    out
}

/// `entity.getLook` (`getVectorForRotation`), com a tabela de seno do jogo.
pub fn look_vector(pitch: f32, yaw: f32) -> [f32; 3] {
    let f = cos(-yaw * 0.017453292 - PI);
    let f1 = sin(-yaw * 0.017453292 - PI);
    let f2 = -cos(-pitch * 0.017453292);
    let f3 = sin(-pitch * 0.017453292);
    [f1 * f2, f3, f * f2]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits(s: &str) -> f32 {
        f32::from_bits(u32::from_str_radix(s, 16).unwrap())
    }

    fn close(a: f32, b: f32, what: &str, ctx: &str) {
        assert!((a - b).abs() <= 1e-6 + 1e-6 * b.abs(), "{what}: Rust {a} ({:08x}) != Java {b} ({:08x}) em {ctx}", a.to_bits(), b.to_bits());
    }

    /// Mesma regra de GenCeu.biomeAt.
    fn biome_at(x: i32, z: i32) -> u8 {
        let ids: Vec<u8> = BIOMES.iter().map(|b| b.0).collect();
        let k = ((x >> 3) * 31 + (z >> 3) * 17).rem_euclid(ids.len() as i32);
        ids[k as usize]
    }

    #[test]
    fn contra_o_java() {
        let golden = include_str!("sky_golden.txt");
        let (mut n_env, mut n_sky, mut n_biome, mut n_table, mut n_lm) = (0, 0, 0, 0, 0);
        let mut max_diff = 0f32;
        for line in golden.lines() {
            let p: Vec<&str> = line.split_whitespace().collect();
            match p.first().copied() {
                Some("biome") => {
                    let id: u8 = p[1].parse().unwrap();
                    let (x, y, z): (i32, i32, i32) = (p[2].parse().unwrap(), p[3].parse().unwrap(), p[4].parse().unwrap());
                    let t = float_temperature(id, x, y, z);
                    close(t, bits(p[5]), "temperatura", line);
                    assert_eq!(biome_sky_color(id, bits(p[5])), p[6].parse::<i32>().unwrap(), "{line}");
                    n_biome += 1;
                }
                Some("table") => {
                    let w = SkyWorld { dimension: p[1].parse().unwrap(), ..Default::default() };
                    for (i, v) in w.light_table().iter().enumerate() {
                        close(*v, bits(p[2 + i]), "tabela de brilho", line);
                    }
                    n_table += 1;
                }
                Some("env") => {
                    let w = SkyWorld {
                        dimension: p[1].parse().unwrap(),
                        time: p[2].parse().unwrap(),
                        rain: bits(p[4]),
                        thunder: bits(p[5]),
                        lightning: p[6].parse().unwrap(),
                        flat: false,
                    };
                    let pt = bits(p[3]);
                    let angle = w.celestial_angle(pt);
                    close(angle, bits(p[7]), "ângulo", line);
                    let sr = w.sunrise_colors(angle);
                    assert_eq!(sr.is_some(), p[8] == "1", "nascer/pôr do sol em {line}");
                    if let Some(a) = sr {
                        for k in 0..4 {
                            close(a[k], bits(p[9 + k]), "cor do pôr do sol", line);
                        }
                    }
                    let fog = w.fog_color(angle);
                    let cloud = w.cloud_color(angle);
                    for k in 0..3 {
                        close(fog[k], bits(p[13 + k]), "neblina", line);
                        close(cloud[k], bits(p[16 + k]), "nuvens", line);
                    }
                    close(w.star_brightness(angle), bits(p[19]), "estrelas", line);
                    close(w.sun_brightness(w.celestial_angle(1.0)), bits(p[20]), "brilho do sol", line);
                    assert_eq!(w.skylight_subtracted(pt), p[21].parse::<i32>().unwrap(), "luz do céu subtraída em {line}");
                    close(w.sun_brightness_factor(angle), bits(p[22]), "fator do sol", line);
                    assert_eq!(w.moon_phase(), p[23].parse::<i32>().unwrap(), "fase da lua em {line}");
                    close(w.rain_strength(), bits(p[24]), "chuva", line);
                    close(w.thunder_strength(), bits(p[25]), "trovão", line);
                    n_env += 1;
                }
                Some("sky") => {
                    let w = SkyWorld {
                        dimension: p[1].parse().unwrap(),
                        time: p[2].parse().unwrap(),
                        rain: bits(p[4]),
                        thunder: bits(p[5]),
                        lightning: p[6].parse().unwrap(),
                        flat: false,
                    };
                    let pt = bits(p[3]);
                    let (x, y, z): (f64, f64, f64) = (p[7].parse().unwrap(), p[8].parse().unwrap(), p[9].parse().unwrap());
                    let (bx, by, bz) = (x.floor() as i32, y.floor() as i32, z.floor() as i32);
                    let rgb = sky_blend_color(bx, by, bz, sky_blend_radius(10, true), &biome_at);
                    let c = w.sky_color(w.celestial_angle(pt), rgb, pt);
                    for k in 0..3 {
                        let exp = bits(p[10 + k]);
                        max_diff = max_diff.max((c[k] - exp).abs());
                        close(c[k], exp, "cor do céu", line);
                    }
                    n_sky += 1;
                }
                Some("lightmap") => {
                    let sun = bits(p[1]);
                    let (flicker, gamma) = (bits(p[2]), bits(p[3]));
                    let dim: i32 = p[4].parse().unwrap();
                    let lightning = p[5] == "1";
                    // Reconstrói um mundo cuja luz do sol é `sun` (ângulo que dá esse brilho).
                    // O mapa de luz só depende de sun_brightness; testa a parte aritmética.
                    let w = SkyWorld { dimension: dim, lightning: lightning as i32, ..Default::default() };
                    let lm = lightmap_core(&w, sun, flicker, gamma, None, 1.0);
                    for (k, s) in p[7..].iter().enumerate() {
                        let idx = [0usize, 15, 16, 17, 100, 240, 255][k];
                        let v: Vec<u8> = s.split(',').map(|x| x.parse().unwrap()).collect();
                        assert_eq!(lm[idx].to_vec(), v, "mapa de luz [{idx}] em {line}");
                    }
                    n_lm += 1;
                }
                _ => {}
            }
        }
        println!("céu: {n_env} env, {n_sky} cores de céu (maior diferença {max_diff:e}), {n_biome} biomas, {n_table} tabelas, {n_lm} mapas de luz conferidos com o Java");
        assert!(n_env > 1000 && n_sky > 3000 && n_biome > 100 && n_table == 3 && n_lm > 50);
    }

    #[test]
    fn estrelas_do_java() {
        let golden = include_str!("sky_golden.txt");
        let mut stars = None;
        let mut first = Vec::new();
        let mut sum = 0f64;
        for line in golden.lines() {
            let p: Vec<&str> = line.split_whitespace().collect();
            match p.first().copied() {
                Some("stars") => stars = Some(p[1].parse::<usize>().unwrap()),
                Some("star") => first.push([bits(p[2]), bits(p[3]), bits(p[4])]),
                Some("starsum") => sum = p[1].parse().unwrap(),
                _ => {}
            }
        }
        let v = crate::skygeo::star_vertices();
        assert_eq!(v.len(), stars.unwrap());
        for (i, f) in first.iter().enumerate() {
            for k in 0..3 {
                close(v[i][k], f[k], "vértice de estrela", &format!("{i}"));
            }
        }
        let s: f64 = v.iter().map(|p| p[0] as f64 + p[1] as f64 * 3.0 + p[2] as f64 * 7.0).sum();
        assert!((s - sum).abs() < 1e-2, "soma {s} != {sum}");
    }

    #[test]
    fn horas_conhecidas() {
        // Meio-dia (6000): ângulo 0, céu claro de planície, sol a pino.
        let w = SkyWorld { time: 6000, ..Default::default() };
        assert_eq!(w.celestial_angle(0.0), 0.0);
        assert_eq!(w.skylight_subtracted(0.0), 0);
        // Meia-noite (18000): ângulo 0.5, lua cheia no zênite oposto, luz do céu -11.
        let w = SkyWorld { time: 18000, ..Default::default() };
        assert!((w.celestial_angle(0.0) - 0.5).abs() < 1e-6);
        assert_eq!(w.skylight_subtracted(0.0), 11);
        assert_eq!(SkyWorld { time: 24000 * 5 + 1, ..Default::default() }.moon_phase(), 5);
        assert_eq!(SkyWorld { time: 24000 * 8, ..Default::default() }.moon_phase(), 0);
        // Pôr do sol (12000) tem cores; meio-dia não.
        let w = SkyWorld { time: 12000, ..Default::default() };
        assert!(w.sunrise_colors(w.celestial_angle(0.0)).is_some());
        let w = SkyWorld { time: 6000, ..Default::default() };
        assert!(w.sunrise_colors(w.celestial_angle(0.0)).is_none());
    }
}
