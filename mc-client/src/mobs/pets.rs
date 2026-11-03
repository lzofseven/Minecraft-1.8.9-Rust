//! Lobo, jaguatirica e coelho: `ModelWolf`, `ModelOcelot`, `ModelRabbit` com os `Render*` e camadas.

use glam::{Mat4, Vec3};

use super::{living, Ctx, DEG, TRANSLUCENT};
use crate::entities::Entity;
use crate::gen_mob_models as g;
use crate::mob_rig::Rig;
use crate::models::Anim;
use crate::mth::{cos, sin, PI};

/// `EntityWolf.getShakeAngle`.
fn shake_angle(e: &Entity, t: f32, off: f32) -> f32 {
    let s = &e.mob_state;
    let mut f = (s.shake_prev + (s.shake - s.shake_prev) * t + off) / 1.8;
    f = f.clamp(0.0, 1.0);
    sin(f * PI) * sin(f * PI * 11.0) * 0.15 * PI
}

/// `EntityWolf.getShakeAngle` (para os testes).
#[cfg(test)]
pub fn pose_shake(e: &Entity, t: f32, off: f32) -> f32 {
    shake_angle(e, t, off)
}

/// `EntityWolf.getTailRotation`.
fn tail_rotation(e: &Entity) -> f32 {
    let (angry, tamed) = (e.meta_byte(16) & 2 != 0, e.meta_byte(16) & 4 != 0);
    if angry {
        1.5393804
    } else if tamed {
        let health = match e.meta.get(&18) {
            Some(crate::entity_net::MetaValue::Float(h)) => *h,
            _ => 20.0,
        };
        (0.55 - (20.0 - health) * 0.02) * PI
    } else {
        0.62831855
    }
}

/// `ModelWolf.setLivingAnimations` + `setRotationAngles` (a rotação do rabo vem de `handleRotationFloat`).
pub fn pose_wolf(rig: &mut Rig, e: &Entity, t: f32, a: &Anim) {
    let (head, body, mane, tail) = (rig.i("wolfHeadMain"), rig.i("wolfBody"), rig.i("wolfMane"), rig.i("wolfTail"));
    let l = [rig.i("wolfLeg1"), rig.i("wolfLeg2"), rig.i("wolfLeg3"), rig.i("wolfLeg4")];
    let (angry, sitting) = (e.meta_byte(16) & 2 != 0, e.meta_byte(16) & 1 != 0);
    let (f, f1) = (a.limb_swing, a.limb_amount);
    rig.rot[tail].y = if angry { 0.0 } else { cos(f * 0.6662) * 1.4 * f1 };
    if sitting {
        rig.pivot[mane] = Vec3::new(-1.0, 16.0, -3.0);
        rig.rot[mane].x = 1.2566371;
        rig.rot[mane].y = 0.0;
        rig.pivot[body] = Vec3::new(0.0, 18.0, 0.0);
        rig.rot[body].x = 0.7853982;
        rig.pivot[tail] = Vec3::new(-1.0, 21.0, 6.0);
        rig.pivot[l[0]] = Vec3::new(-2.5, 22.0, 2.0);
        rig.rot[l[0]].x = 4.712389;
        rig.pivot[l[1]] = Vec3::new(0.5, 22.0, 2.0);
        rig.rot[l[1]].x = 4.712389;
        rig.rot[l[2]].x = 5.811947;
        rig.pivot[l[2]] = Vec3::new(-2.49, 17.0, -4.0);
        rig.rot[l[3]].x = 5.811947;
        rig.pivot[l[3]] = Vec3::new(0.51, 17.0, -4.0);
    } else {
        rig.pivot[body] = Vec3::new(0.0, 14.0, 2.0);
        rig.rot[body].x = 1.5707964;
        rig.pivot[mane] = Vec3::new(-1.0, 14.0, -3.0);
        rig.rot[mane].x = rig.rot[body].x;
        rig.pivot[tail] = Vec3::new(-1.0, 12.0, 8.0);
        rig.pivot[l[0]] = Vec3::new(-2.5, 16.0, 7.0);
        rig.pivot[l[1]] = Vec3::new(0.5, 16.0, 7.0);
        rig.pivot[l[2]] = Vec3::new(-2.5, 16.0, -4.0);
        rig.pivot[l[3]] = Vec3::new(0.5, 16.0, -4.0);
        rig.rot[l[0]].x = cos(f * 0.6662) * 1.4 * f1;
        rig.rot[l[1]].x = cos(f * 0.6662 + PI) * 1.4 * f1;
        rig.rot[l[2]].x = cos(f * 0.6662 + PI) * 1.4 * f1;
        rig.rot[l[3]].x = cos(f * 0.6662) * 1.4 * f1;
    }
    let s = &e.mob_state;
    let interested = (s.head_course_prev + (s.head_course - s.head_course_prev) * t) * 0.15 * PI;
    rig.rot[head].z = interested + shake_angle(e, t, 0.0);
    rig.rot[mane].z = shake_angle(e, t, -0.08);
    rig.rot[body].z = shake_angle(e, t, -0.16);
    rig.rot[tail].z = shake_angle(e, t, -0.2);
    // setRotationAngles
    rig.rot[head].x = a.head_pitch / DEG;
    rig.rot[head].y = a.head_yaw / DEG;
    rig.rot[tail].x = tail_rotation(e);
}

