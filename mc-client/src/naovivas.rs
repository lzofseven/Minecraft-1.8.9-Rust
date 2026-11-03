//! Entidades não-vivas do cliente: item no chão (`RenderEntityItem`), orbe de XP (`RenderXPOrb`),
//! flecha (`RenderArrow`), projéteis (`RenderSnowball`, `RenderPotion`, `RenderFireball`).
//! Os números são os dos `Render*.java` da 1.8.9; os itens vêm dos modelos JSON do jar
//! (`first_person::ItemModels`).

use std::collections::HashMap;

use glam::{DVec3, Mat4, Vec3};
use mc_render::{Camera, Jar, Renderer, Tex};
use mc_world::blocks::BlockTable;
use mc_world::mesh::{atlas_uv, Geometry, Padded, Vertex};
use mc_world::World;

use crate::camera::{rot, tr};
use crate::entities::{Anchor, Entity, Kind};
use crate::entity_net::{MetaValue, Slot};
use crate::first_person::ItemModels;
use crate::models::{diffuse_with, Part, Sink, FULL_LIGHT, WORLD_LIGHTS};
use crate::mobs::TRANSLUCENT;
use crate::mth::sin;
use crate::skins::Textures;

/// Tipos do `S0EPacketSpawnObject` (`NetHandlerPlayClient.handleSpawnObject`).
pub const BOAT: u8 = 1;
pub const ITEM: u8 = 2;
pub const MINECART: u8 = 10;
pub const TNT: u8 = 50;
pub const CRYSTAL: u8 = 51;
pub const FALLING_BLOCK: u8 = 70;
pub const FISH_HOOK: u8 = 90;
pub const ARROW: u8 = 60;
pub const SNOWBALL: u8 = 61;
pub const EGG: u8 = 62;
pub const LARGE_FIREBALL: u8 = 63;
pub const SMALL_FIREBALL: u8 = 64;
pub const ENDER_PEARL: u8 = 65;
pub const ENDER_EYE: u8 = 72;
pub const POTION: u8 = 73;
pub const EXP_BOTTLE: u8 = 75;
pub const FIREWORK: u8 = 76;
/// Tipos sintéticos (não existem no protocolo): orbe de XP (S11), pintura (S10), raio (S2C).
pub const XP_ORB: u8 = 201;
pub const PAINTING: u8 = 200;
pub const LIGHTNING: u8 = 202;
pub const ITEM_FRAME: u8 = 71;
pub const WITHER_SKULL: u8 = 66;
/// `EntityLeashKnot` (objeto 77): o nó da coleira, preso a uma cerca.
pub const LEASH_KNOT: u8 = 77;

/// Estado por tick das entidades não-vivas.
#[derive(Clone, Debug, Default)]
pub struct ObjState {
    /// `EntityItem.hoverStart`.
    pub hover_start: f32,
    /// `EntityArrow.inGround` e `arrowShake`.
    pub in_ground: bool,
    pub shake: i32,
    /// Só no gancho de teste: idade do TNT congelada (a do jogo morre em 80 ticks pelo servidor).
    pub fixed_age: Option<i32>,
    /// Raio (`EntityLightningBolt`): `boltVertex`, `lightningState`, `boltLivingTime` e o `Random` da entidade.
    pub bolt_vertex: i64,
    pub bolt_state: i32,
    pub bolt_life: i32,
    pub rng: Option<i64>,
    pub expired: bool,
    /// Bloco pendurado (pintura e moldura) e direção horizontal (0 sul, 1 oeste, 2 norte, 3 leste).
    pub hanging: [i32; 3],
    pub facing: u8,
    /// Raio: os tons do trovão e da explosão a tocar neste tick (`lightningState == 2`) e se ainda está aceso
    /// (`lightningState >= 0`, que põe o `lastLightningBolt` do mundo em 2).
    pub thunder: Option<(f32, f32)>,
    pub flash: bool,
    /// `EntityFireball.accelerationX/Y/Z` (o S0E manda a direção; o cliente normaliza para 0.1).
    pub accel: DVec3,
    /// Flecha cravada: o bloco (`xTile/yTile/zTile`, `inTile`/`inData`) e há quantos ticks.
    pub tile: (i32, i32, i32),
    pub tile_state: u16,
    pub ticks_in_ground: i32,
    /// `EntityPickupFX`: coletado (S0D) por quem, e há quantos ticks (some no 3º). Fora do mundo: não anda.
    pub pickup: Option<(i32, i32)>,
}

/// `hoverStart = (float)(Math.random() * Math.PI * 2.0)`: sorteado por entidade.
pub fn hover_for(id: i32) -> f32 {
    let h = (id as u32).wrapping_mul(2654435761) ^ 0x9E3779B9;
    let h = h.wrapping_mul(2246822519) >> 8;
    h as f32 / (1u32 << 24) as f32 * std::f32::consts::TAU
}

/// Item usado por cada projétil de `RenderSnowball` (ids de item da 1.8.9).
pub fn projectile_item(kind: u8, data: i32) -> Option<(i16, i16)> {
    Some(match kind {
        SNOWBALL => (332, 0),
        EGG => (344, 0),
        ENDER_PEARL => (368, 0),
        ENDER_EYE => (381, 0),
        POTION => (373, data as i16),
        EXP_BOTTLE => (384, 0),
        FIREWORK => (401, 0),
        _ => return None,
    })
}

/// `RenderEntityItem.func_177078_a`: cópias desenhadas pela quantidade da pilha.
pub fn model_count(stack_size: i32) -> usize {
    if stack_size > 48 {
        5
    } else if stack_size > 32 {
        4
    } else if stack_size > 16 {
        3
    } else if stack_size > 1 {
        2
    } else {
        1
    }
}

/// Altura do balanço do item: `sin((age + parcial) / 10 + hoverStart) * 0.1 + 0.1`.
pub fn bob(age: f32, hover: f32) -> f32 {
    sin(age / 10.0 + hover) * 0.1 + 0.1
}

/// Giro do item em graus: `((age + parcial) / 20 + hoverStart) * 57.295776`.
pub fn spin_deg(age: f32, hover: f32) -> f32 {
    (age / 20.0 + hover) * 57.295776
}

/// `EntityXPOrb.getTextureByXP`.
pub fn xp_texture(value: i32) -> i32 {
    match value {
        2477.. => 10,
        1237.. => 9,
        617.. => 8,
        307.. => 7,
        149.. => 6,
        73.. => 5,
        37.. => 4,
        17.. => 3,
        7.. => 2,
        3.. => 1,
        _ => 0,
    }
}

/// `java.util.Random` mínimo (semente fixa 187 das cópias do item).
struct JRandom(i64);

impl JRandom {
    fn new(seed: i64) -> Self {
        Self((seed ^ 0x5DEECE66D) & ((1 << 48) - 1))
    }
    fn next(&mut self, bits: u32) -> i32 {
        self.0 = (self.0.wrapping_mul(0x5DEECE66D).wrapping_add(0xB)) & ((1 << 48) - 1);
        (self.0 >> (48 - bits)) as i32
    }
    fn next_int(&mut self, bound: i32) -> i32 {
        if bound & -bound == bound {
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
    fn next_long(&mut self) -> i64 {
        ((self.next(32) as i64) << 32).wrapping_add(self.next(32) as i64)
    }
    fn next_float(&mut self) -> f32 {
        self.next(24) as f32 / (1 << 24) as f32
    }
}

/// Quadro de desenho de uma entidade não-viva.
/// Estado do quadro compartilhado entre as entidades (câmera, mundo, geometria aditiva, hora).
pub struct Fx<'a> {
    pub cam: &'a Camera,
    pub world: &'a World,
    pub glow: &'a mut HashMap<Tex, Geometry>,
    pub time: i64,
    pub cache: &'a mut Cache,
}

/// Modelos assados que não mudam entre quadros.
#[derive(Default)]
pub struct Cache {
    lib: mc_world::model::ModelLib,
    /// Moldura (normal, com mapa): quads no espaço do bloco, UV já no atlas.
    frame: [Option<Vec<(Vec<Vec3>, Vec<[f32; 2]>, Vec3)>>; 2],
    /// Mapas do 0x34 por id (o dano do `filled_map`).
    pub maps: crate::maps::Maps,
}

pub struct Cx<'a> {
    pub cam: &'a Camera,
    pub world: &'a World,
    pub glow: &'a mut HashMap<Tex, Geometry>,
    pub time: i64,
    pub cache: &'a mut Cache,
    pub t: f32,
    pub light: [u8; 4],
    pub textures: &'a mut Textures,
    pub r: &'a mut Renderer,
    pub jar: &'a mut Jar,
    pub items: &'a mut ItemModels,
    pub table: &'a BlockTable,
    pub out: &'a mut HashMap<Tex, Geometry>,
}

/// Um quad com luz difusa pela normal (`RenderHelper` ligado, normal unitária pelo `RescaleNormal`).
#[allow(clippy::too_many_arguments)]
fn quad(geo: &mut Geometry, m: &Mat4, p: [Vec3; 4], uv: [[f32; 2]; 4], normal: Vec3, tint: [f32; 3], alpha: u8, light: [u8; 4]) {
    use mc_world::mesh::Vertex;
    let n = m.transform_vector3(normal).normalize_or_zero();
    let k = diffuse_with(n, &WORLD_LIGHTS);
    let color = [(tint[0] * k * 255.0) as u8, (tint[1] * k * 255.0) as u8, (tint[2] * k * 255.0) as u8, alpha];
    let v = |i: usize| Vertex { pos: m.transform_point3(p[i]).to_array(), uv: uv[i], color, light };
    geo.quad([v(0), v(1), v(2), v(3)]);
}

/// Item/bloco do `ItemStack` com a transformação do `RenderItem.renderItem(stack, model)`.
fn emit_stack(cx: &mut Cx, id: i16, damage: i16, m: Mat4, ground: bool, light: [u8; 4]) -> bool {
    let Some(model) = cx.items.get(id, damage, cx.jar, cx.table, cx.r) else { return false };
    if model.entity {
        return false;
    }
    let mut m = m;
    if ground {
        if model.gui3d {
            m *= Mat4::from_scale(Vec3::splat(0.5));
        }
        if let Some(d) = model.display_ground {
            m *= d.matrix();
        }
    }
    let m = m * Mat4::from_scale(Vec3::splat(0.5)) * tr(-0.5, -0.5, -0.5);
    model.emit_raw(m, &WORLD_LIGHTS, light, cx.out.entry(model.tex).or_default());
    true
}

