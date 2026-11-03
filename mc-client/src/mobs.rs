//! Mobs da 1.8.9 com modelo do jogo: as caixas vêm de `gen_mob_models.rs` (geradas dos
//! construtores `ModelXxx` pelo `scripts/gen_mob_models.py`) e as poses são os
//! `setRotationAngles` portados com os mesmos números do Java.

pub mod horse;
pub mod hostile;
pub mod boss;
pub mod misc;
pub mod pets;
pub mod stands;
#[cfg(test)]
mod group_tests;

use std::collections::HashMap;

use glam::{DVec3, Mat4, Vec3};
use mc_render::{Jar, Renderer, Tex};
use mc_world::mesh::Geometry;

use crate::entities::Entity;
use crate::entity_net::MetaValue;
use crate::gen_mob_models as g;
use crate::mob_rig::Rig;
use crate::models::{Anim, Sink, FULL_LIGHT, WORLD_LIGHTS};
use crate::mth::{cos, sin, PI};
use crate::skins::Textures;

/// Marca no id de textura personalizada: a geometria vai para o passe translúcido.
pub const TRANSLUCENT: u32 = 1 << 31;
/// Marca no id de textura: mistura aditiva (`blendFunc(1, 1)`), sem escrita de profundidade (camadas de olhos e carga).
pub const ADDITIVE: u32 = 1 << 30;

/// Entrada de um tick de `onUpdate`/`onLivingUpdate` (o que as classes de entidade leem).
pub struct TickIn<'a> {
    pub id: u8,
    pub eid: i32,
    pub meta: &'a HashMap<u8, MetaValue>,
    pub on_ground: bool,
    /// `inWater` (`Entity.handleWaterMovement`).
    pub in_water: bool,
    /// `renderYawOffset`.
    pub body_yaw: f32,
    /// `rotationYaw` e `posY` (o anel de movimento do dragão).
    pub yaw: f32,
    pub y: f64,
    /// `motionX/Y/Z` (pacotes de velocidade).
    pub vel: DVec3,
}

/// `java.util.Random` (LCG de 48 bits).
#[derive(Clone, Debug)]
pub struct JRandom(u64);

impl JRandom {
    pub fn new(seed: i64) -> Self {
        Self((seed as u64 ^ 0x5DEECE66D) & ((1 << 48) - 1))
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.0 = (self.0.wrapping_mul(0x5DEECE66D).wrapping_add(0xB)) & ((1 << 48) - 1);
        (self.0 >> (48 - bits)) as i32
    }

    pub fn next_float(&mut self) -> f32 {
        self.next(24) as f32 / (1 << 24) as f32
    }

    pub fn next_int(&mut self, bound: i32) -> i32 {
        if bound & -bound == bound {
            return ((bound as i64 * self.next(31) as i64) >> 31) as i32;
        }
        loop {
            let bits = self.next(31);
            let val = bits % bound;
            if bits - val + (bound - 1) >= 0 {
                return val;
            }
        }
    }
}

impl TickIn<'_> {
    pub fn byte(&self, i: u8) -> i8 {
        match self.meta.get(&i) {
            Some(MetaValue::Byte(b)) => *b,
            _ => 0,
        }
    }
}