/// `RenderWolf` + `LayerWolfCollar`.
pub fn wolf(e: &Entity, pos: Vec3, t: f32, anim: &Anim, body: f32, death: f32, cx: &mut Ctx) -> bool {
    let m = living(pos, body, death, Vec3::ONE);
    let (tamed, angry) = (e.meta_byte(16) & 4 != 0, e.meta_byte(16) & 2 != 0);
    let path = if tamed {
        "entity/wolf/wolf_tame"
    } else if angry {
        "entity/wolf/wolf_angry"
    } else {
        "entity/wolf/wolf"
    };
    let Some(tex) = cx.textures.jar(cx.r, cx.jar, path) else { return true };
    let mut rig = Rig::new(&g::WOLF);
    pose_wolf(&mut rig, e, t, anim);
    let (mut tint, light) = (cx.tint, cx.light);
    let st = &e.mob_state;
    if st.wolf_wet {
        // RenderWolf.doRender: `color(f, f, f)` com `getBrightness(t) * getShadingWhileWet(t)`.
        let shading = 0.75 + (st.shake_prev + (st.shake - st.shake_prev) * t) / 2.0 * 0.25;
        let brightness = cx.brightness;
        tint = [brightness * shading; 3];
    }
    emit_wolf(&rig, &mut cx.sink(tex, tint, 255, light), &m, anim.child);
    if tamed && !e.invisible() {
        if let Some(collar) = cx.textures.jar(cx.r, cx.jar, "entity/wolf/wolf_collar") {
            // LayerWolfCollar: EnumDyeColor.byDyeDamage(meta 20).getMetadata() = 15 - dano.
            let c = g::SHEEP_COLORS[(15 - (e.meta_byte(20) & 15)) as usize];
            let tc = [c[0] * cx.tint[0], c[1] * cx.tint[1], c[2] * cx.tint[2]];
            emit_wolf(&rig, &mut cx.sink(collar | TRANSLUCENT, tc, 255, light), &m, anim.child);
        }
    }
    true
}

/// `ModelWolf.render`: cabeça e rabo com `renderWithRotation`; filhote = cabeça deslocada e o resto pela metade.
fn emit_wolf(rig: &Rig, sink: &mut crate::models::Sink, m: &Mat4, child: bool) {
    let (head, tail) = (rig.i("wolfHeadMain"), rig.i("wolfTail"));
    let rest = ["wolfBody", "wolfLeg1", "wolfLeg2", "wolfLeg3", "wolfLeg4", "wolfMane"].map(|n| rig.i(n));
    if child {
        rig.emit_rot(sink, &(*m * Mat4::from_translation(Vec3::new(0.0, 5.0 / 16.0, 2.0 / 16.0))), &[head]);
        let m2 = *m * Mat4::from_scale(Vec3::splat(0.5)) * Mat4::from_translation(Vec3::new(0.0, 24.0 / 16.0, 0.0));
        rig.emit(sink, &m2, &rest);
        rig.emit_rot(sink, &m2, &[tail]);
    } else {
        rig.emit_rot(sink, m, &[head]);
        rig.emit(sink, m, &rest);
        rig.emit_rot(sink, m, &[tail]);
    }
}