/// Desenha uma entidade não-viva do grupo "itens e projéteis". `false` = não é daqui.
pub fn draw(e: &Entity, pos: Vec3, cx: &mut Cx) -> bool {
    let Kind::Object { kind, data } = e.kind else { return false };
    let age = e.age as f32 + cx.t;
    match kind {
        ITEM => {
            draw_item(e, pos, age, cx);
            true
        }
        XP_ORB => {
            draw_xp(e, pos, data, age, cx);
            true
        }
        ARROW => {
            draw_arrow(e, pos, cx);
            true
        }
        SNOWBALL | EGG | ENDER_PEARL | ENDER_EYE | POTION | EXP_BOTTLE | FIREWORK => {
            // RenderSnowball: escala 0.5 e giro para a câmera; o olho do Ender e o foguete: luz cheia/normal.
            let (id, dmg) = projectile_item(kind, data).unwrap();
            let light = if kind == ENDER_EYE { FULL_LIGHT } else { cx.light };
            let m = tr(pos.x, pos.y, pos.z) * Mat4::from_scale(Vec3::splat(0.5)) * rot(-cx.cam.yaw, 0.0, 1.0, 0.0) * rot(cx.cam.pitch, 1.0, 0.0, 0.0);
            // renderItemModelTransform: preTransform dobra o que não é 3D; depois o display `ground`.
            let flat = cx.items.get(id, dmg, cx.jar, cx.table, cx.r).is_some_and(|m| !m.gui3d);
            let m = if flat { m * Mat4::from_scale(Vec3::splat(2.0)) } else { m };
            emit_stack(cx, id, dmg, m, true, light);
            true
        }
        TNT | FALLING_BLOCK => {
            let white = cx.textures.white(cx.r);
            let pure = Pure { t: cx.t, light: cx.light, time: cx.time, white, world: cx.world, table: cx.table };
            draw_pure(e, pos, &pure, cx.out, cx.glow)
        }
        BOAT => {
            draw_boat(e, pos, cx);
            true
        }
        MINECART => {
            draw_minecart(e, pos, data, cx);
            true
        }
        CRYSTAL => {
            draw_crystal(e, pos, cx);
            true
        }
        FISH_HOOK => {
            draw_bobber(pos, cx);
            true
        }
        PAINTING => {
            draw_painting(e, pos, data, cx);
            true
        }
        ITEM_FRAME => {
            draw_frame(e, cx);
            true
        }
        WITHER_SKULL => {
            draw_wither_skull(e, pos, cx);
            true
        }
        LIGHTNING => {
            draw_lightning(e, pos, cx);
            true
        }
        LARGE_FIREBALL | SMALL_FIREBALL => {
            let s = if kind == LARGE_FIREBALL { 2.0 } else { 0.5 };
            draw_fireball(pos, s, cx);
            true
        }
        LEASH_KNOT => {
            draw_knot(pos, cx);
            true
        }
        _ => false,
    }
}

fn draw_item(e: &Entity, pos: Vec3, age: f32, cx: &mut Cx) {
    let Some(MetaValue::Slot(Some(s))) = e.meta.get(&10) else { return };
    let Slot { id, damage, count, .. } = *s;
    // Baú, cabeça e estandarte (`builtin/entity`): o tile entity do item, com o NBT (dono, padrões).
    let builtin = crate::entities::Worn::from_slot(s).entity.and_then(crate::tile_entities::entity_from_variant);
    let Some(model) = cx.items.get(id, damage, cx.jar, cx.table, cx.r) else { return };
    let n = model_count(count as i32);
    let hover = e.obj.hover_start;
    let f2 = model.display_ground.map_or(1.0, |d| d.scale.y);
    let mut m = tr(pos.x, pos.y + bob(age, hover) + 0.25 * f2, pos.z) * rot(spin_deg(age, hover), 0.0, 1.0, 0.0);
    if !model.gui3d {
        m *= tr(-0.0, -0.0, -0.046875 * (n as f32 - 1.0) * 0.5);
    }
    let mut rng = JRandom::new(187);
    for j in 0..n {
        let mut mj = m;
        if j > 0 {
            let f = (rng.next_float() * 2.0 - 1.0) * 0.15;
            let f1 = (rng.next_float() * 2.0 - 1.0) * 0.15;
            let f2 = (rng.next_float() * 2.0 - 1.0) * 0.15;
            mj *= tr(f, f1, f2);
        }
        match &builtin {
            Some(item) => emit_builtin(cx, id, damage, item, mj, cx.light),
            None => {
                emit_stack(cx, id, damage, mj, true, cx.light);
            }
        }
    }
}

/// `RenderItem.renderItem` de um item `builtin/entity` no chão: `display.ground`, o 0,5, o giro de 180° e o
/// `TileEntityItemStackRenderer.renderByItem`.
fn emit_builtin(cx: &mut Cx, id: i16, damage: i16, item: &crate::tile_entities::EntityItem, m: Mat4, light: [u8; 4]) {
    let Some(model) = cx.items.get(id, damage, cx.jar, cx.table, cx.r) else { return };
    let Some(tex) = crate::tile_entities::item_texture(item, cx.textures, cx.r, cx.jar) else { return };
    let mut m = m;
    if model.gui3d {
        m *= Mat4::from_scale(Vec3::splat(0.5));
    }
    if let Some(d) = model.display_ground {
        m *= d.matrix();
    }
    let m = crate::first_person::builtin_matrix(m * Mat4::from_scale(Vec3::splat(0.5)) * tr(-0.5, -0.5, -0.5));
    let mut sink = crate::models::Sink { geo: cx.out.entry(Tex::Custom(tex)).or_default(), tex: (64.0, 64.0), tint: [1.0; 3], alpha: 255, light, lights: WORLD_LIGHTS };
    crate::tile_entities::emit_item(item, &mut sink, &m);
}

fn draw_xp(e: &Entity, pos: Vec3, value: i32, age: f32, cx: &mut Cx) {
    let Some(tex) = cx.textures.jar(cx.r, cx.jar, "entity/experience_orb") else { return };
    let i = xp_texture(value);
    let (u0, u1) = ((i % 4 * 16) as f32 / 64.0, (i % 4 * 16 + 16) as f32 / 64.0);
    let (v0, v1) = ((i / 4 * 16) as f32 / 64.0, (i / 4 * 16 + 16) as f32 / 64.0);
    // getBrightnessForRender: +7,5 níveis de luz de bloco (j + 120, no máximo 240).
    let mut light = cx.light;
    light[1] = (light[1] as i32 + 127).min(255) as u8;
    let f = (e.age as f32 + cx.t) / 2.0;
    let _ = age;
    let r = ((sin(f) + 1.0) * 0.5 * 255.0) as i32;
    let b = ((sin(f + 4.1887903) + 1.0) * 0.1 * 255.0) as i32;
    let tint = [r as f32 / 255.0, 1.0, b as f32 / 255.0];
    let m = tr(pos.x, pos.y, pos.z) * rot(180.0 - cx.cam.yaw, 0.0, 1.0, 0.0) * rot(-cx.cam.pitch, 1.0, 0.0, 0.0) * Mat4::from_scale(Vec3::splat(0.3));
    let p = [Vec3::new(-0.5, -0.25, 0.0), Vec3::new(0.5, -0.25, 0.0), Vec3::new(0.5, 0.75, 0.0), Vec3::new(-0.5, 0.75, 0.0)];
    let uv = [[u0, v1], [u1, v1], [u1, v0], [u0, v0]];
    let geo = cx.out.entry(Tex::Custom(tex | TRANSLUCENT)).or_default();
    quad(geo, &m, p, uv, Vec3::Y, tint, 128, light);
}

fn draw_fireball(pos: Vec3, scale: f32, cx: &mut Cx) {
    // O sprite é o `particle` de Items.fire_charge (items/fireball): a camada 0 do modelo do item, inteira.
    let Some(model) = cx.items.get(385, 0, cx.jar, cx.table, cx.r) else { return };
    let m = tr(pos.x, pos.y, pos.z) * Mat4::from_scale(Vec3::splat(scale)) * rot(180.0 - cx.cam.yaw, 0.0, 1.0, 0.0) * rot(-cx.cam.pitch, 1.0, 0.0, 0.0);
    let p = [Vec3::new(-0.5, -0.25, 0.0), Vec3::new(0.5, -0.25, 0.0), Vec3::new(0.5, 0.75, 0.0), Vec3::new(-0.5, 0.75, 0.0)];
    let uv = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
    // EntityFireball.getBrightnessForRender = 15728880 (luz cheia).
    quad(cx.out.entry(model.tex).or_default(), &m, p, uv, Vec3::Y, [1.0; 3], 255, FULL_LIGHT);
}

fn draw_arrow(e: &Entity, pos: Vec3, cx: &mut Cx) {
    let Some(tex) = cx.textures.jar(cx.r, cx.jar, "entity/arrow") else { return };
    let t = cx.t;
    let yaw = e.prev_yaw + (e.yaw - e.prev_yaw) * t;
    let pitch = e.prev_pitch + (e.pitch - e.prev_pitch) * t;
    let mut m = tr(pos.x, pos.y, pos.z) * rot(yaw - 90.0, 0.0, 1.0, 0.0) * rot(pitch, 0.0, 0.0, 1.0);
    let shake = e.obj.shake as f32 - t;
    if shake > 0.0 {
        m *= rot(-sin(shake * 3.0) * shake, 0.0, 0.0, 1.0);
    }
    m = m * rot(45.0, 1.0, 0.0, 0.0) * Mat4::from_scale(Vec3::splat(0.05625)) * tr(-4.0, 0.0, 0.0);
    let (u0, u1, v0, v1) = (0.0, 0.5, 0.0, 5.0 / 32.0);
    let (tu0, tu1, tv0, tv1) = (0.0, 0.15625, 5.0 / 32.0, 10.0 / 32.0);
    let geo = cx.out.entry(Tex::Custom(tex)).or_default();
    let v = Vec3::new;
    // Placa traseira, duas faces (normais +X e -X).
    let a = [v(-7.0, -2.0, -2.0), v(-7.0, -2.0, 2.0), v(-7.0, 2.0, 2.0), v(-7.0, 2.0, -2.0)];
    quad(geo, &m, a, [[tu0, tv0], [tu1, tv0], [tu1, tv1], [tu0, tv1]], Vec3::X, [1.0; 3], 255, cx.light);
    let b = [v(-7.0, 2.0, -2.0), v(-7.0, 2.0, 2.0), v(-7.0, -2.0, 2.0), v(-7.0, -2.0, -2.0)];
    quad(geo, &m, b, [[tu0, tv0], [tu1, tv0], [tu1, tv1], [tu0, tv1]], -Vec3::X, [1.0; 3], 255, cx.light);
    // Quatro aletas giradas de 90° em X.
    for _ in 0..4 {
        m *= rot(90.0, 1.0, 0.0, 0.0);
        let c = [v(-8.0, -2.0, 0.0), v(8.0, -2.0, 0.0), v(8.0, 2.0, 0.0), v(-8.0, 2.0, 0.0)];
        quad(geo, &m, c, [[u0, v0], [u1, v0], [u1, v1], [u0, v1]], Vec3::Z, [1.0; 3], 255, cx.light);
    }
}

const FACE_N: [Vec3; 6] = [Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 0.0, 1.0), Vec3::new(-1.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)];

/// Modelo do bloco (primeira variante, sem bits dos vizinhos).
fn block_model(table: &BlockTable, state: u16) -> Option<&mc_world::model::BakedModel> {
    let m = table.models.as_ref()?;
    if state == 0 || !m.has(state) {
        return None;
    }
    Some(m.get(state, 0)?.pick(0, 0, 0))
}

/// `BlockModelRenderer.renderModelBrightness`: cor = brilho x tinta, luz difusa do GL. Com `white`, o brilho
/// aditivo do TNT: textura desligada (pixel branco), sem difusa, `alpha` pelo canal.
#[allow(clippy::too_many_arguments)]
fn emit_block(geo: &mut Geometry, table: &BlockTable, state: u16, m: &Mat4, brightness: f32, white: Option<u8>, light: [u8; 4]) -> bool {
    let Some(model) = block_model(table, state) else { return false };
    for q in &model.quads {
        let n = m.transform_vector3(FACE_N[q.face as usize % 6]).normalize_or_zero();
        let (k, alpha) = match white {
            Some(a) => (1.0, a),
            None => (diffuse_with(n, &WORLD_LIGHTS), 255),
        };
        let tint = if q.tint >= 0 && white.is_none() { table.tint_color(state) } else { 0xFFFFFF };
        let ch = |sh: u32| (((tint >> sh) & 0xFF) as f32 * brightness * k).clamp(0.0, 255.0) as u8;
        let color = [ch(16), ch(8), ch(0), alpha];
        let v = |i: usize| Vertex {
            pos: m.transform_point3(Vec3::from(q.pos[i])).to_array(),
            uv: if white.is_some() { [0.5, 0.5] } else { atlas_uv(q.tex, q.uv[i][0], q.uv[i][1]) },
            color,
            light,
        };
        geo.quad([v(0), v(1), v(2), v(3)]);
    }
    true
}

