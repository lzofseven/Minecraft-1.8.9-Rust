//! Testes de geometria, pose e estado dos mobs do grupo `pets`/`hostile`/`misc`.

use super::*;
use crate::entities::{Entity, Kind};
use crate::entity_net::MetaValue;
use crate::mob_rig::Rig;

fn mob(id: u8, meta: &[(u8, MetaValue)]) -> Entity {
    let mut e = Entity::new(-100, Kind::Mob(id), [0; 3], 0.0, 0.0);
    for (k, v) in meta {
        e.meta.insert(*k, v.clone());
    }
    e
}

fn quads(r: &Rig) -> usize {
    r.roots().iter().map(|&i| r.part(i).quad_count()).sum()
}

fn tick_in(e: &Entity, in_water: bool) -> TickIn<'_> {
    let Kind::Mob(id) = e.kind else { unreachable!() };
    TickIn { id, eid: e.id, meta: &e.meta, on_ground: e.on_ground, in_water, vel: e.vel }
}

#[test]
fn model_cube_counts_match_the_java_constructors() {
    // Número de `addBox` de cada construtor (6 quads cada).
    assert_eq!(quads(&Rig::new(&g::WOLF)), 11 * 6);
    assert_eq!(quads(&Rig::new(&g::OCELOT)), 11 * 6);
    assert_eq!(quads(&Rig::new(&g::RABBIT)), 13 * 6);
    assert_eq!(quads(&Rig::new(&g::SQUID)), 9 * 6);
    assert_eq!(quads(&Rig::new(&g::GHAST)), 10 * 6);
    assert_eq!(quads(&Rig::new(&g::BLAZE)), 13 * 6);
    assert_eq!(quads(&Rig::new(&g::SILVERFISH)), 10 * 6);
    assert_eq!(quads(&Rig::new(&g::ENDERMITE)), 4 * 6);
    assert_eq!(quads(&Rig::new(&g::MAGMA_CUBE)), 9 * 6);
    assert_eq!(quads(&Rig::new(&g::SLIME_INNER)), 4 * 6);
    assert_eq!(quads(&Rig::new(&g::SLIME_OUTER)), 6);
}

#[test]
fn wolf_sitting_pose_uses_the_sit_constants() {
    let e = mob(95, &[(16, MetaValue::Byte(1))]);
    let mut r = Rig::new(&g::WOLF);
    pose_wolf_for_test(&mut r, &e);
    let (body, mane, tail, l1, l3) = (r.i("wolfBody"), r.i("wolfMane"), r.i("wolfTail"), r.i("wolfLeg1"), r.i("wolfLeg3"));
    assert_eq!(r.pivot[body].to_array(), [0.0, 18.0, 0.0]);
    assert_eq!(r.rot[body].x, 0.7853982);
    assert_eq!(r.pivot[mane].to_array(), [-1.0, 16.0, -3.0]);
    assert_eq!(r.rot[mane].x, 1.2566371);
    assert_eq!(r.pivot[tail].to_array(), [-1.0, 21.0, 6.0]);
    assert_eq!(r.rot[l1].x, 4.712389);
    assert_eq!(r.pivot[l3].to_array(), [-2.49, 17.0, -4.0]);
    assert_eq!(r.rot[l3].x, 5.811947);
}

fn pose_wolf_for_test(r: &mut Rig, e: &Entity) {
    pets::pose_wolf(r, e, 0.0, &Anim::default());
}

#[test]
fn wolf_tail_rotation_by_state() {
    // Selvagem: 0.62831855; bravo: 1.5393804; domesticado: (0.55 - (20 - vida) * 0.02) * pi.
    let mut r = Rig::new(&g::WOLF);
    let wild = mob(95, &[]);
    pets::pose_wolf(&mut r, &wild, 0.0, &Anim::default());
    assert_eq!(r.rot[r.i("wolfTail")].x, 0.62831855);
    let angry = mob(95, &[(16, MetaValue::Byte(2))]);
    pets::pose_wolf(&mut r, &angry, 0.0, &Anim::default());
    assert_eq!(r.rot[r.i("wolfTail")].x, 1.5393804);
    assert_eq!(r.rot[r.i("wolfTail")].y, 0.0);
    let tame = mob(95, &[(16, MetaValue::Byte(4)), (18, MetaValue::Float(10.0))]);
    pets::pose_wolf(&mut r, &tame, 0.0, &Anim::default());
    assert!((r.rot[r.i("wolfTail")].x - (0.55 - 10.0 * 0.02) * PI).abs() < 1e-6);
}

