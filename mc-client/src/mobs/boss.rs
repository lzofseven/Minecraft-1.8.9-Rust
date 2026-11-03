//! Guardião (+ancião) e wither (+aura).

use glam::{Mat4, Vec3};

use super::{living, Ctx, ADDITIVE};
use crate::entities::Entity;
use crate::entity_net::MetaValue;
use crate::gen_mob_models as g;
use crate::mob_rig::Rig;
use crate::models::{Anim, Sink, FULL_LIGHT, WORLD_LIGHTS};
use crate::mth::{cos, sin, PI};
use mc_world::mesh::Geometry;

use super::DEG;

fn meta_int(e: &Entity, i: u8) -> i32 {
    match e.meta.get(&i) {
        Some(MetaValue::Int(v)) => *v,
        _ => 0,
    }
}

/// `ModelGuardian.setRotationAngles` (o olho que segue o alvo e o raio ficam pendentes).
pub fn pose_guardian(rig: &mut Rig, e: &Entity, t: f32, a: &Anim) {
    let body = rig.i("guardianBody");
    let age = a.age;
    rig.rot[body].y = a.head_yaw / DEG;
    rig.rot[body].x = a.head_pitch / DEG;
    let rx = [1.75, 0.25, 0.0, 0.0, 0.5, 0.5, 0.5, 0.5, 1.25, 0.75, 0.0, 0.0f32];
    let ry = [0.0, 0.0, 0.0, 0.0, 0.25, 1.75, 1.25, 0.75, 0.0, 0.0, 0.0, 0.0f32];
    let rz = [0.0, 0.0, 0.25, 1.75, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.75, 1.25f32];
    let px = [0.0, 0.0, 8.0, -8.0, -8.0, 8.0, 8.0, -8.0, 0.0, 0.0, 8.0, -8.0f32];
    let py = [-8.0, -8.0, -8.0, -8.0, 0.0, 0.0, 0.0, 0.0, 8.0, 8.0, 8.0, 8.0f32];
    let pz = [8.0, -8.0, 0.0, 0.0, -8.0, -8.0, 8.0, 8.0, 8.0, -8.0, 0.0, 0.0f32];
    let s = &e.mob_state;
    let spikes = s.guardian_spikes_prev + (s.guardian_spikes - s.guardian_spikes_prev) * t;
    let f = (1.0 - spikes) * 0.55;
    for i in 0..12 {
        let sp = rig.i(&format!("guardianSpines_{i}"));
        rig.rot[sp] = Vec3::new(PI * rx[i], PI * ry[i], PI * rz[i]);
        let w = 1.0 + cos(age * 1.5 + i as f32) * 0.01 - f;
        rig.pivot[sp] = Vec3::new(px[i] * w, 16.0 + py[i] * w, pz[i] * w);
    }
    let eye = rig.i("guardianEye");
    rig.pivot[eye].z = -8.25;
    rig.show[eye] = true;
    let tail = s.guardian_tail_prev + (s.guardian_tail - s.guardian_tail_prev) * t;
    let (t0, t1, t2) = (rig.i("guardianTail_0"), rig.i("guardianTail_1"), rig.i("guardianTail_2"));
    rig.rot[t0].y = sin(tail) * PI * 0.05;
    rig.rot[t1].y = sin(tail) * PI * 0.1;
    rig.pivot[t1] = Vec3::new(-1.5, 0.5, 14.0);
    rig.rot[t2].y = sin(tail) * PI * 0.15;
    rig.pivot[t2] = Vec3::new(0.5, 0.5, 6.0);
}

/// `RenderGuardian`: textura do ancião e escala 2.35.
pub fn guardian(e: &Entity, pos: Vec3, t: f32, anim: &Anim, body: f32, death: f32, cx: &mut Ctx) -> bool {
    let elder = meta_int(e, 16) & 4 != 0;
    let Some(tex) = cx.textures.jar(cx.r, cx.jar, if elder { "entity/guardian_elder" } else { "entity/guardian" }) else { return true };
    let m = living(pos, body, death, Vec3::splat(if elder { 2.35 } else { 1.0 }));
    let mut rig = Rig::new(&g::GUARDIAN);
    pose_guardian(&mut rig, e, t, anim);
    let (tint, light) = (cx.tint, cx.light);
    let root = [rig.i("guardianBody")];
    rig.emit(&mut cx.sink(tex, tint, 255, light), &m, &root);
    true
}