/// Estado por tick que o Java guarda nas entidades e o modelo lê (`EntityCreeper`, `EntityChicken`,
/// `EntityWolf`, `EntityRabbit`, `EntitySlime`...).
#[derive(Clone, Debug, Default)]
pub struct MobState {
    /// `timeSinceIgnited` do creeper (anterior e atual).
    pub swell_prev: i32,
    pub swell: i32,
    /// `EntityChicken`: `wingRotation`, `destPos`, `wingRotDelta` e os valores do tick anterior.
    pub wing_rot: f32,
    pub wing_rot_prev: f32,
    pub dest_pos: f32,
    pub dest_pos_prev: f32,
    pub wing_delta: f32,
    /// `EntityWolf`: `headRotationCourse`, `isShaking`, `timeWolfIsShaking` (e anteriores).
    pub head_course: f32,
    pub head_course_prev: f32,
    pub wolf_shaking: bool,
    pub wolf_wet: bool,
    pub shake: f32,
    pub shake_prev: f32,
    /// `EntityRabbit`: `field_175540_bm` (contador do pulo) e `field_175535_bn` (duração).
    pub hop: i32,
    pub hop_len: i32,
    /// `EntitySlime`/`EntityMagmaCube`: `squishAmount`, `squishFactor`, `prevSquishFactor`, `wasOnGround`.
    pub squish_amount: f32,
    pub squish: f32,
    pub squish_prev: f32,
    pub was_on_ground: bool,
    /// `EntitySquid`: `squidPitch`, `squidYaw`, `squidRotation`, `tentacleAngle` (e anteriores),
    /// `rotationVelocity` e `field_70871_bB` (velocidade de giro).
    pub squid_pitch: f32,
    pub squid_pitch_prev: f32,
    pub squid_yaw: f32,
    pub squid_yaw_prev: f32,
    pub squid_rot: f32,
    pub squid_rot_vel: f32,
    pub tentacle: f32,
    pub tentacle_prev: f32,
    pub squid_spin: f32,
    pub squid_init: bool,
    /// `EntityIronGolem`: `attackTimer` e `holdRoseTick`.
    pub golem_attack: i32,
    pub golem_rose: i32,
    /// `EntityGuardian`: `field_175482_b` (cauda) e anterior, `field_175483_bk` (velocidade),
    /// `field_175485_bl` (espinhos) e anterior, `field_175479_bo` (contagem do laser).
    pub guardian_tail: f32,
    pub guardian_tail_prev: f32,
    pub guardian_speed: f32,
    pub guardian_spikes: f32,
    pub guardian_spikes_prev: f32,
    pub guardian_laser: i32,
    pub rng: Option<JRandom>,
    /// `EntityWither`: `field_82221_e` (giro das cabeças laterais), `field_82220_d` (inclinação).
    pub wither_yaw: [f32; 2],
    pub wither_pitch: [f32; 2],
    /// `EntityDragon`: `animTime` (e anterior), anel `ringBuffer[64]` de (yaw, y) e `ringBufferIndex`.
    pub dragon_anim: f32,
    pub dragon_anim_prev: f32,
    pub dragon_ring: Vec<[f64; 2]>,
    pub dragon_idx: i32,
    /// `EntityHorse`: `headLean` (comer), `rearingAmount`, `mouthOpenness` (e anteriores) e `field_110278_bp` (rabo).
    pub horse_lean: f32,
    pub horse_lean_prev: f32,
    pub horse_rear: f32,
    pub horse_rear_prev: f32,
    pub horse_mouth: f32,
    pub horse_mouth_prev: f32,
    pub horse_tail: i32,
}

impl MobState {
    /// `EntityDragon.getMovementOffsets(p, partial)` (yaw, y); `dead` zera o parcial.
    pub fn dragon_offsets(&self, p: i32, partial: f32, dead: bool) -> [f64; 2] {
        if self.dragon_ring.len() != 64 {
            return [0.0, 0.0];
        }
        let partial = 1.0 - if dead { 0.0 } else { partial } as f64;
        let i = ((self.dragon_idx - p) & 63) as usize;
        let j = ((self.dragon_idx - p - 1) & 63) as usize;
        let d0 = self.dragon_ring[i][0];
        let mut d1 = (self.dragon_ring[j][0] - d0) % 360.0;
        if d1 >= 180.0 {
            d1 -= 360.0;
        }
        if d1 < -180.0 {
            d1 += 360.0;
        }
        [d0 + d1 * partial, self.dragon_ring[i][1] + (self.dragon_ring[j][1] - self.dragon_ring[i][1]) * partial]
    }
}

