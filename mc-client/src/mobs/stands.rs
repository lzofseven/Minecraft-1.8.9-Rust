//! Armor stand (`ArmorStandRenderer` + `ModelArmorStand`) e cristal do End (`RenderEnderCrystal`).

use glam::{Mat4, Vec3};

use super::Ctx;
use crate::entities::Entity;
use crate::entity_net::MetaValue;
use crate::gen_mob_models as g;
use crate::mob_rig::Rig;
use crate::models::Anim;
use crate::mth::sin;

const RAD: f32 = 0.017453292;

/// `Rotations` do metadado (x, y, z em graus) ou o padrão do `EntityArmorStand`.
fn rotation(e: &Entity, index: u8, default: [f32; 3]) -> Vec3 {
    match e.meta.get(&index) {
        Some(MetaValue::Rot(x, y, z)) => Vec3::new(*x, *y, *z),
        _ => Vec3::from(default),
    }
}

/// `ModelArmorStandArmor.setRotationAngles` + `ModelArmorStand.setRotationAngles`.
pub fn pose_stand(rig: &mut Rig, e: &Entity) {
    let flags = e.meta_byte(10);
    let set = |rig: &mut Rig, name: &str, r: Vec3| {
        let i = rig.i(name);
        rig.rot[i] = r * RAD;
    };
    set(rig, "bipedHead", rotation(e, 11, [0.0; 3]));
    let head = rig.i("bipedHead");
    rig.pivot[head] = Vec3::new(0.0, 1.0, 0.0);
    let body = rotation(e, 12, [0.0; 3]);
    set(rig, "bipedBody", body);
    set(rig, "bipedLeftArm", rotation(e, 13, [-10.0, 0.0, -10.0]));
    set(rig, "bipedRightArm", rotation(e, 14, [-15.0, 0.0, 10.0]));
    set(rig, "bipedLeftLeg", rotation(e, 15, [-1.0, 0.0, -1.0]));
    set(rig, "bipedRightLeg", rotation(e, 16, [1.0, 0.0, 1.0]));
    let (ll, rl, hw) = (rig.i("bipedLeftLeg"), rig.i("bipedRightLeg"), rig.i("bipedHeadwear"));
    rig.pivot[ll] = Vec3::new(1.9, 11.0, 0.0);
    rig.pivot[rl] = Vec3::new(-1.9, 11.0, 0.0);
    rig.rot[hw] = rig.rot[head];
    rig.pivot[hw] = rig.pivot[head];
    // ModelArmorStand: braços só com `ShowArms` (0x04), a base some com `NoBasePlate` (0x08).
    let (la, ra, sb) = (rig.i("bipedLeftArm"), rig.i("bipedRightArm"), rig.i("standBase"));
    rig.show[la] = flags & 0x04 != 0;
    rig.show[ra] = flags & 0x04 != 0;
    rig.show[sb] = flags & 0x08 == 0;
    rig.pivot[ll] = Vec3::new(1.9, 12.0, 0.0);
    rig.pivot[rl] = Vec3::new(-1.9, 12.0, 0.0);
    for n in ["standRightSide", "standLeftSide", "standWaist"] {
        set(rig, n, body);
    }
    rig.rot[sb] = Vec3::new(0.0, RAD * -e.yaw, 0.0);
}

/// Armor stand: a matriz de `RendererLivingEntity` com `rotateCorpse` só de giro (sem a inclinação da morte).
pub fn armor_stand(e: &Entity, pos: Vec3, body: f32, cx: &mut Ctx) -> bool {
    let Some(tex) = cx.textures.jar(cx.r, cx.jar, "entity/armorstand/wood") else { return true };
    let m = Mat4::from_translation(pos) * Mat4::from_rotation_y((180.0 - body).to_radians()) * Mat4::from_scale(Vec3::new(-1.0, -1.0, 1.0)) * Mat4::from_translation(Vec3::new(0.0, -1.5078125, 0.0));
    let mut rig = Rig::new(&g::ARMOR_STAND);
    pose_stand(&mut rig, e);
    let small = e.meta_byte(10) & 0x01 != 0;
    let sneak = e.sneaking();
    let (tint, light) = (cx.tint, cx.light);
    let mut sink = cx.sink(tex, tint, 255, light);
    let tr = |y: f32| Mat4::from_translation(Vec3::new(0.0, y, 0.0));
    let stand = ["standRightSide", "standLeftSide", "standWaist", "standBase"].map(|n| rig.i(n));
    let (head, hw) = (rig.i("bipedHead"), rig.i("bipedHeadwear"));
    let rest = ["bipedBody", "bipedRightArm", "bipedLeftArm", "bipedRightLeg", "bipedLeftLeg"].map(|n| rig.i(n));
    if small {
        // ModelBiped.render do filhote e depois as peças do suporte pela metade.
        rig.emit(&mut sink, &(m * Mat4::from_scale(Vec3::splat(0.75)) * tr(1.0)), &[head]);
        let half = m * Mat4::from_scale(Vec3::splat(0.5)) * tr(1.5);
        rig.emit(&mut sink, &half, &rest);
        rig.emit(&mut sink, &half, &[hw]);
        rig.emit(&mut sink, &half, &stand);
    } else {
        let mm = if sneak { m * tr(0.2) } else { m };
        rig.emit(&mut sink, &mm, &[head]);
        rig.emit(&mut sink, &mm, &rest);
        rig.emit(&mut sink, &mm, &[hw]);
        rig.emit(&mut sink, &mm, &stand);
    }
    // ArmorStandRenderer: LayerBipedArmor (ModelArmorStandArmor), LayerHeldItem e LayerCustomHead.
    let repose = |b: &mut crate::models::Biped| pose_armor(b, e);
    let mut main = crate::models::Biped::armor(0);
    repose(&mut main);
    let worn = |i: usize| e.equipment.get(i).and_then(Option::as_ref).map(crate::entities::Worn::from_slot);
    let anim = Anim { sneak, child: small, held_right: e.equipment[0].is_some() as i32, ..Default::default() };
    crate::entities::biped_layers(&main, false, &anim, &m, worn(0), [worn(1), worn(2), worn(3), worn(4)], light, tint, cx.lights, false, Some(&repose), cx.items, cx.table, cx.textures, cx.r, cx.jar, cx.out);
    true
}

