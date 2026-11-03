//! Lula, morcego, golem de ferro, boneco de neve e bruxa.

use glam::{Mat4, Vec3};
use mc_render::Tex;

use super::{living, Ctx, DEG};
use crate::entities::Entity;
use crate::gen_mob_models as g;
use crate::mob_rig::Rig;
use crate::models::Anim;
use crate::mth::{cos, sin, PI};

/// `RenderSquid.rotateCorpse` (sem a inclinação da morte) + escala -1 e o -1.5078125.
fn squid_matrix(pos: Vec3, body: f32, e: &Entity, t: f32) -> Mat4 {
    let s = &e.mob_state;
    let pitch = s.squid_pitch_prev + (s.squid_pitch - s.squid_pitch_prev) * t;
    let yaw = s.squid_yaw_prev + (s.squid_yaw - s.squid_yaw_prev) * t;
    Mat4::from_translation(pos)
        * Mat4::from_translation(Vec3::new(0.0, 0.5, 0.0))
        * Mat4::from_rotation_y((180.0 - body).to_radians())
        * Mat4::from_rotation_x(pitch.to_radians())
        * Mat4::from_rotation_y(yaw.to_radians())
        * Mat4::from_translation(Vec3::new(0.0, -1.2, 0.0))
        * Mat4::from_scale(Vec3::new(-1.0, -1.0, 1.0))
        * Mat4::from_translation(Vec3::new(0.0, -1.5078125, 0.0))
}

/// `ModelSquid.setRotationAngles`: todos os tentáculos giram com o ângulo de `handleRotationFloat`.
pub fn pose_squid(rig: &mut Rig, tentacle: f32) {
    for i in 0..8 {
        let p = rig.i(&format!("squidTentacles_{i}"));
        rig.rot[p].x = tentacle;
    }
}

pub fn squid(e: &Entity, pos: Vec3, t: f32, _anim: &Anim, body: f32, _death: f32, cx: &mut Ctx) -> bool {
    let Some(tex) = cx.textures.jar(cx.r, cx.jar, "entity/squid") else { return true };
    let s = &e.mob_state;
    let m = squid_matrix(pos, body, e, t);
    let mut rig = Rig::new(&g::SQUID);
    pose_squid(&mut rig, s.tentacle_prev + (s.tentacle - s.tentacle_prev) * t);
    let roots = rig.roots();
    let (tint, light) = (cx.tint, cx.light);
    rig.emit(&mut cx.sink(tex, tint, 255, light), &m, &roots);
    true
}

/// `LayerHeldItemWitch`: o item preso no nariz (`villagerNose.postRender`, que só aplica o ponto de giro e o
/// giro do próprio nariz) com os ajustes de baú, arco, item 3D e item plano, e o `renderItem(THIRD_PERSON)`.
fn witch_held(rig: &Rig, m: &Mat4, s: &crate::entity_net::Slot, cx: &mut Ctx) {
    use crate::camera::{rot, tr};
    use crate::gen_item_flags::{FULL_3D, ROTATE_AROUND};
    let nose = rig.i("villagerNose");
    let mut p = crate::models::Part::new(rig.pivot[nose].to_array());
    p.rot = rig.rot[nose];
    let mut mm = p.matrix(m) * tr(-0.0625, 0.53125, 0.21875);
    let id = s.id;
    if (0..256).contains(&id) && crate::block_physics::RENDER_TYPE_2.contains(&(id as u16)) {
        mm *= tr(0.0, 0.0625, -0.25) * rot(30.0, 1.0, 0.0, 0.0) * rot(-5.0, 0.0, 1.0, 0.0) * Mat4::from_scale(Vec3::new(0.375, -0.375, 0.375));
    } else if id == 261 {
        mm *= tr(0.0, 0.125, -0.125) * rot(-45.0, 0.0, 1.0, 0.0) * Mat4::from_scale(Vec3::new(0.625, -0.625, 0.625)) * rot(-100.0, 1.0, 0.0, 0.0) * rot(-20.0, 0.0, 1.0, 0.0);
    } else if FULL_3D.contains(&id) {
        if ROTATE_AROUND.contains(&id) {
            mm *= rot(180.0, 0.0, 0.0, 1.0) * tr(0.0, -0.0625, 0.0);
        }
        // RenderWitch.transformHeldFull3DItemLayer.
        mm *= tr(0.0, 0.1875, 0.0) * tr(0.0625, -0.125, 0.0) * Mat4::from_scale(Vec3::new(0.625, -0.625, 0.625));
    } else {
        mm *= tr(0.1875, 0.1875, 0.0) * Mat4::from_scale(Vec3::splat(0.875)) * rot(-20.0, 0.0, 0.0, 1.0) * rot(-60.0, 1.0, 0.0, 0.0) * rot(-30.0, 0.0, 0.0, 1.0);
    }
    mm *= rot(-15.0, 1.0, 0.0, 0.0) * rot(40.0, 0.0, 0.0, 1.0);
    let Some(model) = cx.items.get(id, s.damage, cx.jar, cx.table, cx.r) else { return };
    if model.entity {
        let Some(item) = crate::entities::Worn::from_slot(s).entity.and_then(crate::tile_entities::entity_from_variant) else { return };
        let Some(tex) = crate::tile_entities::item_texture(&item, cx.textures, cx.r, cx.jar) else { return };
        model.emit_entity(&item, mm, true, &cx.lights, cx.light, cx.out.entry(Tex::Custom(tex)).or_default());
        return;
    }
    model.emit(mm, true, &cx.lights, cx.light, cx.out.entry(model.tex).or_default());
}

