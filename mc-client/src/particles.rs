//! Partículas da 1.8.9: `EffectRenderer` e as subclasses de `EntityFX`, com os números do Java
//! (`net.minecraft.client.particle.*`), mais os pacotes S2A/S27/S28/S0B e o `randomDisplayTick`.

use std::collections::{HashMap, VecDeque};

use glam::DVec3;
use mc_render::particle_pass::MIRRORED;
use mc_render::{Camera, ParticleBatch, ParticleDraw, ParticleInstance, Tex};
use mc_world::blocks::BlockTable;
use mc_world::gen_shapes::{F_LIQUID, F_SOLID, FLAGS};
use mc_world::mesh::ATLAS_COLS;
use mc_world::World;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rayon::prelude::*;

/// A partir de quantas partículas por camada o tick e a montagem do quadro rodam em paralelo.
const PAR_MIN: usize = 256;

/// Literal `float` do Java promovido a `double` (`0.98F` vira `0.9800000190734863D`).
const fn d(x: f32) -> f64 {
    x as f64
}
const F07: f64 = d(0.7);
const F098: f64 = d(0.98);
const F096: f64 = d(0.96);

/// Blocos com `getRenderType() == -1` (ar, pistão em movimento, placas, portal do fim, cabeças, barreira, bandeiras).
const NO_RENDER: [u16; 9] = [0, 36, 63, 68, 119, 144, 166, 176, 177];

/// `EnumParticleTypes`: (id, ignora distância, quantidade de argumentos).
pub fn particle_info(id: i32) -> (bool, usize) {
    match id {
        0..=2 | 41 => (true, 0),
        36 => (false, 2),
        37 | 38 => (false, 1),
        _ => (false, 0),
    }
}

// ---------------------------------------------------------------------------------------------
// Caixas e colisão (`Entity.moveEntity`, sem degrau: `stepHeight` das partículas é 0)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: DVec3,
    pub max: DVec3,
}

impl Aabb {
    fn offset(self, v: DVec3) -> Self {
        Self { min: self.min + v, max: self.max + v }
    }
    fn add_coord(self, v: DVec3) -> Self {
        Self { min: self.min + v.min(DVec3::ZERO), max: self.max + v.max(DVec3::ZERO) }
    }
    fn intersects(&self, o: &Aabb) -> bool {
        self.min.x < o.max.x && self.max.x > o.min.x && self.min.y < o.max.y && self.max.y > o.min.y && self.min.z < o.max.z && self.max.z > o.min.z
    }
    fn clip_x(&self, o: &Aabb, mut v: f64) -> f64 {
        if o.max.y > self.min.y && o.min.y < self.max.y && o.max.z > self.min.z && o.min.z < self.max.z {
            if v > 0.0 && o.max.x <= self.min.x {
                v = v.min(self.min.x - o.max.x);
            } else if v < 0.0 && o.min.x >= self.max.x {
                v = v.max(self.max.x - o.min.x);
            }
        }
        v
    }
    fn clip_y(&self, o: &Aabb, mut v: f64) -> f64 {
        if o.max.x > self.min.x && o.min.x < self.max.x && o.max.z > self.min.z && o.min.z < self.max.z {
            if v > 0.0 && o.max.y <= self.min.y {
                v = v.min(self.min.y - o.max.y);
            } else if v < 0.0 && o.min.y >= self.max.y {
                v = v.max(self.max.y - o.min.y);
            }
        }
        v
    }
    fn clip_z(&self, o: &Aabb, mut v: f64) -> f64 {
        if o.max.x > self.min.x && o.min.x < self.max.x && o.max.y > self.min.y && o.min.y < self.max.y {
            if v > 0.0 && o.max.z <= self.min.z {
                v = v.min(self.min.z - o.max.z);
            } else if v < 0.0 && o.min.z >= self.max.z {
                v = v.max(self.max.z - o.min.z);
            }
        }
        v
    }
}

/// Entidade vista pelas partículas (emissores de crítico).
#[derive(Clone, Copy)]
pub struct EntView {
    pub pos: DVec3,
    pub width: f32,
    pub height: f32,
}

/// O que as partículas enxergam do jogo.
pub struct Env<'a> {
    pub world: &'a World,
    pub table: &'a BlockTable,
    /// Jogador local: pés e velocidade (`RenderViewEntity`).
    pub player: (DVec3, DVec3),
    pub entity: &'a dyn Fn(i32) -> Option<EntView>,
}

fn fl(v: f64) -> i32 {
    v.floor() as i32
}

impl Env<'_> {
    fn st(&self, x: i32, y: i32, z: i32) -> u16 {
        self.world.block(x, y, z)
    }

    /// `World.getCollidingBoundingBoxes` só com blocos.
    fn colliding(&self, area: &Aabb) -> Vec<Aabb> {
        let mut out = Vec::new();
        let mut tmp: Vec<[f64; 6]> = Vec::new();
        let (i, j) = (fl(area.min.x), fl(area.max.x + 1.0));
        let (k, l) = (fl(area.min.y), fl(area.max.y + 1.0));
        let (i1, j1) = (fl(area.min.z), fl(area.max.z + 1.0));
        for x in i..j {
            for z in i1..j1 {
                if !self.world.is_loaded(x, z) {
                    continue;
                }
                for y in (k - 1)..l {
                    let st = self.st(x, y, z);
                    if st == 0 {
                        continue;
                    }
                    tmp.clear();
                    self.table.collision(st, &|dx, dy, dz| self.st(x + dx, y + dy, z + dz), &mut tmp);
                    let o = DVec3::new(x as f64, y as f64, z as f64);
                    for b in &tmp {
                        let bb = Aabb { min: o + DVec3::new(b[0], b[1], b[2]), max: o + DVec3::new(b[3], b[4], b[5]) };
                        if bb.intersects(area) {
                            out.push(bb);
                        }
                    }
                }
            }
        }
        out
    }

    /// `World.getLightFor`.
    fn light_for(&self, sky: bool, x: i32, y: i32, z: i32) -> i32 {
        let y = y.max(0);
        let default = if sky { 15 } else { 0 };
        if y > 255 || !self.world.is_loaded(x, z) {
            return default;
        }
        match self.world.columns.get(&(x >> 4, z >> 4)).and_then(|c| c.sections[(y >> 4) as usize].as_ref()) {
            None => {
                if sky && self.world.has_sky {
                    15
                } else {
                    0
                }
            }
            Some(s) => {
                let l = s.light(mc_world::Section::index(x & 15, y & 15, z & 15)) as i32;
                if sky {
                    l >> 4
                } else {
                    l & 15
                }
            }
        }
    }

    /// `World.getLightFromNeighborsFor`.
    fn light_nb(&self, sky: bool, x: i32, y: i32, z: i32) -> i32 {
        if sky && !self.world.has_sky {
            return 0;
        }
        let y = y.max(0);
        if y > 255 || !self.world.is_loaded(x, z) {
            return if sky { 15 } else { 0 };
        }
        if self.table.neighbor_light(self.st(x, y, z)) {
            [(0, 1, 0), (1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1)]
                .iter()
                .map(|(dx, dy, dz)| self.light_for(sky, x + dx, y + dy, z + dz))
                .max()
                .unwrap_or(0)
        } else {
            self.light_for(sky, x, y, z)
        }
    }

    /// `World.getCombinedLight(pos, 0)` como (céu, bloco), de 0 a 15.
    pub fn combined(&self, x: i32, y: i32, z: i32) -> (i32, i32) {
        (self.light_nb(true, x, y, z), self.light_nb(false, x, y, z))
    }
}

// ---------------------------------------------------------------------------------------------
// A partícula
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum K {
    Digging,
    Smoke,
    Crit2,
    Flame,
    Bubble,
    Rain,
    Drop,
    Portal,
    Enchant,
    Spell,
    Explode,
    Aura,
    Heart,
    Lava,
    Note,
    Redstone,
    Cloud,
    Snow,
    Suspend,
    Wake,
    Huge,
    Large,
    Breaking,
}