/// `Entity.getBrightness`: brilho 0..1 do ponto da luz da entidade, com o escurecimento do dia.
fn brightness(light: [u8; 4], time: i64) -> f32 {
    crate::hud_stats::brightness_at((light[1] / 17) as i32, (light[0] / 17) as i32, time)
}

/// `RenderTNTPrimed`: `fuse` cai 1 por tick desde 80; pisca branco aditivo a cada 5 ticks.
pub fn tnt_scale(fuse: i32, t: f32) -> f32 {
    if fuse as f32 - t + 1.0 < 10.0 {
        let f = (1.0 - (fuse as f32 - t + 1.0) / 10.0).clamp(0.0, 1.0);
        let f = f * f;
        1.0 + f * f * 0.3
    } else {
        1.0
    }
}

/// O que o desenho das entidades "puras" (TNT, bloco caindo) usa: nada que mude, então podem ser desenhadas em
/// paralelo (`Entities::draw` com muitas delas).
pub struct Pure<'a> {
    pub t: f32,
    pub light: [u8; 4],
    pub time: i64,
    pub white: u32,
    pub world: &'a World,
    pub table: &'a BlockTable,
}

/// Entidades que só leem o mundo e a tabela de blocos.
pub fn is_pure(kind: u8) -> bool {
    matches!(kind, TNT | FALLING_BLOCK)
}

/// `RenderTNTPrimed` e `RenderFallingBlock`.
pub fn draw_pure(e: &Entity, pos: Vec3, cx: &Pure, out: &mut HashMap<Tex, Geometry>, glow: &mut HashMap<Tex, Geometry>) -> bool {
    match e.kind {
        Kind::Object { kind: TNT, .. } => draw_tnt(e, pos, cx, out, glow),
        Kind::Object { kind: FALLING_BLOCK, data } => draw_falling(e, pos, data, cx, out),
        _ => return false,
    }
    true
}

fn draw_tnt(e: &Entity, pos: Vec3, cx: &Pure, out: &mut HashMap<Tex, Geometry>, glow: &mut HashMap<Tex, Geometry>) {
    let fuse = 80 - e.obj.fixed_age.unwrap_or(e.age as i32);
    let t = cx.t;
    let mut m = tr(pos.x, pos.y + 0.5, pos.z);
    let s = tnt_scale(fuse, t);
    m *= Mat4::from_scale(Vec3::splat(s));
    let base = m * tr(-0.5, -0.5, 0.5) * rot(90.0, 0.0, 1.0, 0.0);
    let state = 46 << 4;
    emit_block(out.entry(Tex::Blocks).or_default(), cx.table, state, &base, brightness(cx.light, cx.time), None, cx.light);
    if fuse / 5 % 2 == 0 {
        let a = (1.0 - (fuse as f32 - t + 1.0) / 100.0) * 0.8;
        let white = cx.white;
        let flash = m * tr(-0.5, -0.5, 0.5) * rot(90.0, 0.0, 1.0, 0.0) * tr(0.0, 0.0, 1.0) * rot(90.0, 0.0, 1.0, 0.0);
        emit_block(glow.entry(Tex::Custom(white)).or_default(), cx.table, state, &flash, 1.0, Some((a.clamp(0.0, 1.0) * 255.0) as u8), FULL_LIGHT);
    }
}

/// Estado (id << 4 | meta) do dado do S0E do bloco que cai: `Block.getStateById(data & 0xffff)`.
pub fn falling_state(data: i32) -> u16 {
    (((data & 0xFFF) << 4) | ((data >> 12) & 0xF)) as u16
}

/// Luz empacotada (céu << 4 | bloco) de uma célula do mundo.
fn cell_light(world: &World, x: i32, y: i32, z: i32) -> u8 {
    let open = if world.has_sky { 0xF0 } else { 0 };
    if !(0..256).contains(&y) {
        return open;
    }
    let Some(col) = world.columns.get(&(x >> 4, z >> 4)) else { return open };
    match &col.sections[(y >> 4) as usize] {
        Some(s) => s.light(mc_world::chunk::Section::index(x & 15, y & 15, z & 15)),
        None => open,
    }
}

/// `RenderFallingBlock`: o modelo do bloco com a luz suave e a oclusão do mundo em volta do `BlockPos` da
/// entidade (o mesmo gerador de malha do terreno, numa cópia pequena), centrado na posição da entidade.
fn draw_falling(e: &Entity, pos: Vec3, data: i32, cx: &Pure, out: &mut HashMap<Tex, Geometry>) {
    let state = falling_state(data);
    if !cx.table.models.as_ref().is_some_and(|m| m.has(state)) {
        return;
    }
    let bp = [e.pos.x.floor() as i32, e.pos.y.floor() as i32, e.pos.z.floor() as i32];
    if cx.world.block(bp[0], bp[1], bp[2]) == state {
        return;
    }
    // Só o bloco da entidade é montado; os 26 vizinhos dão a oclusão, a luz suave e o corte de faces (antes a
    // cópia 7x7x7 inteira virava malha a cada quadro: o chão em volta saía junto, deslocado, e 1000 blocos caindo
    // custavam ~200 ms por quadro). O `Padded` é reaproveitado por thread: só as 27 células lidas são regravadas.
    thread_local! {
        static PADDED: std::cell::RefCell<Option<Padded>> = const { std::cell::RefCell::new(None) };
    }
    let mesh = PADDED.with(|cell| {
        let mut cell = cell.borrow_mut();
        let p = cell.get_or_insert_with(|| Padded::new(0));
        for dy in -1..=1 {
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let i = Padded::idx(8 + dx, 8 + dy, 8 + dz);
                    p.blocks[i] = cx.world.block(bp[0] + dx, bp[1] + dy, bp[2] + dz);
                    p.light[i] = cell_light(cx.world, bp[0] + dx, bp[1] + dy, bp[2] + dz);
                }
            }
        }
        p.blocks[Padded::idx(8, 8, 8)] = state;
        mc_world::mesh::mesh_one(p, [bp[0] - 8, bp[1] - 8, bp[2] - 8], cx.table, [8, 8, 8])
    });
    // setTranslation(-x - 0.5, -y, -z - 0.5) e depois o translate da posição.
    let d = pos - Vec3::new(bp[0] as f32 + 0.5, bp[1] as f32, bp[2] as f32 + 0.5);
    let geo = out.entry(Tex::Blocks).or_default();
    let base = geo.vertices.len() as u32;
    geo.vertices.extend(mesh.opaque.vertices.iter().map(|v| Vertex { pos: [v.pos[0] + d.x, v.pos[1] + d.y, v.pos[2] + d.z], ..*v }));
    geo.indices.extend(mesh.opaque.indices.iter().map(|i| i + base));
}

/// `RenderBoat` e a oscilação por golpe (`timeSinceHit`, `damageTaken`, `forwardDirection`).
fn draw_boat(e: &Entity, pos: Vec3, cx: &mut Cx) {
    let Some(tex) = cx.textures.jar(cx.r, cx.jar, "entity/boat") else { return };
    let yaw = crate::entities::lerp_rot(e.prev_yaw, e.yaw, cx.t);
    let mut m = tr(pos.x, pos.y + 0.25, pos.z) * rot(180.0 - yaw, 0.0, 1.0, 0.0);
    let hit = e.meta_int(17) as f32 - cx.t;
    let mut dmg = match e.meta.get(&19) {
        Some(MetaValue::Float(f)) => *f,
        _ => 0.0,
    } - cx.t;
    if dmg < 0.0 {
        dmg = 0.0;
    }
    if hit > 0.0 {
        m *= rot(sin(hit) * hit * dmg / 10.0 * e.meta_int(18) as f32, 1.0, 0.0, 0.0);
    }
    m *= Mat4::from_scale(Vec3::new(-1.0, -1.0, 1.0));
    let rig = crate::mob_rig::Rig::new(&crate::gen_mob_models::BOAT);
    let mut sink = crate::models::Sink { geo: cx.out.entry(Tex::Custom(tex)).or_default(), tex: (64.0, 32.0), tint: [1.0; 3], alpha: 255, light: cx.light, lights: WORLD_LIGHTS };
    rig.emit(&mut sink, &m, &rig.roots());
}

/// Bloco mostrado no carrinho: (estado, deslocamento em pixels). `getDisplayTile` / `getDisplayTileOffset`.
pub fn minecart_tile(e: &Entity, kind_data: i32) -> (u16, i32) {
    let has = e.meta_byte(22) == 1;
    let (default, off) = match kind_data {
        1 => (0, 8),           // baú: renderType 2 (sem modelo JSON)
        2 => (61 << 4 | 2, 6), // fornalha, FACING = norte
        3 => (46 << 4, 6),
        4 => (52 << 4, 6),
        5 => (154 << 4, 1),
        6 => (137 << 4, 6),
        _ => (0, 6),
    };
    if has {
        let v = e.meta_int(20);
        (falling_state(v), e.meta_int(21))
    } else {
        (default, off)
    }
}

/// `RenderMinecart` (sem seguir os trilhos: usa a posição e a rotação do servidor).
/// `EntityMinecart.matrix`: as duas pontas de cada forma de trilho (`EnumRailDirection.getMetadata`).
const RAIL_MATRIX: [[[i32; 3]; 2]; 10] = [
    [[0, 0, -1], [0, 0, 1]],
    [[-1, 0, 0], [1, 0, 0]],
    [[-1, -1, 0], [1, 0, 0]],
    [[-1, 0, 0], [1, -1, 0]],
    [[0, 0, -1], [0, -1, 1]],
    [[0, -1, -1], [0, 0, 1]],
    [[0, 0, 1], [1, 0, 0]],
    [[0, 0, 1], [-1, 0, 0]],
    [[0, 0, -1], [-1, 0, 0]],
    [[0, 0, -1], [1, 0, 0]],
];

/// `BlockRailBase.isRailBlock` e a forma (`getRailDirection`): o trilho comum usa as 10, os outros os 3 bits baixos.
fn rail_shape(world: &World, x: i32, y: i32, z: i32) -> Option<usize> {
    let st = world.block(x, y, z);
    match (st >> 4, (st & 15) as usize) {
        (66, m) if m < 10 => Some(m),
        (27 | 28 | 157, m) if m & 7 < 6 => Some(m & 7),
        _ => None,
    }
}

/// `EntityMinecart.func_70489_a`: o ponto do trilho debaixo de (x, y, z).
fn rail_pos(world: &World, mut x: f64, mut y: f64, mut z: f64) -> Option<glam::DVec3> {
    let (i, mut j, k) = (x.floor() as i32, y.floor() as i32, z.floor() as i32);
    if rail_shape(world, i, j - 1, k).is_some() {
        j -= 1;
    }
    let a = RAIL_MATRIX[rail_shape(world, i, j, k)?];
    let (d1, d2, d3) = (i as f64 + 0.5 + a[0][0] as f64 * 0.5, j as f64 + 0.0625 + a[0][1] as f64 * 0.5, k as f64 + 0.5 + a[0][2] as f64 * 0.5);
    let (d4, d5, d6) = (i as f64 + 0.5 + a[1][0] as f64 * 0.5, j as f64 + 0.0625 + a[1][1] as f64 * 0.5, k as f64 + 0.5 + a[1][2] as f64 * 0.5);
    let (d7, d8, d9) = (d4 - d1, (d5 - d2) * 2.0, d6 - d3);
    let d0 = if d7 == 0.0 {
        x = i as f64 + 0.5;
        z - k as f64
    } else if d9 == 0.0 {
        z = k as f64 + 0.5;
        x - i as f64
    } else {
        ((x - d1) * d7 + (z - d3) * d9) * 2.0
    };
    let _ = (x, z);
    x = d1 + d7 * d0;
    y = d2 + d8 * d0;
    z = d3 + d9 * d0;
    if d8 < 0.0 {
        y += 1.0;
    }
    if d8 > 0.0 {
        y += 0.5;
    }
    Some(glam::DVec3::new(x, y, z))
}

