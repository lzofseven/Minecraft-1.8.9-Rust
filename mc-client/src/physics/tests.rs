//! Paridade com as contas do Java (tabelas geradas por scripts/paridade/Paridade.java)
//! e casos de colisão do `moveEntity`.

use super::*;
use mc_world::{ChunkData, Section};

#[allow(dead_code)]
mod paridade {
    include!("paridade.rs");
}

/// Chão de pedra com o topo em y = 64 (blocos em y = 63), 5x5 colunas.
fn flat_world() -> World {
    let mut w = World::new(true);
    for cx in -2..=2 {
        for cz in -2..=4 {
            let mut s = Box::new(Section::empty(true));
            for z in 0..16 {
                for x in 0..16 {
                    s.blocks[Section::index(x, 15, z)] = 1 << 4;
                }
            }
            w.apply(ChunkData { x: cx, z: cz, full: true, mask: 1 << 3, sections: vec![(3, s)], biomes: None });
        }
    }
    w
}

/// Jogador parado no chão (vy de equilíbrio do chão, como no jogo).
fn on_floor(x: f64, z: f64) -> Player {
    let mut p = Player::default();
    p.set_position(DVec3::new(x, 64.0, z));
    p.on_ground = true;
    p.vel.y = (0.0 - 0.08) * 0.9800000190734863;
    p
}

fn in_air(y: f64) -> Player {
    let mut p = Player::default();
    p.set_position(DVec3::new(0.0, y, 0.0));
    p
}

/// Roda `expected.len()` ticks e compara a posição bit a bit com o Java.
fn check(name: &str, mut p: Player, keys: Keys, expected: &[[f64; 3]]) {
    let w = flat_world();
    let t = BlockTable::new();
    for (i, e) in expected.iter().enumerate() {
        p.tick(&w, &t, &keys);
        let got = [p.pos.x, p.pos.y, p.pos.z];
        assert_eq!(got, *e, "{name}: tick {i} difere do Java");
    }
}

#[test]
fn andar_igual_ao_java() {
    check("andar", on_floor(0.0, 0.0), Keys { forward: true, ..Default::default() }, &paridade::ANDAR);
    // Velocidade de cruzeiro: 0,2158 bloco/tick (4,317 m/s).
    let v = paridade::ANDAR[39][2] - paridade::ANDAR[38][2];
    assert!((v - 0.21585).abs() < 1e-4, "{v}");
}

#[test]
fn correr_igual_ao_java() {
    check("correr", on_floor(0.0, 0.0), Keys { forward: true, sprint: true, ..Default::default() }, &paridade::CORRER);
    let v = paridade::CORRER[39][2] - paridade::CORRER[38][2];
    assert!((v - 0.2806).abs() < 1e-4, "{v}");
}

#[test]
fn agachar_igual_ao_java() {
    check("agachar", on_floor(0.0, 0.0), Keys { forward: true, sneak: true, ..Default::default() }, &paridade::AGACHAR);
}

#[test]
fn pulo_igual_ao_java() {
    check("pular", on_floor(0.0, 0.0), Keys { jump: true, ..Default::default() }, &paridade::PULAR);
    let top = paridade::PULAR.iter().map(|p| p[1]).fold(0.0, f64::max) - 64.0;
    // Na 1.8 o vy abaixo de 0,005 zera: o topo é 1,249 (na 1.9+ é 1,252).
    assert!((top - 1.2491870787446828).abs() < 1e-12, "{top}");
}

#[test]
fn correr_pulando_igual_ao_java() {
    check("correr pulando", on_floor(0.0, 0.0), Keys { forward: true, sprint: true, jump: true, ..Default::default() }, &paridade::CORRER_PULANDO);
}

#[test]
fn queda_igual_ao_java() {
    check("cair", in_air(84.0), Keys::default(), &paridade::CAIR);
}

#[test]
fn voo_igual_ao_java() {
    let fly = || {
        let mut p = in_air(70.0);
        p.flying = true;
        p.allow_flying = true;
        p
    };
    check("voar", fly(), Keys { forward: true, ..Default::default() }, &paridade::VOAR);
    check("voar correndo", fly(), Keys { forward: true, sprint: true, ..Default::default() }, &paridade::VOAR_CORRENDO);
    check("voo subir", fly(), Keys { jump: true, ..Default::default() }, &paridade::VOO_SUBIR);
}