/// `ModelBat.setRotationAngles`.
pub fn pose_bat(rig: &mut Rig, hanging: bool, a: &Anim) {
    let (head, bd) = (rig.i("batHead"), rig.i("batBody"));
    let (rw, orw, lw, olw) = (rig.i("batRightWing"), rig.i("batOuterRightWing"), rig.i("batLeftWing"), rig.i("batOuterLeftWing"));
    if hanging {
        rig.rot[head].x = a.head_pitch / DEG;
        rig.rot[head].y = PI - a.head_yaw / DEG;
        rig.rot[head].z = PI;
        rig.pivot[head] = Vec3::new(0.0, -2.0, 0.0);
        rig.pivot[rw] = Vec3::new(-3.0, 0.0, 3.0);
        rig.pivot[lw] = Vec3::new(3.0, 0.0, 3.0);
        rig.rot[bd].x = PI;
        rig.rot[rw].x = -0.15707964;
        rig.rot[rw].y = -1.2566371;
        rig.rot[orw].y = -1.7278761;
        rig.rot[lw].x = rig.rot[rw].x;
        rig.rot[lw].y = -rig.rot[rw].y;
        rig.rot[olw].y = -rig.rot[orw].y;
    } else {
        rig.rot[head].x = a.head_pitch / DEG;
        rig.rot[head].y = a.head_yaw / DEG;
        rig.rot[head].z = 0.0;
        rig.pivot[head] = Vec3::ZERO;
        rig.pivot[rw] = Vec3::ZERO;
        rig.pivot[lw] = Vec3::ZERO;
        rig.rot[bd].x = 0.7853982 + cos(a.age * 0.1) * 0.15;
        rig.rot[bd].y = 0.0;
        rig.rot[rw].y = cos(a.age * 1.3) * PI * 0.25;
        rig.rot[lw].y = -rig.rot[rw].y;
        rig.rot[orw].y = rig.rot[rw].y * 0.5;
        rig.rot[olw].y = -rig.rot[rw].y * 0.5;
    }
}

/// `RenderBat`: escala 0.35 e o balanço vertical de `rotateCorpse`.
pub fn bat(e: &Entity, pos: Vec3, _t: f32, anim: &Anim, body: f32, death: f32, cx: &mut Ctx) -> bool {
    let Some(tex) = cx.textures.jar(cx.r, cx.jar, "entity/bat") else { return true };
    let hanging = e.meta_byte(16) & 1 != 0;
    let dy = if hanging { -0.1 } else { cos(anim.age * 0.3) * 0.1 };
    let m = living(pos + Vec3::new(0.0, dy, 0.0), body, death, Vec3::splat(0.35));
    let mut rig = Rig::new(&g::BAT);
    pose_bat(&mut rig, hanging, anim);
    let roots = [rig.i("batHead"), rig.i("batBody")];
    let (tint, light) = (cx.tint, cx.light);
    rig.emit(&mut cx.sink(tex, tint, 255, light), &m, &roots);
    true
}

