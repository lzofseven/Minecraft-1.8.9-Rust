//! Slime e cubo de magma (`ModelSlime`, `ModelMagmaCube`, `RenderSlime`, `LayerSlimeGel`).

use glam::Vec3;

use super::{living, Ctx, ADDITIVE, TRANSLUCENT};
use crate::entities::Entity;
use crate::entity_net::MetaValue;
use crate::gen_mob_models as g;
use crate::mob_rig::Rig;
use crate::models::{Anim, Biped, FULL_LIGHT};
use crate::mth::{cos, sin, PI};

use super::DEG;

/// `RenderSlime/RenderMagmaCube.preRenderCallback`: escala pelo tamanho e esmagamento.
fn squish_scale(e: &Entity, t: f32) -> (f32, Vec3) {
    let size = e.meta_byte(16) as f32;
    let s = &e.mob_state;
    let f4 = (s.squish_prev + (s.squish - s.squish_prev) * t) / (size * 0.5 + 1.0);
    let f5 = 1.0 / (f4 + 1.0);
    (s.squish_prev + (s.squish - s.squish_prev) * t, Vec3::new(f5 * size, 1.0 / f5 * size, f5 * size))
}

pub fn slime(e: &Entity, id: u8, pos: Vec3, t: f32, anim: &Anim, body: f32, death: f32, cx: &mut Ctx) -> bool {
    let (squish, scale) = squish_scale(e, t);
    let m = living(pos, body, death, scale);
    let (tint, mut light) = (cx.tint, cx.light);
    if id == 62 {
        // EntityMagmaCube.getBrightnessForRender = 15728880.
        light = FULL_LIGHT;
        let Some(tex) = cx.textures.jar(cx.r, cx.jar, "entity/slime/magmacube") else { return true };
        let mut rig = Rig::new(&g::MAGMA_CUBE);
        let f = squish.max(0.0);
        for i in 0..8 {
            let p = rig.i(&format!("segments_{i}"));
            rig.pivot[p].y = (-(4 - i as i32)) as f32 * f * 1.7;
        }
        let mut roots = vec![rig.i("core")];
        roots.extend((0..8).map(|i| rig.i(&format!("segments_{i}"))));
        rig.emit(&mut cx.sink(tex, tint, 255, light), &m, &roots);
        return true;
    }
    let Some(tex) = cx.textures.jar(cx.r, cx.jar, "entity/slime/slime") else { return true };
    let _ = anim;
    let inner = Rig::new(&g::SLIME_INNER);
    let roots = inner.roots();
    inner.emit(&mut cx.sink(tex, tint, 255, light), &m, &roots);
    if !e.invisible() {
        // LayerSlimeGel: ModelSlime(0) com mistura alfa normal.
        let outer = Rig::new(&g::SLIME_OUTER);
        let roots = outer.roots();
        outer.emit(&mut cx.sink(tex | TRANSLUCENT, [1.0; 3], 255, light), &m, &roots);
    }
    true
}

/// `RenderGhast`: textura de ataque pelo byte 16, escala 4.5 e o corpo 0.6 acima.
pub fn ghast(e: &Entity, pos: Vec3, _t: f32, anim: &Anim, body: f32, death: f32, cx: &mut Ctx) -> bool {
    let path = if e.meta_byte(16) != 0 { "entity/ghast/ghast_shooting" } else { "entity/ghast/ghast" };
    let Some(tex) = cx.textures.jar(cx.r, cx.jar, path) else { return true };
    let m = living(pos, body, death, Vec3::splat(4.5)) * glam::Mat4::from_translation(Vec3::new(0.0, 0.6, 0.0));
    let mut rig = Rig::new(&g::GHAST);
    for i in 0..9 {
        let p = rig.i(&format!("tentacles_{i}"));
        rig.rot[p].x = 0.2 * sin(anim.age * 0.3 + i as f32) + 0.4;
    }
    let roots = rig.roots();
    let (tint, light) = (cx.tint, cx.light);
    rig.emit(&mut cx.sink(tex, tint, 255, light), &m, &roots);
    true
}