/// `ModelWither.setLivingAnimations` + `setRotationAngles`.
pub fn pose_wither(rig: &mut Rig, e: &Entity, a: &Anim) {
    let c = cos(a.age * 0.1);
    let (b1, b2) = (rig.i("field_82905_a_1"), rig.i("field_82905_a_2"));
    rig.rot[b1].x = (0.065 + 0.05 * c) * PI;
    rig.pivot[b2] = Vec3::new(-2.0, 6.9 + cos(rig.rot[b1].x) * 10.0, -0.5 + sin(rig.rot[b1].x) * 10.0);
    rig.rot[b2].x = (0.265 + 0.1 * c) * PI;
    let (h0, h1, h2) = (rig.i("field_82904_b_0"), rig.i("field_82904_b_1"), rig.i("field_82904_b_2"));
    for (h, j) in [(h1, 0), (h2, 1)] {
        rig.rot[h].y = (e.mob_state.wither_yaw[j] - e.body_yaw) / DEG;
        rig.rot[h].x = e.mob_state.wither_pitch[j] / DEG;
    }
    rig.rot[h0].y = a.head_yaw / DEG;
    rig.rot[h0].x = a.head_pitch / DEG;
}

/// `RenderWither` + `LayerWitherAura`.
pub fn wither(e: &Entity, pos: Vec3, t: f32, anim: &Anim, body: f32, death: f32, cx: &mut Ctx) -> bool {
    let invul = meta_int(e, 20);
    let invulnerable_tex = invul > 0 && (invul > 80 || invul / 5 % 2 != 1);
    let path = if invulnerable_tex { "entity/wither/wither_invulnerable" } else { "entity/wither/wither" };
    let Some(tex) = cx.textures.jar(cx.r, cx.jar, path) else { return true };
    let mut scale = 2.0;
    if invul > 0 {
        scale -= (invul as f32 - t) / 220.0 * 0.5;
    }
    let m = living(pos, body, death, Vec3::splat(scale));
    let mut rig = Rig::new(&g::WITHER);
    pose_wither(&mut rig, e, anim);
    let roots = rig.roots();
    let (tint, light) = (cx.tint, cx.light);
    rig.emit(&mut cx.sink(tex, tint, 255, light), &m, &roots);
    // isArmored: vida <= metade da máxima (300).
    let health = match e.meta.get(&6) {
        Some(MetaValue::Float(h)) => *h,
        _ => 300.0,
    };
    if health <= 150.0 && !e.invisible() {
        if let Some(armor) = cx.textures.jar(cx.r, cx.jar, "entity/wither/wither_armor") {
            let mut ar = Rig::new(&g::WITHER_ARMOR);
            ar.rot = rig.rot.clone();
            ar.pivot = rig.pivot.clone();
            let mut tmp = Geometry::default();
            let mut sink = Sink { geo: &mut tmp, tex: ar.tex(), tint: [0.5; 3], alpha: 255, light: FULL_LIGHT, lights: WORLD_LIGHTS };
            ar.emit(&mut sink, &m, &roots);
            let age = e.age as f32 + t;
            let (du, dv) = (cos(age * 0.02) * 3.0, age * 0.01);
            for v in &mut tmp.vertices {
                v.uv[0] += du;
                v.uv[1] += dv;
                v.color = [128, 128, 128, 255];
            }
            let dst = cx.out.entry(mc_render::Tex::Custom(armor | ADDITIVE)).or_default();
            let base = dst.vertices.len() as u32;
            dst.vertices.extend(tmp.vertices.iter().copied());
            dst.indices.extend(tmp.indices.iter().map(|i| i + base));
        }
    }
    let _ = Mat4::IDENTITY;
    true
}

