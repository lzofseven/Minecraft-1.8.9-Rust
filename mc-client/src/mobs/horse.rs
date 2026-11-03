//! Cavalo, burro, mula, cavalo-zumbi e cavalo-esqueleto (`ModelHorse`, `RenderHorse`).

use std::cell::RefCell;
use std::collections::HashMap;

use glam::{Mat4, Vec3};

use super::{living, Ctx, DEG};
use crate::entities::Entity;
use crate::entity_net::MetaValue;
use crate::gen_mob_models as g;
use crate::mob_rig::Rig;
use crate::models::Anim;
use crate::mth::{cos, sin, PI};

fn meta_int(e: &Entity, i: u8) -> i32 {
    match e.meta.get(&i) {
        Some(MetaValue::Int(v)) => *v,
        _ => 0,
    }
}

/// `EntityHorse.getHorseWatchableBoolean(bit)`.
fn flag(e: &Entity, bit: i32) -> bool {
    meta_int(e, 16) & bit != 0
}

/// Estado por tick de `EntityHorse.onUpdate`/`onLivingUpdate` (a parte que o cliente roda).
pub fn tick(s: &mut super::MobState, flags: i32, rng_seed: i64) {
    let rng = s.rng.get_or_insert_with(|| super::JRandom::new(rng_seed));
    if rng.next_int(200) == 0 {
        s.horse_tail = 1;
    }
    if s.horse_tail > 0 {
        s.horse_tail += 1;
        if s.horse_tail > 8 {
            s.horse_tail = 0;
        }
    }
    s.horse_lean_prev = s.horse_lean;
    if flags & 32 != 0 {
        s.horse_lean += (1.0 - s.horse_lean) * 0.4 + 0.05;
        if s.horse_lean > 1.0 {
            s.horse_lean = 1.0;
        }
    } else {
        s.horse_lean += (0.0 - s.horse_lean) * 0.4 - 0.05;
        if s.horse_lean < 0.0 {
            s.horse_lean = 0.0;
        }
    }
    s.horse_rear_prev = s.horse_rear;
    if flags & 64 != 0 {
        s.horse_lean = 0.0;
        s.horse_lean_prev = 0.0;
        s.horse_rear += (1.0 - s.horse_rear) * 0.4 + 0.05;
        if s.horse_rear > 1.0 {
            s.horse_rear = 1.0;
        }
    } else {
        s.horse_rear += (0.8 * s.horse_rear * s.horse_rear * s.horse_rear - s.horse_rear) * 0.6 - 0.05;
        if s.horse_rear < 0.0 {
            s.horse_rear = 0.0;
        }
    }
    s.horse_mouth_prev = s.horse_mouth;
    if flags & 128 != 0 {
        s.horse_mouth += (1.0 - s.horse_mouth) * 0.7 + 0.05;
        if s.horse_mouth > 1.0 {
            s.horse_mouth = 1.0;
        }
    } else {
        s.horse_mouth += (0.0 - s.horse_mouth) * 0.7 - 0.05;
        if s.horse_mouth < 0.0 {
            s.horse_mouth = 0.0;
        }
    }
}