impl MobState {
    /// Um tick de `onUpdate`/`onLivingUpdate` do cliente.
    pub fn tick(&mut self, i: &TickIn) {
        let on_ground = i.on_ground;
        match i.id {
            50 => {
                self.swell_prev = self.swell;
                self.swell += i.byte(16) as i32;
                self.swell = self.swell.max(0).min(30);
            }
            93 => {
                self.wing_rot_prev = self.wing_rot;
                self.dest_pos_prev = self.dest_pos;
                self.dest_pos = ((self.dest_pos as f64) + (if on_ground { -1.0 } else { 4.0 }) * 0.3) as f32;
                self.dest_pos = self.dest_pos.clamp(0.0, 1.0);
                if !on_ground && self.wing_delta < 1.0 {
                    self.wing_delta = 1.0;
                }
                self.wing_delta = (self.wing_delta as f64 * 0.9) as f32;
                self.wing_rot += self.wing_delta * 2.0;
            }
            95 => {
                // EntityWolf.onUpdate (`isWet()` aqui é só dentro d'água; a chuva não entra).
                self.head_course_prev = self.head_course;
                let begging = i.byte(19) == 1;
                self.head_course += ((if begging { 1.0 } else { 0.0 }) - self.head_course) * 0.4;
                if i.in_water {
                    self.wolf_wet = true;
                    self.wolf_shaking = false;
                    self.shake = 0.0;
                    self.shake_prev = 0.0;
                } else if (self.wolf_wet || self.wolf_shaking) && self.wolf_shaking {
                    self.shake_prev = self.shake;
                    self.shake += 0.05;
                    if self.shake_prev >= 2.0 {
                        self.wolf_wet = false;
                        self.wolf_shaking = false;
                        self.shake_prev = 0.0;
                        self.shake = 0.0;
                    }
                }
            }
            101 => {
                // EntityRabbit.onLivingUpdate.
                if self.hop != self.hop_len {
                    self.hop += 1;
                } else if self.hop_len != 0 {
                    self.hop = 0;
                    self.hop_len = 0;
                }
            }
            55 | 62 => {
                // EntitySlime.onUpdate / EntityMagmaCube.alterSquishAmount.
                self.squish += (self.squish_amount - self.squish) * 0.5;
                self.squish_prev = self.squish;
                if on_ground && !self.was_on_ground {
                    self.squish_amount = -0.5;
                } else if !on_ground && self.was_on_ground {
                    self.squish_amount = 1.0;
                }
                self.was_on_ground = on_ground;
                self.squish_amount *= if i.id == 62 { 0.9 } else { 0.6 };
            }
            94 => {
                // EntitySquid.onLivingUpdate (o lado remoto: sem passeio aleatório, só animação).
                if !self.squid_init {
                    self.squid_init = true;
                    self.squid_rot_vel = 1.0 / (JRandom::new(1 + i.eid as i64).next_float() + 1.0) * 0.2;
                }
                self.squid_pitch_prev = self.squid_pitch;
                self.squid_yaw_prev = self.squid_yaw;
                self.tentacle_prev = self.tentacle;
                self.squid_rot += self.squid_rot_vel;
                if self.squid_rot as f64 > 6.283185307179586 {
                    self.squid_rot = 6.2831855;
                }
                if i.in_water {
                    if self.squid_rot < PI {
                        let f = self.squid_rot / PI;
                        self.tentacle = sin(f * f * PI) * PI * 0.25;
                        if f as f64 > 0.75 {
                            self.squid_spin = 1.0;
                        } else {
                            self.squid_spin *= 0.8;
                        }
                    } else {
                        self.tentacle = 0.0;
                        self.squid_spin *= 0.99;
                    }
                    let f1 = (i.vel.x * i.vel.x + i.vel.z * i.vel.z).sqrt();
                    self.squid_yaw = (self.squid_yaw as f64 + std::f64::consts::PI * self.squid_spin as f64 * 1.5) as f32;
                    self.squid_pitch += (-(f1.atan2(i.vel.y) as f32) * 180.0 / PI - self.squid_pitch) * 0.1;
                } else {
                    self.tentacle = sin(self.squid_rot).abs() * PI * 0.25;
                    self.squid_pitch = (self.squid_pitch as f64 + (-90.0 - self.squid_pitch) as f64 * 0.02) as f32;
                }
            }
            100 => {
                let flags = match i.meta.get(&16) {
                    Some(MetaValue::Int(v)) => *v,
                    _ => 0,
                };
                horse::tick(self, flags, 1 + i.eid as i64);
            }
            63 => {
                // EntityDragon.onLivingUpdate: animTime e anel de movimento (vivo).
                self.dragon_anim_prev = self.dragon_anim;
                let dead = matches!(i.meta.get(&6), Some(MetaValue::Float(h)) if *h <= 0.0);
                if !dead {
                    let mut f10 = 0.2 / ((i.vel.x * i.vel.x + i.vel.z * i.vel.z).sqrt() as f32 * 10.0 + 1.0);
                    f10 *= 2f64.powf(i.vel.y) as f32;
                    self.dragon_anim += f10;
                    let yaw = crate::mth::wrap_degrees(i.yaw) as f64;
                    if self.dragon_ring.is_empty() {
                        self.dragon_ring = vec![[yaw, i.y]; 64];
                        self.dragon_idx = -1;
                    }
                    self.dragon_idx += 1;
                    if self.dragon_idx == 64 {
                        self.dragon_idx = 0;
                    }
                    self.dragon_ring[self.dragon_idx as usize] = [yaw, i.y];
                }
            }
            68 => {
                // EntityGuardian.onLivingUpdate (lado do cliente).
                let moving = match i.meta.get(&16) {
                    Some(MetaValue::Int(v)) => v & 2 != 0,
                    _ => false,
                };
                let has_target = matches!(i.meta.get(&17), Some(MetaValue::Int(v)) if *v != 0);
                let elder = matches!(i.meta.get(&16), Some(MetaValue::Int(v)) if v & 4 != 0);
                self.guardian_tail_prev = self.guardian_tail;
                if !i.in_water {
                    self.guardian_speed = 2.0;
                } else if moving {
                    if self.guardian_speed < 0.5 {
                        self.guardian_speed = 4.0;
                    } else {
                        self.guardian_speed += (0.5 - self.guardian_speed) * 0.1;
                    }
                } else {
                    self.guardian_speed += (0.125 - self.guardian_speed) * 0.2;
                }
                self.guardian_tail += self.guardian_speed;
                self.guardian_spikes_prev = self.guardian_spikes;
                if !i.in_water {
                    let rng = self.rng.get_or_insert_with(|| JRandom::new(1 + i.eid as i64));
                    self.guardian_spikes = rng.next_float();
                } else if moving {
                    self.guardian_spikes += (0.0 - self.guardian_spikes) * 0.25;
                } else {
                    self.guardian_spikes += (1.0 - self.guardian_spikes) * 0.06;
                }
                if has_target && self.guardian_laser < if elder { 60 } else { 80 } {
                    self.guardian_laser += 1;
                }
            }
            64 => {
                // EntityWither.onLivingUpdate: cabeça lateral sem alvo (metadado 18/19) segue o corpo; com alvo,
                // `Entities::tick_wither_heads` mira nele.
                let step = |a: f32, b: f32, max: f32| a + crate::mth::wrap_degrees(b - a).clamp(-max, max);
                for j in 0..2 {
                    let target = match i.meta.get(&(18 + j as u8)) {
                        Some(MetaValue::Int(v)) => *v,
                        _ => 0,
                    };
                    if target <= 0 {
                        self.wither_yaw[j] = step(self.wither_yaw[j], i.body_yaw, 10.0);
                    }
                }
            }
            99 => {
                // EntityIronGolem.onLivingUpdate.
                self.golem_attack = (self.golem_attack - 1).max(0);
                self.golem_rose = (self.golem_rose - 1).max(0);
            }
            _ => {}
        }
    }