/// O bug relatado: segurar Shift voando desce e, ao tocar o chão, o voo desliga
/// e o pacote de habilidades sai no mesmo tick.
#[test]
fn shift_desce_e_desliga_o_voo() {
    let w = flat_world();
    let t = BlockTable::new();
    let mut p = in_air(66.0);
    p.flying = true;
    p.allow_flying = true;
    let keys = Keys { sneak: true, ..Default::default() };
    let mut landed_at = None;
    for (i, e) in paridade::VOO_DESCER.iter().enumerate() {
        let out = p.tick(&w, &t, &keys);
        assert_eq!([p.pos.x, p.pos.y, p.pos.z], *e, "tick {i}");
        if out.abilities {
            landed_at.get_or_insert(i);
        }
        if i == 0 {
            // C0B "começou a agachar" vai antes do movimento.
            assert_eq!(out.actions, vec![0]);
        }
    }
    let i = landed_at.expect("o voo desligou");
    assert!(!p.flying && p.on_ground, "tick {i}");
}

#[test]
fn duplo_espaco_liga_o_voo_e_corrida_dupla() {
    let w = flat_world();
    let t = BlockTable::new();
    let mut p = on_floor(0.0, 0.0);
    p.allow_flying = true;
    let none = Keys::default();
    let jump = Keys { jump: true, ..Default::default() };
    // Toque, solta, toque: dentro dos 7 ticks do `flyToggleTimer`.
    assert!(!p.tick(&w, &t, &jump).abilities);
    p.tick(&w, &t, &none);
    let out = p.tick(&w, &t, &jump);
    assert!(out.abilities && p.flying);

    // Duplo W: corre no segundo toque dentro de 7 ticks.
    let mut p = on_floor(0.0, 0.0);
    let fwd = Keys { forward: true, ..Default::default() };
    p.tick(&w, &t, &fwd);
    assert!(!p.sprinting);
    p.tick(&w, &t, &none);
    let out = p.tick(&w, &t, &fwd);
    assert!(p.sprinting && out.actions == vec![3]);
    // Soltar o W encerra a corrida.
    let out = p.tick(&w, &t, &none);
    assert!(!p.sprinting && out.actions == vec![4]);
}

#[test]
fn fome_baixa_nao_deixa_correr() {
    let w = flat_world();
    let t = BlockTable::new();
    let mut p = on_floor(0.0, 0.0);
    p.food_level = 6;
    p.tick(&w, &t, &Keys { forward: true, sprint: true, ..Default::default() });
    assert!(!p.sprinting);
}

#[test]
fn degrau_de_laje_e_parede() {
    let mut w = flat_world();
    let t = BlockTable::new();
    w.set_block(0, 64, 3, 44 << 4);
    w.set_block(3, 64, 0, 1 << 4);
    w.set_block(3, 65, 0, 1 << 4);
    let fwd = Keys { forward: true, ..Default::default() };
    let mut p = on_floor(0.5, 0.5);
    let mut top: f64 = 0.0;
    for _ in 0..40 {
        p.tick(&w, &t, &fwd);
        top = top.max(p.pos.y);
    }
    assert!(p.pos.z > 4.0 && top == 64.5, "subiu na laje: {:?}, topo {top}", p.pos);

    let mut p = on_floor(0.5, 0.5);
    p.yaw = -90.0;
    for _ in 0..40 {
        p.tick(&w, &t, &fwd);
    }
    assert!((p.pos.x - (3.0 - HALF_WIDTH)).abs() < 1e-9 && p.collided_h, "parou na parede: {:?}", p.pos);
}

#[test]
fn agachado_nao_cai_da_beirada() {
    let mut w = flat_world();
    let t = BlockTable::new();
    // Buraco de 2 blocos na frente.
    for z in 2..4 {
        for x in -2..3 {
            w.set_block(x, 63, z, 0);
        }
    }
    let mut p = on_floor(0.5, 0.5);
    for _ in 0..60 {
        p.tick(&w, &t, &Keys { forward: true, sneak: true, ..Default::default() });
    }
    assert!(p.pos.y == 64.0 && p.pos.z < 2.0 + HALF_WIDTH, "{:?}", p.pos);
}