/// `ModelArmorStandArmor.setRotationAngles`: cabeça, corpo, braços e pernas com os ângulos do metadado.
fn pose_armor(b: &mut crate::models::Biped, e: &Entity) {
    b.head.rot = rotation(e, 11, [0.0; 3]) * RAD;
    b.head.pivot = Vec3::new(0.0, 1.0, 0.0);
    b.body.rot = rotation(e, 12, [0.0; 3]) * RAD;
    b.left_arm.rot = rotation(e, 13, [-10.0, 0.0, -10.0]) * RAD;
    b.right_arm.rot = rotation(e, 14, [-15.0, 0.0, 10.0]) * RAD;
    b.left_leg.rot = rotation(e, 15, [-1.0, 0.0, -1.0]) * RAD;
    b.left_leg.pivot = Vec3::new(1.9, 11.0, 0.0);
    b.right_leg.rot = rotation(e, 16, [1.0, 0.0, 1.0]) * RAD;
    b.right_leg.pivot = Vec3::new(-1.9, 11.0, 0.0);
    b.hat.rot = b.head.rot;
    b.hat.pivot = b.head.pivot;
}

/// `RenderEnderCrystal` com `ModelEnderCrystal(0.0F, true)`.
pub fn crystal(e: &Entity, pos: Vec3, t: f32, cx: &mut Ctx) -> bool {
    let Some(tex) = cx.textures.jar(cx.r, cx.jar, "entity/endercrystal/endercrystal") else { return true };
    // `innerRotation` começa num `nextInt(100000)` por entidade.
    let inner = (e.id as u32).wrapping_mul(2654435761) % 100000;
    let f = (inner as f32 + e.age as f32) + t;
    let bob = {
        let b = sin(f * 0.2) / 2.0 + 0.5;
        (b * b + b) * 0.2
    };
    let rig = Rig::new(&g::ENDER_CRYSTAL);
    let (tint, light) = (cx.tint, cx.light);
    let mut sink = cx.sink(tex, tint, 255, light);
    use crate::camera::rot;
    let axis = |deg: f32| Mat4::from_axis_angle(Vec3::new(0.7071, 0.0, 0.7071).normalize(), deg.to_radians());
    let _ = rot;
    let spin = f * 3.0;
    let mut m = Mat4::from_translation(pos) * Mat4::from_scale(Vec3::splat(2.0)) * Mat4::from_translation(Vec3::new(0.0, -0.5, 0.0));
    rig.emit(&mut sink, &m, &[rig.i("base")]);
    m *= Mat4::from_rotation_y(spin.to_radians()) * Mat4::from_translation(Vec3::new(0.0, 0.8 + bob, 0.0)) * axis(60.0);
    rig.emit(&mut sink, &m, &[rig.i("glass")]);
    m *= Mat4::from_scale(Vec3::splat(0.875)) * axis(60.0) * Mat4::from_rotation_y(spin.to_radians());
    rig.emit(&mut sink, &m, &[rig.i("glass")]);
    m *= Mat4::from_scale(Vec3::splat(0.875)) * axis(60.0) * Mat4::from_rotation_y(spin.to_radians());
    rig.emit(&mut sink, &m, &[rig.i("cube")]);
    true
}

#[allow(dead_code)]
fn _anim(_: &Anim) {}