/// `ModelBlaze.setRotationAngles`: três anéis de hastes girando.
pub fn pose_blaze(rig: &mut Rig, a: &Anim) {
    let age = a.age;
    let mut f = age * PI * -0.1;
    for i in 0..4usize {
        let p = rig.i(&format!("blazeSticks_{i}"));
        rig.pivot[p].y = -2.0 + cos(((i * 2) as f32 + age) * 0.25);
        rig.pivot[p].x = cos(f) * 9.0;
        rig.pivot[p].z = sin(f) * 9.0;
        f += 1.0;
    }
    f = 0.7853982 + age * PI * 0.03;
    for i in 4..8usize {
        let p = rig.i(&format!("blazeSticks_{i}"));
        rig.pivot[p].y = 2.0 + cos(((i * 2) as f32 + age) * 0.25);
        rig.pivot[p].x = cos(f) * 7.0;
        rig.pivot[p].z = sin(f) * 7.0;
        f += 1.0;
    }
    f = 0.47123894 + age * PI * -0.05;
    for i in 8..12usize {
        let p = rig.i(&format!("blazeSticks_{i}"));
        rig.pivot[p].y = 11.0 + cos((i as f32 * 1.5 + age) * 0.5);
        rig.pivot[p].x = cos(f) * 5.0;
        rig.pivot[p].z = sin(f) * 5.0;
        f += 1.0;
    }
    let h = rig.i("blazeHead");
    rig.rot[h].y = a.head_yaw / DEG;
    rig.rot[h].x = a.head_pitch / DEG;
}

/// `RenderBlaze` (brilho cheio por `getBrightnessForRender`).
pub fn blaze(pos: Vec3, anim: &Anim, body: f32, death: f32, cx: &mut Ctx) -> bool {
    let Some(tex) = cx.textures.jar(cx.r, cx.jar, "entity/blaze") else { return true };
    let m = living(pos, body, death, Vec3::ONE);
    let mut rig = Rig::new(&g::BLAZE);
    pose_blaze(&mut rig, anim);
    let roots = rig.roots();
    let tint = cx.tint;
    rig.emit(&mut cx.sink(tex, tint, 255, FULL_LIGHT), &m, &roots);
    true
}

/// `ModelEnderMite`/`ModelSilverfish.setRotationAngles` (segmentos ondulando) e `RenderEndermite/Silverfish` (morte 180).
pub fn bugs(id: u8, pos: Vec3, anim: &Anim, body: f32, death: f32, cx: &mut Ctx) -> bool {
    let age = anim.age;
    let m = living(pos, body, death * 2.0, Vec3::ONE);
    let (tint, light) = (cx.tint, cx.light);
    if id == 67 {
        let Some(tex) = cx.textures.jar(cx.r, cx.jar, "entity/endermite") else { return true };
        let mut rig = Rig::new(&g::ENDERMITE);
        for i in 0..4usize {
            let p = rig.i(&format!("field_178713_d_{i}"));
            let d = (i as i32 - 2).abs();
            rig.rot[p].y = cos(age * 0.9 + i as f32 * 0.15 * PI) * PI * 0.01 * (1 + d) as f32;
            rig.pivot[p].x = sin(age * 0.9 + i as f32 * 0.15 * PI) * PI * 0.1 * d as f32;
        }
        let roots = rig.roots();
        rig.emit(&mut cx.sink(tex, tint, 255, light), &m, &roots);
    } else {
        let Some(tex) = cx.textures.jar(cx.r, cx.jar, "entity/silverfish") else { return true };
        let mut rig = Rig::new(&g::SILVERFISH);
        let part = |rig: &Rig, i: usize| rig.i(&format!("silverfishBodyParts_{i}"));
        for i in 0..7usize {
            let p = part(&rig, i);
            let d = (i as i32 - 2).abs();
            rig.rot[p].y = cos(age * 0.9 + i as f32 * 0.15 * PI) * PI * 0.05 * (1 + d) as f32;
            rig.pivot[p].x = sin(age * 0.9 + i as f32 * 0.15 * PI) * PI * 0.2 * d as f32;
        }
        let (w0, w1, w2) = (rig.i("silverfishWings_0"), rig.i("silverfishWings_1"), rig.i("silverfishWings_2"));
        let (b1, b2, b4) = (part(&rig, 1), part(&rig, 2), part(&rig, 4));
        rig.rot[w0].y = rig.rot[b2].y;
        rig.rot[w1].y = rig.rot[b4].y;
        rig.pivot[w1].x = rig.pivot[b4].x;
        rig.rot[w2].y = rig.rot[b1].y;
        rig.pivot[w2].x = rig.pivot[b1].x;
        let roots = rig.roots();
        rig.emit(&mut cx.sink(tex, tint, 255, light), &m, &roots);
    }
    true
}

