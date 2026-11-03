//! Efeitos de renderizador comuns a todas as entidades: tinta de dano (`RendererLivingEntity.setBrightness`),
//! piscar do creeper (`RenderCreeper.getColorMultiplier`), sombra (`Render.renderShadow`) e fogo
//! (`Render.renderEntityOnFire`).
//!
//! A tinta do Java é uma mistura `lerp(textura x cor do vértice, cor, a)` antes do mapa de luz (combinadores
//! de textura). O shader faz a mesma conta lendo o byte `light[3]` do vértice: bit 7 = branco (senão vermelho)
//! e os 7 bits baixos = `a * 127`.

use std::collections::HashMap;

use glam::{DVec3, Mat4, Vec3};
use mc_render::Tex;
use mc_world::blocks::BlockTable;
use mc_world::mesh::{atlas_uv, Geometry, Vertex};
use mc_world::World;

use crate::entities::{Entity, Kind};
use crate::gen_shadows as sh;
use crate::mobs::TRANSLUCENT;

/// Byte de `light[3]`: mistura `a` (0..1) com vermelho (dano) ou branco (creeper).
pub fn overlay_code(white: bool, a: f32) -> u8 {
    ((a.clamp(0.0, 1.0) * 127.0).round() as u8) | if white { 0x80 } else { 0 }
}

/// Inverso do código, como o shader o lê: (branco, a).
pub fn overlay_decode(b: u8) -> (bool, f32) {
    (b >= 128, (b & 0x7F) as f32 / 127.0)
}

/// `RenderCreeper.getColorMultiplier`: a mistura com branco (`1 - alfa`) nas décimas ímpares do piscar.
pub fn creeper_flash(swell_prev: i32, swell: i32, t: f32) -> Option<f32> {
    let f = (swell_prev as f32 + (swell - swell_prev) as f32 * t) / 28.0;
    if (f * 10.0) as i32 % 2 == 0 {
        return None;
    }
    let i = ((f * 0.2 * 255.0) as i32).clamp(0, 255);
    Some(1.0 - i as f32 / 255.0)
}

/// Código da tinta de um mob/jogador neste quadro, ou `None`. Dano (`hurtTime`/`deathTime`) tem prioridade.
pub fn tint_code(e: &Entity, t: f32) -> Option<(u8, bool)> {
    let flash = match e.kind {
        Kind::Mob(50) => creeper_flash(e.mob_state.swell_prev, e.mob_state.swell, t),
        _ => None,
    };
    if e.hurt_time > 0 || e.death_time > 0 {
        Some((overlay_code(false, 0.3), false))
    } else {
        flash.map(|a| (overlay_code(true, a), true))
    }
}

/// Aplica a tinta aos vértices acrescentados desde `before`. Camadas sem `shouldCombineTextures`
/// (marcadas como translúcidas pelos mobs) só recebem o piscar do creeper, não o vermelho do dano.
pub fn apply_tint(out: &mut HashMap<Tex, Geometry>, before: &HashMap<Tex, usize>, code: u8, all_layers: bool) {
    for (tex, geo) in out.iter_mut() {
        if !all_layers && matches!(tex, Tex::Custom(id) if id & TRANSLUCENT != 0) {
            continue;
        }
        let from = before.get(tex).copied().unwrap_or(0);
        for v in &mut geo.vertices[from..] {
            v.light[3] = code;
        }
    }
}

/// Tamanhos dos vértices por textura (para saber o que um desenho acrescentou).
pub fn snapshot(out: &HashMap<Tex, Geometry>) -> HashMap<Tex, usize> {
    out.iter().map(|(k, g)| (*k, g.vertices.len())).collect()
}

/// `Render.shadowSize` e `shadowOpaque` da entidade (tabelas geradas dos construtores dos `Render*`).
pub fn shadow_of(e: &Entity) -> (f32, f32) {
    let (mut size, opaque) = match e.kind {
        Kind::Player { .. } => sh::PLAYER_SHADOW,
        Kind::Mob(id) => sh::MOB_SHADOW.iter().find(|r| r.0 == id).map_or((0.0, 1.0), |r| (r.2, r.3)),
        Kind::Object { kind, .. } if kind == crate::naovivas::XP_ORB => sh::XP_ORB_SHADOW,
        Kind::Object { kind, .. } => sh::OBJECT_SHADOW.iter().find(|r| r.0 == kind).map_or((0.0, 1.0), |r| (r.2, r.3)),
    };
    // doRender troca o tamanho: lesma = 0,25 x tamanho; aldeão filhote começa em 0,25 (RenderVillager).
    if matches!(e.kind, Kind::Mob(55)) {
        size = 0.25 * e.meta_byte(16).max(1) as f32;
    }
    if matches!(e.kind, Kind::Mob(120)) && e.child() {
        size = 0.25;
    }
    // Render.renderShadow: EntityLiving filhote reduz à metade (getRenderSizeModifier = 1).
    if matches!(e.kind, Kind::Mob(_)) && (e.child() || (90..=101).contains(&mob_id(e)) && e.meta_byte(12) < 0) {
        size *= 0.5;
    }
    (size, opaque)
}