    /// `handleStatusUpdate` dos mobs que animam por estado.
    pub fn status(&mut self, id: u8, status: i8) {
        match (id, status) {
            // EntityWolf: sacudir.
            (95, 8) => {
                self.wolf_shaking = true;
                self.shake = 0.0;
                self.shake_prev = 0.0;
            }
            // EntityRabbit: pulo.
            (101, 1) => {
                self.hop_len = 10;
                self.hop = 0;
            }
            // EntitySquid: reinicia o ciclo dos tentáculos.
            (94, 19) => self.squid_rot = 0.0,
            // EntityIronGolem: soco e rosa.
            (99, 4) => self.golem_attack = 10,
            (99, 11) => self.golem_rose = 400,
            _ => {}
        }
    }
}

pub(crate) const DEG: f32 = 57.295776;

/// Onde um mob grava: texturas, renderer, jar e as geometrias por textura.
pub struct Ctx<'a> {
    pub textures: &'a mut Textures,
    pub r: &'a mut Renderer,
    pub jar: &'a mut Jar,
    pub out: &'a mut HashMap<Tex, Geometry>,
    pub light: [u8; 4],
    pub tint: [f32; 3],
    /// `Entity.getBrightness(partialTicks)` no ponto dos olhos.
    pub brightness: f32,
    pub items: &'a mut crate::first_person::ItemModels,
    pub table: &'a mc_world::blocks::BlockTable,
    /// Luzes do `RenderHelper` no espaço do modelo (no mundo, `WORLD_LIGHTS`).
    pub lights: [Vec3; 2],
    /// O corpo sai (ver `naovivas::Fx::body`).
    pub body: bool,
}

impl Ctx<'_> {
    pub(crate) fn sink(&mut self, tex: u32, tint: [f32; 3], alpha: u8, light: [u8; 4]) -> Sink<'_> {
        Sink { geo: self.out.entry(Tex::Custom(tex)).or_default(), tex: (64.0, 32.0), tint, alpha, light, lights: self.lights }
    }
}

/// Matriz do `RendererLivingEntity.doRender` com escala por eixo (creeper).
pub(crate) fn living(pos: Vec3, body: f32, death: f32, s: Vec3) -> Mat4 {
    Mat4::from_translation(pos)
        * Mat4::from_rotation_y((180.0 - body).to_radians())
        * Mat4::from_rotation_z(death.to_radians())
        * Mat4::from_scale(Vec3::new(-s.x, -s.y, s.z))
        * Mat4::from_translation(Vec3::new(0.0, -1.5078125, 0.0))
}

/// Normais de `EnumFacing` na ordem baixo, cima, norte, sul, oeste, leste.
const FACE_NORMALS: [Vec3; 6] = [Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 0.0, 1.0), Vec3::new(-1.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)];

/// `BlockRendererDispatcher.renderBlockBrightness(state, 1.0)`: o modelo do estado (id << 4 | meta)
/// em coordenadas de bloco (0..1) na matriz `m`, no atlas de blocos.
pub(crate) fn emit_block(cx: &mut Ctx, state: u16, m: &Mat4, light: [u8; 4]) {
    use mc_world::mesh::{atlas_uv, Vertex};
    let Some(models) = cx.table.models.as_ref() else { return };
    let Some(w) = models.get(state, 0) else { return };
    let Some((_, model)) = w.models.first() else { return };
    let tint = cx.table.tint_color(state);
    let geo = cx.out.entry(Tex::Blocks).or_default();
    for q in &model.quads {
        let n = m.transform_vector3(FACE_NORMALS[q.face as usize % 6]).normalize_or_zero();
        let k = crate::models::diffuse_with(n, &cx.lights);
        let c = if q.tint >= 0 { tint } else { 0xFFFFFF };
        let ch = |sh: u32| (((c >> sh) & 0xFF) as f32 * k) as u8;
        let color = [ch(16), ch(8), ch(0), 255];
        let vert = |i: usize| Vertex { pos: m.transform_point3(Vec3::from(q.pos[i])).to_array(), uv: atlas_uv(q.tex, q.uv[i][0], q.uv[i][1]), color, light };
        geo.quad([vert(0), vert(1), vert(2), vert(3)]);
    }
}