/// `ModelIronGolem.func_78172_a`: onda triangular.
fn tri(x: f32, p: f32) -> f32 {
    ((x % p - p * 0.5).abs() - p * 0.25) / (p * 0.25)
}

/// `ModelIronGolem.setLivingAnimations` + `setRotationAngles`.
pub fn pose_golem(rig: &mut Rig, e: &Entity, t: f32, a: &Anim) {
    let (head, ll, rl, ra, la) = (rig.i("ironGolemHead"), rig.i("ironGolemLeftLeg"), rig.i("ironGolemRightLeg"), rig.i("ironGolemRightArm"), rig.i("ironGolemLeftArm"));
    let (f, f1) = (a.limb_swing, a.limb_amount);
    let s = &e.mob_state;
    if s.golem_attack > 0 {
        rig.rot[ra].x = -2.0 + 1.5 * tri(s.golem_attack as f32 - t, 10.0);
        rig.rot[la].x = -2.0 + 1.5 * tri(s.golem_attack as f32 - t, 10.0);
    } else if s.golem_rose > 0 {
        rig.rot[ra].x = -0.8 + 0.025 * tri(s.golem_rose as f32, 70.0);
        rig.rot[la].x = 0.0;
    } else {
        rig.rot[ra].x = (-0.2 + 1.5 * tri(f, 13.0)) * f1;
        rig.rot[la].x = (-0.2 - 1.5 * tri(f, 13.0)) * f1;
    }
    rig.rot[head].y = a.head_yaw / DEG;
    rig.rot[head].x = a.head_pitch / DEG;
    rig.rot[ll].x = -1.5 * tri(f, 13.0) * f1;
    rig.rot[rl].x = 1.5 * tri(f, 13.0) * f1;
    rig.rot[ll].y = 0.0;
    rig.rot[rl].y = 0.0;
}

/// `RenderIronGolem`: balanço lateral ao andar.
pub fn golem(e: &Entity, pos: Vec3, t: f32, anim: &Anim, body: f32, death: f32, cx: &mut Ctx) -> bool {
    let Some(tex) = cx.textures.jar(cx.r, cx.jar, "entity/iron_golem") else { return true };
    let mut tilt = 0.0;
    if e.limb_amount as f64 >= 0.01 {
        let p = 13.0;
        let ls = e.limb_swing - e.limb_amount * (1.0 - t) + 6.0;
        tilt = 6.5 * ((ls % p - p * 0.5).abs() - p * 0.25) / (p * 0.25);
    }
    let m = living(pos, body, death + tilt, Vec3::ONE);
    let mut rig = Rig::new(&g::IRON_GOLEM);
    pose_golem(&mut rig, e, t, anim);
    let roots = rig.roots();
    let (tint, light) = (cx.tint, cx.light);
    rig.emit(&mut cx.sink(tex, tint, 255, light), &m, &roots);
    if e.mob_state.golem_rose != 0 {
        // LayerIronGolemFlower: a papoula (vermelha, id 38) na mão direita.
        use crate::camera::{rot, tr};
        let fm = m * rot(5.0 + 180.0 * rig.rot[rig.i("ironGolemRightArm")].x / PI, 1.0, 0.0, 0.0) * rot(90.0, 1.0, 0.0, 0.0) * tr(-0.9375, -0.625, -0.9375) * Mat4::from_scale(Vec3::new(0.5, -0.5, 0.5));
        super::emit_block(cx, 38 << 4, &fm, light);
    }
    true
}

/// `ModelSnowMan.setRotationAngles`.
pub fn pose_snowman(rig: &mut Rig, a: &Anim) {
    let (head, bd, rh, lh) = (rig.i("head"), rig.i("body"), rig.i("rightHand"), rig.i("leftHand"));
    rig.rot[head].y = a.head_yaw / DEG;
    rig.rot[head].x = a.head_pitch / DEG;
    rig.rot[bd].y = a.head_yaw / DEG * 0.25;
    let (s, c) = (sin(rig.rot[bd].y), cos(rig.rot[bd].y));
    rig.rot[rh].z = 1.0;
    rig.rot[lh].z = -1.0;
    rig.rot[rh].y = 0.0 + rig.rot[bd].y;
    rig.rot[lh].y = PI + rig.rot[bd].y;
    rig.pivot[rh].x = c * 5.0;
    rig.pivot[rh].z = -s * 5.0;
    rig.pivot[lh].x = -c * 5.0;
    rig.pivot[lh].z = s * 5.0;
}