impl K {
    /// Classes cujo `onUpdate` sorteia (`rand`): no tick paralelo elas rodam em ordem, com o gerador comum.
    fn uses_rng(self) -> bool {
        matches!(self, K::Rain | K::Lava | K::Huge)
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Icon {
    None,
    /// Textura do atlas de blocos.
    Atlas(u16),
    /// Textura própria (índice em `Particles::sprites`).
    Sprite(u32),
}

/// Partícula a criar (`World.spawnParticle`).
struct Spawn {
    id: i32,
    pos: DVec3,
    vel: DVec3,
}

#[derive(Clone, Debug)]
pub struct Fx {
    pub k: K,
    pub pos: DVec3,
    pub prev: DVec3,
    pub vel: DVec3,
    bb: Aabb,
    w: f32,
    h: f32,
    pub ground: bool,
    no_clip: bool,
    pub dead: bool,
    pub age: i32,
    pub max_age: i32,
    pub scale: f32,
    /// `smokeParticleScale`, `flameScale`... conforme a classe.
    base_scale: f32,
    pub gravity: f32,
    pub rgb: [f32; 3],
    pub alpha: f32,
    tex_idx: i32,
    jit: [f32; 2],
    layer: u8,
    icon: Icon,
    /// Portal e mesa de encantamento: ponto de origem.
    origin: DVec3,
    src: Option<[i32; 3]>,
    /// Gota: contador de balanço; Huge: tempo; Large: quadros.
    counter: i32,
    water: bool,
}

type R = StdRng;

fn rd<G: Rng + ?Sized>(r: &mut G) -> f64 {
    r.gen::<f64>()
}
fn rf<G: Rng + ?Sized>(r: &mut G) -> f32 {
    r.gen::<f32>()
}

/// Gerador das partículas que não sorteiam nada no `onUpdate` (tick paralelo): nunca é lido.
struct NoRng;

impl rand::RngCore for NoRng {
    fn next_u32(&mut self) -> u32 {
        unreachable!("partícula sem sorteio no onUpdate")
    }
    fn next_u64(&mut self) -> u64 {
        unreachable!("partícula sem sorteio no onUpdate")
    }
    fn fill_bytes(&mut self, _: &mut [u8]) {
        unreachable!("partícula sem sorteio no onUpdate")
    }
    fn try_fill_bytes(&mut self, _: &mut [u8]) -> Result<(), rand::Error> {
        unreachable!("partícula sem sorteio no onUpdate")
    }
}

impl Fx {
    /// `Entity.setSize`: a caixa fica ancorada no canto mínimo.
    fn set_size(&mut self, w: f32, h: f32) {
        if w != self.w || h != self.h {
            self.w = w;
            self.h = h;
            self.bb = Aabb { min: self.bb.min, max: self.bb.min + DVec3::new(w as f64, h as f64, w as f64) };
        }
    }

    fn set_position(&mut self, p: DVec3) {
        self.pos = p;
        let hw = (self.w / 2.0) as f64;
        self.bb = Aabb { min: DVec3::new(p.x - hw, p.y, p.z - hw), max: DVec3::new(p.x + hw, p.y + self.h as f64, p.z + hw) };
    }

    /// `EntityFX(world, x, y, z)`.
    fn base(r: &mut R, k: K, pos: DVec3) -> Self {
        let mut f = Self {
            k,
            pos,
            prev: pos,
            vel: DVec3::ZERO,
            bb: Aabb { min: pos, max: pos },
            w: 0.0,
            h: 0.0,
            ground: false,
            no_clip: false,
            dead: false,
            age: 0,
            max_age: 0,
            scale: 0.0,
            base_scale: 0.0,
            gravity: 0.0,
            rgb: [1.0; 3],
            alpha: 1.0,
            tex_idx: 0,
            jit: [0.0; 2],
            layer: 0,
            icon: Icon::None,
            origin: DVec3::ZERO,
            src: None,
            counter: 0,
            water: false,
        };
        f.set_size(0.2, 0.2);
        f.set_position(pos);
        f.jit = [rf(r) * 3.0, rf(r) * 3.0];
        f.scale = (rf(r) * 0.5 + 0.5) * 2.0;
        f.max_age = (4.0 / (rf(r) * 0.9 + 0.1)) as i32;
        f
    }

    /// `EntityFX(world, x, y, z, vx, vy, vz)`.
    fn base7(r: &mut R, k: K, pos: DVec3, v: DVec3) -> Self {
        let mut f = Self::base(r, k, pos);
        let mut m = DVec3::new(v.x + (rd(r) * 2.0 - 1.0) * d(0.4), v.y + (rd(r) * 2.0 - 1.0) * d(0.4), v.z + (rd(r) * 2.0 - 1.0) * d(0.4));
        let a = ((rd(r) + rd(r) + 1.0) as f32) * 0.15;
        let b = (m.length()) as f32;
        m.x = m.x / b as f64 * a as f64 * d(0.4);
        m.y = m.y / b as f64 * a as f64 * d(0.4) + d(0.1);
        m.z = m.z / b as f64 * a as f64 * d(0.4);
        f.vel = m;
        f
    }

    pub fn multiply_velocity(&mut self, k: f32) {
        self.vel.x *= k as f64;
        self.vel.y = (self.vel.y - d(0.1)) * k as f64 + d(0.1);
        self.vel.z *= k as f64;
    }

    pub fn scale_by(&mut self, k: f32) {
        self.set_size(0.2 * k, 0.2 * k);
        self.scale *= k;
    }

    pub fn set_rgb(&mut self, r: f32, g: f32, b: f32) {
        self.rgb = [r, g, b];
    }

    fn set_tex(&mut self, i: i32) {
        self.tex_idx = i;
    }

    fn reset_to_bb(&mut self) {
        self.pos = DVec3::new((self.bb.min.x + self.bb.max.x) / 2.0, self.bb.min.y, (self.bb.min.z + self.bb.max.z) / 2.0);
    }

    /// `Entity.moveEntity` para partículas.
    fn move_by(&mut self, env: &Env, v: DVec3) {
        if self.no_clip {
            self.bb = self.bb.offset(v);
            self.reset_to_bb();
            return;
        }
        let mut m = v;
        let boxes = env.colliding(&self.bb.add_coord(v));
        for b in &boxes {
            m.y = b.clip_y(&self.bb, m.y);
        }
        self.bb = self.bb.offset(DVec3::new(0.0, m.y, 0.0));
        for b in &boxes {
            m.x = b.clip_x(&self.bb, m.x);
        }
        self.bb = self.bb.offset(DVec3::new(m.x, 0.0, 0.0));
        for b in &boxes {
            m.z = b.clip_z(&self.bb, m.z);
        }
        self.bb = self.bb.offset(DVec3::new(0.0, 0.0, m.z));
        self.reset_to_bb();
        self.ground = v.y != m.y && v.y < 0.0;
        if v.x != m.x {
            self.vel.x = 0.0;
        }
        if v.z != m.z {
            self.vel.z = 0.0;
        }
        if v.y != m.y {
            self.vel.y = 0.0;
        }
    }

    fn mv(&mut self, env: &Env) {
        let v = self.vel;
        self.move_by(env, v);
    }

    fn damp(&mut self, k: f64) {
        self.vel *= k;
    }

    fn ground_damp(&mut self) {
        if self.ground {
            self.vel.x *= F07;
            self.vel.z *= F07;
        }
    }

    /// `if(particleAge++ >= particleMaxAge) setDead()`.
    fn age_tick(&mut self) {
        if self.age >= self.max_age {
            self.dead = true;
        }
        self.age += 1;
    }

    /// `particleMaxAge-- <= 0` (as partículas que contam para trás).
    fn countdown(&mut self) {
        if self.max_age <= 0 {
            self.dead = true;
        }
        self.max_age -= 1;
    }

    fn frac(&self, pt: f32) -> f32 {
        (self.age as f32 + pt) / self.max_age as f32
    }

    /// `onUpdate` de cada classe.
    fn update<G: Rng + ?Sized>(&mut self, env: &Env, r: &mut G, out: &mut Vec<Spawn>) {
        let in_water = |p: DVec3| matches!(env.st(fl(p.x), fl(p.y), fl(p.z)) >> 4, 8 | 9);
        match self.k {
            K::Digging | K::Breaking => {
                self.prev = self.pos;
                self.age_tick();
                self.vel.y -= 0.04 * self.gravity as f64;
                self.mv(env);
                self.damp(F098);
                self.ground_damp();
            }
            K::Smoke | K::Redstone | K::Cloud | K::Explode => {
                self.prev = self.pos;
                self.age_tick();
                self.tex_idx = 7 - self.age * 8 / self.max_age;
                if self.k == K::Explode {
                    self.vel.y += 0.004;
                    self.mv(env);
                    self.damp(d(0.9));
                } else {
                    if self.k == K::Smoke {
                        self.vel.y += 0.004;
                    }
                    self.mv(env);
                    if self.k != K::Cloud && self.pos.y == self.prev.y {
                        self.vel.x *= 1.1;
                        self.vel.z *= 1.1;
                    }
                    self.damp(F096);
                    if self.k == K::Cloud {
                        let (pp, pv) = env.player;
                        if (pp - self.pos).length_squared() < 4.0 && self.pos.y > pp.y {
                            let y = self.pos.y + (pp.y - self.pos.y) * 0.2;
                            self.vel.y += (pv.y - self.vel.y) * 0.2;
                            self.set_position(DVec3::new(self.pos.x, y, self.pos.z));
                        }
                    }
                }
                self.ground_damp();
            }
            K::Crit2 => {
                self.prev = self.pos;
                self.age_tick();
                self.mv(env);
                self.rgb[1] = (self.rgb[1] as f64 * 0.96) as f32;
                self.rgb[2] = (self.rgb[2] as f64 * 0.9) as f32;
                self.damp(F07);
                self.vel.y -= d(0.02);
                self.ground_damp();
            }
            K::Flame => {
                self.prev = self.pos;
                self.age_tick();
                self.mv(env);
                self.damp(F096);
                self.ground_damp();
            }
            K::Bubble => {
                self.prev = self.pos;
                self.vel.y += 0.002;
                self.mv(env);
                self.damp(d(0.85));
                if !in_water(self.pos) {
                    self.dead = true;
                }
                self.countdown();
            }
            K::Rain => {
                self.prev = self.pos;
                self.vel.y -= self.gravity as f64;
                self.mv(env);
                self.damp(F098);
                self.countdown();
                if self.ground {
                    if rd(r) < 0.5 {
                        self.dead = true;
                    }
                    self.vel.x *= F07;
                    self.vel.z *= F07;
                }
                let (x, y, z) = (fl(self.pos.x), fl(self.pos.y), fl(self.pos.z));
                let st = env.st(x, y, z);
                let fg = FLAGS[(st >> 4) as usize & 255];
                if fg & (F_LIQUID | F_SOLID) != 0 {
                    let top = if fg & F_LIQUID != 0 { 1.0 - liquid_height(st & 15) as f64 } else { block_max_y(env, st, x, y, z) };
                    if self.pos.y < y as f64 + top {
                        self.dead = true;
                    }
                }
            }
            K::Drop => {
                self.prev = self.pos;
                if self.water {
                    self.rgb = [0.2, 0.3, 1.0];
                } else {
                    self.rgb = [1.0, 16.0 / (40 - self.counter + 16) as f32, 4.0 / (40 - self.counter + 8) as f32];
                }
                self.vel.y -= self.gravity as f64;
                let bob = self.counter;
                self.counter -= 1;
                if bob > 0 {
                    self.damp(0.02);
                    self.tex_idx = 113;
                } else {
                    self.tex_idx = 112;
                }
                self.mv(env);
                self.damp(F098);
                self.countdown();
                if self.ground {
                    if self.water {
                        self.dead = true;
                        out.push(Spawn { id: 5, pos: self.pos, vel: DVec3::ZERO });
                    } else {
                        self.tex_idx = 114;
                    }
                    self.vel.x *= F07;
                    self.vel.z *= F07;
                }
                let (x, y, z) = (fl(self.pos.x), fl(self.pos.y), fl(self.pos.z));
                let st = env.st(x, y, z);
                let fg = FLAGS[(st >> 4) as usize & 255];
                if fg & (F_LIQUID | F_SOLID) != 0 {
                    let h = if fg & F_LIQUID != 0 { liquid_height(st & 15) as f64 } else { 0.0 };
                    if self.pos.y < (y + 1) as f64 - h {
                        self.dead = true;
                    }
                }
            }
            K::Portal => {
                self.prev = self.pos;
                let f = self.age as f32 / self.max_age as f32;
                let v3 = -f + f * f * 2.0;
                let v4 = (1.0 - v3) as f64;
                self.pos = DVec3::new(
                    self.origin.x + self.vel.x * v4,
                    self.origin.y + self.vel.y * v4 + (1.0 - f) as f64,
                    self.origin.z + self.vel.z * v4,
                );
                self.age_tick();
            }
            K::Enchant => {
                self.prev = self.pos;
                let mut f = self.age as f32 / self.max_age as f32;
                f = 1.0 - f;
                let mut g = 1.0 - f;
                g *= g;
                g *= g;
                self.pos = DVec3::new(
                    self.origin.x + self.vel.x * f as f64,
                    self.origin.y + self.vel.y * f as f64 - (g * 1.2) as f64,
                    self.origin.z + self.vel.z * f as f64,
                );
                self.age_tick();
            }
            K::Spell => {
                self.prev = self.pos;
                self.age_tick();
                self.tex_idx = self.counter + (7 - self.age * 8 / self.max_age);
                self.vel.y += 0.004;
                self.mv(env);
                if self.pos.y == self.prev.y {
                    self.vel.x *= 1.1;
                    self.vel.z *= 1.1;
                }
                self.damp(F096);
                self.ground_damp();
            }
            K::Aura => {
                self.prev = self.pos;
                self.mv(env);
                self.damp(0.99);
                self.countdown();
            }
            K::Heart | K::Note => {
                self.prev = self.pos;
                self.age_tick();
                self.mv(env);
                if self.pos.y == self.prev.y {
                    self.vel.x *= 1.1;
                    self.vel.z *= 1.1;
                }
                self.damp(if self.k == K::Heart { d(0.86) } else { d(0.66) });
                self.ground_damp();
            }
            K::Lava => {
                self.prev = self.pos;
                self.age_tick();
                let f = self.age as f32 / self.max_age as f32;
                if rf(r) > f {
                    out.push(Spawn { id: 11, pos: self.pos, vel: self.vel });
                }
                self.vel.y -= 0.03;
                self.mv(env);
                self.damp(d(0.999));
                self.ground_damp();
            }
            K::Snow => {
                self.prev = self.pos;
                self.age_tick();
                self.tex_idx = 7 - self.age * 8 / self.max_age;
                self.vel.y -= 0.03;
                self.mv(env);
                self.damp(d(0.99));
                self.ground_damp();
            }
            K::Suspend => {
                self.prev = self.pos;
                self.mv(env);
                if !in_water(self.pos) {
                    self.dead = true;
                }
                self.countdown();
            }
            K::Wake => {
                self.prev = self.pos;
                self.vel.y -= self.gravity as f64;
                self.mv(env);
                self.damp(F098);
                let n = 60 - self.max_age;
                let s = n as f32 * 0.001;
                self.set_size(s, s);
                self.tex_idx = 19 + n % 4;
                self.countdown();
            }
            K::Huge => {
                for _ in 0..6 {
                    let p = self.pos + DVec3::new((rd(r) - rd(r)) * 4.0, (rd(r) - rd(r)) * 4.0, (rd(r) - rd(r)) * 4.0);
                    out.push(Spawn { id: 1, pos: p, vel: DVec3::new((self.counter as f32 / 8.0) as f64, 0.0, 0.0) });
                }
                self.counter += 1;
                if self.counter == 8 {
                    self.dead = true;
                }
            }
            K::Large => {
                self.prev = self.pos;
                self.counter += 1;
                if self.counter == self.max_age {
                    self.dead = true;
                }
            }
        }
    }

    /// Luz (céu, bloco) em 0..240 como `getBrightnessForRender`, depois do ajuste de cada classe.
    fn light16(&self, env: &Env, pt: f32) -> (i32, i32) {
        if self.k == K::Large {
            return (240, 240);
        }
        let base = if env.world.is_loaded(fl(self.pos.x), fl(self.pos.z)) {
            let (s, b) = env.combined(fl(self.pos.x), fl(self.pos.y + self.h as f64 * 0.85), fl(self.pos.z));
            (s * 16, b * 16)
        } else {
            (0, 0)
        };
        let base = if self.k == K::Digging && base == (0, 0) {
            match self.src {
                Some([x, y, z]) if env.world.is_loaded(x, z) => {
                    let (s, b) = env.combined(x, y, z);
                    (s * 16, b * 16)
                }
                _ => base,
            }
        } else {
            base
        };
        let (mut s, mut b) = base;
        match self.k {
            K::Flame => {
                let f = self.frac(pt).clamp(0.0, 1.0);
                b = (b + (f * 15.0 * 16.0) as i32).min(240);
            }
            K::Portal | K::Enchant => {
                let mut f = self.age as f32 / self.max_age as f32;
                f *= f;
                f *= f;
                s = (s + (f * 15.0 * 16.0) as i32).min(240);
            }
            K::Lava => b = 240,
            K::Drop if !self.water => {
                s = 0;
                b = 240;
            }
            _ => {}
        }
        (s, b)
    }

    /// Escala do quadro (`renderParticle` de cada classe).
    fn render_scale(&self, pt: f32) -> f32 {
        let f = self.frac(pt);
        let grow = (f * 32.0).clamp(0.0, 1.0);
        match self.k {
            K::Smoke | K::Crit2 | K::Cloud | K::Heart | K::Note | K::Redstone | K::Snow => self.base_scale * grow,
            K::Flame => self.base_scale * (1.0 - f * f * 0.5),
            K::Lava => self.base_scale * (1.0 - f * f),
            K::Portal => {
                let g = 1.0 - f;
                self.base_scale * (1.0 - g * g)
            }
            _ => self.scale,
        }
    }
}

/// `BlockLiquid.getLiquidHeightPercent`.
fn liquid_height(level: u16) -> f32 {
    let l = if level >= 8 { 0 } else { level };
    (l + 1) as f32 / 9.0
}

fn block_max_y(env: &Env, st: u16, x: i32, y: i32, z: i32) -> f64 {
    env.table.selection(st, &|dx, dy, dz| env.st(x + dx, y + dy, z + dz)).map_or(1.0, |b| b[4])
}

// ---------------------------------------------------------------------------------------------
// Fábricas (`EffectRenderer.registerVanillaParticles`)
// ---------------------------------------------------------------------------------------------

/// `Block.getStateById` -> estado do mundo (`id << 4 | meta`).
pub fn state_from_arg(a: i32) -> u16 {
    (((a & 0xFFF) << 4) | ((a >> 12) & 15)) as u16
}

impl Particles {
    /// Textura "particle" do estado (`BlockModelShapes.getTexture`).
    fn block_icon(env: &Env, st: u16) -> Icon {
        match env.table.models.as_ref().and_then(|m| m.get(st, 0)) {
            Some(w) => Icon::Atlas(w.models[0].1.particle),
            None => Icon::Atlas(0),
        }
    }

    fn sprite_id(&mut self, name: &str) -> Icon {
        let i = self.sprites.iter().position(|s| s == name).unwrap_or_else(|| {
            self.sprites.push(name.to_string());
            self.sprites.len() - 1
        });
        Icon::Sprite(i as u32)
    }

    /// `EntityDiggingFX` (+ `setBlockPos`).
    fn digging(&mut self, env: &Env, pos: DVec3, v: DVec3, st: u16, src: [i32; 3], dust: bool) -> Fx {
        let r = &mut self.rng;
        let mut f = Fx::base7(r, K::Digging, pos, v);
        f.layer = 1;
        f.icon = Self::block_icon(env, st);
        f.gravity = 1.0;
        f.rgb = [0.6; 3];
        f.scale /= 2.0;
        f.src = Some(src);
        if dust {
            f.vel = v;
        }
        if st >> 4 != 2 {
            let c = env.table.tint_color(st);
            f.rgb[0] *= ((c >> 16) & 255) as f32 / 255.0;
            f.rgb[1] *= ((c >> 8) & 255) as f32 / 255.0;
            f.rgb[2] *= (c & 255) as f32 / 255.0;
        }
        f
    }

    /// `EntityBreakingFX` com o ícone do item.
    fn breaking(&mut self, pos: DVec3, v: Option<DVec3>, icon: Icon) -> Fx {
        let r = &mut self.rng;
        let mut f = Fx::base7(r, K::Breaking, pos, DVec3::ZERO);
        f.layer = 1;
        f.icon = icon;
        f.rgb = [1.0; 3];
        f.gravity = 1.0;
        f.scale /= 2.0;
        // Os construtores de 6 argumentos chamam `EntityFX(world, x, y, z, 0, 0, 0)`.
        if let Some(v) = v {
            f.vel = f.vel * d(0.1) + v;
        }
        f
    }

    /// Cria a partícula do tipo `id` (sem registrar).
    fn make(&mut self, env: &Env, id: i32, pos: DVec3, v: DVec3, args: &[i32]) -> Option<Fx> {
        let z = DVec3::ZERO;
        let r = &mut self.rng;
        Some(match id {
            0 => {
                let mut f = Fx::base7(r, K::Explode, pos, v);
                f.vel = DVec3::new(v.x + (rd(r) * 2.0 - 1.0) * d(0.05), v.y + (rd(r) * 2.0 - 1.0) * d(0.05), v.z + (rd(r) * 2.0 - 1.0) * d(0.05));
                let c = rf(r) * 0.3 + 0.7;
                f.rgb = [c; 3];
                f.scale = rf(r) * rf(r) * 6.0 + 1.0;
                f.max_age = (16.0 / (rf(r) as f64 * 0.8 + 0.2)) as i32 + 2;
                f
            }
            1 => {
                let mut f = Fx::base7(r, K::Large, pos, z);
                f.vel = DVec3::ZERO;
                f.layer = 3;
                f.max_age = 6 + r.gen_range(0..4);
                let c = rf(r) * 0.6 + 0.4;
                f.rgb = [c; 3];
                f.base_scale = 1.0 - v.x as f32 * 0.5;
                f
            }
            2 => {
                let mut f = Fx::base7(r, K::Huge, pos, z);
                f.layer = 1;
                f
            }
            4 => {
                let mut f = Fx::base7(r, K::Bubble, pos, v);
                f.rgb = [1.0; 3];
                f.set_tex(32);
                f.set_size(0.02, 0.02);
                f.scale *= rf(r) * 0.6 + 0.2;
                let j = |r: &mut R| (rd(r) * 2.0 - 1.0) * d(0.02);
                f.vel = DVec3::new(v.x * d(0.2) + j(r), v.y * d(0.2) + j(r), v.z * d(0.2) + j(r));
                f.max_age = (8.0 / (rd(r) * 0.8 + 0.2)) as i32;
                f
            }
            5 | 39 => {
                let mut f = Self::rain(r, pos);
                if id == 5 {
                    f.gravity = 0.04;
                    f.tex_idx += 1;
                    if v.y == 0.0 && (v.x != 0.0 || v.z != 0.0) {
                        f.vel = DVec3::new(v.x, v.y + 0.1, v.z);
                    }
                }
                f
            }
            6 => {
                let mut f = Fx::base7(r, K::Wake, pos, z);
                f.vel.x *= d(0.3);
                f.vel.z *= d(0.3);
                f.rgb = [1.0; 3];
                f.set_tex(19);
                f.set_size(0.01, 0.01);
                f.max_age = (8.0 / (rd(r) * 0.8 + 0.2)) as i32;
                f.gravity = 0.0;
                f.vel = v;
                f
            }
            7 => {
                let mut f = Fx::base7(r, K::Suspend, pos - DVec3::new(0.0, 0.125, 0.0), v);
                f.rgb = [0.4, 0.4, 0.7];
                f.set_tex(0);
                f.set_size(0.01, 0.01);
                f.scale *= rf(r) * 0.6 + 0.2;
                f.vel = v * 0.0;
                f.max_age = (16.0 / (rd(r) * 0.8 + 0.2)) as i32;
                f
            }
            8 | 21 | 22 => {
                let mut f = Fx::base7(r, K::Aura, pos, v);
                let c = rf(r) * 0.1 + 0.2;
                f.rgb = [c; 3];
                f.set_tex(0);
                f.set_size(0.02, 0.02);
                f.scale *= rf(r) * 0.6 + 0.5;
                f.vel *= d(0.02);
                f.max_age = (20.0 / (rd(r) * 0.8 + 0.2)) as i32;
                f.no_clip = true;
                if id == 21 {
                    f.set_tex(82);
                    f.rgb = [1.0; 3];
                }
                f
            }
            9 | 10 => {
                let mut f = Self::crit2(r, pos, v);
                if id == 10 {
                    f.rgb = [f.rgb[0] * 0.3, f.rgb[1] * 0.8, f.rgb[2]];
                    f.tex_idx += 1;
                }
                f
            }
            11 | 12 => Self::smoke(r, pos, v, if id == 12 { 2.5 } else { 1.0 }),
            13..=17 => {
                let (a, b) = (0.5 - rd(r), 0.5 - rd(r));
                let mut f = Fx::base7(r, K::Spell, pos, DVec3::new(a, v.y, b));
                f.counter = 128;
                f.vel.y *= d(0.2);
                if v.x == 0.0 && v.z == 0.0 {
                    f.vel.x *= d(0.1);
                    f.vel.z *= d(0.1);
                }
                f.scale *= 0.75;
                f.max_age = (8.0 / (rd(r) * 0.8 + 0.2)) as i32;
                match id {
                    14 => f.counter = 144,
                    15 => f.set_rgb(v.x as f32, v.y as f32, v.z as f32),
                    16 => {
                        f.alpha = 0.15;
                        f.set_rgb(v.x as f32, v.y as f32, v.z as f32);
                    }
                    17 => {
                        f.counter = 144;
                        let c = rf(r) * 0.5 + 0.35;
                        f.set_rgb(c, 0.0, c);
                    }
                    _ => {}
                }
                f
            }
            18 | 19 => {
                let mut f = Fx::base7(r, K::Drop, pos, z);
                f.vel = DVec3::ZERO;
                f.water = id == 18;
                f.rgb = if f.water { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] };
                f.set_tex(113);
                f.set_size(0.01, 0.01);
                f.gravity = 0.06;
                f.counter = 40;
                f.max_age = (64.0 / (rd(r) * 0.8 + 0.2)) as i32;
                f
            }
            20 | 34 => {
                let p = if id == 20 { pos + DVec3::new(0.0, 0.5, 0.0) } else { pos };
                let mut f = Fx::base7(r, K::Heart, p, z);
                f.vel *= d(0.01);
                f.vel.y += 0.1;
                f.scale *= 0.75;
                f.scale *= 2.0;
                f.base_scale = f.scale;
                f.max_age = 16;
                f.set_tex(80);
                if id == 20 {
                    f.set_tex(81);
                    f.rgb = [1.0; 3];
                }
                f
            }
            23 => {
                let mut f = Fx::base7(r, K::Note, pos, z);
                f.vel *= d(0.01);
                f.vel.y += 0.2;
                let s = |o: f32| ((v.x as f32 + o) * std::f32::consts::PI * 2.0).sin() * 0.65 + 0.35;
                f.rgb = [s(0.0), s(0.33333334), s(0.6666667)];
                f.scale *= 0.75;
                f.scale *= 2.0;
                f.base_scale = f.scale;
                f.max_age = 6;
                f.set_tex(64);
                f
            }
            24 => {
                let mut f = Fx::base7(r, K::Portal, pos, v);
                f.vel = v;
                f.origin = pos;
                f.pos = pos;
                let c = rf(r) * 0.6 + 0.4;
                let s = rf(r) * 0.2 + 0.5;
                f.scale = s;
                f.base_scale = s;
                f.rgb = [c * 0.9, c * 0.3, c];
                f.max_age = (rd(r) * 10.0) as i32 + 40;
                f.no_clip = true;
                f.set_tex((rd(r) * 8.0) as i32);
                f
            }
            25 => {
                let mut f = Fx::base7(r, K::Enchant, pos, v);
                f.vel = v;
                f.origin = pos;
                f.pos = pos + v;
                f.prev = f.pos;
                let c = rf(r) * 0.6 + 0.4;
                f.scale = rf(r) * 0.5 + 0.2;
                f.rgb = [c * 0.9, c * 0.9, c];
                f.max_age = (rd(r) * 10.0) as i32 + 30;
                f.no_clip = true;
                f.set_tex((rd(r) * 26.0 + 1.0 + 224.0) as i32);
                f
            }
            26 => {
                let mut f = Fx::base7(r, K::Flame, pos, v);
                f.vel = f.vel * d(0.01) + v;
                f.pos.x += ((rf(r) - rf(r)) * 0.05) as f64;
                f.pos.y += ((rf(r) - rf(r)) * 0.05) as f64;
                f.pos.z += ((rf(r) - rf(r)) * 0.05) as f64;
                f.base_scale = f.scale;
                f.rgb = [1.0; 3];
                f.max_age = (8.0 / (rd(r) * 0.8 + 0.2)) as i32 + 4;
                f.no_clip = true;
                f.set_tex(48);
                f
            }
            27 => {
                let mut f = Fx::base7(r, K::Lava, pos, z);
                f.vel *= d(0.8);
                f.vel.y = (rf(r) * 0.4 + 0.05) as f64;
                f.rgb = [1.0; 3];
                f.scale *= rf(r) * 2.0 + 0.2;
                f.base_scale = f.scale;
                f.max_age = (16.0 / (rd(r) * 0.8 + 0.2)) as i32;
                f.set_tex(49);
                f
            }
            29 => {
                let mut f = Fx::base7(r, K::Cloud, pos, z);
                f.vel = f.vel * d(0.1) + v;
                let c = 1.0 - (rd(r) * d(0.3)) as f32;
                f.rgb = [c; 3];
                f.scale *= 0.75;
                f.scale *= 2.5;
                f.base_scale = f.scale;
                f.max_age = (8.0 / (rd(r) * 0.8 + 0.3)) as i32;
                f.max_age = (f.max_age as f32 * 2.5) as i32;
                f
            }
            30 => {
                let (rr, gg, bb) = (if v.x as f32 == 0.0 { 1.0 } else { v.x as f32 }, v.y as f32, v.z as f32);
                let mut f = Fx::base7(r, K::Redstone, pos, z);
                f.vel *= d(0.1);
                let k = (rd(r) as f32) * 0.4 + 0.6;
                let ch = |m: f32, r: &mut R| ((rd(r) * d(0.2)) as f32 + 0.8) * m * k;
                f.rgb = [ch(rr, r), ch(gg, r), ch(bb, r)];
                f.scale *= 0.75;
                f.base_scale = f.scale;
                f.max_age = (8.0 / (rd(r) * 0.8 + 0.2)) as i32;
                f
            }
            32 => {
                let mut f = Fx::base7(r, K::Snow, pos, v);
                f.vel = f.vel * d(0.1) + v;
                let c = 1.0 - (rd(r) * d(0.3)) as f32;
                f.rgb = [c; 3];
                f.scale *= 0.75;
                f.base_scale = f.scale;
                f.max_age = (8.0 / (rd(r) * 0.8 + 0.2)) as i32;
                f
            }
            31 | 33 => {
                let name = if id == 31 { "items/snowball" } else { "items/slimeball" };
                let icon = self.sprite_id(name);
                self.breaking(pos, None, icon)
            }
            36 => {
                // `getParticleIcon(item, dano)`: textura "particle" do modelo do item (resolvida em `prepare_item_sprite`).
                let name = self.item_sprite.take()?;
                let icon = self.sprite_id(&name);
                self.breaking(pos, Some(v), icon)
            }
            37 | 38 => {
                let st = state_from_arg(*args.first()?);
                if id == 38 && NO_RENDER.contains(&(st >> 4)) {
                    return None;
                }
                let src = [fl(pos.x), fl(pos.y), fl(pos.z)];
                self.digging(env, pos, v, st, src, id == 38)
            }
            _ => return None,
        })
    }

    fn rain(r: &mut R, pos: DVec3) -> Fx {
        let mut f = Fx::base7(r, K::Rain, pos, DVec3::ZERO);
        f.vel.x *= d(0.3);
        f.vel.y = rd(r) * d(0.2) + d(0.1);
        f.vel.z *= d(0.3);
        f.rgb = [1.0; 3];
        f.set_tex(19 + r.gen_range(0..4));
        f.set_size(0.01, 0.01);
        f.gravity = 0.06;
        f.max_age = (8.0 / (rd(r) * 0.8 + 0.2)) as i32;
        f
    }

    fn crit2(r: &mut R, pos: DVec3, v: DVec3) -> Fx {
        let mut f = Fx::base7(r, K::Crit2, pos, DVec3::ZERO);
        f.vel *= d(0.1);
        f.vel += v * 0.4;
        let c = (rd(r) * d(0.3) + d(0.6)) as f32;
        f.rgb = [c; 3];
        f.scale *= 0.75;
        f.base_scale = f.scale;
        f.max_age = (6.0 / (rd(r) * 0.8 + 0.6)) as i32;
        f.set_tex(65);
        // O construtor termina com um `onUpdate()`.
        let mut sink = Vec::new();
        f.crit_first_update(&mut sink);
        f
    }

    fn smoke(r: &mut R, pos: DVec3, v: DVec3, sc: f32) -> Fx {
        let mut f = Fx::base7(r, K::Smoke, pos, DVec3::ZERO);
        f.vel = f.vel * d(0.1) + v;
        let c = (rd(r) * d(0.3)) as f32;
        f.rgb = [c; 3];
        f.scale *= 0.75;
        f.scale *= sc;
        f.base_scale = f.scale;
        f.max_age = (8.0 / (rd(r) * 0.8 + 0.2)) as i32;
        f.max_age = (f.max_age as f32 * sc) as i32;
        f
    }
}

impl Fx {
    /// `onUpdate()` do construtor do crítico: roda sem mundo (a partícula ainda não colide com nada).
    fn crit_first_update(&mut self, _out: &mut Vec<Spawn>) {
        self.prev = self.pos;
        self.age_tick();
        let v = self.vel;
        self.bb = self.bb.offset(v);
        self.reset_to_bb();
        self.rgb[1] = (self.rgb[1] as f64 * 0.96) as f32;
        self.rgb[2] = (self.rgb[2] as f64 * 0.9) as f32;
        self.vel *= F07;
        self.vel.y -= d(0.02);
    }
}

// ---------------------------------------------------------------------------------------------
// EffectRenderer
// ---------------------------------------------------------------------------------------------

struct Emitter {
    entity: i32,
    age: i32,
    kind: i32,
}

pub struct Particles {
    layers: [[VecDeque<Fx>; 2]; 4],
    emitters: Vec<Emitter>,
    rng: StdRng,
    /// `GameSettings.particleSetting` (0 todas, 1 diminuídas, 2 mínimas).
    pub setting: i32,
    sprites: Vec<String>,
    tex_ids: HashMap<String, Option<u32>>,
    /// Textura de partícula do item do S2A 36, preparada antes de `handle_particles`.
    item_sprite: Option<String>,
    item_lib: mc_world::model::ModelLib,
    /// Quantas saíram do começo de cada camada pelo limite de 4000 (o tick paralelo acha os índices por aqui).
    pops: [[u64; 2]; 4],
    /// Reaproveitado entre quadros por `draw`.
    draw_scratch: Vec<Option<(Tex, ParticleInstance)>>,
}

impl Default for Particles {
    fn default() -> Self {
        Self::new(rand::random())
    }
}

impl Particles {
    pub fn new(seed: u64) -> Self {
        Self {
            layers: Default::default(),
            emitters: Vec::new(),
            rng: StdRng::seed_from_u64(seed),
            setting: 0,
            sprites: Vec::new(),
            tex_ids: HashMap::new(),
            item_sprite: None,
            item_lib: Default::default(),
            pops: [[0; 2]; 4],
            draw_scratch: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.layers.iter_mut().flatten().for_each(VecDeque::clear);
        self.emitters.clear();
    }

    /// `EffectRenderer.getStatistics` (o `P:` do F3).
    pub fn count(&self) -> usize {
        self.layers.iter().flatten().map(VecDeque::len).sum()
    }

    #[cfg(test)]
    pub fn layer_len(&self, layer: usize, alpha_layer: usize) -> usize {
        self.layers[layer][alpha_layer].len()
    }

    /// `EffectRenderer.addEffect`: no máximo 4000 por camada, a mais antiga sai.
    pub fn add_effect(&mut self, f: Fx) {
        let j = if f.alpha != 1.0 { 0 } else { 1 };
        let list = &mut self.layers[f.layer as usize][j];
        if list.len() >= 4000 {
            list.pop_front();
            self.pops[f.layer as usize][j] += 1;
        }
        list.push_back(f);
    }

    /// `RenderGlobal.spawnEntityFX`: filtros de opção e distância, depois a fábrica.
    /// Devolve onde a partícula ficou, para ajustes depois (cor, velocidade).
    pub fn spawn(&mut self, env: &Env, id: i32, long: bool, pos: DVec3, vel: DVec3, args: &[i32]) -> Option<(usize, usize)> {
        let mut setting = self.setting;
        if setting == 1 && self.rng.gen_range(0..3) == 0 {
            setting = 2;
        }
        if !long {
            if (env.player.0 - pos).length_squared() > 256.0 || setting > 1 {
                return None;
            }
        }
        let fx = self.make(env, id, pos, vel, args)?;
        let j = if fx.alpha != 1.0 { 0 } else { 1 };
        let l = fx.layer as usize;
        self.add_effect(fx);
        Some((l, j))
    }

    /// Atalho para `World.spawnParticle(tipo, ...)`.
    pub fn spawn_type(&mut self, env: &Env, id: i32, pos: DVec3, vel: DVec3) {
        self.spawn(env, id, particle_info(id).0, pos, vel, &[]);
    }

    pub fn tick(&mut self, env: &Env) {
        for l in 0..4 {
            for j in 0..2 {
                // `onUpdate` das que não sorteiam nada, adiantado em paralelo sobre cópias (o mundo é só leitura).
                let mut pre: Vec<Option<(Fx, Vec<Spawn>)>> = Vec::new();
                if self.layers[l][j].len() >= PAR_MIN {
                    let (world, table, player) = (env.world, env.table, env.player);
                    self.layers[l][j]
                        .par_iter()
                        .with_min_len(64)
                        .map_init(
                            || Env { world, table, player, entity: &no_entity },
                            |env, f| {
                                if f.k.uses_rng() {
                                    return None;
                                }
                                let (mut f, mut out) = (f.clone(), Vec::new());
                                f.update(env, &mut NoRng, &mut out);
                                Some((f, out))
                            },
                        )
                        .collect_into_vec(&mut pre);
                }
                // O laço do Java, em ordem: o que nasce entra no fim e é atualizado no mesmo laço; passando
                // de 4000, a mais antiga sai (`remove(0)`) e a seguinte fica sem atualizar neste tick.
                // `seq` é a posição contada desde o começo do tick, para achar o resultado adiantado.
                let pops0 = self.pops[l][j];
                let mut i = 0;
                while i < self.layers[l][j].len() {
                    let seq = i + (self.pops[l][j] - pops0) as usize;
                    let out = match pre.get_mut(seq).and_then(Option::take) {
                        Some((f, out)) => {
                            self.layers[l][j][i] = f;
                            out
                        }
                        None => {
                            let mut out = Vec::new();
                            let Self { layers, rng, .. } = self;
                            layers[l][j][i].update(env, rng, &mut out);
                            out
                        }
                    };
                    for s in out {
                        self.spawn_type(env, s.id, s.pos, s.vel);
                    }
                    i += 1;
                }
                self.layers[l][j].retain(|f| !f.dead);
            }
        }
        let mut emitters = std::mem::take(&mut self.emitters);
        for e in &mut emitters {
            self.emitter_update(env, e);
        }
        emitters.retain(|e| e.age < 3);
        emitters.append(&mut self.emitters);
        self.emitters = emitters;
    }

    fn emitter_update(&mut self, env: &Env, e: &mut Emitter) {
        if let Some(t) = (env.entity)(e.entity) {
            for _ in 0..16 {
                let r = &mut self.rng;
                let (x, y, z) = (rf(r) * 2.0 - 1.0, rf(r) * 2.0 - 1.0, rf(r) * 2.0 - 1.0);
                let (x, y, z) = (x as f64, y as f64, z as f64);
                if x * x + y * y + z * z <= 1.0 {
                    let p = DVec3::new(
                        t.pos.x + x * t.width as f64 / 4.0,
                        t.pos.y + (t.height / 2.0) as f64 + y * t.height as f64 / 4.0,
                        t.pos.z + z * t.width as f64 / 4.0,
                    );
                    self.spawn(env, e.kind, false, p, DVec3::new(x, y + 0.2, z), &[]);
                }
            }
        }
        e.age += 1;
    }

    /// Gancho de carga `MINE_RS_TEST_PARTICULAS=n[@t]`: n partículas por tick na frente do jogador do
    /// `--test-world` (metade de vários tipos na camada 0, metade em blocos quebrados na camada 1); no tick t
    /// tudo para e o desenho usa pt = 0,5, para prints iguais entre execuções.
    pub fn test_fill(&mut self, env: &Env, n: usize, tick: u64) {
        const MIX: [i32; 13] = [26, 11, 27, 30, 9, 23, 13, 24, 29, 0, 19, 34, 37];
        let h = |i: u64| {
            let mut z = tick.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ i.wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 30)).wrapping_mul(0x94D0_49BB_1331_11EB);
            ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
        };
        if tick % 20 == 0 {
            let por_camada: Vec<usize> = self.layers.iter().flatten().map(VecDeque::len).collect();
            println!("[particulas] tick {tick}: {} {por_camada:?}", self.count());
        }
        let half = n / 2;
        for i in 0..half as u64 {
            let p = DVec3::new(-4.0 + h(i * 4) * 8.0, 64.5 + h(i * 4 + 1) * 4.0, 3.0 + h(i * 4 + 2) * 8.0);
            let id = MIX[(i % MIX.len() as u64) as usize];
            self.spawn(env, id, true, p, DVec3::new(0.0, 0.02, 0.0), &[1 << 12]);
        }
        for b in 0..(n - half).div_ceil(64) as u64 {
            let (x, z) = ((h(1 << 20 | b) * 8.0) as i32 - 4, (h(1 << 21 | b) * 8.0) as i32 + 3);
            self.block_destroy(env, (x, 65, z), [1u16, 2, 3, 4, 5, 17, 18][(b % 7) as usize] << 4);
        }
    }

