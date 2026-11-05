//! Atmosfera do cliente: liga o mundo (biomas, hora, clima, bloco no olho) às contas de céu,
//! neblina, nuvens e mapa de luz de `mc_render::sky` (portadas de WorldProvider/World/
//! EntityRenderer da 1.8.9).

use mc_render::sky::{self, Effects, FogIn, FogMode, Medium, SkyWorld};
use glam::{Mat4, Vec3};
use mc_render::skygeo::{cloud_draws, sky_draws, weather_draws, CloudFrame, SkyFrame, WeatherFrame, WeatherWorld};
use mc_render::{Sky, SkyDraw};
use mc_world::blocks::BlockTable;
use mc_world::chunk::Section;
use mc_world::World;

use crate::rain::{add_rain_particles, RainFx};

/// Entrada de um quadro.
pub struct FrameIn {
    /// Fração do tick (partialTicks).
    pub pt: f32,
    /// Pés do jogador, já interpolados.
    pub feet: [f64; 3],
    /// Posição do tick (sem interpolar): `entity.posX/Y/Z`.
    pub pos: [f64; 3],
    pub eye_height: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub render_distance: i32,
    /// Opção "Distância LOD" ligada: fim da neblina do mundo em blocos (0 = a distância de renderização).
    pub lod_far: f32,
    pub fancy: bool,
    /// Opção de nuvens: 0 desligadas, 1 rápidas, 2 fancy.
    pub clouds: i32,
    pub gamma: f32,
    pub fx: Effects,
}

pub struct FrameOut {
    pub sky: Sky,
    pub sky_draws: Vec<SkyDraw>,
    /// Chuva e neve (`renderRainSnow`).
    pub weather_draws: Vec<SkyDraw>,
    pub late_draws: Vec<SkyDraw>,
}

/// O mundo carregado visto por `renderRainSnow`.
struct Ww<'a>(&'a World);

impl WeatherWorld for Ww<'_> {
    fn biome(&self, x: i32, z: i32) -> u8 {
        self.0.biome_at(x, z)
    }

    fn precipitation_height(&self, x: i32, z: i32) -> i32 {
        self.0.precipitation_height(x, z)
    }

    /// `getLightFromNeighborsFor` (sem a regra dos blocos que olham os vizinhos), luz crua do chunk.
    fn light(&self, x: i32, y: i32, z: i32) -> (u8, u8) {
        if !(0..256).contains(&y) {
            return (if self.0.has_sky { 15 } else { 0 }, 0);
        }
        let Some(col) = self.0.columns.get(&(x >> 4, z >> 4)) else { return (0, 0) };
        match &col.sections[(y >> 4) as usize] {
            Some(s) => {
                let l = s.light(Section::index(x & 15, y & 15, z & 15));
                (l >> 4, l & 15)
            }
            None => (if self.0.has_sky { 15 } else { 0 }, 0),
        }
    }
}

pub struct Atmosphere {
    /// OptiFine do jogador: névoa, céu, nuvens e chuva desligados.
    pub vis: crate::visuals::Visuals,
    pub world: SkyWorld,
    /// Mistura de cor do céu em cache: (posição, raio, `World.biome_rev`) -> cor.
    blend: Option<((i32, i32, i32, i32, u32), i32)>,
    flicker: f32,
    flicker_d: f32,
    fog1: f32,
    fog2: f32,
    cloud_ticks: i32,
    /// `EntityRenderer.rendererUpdateCount`.
    renderer_ticks: i32,
    rng: u64,
    /// `EntityRenderer.rainSoundCounter`.
    rain_sound_counter: i32,
}

impl Default for Atmosphere {
    fn default() -> Self {
        Self { vis: Default::default(), world: SkyWorld { time: 6000, ..Default::default() }, blend: None, flicker: 0.0, flicker_d: 0.0, fog1: 0.0, fog2: 0.0, cloud_ticks: 0, renderer_ticks: 0, rng: 0x2545F4914F6CDD1D, rain_sound_counter: 0 }
    }
}

impl Atmosphere {
    /// Novo mundo (entrada ou troca de dimensão).
    pub fn reset(&mut self, dimension: i32) {
        self.world = SkyWorld { dimension, time: self.world.time, ..Default::default() };
        self.blend = None;
    }

    /// Só para o modo de teste sem servidor: brilho do local já estabilizado em céu aberto.
    pub fn open_sky_for_test(&mut self) {
        self.fog1 = 1.0;
        self.fog2 = 1.0;
    }