fn mob_id(e: &Entity) -> u8 {
    match e.kind {
        Kind::Mob(id) => id,
        _ => 0,
    }
}

/// Opacidade base da sombra: `(1 - distância ao quadrado / 256) * shadowOpaque`.
pub fn shadow_strength(dist2: f64, opaque: f32) -> f32 {
    ((1.0 - dist2 / 256.0) * opaque as f64) as f32
}

/// Nível de luz (`getLightFromNeighbors` sem a regra dos vizinhos) de uma célula: céu menos o escurecimento do dia, ou bloco.
fn light_level(world: &World, x: i32, y: i32, z: i32, time: i64) -> i32 {
    let l = cell_light(world, x, y, z);
    ((l & 15) as i32).max((l >> 4) as i32 - crate::hud_stats::skylight_subtracted(time))
}

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

/// `Render.renderShadow`: um quad por bloco inteiro abaixo da entidade, preto com alfa pela altura e luz.
#[allow(clippy::too_many_arguments)]
pub fn shadow(geo: &mut Geometry, world: &World, table: &BlockTable, pos: DVec3, size: f32, strength: f32, time: i64, light: [u8; 4]) {
    let s = size as f64;
    let (x0, x1) = ((pos.x - s).floor() as i32, (pos.x + s).floor() as i32);
    let (y0, y1) = ((pos.y - s).floor() as i32, pos.y.floor() as i32);
    let (z0, z1) = ((pos.z - s).floor() as i32, (pos.z + s).floor() as i32);
    for y in y0..=y1 {
        for z in z0..=z1 {
            for x in x0..=x1 {
                let below = world.block(x, y - 1, z);
                if below == 0 || light_level(world, x, y, z, time) <= 3 || !table.full_cube(below) {
                    continue;
                }
                let bright = crate::hud_stats::light_brightness(light_level(world, x, y, z, time)) as f64;
                let a = (strength as f64 - (pos.y - y as f64) / 2.0) * 0.5 * bright;
                if a < 0.0 {
                    continue;
                }
                let a = a.min(1.0);
                let (xa, xb) = (x as f64, x as f64 + 1.0);
                let (za, zb) = (z as f64, z as f64 + 1.0);
                let yy = y as f64 + 0.015625;
                let u = |px: f64| ((pos.x - px) / 2.0 / s + 0.5) as f32;
                let v = |pz: f64| ((pos.z - pz) / 2.0 / s + 0.5) as f32;
                let color = [255, 255, 255, (a * 255.0) as u8];
                let vert = |px: f64, pz: f64| Vertex { pos: [px as f32, yy as f32, pz as f32], uv: [u(px), v(pz)], color, light };
                geo.quad([vert(xa, za), vert(xa, zb), vert(xb, zb), vert(xb, za)]);
            }
        }
    }
}

/// `Render.renderEntityOnFire`: camadas de chama (`fire_layer_0/1` do atlas), viradas para a câmera em Y.
pub fn fire(geo: &mut Geometry, table: &BlockTable, pos: Vec3, width: f32, height: f32, view_yaw: f32) {
    let tile = |n: &str| table.textures.iter().position(|t| t == n).map(|i| i as u16);
    let (Some(t0), Some(t1)) = (tile("fire_layer_0"), tile("fire_layer_1")) else { return };
    let s = width * 1.4;
    let mut f17 = height / s;
    let mut m = Mat4::from_translation(pos) * Mat4::from_scale(Vec3::splat(s)) * Mat4::from_rotation_y((-view_yaw).to_radians());
    m *= Mat4::from_translation(Vec3::new(0.0, 0.0, -0.3 + (f17 as i32) as f32 * 0.02));
    let (mut x, mut y0, mut z, mut idx) = (0.5f32, 0.0f32, 0.0f32, 0);
    while f17 > 0.0 {
        let tile = if idx % 2 == 0 { t0 } else { t1 };
        let (mut ua, mut ub) = (0.0f32, 16.0f32);
        if idx / 2 % 2 == 0 {
            std::mem::swap(&mut ua, &mut ub);
        }
        let c = |p: [f32; 3], u: f32, v: f32| Vertex { pos: m.transform_point3(Vec3::from(p)).to_array(), uv: atlas_uv(tile, u, v), color: [255; 4], light: crate::models::FULL_LIGHT };
        geo.quad([c([x, -y0, z], ub, 16.0), c([-x, -y0, z], ua, 16.0), c([-x, 1.4 - y0, z], ua, 0.0), c([x, 1.4 - y0, z], ub, 0.0)]);
        f17 -= 0.45;
        y0 -= 0.45;
        x *= 0.9;
        z += 0.03;
        idx += 1;
    }
}