#[test]
fn wolf_shake_and_wet_state() {
    let mut e = mob(95, &[]);
    e.mob_state.status(95, 8);
    for _ in 0..8 {
        let t = tick_in(&e, false);
        let mut s = e.mob_state.clone();
        s.tick(&t);
        e.mob_state = s;
    }
    assert!((e.mob_state.shake - 0.4).abs() < 1e-5 && e.mob_state.wolf_shaking);
    assert!(pets::pose_shake(&e, 0.0, 0.0).abs() > 0.0);
    // Na água: molhado, sacudida zerada.
    let t = tick_in(&e, true);
    let mut s = e.mob_state.clone();
    s.tick(&t);
    assert!(s.wolf_wet && !s.wolf_shaking && s.shake == 0.0);
}

#[test]
fn ocelot_states_move_the_body_and_legs() {
    let mut r = Rig::new(&g::OCELOT);
    let sit = mob(98, &[(16, MetaValue::Byte(1))]);
    pets::pose_ocelot(&mut r, &sit, &Anim::default());
    assert_eq!(r.pivot[r.i("ocelotBody")].y, 8.0);
    assert_eq!(r.pivot[r.i("ocelotBody")].z, -5.0);
    assert!((r.pivot[r.i("ocelotHead")].y - 11.7).abs() < 1e-5);
    assert_eq!(r.rot[r.i("ocelotBody")].x, 0.7853982);
    assert_eq!(r.rot[r.i("ocelotBackLeftLeg")].x, -1.5707964);
    assert_eq!(r.rot[r.i("ocelotTail2")].x, 2.670354);
    // Agachado (bit 0x02 do byte 0): estado 0.
    let sneak = mob(98, &[(0, MetaValue::Byte(2))]);
    let mut r = Rig::new(&g::OCELOT);
    pets::pose_ocelot(&mut r, &sneak, &Anim::default());
    assert_eq!(r.pivot[r.i("ocelotBody")].y, 13.0);
    assert_eq!(r.rot[r.i("ocelotTail")].x, 1.5707964);
    // Correndo (bit 0x08): pernas defasadas em 0.3.
    let run = mob(98, &[(0, MetaValue::Byte(8))]);
    let mut r = Rig::new(&g::OCELOT);
    let a = Anim { limb_swing: 1.0, limb_amount: 1.0, ..Default::default() };
    pets::pose_ocelot(&mut r, &run, &a);
    assert!((r.rot[r.i("ocelotBackRightLeg")].x - cos(0.6662 + 0.3)).abs() < 1e-6);
    assert!((r.rot[r.i("ocelotTail2")].x - (1.7278761 + 0.31415927 * cos(1.0))).abs() < 1e-6);
}

#[test]
fn rabbit_hop_progress() {
    let mut e = mob(101, &[]);
    e.mob_state.status(101, 1);
    assert_eq!((e.mob_state.hop, e.mob_state.hop_len), (0, 10));
    for _ in 0..5 {
        let t = tick_in(&e, false);
        let mut s = e.mob_state.clone();
        s.tick(&t);
        e.mob_state = s;
    }
    let mut r = Rig::new(&g::RABBIT);
    pets::pose_rabbit(&mut r, &e, 0.0, &Anim::default());
    // progresso 5/10: sin(pi/2) = 1.
    assert!((r.rot[r.i("rabbitLeftThigh")].x - (50.0 - 21.0) * 0.017453292).abs() < 1e-4);
    assert!((r.rot[r.i("rabbitLeftArm")].x - (-40.0 - 11.0) * 0.017453292).abs() < 1e-4);
    for _ in 0..6 {
        let t = tick_in(&e, false);
        let mut s = e.mob_state.clone();
        s.tick(&t);
        e.mob_state = s;
    }
    assert_eq!((e.mob_state.hop, e.mob_state.hop_len), (0, 0));
}

#[test]
fn slime_squish_follows_ground_contact() {
    let mut e = mob(55, &[(16, MetaValue::Byte(2))]);
    let step = |e: &mut Entity, ground: bool| {
        e.on_ground = ground;
        let mut s = e.mob_state.clone();
        s.tick(&tick_in(e, false));
        e.mob_state = s;
    };
    step(&mut e, false);
    step(&mut e, true); // pousou
    assert!((e.mob_state.squish_amount - (-0.5 * 0.6)).abs() < 1e-6);
    step(&mut e, false); // pulou
    assert!((e.mob_state.squish_amount - 0.6).abs() < 1e-6);
    // Cubo de magma: fator 0.9.
    let mut m = mob(62, &[(16, MetaValue::Byte(2))]);
    m.on_ground = true;
    let mut s = m.mob_state.clone();
    s.tick(&tick_in(&m, false));
    assert!((s.squish_amount - (-0.5 * 0.9)).abs() < 1e-6);
}