/// `ModelHorse.setLivingAnimations`. `ridden` é `riddenByEntity != null`.
pub fn pose_horse(rig: &mut Rig, e: &Entity, t: f32, a: &Anim, ridden: bool) {
    let s = &e.mob_state;
    let (f, f1) = (a.limb_swing, a.limb_amount);
    let mut yaw = a.head_yaw;
    yaw = yaw.clamp(-20.0, 20.0);
    let mut pitch = a.head_pitch / DEG;
    if f1 > 0.2 {
        pitch += cos(f * 0.4) * 0.15 * f1;
    }
    let eat = s.horse_lean_prev + (s.horse_lean - s.horse_lean_prev) * t;
    let rear = s.horse_rear_prev + (s.horse_rear - s.horse_rear_prev) * t;
    let not_rear = 1.0 - rear;
    let mouth = s.horse_mouth_prev + (s.horse_mouth - s.horse_mouth_prev) * t;
    let tail_swing = s.horse_tail != 0;
    let saddled = flag(e, 4);
    let age = e.age as f32 + t;
    let l19 = cos(f * 0.6662 + PI);
    let l20 = l19 * 0.8 * f1;
    let model = rig.model;
    let i = |n: &str| model.parts.iter().position(|p| p.name == n).unwrap_or_else(|| panic!("parte {n} inexistente"));
    let (head, tail_base, tail_mid, tail_tip, body) = (i("head"), i("tailBase"), i("tailMiddle"), i("tailTip"), i("body"));
    let (rchest, lchest) = (i("muleRightChest"), i("muleLeftChest"));
    rig.pivot[head].y = 4.0;
    rig.pivot[head].z = -10.0;
    rig.pivot[tail_base].y = 3.0;
    rig.pivot[tail_mid].z = 14.0;
    rig.pivot[rchest].y = 3.0;
    rig.pivot[rchest].z = 10.0;
    rig.rot[body].x = 0.0;
    rig.rot[head].x = 0.5235988 + pitch;
    rig.rot[head].y = yaw / DEG;
    let m = rear.max(eat);
    rig.rot[head].x = rear * (0.2617994 + pitch) + eat * 2.18166 + (1.0 - m) * rig.rot[head].x;
    rig.rot[head].y = rear * yaw / DEG + (1.0 - m) * rig.rot[head].y;
    rig.pivot[head].y = rear * -6.0 + eat * 11.0 + (1.0 - m) * rig.pivot[head].y;
    rig.pivot[head].z = rear * -1.0 + eat * -10.0 + (1.0 - m) * rig.pivot[head].z;
    rig.pivot[tail_base].y = rear * 9.0 + not_rear * rig.pivot[tail_base].y;
    rig.pivot[tail_mid].z = rear * 18.0 + not_rear * rig.pivot[tail_mid].z;
    rig.pivot[rchest].y = rear * 5.5 + not_rear * rig.pivot[rchest].y;
    rig.pivot[rchest].z = rear * 15.0 + not_rear * rig.pivot[rchest].z;
    rig.rot[body].x = rear * -45.0 / DEG + not_rear * rig.rot[body].x;
    let (hp, hr) = (rig.pivot[head], rig.rot[head]);
    for n in ["horseLeftEar", "horseRightEar", "muleLeftEar", "muleRightEar", "neck", "mane"] {
        let k = i(n);
        rig.pivot[k].y = hp.y;
        rig.pivot[k].z = hp.z;
        rig.rot[k].x = hr.x;
        rig.rot[k].y = hr.y;
    }
    let (lip, jaw) = (i("field_178711_b"), i("field_178712_c"));
    rig.pivot[lip].y = 0.02;
    rig.pivot[jaw].y = 0.0;
    rig.pivot[lip].z = 0.02 - mouth * 1.0;
    rig.pivot[jaw].z = 0.0 + mouth * 1.0;
    rig.rot[lip].x = 0.0 - 0.09424778 * mouth;
    rig.rot[jaw].x = 0.0 + 0.15707964 * mouth;
    rig.rot[lip].y = 0.0;
    rig.rot[jaw].y = 0.0;
    rig.rot[lchest].x = l20 / 5.0;
    rig.rot[rchest].x = -l20 / 5.0;
    let l24 = 0.2617994 * rear;
    let l25 = cos(age * 0.6 + PI);
    let (fl, fr, bl, br) = (i("frontLeftLeg"), i("frontRightLeg"), i("backLeftLeg"), i("backRightLeg"));
    rig.pivot[fl].y = -2.0 * rear + 9.0 * not_rear;
    rig.pivot[fl].z = -2.0 * rear + -8.0 * not_rear;
    rig.pivot[fr].y = rig.pivot[fl].y;
    rig.pivot[fr].z = rig.pivot[fl].z;
    let (bls, brs, fls, frs) = (i("backLeftShin"), i("backRightShin"), i("frontLeftShin"), i("frontRightShin"));
    let (blh, brh, flh, frh) = (i("backLeftHoof"), i("backRightHoof"), i("frontLeftHoof"), i("frontRightHoof"));
    rig.pivot[bls].y = rig.pivot[bl].y + sin(1.5707964 + l24 + not_rear * -l19 * 0.5 * f1) * 7.0;
    rig.pivot[bls].z = rig.pivot[bl].z + cos(4.712389 + l24 + not_rear * -l19 * 0.5 * f1) * 7.0;
    rig.pivot[brs].y = rig.pivot[br].y + sin(1.5707964 + l24 + not_rear * l19 * 0.5 * f1) * 7.0;
    rig.pivot[brs].z = rig.pivot[br].z + cos(4.712389 + l24 + not_rear * l19 * 0.5 * f1) * 7.0;
    let l26 = (-1.0471976 + l25) * rear + l20 * not_rear;
    let l27 = (-1.0471976 + -l25) * rear + -l20 * not_rear;
    rig.pivot[fls].y = rig.pivot[fl].y + sin(1.5707964 + l26) * 7.0;
    rig.pivot[fls].z = rig.pivot[fl].z + cos(4.712389 + l26) * 7.0;
    rig.pivot[frs].y = rig.pivot[fr].y + sin(1.5707964 + l27) * 7.0;
    rig.pivot[frs].z = rig.pivot[fr].z + cos(4.712389 + l27) * 7.0;
    rig.rot[bl].x = l24 + -l19 * 0.5 * f1 * not_rear;
    rig.rot[bls].x = -0.08726646 * rear + (-l19 * 0.5 * f1 - (0.0f32).max(l19 * 0.5 * f1)) * not_rear;
    rig.rot[blh].x = rig.rot[bls].x;
    rig.rot[br].x = l24 + l19 * 0.5 * f1 * not_rear;
    rig.rot[brs].x = -0.08726646 * rear + (l19 * 0.5 * f1 - (0.0f32).max(-l19 * 0.5 * f1)) * not_rear;
    rig.rot[brh].x = rig.rot[brs].x;
    rig.rot[fl].x = l26;
    rig.rot[fls].x = (rig.rot[fl].x + PI * (0.0f32).max(0.2 + l25 * 0.2)) * rear + (l20 + (0.0f32).max(l19 * 0.5 * f1)) * not_rear;
    rig.rot[flh].x = rig.rot[fls].x;
    rig.rot[fr].x = l27;
    rig.rot[frs].x = (rig.rot[fr].x + PI * (0.0f32).max(0.2 - l25 * 0.2)) * rear + (-l20 + (0.0f32).max(-l19 * 0.5 * f1)) * not_rear;
    rig.rot[frh].x = rig.rot[frs].x;
    for (h, sh) in [(blh, bls), (brh, brs), (flh, fls), (frh, frs)] {
        rig.pivot[h].y = rig.pivot[sh].y;
        rig.pivot[h].z = rig.pivot[sh].z;
    }
    if saddled {
        let (sb, sf, sk) = (i("horseSaddleBottom"), i("horseSaddleFront"), i("horseSaddleBack"));
        rig.pivot[sb].y = rear * 0.5 + not_rear * 2.0;
        rig.pivot[sb].z = rear * 11.0 + not_rear * 2.0;
        let (lr, rr, lm, rm) = (i("horseLeftSaddleRope"), i("horseRightSaddleRope"), i("horseLeftSaddleMetal"), i("horseRightSaddleMetal"));
        for k in [sf, sk, lr, rr, lm, rm] {
            rig.pivot[k].y = rig.pivot[sb].y;
            rig.pivot[k].z = rig.pivot[sb].z;
        }
        rig.pivot[lchest].y = rig.pivot[rchest].y;
        rig.pivot[lchest].z = rig.pivot[rchest].z;
        for k in [sb, sf, sk] {
            rig.rot[k].x = rig.rot[body].x;
        }
        let (lrein, rrein, ropes, lfm, rfm) = (i("horseLeftRein"), i("horseRightRein"), i("horseFaceRopes"), i("horseLeftFaceMetal"), i("horseRightFaceMetal"));
        for k in [lrein, rrein, ropes, lfm, rfm] {
            rig.pivot[k].y = rig.pivot[head].y;
            rig.pivot[k].z = rig.pivot[head].z;
        }
        rig.rot[lrein].x = pitch;
        rig.rot[rrein].x = pitch;
        for k in [ropes, lfm, rfm] {
            rig.rot[k].x = rig.rot[head].x;
        }
        for k in [ropes, lfm, lrein, rfm, rrein] {
            rig.rot[k].y = rig.rot[head].y;
        }
        if ridden {
            for k in [lr, lm, rr, rm] {
                rig.rot[k].x = -1.0471976;
                rig.rot[k].z = 0.0;
            }
        } else {
            for k in [lr, lm, rr, rm] {
                rig.rot[k].x = l20 / 3.0;
            }
            rig.rot[lr].z = l20 / 5.0;
            rig.rot[lm].z = l20 / 5.0;
            rig.rot[rr].z = -l20 / 5.0;
            rig.rot[rm].z = -l20 / 5.0;
        }
    }
    let mut l21 = -1.3089 + f1 * 1.5;
    if l21 > 0.0 {
        l21 = 0.0;
    }
    if tail_swing {
        rig.rot[tail_base].y = cos(age * 0.7);
        l21 = 0.0;
    } else {
        rig.rot[tail_base].y = 0.0;
    }
    rig.rot[tail_mid].y = rig.rot[tail_base].y;
    rig.rot[tail_tip].y = rig.rot[tail_base].y;
    rig.pivot[tail_mid].y = rig.pivot[tail_base].y;
    rig.pivot[tail_tip].y = rig.pivot[tail_base].y;
    rig.pivot[tail_mid].z = rig.pivot[tail_base].z;
    rig.pivot[tail_tip].z = rig.pivot[tail_base].z;
    rig.rot[tail_base].x = l21;
    rig.rot[tail_mid].x = l21;
    rig.rot[tail_tip].x = -0.2618 + l21;
}