/// `ItemRenderer.renderFireInFirstPerson`: duas camadas de `fire_layer_1` no espaço da câmera, sem o balanço
/// da caminhada, com alfa 0,9 e sem luz (o lightmap já foi desligado).
pub fn first_person_fire(table: &BlockTable) -> Option<Geometry> {
    let t = table.textures.iter().position(|t| t == "fire_layer_1")? as u16;
    let mut geo = Geometry::default();
    let color = [255, 255, 255, (0.9f32 * 255.0) as u8];
    for i in 0..2 {
        let s = (i * 2 - 1) as f32;
        let m = Mat4::from_translation(Vec3::new(-s * 0.24, -0.3, 0.0)) * Mat4::from_rotation_y((s * 10.0).to_radians());
        let c = |x: f32, y: f32, u: f32, v: f32| Vertex { pos: m.transform_point3(Vec3::new(x, y, -0.5)).to_array(), uv: atlas_uv(t, u, v), color, light: crate::models::FULL_LIGHT };
        geo.quad([c(-0.5, -0.5, 16.0, 16.0), c(0.5, -0.5, 0.0, 16.0), c(0.5, 0.5, 0.0, 0.0), c(-0.5, 0.5, 16.0, 0.0)]);
    }
    Some(geo)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_code_round_trips() {
        let (white, a) = overlay_decode(overlay_code(false, 0.3));
        assert!(!white && (a - 0.3).abs() < 0.01);
        let (white, a) = overlay_decode(overlay_code(true, 0.85));
        assert!(white && (a - 0.85).abs() < 0.01);
        assert_eq!(overlay_code(false, 0.0), 0);
    }

    #[test]
    fn creeper_flashes_on_odd_tenths_only() {
        // f = swell / 28: 3/28 ~ 0.107 -> (int)(1.07) = 1 (ímpar): pisca; 6/28 -> 2 (par): não.
        let a = creeper_flash(3, 3, 0.0).unwrap();
        assert!(a > 0.9 && a < 1.0, "{a}");
        assert!(creeper_flash(6, 6, 0.0).is_none());
        assert!(creeper_flash(0, 0, 0.0).is_none());
        // Perto de explodir (f ~ 1): (int)(10.7) = 10 par; 30/28 -> 10 par; 27/28 -> 9 ímpar, a = 1 - 51/255 = 0,8.
        assert!((creeper_flash(27, 27, 0.0).unwrap() - 0.8).abs() < 0.02);
    }

    #[test]
    fn sombra_de_item_parado_na_pedra() {
        let mut w = World::new(true);
        let mut sec = Box::new(mc_world::chunk::Section::empty(true));
        for z in 0..16 {
            for x in 0..16 {
                sec.blocks[mc_world::chunk::Section::index(x, 3, z)] = 1 << 4;
            }
        }
        w.apply(mc_world::ChunkData { x: 0, z: 0, full: true, mask: 1 << 6, sections: vec![(6, sec)], biomes: None });
        let t = BlockTable::new();
        let mut g = Geometry::default();
        shadow(&mut g, &w, &t, DVec3::new(5.5, 100.0, 5.5), 0.15, shadow_strength(25.0, 0.75), 6000, [255, 255, 0, 0]);
        assert!(!g.indices.is_empty(), "a sombra precisa de pelo menos um quad em cima da pedra");
        assert!(g.vertices.iter().all(|v| v.color[3] > 0));
    }

    #[test]
    fn shadow_strength_fades_with_distance() {
        assert_eq!(shadow_strength(0.0, 1.0), 1.0);
        assert!((shadow_strength(128.0, 1.0) - 0.5).abs() < 1e-6);
        assert!((shadow_strength(0.0, 0.75) - 0.75).abs() < 1e-6);
        assert!(shadow_strength(256.0, 1.0) <= 0.0);
    }

    #[test]
    fn shadow_sizes_come_from_the_java_renderers() {
        let pig = Entity::new(1, Kind::Mob(90), [0; 3], 0.0, 0.0);
        assert!((shadow_of(&pig).0 - 0.7).abs() < 1e-6);
        let item = Entity::new(2, Kind::Object { kind: 2, data: 0 }, [0; 3], 0.0, 0.0);
        assert_eq!(shadow_of(&item), (0.15000000596046448, 0.75));
        let arrow = Entity::new(3, Kind::Object { kind: 60, data: 0 }, [0; 3], 0.0, 0.0);
        assert_eq!(shadow_of(&arrow).0, 0.0);
        let mut slime = Entity::new(4, Kind::Mob(55), [0; 3], 0.0, 0.0);
        slime.meta.insert(16, crate::entity_net::MetaValue::Byte(4));
        assert_eq!(shadow_of(&slime).0, 1.0);
    }
}