    pub fn set_time(&mut self, time: i64) {
        self.world.time = time.abs();
    }

    /// `S2BPacketChangeGameState` (NetHandlerPlayClient.handleChangeGameState).
    pub fn game_state(&mut self, reason: u8, value: f32) {
        // ofWeather:false / mod_weather: chuva e trovão ficam em 0 (nem o céu escurece).
        if !self.vis.weather {
            self.world.rain = 0.0;
            self.world.thunder = 0.0;
            return;
        }
        match reason {
            1 => self.world.rain = 0.0,
            2 => self.world.rain = 1.0,
            7 => self.world.rain = value,
            8 => self.world.thunder = value,
            _ => {}
        }
    }

    fn random(&mut self) -> f64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Um tick de `EntityRenderer.updateRenderer` (cintilar das tochas, brilho da neblina) e
    /// de `RenderGlobal.updateClouds`.
    pub fn tick(&mut self, world: &World, feet: [f64; 3], render_distance: i32) {
        let (a, b, c) = (self.random(), self.random(), self.random());
        let d = self.random();
        self.flicker_d = (self.flicker_d as f64 + (a - b) * c * d) as f32;
        self.flicker_d = (self.flicker_d as f64 * 0.9) as f32;
        self.flicker += (self.flicker_d - self.flicker) * 1.0;
        self.renderer_ticks = self.renderer_ticks.wrapping_add(1);
        self.fog2 = self.fog1;
        let (x, y, z) = (feet[0].floor() as i32, feet[1].floor() as i32, feet[2].floor() as i32);
        if std::env::var_os("MINE_RS_CEU").is_some() {
            return;
        }
        let f3 = self.world.light_table()[Self::light_at(world, x, y, z, self.world.skylight_subtracted(1.0)) as usize];
        let f4 = render_distance as f32 / 32.0;
        let f2 = f3 * (1.0 - f4) + f4;
        self.fog1 += (f2 - self.fog1) * 0.1;
        self.cloud_ticks += 1;
    }

    /// `EntityRenderer.addRainParticles`, logo depois do `++rendererUpdateCount` do tick.
    pub fn rain_fx(&mut self, world: &World, table: &BlockTable, feet: [f64; 3], fancy: bool, setting: i32) -> Vec<RainFx> {
        if !(self.vis.rain && self.vis.weather) {
            return Vec::new();
        }
        let tick = self.renderer_ticks;
        let strength = self.world.rain_strength();
        add_rain_particles(world, table, tick, feet, strength, fancy, setting, &mut self.rain_sound_counter)
    }

    /// `World.getLightFromNeighbors` sem a regra dos blocos que olham os vizinhos.
    fn light_at(world: &World, x: i32, y: i32, z: i32, subtracted: i32) -> i32 {
        if !(0..256).contains(&y) {
            return 15;
        }
        let Some(col) = world.columns.get(&(x >> 4, z >> 4)) else { return 0 };
        match &col.sections[(y >> 4) as usize] {
            Some(s) => {
                let l = s.light(Section::index(x & 15, y & 15, z & 15)) as i32;
                let (sky, blk) = (l >> 4, l & 15);
                blk.max(if world.has_sky { sky - subtracted } else { 0 }).clamp(0, 15)
            }
            None => 15 - subtracted.min(15),
        }
    }

    /// `ActiveRenderInfo.getBlockAtEntityViewpoint`.
    fn medium_at(world: &World, p: [f64; 3]) -> Medium {
        let mat = |s: u16| match s >> 4 {
            8 | 9 => Medium::Water,
            10 | 11 => Medium::Lava,
            _ => Medium::Air,
        };
        let (x, y, z) = (p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32);
        let s = world.block(x, y, z);
        let m = mat(s);
        if m != Medium::Air {
            let meta = (s & 15) as i32;
            let lh = ((if meta >= 8 { 0 } else { meta }) + 1) as f32 / 9.0 - 0.11111111;
            if p[1] >= (y + 1) as f64 - lh as f64 {
                return mat(world.block(x, y + 1, z));
            }
        }
        m
    }