thread_local! {
    /// `LayeredTexture` já montadas (chave = camadas), para não recompor a cada quadro.
    static LAYERED: RefCell<HashMap<String, Option<u32>>> = RefCell::new(HashMap::new());
}

/// `LayeredTexture`: as imagens empilhadas com mistura alfa normal (`drawImage`).
fn layered(cx: &mut Ctx, layers: &[&str]) -> Option<u32> {
    let key = layers.join("+");
    if let Some(v) = LAYERED.with(|c| c.borrow().get(&key).copied()) {
        return v;
    }
    let mut base: Option<image::RgbaImage> = None;
    for path in layers {
        let Some(img) = cx.jar.texture(path) else { continue };
        match &mut base {
            None => base = Some(img),
            Some(b) => {
                for (x, y, px) in img.enumerate_pixels() {
                    if x >= b.width() || y >= b.height() {
                        continue;
                    }
                    let d = b.get_pixel_mut(x, y);
                    let sa = px[3] as f32 / 255.0;
                    let da = d[3] as f32 / 255.0;
                    let oa = sa + da * (1.0 - sa);
                    if oa > 0.0 {
                        for c in 0..3 {
                            d[c] = ((px[c] as f32 * sa + d[c] as f32 * da * (1.0 - sa)) / oa).round() as u8;
                        }
                    }
                    d[3] = (oa * 255.0).round() as u8;
                }
            }
        }
    }
    let id = base.map(|img| cx.r.create_texture(img.width(), img.height(), img.as_raw()));
    LAYERED.with(|c| c.borrow_mut().insert(key, id));
    id
}