/// Pernas de quadrúpede (`ModelQuadruped`/`ModelCreeper`): pares opostos em fase.
fn legs4(rig: &mut Rig, names: [&str; 4], a: &Anim) {
    let swing = |ph: f32| cos(a.limb_swing * 0.6662 + ph) * 1.4 * a.limb_amount;
    let l: Vec<usize> = names.iter().map(|n| rig.i(n)).collect();
    rig.rot[l[0]].x = swing(0.0);
    rig.rot[l[1]].x = swing(PI);
    rig.rot[l[2]].x = swing(PI);
    rig.rot[l[3]].x = swing(0.0);
}

/// `ModelQuadruped.setRotationAngles`.
pub fn pose_quadruped(rig: &mut Rig, a: &Anim) {
    let (h, b) = (rig.i("head"), rig.i("body"));
    rig.rot[h].x = a.head_pitch / DEG;
    rig.rot[h].y = a.head_yaw / DEG;
    rig.rot[b].x = 1.5707964;
    legs4(rig, ["leg1", "leg2", "leg3", "leg4"], a);
}

/// `ModelQuadruped.render`: filhote = cabeça deslocada e o resto com metade do tamanho.
fn emit_quadruped(rig: &Rig, sink: &mut Sink, m: &Mat4, child: bool, child_y: f32, child_z: f32) {
    sink.tex = rig.tex();
    let (head, rest) = (rig.i("head"), ["body", "leg1", "leg2", "leg3", "leg4"].map(|n| rig.i(n)));
    if child {
        rig.emit(sink, &(*m * Mat4::from_translation(Vec3::new(0.0, child_y / 16.0, child_z / 16.0))), &[head]);
        let m2 = *m * Mat4::from_scale(Vec3::splat(0.5)) * Mat4::from_translation(Vec3::new(0.0, 24.0 / 16.0, 0.0));
        rig.emit(sink, &m2, &rest);
    } else {
        rig.emit(sink, m, &[head]);
        rig.emit(sink, m, &rest);
    }
}

/// `ModelSpider.setRotationAngles`.
pub fn pose_spider(rig: &mut Rig, a: &Anim) {
    let l: Vec<usize> = (1..=8).map(|i| rig.i(&format!("spiderLeg{i}"))).collect();
    let head = rig.i("spiderHead");
    rig.rot[head].y = a.head_yaw / DEG;
    rig.rot[head].x = a.head_pitch / DEG;
    let (z0, y0) = (0.7853982f32, 0.3926991f32);
    let z = [-z0, z0, -z0 * 0.74, z0 * 0.74, -z0 * 0.74, z0 * 0.74, -z0, z0];
    let y = [y0 * 2.0, -y0 * 2.0, y0, -y0, -y0, y0, -y0 * 2.0, y0 * 2.0];
    let s = a.limb_swing * 0.6662;
    let s2 = a.limb_swing * 0.6662 * 2.0;
    let dy = [
        -(cos(s2) * 0.4) * a.limb_amount,
        -(cos(s2 + PI) * 0.4) * a.limb_amount,
        -(cos(s2 + 1.5707964) * 0.4) * a.limb_amount,
        -(cos(s2 + 4.712389) * 0.4) * a.limb_amount,
    ];
    let dz = [
        (sin(s) * 0.4).abs() * a.limb_amount,
        (sin(s + PI) * 0.4).abs() * a.limb_amount,
        (sin(s + 1.5707964) * 0.4).abs() * a.limb_amount,
        (sin(s + 4.712389) * 0.4).abs() * a.limb_amount,
    ];
    for i in 0..8 {
        let sign = if i % 2 == 0 { 1.0 } else { -1.0 };
        rig.rot[l[i]].z = z[i] + sign * dz[i / 2];
        rig.rot[l[i]].y = y[i] + sign * dy[i / 2];
    }
}

/// Objetos com modelo próprio (armor stand 78, cristal do End 51); `false` cai na caixa.
pub fn draw_object(e: &Entity, kind: u8, pos: Vec3, t: f32, body: f32, cx: &mut Ctx) -> bool {
    match kind {
        78 => stands::armor_stand(e, pos, body, cx),
        51 => stands::crystal(e, pos, t, cx),
        _ => false,
    }
}