    /// Tudo que o renderizador precisa neste quadro.
    pub fn frame(&mut self, world: &World, i: &FrameIn) -> FrameOut {
        let w = self.world;
        let pt = i.pt;
        let eye = [i.feet[0], i.feet[1] + i.eye_height, i.feet[2]];
        let angle = w.celestial_angle(pt);
        // World.getSkyColorBody: o bloco dos pés da entidade (posX, posY, posZ), não o do olho.
        let (bx, by, bz) = (i.feet[0].floor() as i32, i.feet[1].floor() as i32, i.feet[2].floor() as i32);
        let radius = sky::sky_blend_radius(i.render_distance, i.fancy);
        let key = (bx, by, bz, radius, world.biome_rev);
        let rgb = match self.blend {
            Some((k, c)) if k == key => c,
            _ => {
                let c = sky::sky_blend_color(bx, by, bz, radius, &|x, z| world.biome_at(x, z));
                self.blend = Some((key, c));
                c
            }
        };
        let sky_color = w.sky_color(angle, rgb, pt);
        let medium = Self::medium_at(world, eye);
        let far = (i.render_distance * 16) as f32;
        let fog_color = sky::update_fog_color(&FogIn {
            w: &w,
            pt,
            render_distance_chunks: i.render_distance,
            look: sky::look_vector(i.pitch, i.yaw),
            sky: sky_color,
            medium,
            fx: i.fx,
            smooth_brightness: self.fog2 + (self.fog1 - self.fog2) * pt,
            eye_y: i.feet[1],
        });
        // ofFogType:3 tira a névoa do ar (a cegueira continua).
        let vis = self.vis;
        let fog_medium = medium;
        let no_fog = !vis.fog && fog_medium == Medium::Air && i.fx.blindness.is_none();
        let enc = |m: FogMode| match m {
            _ if no_fog => [0.0, 0.0, 0.0, -1.0],
            FogMode::Linear { start, end } => [start, end, 0.0, 0.0],
            FogMode::Exp { density } => [0.0, 1.0, density, 1.0],
        };
        let (ex, ez) = (i.feet[0] as i32, i.feet[2] as i32);
        let sky = Sky {
            fog_color,
            fog_world: enc(sky::setup_fog(0, far.max(i.lod_far), &w, fog_medium, &i.fx, ex, ez)),
            fog_sky: enc(sky::setup_fog(-1, far, &w, fog_medium, &i.fx, ex, ez)),
            lightmap: sky::lightmap(&w, self.flicker, i.gamma, i.fx.night_vision, pt),
        };

        let mut sky_list = Vec::new();
        let mut late = Vec::new();
        if i.render_distance >= 4 {
            sky_list = sky_draws(&SkyFrame {
                dimension: w.dimension,
                surface: w.is_surface(),
                sky_colored: w.is_sky_colored(),
                sky_color,
                angle,
                rain: w.rain_strength(),
                star_brightness: w.star_brightness(angle),
                moon_phase: w.moon_phase(),
                sunrise: w.sunrise_colors(angle),
                eye_above_horizon: eye[1] - w.horizon(),
            });
            // OptiFine: domo (com a faixa do pôr do sol e o plano de baixo), sol e lua, estrelas.
            use mc_render::skygeo::{SkyBlend, SkyTex};
            sky_list.retain(|d| match (d.tex, d.blend) {
                (SkyTex::Sun | SkyTex::Moon, _) => vis.sun_moon,
                (SkyTex::White, SkyBlend::Additive) => vis.stars,
                (SkyTex::White, _) => vis.sky,
                _ => true,
            });
            if i.clouds != 0 && vis.clouds && w.is_surface() {
                let c = cloud_draws(&CloudFrame {
                    mode: i.clouds as u8,
                    color: w.cloud_color(angle),
                    eye_y: i.feet[1] as f32,
                    cloud_height: w.cloud_height(),
                    ticks: (self.cloud_ticks as f32 + pt) as f64,
                    x: i.feet[0],
                    z: i.feet[2],
                });
                if eye[1] < 128.0 {
                    sky_list.extend(c);
                } else {
                    late = c;
                }
            }
        }
        let mut weather = if !(vis.rain && vis.weather) { Vec::new() } else { weather_draws(
            &WeatherFrame { strength: w.rain_strength(), fancy: i.fancy, ticks: self.renderer_ticks, pt, pos: i.pos, feet: i.feet, lightmap: &sky.lightmap },
            &Ww(world),
        ) };
        // `orientCamera` termina com `translate(0, -eyeHeight, 0)`: o mundo, o céu e as nuvens do Java
        // ficam com a origem nos pés e a câmera na altura dos olhos.
        let shift = Mat4::from_translation(Vec3::new(0.0, -i.eye_height as f32, 0.0));
        for d in sky_list.iter_mut().chain(late.iter_mut()).chain(weather.iter_mut()) {
            d.model = shift * d.model;
        }
        FrameOut { sky, sky_draws: sky_list, weather_draws: weather, late_draws: late }
    }
}