/// `ModelOcelot.setLivingAnimations` + `setRotationAngles`.
pub fn pose_ocelot(rig: &mut Rig, e: &Entity, a: &Anim) {
    let (head, body, tail, tail2) = (rig.i("ocelotHead"), rig.i("ocelotBody"), rig.i("ocelotTail"), rig.i("ocelotTail2"));
    let (bl, br, fl, fr) = (rig.i("ocelotBackLeftLeg"), rig.i("ocelotBackRightLeg"), rig.i("ocelotFrontLeftLeg"), rig.i("ocelotFrontRightLeg"));
    let set_yz = |rig: &mut Rig, i: usize, y: f32, z: f32| {
        rig.pivot[i].y = y;
        rig.pivot[i].z = z;
    };
    set_yz(rig, body, 12.0, -10.0);
    set_yz(rig, head, 15.0, -9.0);
    set_yz(rig, tail, 15.0, 8.0);
    set_yz(rig, tail2, 20.0, 14.0);
    set_yz(rig, fl, 13.8, -5.0);
    set_yz(rig, fr, 13.8, -5.0);
    set_yz(rig, bl, 18.0, 5.0);
    set_yz(rig, br, 18.0, 5.0);
    rig.rot[tail].x = 0.9;
    let flags = e.meta_byte(0);
    let state: i32;
    if flags & 0x02 != 0 {
        // isSneaking
        rig.pivot[body].y += 1.0;
        rig.pivot[head].y += 2.0;
        rig.pivot[tail].y += 1.0;
        rig.pivot[tail2].y += -4.0;
        rig.pivot[tail2].z += 2.0;
        rig.rot[tail].x = 1.5707964;
        rig.rot[tail2].x = 1.5707964;
        state = 0;
    } else if flags & 0x08 != 0 {
        // isSprinting
        rig.pivot[tail2].y = rig.pivot[tail].y;
        rig.pivot[tail2].z += 2.0;
        rig.rot[tail].x = 1.5707964;
        rig.rot[tail2].x = 1.5707964;
        state = 2;
    } else if e.meta_byte(16) & 1 != 0 {
        // isSitting
        rig.rot[body].x = 0.7853982;
        rig.pivot[body].y += -4.0;
        rig.pivot[body].z += 5.0;
        rig.pivot[head].y += -3.3;
        rig.pivot[head].z += 1.0;
        rig.pivot[tail].y += 8.0;
        rig.pivot[tail].z += -2.0;
        rig.pivot[tail2].y += 2.0;
        rig.pivot[tail2].z += -0.8;
        rig.rot[tail].x = 1.7278761;
        rig.rot[tail2].x = 2.670354;
        rig.rot[fl].x = -0.15707964;
        rig.rot[fr].x = -0.15707964;
        set_yz(rig, fl, 15.8, -7.0);
        set_yz(rig, fr, 15.8, -7.0);
        rig.rot[bl].x = -1.5707964;
        rig.rot[br].x = -1.5707964;
        set_yz(rig, bl, 21.0, 1.0);
        set_yz(rig, br, 21.0, 1.0);
        state = 3;
    } else {
        state = 1;
    }
    // setRotationAngles
    let (f, f1) = (a.limb_swing, a.limb_amount);
    rig.rot[head].x = a.head_pitch / DEG;
    rig.rot[head].y = a.head_yaw / DEG;
    if state != 3 {
        rig.rot[body].x = 1.5707964;
        if state == 2 {
            rig.rot[bl].x = cos(f * 0.6662) * 1.0 * f1;
            rig.rot[br].x = cos(f * 0.6662 + 0.3) * 1.0 * f1;
            rig.rot[fl].x = cos(f * 0.6662 + PI + 0.3) * 1.0 * f1;
            rig.rot[fr].x = cos(f * 0.6662 + PI) * 1.0 * f1;
            rig.rot[tail2].x = 1.7278761 + 0.31415927 * cos(f) * f1;
        } else {
            rig.rot[bl].x = cos(f * 0.6662) * 1.0 * f1;
            rig.rot[br].x = cos(f * 0.6662 + PI) * 1.0 * f1;
            rig.rot[fl].x = cos(f * 0.6662 + PI) * 1.0 * f1;
            rig.rot[fr].x = cos(f * 0.6662) * 1.0 * f1;
            if state == 1 {
                rig.rot[tail2].x = 1.7278761 + 0.7853982 * cos(f) * f1;
            } else {
                rig.rot[tail2].x = 1.7278761 + 0.47123894 * cos(f) * f1;
            }
        }
    }
}

/// `RenderOcelot`: 4 pelagens (metadado 18) e escala 0.8 se domesticada.
pub fn ocelot(e: &Entity, pos: Vec3, _t: f32, anim: &Anim, body: f32, death: f32, cx: &mut Ctx) -> bool {
    let tamed = e.meta_byte(16) & 4 != 0;
    let path = match e.meta_byte(18) {
        1 => "entity/cat/black",
        2 => "entity/cat/red",
        3 => "entity/cat/siamese",
        _ => "entity/cat/ocelot",
    };
    let Some(tex) = cx.textures.jar(cx.r, cx.jar, path) else { return true };
    let m = living(pos, body, death, Vec3::splat(if tamed { 0.8 } else { 1.0 }));
    let mut rig = Rig::new(&g::OCELOT);
    pose_ocelot(&mut rig, e, anim);
    let (tint, light) = (cx.tint, cx.light);
    let mut sink = cx.sink(tex, tint, 255, light);
    let head = rig.i("ocelotHead");
    let rest = ["ocelotBody", "ocelotTail", "ocelotTail2", "ocelotBackLeftLeg", "ocelotBackRightLeg", "ocelotFrontLeftLeg", "ocelotFrontRightLeg"].map(|n| rig.i(n));
    if anim.child {
        let mh = m * Mat4::from_scale(Vec3::splat(1.5 / 2.0)) * Mat4::from_translation(Vec3::new(0.0, 10.0 / 16.0, 4.0 / 16.0));
        rig.emit(&mut sink, &mh, &[head]);
        let m2 = m * Mat4::from_scale(Vec3::splat(0.5)) * Mat4::from_translation(Vec3::new(0.0, 24.0 / 16.0, 0.0));
        rig.emit(&mut sink, &m2, &rest);
    } else {
        rig.emit(&mut sink, &m, &[head]);
        rig.emit(&mut sink, &m, &rest);
    }
    true
}