/// `ModelDragon.updateRotations`: leva o ângulo para [-180, 180).
fn wrap(mut d: f64) -> f32 {
    while d >= 180.0 {
        d -= 360.0;
    }
    while d < -180.0 {
        d += 360.0;
    }
    d as f32
}

/// `ModelDragon.render`: coluna, cabeça, corpo, asas e pernas dos dois lados e a cauda, com o anel de
/// movimento de `EntityDragon.getMovementOffsets`. `m` é a matriz depois do `rotateCorpse`.
pub fn emit_dragon(rig: &mut Rig, e: &Entity, t: f32, m: &Mat4, sink: &mut Sink) {
    let s = &e.mob_state;
    let dead = matches!(e.meta.get(&6), Some(MetaValue::Float(h)) if *h <= 0.0);
    let off = |p: i32| s.dragon_offsets(p, t, dead);
    let anim = s.dragon_anim_prev + (s.dragon_anim - s.dragon_anim_prev) * t;
    let (jaw, spine, head, body) = (rig.i("jaw"), rig.i("spine"), rig.i("head"), rig.i("body"));
    rig.rot[jaw].x = (((anim * PI * 2.0) as f64).sin() + 1.0) as f32 * 0.2;
    let mut f10 = (((anim * PI * 2.0 - 1.0) as f64).sin() + 1.0) as f32;
    f10 = (f10 * f10 * 1.0 + f10 * 2.0) * 0.05;
    let m1 = *m * Mat4::from_translation(Vec3::new(0.0, (f10 - 2.0) / 1.0, -3.0)) * Mat4::from_rotation_x((f10 * 2.0).to_radians());
    let mut p = [0.0f32, 20.0, -12.0]; // x, y, z do ponto de giro acumulado
    let off6 = off(6);
    let f16 = wrap(off(5)[0] - off(10)[0]);
    let f17 = wrap(off(5)[0] + (f16 / 2.0) as f64);
    let f18 = anim * PI * 2.0;
    let k = 1.5f32;
    let pi = PI;
    let step = |rig: &mut Rig, sink: &mut Sink, p: &mut [f32; 3], ry: f32, rx: f32, rzv: f32| {
        rig.rot[spine] = Vec3::new(rx, ry, rzv);
        rig.pivot[spine] = Vec3::new(p[0], p[1], p[2]);
        p[1] = (p[1] as f64 + (rx as f64).sin() * 10.0) as f32;
        p[2] = (p[2] as f64 - (ry as f64).cos() * (rx as f64).cos() * 10.0) as f32;
        p[0] = (p[0] as f64 - (ry as f64).sin() * (rx as f64).cos() * 10.0) as f32;
        rig.emit(sink, &m1, &[spine]);
    };
    for i in 0..5 {
        let o = off(5 - i);
        let f21 = (((i as f32) * 0.45 + f18) as f64).cos() as f32 * 0.15;
        let ry = wrap(o[0] - off6[0]) * pi / 180.0 * k;
        let rx = f21 + (o[1] - off6[1]) as f32 * pi / 180.0 * k * 5.0;
        let rz = -wrap(o[0] - f17 as f64) * pi / 180.0 * k;
        step(rig, sink, &mut p, ry, rx, rz);
    }
    rig.pivot[head] = Vec3::new(p[0], p[1], p[2]);
    let o0 = off(0);
    rig.rot[head].y = wrap(o0[0] - off6[0]) * pi / 180.0 * 1.0;
    rig.rot[head].z = -wrap(o0[0] - f17 as f64) * pi / 180.0 * 1.0;
    rig.emit(sink, &m1, &[head]);
    let mb = m1 * Mat4::from_translation(Vec3::new(0.0, 1.0, 0.0)) * Mat4::from_rotation_z((-f16 * k).to_radians()) * Mat4::from_translation(Vec3::new(0.0, -1.0, 0.0));
    rig.rot[body].z = 0.0;
    rig.emit(sink, &mb, &[body]);
    let (wing, wing_tip, rear, rear_tip, rear_foot, front, front_tip, front_foot) = (
        rig.i("wing"),
        rig.i("wingTip"),
        rig.i("rearLeg"),
        rig.i("rearLegTip"),
        rig.i("rearFoot"),
        rig.i("frontLeg"),
        rig.i("frontLegTip"),
        rig.i("frontFoot"),
    );
    let mut mw = mb;
    for _ in 0..2 {
        let f21 = anim * PI * 2.0;
        rig.rot[wing] = Vec3::new(0.125 - (f21 as f64).cos() as f32 * 0.2, 0.25, ((f21 as f64).sin() + 0.125) as f32 * 0.8);
        rig.rot[wing_tip].z = -(((f21 + 2.0) as f64).sin() + 0.5) as f32 * 0.75;
        rig.rot[rear].x = 1.0 + f10 * 0.1;
        rig.rot[rear_tip].x = 0.5 + f10 * 0.1;
        rig.rot[rear_foot].x = 0.75 + f10 * 0.1;
        rig.rot[front].x = 1.3 + f10 * 0.1;
        rig.rot[front_tip].x = -0.5 - f10 * 0.1;
        rig.rot[front_foot].x = 0.75 + f10 * 0.1;
        rig.emit(sink, &mw, &[wing, front, rear]);
        mw *= Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0));
    }
    // Cauda: 12 segmentos a partir de (0, 10, 60).
    let mut f20 = -(((anim * PI * 2.0) as f64).sin() as f32) * 0.0;
    let f18 = anim * PI * 2.0;
    p = [0.0, 10.0, 60.0];
    let off11 = off(11);
    for i in 0..12 {
        let o = off(12 + i);
        f20 = (f20 as f64 + (((i as f32) * 0.45 + f18) as f64).sin() * 0.05000000074505806) as f32;
        let ry = (wrap(o[0] - off11[0]) * k + 180.0) * pi / 180.0;
        let rx = f20 + (o[1] - off11[1]) as f32 * pi / 180.0 * k * 5.0;
        let rz = wrap(o[0] - f17 as f64) * pi / 180.0 * k;
        step(rig, sink, &mut p, ry, rx, rz);
    }
}