/// Gaussiana para o tremor do enderman provocado (`Random.nextGaussian`; xorshift + Box-Muller).
fn gaussian() -> f64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static S: AtomicU64 = AtomicU64::new(0x2545F4914F6CDD1D);
    let next = || {
        let mut x = S.load(Ordering::Relaxed);
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        S.store(x, Ordering::Relaxed);
        (x >> 11) as f64 / (1u64 << 53) as f64
    };
    let (u, v) = (next().max(1e-12), next());
    (-2.0 * u.ln()).sqrt() * (2.0 * std::f64::consts::PI * v).cos()
}

/// `ModelEnderman.setRotationAngles` (sobre o `ModelBiped.setRotationAngles`) e `RenderEnderman`.
pub fn enderman(e: &Entity, pos: Vec3, _t: f32, anim: &Anim, body: f32, death: f32, cx: &mut Ctx) -> bool {
    let Some(tex) = cx.textures.jar(cx.r, cx.jar, "entity/enderman/enderman") else { return true };
    let carrying = match e.meta.get(&16) {
        Some(MetaValue::Short(s)) => s & 255 != 0,
        _ => false,
    };
    let screaming = e.meta_byte(18) > 0;
    let mut pos = pos;
    if screaming {
        pos.x += (gaussian() * 0.02) as f32;
        pos.z += (gaussian() * 0.02) as f32;
    }
    let m = living(pos, body, death, Vec3::ONE);
    let rig = Rig::new(&g::ENDERMAN);
    let mk = |n: &str| rig.part(rig.i(n));
    let mut b = Biped {
        head: mk("bipedHead"),
        hat: mk("bipedHeadwear"),
        body: mk("bipedBody"),
        right_arm: mk("bipedRightArm"),
        left_arm: mk("bipedLeftArm"),
        right_leg: mk("bipedRightLeg"),
        left_leg: mk("bipedLeftLeg"),
        wear: None,
        tex: rig.tex(),
    };
    let a = Anim { held_right: 0, riding: false, sneak: false, aimed_bow: false, child: false, ..*anim };
    b.pose(&a);
    let off = -14.0;
    b.body.rot.x = 0.0;
    b.body.pivot.y = off;
    b.body.pivot.z = -0.0;
    for p in [&mut b.right_arm, &mut b.left_arm, &mut b.right_leg, &mut b.left_leg] {
        p.rot.x = (p.rot.x as f64 * 0.5) as f32;
        p.rot.x = p.rot.x.clamp(-0.4, 0.4);
    }
    if carrying {
        b.right_arm.rot.x = -0.5;
        b.left_arm.rot.x = -0.5;
        b.right_arm.rot.z = 0.05;
        b.left_arm.rot.z = -0.05;
    }
    b.right_arm.pivot.z = 0.0;
    b.left_arm.pivot.z = 0.0;
    b.right_leg.pivot.z = 0.0;
    b.left_leg.pivot.z = 0.0;
    b.right_leg.pivot.y = 9.0 + off;
    b.left_leg.pivot.y = 9.0 + off;
    b.head.pivot.z = -0.0;
    b.head.pivot.y = off + 1.0;
    b.hat.copy_pose(&b.head);
    if screaming {
        b.head.pivot.y -= 5.0;
    }
    let (tint, light) = (cx.tint, cx.light);
    let draw = |sink: &mut crate::models::Sink| {
        sink.tex = b.tex;
        for p in [&b.head, &b.body, &b.right_arm, &b.left_arm, &b.right_leg, &b.left_leg, &b.hat] {
            sink.part(p, &m);
        }
    };
    draw(&mut cx.sink(tex, tint, 255, light));
    if let Some(eyes) = cx.textures.jar(cx.r, cx.jar, "entity/enderman/enderman_eyes") {
        // LayerEndermanEyes: brilho cheio (0xf0f0). Mistura aditiva: ver `mobs::ADDITIVE`.
        draw(&mut cx.sink(eyes | ADDITIVE, [1.0; 3], 255, FULL_LIGHT));
    }
    if carrying {
        // LayerHeldBlock: bloco = id (byte baixo do short 16) e meta (byte 17).
        let id = match e.meta.get(&16) {
            Some(MetaValue::Short(s)) => (*s & 255) as u16,
            _ => 0,
        };
        let state = (id << 4) | (e.meta_byte(17) as u16 & 15);
        use crate::camera::{rot, tr};
        let hm = m * tr(0.0, 0.6875, -0.75) * rot(20.0, 1.0, 0.0, 0.0) * rot(45.0, 0.0, 1.0, 0.0) * tr(0.25, 0.1875, 0.25) * glam::Mat4::from_scale(Vec3::new(-0.5, -0.5, 0.5));
        super::emit_block(cx, state, &hm, light);
    }
    true
}