/// `EntityMinecart.func_70495_a`: o ponto do trilho `off` blocos à frente ao longo dele.
fn rail_offset(world: &World, mut x: f64, mut y: f64, mut z: f64, off: f64) -> Option<glam::DVec3> {
    let (i, mut j, k) = (x.floor() as i32, y.floor() as i32, z.floor() as i32);
    if rail_shape(world, i, j - 1, k).is_some() {
        j -= 1;
    }
    let shape = rail_shape(world, i, j, k)?;
    y = j as f64;
    if (2..=5).contains(&shape) {
        y = (j + 1) as f64;
    }
    let a = RAIL_MATRIX[shape];
    let (mut d0, mut d1) = ((a[1][0] - a[0][0]) as f64, (a[1][2] - a[0][2]) as f64);
    let d2 = (d0 * d0 + d1 * d1).sqrt();
    d0 /= d2;
    d1 /= d2;
    x += d0 * off;
    z += d1 * off;
    if a[0][1] != 0 && x.floor() as i32 - i == a[0][0] && z.floor() as i32 - k == a[0][2] {
        y += a[0][1] as f64;
    } else if a[1][1] != 0 && x.floor() as i32 - i == a[1][0] && z.floor() as i32 - k == a[1][2] {
        y += a[1][1] as f64;
    }
    rail_pos(world, x, y, z)
}

fn draw_minecart(e: &Entity, pos: Vec3, kind_data: i32, cx: &mut Cx) {
    let Some(tex) = cx.textures.jar(cx.r, cx.jar, "entity/minecart") else { return };
    let t = cx.t;
    let mut l = (e.id as i64).wrapping_mul(493286711);
    l = l.wrapping_mul(l).wrapping_mul(4392167121).wrapping_add(l.wrapping_mul(98761));
    let j = |sh: u32| (((l >> sh & 7) as f32 + 0.5) / 8.0 - 0.5) * 0.004;
    let mut yaw = crate::entities::lerp_rot(e.prev_yaw, e.yaw, t);
    let mut pitch = e.prev_pitch + (e.pitch - e.prev_pitch) * t;
    // RenderMinecart: em cima de trilho, posição, giro e inclinação saem do próprio trilho.
    let p = e.prev.lerp(e.pos, t as f64);
    let mut pos = pos;
    if let Some(on) = rail_pos(cx.world, p.x, p.y, p.z) {
        let a = rail_offset(cx.world, p.x, p.y, p.z, 0.30000001192092896).unwrap_or(on);
        let b = rail_offset(cx.world, p.x, p.y, p.z, -0.30000001192092896).unwrap_or(on);
        pos = Vec3::new(on.x as f32, ((a.y + b.y) / 2.0) as f32, on.z as f32);
        let d = b - a;
        if d.length() != 0.0 {
            let d = d.normalize();
            yaw = (d.z.atan2(d.x) * 180.0 / std::f64::consts::PI) as f32;
            pitch = (d.y.atan() * 73.0) as f32;
        }
    }
    let mut m = tr(j(16), j(20), j(24)) * tr(pos.x, pos.y + 0.375, pos.z) * rot(180.0 - yaw, 0.0, 1.0, 0.0) * rot(-pitch, 0.0, 0.0, 1.0);
    let roll = e.meta_int(17) as f32 - t;
    let mut dmg = match e.meta.get(&19) {
        Some(MetaValue::Float(f)) => *f,
        _ => 0.0,
    } - t;
    if dmg < 0.0 {
        dmg = 0.0;
    }
    if roll > 0.0 {
        m *= rot(sin(roll) * roll * dmg / 10.0 * e.meta_int(18) as f32, 1.0, 0.0, 0.0);
    }
    let (state, off) = minecart_tile(e, kind_data);
    if state != 0 {
        let bm = m * Mat4::from_scale(Vec3::splat(0.75)) * tr(-0.5, (off - 8) as f32 / 16.0, 0.5) * rot(90.0, 0.0, 1.0, 0.0);
        let b = brightness(cx.light, cx.time);
        emit_block(cx.out.entry(Tex::Blocks).or_default(), cx.table, state, &bm, b, None, cx.light);
    }
    m *= Mat4::from_scale(Vec3::new(-1.0, -1.0, 1.0));
    let mut rig = crate::mob_rig::Rig::new(&crate::gen_mob_models::MINECART);
    let i = rig.i("sideModels_5");
    rig.pivot[i].y = 4.0 - -0.1;
    let mut sink = crate::models::Sink { geo: cx.out.entry(Tex::Custom(tex)).or_default(), tex: (64.0, 32.0), tint: [1.0; 3], alpha: 255, light: cx.light, lights: WORLD_LIGHTS };
    rig.emit(&mut sink, &m, &rig.roots());
}

/// `RenderEnderCrystal`: base, e o vidro girando em três camadas (`innerRotation` sorteado + 1 por tick).
fn draw_crystal(e: &Entity, pos: Vec3, cx: &mut Cx) {
    let Some(tex) = cx.textures.jar(cx.r, cx.jar, "entity/endercrystal/endercrystal") else { return };
    let rotation = (e.obj.hover_start * 10000.0) + e.age as f32 + cx.t;
    let f = sin(rotation * 0.2) / 2.0 + 0.5;
    let bobbing = (f * f + f) * 0.2;
    let spin = rotation * 3.0;
    let axis = (0.7071, 0.0, 0.7071);
    let rig = crate::mob_rig::Rig::new(&crate::gen_mob_models::ENDER_CRYSTAL);
    let (glass, cube, base) = (rig.i("glass"), rig.i("cube"), rig.i("base"));
    let mut sink = crate::models::Sink { geo: cx.out.entry(Tex::Custom(tex)).or_default(), tex: (64.0, 32.0), tint: [1.0; 3], alpha: 255, light: cx.light, lights: WORLD_LIGHTS };
    let mut m = tr(pos.x, pos.y, pos.z) * Mat4::from_scale(Vec3::splat(2.0)) * tr(0.0, -0.5, 0.0);
    rig.emit(&mut sink, &m, &[base]);
    m = m * rot(spin, 0.0, 1.0, 0.0) * tr(0.0, 0.8 + bobbing, 0.0) * rot(60.0, axis.0, axis.1, axis.2);
    rig.emit(&mut sink, &m, &[glass]);
    m = m * Mat4::from_scale(Vec3::splat(0.875)) * rot(60.0, axis.0, axis.1, axis.2) * rot(spin, 0.0, 1.0, 0.0);
    rig.emit(&mut sink, &m, &[glass]);
    m = m * Mat4::from_scale(Vec3::splat(0.875)) * rot(60.0, axis.0, axis.1, axis.2) * rot(spin, 0.0, 1.0, 0.0);
    rig.emit(&mut sink, &m, &[cube]);
}

/// Boia do anzol (`RenderFish`): sprite de `particles.png` virado para a câmera (a linha fica pendente).
fn draw_bobber(pos: Vec3, cx: &mut Cx) {
    let Some(tex) = cx.textures.jar(cx.r, cx.jar, "particle/particles") else { return };
    let m = tr(pos.x, pos.y, pos.z) * Mat4::from_scale(Vec3::splat(0.5)) * rot(180.0 - cx.cam.yaw, 0.0, 1.0, 0.0) * rot(-cx.cam.pitch, 1.0, 0.0, 0.0);
    let p = [Vec3::new(-0.5, -0.5, 0.0), Vec3::new(0.5, -0.5, 0.0), Vec3::new(0.5, 0.5, 0.0), Vec3::new(-0.5, 0.5, 0.0)];
    let uv = [[0.0625, 0.1875], [0.125, 0.1875], [0.125, 0.125], [0.0625, 0.125]];
    quad(cx.out.entry(Tex::Custom(tex)).or_default(), &m, p, uv, Vec3::Y, [1.0; 3], 255, cx.light);
}

/// `RenderLeashKnot.doRender`: `ModelLeashKnot`, a caixa 6x8x6 em (0,0) de uma textura 32x32, com
/// `scale(-1,-1,1)` e `0.0625` (o `Sink` divide os pixels por 16), e a luz da entidade.
fn draw_knot(pos: Vec3, cx: &mut Cx) {
    let Some(tex) = cx.textures.jar(cx.r, cx.jar, "entity/lead_knot") else { return };
    let part = Part::new([0.0; 3]).cube((0, 0), [-3.0, -6.0, -3.0], [6, 8, 6], 0.0, false);
    let m = tr(pos.x, pos.y, pos.z) * Mat4::from_scale(Vec3::new(-1.0, -1.0, 1.0));
    let mut sink = Sink { geo: cx.out.entry(Tex::Custom(tex)).or_default(), tex: (32.0, 32.0), tint: [1.0; 3], alpha: 255, light: cx.light, lights: WORLD_LIGHTS };
    sink.part(&part, &m);
}