/// Caminho das texturas por tipo (`RenderHorse.getEntityTexture`) e a composição do tipo 0.
fn horse_texture(cx: &mut Ctx, e: &Entity) -> Option<u32> {
    let kind = e.meta_byte(19);
    if kind == 0 {
        const COATS: [&str; 7] = ["white", "creamy", "chestnut", "brown", "black", "gray", "darkbrown"];
        const MARKS: [&str; 5] = ["", "markings_white", "markings_whitefield", "markings_whitedots", "markings_blackdots"];
        const ARMOR: [&str; 4] = ["", "armor/horse_armor_iron", "armor/horse_armor_gold", "armor/horse_armor_diamond"];
        let variant = meta_int(e, 20);
        let (coat, mark) = ((variant & 255) as usize, ((variant & 0xff00) >> 8) as usize);
        let armor = meta_int(e, 22) as usize;
        if coat >= COATS.len() || mark >= MARKS.len() || armor >= ARMOR.len() {
            return None;
        }
        let mut paths = vec![format!("entity/horse/horse_{}", COATS[coat])];
        if mark > 0 {
            paths.push(format!("entity/horse/horse_{}", MARKS[mark]));
        }
        if armor > 0 {
            paths.push(format!("entity/horse/{}", ARMOR[armor]));
        }
        if paths.len() == 1 {
            return cx.textures.jar(cx.r, cx.jar, &paths[0]);
        }
        let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
        return layered(cx, &refs);
    }
    let path = match kind {
        1 => "entity/horse/donkey",
        2 => "entity/horse/mule",
        3 => "entity/horse/horse_zombie",
        _ => "entity/horse/horse_skeleton",
    };
    cx.textures.jar(cx.r, cx.jar, path)
}