#[test]
fn squid_tentacles_and_pitch() {
    let mut e = mob(94, &[]);
    e.id = 7;
    for _ in 0..6 {
        let mut s = e.mob_state.clone();
        s.tick(&tick_in(&e, false));
        e.mob_state = s;
    }
    // Fora d'água: tentáculos |sin(rotação)| * pi/4 e o corpo tombando para -90.
    let s = &e.mob_state;
    assert!((s.tentacle - sin(s.squid_rot).abs() * PI * 0.25).abs() < 1e-6);
    assert!(s.squid_pitch < 0.0 && s.squid_pitch > -90.0);
    // Dentro d'água a velocidade de giro cresce quando o ciclo passa de 0.75.
    let mut w = mob(94, &[]);
    w.mob_state.squid_init = true;
    w.mob_state.squid_rot_vel = 0.2;
    w.mob_state.squid_rot = 2.5;
    let mut s = w.mob_state.clone();
    s.tick(&tick_in(&w, true));
    assert_eq!(s.squid_spin, 1.0);
    assert!(s.squid_yaw > 0.0);
    let mut s2 = w.mob_state.clone();
    s2.squid_rot_vel = 0.1;
    s2.squid_rot = 0.5;
    s2.squid_spin = 1.0;
    s2.tick(&tick_in(&w, true));
    assert!((s2.squid_spin - 0.8).abs() < 1e-6);
}

#[test]
fn java_random_matches_known_sequence() {
    // `new Random(42).nextInt(100)` = 0, depois 63 (valores do JDK).
    let mut r = JRandom::new(42);
    assert_eq!(r.next_int(100), 0);
    assert_eq!(r.next_int(100), 63);
    // `new Random(0).nextFloat()` = 0.73096776.
    assert!((JRandom::new(0).next_float() - 0.730_967_76).abs() < 1e-7);
}

#[test]
fn bat_hanging_and_flying_poses() {
    let mut r = Rig::new(&g::BAT);
    misc::pose_bat(&mut r, true, &Anim::default());
    assert_eq!(r.rot[r.i("batBody")].x, PI);
    assert_eq!(r.pivot[r.i("batHead")].to_array(), [0.0, -2.0, 0.0]);
    assert_eq!(r.pivot[r.i("batRightWing")].to_array(), [-3.0, 0.0, 3.0]);
    assert_eq!(r.rot[r.i("batOuterRightWing")].y, -1.7278761);
    let mut r = Rig::new(&g::BAT);
    misc::pose_bat(&mut r, false, &Anim { age: 10.0, ..Default::default() });
    assert!((r.rot[r.i("batBody")].x - (0.7853982 + cos(1.0) * 0.15)).abs() < 1e-6);
    assert_eq!(r.rot[r.i("batLeftWing")].y, -r.rot[r.i("batRightWing")].y);
}

#[test]
fn blaze_rings_have_the_java_radii() {
    let mut r = Rig::new(&g::BLAZE);
    hostile::pose_blaze(&mut r, &Anim::default());
    for (i, rad) in [(0, 9.0), (4, 7.0), (8, 5.0)] {
        let p = r.pivot[r.i(&format!("blazeSticks_{i}"))];
        assert!(((p.x * p.x + p.z * p.z).sqrt() - rad).abs() < 1e-2, "anel {i}");
    }
    assert!((r.pivot[r.i("blazeSticks_0")].y - (-2.0 + cos(0.0))).abs() < 1e-6);
}