pub fn snowman(e: &Entity, pos: Vec3, _t: f32, anim: &Anim, body: f32, death: f32, cx: &mut Ctx) -> bool {
    let Some(tex) = cx.textures.jar(cx.r, cx.jar, "entity/snowman") else { return true };
    let m = living(pos, body, death, Vec3::ONE);
    let mut rig = Rig::new(&g::SNOWMAN);
    pose_snowman(&mut rig, anim);
    let roots = rig.roots();
    let (tint, light) = (cx.tint, cx.light);
    rig.emit(&mut cx.sink(tex, tint, 255, light), &m, &roots);
    if !e.invisible() {
        // LayerSnowmanHead: a abóbora (id 86) com a transformação HEAD do item (identidade) e o -0.5 do item.
        use crate::camera::{rot, tr};
        let head = rig.part(rig.i("head")).matrix(&m);
        let hm = head * tr(0.0, -0.34375, 0.0) * rot(180.0, 0.0, 1.0, 0.0) * Mat4::from_scale(Vec3::new(0.625, -0.625, -0.625)) * tr(-0.5, -0.5, -0.5);
        super::emit_block(cx, 86 << 4, &hm, light);
    }
    true
}

/// `ModelWitch.setRotationAngles` sobre o `ModelVillager` (o nariz balança pelo id e pelo tempo de vida).
pub fn pose_witch(rig: &mut Rig, e: &Entity, holding: bool, a: &Anim) {
    let (head, nose, arms, rl, ll) = (rig.i("villagerHead"), rig.i("villagerNose"), rig.i("villagerArms"), rig.i("rightVillagerLeg"), rig.i("leftVillagerLeg"));
    rig.rot[head].y = a.head_yaw / DEG;
    rig.rot[head].x = a.head_pitch / DEG;
    rig.pivot[arms].y = 3.0;
    rig.pivot[arms].z = -1.0;
    rig.rot[arms].x = -0.75;
    rig.rot[rl].x = cos(a.limb_swing * 0.6662) * 1.4 * a.limb_amount * 0.5;
    rig.rot[ll].x = cos(a.limb_swing * 0.6662 + PI) * 1.4 * a.limb_amount * 0.5;
    rig.rot[rl].y = 0.0;
    rig.rot[ll].y = 0.0;
    let f = 0.01 * (e.id % 10) as f32;
    let ticks = e.age as f32;
    rig.rot[nose].x = sin(ticks * f) * 4.5 * PI / 180.0;
    rig.rot[nose].y = 0.0;
    rig.rot[nose].z = cos(ticks * f) * 2.5 * PI / 180.0;
    if holding {
        // offsetY/Z do ModelRenderer vão em blocos: somam 16 vezes ao ponto de giro.
        rig.rot[nose].x = -0.9;
        rig.pivot[nose].z += -0.09375 * 16.0;
        rig.pivot[nose].y += 0.1875 * 16.0;
    }
}

/// `RenderWitch`: escala 0.9375 e o nariz erguido quando segura algo.
pub fn witch(e: &Entity, pos: Vec3, _t: f32, anim: &Anim, body: f32, death: f32, cx: &mut Ctx) -> bool {
    let Some(tex) = cx.textures.jar(cx.r, cx.jar, "entity/witch") else { return true };
    let m = living(pos, body, death, Vec3::splat(0.9375));
    let mut rig = Rig::new(&g::WITCH);
    pose_witch(&mut rig, e, e.equipment[0].is_some(), anim);
    let roots = rig.roots();
    let (tint, light) = (cx.tint, cx.light);
    rig.emit(&mut cx.sink(tex, tint, 255, light), &m, &roots);
    if let Some(s) = &e.equipment[0] {
        witch_held(&rig, &m, s, cx);
    }
    true
}