/// Desenha o mob se ele já tem modelo; devolve `false` para cair na caixa.
pub fn draw(e: &Entity, id: u8, pos: Vec3, t: f32, anim: &Anim, body: f32, death: f32, cx: &mut Ctx) -> bool {
    let child = anim.child;
    let (tint, light) = (cx.tint, cx.light);
    match id {
        // EntityCreeper: inchaço antes de explodir (RenderCreeper.preRenderCallback) e camada de carga.
        50 => {
            let Some(tex) = cx.textures.jar(cx.r, cx.jar, "entity/creeper/creeper") else { return true };
            let s = &e.mob_state;
            let flash = (s.swell_prev as f32 + (s.swell - s.swell_prev) as f32 * t) / 28.0;
            let f1 = 1.0 + sin(flash * 100.0) * flash * 0.01;
            let f = flash.clamp(0.0, 1.0);
            let f = f * f;
            let f = f * f;
            let (sx, sy) = ((1.0 + f * 0.4) * f1, (1.0 + f * 0.1) / f1);
            let m = living(pos, body, death, Vec3::new(sx, sy, sx));
            let mut rig = Rig::new(&g::CREEPER);
            let (h, hy) = (rig.i("head"), rig.i("head"));
            let _ = hy;
            rig.rot[h].y = anim.head_yaw / DEG;
            rig.rot[h].x = anim.head_pitch / DEG;
            legs4(&mut rig, ["leg1", "leg2", "leg3", "leg4"], anim);
            let roots = ["head", "body", "leg1", "leg2", "leg3", "leg4"].map(|n| rig.i(n));
            rig.emit(&mut cx.sink(tex, tint, 255, light), &m, &roots);
            if e.meta_byte(17) == 1 {
                if let Some(armor) = cx.textures.jar(cx.r, cx.jar, "entity/creeper/creeper_armor") {
                    // LayerCreeperCharge: ModelCreeper(2.0), textura rolando ticks*0.01, cor 0.5, sem luz
                    // (`disableLighting`) e `blendFunc(1, 1)`.
                    let mut charged = Rig::new(&g::CREEPER_CHARGED);
                    charged.rot = rig.rot.clone();
                    debug_assert_eq!(charged.rot.len(), rig.rot.len());
                    let mut tmp = Geometry::default();
                    let mut sink = Sink { geo: &mut tmp, tex: (64.0, 32.0), tint: [0.5; 3], alpha: 255, light: FULL_LIGHT, lights: WORLD_LIGHTS };
                    charged.emit(&mut sink, &m, &roots);
                    let scroll = (e.age as f32 + t) * 0.01;
                    for v in &mut tmp.vertices {
                        v.uv[0] += scroll;
                        v.uv[1] += scroll;
                        v.color = [128, 128, 128, 255];
                    }
                    let dst = cx.out.entry(Tex::Custom(armor | ADDITIVE)).or_default();
                    let base = dst.vertices.len() as u32;
                    dst.vertices.extend(tmp.vertices.iter().copied());
                    dst.indices.extend(tmp.indices.iter().map(|i| i + base));
                }
            }
            true
        }
        // Aranha e aranha da caverna (escala 0.7) com a camada de olhos acesos.
        52 | 59 => {
            let cave = id == 59;
            let Some(tex) = cx.textures.jar(cx.r, cx.jar, if cave { "entity/spider/cave_spider" } else { "entity/spider/spider" }) else { return true };
            let scale = if cave { 0.7 } else { 1.0 };
            // RenderSpider.getDeathMaxRotation = 180 (o padrão é 90).
            let m = living(pos, body, death * 2.0, Vec3::splat(scale));
            let mut rig = Rig::new(&g::SPIDER);
            pose_spider(&mut rig, anim);
            let roots = rig.roots();
            rig.emit(&mut cx.sink(tex, tint, 255, light), &m, &roots);
            if let Some(eyes) = cx.textures.jar(cx.r, cx.jar, "entity/spider_eyes") {
                // LayerSpiderEyes: mesmo modelo, brilho total (61680), `blendFunc(1, 1)` sem teste alfa.
                rig.emit(&mut cx.sink(eyes | ADDITIVE, [1.0; 3], 255, FULL_LIGHT), &m, &roots);
            }
            true
        }
        // Porco (sela pela metadata 16), vaca, ovelha (lã colorida ou tosada).
        90 | 92 | 91 | 96 => {
            let m = living(pos, body, death, Vec3::ONE);
            let (path, model, cy, cz): (&str, &'static crate::mob_rig::GenModel, f32, f32) = match id {
                90 => ("entity/pig/pig", &g::PIG, 4.0, 4.0),
                92 => ("entity/cow/cow", &g::COW, 8.0, 6.0),
                96 => ("entity/cow/mooshroom", &g::COW, 8.0, 6.0),
                _ => ("entity/sheep/sheep", &g::SHEEP_BODY, 8.0, 4.0),
            };
            let Some(tex) = cx.textures.jar(cx.r, cx.jar, path) else { return true };
            let mut rig = Rig::new(model);
            pose_quadruped(&mut rig, anim);
            if id == 91 {
                // ModelSheep2.setRotationAngles: a cabeça usa getHeadRotationAngleX (= pitch sem o comer grama).
                let h = rig.i("head");
                rig.rot[h].x = anim.head_pitch / DEG;
            }
            emit_quadruped(&rig, &mut cx.sink(tex, tint, 255, light), &m, child, cy, cz);
            if id == 96 && !child && !e.invisible() {
                // LayerMooshroomMushroom: dois cogumelos nas costas e um na cabeça (`renderBlockBrightness`).
                let red = 40u16 << 4;
                let base = m * Mat4::from_scale(Vec3::new(1.0, -1.0, 1.0)) * Mat4::from_translation(Vec3::new(0.2, 0.35, 0.5)) * Mat4::from_rotation_y(42f32.to_radians());
                let tr = |x: f32, y: f32, z: f32| Mat4::from_translation(Vec3::new(x, y, z));
                emit_block(cx, red, &(base * tr(-0.5, -0.5, 0.5)), light);
                emit_block(cx, red, &(base * tr(0.1, 0.0, -0.6) * Mat4::from_rotation_y(42f32.to_radians()) * tr(-0.5, -0.5, 0.5)), light);
                let head = rig.part(rig.i("head")).matrix(&m);
                emit_block(cx, red, &(head * Mat4::from_scale(Vec3::new(1.0, -1.0, 1.0)) * tr(0.0, 0.7, -0.2) * Mat4::from_rotation_y(12f32.to_radians()) * tr(-0.5, -0.5, 0.5)), light);
            }
            if id == 90 && e.meta_byte(16) == 1 {
                if let Some(saddle) = cx.textures.jar(cx.r, cx.jar, "entity/pig/pig_saddle") {
                    let mut sr = Rig::new(&g::PIG_SADDLE);
                    sr.rot = rig.rot.clone();
                    sr.pivot = rig.pivot.clone();
                    emit_quadruped(&sr, &mut cx.sink(saddle, tint, 255, light), &m, child, cy, cz);
                }
            }
            if id == 91 && e.meta_byte(16) & 0x10 == 0 {
                if let Some(fur) = cx.textures.jar(cx.r, cx.jar, "entity/sheep/sheep_fur") {
                    let c = g::SHEEP_COLORS[(e.meta_byte(16) & 15) as usize];
                    let mut wr = Rig::new(&g::SHEEP_WOOL);
                    wr.rot = rig.rot.clone();
                    wr.pivot = rig.pivot.clone();
                    emit_quadruped(&wr, &mut cx.sink(fur, [c[0] * tint[0], c[1] * tint[1], c[2] * tint[2]], 255, light), &m, child, cy, cz);
                }
            }
            true
        }
        // Galinha: asas com `handleRotationFloat`.
        93 => {
            let Some(tex) = cx.textures.jar(cx.r, cx.jar, "entity/chicken") else { return true };
            let m = living(pos, body, death, Vec3::ONE);
            let s = &e.mob_state;
            let wing = s.wing_rot_prev + (s.wing_rot - s.wing_rot_prev) * t;
            let dest = s.dest_pos_prev + (s.dest_pos - s.dest_pos_prev) * t;
            let flap = (sin(wing) + 1.0) * dest;
            let mut rig = Rig::new(&g::CHICKEN);
            let (h, bill, chin, b) = (rig.i("head"), rig.i("bill"), rig.i("chin"), rig.i("body"));
            rig.rot[h].x = anim.head_pitch / DEG;
            rig.rot[h].y = anim.head_yaw / DEG;
            for p in [bill, chin] {
                rig.rot[p].x = rig.rot[h].x;
                rig.rot[p].y = rig.rot[h].y;
            }
            rig.rot[b].x = 1.5707964;
            let (rl, ll, rw, lw) = (rig.i("rightLeg"), rig.i("leftLeg"), rig.i("rightWing"), rig.i("leftWing"));
            rig.rot[rl].x = cos(anim.limb_swing * 0.6662) * 1.4 * anim.limb_amount;
            rig.rot[ll].x = cos(anim.limb_swing * 0.6662 + PI) * 1.4 * anim.limb_amount;
            rig.rot[rw].z = flap;
            rig.rot[lw].z = -flap;
            let mut sink = cx.sink(tex, tint, 255, light);
            sink.tex = rig.tex();
            if child {
                rig.emit(&mut sink, &(m * Mat4::from_translation(Vec3::new(0.0, 5.0 / 16.0, 2.0 / 16.0))), &[h, bill, chin]);
                let m2 = m * Mat4::from_scale(Vec3::splat(0.5)) * Mat4::from_translation(Vec3::new(0.0, 24.0 / 16.0, 0.0));
                rig.emit(&mut sink, &m2, &[b, rl, ll, rw, lw]);
            } else {
                rig.emit(&mut sink, &m, &[h, bill, chin, b, rl, ll, rw, lw]);
            }
            true
        }
        95 => pets::wolf(e, pos, t, anim, body, death, cx),
        98 => pets::ocelot(e, pos, t, anim, body, death, cx),
        101 => pets::rabbit(e, pos, t, anim, body, death, cx),
        55 | 62 => hostile::slime(e, id, pos, t, anim, body, death, cx),
        100 => horse::horse(e, pos, t, anim, body, death, cx),
        63 => boss::dragon(e, pos, t, death, cx),
        68 => boss::guardian(e, pos, t, anim, body, death, cx),
        64 => boss::wither(e, pos, t, anim, body, death, cx),
        94 => misc::squid(e, pos, t, anim, body, death, cx),
        65 => misc::bat(e, pos, t, anim, body, death, cx),
        99 => misc::golem(e, pos, t, anim, body, death, cx),
        97 => misc::snowman(e, pos, t, anim, body, death, cx),
        66 => misc::witch(e, pos, t, anim, body, death, cx),
        56 => hostile::ghast(e, pos, t, anim, body, death, cx),
        61 => hostile::blaze(pos, anim, body, death, cx),
        58 => hostile::enderman(e, pos, t, anim, body, death, cx),
        60 | 67 => hostile::bugs(id, pos, anim, body, death, cx),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quads(r: &Rig) -> usize {
        r.roots().iter().map(|&i| r.part(i).quad_count()).sum()
    }

    #[test]
    fn creeper_geometry() {
        // 6 caixas (cabeça, corpo, 4 pernas) + a armadura que não é desenhada.
        let r = Rig::new(&g::CREEPER);
        assert_eq!(r.model.parts.len(), 7);
        assert_eq!(quads(&r), 7 * 6);
        let head = &g::CREEPER.parts[r.i("head")];
        assert_eq!(head.pivot, [0.0, 6.0, 0.0]);
        assert_eq!(head.cubes[0].size, [8, 8, 8]);
        assert_eq!(g::CREEPER_CHARGED.parts[0].cubes[0].inflate, 2.0);
        assert_eq!(g::CREEPER.parts[r.i("leg1")].pivot, [-2.0, 18.0, 4.0]);
    }

    #[test]
    fn quadruped_models() {
        // Porco: cabeça com focinho (2 caixas) + corpo + 4 pernas = 7 caixas.
        assert_eq!(quads(&Rig::new(&g::PIG)), 7 * 6);
        // Vaca: 4 pernas + cabeça + corpo + 2 chifres + úbere (addBox do Java: 6 no ModelCow + 6 no Quadruped).
        assert_eq!(quads(&Rig::new(&g::COW)), 9 * 6);
    }

    #[test]
    fn spider_has_eight_legs_and_pose_is_symmetric() {
        let mut r = Rig::new(&g::SPIDER);
        assert_eq!(quads(&r), 11 * 6);
        let a = Anim { limb_swing: 0.0, limb_amount: 0.0, ..Default::default() };
        pose_spider(&mut r, &a);
        let (l1, l2) = (r.rot[r.i("spiderLeg1")], r.rot[r.i("spiderLeg2")]);
        assert_eq!(l1.z, -l2.z);
        assert_eq!(l1.y, -l2.y);
        assert!((l1.z + 0.7853982).abs() < 1e-6);
    }

    #[test]
    fn quadruped_legs_swing_in_opposite_pairs() {
        let mut r = Rig::new(&g::PIG);
        pose_quadruped(&mut r, &Anim { limb_swing: 1.0, limb_amount: 1.0, ..Default::default() });
        let l: Vec<f32> = (1..=4).map(|i| r.rot[r.i(&format!("leg{i}"))].x).collect();
        assert!((l[0] - cos(0.6662) * 1.4).abs() < 1e-5);
        assert_eq!(l[1], l[2]);
        assert!((l[0] + l[1]).abs() < 1e-4);
        assert!((r.rot[r.i("body")].x - 1.5707964).abs() < 1e-7);
    }

    #[test]
    fn chicken_wings_and_creeper_fuse_tick() {
        let mut s = MobState::default();
        let meta = HashMap::new();
        let ti = |id: u8, ground: bool| TickIn { id, eid: 0, meta: &meta, on_ground: ground, in_water: false, body_yaw: 0.0, yaw: 0.0, y: 0.0, vel: DVec3::ZERO };
        for _ in 0..10 {
            s.tick(&ti(93, false));
        }
        assert!(s.dest_pos == 1.0 && s.wing_rot > 0.0 && s.wing_delta < 1.0);
        let mut c = MobState::default();
        let mut on = HashMap::new();
        on.insert(16, MetaValue::Byte(1));
        for _ in 0..40 {
            c.tick(&TickIn { id: 50, eid: 0, meta: &on, on_ground: true, in_water: false, body_yaw: 0.0, yaw: 0.0, y: 0.0, vel: DVec3::ZERO });
        }
        assert_eq!(c.swell, 30);
        let mut off = HashMap::new();
        off.insert(16, MetaValue::Byte(-1));
        c.tick(&TickIn { id: 50, eid: 0, meta: &off, on_ground: true, in_water: false, body_yaw: 0.0, yaw: 0.0, y: 0.0, vel: DVec3::ZERO });
        assert_eq!((c.swell_prev, c.swell), (30, 29));
    }

    #[test]
    fn sheep_colors_come_from_the_table() {
        assert_eq!(g::SHEEP_COLORS[0], [1.0, 1.0, 1.0]);
        assert_eq!(g::SHEEP_COLORS[15], [0.1, 0.1, 0.1]);
        assert_eq!(g::SHEEP_COLORS[1], [0.85, 0.5, 0.2]);
    }
}