    /// `(n, t)` do `MINE_RS_TEST_PARTICULAS`.
    pub fn test_hook() -> Option<(usize, u64)> {
        static HOOK: std::sync::OnceLock<Option<(usize, u64)>> = std::sync::OnceLock::new();
        *HOOK.get_or_init(|| {
            let v = std::env::var("MINE_RS_TEST_PARTICULAS").ok()?;
            let (n, t) = v.split_once('@').unwrap_or((&v, ""));
            Some((n.parse().ok()?, t.parse().unwrap_or(u64::MAX)))
        })
    }

    /// `emitParticleAtEntity` (S0B 4 = crítico, 5 = crítico mágico). O construtor já roda um `onUpdate`.
    pub fn emit_at_entity(&mut self, env: &Env, entity: i32, kind: i32) {
        let mut e = Emitter { entity, age: 0, kind };
        self.emitter_update(env, &mut e);
        self.emitters.push(e);
    }

    // --- blocos -------------------------------------------------------------------------------

    /// `addBlockDestroyEffects`: 4x4x4 partículas do bloco.
    pub fn block_destroy(&mut self, env: &Env, (x, y, z): (i32, i32, i32), st: u16) {
        if st >> 4 == 0 {
            return;
        }
        let n = 4;
        for j in 0..n {
            for k in 0..n {
                for l in 0..n {
                    let p = DVec3::new(x as f64 + (j as f64 + 0.5) / n as f64, y as f64 + (k as f64 + 0.5) / n as f64, z as f64 + (l as f64 + 0.5) / n as f64);
                    let v = p - DVec3::new(x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5);
                    let f = self.digging(env, p, v, st, [x, y, z], false);
                    self.add_effect(f);
                }
            }
        }
    }