/// `RenderDragon` (+ `LayerEnderDragonEyes`); a morte (explosão, raio) e o raio de recarga ficam pendentes.
pub fn dragon(e: &Entity, pos: Vec3, t: f32, death: f32, cx: &mut Ctx) -> bool {
    let Some(tex) = cx.textures.jar(cx.r, cx.jar, "entity/enderdragon/dragon") else { return true };
    let s = &e.mob_state;
    let dead = matches!(e.meta.get(&6), Some(MetaValue::Float(h)) if *h <= 0.0);
    let off7 = s.dragon_offsets(7, t, dead)[0] as f32;
    let dy = (s.dragon_offsets(5, t, dead)[1] - s.dragon_offsets(10, t, dead)[1]) as f32;
    let m = Mat4::from_translation(pos)
        * Mat4::from_rotation_y((-off7).to_radians())
        * Mat4::from_rotation_x((dy * 10.0).to_radians())
        * Mat4::from_translation(Vec3::new(0.0, 0.0, 1.0))
        * Mat4::from_rotation_z(death.to_radians())
        * Mat4::from_scale(Vec3::new(-1.0, -1.0, 1.0))
        * Mat4::from_translation(Vec3::new(0.0, -1.5078125, 0.0));
    let mut rig = Rig::new(&g::DRAGON);
    let (tint, light) = (cx.tint, cx.light);
    emit_dragon(&mut rig, e, t, &m, &mut cx.sink(tex, tint, 255, light));
    if let Some(eyes) = cx.textures.jar(cx.r, cx.jar, "entity/enderdragon/dragon_eyes") {
        let mut rig = Rig::new(&g::DRAGON);
        emit_dragon(&mut rig, e, t, &m, &mut cx.sink(eyes | ADDITIVE, [1.0; 3], 255, FULL_LIGHT));
    }
    true
}