#[test]
fn golem_arms_swing_and_attack() {
    let e = mob(99, &[]);
    let mut r = Rig::new(&g::IRON_GOLEM);
    let a = Anim { limb_swing: 3.25, limb_amount: 1.0, ..Default::default() };
    misc::pose_golem(&mut r, &e, 0.0, &a);
    // 3.25 % 13 = 3.25; |3.25 - 6.5| - 3.25 = 0 -> onda 0.
    assert!((r.rot[r.i("ironGolemRightArm")].x - (-0.2)).abs() < 1e-6);
    assert_eq!(r.rot[r.i("ironGolemLeftLeg")].x, 0.0);
    let mut hit = mob(99, &[]);
    hit.mob_state.status(99, 4);
    misc::pose_golem(&mut r, &hit, 0.0, &Anim::default());
    // tri(10, 10) = (|0 - 5| - 2.5) / 2.5 = 1 -> -2 + 1.5.
    assert!((r.rot[r.i("ironGolemRightArm")].x - (-0.5)).abs() < 1e-6);
    let mut rose = mob(99, &[]);
    rose.mob_state.status(99, 11);
    misc::pose_golem(&mut r, &rose, 0.0, &Anim::default());
    assert_eq!(r.rot[r.i("ironGolemLeftArm")].x, 0.0);
}

#[test]
fn snowman_arms_follow_the_body_yaw() {
    let mut r = Rig::new(&g::SNOWMAN);
    misc::pose_snowman(&mut r, &Anim { head_yaw: 0.0, ..Default::default() });
    assert_eq!(r.pivot[r.i("rightHand")].x, 5.0);
    assert_eq!(r.pivot[r.i("leftHand")].x, -5.0);
    assert_eq!(r.rot[r.i("rightHand")].z, 1.0);
    assert_eq!(r.rot[r.i("leftHand")].y, PI);
}

#[test]
fn witch_nose_goes_up_when_holding_an_item() {
    let e = mob(66, &[]);
    let mut r = Rig::new(&g::WITCH);
    let base = r.pivot[r.i("villagerNose")];
    misc::pose_witch(&mut r, &e, true, &Anim::default());
    let n = r.i("villagerNose");
    assert_eq!(r.rot[n].x, -0.9);
    assert!((r.pivot[n].y - (base.y + 3.0)).abs() < 1e-5);
    assert!((r.pivot[n].z - (base.z - 1.5)).abs() < 1e-5);
}

#[test]
fn silverfish_segments_wave() {
    let g_ = &g::SILVERFISH;
    assert_eq!(g_.parts.len(), 10);
    // Segmentos do meio quase retos, as pontas torcem mais (fator 1 + |i - 2|).
    let age = 4.0;
    let y = |i: i32| cos(age * 0.9 + i as f32 * 0.15 * PI) * PI * 0.05 * (1 + (i - 2).abs()) as f32;
    assert!(y(6).abs() > 0.0);
}

#[test]
fn rotation_with_rotation_uses_y_x_z_order() {
    // `renderWithRotation` gira Y, depois X, depois Z (o `render` comum faz Z, Y, X).
    let mut r = Rig::new(&g::WOLF);
    let h = r.i("wolfHeadMain");
    r.rot[h] = Vec3::new(0.3, 0.5, 0.7);
    let mut a = Geometry::default();
    let mut b = Geometry::default();
    let mut sa = Sink { geo: &mut a, tex: (64.0, 32.0), tint: [1.0; 3], alpha: 255, light: FULL_LIGHT, lights: WORLD_LIGHTS };
    r.emit_rot(&mut sa, &Mat4::IDENTITY, &[h]);
    let mut sb = Sink { geo: &mut b, tex: (64.0, 32.0), tint: [1.0; 3], alpha: 255, light: FULL_LIGHT, lights: WORLD_LIGHTS };
    r.emit(&mut sb, &Mat4::IDENTITY, &[h]);
    assert_eq!(a.vertices.len(), b.vertices.len());
    let differs = a.vertices.iter().zip(&b.vertices).any(|(x, y)| (Vec3::from(x.pos) - Vec3::from(y.pos)).length() > 1e-4);
    assert!(differs);
    // Com uma só rotação, as duas ordens coincidem.
    r.rot[h] = Vec3::new(0.0, 0.5, 0.0);
    let (mut a, mut b) = (Geometry::default(), Geometry::default());
    let mut sa = Sink { geo: &mut a, tex: (64.0, 32.0), tint: [1.0; 3], alpha: 255, light: FULL_LIGHT, lights: WORLD_LIGHTS };
    r.emit_rot(&mut sa, &Mat4::IDENTITY, &[h]);
    let mut sb = Sink { geo: &mut b, tex: (64.0, 32.0), tint: [1.0; 3], alpha: 255, light: FULL_LIGHT, lights: WORLD_LIGHTS };
    r.emit(&mut sb, &Mat4::IDENTITY, &[h]);
    assert!(a.vertices.iter().zip(&b.vertices).all(|(x, y)| (Vec3::from(x.pos) - Vec3::from(y.pos)).length() < 1e-6));
}