#[test]
fn gelo_e_areia_das_almas() {
    let mut w = flat_world();
    let t = BlockTable::new();
    for z in -2..30 {
        w.set_block(0, 63, z, 79 << 4);
    }
    let fwd = Keys { forward: true, ..Default::default() };
    let mut ice = on_floor(0.5, 0.5);
    let mut stone = on_floor(5.5, 0.5);
    for _ in 0..30 {
        ice.tick(&w, &t, &fwd);
        stone.tick(&w, &t, &fwd);
    }
    // No gelo (0,98) a aceleração é menor, mas soltando a tecla ele desliza muito mais.
    let (zi, zs) = (ice.pos.z, stone.pos.z);
    for _ in 0..20 {
        ice.tick(&w, &t, &Keys::default());
        stone.tick(&w, &t, &Keys::default());
    }
    assert!(ice.pos.z - zi > 3.0 * (stone.pos.z - zs), "deslize: {} x {}", ice.pos.z - zi, stone.pos.z - zs);

    let mut w = flat_world();
    for z in -2..30 {
        w.set_block(0, 63, z, 88 << 4);
    }
    let mut sand = on_floor(0.5, 0.5);
    sand.set_position(DVec3::new(0.5, 63.875, 0.5));
    let mut stone = on_floor(5.5, 0.5);
    for _ in 0..30 {
        sand.tick(&w, &t, &fwd);
        stone.tick(&w, &t, &fwd);
    }
    assert!(sand.pos.z < stone.pos.z * 0.6, "alma mais lenta: {} x {}", sand.pos.z, stone.pos.z);
}

#[test]
fn agua_puxa_para_baixo_e_espaco_sobe() {
    let mut w = flat_world();
    let t = BlockTable::new();
    for y in 64..70 {
        for x in -3..4 {
            for z in -3..4 {
                w.set_block(x, y, z, 9 << 4);
            }
        }
    }
    let mut p = in_air(67.0);
    for _ in 0..10 {
        p.tick(&w, &t, &Keys::default());
    }
    assert!(p.in_water && p.vel.y < 0.0);
    let y = p.pos.y;
    for _ in 0..20 {
        p.tick(&w, &t, &Keys { jump: true, ..Default::default() });
    }
    assert!(p.pos.y > y, "nadou para cima: {} -> {}", y, p.pos.y);
}

#[test]
fn escada_sobe_ao_encostar() {
    let mut w = flat_world();
    let t = BlockTable::new();
    for y in 64..70 {
        w.set_block(0, y, 1, 65 << 4 | 2);
        w.set_block(0, y, 2, 1 << 4);
    }
    let mut p = on_floor(0.5, 1.5);
    for _ in 0..20 {
        p.tick(&w, &t, &Keys { forward: true, ..Default::default() });
    }
    assert!(p.pos.y > 65.0, "subiu: {:?}", p.pos);
}

#[test]
fn fov_de_corrida_e_voo() {
    let mut p = Player::default();
    assert_eq!(p.fov_modifier(), 1.0);
    p.set_sprinting(true);
    // (0,13/0,1 + 1)/2 = 1,15.
    assert!((p.fov_modifier() - 1.15).abs() < 1e-6);
    p.flying = true;
    assert!((p.fov_modifier() - 1.1 * 1.15).abs() < 1e-6);
}

/// Montado: o Shift vai no C0C (é assim que o servidor sabe que é para descer/desmontar)
/// e o C0B de agachar não sai; a posição segue o assento.
#[test]
fn montado_manda_c0c_com_shift() {
    let w = flat_world();
    let t = BlockTable::new();
    let mut p = on_floor(0.0, 0.0);
    p.riding = Some(7);
    p.mount = Some(DVec3::new(3.0, 70.0, 3.0));
    let out = p.tick(&w, &t, &Keys { sneak: true, forward: true, ..Default::default() });
    assert_eq!(out.movement, Some(MovePacket::Look));
    assert!(out.actions.is_empty());
    // moveForward depois do *0.98, com o 0.3 do agachar.
    assert_eq!(out.steer, Some((0.0, (1.0f64 * 0.3) as f32 * 0.98, false, true)));
    assert_eq!(p.pos, DVec3::new(3.0, 70.0 - 0.35, 3.0));
    // Desmontou: o próximo tick a pé manda o C0B de agachar.
    p.riding = None;
    let out = p.tick(&w, &t, &Keys { sneak: true, ..Default::default() });
    assert_eq!(out.actions, vec![0]);
}