/// `RenderLiving.renderLeash`: os vértices da corda como duas fitas de 25 pares (`begin(5, POSITION_COLOR)`,
/// GL_TRIANGLE_STRIP). Cada par é (A, B) de um dos 25 pedaços; a cor alterna entre os pares pares (escurecidos
/// por 0,7). Os cálculos seguem o Java: `interpolateValue` em double, floats onde o Java usa float.
pub fn leash_strips(mob: &Entity, holder: &Anchor, partial: f32) -> [Vec<(DVec3, [u8; 4])>; 2] {
    const DEG: f64 = 0.01745329238474369;
    let p9 = partial as f64;
    let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
    // Os ângulos do segurador usam metade do parcial (`partial * 0.5F`).
    let half = (partial * 0.5) as f64;
    let yaw = lerp(holder.prev_yaw as f64, holder.yaw as f64, half) * DEG;
    let pitch = lerp(holder.prev_pitch as f64, holder.pitch as f64, half) * DEG;
    let (mut cx, mut cz, mut sy_h) = (yaw.cos(), yaw.sin(), pitch.sin());
    // EntityHanging (o nó pendurado): a corda sai de baixo, sem o ângulo do segurador.
    if holder.hanging {
        (cx, cz, sy_h) = (0.0, 0.0, -1.0);
    }
    let cp = pitch.cos();
    let eye = holder.eye * 0.7;
    // Ponto de fixação no segurador (lvt_25, lvt_27, lvt_29).
    let hx = lerp(holder.prev.x, holder.pos.x, p9) - cx * 0.7 - cz * 0.5 * cp;
    let hy = lerp(holder.prev.y + eye, holder.pos.y + eye, p9) - sy_h * 0.5 - 0.25;
    let hz = lerp(holder.prev.z, holder.pos.z, p9) - cz * 0.7 + cx * 0.5 * cp;
    // Ponto de fixação no mob (lvt_31 a lvt_37): o lado do corpo, a 0,4 da largura, por renderYawOffset + 90 graus.
    let (mw, mh) = {
        let (w, h) = mob.size();
        (w as f64, h as f64)
    };
    let body = lerp(mob.prev_body_yaw as f64, mob.body_yaw as f64, p9) * DEG + std::f64::consts::FRAC_PI_2;
    let (ox, oz) = (body.cos() * mw * 0.4, body.sin() * mw * 0.4);
    let sx = lerp(mob.prev.x, mob.pos.x, p9) + ox;
    let sy_m = lerp(mob.prev.y, mob.pos.y, p9);
    let sz = lerp(mob.prev.z, mob.pos.z, p9) + oz;
    // A base da corda (p4 do doRender) desce (1,6 - altura) / 2.
    let base_y = sy_m - (1.6 - mh) * 0.5;
    // Os deltas passam por float no Java: `(double)((float)(...))`.
    let dx = (hx - sx) as f32 as f64;
    let dy = (hy - sy_m) as f32 as f64;
    let dz = (hz - sz) as f32 as f64;

    let mut out: [Vec<(DVec3, [u8; 4])>; 2] = [Vec::with_capacity(50), Vec::with_capacity(50)];
    for i in 0..=24u32 {
        // lvt_49..51: 0,5 / 0,4 / 0,3, vezes 0,7 nos pares; `WorldRenderer.color` trunca com `(int)`.
        let mut c = [0.5f32, 0.4, 0.3];
        if i % 2 == 0 {
            c = c.map(|v| v * 0.7);
        }
        let col = [(c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8, 255];
        let f = i as f32 / 24.0;
        let t = f as f64;
        // lvt_41: (f * f + f) em float; o balanço (24 - i) / 18 + 0,125 em float.
        let q = (f * f + f) as f64;
        let yoff = ((24.0 - i as f32) / 18.0 + 0.125) as f64;
        let x = sx + dx * t;
        let y = base_y + dy * q * 0.5 + yoff;
        let z = sz + dz * t;
        let p = |a: f64, b: f64, c: f64| DVec3::new(x + a, y + b, z + c);
        // Fita 1: A = (x, y, z) e B = (x + 0,025, y + 0,025, z). Fita 2: A = (x, y + 0,025, z), B = (x + 0,025, y, z + 0,025).
        out[0].push((p(0.0, 0.0, 0.0), col));
        out[0].push((p(0.025, 0.025, 0.0), col));
        out[1].push((p(0.0, 0.025, 0.0), col));
        out[1].push((p(0.025, 0.0, 0.025), col));
    }
    out
}

/// `RenderLiving.renderLeash`: a corda de `mob` até `holder`, em quads só de cor, sem textura e sem luz (o
/// `disableTexture2D` e o `disableLighting` do Java). `white` é o UV de um ponto branco do atlas.
pub fn leash_rope(geo: &mut Geometry, mob: &Entity, holder: &Anchor, partial: f32, white: [f32; 2]) {
    for strip in leash_strips(mob, holder, partial) {
        // Os triângulos (A0, B0, A1) e (B0, A1, B1) da fita cobrem o quadrilátero (A0, B0, B1, A1).
        for k in 0..strip.len() / 2 - 1 {
            let v = |j: usize| {
                let (p, col) = strip[j];
                Vertex { pos: p.as_vec3().to_array(), uv: white, color: col, light: FULL_LIGHT }
            };
            geo.quad([v(2 * k), v(2 * k + 1), v(2 * k + 3), v(2 * k + 2)]);
        }
    }
}

/// Posição do centro da pintura/moldura (`EntityHanging.updateBoundingBox`) a partir do bloco pendurado.
pub fn hanging_pos(hanging: [i32; 3], facing: u8, width_px: i32, height_px: i32) -> [f64; 3] {
    // EnumFacing horizontal: 0 sul, 1 oeste, 2 norte, 3 leste; rotateYCCW: sul->leste, oeste->sul, norte->oeste, leste->norte.
    const FRONT: [(f64, f64); 4] = [(0.0, 1.0), (-1.0, 0.0), (0.0, -1.0), (1.0, 0.0)];
    const CCW: [usize; 4] = [3, 0, 1, 2];
    let half = |px: i32| if px % 32 == 0 { 0.5 } else { 0.0 };
    let (fx, fz) = FRONT[facing as usize & 3];
    let (cx, cz) = FRONT[CCW[facing as usize & 3]];
    let (d4, d5) = (half(width_px), half(height_px));
    [hanging[0] as f64 + 0.5 - fx * 0.46875 + d4 * cx, hanging[1] as f64 + 0.5 + d5, hanging[2] as f64 + 0.5 - fz * 0.46875 + d4 * cz]
}

/// Índice em `ARTS` pelo título (`EnumArt.title`); desconhecido cai em `Kebab`.
pub fn art_index(title: &str) -> usize {
    crate::gen_paintings::ARTS.iter().position(|a| a.title == title).unwrap_or(0)
}

/// `RenderPainting`: um conjunto de seis faces por pedaço de 16x16, cada um com a luz da sua célula.
fn draw_painting(e: &Entity, pos: Vec3, data: i32, cx: &mut Cx) {
    let Some(tex) = cx.textures.jar(cx.r, cx.jar, "painting/paintings_kristoffer_zetterstrand") else { return };
    let art = crate::gen_paintings::ARTS[(data & 0xFF) as usize % crate::gen_paintings::ARTS.len()];
    let facing = ((data >> 8) & 3) as u8;
    let (sx, sy, ox, oy) = (art.size_x as i32, art.size_y as i32, art.offset_x as i32, art.offset_y as i32);
    let _ = e;
    let m = tr(pos.x, pos.y, pos.z) * rot(180.0 - facing as f32 * 90.0, 0.0, 1.0, 0.0) * Mat4::from_scale(Vec3::splat(0.0625));
    let (lo_x, lo_y) = (-(sx as f32) / 2.0, -(sy as f32) / 2.0);
    let (d, e0, e1) = (0.5f32, 0.001953125f32, 0.7519531f32);
    let v = Vec3::new;
    let geo = cx.out.entry(Tex::Custom(tex)).or_default();
    for i in 0..sx / 16 {
        for j in 0..sy / 16 {
            let (xr, xl) = (lo_x + ((i + 1) * 16) as f32, lo_x + (i * 16) as f32);
            let (yt, yb) = (lo_y + ((j + 1) * 16) as f32, lo_y + (j * 16) as f32);
            let light = painting_light(cx.world, pos, facing, (xr + xl) / 2.0, (yt + yb) / 2.0);
            let ua = (ox + sx - i * 16) as f32 / 256.0;
            let ub = (ox + sx - (i + 1) * 16) as f32 / 256.0;
            let va = (oy + sy - j * 16) as f32 / 256.0;
            let vb = (oy + sy - (j + 1) * 16) as f32 / 256.0;
            let w = [1.0; 3];
            quad(geo, &m, [v(xr, yb, -d), v(xl, yb, -d), v(xl, yt, -d), v(xr, yt, -d)], [[ub, va], [ua, va], [ua, vb], [ub, vb]], -Vec3::Z, w, 255, light);
            quad(geo, &m, [v(xr, yt, d), v(xl, yt, d), v(xl, yb, d), v(xr, yb, d)], [[0.75, 0.0], [0.8125, 0.0], [0.8125, 0.0625], [0.75, 0.0625]], Vec3::Z, w, 255, light);
            quad(geo, &m, [v(xr, yt, -d), v(xl, yt, -d), v(xl, yt, d), v(xr, yt, d)], [[0.75, e0], [0.8125, e0], [0.8125, e0], [0.75, e0]], Vec3::Y, w, 255, light);
            quad(geo, &m, [v(xr, yb, d), v(xl, yb, d), v(xl, yb, -d), v(xr, yb, -d)], [[0.75, e0], [0.8125, e0], [0.8125, e0], [0.75, e0]], -Vec3::Y, w, 255, light);
            quad(geo, &m, [v(xr, yt, d), v(xr, yb, d), v(xr, yb, -d), v(xr, yt, -d)], [[e1, 0.0], [e1, 0.0625], [e1, 0.0625], [e1, 0.0]], -Vec3::X, w, 255, light);
            quad(geo, &m, [v(xl, yt, -d), v(xl, yb, -d), v(xl, yb, d), v(xl, yt, d)], [[e1, 0.0], [e1, 0.0625], [e1, 0.0625], [e1, 0.0]], Vec3::X, w, 255, light);
        }
    }
}

/// `RenderPainting.setLightmap`: a célula na frente do pedaço (`getCombinedLight`).
fn painting_light(world: &World, pos: Vec3, facing: u8, xc: f32, yc: f32) -> [u8; 4] {
    let (px, py, pz) = (pos.x as f64, pos.y as f64, pos.z as f64);
    let y = (py + (yc / 16.0) as f64).floor() as i32;
    let (mut x, mut z) = (px.floor() as i32, pz.floor() as i32);
    let off = (xc / 16.0) as f64;
    match facing {
        2 => x = (px + off).floor() as i32,
        1 => z = (pz - off).floor() as i32,
        0 => x = (px - off).floor() as i32,
        _ => z = (pz + off).floor() as i32,
    }
    let l = cell_light(world, x, y, z);
    [(l >> 4) * 17, (l & 15) * 17, 255, 0]
}

/// O mapa da moldura (`renderMap(data, true)` com `disableLighting`).
fn draw_map(cx: &mut Cx, id: i32, m: &Mat4) {
    crate::maps::emit(&mut cx.cache.maps, cx.textures, cx.r, cx.jar, id, m, true, cx.light, 1.0, cx.out);
}

/// Quads do modelo `block/item_frame` (ou `item_frame_map`) no atlas de blocos.
fn frame_model(cx: &mut Cx, map: bool) -> Vec<(Vec<Vec3>, Vec<[f32; 2]>, Vec3)> {
    let slot = map as usize;
    if let Some(q) = &cx.cache.frame[slot] {
        return q.clone();
    }
    let name = if map { "block/item_frame_map" } else { "block/item_frame" };
    let jar = &mut *cx.jar;
    let mut bytes = |p: &str| jar.bytes(p);
    let mut out = Vec::new();
    if let Some(model) = cx.cache.lib.resolve(name, &mut bytes) {
        let mut names: Vec<String> = Vec::new();
        let baked = mc_world::model::bake(&model, mc_world::model::Rot::new(0, 0), false, &mut |t: &str| {
            let i = names.iter().position(|n| n == t).unwrap_or_else(|| {
                names.push(t.to_string());
                names.len() - 1
            });
            i as u16
        });
        for q in &baked.quads {
            let tile = names.get(q.tex as usize).and_then(|n| n.strip_prefix("blocks/")).and_then(|b| cx.table.textures.iter().position(|t| t == b));
            let Some(tile) = tile else { continue };
            let pos = q.pos.iter().map(|p| Vec3::from(*p)).collect();
            let uv = q.uv.iter().map(|[u, v]| atlas_uv(tile as u16, *u, *v)).collect();
            out.push((pos, uv, FACE_N[q.face as usize % 6]));
        }
    }
    cx.cache.frame[slot] = Some(out.clone());
    out
}

/// `RenderItemFrame`: a moldura e o item (rotação em passos de 45° em Z, display `fixed`). Mapa: pendente.
fn draw_frame(e: &Entity, cx: &mut Cx) {
    let item = match e.meta.get(&8) {
        Some(MetaValue::Slot(Some(s))) => Some(s.clone()),
        _ => None,
    };
    let is_map = item.as_ref().is_some_and(|s| s.id == 358);
    let h = e.obj.hanging;
    let facing = e.obj.facing;
    let center = tr(h[0] as f32 + 0.5, h[1] as f32 + 0.5, h[2] as f32 + 0.5) * rot(180.0 - facing as f32 * 90.0, 0.0, 1.0, 0.0);
    let quads = frame_model(cx, is_map);
    {
        let m = center * tr(-0.5, -0.5, -0.5);
        let geo = cx.out.entry(Tex::Blocks).or_default();
        for (p, uv, n) in &quads {
            let p = [p[0], p[1], p[2], p[3]];
            let uv = [uv[0], uv[1], uv[2], uv[3]];
            quad(geo, &m, p, uv, *n, [1.0; 3], 255, cx.light);
        }
    }
    let Some(s) = item else { return };
    if is_map {
        // RenderItemFrame.renderItem com ItemMap: giro em passos de 90°, e `renderMap(data, true)`.
        let mut rotation = e.meta_byte(9) as i32;
        if rotation < 0 {
            rotation += 256;
        }
        let i = rotation % 4 * 2;
        let m = center * tr(0.0, 0.0, 0.4375) * rot(i as f32 * 360.0 / 8.0, 0.0, 0.0, 1.0) * rot(180.0, 0.0, 0.0, 1.0) * Mat4::from_scale(Vec3::splat(0.0078125)) * tr(-64.0, -64.0, 0.0) * tr(0.0, 0.0, -1.0);
        draw_map(cx, s.damage as i32, &m);
        return;
    }
    let Some(model) = cx.items.get(s.id, s.damage, cx.jar, cx.table, cx.r) else { return };
    if model.entity {
        return;
    }
    let mut rotation = e.meta_byte(9) as i32;
    if rotation < 0 {
        rotation += 256;
    }
    let mut m = center * tr(0.0, 0.0, 0.4375) * rot(rotation as f32 * 360.0 / 8.0, 0.0, 0.0, 1.0) * Mat4::from_scale(Vec3::splat(0.5));
    if !model.gui3d || s.id == 397 {
        m *= rot(180.0, 0.0, 1.0, 0.0);
    }
    // enableStandardItemLighting dentro da matriz: as luzes giram junto com ela.
    let lights = [m.transform_vector3(WORLD_LIGHTS[0]).normalize(), m.transform_vector3(WORLD_LIGHTS[1]).normalize()];
    if !model.gui3d {
        m *= Mat4::from_scale(Vec3::splat(2.0));
    }
    if let Some(d) = model.display_fixed {
        m *= d.matrix();
    }
    let m = m * Mat4::from_scale(Vec3::splat(0.5)) * tr(-0.5, -0.5, -0.5);
    model.emit_raw(m, &lights, cx.light, cx.out.entry(model.tex).or_default());
}

/// `RenderWitherSkull`: `ModelSkeletonHead()` (UV 0,35 numa textura 64x64) girada pelo yaw e pitch interpolados;
/// a caveira azul (`wither_invulnerable`) quando o byte 10 do metadado é 1.
fn draw_wither_skull(e: &Entity, pos: Vec3, cx: &mut Cx) {
    let path = if e.meta_byte(10) == 1 { "entity/wither/wither_invulnerable" } else { "entity/wither/wither" };
    let Some(tex) = cx.textures.jar(cx.r, cx.jar, path) else { return };
    let t = cx.t;
    // func_82400_a: a diferença do yaw passa por [-180, 180).
    let mut d = e.yaw - e.prev_yaw;
    while d < -180.0 {
        d += 360.0;
    }
    while d >= 180.0 {
        d -= 360.0;
    }
    let yaw = e.prev_yaw + t * d;
    let pitch = e.prev_pitch + (e.pitch - e.prev_pitch) * t;
    let mut head = crate::models::Part::new([0.0; 3]).cube((0, 35), [-4.0, -8.0, -4.0], [8, 8, 8], 0.0, false);
    head.rot = Vec3::new(pitch / 57.295776, yaw / 57.295776, 0.0);
    let m = tr(pos.x, pos.y, pos.z) * Mat4::from_scale(Vec3::new(-1.0, -1.0, 1.0));
    let mut sink = crate::models::Sink { geo: cx.out.entry(Tex::Custom(tex)).or_default(), tex: (64.0, 64.0), tint: [1.0; 3], alpha: 255, light: cx.light, lights: WORLD_LIGHTS };
    sink.part(&head, &m);
}

/// `EntityLightningBolt.onUpdate` do lado do cliente: pisca e some depois de 1 a 3 clarões.
pub fn lightning_tick(o: &mut ObjState, id: i32) {
    let first = o.rng.is_none();
    let mut r = JRandom(o.rng.unwrap_or_else(|| JRandom::new(hover_for(id).to_bits() as i64).0));
    if first {
        o.bolt_vertex = r.next_long();
        o.bolt_state = 2;
        o.bolt_life = r.next_int(3) + 1;
    }
    // EntityLightningBolt.onUpdate: o som sai do mesmo `rand`, antes de apagar.
    if o.bolt_state == 2 {
        let thunder = 0.8 + r.next_float() * 0.2;
        let explode = 0.5 + r.next_float() * 0.2;
        o.thunder = Some((thunder, explode));
    }
    o.bolt_state -= 1;
    if o.bolt_state < 0 {
        if o.bolt_life == 0 {
            o.expired = o.fixed_age.is_none();
        } else if o.bolt_state < -r.next_int(10) {
            o.bolt_life -= 1;
            o.bolt_state = 1;
            o.bolt_vertex = r.next_long();
        }
    }
    o.flash = o.bolt_state >= 0;
    o.rng = Some(r.0);
}

/// `RenderLightningBolt`: quatro camadas de tiras com `Random(boltVertex)`, cor (0.45, 0.45, 0.5, 0.3) aditiva.
fn draw_lightning(e: &Entity, pos: Vec3, cx: &mut Cx) {
    let white = cx.textures.white(cx.r);
    let vertex = e.obj.bolt_vertex;
    let (mut xs, mut zs) = ([0f64; 8], [0f64; 8]);
    let (mut x0, mut z0) = (0.0f64, 0.0f64);
    let mut r = JRandom::new(vertex);
    for i in (0..8).rev() {
        xs[i] = x0;
        zs[i] = z0;
        x0 += (r.next_int(11) - 5) as f64;
        z0 += (r.next_int(11) - 5) as f64;
    }
    let geo = cx.glow.entry(Tex::Custom(white)).or_default();
    let color = [115, 115, 128, 77];
    let (px, py, pz) = (pos.x as f64, pos.y as f64, pos.z as f64);
    for layer in 0..4 {
        let mut r2 = JRandom::new(vertex);
        for k in 0..3 {
            let a: usize = if k > 0 { 7 - k } else { 7 };
            let b: i32 = if k > 0 { a as i32 - 2 } else { 0 };
            let (mut dx, mut dz) = (xs[a] - x0, zs[a] - z0);
            let mut i = a as i32;
            while i >= b {
                let (dx0, dz0) = (dx, dz);
                if k == 0 {
                    dx += (r2.next_int(11) - 5) as f64;
                    dz += (r2.next_int(11) - 5) as f64;
                } else {
                    dx += (r2.next_int(31) - 15) as f64;
                    dz += (r2.next_int(31) - 15) as f64;
                }
                let mut w1 = 0.1 + layer as f64 * 0.2;
                if k == 0 {
                    w1 *= i as f64 * 0.1 + 1.0;
                }
                let mut w2 = 0.1 + layer as f64 * 0.2;
                if k == 0 {
                    w2 *= (i - 1) as f64 * 0.1 + 1.0;
                }
                let mut strip: Vec<[f32; 3]> = Vec::new();
                for j in 0..5 {
                    let corner = |w: f64| {
                        let (mut vx, mut vz) = (px + 0.5 - w, pz + 0.5 - w);
                        if j == 1 || j == 2 {
                            vx += w * 2.0;
                        }
                        if j == 2 || j == 3 {
                            vz += w * 2.0;
                        }
                        (vx, vz)
                    };
                    let (ux, uz) = corner(w2);
                    let (vx, vz) = corner(w1);
                    strip.push([(ux + dx) as f32, (py + (i * 16) as f64) as f32, (uz + dz) as f32]);
                    strip.push([(vx + dx0) as f32, (py + ((i + 1) * 16) as f64) as f32, (vz + dz0) as f32]);
                }
                for q in 0..4 {
                    let c = |n: usize| Vertex { pos: strip[n], uv: [0.5, 0.5], color, light: FULL_LIGHT };
                    geo.quad_fixed([c(2 * q), c(2 * q + 1), c(2 * q + 3), c(2 * q + 2)]);
                }
                i -= 1;
            }
        }
    }
}

/// Um tick da entidade não-viva (`onUpdate` do lado do cliente): a flecha gira com a velocidade.
pub fn tick(e: &mut Entity) {
    let Kind::Object { kind, .. } = e.kind else { return };
    if kind == LIGHTNING {
        lightning_tick(&mut e.obj, e.id);
    }
    if e.obj.shake > 0 {
        e.obj.shake -= 1;
    }
}

const DEG: f64 = 180.0 / 3.1415927410125732;

/// O giro dos projéteis (`EntityArrow`/`EntityThrowable`/`EntityFireball.onUpdate`): o alvo pela volta curta
/// e 20% do caminho por tick.
fn turn_to(e: &mut Entity, mut yaw: f32, mut pitch: f32) {
    while pitch - e.prev_pitch < -180.0 {
        e.prev_pitch -= 360.0;
    }
    while pitch - e.prev_pitch >= 180.0 {
        e.prev_pitch += 360.0;
    }
    while yaw - e.prev_yaw < -180.0 {
        e.prev_yaw -= 360.0;
    }
    while yaw - e.prev_yaw >= 180.0 {
        e.prev_yaw += 360.0;
    }
    pitch = e.prev_pitch + (pitch - e.prev_pitch) * 0.2;
    yaw = e.prev_yaw + (yaw - e.prev_yaw) * 0.2;
    e.pitch = pitch;
    e.yaw = yaw;
}

/// Caixa de uma entidade de largura `w` e altura `h` com os pés em `pos`.
fn bb_at(pos: DVec3, w: f64, h: f64) -> crate::physics::Bb {
    crate::physics::Bb { min: DVec3::new(pos.x - w / 2.0, pos.y, pos.z - w / 2.0), max: DVec3::new(pos.x + w / 2.0, pos.y + h, pos.z + w / 2.0) }
}

/// `Entity.moveEntity` sem degrau: para no que colide, zera a velocidade do eixo batido e quica no slime.
fn move_object(e: &mut Entity, world: &mc_world::World, table: &BlockTable, w: f64, h: f64) {
    let d = e.vel;
    let (bb, m) = crate::physics::move_box(world, table, bb_at(e.pos, w, h), d);
    e.pos = DVec3::new((bb.min.x + bb.max.x) / 2.0, bb.min.y, (bb.min.z + bb.max.z) / 2.0);
    e.on_ground = m.y != d.y && d.y < 0.0;
    if m.x != d.x {
        e.vel.x = 0.0;
    }
    if m.z != d.z {
        e.vel.z = 0.0;
    }
    if m.y != d.y {
        // Block.onLanded: o slime (165) devolve a queda; o resto zera.
        let below = crate::physics::block_id(world, e.pos.x.floor() as i32, (e.pos.y - 0.20000000298023224).floor() as i32, e.pos.z.floor() as i32);
        if below == 165 && e.vel.y < 0.0 {
            e.vel.y = -e.vel.y;
        } else {
            e.vel.y = 0.0;
        }
    }
}

fn in_water(e: &Entity, world: &mc_world::World, w: f64, h: f64) -> bool {
    crate::physics::box_in_water(world, &bb_at(e.pos, w, h))
}

/// O `onUpdate` do lado do cliente das entidades não vivas que andam sozinhas entre um pacote e outro do
/// servidor (itens, TNT, blocos caindo, projéteis e bolas de fogo). A flecha cravada fica parada até o
/// bloco mudar. Sem as partículas (fumaça do TNT e da bola de fogo, bolhas na água).
pub fn tick_world(e: &mut Entity, world: &mc_world::World, table: &BlockTable) {
    let Kind::Object { kind, .. } = e.kind else { return };
    match kind {
        // EntityItem.onUpdate (o pushOutOfBlocks e a correnteza não foram portados).
        ITEM => {
            e.vel.y -= 0.03999999910593033;
            move_object(e, world, table, 0.25, 0.25);
            let mut f = 0.98f32;
            if e.on_ground {
                f = crate::physics::slipperiness(world, e.pos.x.floor() as i32, e.pos.y.floor() as i32 - 1, e.pos.z.floor() as i32) * 0.98;
            }
            e.vel.x *= f as f64;
            e.vel.y *= 0.9800000190734863;
            e.vel.z *= f as f64;
            if e.on_ground {
                e.vel.y *= -0.5;
            }
        }
        // EntityTNTPrimed.onUpdate e EntityFallingBlock.onUpdate (no cliente o bloco caindo não pousa: o
        // servidor tira a entidade e manda o bloco).
        TNT | FALLING_BLOCK => {
            e.vel.y -= 0.03999999910593033;
            move_object(e, world, table, 0.98, 0.98);
            e.vel *= 0.9800000190734863;
            if kind == TNT && e.on_ground {
                e.vel.x *= 0.699999988079071;
                e.vel.z *= 0.699999988079071;
                e.vel.y *= -0.5;
            }
        }
        // EntityThrowable.onUpdate: no cliente o impacto não para nada; o servidor tira a entidade.
        SNOWBALL | EGG | ENDER_PEARL | POTION | EXP_BOTTLE => {
            e.pos += e.vel;
            let v = e.vel;
            let h = (v.x * v.x + v.z * v.z).sqrt();
            turn_to(e, (v.x.atan2(v.z) * DEG) as f32, (v.y.atan2(h) * DEG) as f32);
            let f = if in_water(e, world, 0.25, 0.25) { 0.8f32 } else { 0.99 };
            let gravity: f32 = match kind {
                POTION => 0.05,
                EXP_BOTTLE => 0.07,
                _ => 0.03,
            };
            e.vel *= f as f64;
            e.vel.y -= gravity as f64;
        }
        // EntityFireball.onUpdate: anda, gira e acelera na direção do disparo.
        LARGE_FIREBALL | SMALL_FIREBALL | WITHER_SKULL => {
            e.pos += e.vel;
            let v = e.vel;
            let h = (v.x * v.x + v.z * v.z).sqrt();
            turn_to(e, (v.z.atan2(v.x) * DEG) as f32 + 90.0, (h.atan2(v.y) * DEG) as f32 - 90.0);
            let size = if kind == LARGE_FIREBALL { 1.0 } else { 0.3125 };
            // EntityWitherSkull.getMotionFactor: 0.73 com a caveira azul (metadado 10).
            let mut f = if kind == WITHER_SKULL && e.meta_byte(10) == 1 { 0.73f32 } else { 0.95 };
            if in_water(e, world, size, size) {
                f = 0.8;
            }
            e.vel += e.obj.accel;
            e.vel *= f as f64;
        }
        ARROW => arrow_tick(e, world, table),
        _ => {}
    }
}

/// `EntityArrow.onUpdate` no cliente: o raio até o próximo ponto acha o bloco (sem os que não têm colisão),
/// a flecha crava 0.05 antes do ponto e balança 7 ticks; cravada, só volta a cair se o bloco mudar.
fn arrow_tick(e: &mut Entity, world: &mc_world::World, table: &BlockTable) {
    let v = e.vel;
    if e.prev_pitch == 0.0 && e.prev_yaw == 0.0 {
        let h = (v.x * v.x + v.z * v.z).sqrt();
        e.yaw = (v.x.atan2(v.z) * DEG) as f32;
        e.pitch = (v.y.atan2(h) * DEG) as f32;
        e.prev_yaw = e.yaw;
        e.prev_pitch = e.pitch;
    }
    if e.obj.in_ground {
        let (x, y, z) = e.obj.tile;
        if world.block(x, y, z) == e.obj.tile_state {
            e.obj.ticks_in_ground += 1;
            return;
        }
        // O bloco mudou: volta a cair com um pouco da velocidade (rand * 0.2 no jogo).
        e.obj.in_ground = false;
        e.vel *= 0.1;
        e.obj.ticks_in_ground = 0;
    }
    let len = e.vel.length();
    if len > 0.0 {
        if let Some(hit) = crate::interact::raycast(world, table, e.pos, e.vel, len).filter(|h| crate::physics::has_collision(world, table, h.pos.0, h.pos.1, h.pos.2)) {
            e.obj.tile = hit.pos;
            e.obj.tile_state = world.block(hit.pos.0, hit.pos.1, hit.pos.2);
            e.vel = hit.point - e.pos;
            let f2 = e.vel.length();
            if f2 > 0.0 {
                e.pos -= e.vel / f2 * 0.05000000074505806;
            }
            e.obj.in_ground = true;
            e.obj.shake = 7;
        }
    }
    e.pos += e.vel;
    let v = e.vel;
    let h = (v.x * v.x + v.z * v.z).sqrt();
    turn_to(e, (v.x.atan2(v.z) * DEG) as f32, (v.y.atan2(h) * DEG) as f32);
    let f = if in_water(e, world, 0.5, 0.5) { 0.6f32 } else { 0.99 };
    e.vel *= f as f64;
    e.vel.y -= 0.05000000074505806;
}

/// Gancho de teste `MINE_RS_TEST_OBJ=item:264,item:1:0:40,xp:20,flecha,bola,...`: lista de entidades.
/// `N*item` repete o item N vezes (teste de carga, ex. `1000*tnt:80`); com muitos a grade fica quadrada.
pub fn test_entities(spec: &str) -> Vec<Entity> {
    let mut out = Vec::new();
    let items: Vec<&str> = spec
        .split(',')
        .filter(|s| !s.is_empty())
        .flat_map(|s| match s.split_once('*') {
            Some((n, it)) => std::iter::repeat_n(it, n.trim().parse().unwrap_or(1)),
            None => std::iter::repeat_n(s, 1),
        })
        .collect();
    let cols = (items.len() as f64).sqrt().ceil().max(6.0) as usize;
    for (n, item) in items.into_iter().enumerate() {
        let mut it = item.split(':');
        let name = it.next().unwrap_or("").trim();
        let rest: Vec<&str> = it.collect();
        let a: Vec<i32> = rest.iter().filter_map(|v| v.trim().parse().ok()).collect();
        let arg = |i: usize, d: i32| a.get(i).copied().unwrap_or(d);
        let (col, row) = ((n % cols) as f64, (n / cols) as f64);
        let (x, z) = (0.5 + (col - (cols as f64 - 1.0) / 2.0) * 1.5, 4.0 + row * 2.0);
        let fixed = [(x * 32.0) as i32, (64.0 * 32.0) as i32, (z * 32.0) as i32];
        let id = -300 - n as i32;
        let (kind, data) = match name {
            "item" => (ITEM, 0),
            "xp" => (XP_ORB, arg(0, 1)),
            "flecha" => (ARROW, 0),
            "bola" => (SNOWBALL, 0),
            "ovo" => (EGG, 0),
            "perola" => (ENDER_PEARL, 0),
            "olho" => (ENDER_EYE, 0),
            "pocao" => (POTION, arg(0, 16)),
            "garrafa" => (EXP_BOTTLE, 0),
            "fogos" => (FIREWORK, 0),
            "bolinha" => (SMALL_FIREBALL, 0),
            "bolona" => (LARGE_FIREBALL, 0),
            "tnt" => (TNT, 0),
            "caindo" => (FALLING_BLOCK, arg(0, 12) | (arg(1, 0) << 12)),
            "barco" => (BOAT, 0),
            "carrinho" => (MINECART, arg(0, 0)),
            "cristal" => (CRYSTAL, 0),
            // O pescador é o próprio jogador do mundo de teste (id -2).
            "anzol" => (FISH_HOOK, -2),
            "raio" => (LIGHTNING, 0),
            "caveira" => (WITHER_SKULL, 0),
            "pintura" => (PAINTING, 0),
            "moldura" => (ITEM_FRAME, 0),
            _ => continue,
        };
        let mut fixed = fixed;
        if !matches!(kind, ITEM | XP_ORB) {
            fixed[1] += 16;
        }
        let mut e = Entity::new(id, Kind::Object { kind, data }, fixed, 0.0, 0.0);
        if kind == ITEM {
            e.meta.insert(10, MetaValue::Slot(Some(Slot { id: arg(0, 264) as i16, count: arg(2, 1) as i8, damage: arg(1, 0) as i16, nbt: None })));
        }
        if kind == PAINTING {
            let idx = art_index(rest.first().copied().unwrap_or("Kebab"));
            let facing = (arg(0, 0) & 3) as u8;
            let art = crate::gen_paintings::ARTS[idx];
            let hanging = [fixed[0].div_euclid(32), 64, fixed[2].div_euclid(32)];
            let p = hanging_pos(hanging, facing, art.size_x as i32, art.size_y as i32);
            e.kind = Kind::Object { kind, data: idx as i32 | (facing as i32) << 8 };
            e.pos = glam::DVec3::new(p[0], p[1], p[2]);
            e.prev = e.pos;
            e.obj.hanging = hanging;
            e.obj.facing = facing;
        }
        if kind == ITEM_FRAME {
            let hanging = [fixed[0].div_euclid(32), 64, fixed[2].div_euclid(32)];
            e.kind = Kind::Object { kind, data: arg(2, 0) };
            e.obj.hanging = hanging;
            e.obj.facing = (arg(2, 0) & 3) as u8;
            e.meta.insert(8, MetaValue::Slot(Some(Slot { id: arg(0, 264) as i16, count: 1, damage: 0, nbt: None })));
            e.meta.insert(9, MetaValue::Byte(arg(1, 0) as i8));
        }
        if kind == LIGHTNING {
            e.obj.fixed_age = Some(0);
        }
        if kind == TNT {
            e.obj.fixed_age = Some(arg(0, 0));
        }
        if kind == ARROW {
            e.yaw = 30.0;
            e.prev_yaw = 30.0;
            e.pitch = -20.0;
            e.prev_pitch = -20.0;
        }
        out.push(e);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carrinho_segue_o_trilho() {
        let mut w = crate::test_world::build();
        // Trilho norte-sul (forma 0) e subida para leste (forma 2) no chão de teste.
        w.set_block(5, 64, 5, 66 << 4);
        w.set_block(8, 64, 5, 66 << 4 | 2);
        let p = rail_pos(&w, 5.3, 64.2, 5.7).unwrap();
        assert!((p.x - 5.5).abs() < 1e-9 && (p.y - 64.0625).abs() < 1e-9 && (p.z - 5.7).abs() < 1e-9, "{p:?}");
        // Na subida, no meio do bloco: d2 (63,5625) + d8·d0 (0,5) + 0,5 do d8 > 0.
        let q = rail_pos(&w, 8.5, 64.2, 5.5).unwrap();
        assert!((q.y - 64.5625).abs() < 1e-9, "{q:?}");
        let (a, b) = (rail_offset(&w, 8.5, 64.2, 5.5, 0.3).unwrap(), rail_offset(&w, 8.5, 64.2, 5.5, -0.3).unwrap());
        assert!(a.y > b.y, "{a:?} {b:?}");
        assert!(rail_pos(&w, 0.5, 64.0, 0.5).is_none());
    }

    #[test]
    fn raio_toca_o_trovao_no_primeiro_tick_e_acende_o_ceu() {
        let mut o = ObjState::default();
        lightning_tick(&mut o, 9);
        let (thunder, explode) = o.thunder.expect("trovão no primeiro tick");
        assert!((0.8..=1.0).contains(&thunder) && (0.5..=0.7).contains(&explode));
        assert!(o.flash);
        o.thunder = None;
        lightning_tick(&mut o, 9);
        assert!(o.thunder.is_none());
    }

    #[test]
    fn model_count_follows_stack_size() {
        let got: Vec<usize> = [1, 2, 16, 17, 32, 33, 48, 49, 64].iter().map(|&n| model_count(n)).collect();
        assert_eq!(got, [1, 2, 2, 3, 3, 4, 4, 5, 5]);
    }

    #[test]
    fn bob_and_spin_follow_java() {
        // sin(0) = 0: o item parte em 0,1 e sobe até 0,2.
        assert!((bob(0.0, 0.0) - 0.1).abs() < 1e-6);
        let top = bob(std::f32::consts::FRAC_PI_2 * 10.0, 0.0);
        assert!((top - 0.2).abs() < 1e-3, "{top}");
        // 20 ticks = 1 radiano = 57,295776 graus.
        assert!((spin_deg(20.0, 0.0) - 57.295776).abs() < 1e-4);
        assert!((spin_deg(0.0, 1.0) - 57.295776).abs() < 1e-4);
    }

    #[test]
    fn xp_orb_texture_thresholds() {
        let got: Vec<i32> = [0, 2, 3, 6, 7, 16, 17, 36, 37, 72, 73, 148, 149, 306, 307, 616, 617, 1236, 1237, 2476, 2477].iter().map(|&v| xp_texture(v)).collect();
        assert_eq!(got, [0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10]);
    }

    #[test]
    fn extra_copies_use_seed_187() {
        let mut r = JRandom::new(187);
        let a: Vec<f32> = (0..3).map(|_| r.next_float()).collect();
        let mut r = JRandom::new(187);
        let b: Vec<f32> = (0..3).map(|_| r.next_float()).collect();
        assert_eq!(a, b);
        assert!(a.iter().all(|v| (0.0..1.0).contains(v)));
    }

    #[test]
    fn arrow_turns_with_velocity_and_falls() {
        let (w, t) = (mc_world::World::new(true), BlockTable::new());
        let mut e = Entity::new(1, Kind::Object { kind: ARROW, data: 0 }, [0, 64 * 32, 0], 0.0, 0.0);
        e.vel = glam::DVec3::new(0.0, 0.0, 1.0);
        e.tick();
        tick_world(&mut e, &w, &t);
        // Sai para +Z (yaw 0) e a velocidade cai 0,05 por tick, com arrasto 0,99.
        assert!(e.pos.z > 0.99 && e.yaw.abs() < 1e-3);
        assert!((e.vel.z - 0.99).abs() < 1e-6 && (e.vel.y + 0.05).abs() < 1e-6);
    }

    /// Chão de pedra com o topo em y = 64 e uma parede de pedra em z = 3 (y 64 a 66).
    fn floor_and_wall() -> mc_world::World {
        let mut w = mc_world::World::new(true);
        for cx in -1..=1 {
            for cz in -1..=1 {
                let s = Box::new(mc_world::Section::empty(true));
                w.apply(mc_world::ChunkData { x: cx, z: cz, full: true, mask: 1 << 4, sections: vec![(4, s)], biomes: None });
            }
        }
        for x in -8..8 {
            for z in -8..8 {
                w.set_block(x, 63, z, 1 << 4);
            }
            for y in 64..67 {
                w.set_block(x, y, 3, 1 << 4);
            }
        }
        w
    }

    #[test]
    fn item_tnt_e_projeteis_andam_no_cliente() {
        let (w, t) = (floor_and_wall(), BlockTable::new());
        // Item solto a 2 blocos do chão: cai e para em cima da pedra.
        let mut item = Entity::new(2, Kind::Object { kind: ITEM, data: 0 }, [16, 66 * 32, 16], 0.0, 0.0);
        let mut tnt = Entity::new(3, Kind::Object { kind: TNT, data: 0 }, [16, 66 * 32, -64], 0.0, 0.0);
        for _ in 0..40 {
            for e in [&mut item, &mut tnt] {
                e.tick();
                tick_world(e, &w, &t);
            }
        }
        assert!((item.pos.y - 64.0).abs() < 1e-6 && item.on_ground, "{:?}", item.pos);
        assert!((tnt.pos.y - 64.0).abs() < 1e-6 && tnt.on_ground);
        // Pérola: anda a velocidade, arrasto 0.99 e gravidade 0.03.
        let mut perola = Entity::new(4, Kind::Object { kind: ENDER_PEARL, data: 0 }, [0, 70 * 32, 0], 0.0, 0.0);
        perola.vel = DVec3::new(0.5, 0.0, 0.0);
        perola.tick();
        tick_world(&mut perola, &w, &t);
        assert!((perola.pos.x - 0.5).abs() < 1e-9 && (perola.vel.x - 0.495).abs() < 1e-6 && (perola.vel.y + 0.03).abs() < 1e-6);
        // Bola de fogo parada que acelera 0.1 por tick na direção do disparo (fator 0.95).
        let mut fogo = Entity::new(5, Kind::Object { kind: LARGE_FIREBALL, data: 0 }, [0, 70 * 32, 0], 0.0, 0.0);
        fogo.obj.accel = DVec3::new(0.1, 0.0, 0.0);
        fogo.tick();
        tick_world(&mut fogo, &w, &t);
        assert!(fogo.pos.x == 0.0 && (fogo.vel.x - 0.095).abs() < 1e-6);
        // Flecha contra a parede: crava 0.05 antes da face e para de andar.
        let mut flecha = Entity::new(6, Kind::Object { kind: ARROW, data: 0 }, [16, 65 * 32, 32], 0.0, 0.0);
        flecha.vel = DVec3::new(0.0, 0.0, 1.5);
        for _ in 0..3 {
            flecha.tick();
            tick_world(&mut flecha, &w, &t);
        }
        assert!(flecha.obj.in_ground && flecha.obj.tile == (0, 64, 3), "{:?} {:?}", flecha.pos, flecha.obj.tile);
        assert!((flecha.pos.z - 2.95).abs() < 0.01, "{:?}", flecha.pos);
        let z = flecha.pos.z;
        flecha.tick();
        tick_world(&mut flecha, &w, &t);
        assert_eq!(flecha.pos.z, z);
    }

    #[test]
    fn tnt_swells_in_the_last_ten_ticks() {
        assert_eq!(tnt_scale(40, 0.0), 1.0);
        assert_eq!(tnt_scale(10, 1.0), 1.0);
        // fuse 0, parcial 0: f = 1 - 1/10 = 0.9 -> 0.9^4 = 0.6561.
        assert!((tnt_scale(0, 0.0) - (1.0 + 0.6561 * 0.3)).abs() < 1e-5);
        assert!(tnt_scale(1, 0.5) > tnt_scale(5, 0.5));
    }

    #[test]
    fn falling_block_state_from_packet_data() {
        // Areia (12) meta 0; id | meta << 12 vira id << 4 | meta.
        assert_eq!(falling_state(12), 12 << 4);
        assert_eq!(falling_state(5 | 3 << 12), 5 << 4 | 3);
    }

    #[test]
    fn test_hook_spawns_items() {
        let v = test_entities("item:264,item:1:0:40,xp:20,flecha,bola");
        assert_eq!(v.len(), 5);
        assert!(matches!(v[1].meta.get(&10), Some(MetaValue::Slot(Some(s))) if s.count == 40 && s.id == 1));
    }

    #[test]
    fn leash_rope_follows_renderleash() {
        // Vaca (0,9 x 1,3) em (0, 64, 0); jogador em (0, 64, 4), olhos a 1,62, yaw e pitch 0; parcial 0,5.
        let cow = Entity::new(1, Kind::Mob(92), [0, 64 * 32, 0], 0.0, 0.0);
        let holder = |pos: DVec3, hanging: bool| Anchor { prev: pos, pos, prev_yaw: 0.0, yaw: 0.0, prev_pitch: 0.0, pitch: 0.0, eye: 1.62, hanging, bb: (pos, pos) };
        let near = |got: DVec3, want: [f64; 3]| (got.x - want[0]).abs() < 1e-4 && (got.y - want[1]).abs() < 1e-4 && (got.z - want[2]).abs() < 1e-4;
        let s = leash_strips(&cow, &holder(DVec3::new(0.0, 64.0, 4.0), false), 0.5);
        // Fixação no jogador: x = 0 - 0,7, y = 64 + 1,62 * 0,7 - 0,25 = 64,884, z = 4 + 0,5. A corda sai do ombro
        // (ombro em z = 0,4 * 0,9 e base em 64 - (1,6 - 1,3) / 2) com o balanço de 24 - i.
        assert!(near(s[0][0].0, [0.0, 63.85 + 24.0 / 18.0 + 0.125, 0.36]), "{:?}", s[0][0]);
        assert!(near(s[0][48].0, [-0.7, 63.85 + 0.884 + 0.125, 4.5]), "{:?}", s[0][48]);
        // i = 12: t = 0,5, caimento 0,884 * (0,75) / 2 e balanço (24 - 12) / 18 + 0,125.
        assert!(near(s[0][24].0, [-0.35, 63.85 + 0.884 * 0.75 * 0.5 + 12.0 / 18.0 + 0.125, 0.36 + 4.14 * 0.5]), "{:?}", s[0][24]);
        // Cores: par 0 escurecido (0,35 / 0,28 / 0,21), par 1 cheio (0,5 / 0,4 / 0,3), tudo truncado.
        assert_eq!(s[0][0].1, [89, 71, 53, 255]);
        assert_eq!(s[0][2].1, [127, 102, 76, 255]);
        assert_eq!(s[0][48].1, [89, 71, 53, 255]);
        // Fita 2: deslocada 0,025 em Y no A e em Z no B.
        assert!(near(s[1][0].0, [0.0, 63.85 + 24.0 / 18.0 + 0.125 + 0.025, 0.36]), "{:?}", s[1][0]);
        assert!(near(s[1][1].0, [0.025, 63.85 + 24.0 / 18.0 + 0.125, 0.385]), "{:?}", s[1][1]);
        // Nó pendurado (EntityLeashKnot): sem ângulo, cai 1 bloco no seno, olho -0,0625; yaw 90 é ignorado.
        let mut knot = holder(DVec3::new(0.5, 64.5, 4.5), true);
        knot.eye = -0.0625;
        knot.yaw = 90.0;
        let s = leash_strips(&cow, &knot, 1.0);
        let hy = 64.5 + (-0.0625) * 0.7 + 0.5 - 0.25;
        assert!(near(s[0][48].0, [0.5, 63.85 + (hy - 64.0) + 0.125, 4.5]), "{:?}", s[0][48]);
    }

    #[test]
    fn leash_angles_use_half_partial() {
        // `interpolateValue(prevRotationYaw, rotationYaw, partial * 0.5F)`: com parcial 1 o yaw vale a metade (45 graus).
        let cow = Entity::new(1, Kind::Mob(92), [0, 64 * 32, 0], 0.0, 0.0);
        let h = Anchor { prev: DVec3::new(0.0, 64.0, 0.0), pos: DVec3::new(0.0, 64.0, 0.0), prev_yaw: 0.0, yaw: 90.0, prev_pitch: 0.0, pitch: 0.0, eye: 1.62, hanging: false, bb: (DVec3::ZERO, DVec3::ZERO) };
        let s = leash_strips(&cow, &h, 1.0);
        let hx = -0.7 * (45f64).to_radians().cos() - 0.5 * (45f64).to_radians().sin();
        assert!((s[0][48].0.x - hx).abs() < 1e-4, "{:?}", s[0][48]);
    }

    #[test]
    fn knot_box_is_six_by_eight_by_six_pixels() {
        // ModelLeashKnot: addBox(-3, -6, -3, 6, 8, 6), com scale(-1,-1,1) e 1/16 por pixel, centro em (1,5; 64,5; 2,5).
        let part = Part::new([0.0; 3]).cube((0, 0), [-3.0, -6.0, -3.0], [6, 8, 6], 0.0, false);
        let m = Mat4::from_translation(Vec3::new(1.5, 64.5, 2.5)) * Mat4::from_scale(Vec3::new(-1.0, -1.0, 1.0));
        let mut geo = Geometry::default();
        let mut sink = Sink { geo: &mut geo, tex: (32.0, 32.0), tint: [1.0; 3], alpha: 255, light: FULL_LIGHT, lights: WORLD_LIGHTS };
        sink.part(&part, &m);
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for v in &geo.vertices {
            let p = Vec3::from(v.pos);
            lo = lo.min(p);
            hi = hi.max(p);
        }
        assert!(((hi - lo) - Vec3::new(0.375, 0.5, 0.375)).abs().max_element() < 1e-5, "{lo:?} {hi:?}");
        assert!((lo.y - 64.375).abs() < 1e-5 && (hi.y - 64.875).abs() < 1e-5, "{lo:?} {hi:?}");
    }
}