pub fn horse(e: &Entity, pos: Vec3, t: f32, anim: &Anim, body: f32, death: f32, cx: &mut Ctx) -> bool {
    let Some(tex) = horse_texture(cx, e) else { return true };
    let kind = e.meta_byte(19);
    let scale = match kind {
        1 => 0.87,
        2 => 0.92,
        _ => 1.0,
    };
    let m = living(pos, body, death, Vec3::splat(scale));
    let mut rig = Rig::new(&g::HORSE);
    pose_horse(&mut rig, e, t, anim, e.ridden_by.is_some());
    let (tint, light) = (cx.tint, cx.light);
    let mut sink = cx.sink(tex, tint, 255, light);
    let adult = !anim.child;
    let saddled = adult && flag(e, 4);
    let chested = adult && flag(e, 8);
    let mule = kind == 1 || kind == 2;
    let size = 0.5f32;
    let names = |v: &[&str]| -> Vec<usize> { v.iter().map(|n| rig.i(n)).collect() };
    if saddled {
        let saddle = names(&["horseFaceRopes", "horseSaddleBottom", "horseSaddleFront", "horseSaddleBack", "horseLeftSaddleRope", "horseLeftSaddleMetal", "horseRightSaddleRope", "horseRightSaddleMetal", "horseLeftFaceMetal", "horseRightFaceMetal"]);
        rig.emit(&mut sink, &m, &saddle);
    }
    let legs = names(&["backLeftLeg", "backLeftShin", "backLeftHoof", "backRightLeg", "backRightShin", "backRightHoof", "frontLeftLeg", "frontLeftShin", "frontLeftHoof", "frontRightLeg", "frontRightShin", "frontRightHoof"]);
    let trunk = names(&["body", "tailBase", "tailMiddle", "tailTip", "neck", "mane"]);
    let tr = |x: f32, y: f32, z: f32| Mat4::from_translation(Vec3::new(x, y, z));
    let (ml, mt) = if adult {
        (m, m)
    } else {
        (m * Mat4::from_scale(Vec3::new(size, 0.5 + size * 0.5, size)) * tr(0.0, 0.95 * (1.0 - size), 0.0), m * Mat4::from_scale(Vec3::splat(size)) * tr(0.0, 1.35 * (1.0 - size), 0.0))
    };
    rig.emit(&mut sink, &ml, &legs);
    rig.emit(&mut sink, &mt, &trunk);
    let ears = if mule { names(&["muleLeftEar", "muleRightEar"]) } else { names(&["horseLeftEar", "horseRightEar"]) };
    let head = names(&["head"]);
    let mh = if adult {
        m
    } else {
        let eat = e.mob_state.horse_lean_prev + (e.mob_state.horse_lean - e.mob_state.horse_lean_prev) * t;
        let s17 = 0.5 + size * size * 0.5;
        let off = if eat <= 0.0 { tr(0.0, 1.35 * (1.0 - size), 0.0) } else { tr(0.0, 0.9 * (1.0 - size) * eat + 1.35 * (1.0 - size) * (1.0 - eat), 0.15 * (1.0 - size) * eat) };
        m * Mat4::from_scale(Vec3::splat(s17)) * off
    };
    rig.emit(&mut sink, &mh, &ears);
    rig.emit(&mut sink, &mh, &head);
    if chested {
        let chests = names(&["muleLeftChest", "muleRightChest"]);
        rig.emit(&mut sink, &m, &chests);
    }
    true
}