/// `ModelRabbit.setRotationAngles` (`partial` é `ageInTicks - ticksExisted`).
pub fn pose_rabbit(rig: &mut Rig, e: &Entity, partial: f32, a: &Anim) {
    let (nose, head, rear, lear) = (rig.i("rabbitNose"), rig.i("rabbitHead"), rig.i("rabbitRightEar"), rig.i("rabbitLeftEar"));
    let x = a.head_pitch * 0.017453292;
    for i in [nose, head, rear, lear] {
        rig.rot[i].x = x;
    }
    let y = a.head_yaw * 0.017453292;
    rig.rot[nose].y = y;
    rig.rot[head].y = y;
    rig.rot[rear].y = y - 0.2617994;
    rig.rot[lear].y = y + 0.2617994;
    let s = &e.mob_state;
    let progress = if s.hop_len == 0 { 0.0 } else { (s.hop as f32 + partial) / s.hop_len as f32 };
    let jump = sin(progress * PI);
    let (lt, rt, lf, rf, la, ra) = (rig.i("rabbitLeftThigh"), rig.i("rabbitRightThigh"), rig.i("rabbitLeftFoot"), rig.i("rabbitRightFoot"), rig.i("rabbitLeftArm"), rig.i("rabbitRightArm"));
    rig.rot[lt].x = (jump * 50.0 - 21.0) * 0.017453292;
    rig.rot[rt].x = rig.rot[lt].x;
    rig.rot[lf].x = jump * 50.0 * 0.017453292;
    rig.rot[rf].x = rig.rot[lf].x;
    rig.rot[la].x = (jump * -40.0 - 11.0) * 0.017453292;
    rig.rot[ra].x = rig.rot[la].x;
}

/// `RenderRabbit`: 7 pelagens pelo metadado 18 (99 = coelho assassino) e a textura "Toast" pelo nome.
pub fn rabbit(e: &Entity, pos: Vec3, t: f32, anim: &Anim, body: f32, death: f32, cx: &mut Ctx) -> bool {
    let path = if strip_formatting(e.meta_str(2)) == "Toast" {
        "entity/rabbit/toast"
    } else {
        match e.meta_byte(18) {
            1 => "entity/rabbit/white",
            2 => "entity/rabbit/black",
            3 => "entity/rabbit/white_splotched",
            4 => "entity/rabbit/gold",
            5 => "entity/rabbit/salt",
            99 => "entity/rabbit/caerbannog",
            _ => "entity/rabbit/brown",
        }
    };
    let Some(tex) = cx.textures.jar(cx.r, cx.jar, path) else { return true };
    let m = living(pos, body, death, Vec3::ONE);
    let mut rig = Rig::new(&g::RABBIT);
    pose_rabbit(&mut rig, e, t, anim);
    let (tint, light) = (cx.tint, cx.light);
    let mut sink = cx.sink(tex, tint, 255, light);
    let front = ["rabbitHead", "rabbitLeftEar", "rabbitRightEar", "rabbitNose"].map(|n| rig.i(n));
    let rest = ["rabbitLeftFoot", "rabbitRightFoot", "rabbitLeftThigh", "rabbitRightThigh", "rabbitBody", "rabbitLeftArm", "rabbitRightArm", "rabbitTail"].map(|n| rig.i(n));
    if anim.child {
        rig.emit(&mut sink, &(m * Mat4::from_translation(Vec3::new(0.0, 5.0 / 16.0, 2.0 / 16.0))), &front);
        let m2 = m * Mat4::from_scale(Vec3::splat(0.5)) * Mat4::from_translation(Vec3::new(0.0, 24.0 / 16.0, 0.0));
        rig.emit(&mut sink, &m2, &rest);
    } else {
        rig.emit(&mut sink, &m, &front);
        rig.emit(&mut sink, &m, &rest);
    }
    true
}

/// `EnumChatFormatting.getTextWithoutFormattingCodes`: tira os `§x`.
pub fn strip_formatting(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\u{a7}' {
            it.next();
        } else {
            out.push(c);
        }
    }
    out
}