    /// `addBlockHitEffects(pos, face)`; `face` na ordem baixo, cima, norte, sul, oeste, leste.
    pub fn block_hit(&mut self, env: &Env, (x, y, z): (i32, i32, i32), face: usize) {
        let st = env.world.block(x, y, z);
        if NO_RENDER.contains(&(st >> 4)) {
            return;
        }
        let b = env.table.selection(st, &|dx, dy, dz| env.st(x + dx, y + dy, z + dz)).unwrap_or([0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
        let f = 0.1;
        let r = &mut self.rng;
        let mut p = DVec3::new(
            x as f64 + r.gen::<f64>() * (b[3] - b[0] - d(f * 2.0)) + d(f) + b[0],
            y as f64 + r.gen::<f64>() * (b[4] - b[1] - d(f * 2.0)) + d(f) + b[1],
            z as f64 + r.gen::<f64>() * (b[5] - b[2] - d(f * 2.0)) + d(f) + b[2],
        );
        match face {
            0 => p.y = y as f64 + b[1] - d(f),
            1 => p.y = y as f64 + b[4] + d(f),
            2 => p.z = z as f64 + b[2] - d(f),
            3 => p.z = z as f64 + b[5] + d(f),
            4 => p.x = x as f64 + b[0] - d(f),
            _ => p.x = x as f64 + b[3] + d(f),
        }
        let mut fx = self.digging(env, p, DVec3::ZERO, st, [x, y, z], false);
        fx.multiply_velocity(0.2);
        fx.scale_by(0.6);
        self.add_effect(fx);
    }

    /// Corrida: `Entity.createRunningParticles`.
    pub fn running(&mut self, env: &Env, pos: DVec3, min_y: f64, vel: DVec3, width: f32) {
        let (x, y, z) = (fl(pos.x), fl(pos.y - 0.2), fl(pos.z));
        let st = env.st(x, y, z);
        if NO_RENDER.contains(&(st >> 4)) {
            return;
        }
        let r = &mut self.rng;
        let p = DVec3::new(pos.x + (rf(r) - 0.5) as f64 * width as f64, min_y + 0.1, pos.z + (rf(r) - 0.5) as f64 * width as f64);
        let arg = (st >> 4) as i32 | ((st & 15) as i32) << 12;
        self.spawn(env, 37, false, p, DVec3::new(-vel.x * 4.0, 1.5, -vel.z * 4.0), &[arg]);
    }

    /// Partículas de poção no corpo (`EntityLivingBase.updatePotionEffects`, metadados 7 e 8).
    pub fn potion_swirls(&mut self, env: &Env, color: i32, ambient: bool, invisible: bool, pos: DVec3, w: f32, h: f32) {
        if color <= 0 {
            return;
        }
        let r = &mut self.rng;
        let mut go = if invisible { r.gen_range(0..15) == 0 } else { r.gen::<bool>() };
        if ambient {
            go &= r.gen_range(0..5) == 0;
        }
        if go {
            let c = |s: i32| ((color >> s) & 255) as f64 / 255.0;
            let p = DVec3::new(pos.x + (rd(r) - 0.5) * w as f64, pos.y + rd(r) * h as f64, pos.z + (rd(r) - 0.5) * w as f64);
            self.spawn(env, if ambient { 16 } else { 15 }, false, p, DVec3::new(c(16), c(8), c(0)), &[]);
        }
    }

    // --- pacotes ------------------------------------------------------------------------------

    /// S2A: `NetHandlerPlayClient.handleParticles`.
    /// `ItemModelMesher.getParticleIcon`: `ModelBlock.resolveTextureName("particle")` do modelo do item;
    /// o `ItemModelGenerator` usa `layer0` quando o JSON não traz `particle`.
    pub fn prepare_item_sprite(&mut self, jar: &mut mc_render::Jar, p: &ParticlePacket) {
        self.item_sprite = None;
        if p.id != 36 {
            return;
        }
        let Some(&id) = p.args.first() else { return };
        let damage = p.args.get(1).copied().unwrap_or(0) as i16;
        let Some(name) = mc_world::items::model_name(id as i16, damage) else { return };
        let mut bytes = |q: &str| jar.bytes(q);
        let Some(model) = self.item_lib.resolve(&format!("item/{name}"), &mut bytes) else { return };
        let mut tex = model.texture("particle");
        if tex == "missingno" {
            tex = model.texture("layer0");
        }
        if tex != "missingno" {
            self.item_sprite = Some(tex);
        }
    }

    pub fn handle_particles(&mut self, env: &Env, p: &ParticlePacket) {
        let id = p.id;
        let args = &p.args[..];
        if p.count == 0 {
            let v = DVec3::new((p.speed * p.off[0]) as f64, (p.speed * p.off[1]) as f64, (p.speed * p.off[2]) as f64);
            self.spawn(env, id, p.long, p.pos, v, args);
        } else {
            for _ in 0..p.count.max(0) {
                let r = &mut self.rng;
                let o = [gaussian(r) * p.off[0] as f64, gaussian(r) * p.off[1] as f64, gaussian(r) * p.off[2] as f64];
                let v = [gaussian(r) * p.speed as f64, gaussian(r) * p.speed as f64, gaussian(r) * p.speed as f64];
                self.spawn(env, id, p.long, p.pos + DVec3::from(o), DVec3::from(v), args);
            }
        }
    }

    /// S27: `Explosion.doExplosionB(true)`. Os blocos afetados somem no mundo (feito por quem chama).
    pub fn explosion(&mut self, env: &Env, e: &ExplosionPacket) {
        let c = e.pos;
        if e.strength >= 2.0 {
            self.spawn(env, 2, true, c, DVec3::new(1.0, 0.0, 0.0), &[]);
        } else {
            self.spawn(env, 1, true, c, DVec3::new(1.0, 0.0, 0.0), &[]);
        }
        for &(bx, by, bz) in &e.blocks {
            let r = &mut self.rng;
            let p = DVec3::new(bx as f64 + rf(r) as f64, by as f64 + rf(r) as f64, bz as f64 + rf(r) as f64);
            let mut dv = p - c;
            let len = dv.length();
            dv /= len;
            let mut k = 0.5 / (len / e.strength as f64 + 0.1);
            k *= (rf(r) * rf(r) + 0.3) as f64;
            dv *= k;
            self.spawn(env, 0, true, (p + c) / 2.0, dv, &[]);
            self.spawn(env, 11, false, p, dv, &[]);
        }
    }

    /// S28: `RenderGlobal.playAuxSFX`, só a parte visual.
    pub fn effect(&mut self, env: &Env, kind: i32, (x, y, z): (i32, i32, i32), data: i32) {
        let v = DVec3::ZERO;
        match kind {
            2000 => {
                let l = data % 3 - 1;
                let i = data / 3 % 3 - 1;
                let (d15, d17, d19) = (x as f64 + l as f64 * 0.6 + 0.5, y as f64 + 0.5, z as f64 + i as f64 * 0.6 + 0.5);
                for _ in 0..10 {
                    let r = &mut self.rng;
                    let d20 = rd(r) * 0.2 + 0.01;
                    let p = DVec3::new(
                        d15 + l as f64 * 0.01 + (rd(r) - 0.5) * i as f64 * 0.5,
                        d17 + (rd(r) - 0.5) * 0.5,
                        d19 + i as f64 * 0.01 + (rd(r) - 0.5) * l as f64 * 0.5,
                    );
                    let vel = DVec3::new(l as f64 * d20 + gaussian(r) * 0.01, -0.03 + gaussian(r) * 0.01, i as f64 * d20 + gaussian(r) * 0.01);
                    self.spawn(env, 11, false, p, vel, &[]);
                }
            }
            2001 => {
                let st = ((data & 0xFFF) << 4 | ((data >> 12) & 255) & 15) as u16;
                self.block_destroy(env, (x, y, z), st);
            }
            2002 => {
                for _ in 0..8 {
                    let r = &mut self.rng;
                    let vel = DVec3::new(gaussian(r) * 0.15, rd(r) * 0.2, gaussian(r) * 0.15);
                    self.spawn(env, 36, false, DVec3::new(x as f64, y as f64, z as f64), vel, &[373, data]);
                }
            }
            2003 => {
                let (d0, d1, d2) = (x as f64 + 0.5, y as f64, z as f64 + 0.5);
                for _ in 0..8 {
                    let r = &mut self.rng;
                    let vel = DVec3::new(gaussian(r) * 0.15, rd(r) * 0.2, gaussian(r) * 0.15);
                    self.spawn(env, 36, false, DVec3::new(d0, d1, d2), vel, &[381]);
                }
                let mut a = 0.0f64;
                while a < 6.283185307179586 {
                    for k in [5.0, 7.0] {
                        self.spawn(env, 24, false, DVec3::new(d0 + a.cos() * 5.0, d1 - 0.4, d2 + a.sin() * 5.0), DVec3::new(a.cos() * -k, 0.0, a.sin() * -k), &[]);
                    }
                    a += 0.15707963267948966;
                }
            }
            2004 => {
                for _ in 0..20 {
                    let r = &mut self.rng;
                    let p = DVec3::new(
                        x as f64 + 0.5 + (rf(r) as f64 - 0.5) * 2.0,
                        y as f64 + 0.5 + (rf(r) as f64 - 0.5) * 2.0,
                        z as f64 + 0.5 + (rf(r) as f64 - 0.5) * 2.0,
                    );
                    self.spawn(env, 11, false, p, v, &[]);
                    self.spawn(env, 26, false, p, v, &[]);
                }
            }
            2005 => {
                // ItemDye.spawnBonemealParticles
                let amount = if data == 0 { 15 } else { data };
                let st = env.st(x, y, z);
                if st >> 4 != 0 {
                    let max_y = block_max_y(env, st, x, y, z);
                    for _ in 0..amount {
                        let r = &mut self.rng;
                        let vel = DVec3::new(gaussian(r) * 0.02, gaussian(r) * 0.02, gaussian(r) * 0.02);
                        let p = DVec3::new(x as f64 + rf(r) as f64, y as f64 + rf(r) as f64 * max_y, z as f64 + rf(r) as f64);
                        self.spawn(env, 21, false, p, vel, &[]);
                    }
                }
            }
            _ => {}
        }
    }

    /// `WorldClient.doVoidFogParticles`: 1000 posições em volta do jogador chamam `randomDisplayTick`.
    pub fn random_display_ticks(&mut self, env: &Env, (px, py, pz): (i32, i32, i32)) {
        for _ in 0..1000 {
            let r = &mut self.rng;
            let (x, y, z) = (px + r.gen_range(0..16) - r.gen_range(0..16), py + r.gen_range(0..16) - r.gen_range(0..16), pz + r.gen_range(0..16) - r.gen_range(0..16));
            let st = env.st(x, y, z);
            if st != 0 {
                self.random_display_tick(env, (x, y, z), st);
            }
        }
    }

    /// `Block.randomDisplayTick` dos blocos portados.
    pub fn random_display_tick(&mut self, env: &Env, (x, y, z): (i32, i32, i32), st: u16) {
        let (bx, by, bz) = (x as f64, y as f64, z as f64);
        let z0 = DVec3::ZERO;
        match st >> 4 {
            // Tocha: FACING pelo meta (1 leste, 2 oeste, 3 sul, 4 norte, resto em pé).
            50 | 76 => {
                let m = st & 15;
                let (ox, oz, horiz) = match m {
                    1 => (-1.0, 0.0, true),
                    2 => (1.0, 0.0, true),
                    3 => (0.0, -1.0, true),
                    4 => (0.0, 1.0, true),
                    _ => (0.0, 0.0, false),
                };
                if st >> 4 == 76 {
                    let r = &mut self.rng;
                    let mut p = DVec3::new(bx + 0.5 + (rd(r) - 0.5) * 0.2, by + 0.7 + (rd(r) - 0.5) * 0.2, bz + 0.5 + (rd(r) - 0.5) * 0.2);
                    if horiz {
                        p.x += 0.27 * ox;
                        p.y += 0.22;
                        p.z += 0.27 * oz;
                    }
                    self.spawn(env, 30, false, p, z0, &[]);
                } else {
                    let (d0, d1, d2) = (bx + 0.5, by + 0.7, bz + 0.5);
                    let p = if horiz { DVec3::new(d0 + 0.27 * ox, d1 + 0.22, d2 + 0.27 * oz) } else { DVec3::new(d0, d1, d2) };
                    self.spawn(env, 11, false, p, z0, &[]);
                    self.spawn(env, 26, false, p, z0, &[]);
                }
            }
            // Fornalha acesa.
            62 => {
                let r = &mut self.rng;
                let d8 = by + rd(r) * 6.0 / 16.0;
                let d14 = rd(r) * 0.6 - 0.3;
                let (d6, d10, d12) = (bx + 0.5, bz + 0.5, 0.52);
                let p = match (st & 7) as u8 {
                    4 => DVec3::new(d6 - d12, d8, d10 + d14),
                    5 => DVec3::new(d6 + d12, d8, d10 + d14),
                    3 => DVec3::new(d6 + d14, d8, d10 + d12),
                    _ => DVec3::new(d6 + d14, d8, d10 - d12),
                };
                self.spawn(env, 11, false, p, z0, &[]);
                self.spawn(env, 26, false, p, z0, &[]);
            }
            // Portal do Nether.
            90 => {
                for _ in 0..4 {
                    let r = &mut self.rng;
                    let mut p = DVec3::new((x as f32 + rf(r)) as f64, (y as f32 + rf(r)) as f64, (z as f32 + rf(r)) as f64);
                    let mut v = DVec3::new((rf(r) as f64 - 0.5) * 0.5, (rf(r) as f64 - 0.5) * 0.5, (rf(r) as f64 - 0.5) * 0.5);
                    let s = r.gen_range(0..2) * 2 - 1;
                    if env.st(x - 1, y, z) >> 4 != 90 && env.st(x + 1, y, z) >> 4 != 90 {
                        p.x = bx + 0.5 + 0.25 * s as f64;
                        v.x = (rf(r) * 2.0 * s as f32) as f64;
                    } else {
                        p.z = bz + 0.5 + 0.25 * s as f64;
                        v.z = (rf(r) * 2.0 * s as f32) as f64;
                    }
                    self.spawn(env, 24, false, p, v, &[]);
                }
            }
            // Mesa de encantamento.
            116 => {
                for i in -2..=2i32 {
                    let mut j = -2i32;
                    while j <= 2 {
                        if i > -2 && i < 2 && j == -1 {
                            j = 2;
                        }
                        if self.rng.gen_range(0..16) == 0 {
                            for k in 0..=1i32 {
                                if env.st(x + i, y + k, z + j) >> 4 == 47 {
                                    if env.st(x + i / 2, y, z + j / 2) != 0 {
                                        break;
                                    }
                                    let r = &mut self.rng;
                                    let v = DVec3::new((i as f32 + rf(r)) as f64 - 0.5, (k as f32 - rf(r) - 1.0) as f64, (j as f32 + rf(r)) as f64 - 0.5);
                                    self.spawn(env, 25, false, DVec3::new(bx + 0.5, by + 2.0, bz + 0.5), v, &[]);
                                }
                            }
                        }
                        j += 1;
                    }
                }
            }
            // Micélio.
            110 => {
                if self.rng.gen_range(0..10) == 0 {
                    let r = &mut self.rng;
                    let p = DVec3::new((x as f32 + rf(r)) as f64, (y as f32 + 1.1) as f64, (z as f32 + rf(r)) as f64);
                    self.spawn(env, 22, false, p, z0, &[]);
                }
            }
            // Água e lava (`BlockLiquid`).
            8 | 9 => {
                let level = st & 15;
                if !(level > 0 && level < 8) && self.rng.gen_range(0..10) == 0 {
                    let r = &mut self.rng;
                    let p = DVec3::new(bx + rf(r) as f64, by + rf(r) as f64, bz + rf(r) as f64);
                    self.spawn(env, 7, false, p, z0, &[]);
                }
            }
            10 | 11 => {
                let up = env.st(x, y + 1, z);
                if up >> 4 == 0 && self.rng.gen_range(0..100) == 0 {
                    let r = &mut self.rng;
                    let p = DVec3::new(bx + rf(r) as f64, by + 1.0 - liquid_height(st & 15) as f64 * 0.0, bz + rf(r) as f64);
                    self.spawn(env, 27, false, p, z0, &[]);
                }
            }
            _ => {}
        }
    }

    // --- desenho ------------------------------------------------------------------------------

    /// Textura própria pelo nome (`particle/particles`, `entity/explosion`, `items/...`).
    fn custom_tex(&mut self, name: &str, load: &mut dyn FnMut(&str) -> Option<u32>) -> Option<u32> {
        *self.tex_ids.entry(name.to_string()).or_insert_with(|| load(name))
    }

    /// Instâncias do quadro, na ordem do `EffectRenderer.renderParticles`; com muitas partículas cada
    /// camada é montada em paralelo (só leitura do mundo) e a ordem se mantém.
    pub fn draw(&mut self, env: &Env, cam: &Camera, partial: f32, load: &mut dyn FnMut(&str) -> Option<u32>) -> ParticleBatch {
        let (yaw, pitch) = (cam.yaw * std::f32::consts::PI / 180.0, cam.pitch * std::f32::consts::PI / 180.0);
        let mut batch = ParticleBatch {
            rot: [yaw.cos(), yaw.sin(), -yaw.sin() * pitch.sin(), yaw.cos() * pitch.sin(), pitch.cos()],
            ..Default::default()
        };
        let sheet = self.custom_tex("particle/particles", load);
        let explosion = self.custom_tex("entity/explosion", load);
        let sprites = self.sprites.clone();
        let mut sprite_tex: Vec<Option<u32>> = Vec::new();
        for s in &sprites {
            sprite_tex.push(self.custom_tex(s, load));
        }
        let (world, table, player) = (env.world, env.table, env.player);
        let mut parts = std::mem::take(&mut self.draw_scratch);
        for l in [0usize, 1, 3] {
            for j in 0..2 {
                let list = &self.layers[l][j];
                if list.is_empty() {
                    continue;
                }
                let one = |env: &mut Env, f: &Fx| -> Option<(Tex, ParticleInstance)> {
                    let (tex, uv) = match (l, f.icon) {
                        (0, _) => (sheet.map(Tex::Custom), None),
                        (3, _) => (explosion.map(Tex::Custom), None),
                        (_, Icon::Atlas(t)) => (Some(Tex::Blocks), Some(atlas_uv(t, f.jit))),
                        (_, Icon::Sprite(i)) => (sprite_tex[i as usize].map(Tex::Custom), Some(sprite_uv(f.jit))),
                        _ => (None, None),
                    };
                    let tex = tex?;
                    if f.k == K::Huge {
                        return None;
                    }
                    Some((tex, f.instance(env, partial, uv)?))
                };
                list.par_iter()
                    .with_min_len(PAR_MIN)
                    .map_init(|| Env { world, table, player, entity: &no_entity }, one)
                    .collect_into_vec(&mut parts);
                let mut cur: Option<(Tex, u32)> = None;
                for (tex, inst) in parts.drain(..).flatten() {
                    let n = batch.instances.len() as u32;
                    if cur.is_none_or(|(t, _)| t != tex) {
                        if let Some((t, start)) = cur.take() {
                            batch.draws.push(ParticleDraw { tex: t, translucent: j == 0, range: start..n });
                        }
                        cur = Some((tex, n));
                    }
                    batch.instances.push(inst);
                }
                if let Some((t, start)) = cur {
                    batch.draws.push(ParticleDraw { tex: t, translucent: j == 0, range: start..batch.instances.len() as u32 });
                }
            }
        }
        self.draw_scratch = parts;
        batch
    }
}

fn no_entity(_: i32) -> Option<EntView> {
    None
}

/// UV do trecho 1/4 x 1/4 do ícone no atlas de blocos (`getInterpolatedU/V`).
fn atlas_uv(tex: u16, jit: [f32; 2]) -> [f32; 4] {
    let n = ATLAS_COLS as f32;
    let (c, r) = ((tex as u32 % ATLAS_COLS) as f32, (tex as u32 / ATLAS_COLS) as f32);
    [(c + jit[0] / 4.0) / n, (c + (jit[0] + 1.0) / 4.0) / n, (r + jit[1] / 4.0) / n, (r + (jit[1] + 1.0) / 4.0) / n]
}

fn sprite_uv(jit: [f32; 2]) -> [f32; 4] {
    [jit[0] / 4.0, (jit[0] + 1.0) / 4.0, jit[1] / 4.0, (jit[1] + 1.0) / 4.0]
}

impl Fx {
    /// `renderParticle`: o quadrado virado para a câmera vira uma instância; os cantos
    /// (`interpPos` +/- `rotationX/XZ/Z/YZ/XY` vezes o tamanho) saem no vertex shader.
    fn instance(&self, env: &Env, pt: f32, icon_uv: Option<[f32; 4]>) -> Option<ParticleInstance> {
        let p = self.prev + (self.pos - self.prev) * pt as f64;
        let c = [p.x as f32, p.y as f32, p.z as f32];
        let (sky, blk) = self.light16(env, pt);
        let lv = |v: i32| (v as f32 * 17.0 / 16.0).clamp(0.0, 255.0) as u8;
        let light = [lv(sky), lv(blk), 255, 0];
        let col = |v: f32| (v.clamp(0.0, 1.0) * 255.0) as u8;
        let alpha = if icon_uv.is_some() { 1.0 } else { self.alpha };
        let color = [col(self.rgb[0]), col(self.rgb[1]), col(self.rgb[2]), col(alpha)];
        // Em camada 3 (explosão grande) a tela inteira é a textura 4x4 de quadros.
        let (u0, u1, v0, v1, s) = if self.k == K::Large {
            let n = (((self.counter as f32 + pt) * 15.0 / self.max_age as f32) as i32).max(0);
            if n > 15 {
                return None;
            }
            let u = (n % 4) as f32 / 4.0;
            let v = (n / 4) as f32 / 4.0;
            (u, u + 0.24975, v, v + 0.24975, 2.0 * self.base_scale)
        } else if let Some(uv) = icon_uv {
            (uv[0], uv[1], uv[2], uv[3], 0.1 * self.scale)
        } else {
            let u = (self.tex_idx % 16) as f32 / 16.0;
            let v = (self.tex_idx / 16) as f32 / 16.0;
            (u, u + 0.0624375, v, v + 0.0624375, 0.1 * self.render_scale(pt))
        };
        let flags = if icon_uv.is_some() { MIRRORED } else { 0 };
        Some(ParticleInstance { pos: c, size: s, uv: [u0, u1, v0, v1], color, light, flags })
    }
}

// ---------------------------------------------------------------------------------------------
// Pacotes
// ---------------------------------------------------------------------------------------------

/// Gaussiana padrão (`Random.nextGaussian`, método polar).
pub fn gaussian(r: &mut impl Rng) -> f64 {
    loop {
        let (a, b) = (2.0 * r.gen::<f64>() - 1.0, 2.0 * r.gen::<f64>() - 1.0);
        let s = a * a + b * b;
        if s < 1.0 && s != 0.0 {
            return a * (-2.0 * s.ln() / s).sqrt();
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParticlePacket {
    pub id: i32,
    pub long: bool,
    pub pos: DVec3,
    pub off: [f32; 3],
    pub speed: f32,
    pub count: i32,
    pub args: Vec<i32>,
}

/// S2A (0x2A).
pub fn parse_particles(body: &[u8]) -> mc_protocol::Result<ParticlePacket> {
    let mut r = mc_protocol::Reader::new(body);
    let raw = r.i32()?;
    // Id desconhecido vira BARRIER (35), como em `readPacketData`.
    let id = if (0..=41).contains(&raw) { raw } else { 35 };
    let long = r.bool()?;
    let pos = DVec3::new(r.f32()? as f64, r.f32()? as f64, r.f32()? as f64);
    let off = [r.f32()?, r.f32()?, r.f32()?];
    let speed = r.f32()?;
    let count = r.i32()?;
    let mut args = Vec::new();
    for _ in 0..particle_info(id).1 {
        args.push(r.varint()?);
    }
    Ok(ParticlePacket { id, long, pos, off, speed, count, args })
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExplosionPacket {
    pub pos: DVec3,
    pub strength: f32,
    pub blocks: Vec<(i32, i32, i32)>,
    /// Empurrão no jogador.
    pub push: [f32; 3],
}

/// S27 (0x27).
pub fn parse_explosion(body: &[u8]) -> mc_protocol::Result<ExplosionPacket> {
    let mut r = mc_protocol::Reader::new(body);
    let pos = DVec3::new(r.f32()? as f64, r.f32()? as f64, r.f32()? as f64);
    let strength = r.f32()?;
    let n = r.i32()?.max(0);
    let (ix, iy, iz) = (pos.x as i32, pos.y as i32, pos.z as i32);
    let mut blocks = Vec::new();
    for _ in 0..n {
        blocks.push((r.u8()? as i8 as i32 + ix, r.u8()? as i8 as i32 + iy, r.u8()? as i8 as i32 + iz));
    }
    let push = [r.f32()?, r.f32()?, r.f32()?];
    Ok(ExplosionPacket { pos, strength, blocks, push })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_world::Column;

    fn open_world() -> World {
        let mut w = World::new(true);
        for cx in -2..2 {
            for cz in -2..2 {
                w.columns.insert((cx, cz), Column::default());
            }
        }
        w
    }

    fn with_env<R>(w: &World, f: impl FnOnce(&Env) -> R) -> R {
        let t = BlockTable::new();
        let none = |_: i32| None;
        f(&Env { world: w, table: &t, player: (DVec3::ZERO, DVec3::ZERO), entity: &none })
    }

    fn fx(k: K, pos: DVec3, vel: DVec3) -> Fx {
        let mut r = StdRng::seed_from_u64(1);
        let mut f = Fx::base(&mut r, k, pos);
        f.vel = vel;
        f.max_age = 1000;
        f
    }

    #[test]
    fn block_break_makes_4x4x4() {
        let w = open_world();
        with_env(&w, |env| {
            let mut p = Particles::new(7);
            p.block_destroy(env, (3, 64, 3), 1 << 4);
            assert_eq!(p.count(), 64);
            assert_eq!(p.layer_len(1, 1), 64);
            // ar não gera nada
            p.block_destroy(env, (3, 64, 3), 0);
            assert_eq!(p.count(), 64);
        });
    }

    #[test]
    fn digging_particles_start_at_cell_centers_with_java_color() {
        let w = open_world();
        with_env(&w, |env| {
            let mut p = Particles::new(7);
            p.block_destroy(env, (0, 10, 0), 1 << 4);
            let f = &p.layers[1][1][0];
            assert_eq!(f.pos, DVec3::new(0.125, 10.125, 0.125));
            // 0.6 * (cor do bloco = branco)
            assert_eq!(f.rgb, [0.6, 0.6, 0.6]);
            assert_eq!(f.gravity, 1.0);
            assert!(f.scale > 0.0);
        });
    }

    #[test]
    fn base_fx_physics_matches_java() {
        // EntityFX.onUpdate: motionY -= 0.04 * gravity; mover; * 0.98f.
        let w = open_world();
        with_env(&w, |env| {
            let mut r = StdRng::seed_from_u64(3);
            let mut f = fx(K::Digging, DVec3::new(0.5, 100.0, 0.5), DVec3::new(0.1, 0.2, 0.0));
            f.gravity = 1.0;
            let (mut y, mut vy, mut x, mut vx) = (100.0f64, 0.2f64, 0.5f64, 0.1f64);
            for _ in 0..10 {
                f.update(env, &mut r, &mut Vec::new());
                vy -= 0.04;
                y += vy;
                x += vx;
                vy *= 0.9800000190734863;
                vx *= 0.9800000190734863;
                assert!((f.pos.y - y).abs() < 1e-12, "y {} {}", f.pos.y, y);
                assert!((f.pos.x - x).abs() < 1e-12);
                assert!((f.vel.y - vy).abs() < 1e-12);
            }
            assert_eq!(f.age, 10);
        });
    }

    #[test]
    fn smoke_rises_and_slows_like_java() {
        let w = open_world();
        with_env(&w, |env| {
            let mut r = StdRng::seed_from_u64(3);
            let mut f = fx(K::Smoke, DVec3::new(0.5, 70.0, 0.5), DVec3::new(0.0, 0.1, 0.0));
            let mut vy = 0.1f64;
            for n in 1..=5 {
                f.update(env, &mut r, &mut Vec::new());
                vy += 0.004;
                vy *= 0.9599999785423279;
                assert!((f.vel.y - vy).abs() < 1e-12);
                // 7 - age * 8 / maxAge com divisão inteira
                assert_eq!(f.tex_idx, 7 - n * 8 / 1000);
            }
        });
    }

    #[test]
    fn flame_and_bubble_and_crit_follow_java() {
        let w = open_world();
        with_env(&w, |env| {
            let mut r = StdRng::seed_from_u64(3);
            let mut fl = fx(K::Flame, DVec3::new(0.5, 70.0, 0.5), DVec3::new(0.0, 0.05, 0.0));
            fl.no_clip = true;
            fl.update(env, &mut r, &mut Vec::new());
            assert!((fl.vel.y - 0.05 * 0.9599999785423279).abs() < 1e-12);
            assert!((fl.pos.y - 70.05).abs() < 1e-12);

            // Crítico: green * 0.96, blue * 0.9, motionY -= 0.02, * 0.7.
            let mut c = fx(K::Crit2, DVec3::new(0.5, 70.0, 0.5), DVec3::new(0.0, 0.2, 0.0));
            c.rgb = [1.0; 3];
            c.update(env, &mut r, &mut Vec::new());
            assert!((c.rgb[1] - 0.96).abs() < 1e-6 && (c.rgb[2] - 0.9).abs() < 1e-6);
            assert!((c.vel.y - (0.2 * 0.699999988079071 - 0.019999999552965164)).abs() < 1e-12);
        });
    }

    #[test]
    fn particle_stops_on_floor() {
        // Um bloco de pedra em y=63: a partícula cai e fica em cima.
        let mut w = open_world();
        w.set_block(0, 63, 0, 1 << 4);
        let t = BlockTable::new();
        let none = |_: i32| None;
        let env = Env { world: &w, table: &t, player: (DVec3::ZERO, DVec3::ZERO), entity: &none };
        let mut r = StdRng::seed_from_u64(3);
        let mut f = fx(K::Digging, DVec3::new(0.5, 66.0, 0.5), DVec3::ZERO);
        f.gravity = 1.0;
        for _ in 0..40 {
            f.update(&env, &mut r, &mut Vec::new());
        }
        assert!(f.ground);
        assert!((f.pos.y - 64.0).abs() < 1e-9, "{}", f.pos.y);
    }

    #[test]
    fn range_and_setting_filters() {
        let w = open_world();
        with_env(&w, |env| {
            let mut p = Particles::new(1);
            // Perto: entra. Longe (> 16 blocos): não entra, a não ser que ignore a distância.
            assert!(p.spawn(env, 11, false, DVec3::new(1.0, 1.0, 1.0), DVec3::ZERO, &[]).is_some());
            assert!(p.spawn(env, 11, false, DVec3::new(20.0, 0.0, 0.0), DVec3::ZERO, &[]).is_none());
            assert!(p.spawn(env, 11, true, DVec3::new(20.0, 0.0, 0.0), DVec3::ZERO, &[]).is_some());
            // Mínimas: só as de longa distância.
            p.setting = 2;
            assert!(p.spawn(env, 11, false, DVec3::new(1.0, 1.0, 1.0), DVec3::ZERO, &[]).is_none());
            assert!(p.spawn(env, 11, true, DVec3::new(1.0, 1.0, 1.0), DVec3::ZERO, &[]).is_some());
            // Diminuídas: cerca de 2/3 passam.
            p.setting = 1;
            p.clear();
            for _ in 0..3000 {
                p.spawn(env, 11, false, DVec3::new(1.0, 1.0, 1.0), DVec3::ZERO, &[]);
            }
            let n = p.count();
            assert!((1800..2200).contains(&n), "{n}");
        });
    }

    /// O laço do `updateEffectLayer` todo em série: a referência do tick paralelo.
    fn tick_serial(p: &mut Particles, env: &Env) {
        for l in 0..4 {
            for j in 0..2 {
                let mut i = 0;
                while i < p.layers[l][j].len() {
                    let mut out = Vec::new();
                    {
                        let Particles { layers, rng, .. } = &mut *p;
                        layers[l][j][i].update(env, rng, &mut out);
                    }
                    for s in out {
                        p.spawn_type(env, s.id, s.pos, s.vel);
                    }
                    i += 1;
                }
                p.layers[l][j].retain(|f| !f.dead);
            }
        }
    }

    #[test]
    fn parallel_tick_matches_serial() {
        let mut w = open_world();
        for x in -8..8 {
            for z in -8..8 {
                w.set_block(x, 1, z, 1 << 4);
            }
        }
        with_env(&w, |env| {
            // Camada 0 cheia, com lava soltando fumaça nela mesma (`remove(0)` no meio do laço) e chuva.
            let fill = |p: &mut Particles, n: usize, t: usize| {
                for i in 0..n {
                    let id = [26, 11, 27, 30, 9, 23, 13, 29, 0, 19, 34, 39][(i + t) % 12];
                    let k = (i * 7 + t * 13) as f64;
                    let pos = DVec3::new((k * 0.37) % 12.0 - 6.0, 2.0 + (k * 0.11) % 3.0, (k * 0.53) % 12.0 - 6.0);
                    p.spawn(env, id, true, pos, DVec3::new(0.0, -0.05, 0.0), &[]);
                }
                for b in 0..8 {
                    p.block_destroy(env, (b - 4, 3, (t % 8) as i32 - 4), (1 + b as u16 % 3) << 4);
                }
            };
            let (mut a, mut b) = (Particles::new(7), Particles::new(7));
            fill(&mut a, 3990, 0);
            fill(&mut b, 3990, 0);
            let pops0 = a.pops[0][1];
            for t in 1..60 {
                a.tick(env);
                tick_serial(&mut b, env);
                assert_eq!(format!("{:?}", a.layers), format!("{:?}", b.layers), "tick {t}");
                fill(&mut a, 300, t);
                fill(&mut b, 300, t);
            }
            assert!(a.layer_len(0, 1) >= 3000 && a.layer_len(1, 1) >= 256);
            assert!(a.pops[0][1] > pops0);
            assert_eq!(a.rng.gen::<u64>(), b.rng.gen::<u64>());
        });
    }

    #[test]
    fn layer_cap_is_4000() {
        let w = open_world();
        with_env(&w, |env| {
            let mut p = Particles::new(1);
            for _ in 0..4100 {
                p.spawn(env, 11, true, DVec3::new(1.0, 1.0, 1.0), DVec3::ZERO, &[]);
            }
            assert_eq!(p.layer_len(0, 1), 4000);
        });
    }

    #[test]
    fn parse_particle_packet() {
        let mut b = Vec::new();
        b.extend(37i32.to_be_bytes());
        b.push(1);
        for v in [1.5f32, 64.0, -3.25, 0.1, 0.2, 0.3, 0.5] {
            b.extend(v.to_be_bytes());
        }
        b.extend(4i32.to_be_bytes());
        b.push(0x91);
        b.push(0x01); // varint 145
        let p = parse_particles(&b).unwrap();
        assert_eq!(p.id, 37);
        assert!(p.long);
        assert_eq!(p.pos, DVec3::new(1.5, 64.0, -3.25));
        assert_eq!(p.count, 4);
        assert_eq!(p.args, vec![145]);
        assert_eq!(state_from_arg(145), 145 << 4);
        assert_eq!(state_from_arg(9 | 3 << 12), 9 << 4 | 3);
        // id desconhecido vira barreira
        b[..4].copy_from_slice(&99i32.to_be_bytes());
        assert_eq!(parse_particles(&b).unwrap().id, 35);
    }

    #[test]
    fn parse_explosion_packet() {
        let mut b = Vec::new();
        for v in [10.5f32, 64.0, -3.5, 4.0] {
            b.extend(v.to_be_bytes());
        }
        b.extend(2i32.to_be_bytes());
        b.extend([1u8, 255, 0, 2, 3, 4]);
        for v in [0.5f32, 0.25, -0.5] {
            b.extend(v.to_be_bytes());
        }
        let e = parse_explosion(&b).unwrap();
        assert_eq!(e.blocks, vec![(11, 63, -3), (12, 67, 1)]);
        assert_eq!(e.push, [0.5, 0.25, -0.5]);
        assert_eq!(e.strength, 4.0);
    }

    #[test]
    fn explosion_and_effects_spawn_particles() {
        let w = open_world();
        with_env(&w, |env| {
            let mut p = Particles::new(5);
            let e = ExplosionPacket { pos: DVec3::new(2.0, 2.0, 2.0), strength: 4.0, blocks: vec![(2, 1, 2), (3, 1, 2)], push: [0.0; 3] };
            p.explosion(env, &e);
            // 1 explosão enorme + por bloco: 1 explode + 1 fumaça
            assert_eq!(p.count(), 1 + 2 * 2);
            p.clear();
            p.effect(env, 2004, (2, 2, 2), 0);
            assert_eq!(p.count(), 40);
            p.clear();
            p.effect(env, 2000, (2, 2, 2), 4);
            assert_eq!(p.count(), 10);
            p.clear();
            p.effect(env, 2001, (2, 2, 2), 1);
            assert_eq!(p.count(), 64);
        });
    }

    #[test]
    fn crit_emitter_makes_particles_for_three_ticks() {
        let w = open_world();
        let t = BlockTable::new();
        let ent = |id: i32| (id == 5).then_some(EntView { pos: DVec3::new(1.0, 64.0, 1.0), width: 0.6, height: 1.8 });
        let env = Env { world: &w, table: &t, player: (DVec3::new(1.0, 64.0, 1.0), DVec3::ZERO), entity: &ent };
        let mut p = Particles::new(5);
        p.emit_at_entity(&env, 5, 9);
        let first = p.count();
        assert!(first > 0 && first <= 16);
        p.tick(&env);
        p.tick(&env);
        p.tick(&env);
        assert!(p.emitters.is_empty());
    }
}
