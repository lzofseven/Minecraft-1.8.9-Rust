use super::*;

fn st(id: i16, count: u8) -> Option<Slot> {
    Some(Slot { id, count, damage: 0, nbt: None, raw_nbt: Vec::new() })
}

fn win(kind: WinKind) -> (Container, Vec<Option<Slot>>) {
    let c = Container::new(kind);
    let n = c.len();
    (c, vec![None; n])
}

const SURVIVAL: Env = Env { creative: false, level: 0 };
const CREATIVE: Env = Env { creative: true, level: 0 };

fn count(slots: &[Option<Slot>], i: usize) -> u8 {
    slots[i].as_ref().map_or(0, |s| s.count)
}

fn pos(c: &Container, i: usize) -> (i32, i32) {
    (c.defs[i].x, c.defs[i].y)
}

// ---------------------------------------------------------------- layout (números dos construtores Java)

#[test]
fn window_sizes() {
    for (k, n) in [
        (WinKind::Player, 45),
        (WinKind::Workbench, 46),
        (WinKind::Furnace, 39),
        (WinKind::Dispenser, 45),
        (WinKind::Hopper, 41),
        (WinKind::Enchant, 38),
        (WinKind::Brewing, 40),
        (WinKind::Anvil, 39),
        (WinKind::Beacon, 37),
        (WinKind::Merchant, 39),
        (WinKind::Horse { chested: false }, 38),
        (WinKind::Horse { chested: true }, 53),
        (WinKind::Chest { rows: 3 }, 63),
        (WinKind::Chest { rows: 6 }, 90),
    ] {
        assert_eq!(defs_for(k).len(), n, "{k:?}");
    }
}

#[test]
fn furnace_layout_matches_java() {
    let c = Container::new(WinKind::Furnace);
    assert_eq!([pos(&c, 0), pos(&c, 1), pos(&c, 2)], [(56, 17), (56, 53), (116, 35)]);
    assert_eq!(c.defs[1].role, Role::FurnaceFuel);
    assert_eq!(c.defs[2].role, Role::FurnaceOutput);
    assert_eq!(pos(&c, 3), (8, 84));
    assert_eq!(pos(&c, 38), (8 + 8 * 18, 142));
}

#[test]
fn player_layout_matches_java() {
    let c = Container::new(WinKind::Player);
    assert_eq!(pos(&c, 0), (144, 36));
    assert_eq!([pos(&c, 1), pos(&c, 2), pos(&c, 3), pos(&c, 4)], [(88, 26), (106, 26), (88, 44), (106, 44)]);
    assert_eq!([pos(&c, 5), pos(&c, 6), pos(&c, 7), pos(&c, 8)], [(8, 8), (8, 26), (8, 44), (8, 62)]);
    assert_eq!(c.defs[5].role, Role::Armor(0));
    assert_eq!(c.defs[8].role, Role::Armor(3));
    assert_eq!(pos(&c, 9), (8, 84));
    assert_eq!(pos(&c, 36), (8, 142));
    assert!(c.defs[5].player_inv && !c.defs[1].player_inv);
}

#[test]
fn crafting_dispenser_hopper_layouts() {
    let w = Container::new(WinKind::Workbench);
    assert_eq!((pos(&w, 0), pos(&w, 1), pos(&w, 9)), ((124, 35), (30, 17), (30 + 36, 17 + 36)));
    let d = Container::new(WinKind::Dispenser);
    assert_eq!((pos(&d, 0), pos(&d, 8)), ((62, 17), (62 + 36, 17 + 36)));
    let h = Container::new(WinKind::Hopper);
    assert_eq!((pos(&h, 0), pos(&h, 4), pos(&h, 5), pos(&h, 32)), ((44, 20), (44 + 72, 20), (8, 51), (8, 109)));
}

#[test]
fn brewing_enchant_anvil_beacon_merchant_horse_layouts() {
    let b = Container::new(WinKind::Brewing);
    assert_eq!([pos(&b, 0), pos(&b, 1), pos(&b, 2), pos(&b, 3)], [(56, 46), (79, 53), (102, 46), (79, 17)]);
    assert_eq!((b.defs[0].role, b.defs[3].role), (Role::BrewPotion, Role::BrewIngredient));
    let e = Container::new(WinKind::Enchant);
    assert_eq!([pos(&e, 0), pos(&e, 1)], [(15, 47), (35, 47)]);
    assert_eq!((e.defs[0].role, e.defs[1].role), (Role::EnchantItem, Role::Lapis));
    let a = Container::new(WinKind::Anvil);
    assert_eq!([pos(&a, 0), pos(&a, 1), pos(&a, 2)], [(27, 47), (76, 47), (134, 47)]);
    assert_eq!(a.defs[2].role, Role::AnvilOutput);
    let k = Container::new(WinKind::Beacon);
    assert_eq!((pos(&k, 0), pos(&k, 1), pos(&k, 28)), ((136, 110), (36, 137), (36, 195)));
    let m = Container::new(WinKind::Merchant);
    assert_eq!([pos(&m, 0), pos(&m, 1), pos(&m, 2)], [(36, 53), (62, 53), (120, 53)]);
    assert_eq!(m.defs[2].role, Role::MerchantResult);
    let h = Container::new(WinKind::Horse { chested: true });
    assert_eq!((pos(&h, 0), pos(&h, 1), pos(&h, 2), pos(&h, 16)), ((8, 18), (8, 36), (80, 18), (80 + 72, 18 + 36)));
    assert_eq!(pos(&h, 17), (8, 102 - 18)); // (3 - 4) * 18 = -18
    let h0 = Container::new(WinKind::Horse { chested: false });
    assert_eq!(pos(&h0, 2), (8, 102 - 18));
}

#[test]
fn chest_layout_depends_on_rows() {
    let c = Container::new(WinKind::Chest { rows: 3 });
    assert_eq!(pos(&c, 0), (8, 18));
    assert_eq!(pos(&c, 27), (8, 103 + (3 - 4) * 18));
    assert_eq!(pos(&c, 62), (8 + 8 * 18, 161 + (3 - 4) * 18));
    let c6 = Container::new(WinKind::Chest { rows: 6 });
    assert_eq!(pos(&c6, 54), (8, 103 + 36));
}

// ---------------------------------------------------------------- validade e limites

#[test]
fn slot_validity() {
    let (c, mut slots) = win(WinKind::Player);
    // capacete de ferro (306) só no 5; peitoral (307) no 6; abóbora (86) e cabeça (397) no 5.
    assert!(c.valid(&slots, 5, st(306, 1).as_ref().unwrap()));
    assert!(!c.valid(&slots, 6, st(306, 1).as_ref().unwrap()));
    assert!(c.valid(&slots, 6, st(307, 1).as_ref().unwrap()));
    assert!(c.valid(&slots, 5, st(86, 1).as_ref().unwrap()));
    assert!(c.valid(&slots, 5, st(397, 1).as_ref().unwrap()));
    assert!(!c.valid(&slots, 0, st(1, 1).as_ref().unwrap())); // resultado da criação
    assert_eq!(c.limit(5, st(306, 1).as_ref().unwrap()), 1);
    slots[0] = st(1, 1);
    let (f, fs) = win(WinKind::Furnace);
    assert!(f.valid(&fs, 1, st(263, 1).as_ref().unwrap())); // carvão
    assert!(f.valid(&fs, 1, st(325, 1).as_ref().unwrap())); // balde
    assert!(!f.valid(&fs, 1, st(1, 1).as_ref().unwrap())); // pedra não queima
    assert_eq!(f.limit(1, st(325, 1).as_ref().unwrap()), 1);
    assert!(!f.valid(&fs, 2, st(263, 1).as_ref().unwrap()));
    let (b, bs) = win(WinKind::Brewing);
    assert!(b.valid(&bs, 0, st(373, 1).as_ref().unwrap()) && b.valid(&bs, 0, st(374, 1).as_ref().unwrap()));
    assert!(!b.valid(&bs, 0, st(1, 1).as_ref().unwrap()));
    assert!(b.valid(&bs, 3, st(372, 1).as_ref().unwrap())); // fungo do Nether
    assert!(!b.valid(&bs, 3, st(1, 1).as_ref().unwrap()));
    let (k, ks) = win(WinKind::Beacon);
    assert!(k.valid(&ks, 0, st(388, 1).as_ref().unwrap()) && !k.valid(&ks, 0, st(1, 1).as_ref().unwrap()));
    let (e, es) = win(WinKind::Enchant);
    let lapis = Slot { id: 351, count: 1, damage: 4, nbt: None, raw_nbt: Vec::new() };
    assert!(e.valid(&es, 1, &lapis));
    assert!(!e.valid(&es, 1, st(351, 1).as_ref().unwrap()));
    let (h, hs) = win(WinKind::Horse { chested: false });
    assert!(h.valid(&hs, 0, st(329, 1).as_ref().unwrap()));
    assert!(!h.valid(&hs, 1, st(417, 1).as_ref().unwrap())); // cavalo sem armadura
}

#[test]
fn fuel_and_smelting_tables_come_from_the_original() {
    assert_eq!(burn_time(st(263, 1).as_ref().unwrap()), 1600); // carvão
    assert_eq!(burn_time(st(327, 1).as_ref().unwrap()), 20000); // balde de lava
    assert_eq!(burn_time(st(280, 1).as_ref().unwrap()), 100); // graveto
    assert_eq!(burn_time(st(5, 1).as_ref().unwrap()), 300); // tábuas
    assert_eq!(burn_time(st(1, 1).as_ref().unwrap()), 0);
    assert!(is_smeltable(st(15, 1).as_ref().unwrap())); // minério de ferro
    assert!(is_smeltable(st(12, 1).as_ref().unwrap())); // areia
    assert!(!is_smeltable(st(1, 1).as_ref().unwrap()) || is_smeltable(st(4, 1).as_ref().unwrap()));
}

#[test]
fn item_info_matches_java() {
    assert_eq!(max_stack(st(1, 1).as_ref().unwrap()), 64);
    assert_eq!(max_stack(st(332, 1).as_ref().unwrap()), 16); // bola de neve
    assert_eq!(max_stack(st(277, 1).as_ref().unwrap()), 1); // espada de diamante
    assert_eq!(max_stack(st(373, 1).as_ref().unwrap()), 1); // poção
    assert!(!is_stackable(&Slot { id: 277, count: 1, damage: 0, nbt: None, raw_nbt: Vec::new() }));
    assert!(has_subtypes(351));
}

// ---------------------------------------------------------------- slotClick

fn chest() -> (Container, Vec<Option<Slot>>) {
    win(WinKind::Chest { rows: 3 })
}

#[test]
fn mode0_left_click_picks_swaps_and_merges() {
    let (mut c, mut s) = chest();
    let mut cur = None;
    s[2] = st(1, 10);
    let r = c.slot_click(&mut s, &mut cur, SURVIVAL, 2, 0, 0);
    assert_eq!(r.map(|x| x.count), Some(10)); // item que estava no slot antes
    assert_eq!(cur.as_ref().map(|x| x.count), Some(10));
    assert!(s[2].is_none());
    // coloca em slot vazio
    c.slot_click(&mut s, &mut cur, SURVIVAL, 4, 0, 0);
    assert!(cur.is_none() && count(&s, 4) == 10);
    // troca com outro item
    s[5] = st(2, 3);
    cur = st(1, 7);
    c.slot_click(&mut s, &mut cur, SURVIVAL, 5, 0, 0);
    assert_eq!((cur.as_ref().unwrap().id, count(&s, 5), s[5].as_ref().unwrap().id), (2, 7, 1));
    // junta pilhas iguais respeitando 64
    s[6] = st(3, 60);
    cur = st(3, 10);
    c.slot_click(&mut s, &mut cur, SURVIVAL, 6, 0, 0);
    assert_eq!((count(&s, 6), cur.as_ref().unwrap().count), (64, 6));
}

#[test]
fn mode0_right_click_halves_and_places_one() {
    let (mut c, mut s) = chest();
    let mut cur = None;
    s[0] = st(1, 11);
    c.slot_click(&mut s, &mut cur, SURVIVAL, 0, 1, 0);
    assert_eq!((cur.as_ref().unwrap().count, count(&s, 0)), (6, 5)); // (11 + 1) / 2 fica no cursor
    c.slot_click(&mut s, &mut cur, SURVIVAL, 1, 1, 0);
    assert_eq!((cur.as_ref().unwrap().count, count(&s, 1)), (5, 1));
    c.slot_click(&mut s, &mut cur, SURVIVAL, 1, 1, 0);
    assert_eq!((cur.as_ref().unwrap().count, count(&s, 1)), (4, 2));
}

#[test]
fn click_outside_drops_cursor() {
    let (mut c, mut s) = chest();
    let mut cur = st(1, 5);
    c.slot_click(&mut s, &mut cur, SURVIVAL, -999, 1, 0);
    assert_eq!(cur.as_ref().unwrap().count, 4);
    c.slot_click(&mut s, &mut cur, SURVIVAL, -999, 0, 0);
    assert!(cur.is_none());
}

#[test]
fn mode1_shift_click_moves_between_halves() {
    let (mut c, mut s) = chest();
    let mut cur = None;
    s[3] = st(1, 20);
    let r = c.slot_click(&mut s, &mut cur, SURVIVAL, 3, 0, 1);
    assert_eq!(r.map(|x| x.count), Some(20));
    // vai para o fim (reverse): último slot da hotbar
    assert!(s[3].is_none());
    assert_eq!(count(&s, 62), 20);
    // de volta (jogador -> baú): primeiro vazio do baú
    c.slot_click(&mut s, &mut cur, SURVIVAL, 62, 0, 1);
    assert_eq!(count(&s, 0), 20);
}

#[test]
fn mode1_fills_existing_stack_first() {
    let (mut c, mut s) = chest();
    let mut cur = None;
    s[30] = st(1, 60);
    s[1] = st(1, 10);
    c.slot_click(&mut s, &mut cur, SURVIVAL, 1, 0, 1);
    assert_eq!(count(&s, 30), 64);
    assert_eq!(count(&s, 62), 6);
}

#[test]
fn furnace_shift_click_routes_smeltables_and_fuel() {
    let (mut c, mut s) = win(WinKind::Furnace);
    let mut cur = None;
    s[3] = st(15, 5); // minério de ferro
    c.slot_click(&mut s, &mut cur, SURVIVAL, 3, 0, 1);
    assert_eq!(count(&s, 0), 5);
    s[4] = st(263, 5); // carvão
    c.slot_click(&mut s, &mut cur, SURVIVAL, 4, 0, 1);
    assert_eq!(count(&s, 1), 5);
    s[5] = st(1, 5); // pedra: nem derrete nem queima -> vai para a hotbar
    c.slot_click(&mut s, &mut cur, SURVIVAL, 5, 0, 1);
    assert_eq!(count(&s, 30), 5);
    // saída -> inventário
    s[2] = st(265, 3);
    c.slot_click(&mut s, &mut cur, SURVIVAL, 2, 0, 1);
    assert!(s[2].is_none());
    assert_eq!(s[3..39].iter().flatten().filter(|x| x.id == 265).map(|x| x.count as u32).sum::<u32>(), 3);
}

#[test]
fn player_shift_click_equips_armor() {
    let (mut c, mut s) = win(WinKind::Player);
    let mut cur = None;
    s[10] = st(306, 1); // capacete de ferro
    c.slot_click(&mut s, &mut cur, SURVIVAL, 10, 0, 1);
    assert!(s[10].is_none());
    assert_eq!(s[5].as_ref().map(|x| x.id), Some(306));
    // um segundo capacete (slot 5 ocupado) vai para a hotbar, como mergeItemStack(36, 45)
    s[11] = st(310, 1);
    c.slot_click(&mut s, &mut cur, SURVIVAL, 11, 0, 1);
    assert_eq!(s[36].as_ref().map(|x| x.id), Some(310));
}

#[test]
fn crafting_result_shift_click_goes_to_inventory_and_uses_ingredients() {
    let (mut c, mut s) = win(WinKind::Workbench);
    let mut cur = None;
    s[0] = st(5, 4);
    s[1] = st(17, 3);
    c.slot_click(&mut s, &mut cur, SURVIVAL, 0, 0, 1);
    // retrySlotClick com a receita recalculada a cada tronco gasto: 3 troncos, 3 vezes 4 tábuas.
    assert!(s[0].is_none());
    assert_eq!(count(&s, 45), 12);
    assert_eq!(count(&s, 1), 0);
}

#[test]
fn anvil_output_needs_xp() {
    let (mut c, mut s) = win(WinKind::Anvil);
    let mut cur = None;
    s[2] = st(276, 1);
    c.props[0] = 5;
    c.slot_click(&mut s, &mut cur, Env { creative: false, level: 4 }, 2, 0, 0);
    assert!(cur.is_none() && s[2].is_some());
    c.slot_click(&mut s, &mut cur, Env { creative: false, level: 5 }, 2, 0, 0);
    assert_eq!(cur.as_ref().map(|x| x.id), Some(276));
    c.props[0] = 0;
    s[2] = st(276, 1);
    cur = None;
    c.slot_click(&mut s, &mut cur, SURVIVAL, 2, 0, 0); // custo 0: não pode pegar
    assert!(cur.is_none());
}

#[test]
fn cannot_place_in_invalid_slots() {
    let (mut c, mut s) = win(WinKind::Furnace);
    let mut cur = st(1, 5);
    c.slot_click(&mut s, &mut cur, SURVIVAL, 2, 0, 0); // saída
    assert!(s[2].is_none() && cur.is_some());
    c.slot_click(&mut s, &mut cur, SURVIVAL, 1, 0, 0); // combustível: pedra não queima
    assert!(s[1].is_none());
    c.slot_click(&mut s, &mut cur, SURVIVAL, 0, 0, 0); // entrada aceita qualquer coisa
    assert_eq!(count(&s, 0), 5);
}

#[test]
fn armor_slot_takes_one_item() {
    let (mut c, mut s) = win(WinKind::Player);
    let mut cur = st(306, 1);
    c.slot_click(&mut s, &mut cur, SURVIVAL, 5, 0, 0);
    assert_eq!(s[5].as_ref().map(|x| x.id), Some(306));
    assert!(cur.is_none());
    cur = st(307, 1);
    c.slot_click(&mut s, &mut cur, SURVIVAL, 5, 0, 0); // peitoral não entra no capacete
    assert_eq!(s[5].as_ref().map(|x| x.id), Some(306));
    assert_eq!(cur.as_ref().map(|x| x.id), Some(307));
}

#[test]
fn mode2_number_key_swaps_with_hotbar() {
    let (mut c, mut s) = chest();
    let mut cur = None;
    s[0] = st(1, 5);
    s[54 + 2] = st(2, 3); // hotbar 2 (os 9 últimos: 54..63)
    c.slot_click(&mut s, &mut cur, SURVIVAL, 0, 2, 2);
    // slot do baú não é do jogador e o item da hotbar é válido: procura vaga; troca pelo primeiro vazio
    assert_eq!(s[54 + 2].as_ref().map(|x| x.id), Some(1));
    assert!(s[0].is_none());
    assert_eq!(s[27..63].iter().flatten().filter(|x| x.id == 2).count(), 1);
    // hotbar vazia: o item vai e o slot esvazia
    let (mut c2, mut s2) = chest();
    s2[1] = st(4, 9);
    c2.slot_click(&mut s2, &mut cur, SURVIVAL, 1, 0, 2);
    assert_eq!(s2[54].as_ref().map(|x| x.count), Some(9));
    assert!(s2[1].is_none());
    // slot de baú vazio recebe o item da hotbar
    c2.slot_click(&mut s2, &mut cur, SURVIVAL, 7, 0, 2);
    assert_eq!(s2[7].as_ref().map(|x| x.count), Some(9));
    assert!(s2[54].is_none());
}

#[test]
fn mode3_clones_in_creative_only() {
    let (mut c, mut s) = chest();
    let mut cur = None;
    s[0] = st(1, 3);
    c.slot_click(&mut s, &mut cur, SURVIVAL, 0, 2, 3);
    assert!(cur.is_none());
    c.slot_click(&mut s, &mut cur, CREATIVE, 0, 2, 3);
    assert_eq!(cur.as_ref().map(|x| x.count), Some(64));
    assert_eq!(count(&s, 0), 3);
}

#[test]
fn mode4_drops_one_or_all() {
    let (mut c, mut s) = chest();
    let mut cur = None;
    s[0] = st(1, 8);
    c.slot_click(&mut s, &mut cur, SURVIVAL, 0, 0, 4);
    assert_eq!(count(&s, 0), 7);
    c.slot_click(&mut s, &mut cur, SURVIVAL, 0, 1, 4);
    assert!(s[0].is_none());
}

fn drag(c: &mut Container, s: &mut Vec<Option<Slot>>, cur: &mut Option<Slot>, slots: &[i32], btn: i32) {
    // GuiContainer: botão 0 -> limite 0 (dividir), 1 -> 1 (um em cada), 2 -> 2 (cheio, criativo)
    let lim = btn;
    c.slot_click(s, cur, CREATIVE, -999, lim << 2, 5);
    for &i in slots {
        c.slot_click(s, cur, CREATIVE, i, 1 | (lim << 2), 5);
    }
    c.slot_click(s, cur, CREATIVE, -999, 2 | (lim << 2), 5);
}

#[test]
fn mode5_drag_split_evenly() {
    let (mut c, mut s) = chest();
    let mut cur = st(1, 10);
    drag(&mut c, &mut s, &mut cur, &[0, 1, 2], 0);
    assert_eq!((count(&s, 0), count(&s, 1), count(&s, 2)), (3, 3, 3));
    assert_eq!(cur.as_ref().unwrap().count, 1);
}

#[test]
fn mode5_drag_one_each_and_full() {
    let (mut c, mut s) = chest();
    let mut cur = st(1, 10);
    drag(&mut c, &mut s, &mut cur, &[0, 1, 2, 3], 1);
    assert_eq!((count(&s, 0), count(&s, 3)), (1, 1));
    assert_eq!(cur.as_ref().unwrap().count, 6);
    let (mut c, mut s) = chest();
    let mut cur = st(1, 64);
    drag(&mut c, &mut s, &mut cur, &[0, 1], 2); // modo 2 (meio): só criativo
    assert_eq!((count(&s, 0), count(&s, 1)), (64, 64));
    assert!(cur.is_none()); // j = 64 - 128 <= 0 -> cursor vazio, como no Java
}

#[test]
fn mode5_drag_skips_invalid_and_full_slots() {
    let (mut c, mut s) = win(WinKind::Furnace);
    let mut cur = st(1, 9);
    drag(&mut c, &mut s, &mut cur, &[0, 1, 2], 0); // 1 (combustível) e 2 (saída) rejeitam pedra
    assert_eq!(count(&s, 0), 9 / 1); // restou só o slot 0 na lista: recebe tudo
    assert!(s[1].is_none() && s[2].is_none());
    assert!(cur.is_none());
}

#[test]
fn mode5_survival_cannot_use_mode2() {
    let (mut c, mut s) = chest();
    let mut cur = st(1, 64);
    c.slot_click(&mut s, &mut cur, SURVIVAL, -999, 2 << 2, 5);
    assert_eq!(c.drag.event, 0);
}

#[test]
fn mode6_double_click_gathers_matching_stacks() {
    let (mut c, mut s) = chest();
    let mut cur = st(1, 10);
    s[0] = st(1, 20);
    s[5] = st(1, 30);
    s[6] = st(2, 5);
    s[40] = st(1, 5);
    c.slot_click(&mut s, &mut cur, SURVIVAL, 7, 0, 6); // slot 7 vazio
    assert_eq!(cur.as_ref().unwrap().count, 64);
    // primeiro passe pega só pilhas incompletas, na ordem dos slots: 0 (20) e 5 (30) -> 10+20+30 = 60, depois 40 (5) -> 64
    assert!(s[0].is_none() && s[5].is_none());
    assert_eq!(count(&s, 40), 1);
    assert_eq!(count(&s, 6), 5);
}

// ---------------------------------------------------------------- troca com aldeão

fn trade(b1: (i16, u8), b2: Option<(i16, u8)>, sell: (i16, u8)) -> Trade {
    let mk = |(id, count): (i16, u8)| Slot { id, count, damage: 0, nbt: None, raw_nbt: Vec::new() };
    Trade { buy1: mk(b1), buy2: b2.map(mk), sell: mk(sell), disabled: false, uses: 0, max_uses: 7 }
}

#[test]
fn merchant_computes_result_slot() {
    let (mut c, mut s) = win(WinKind::Merchant);
    c.trades = vec![trade((388, 1), None, (297, 4)), trade((265, 2), Some((388, 1)), (264, 1))];
    let mut cur = None;
    s[3] = st(388, 5); // esmeraldas no inventário
    c.slot_click(&mut s, &mut cur, SURVIVAL, 3, 0, 0);
    c.slot_click(&mut s, &mut cur, SURVIVAL, 0, 0, 0); // põe no slot de compra
    assert_eq!(s[2].as_ref().map(|x| (x.id, x.count)), Some((297, 4)));
    // pegar o resultado consome o preço
    c.slot_click(&mut s, &mut cur, SURVIVAL, 2, 0, 0);
    assert_eq!(cur.as_ref().map(|x| (x.id, x.count)), Some((297, 4)));
    assert_eq!(s[0].as_ref().map(|x| x.count), Some(4));
}

#[test]
fn merchant_selected_trade_with_two_ingredients() {
    let trades = vec![trade((388, 1), None, (297, 4)), trade((265, 2), Some((388, 1)), (264, 1))];
    let a = st(265, 2).unwrap();
    let b = st(388, 1).unwrap();
    assert_eq!(can_recipe_be_used(&trades, &a, Some(&b), 0), Some(1));
    assert_eq!(can_recipe_be_used(&trades, &a, None, 0), None);
    // índice > 0 usa só a troca escolhida
    assert_eq!(can_recipe_be_used(&trades, &a, Some(&b), 1), Some(1));
    let e = st(388, 1).unwrap();
    assert_eq!(can_recipe_be_used(&trades, &e, None, 1), None);
    assert_eq!(can_recipe_be_used(&trades, &e, None, 0), Some(0));
}

// ---------------------------------------------------------------- enchant / brewing transfer

#[test]
fn enchant_shift_click_puts_one_item_in_slot_zero() {
    let (mut c, mut s) = win(WinKind::Enchant);
    let mut cur = None;
    s[2] = st(276, 1);
    c.slot_click(&mut s, &mut cur, SURVIVAL, 2, 0, 1);
    assert_eq!(s[0].as_ref().map(|x| x.id), Some(276));
    // lápis vai para o slot 1
    s[3] = Some(Slot { id: 351, count: 10, damage: 4, nbt: None, raw_nbt: Vec::new() });
    c.slot_click(&mut s, &mut cur, SURVIVAL, 3, 0, 1);
    assert_eq!(count(&s, 1), 10);
}

#[test]
fn brewing_shift_click_routes_by_item() {
    let (mut c, mut s) = win(WinKind::Brewing);
    let mut cur = None;
    s[4] = st(372, 3); // ingrediente
    c.slot_click(&mut s, &mut cur, SURVIVAL, 4, 0, 1);
    assert_eq!(count(&s, 3), 3);
    s[5] = st(373, 1); // poção
    c.slot_click(&mut s, &mut cur, SURVIVAL, 5, 0, 1);
    assert_eq!(s[0].as_ref().map(|x| x.id), Some(373));
}

#[test]
fn hopper_and_beacon_shift_click() {
    let (mut c, mut s) = win(WinKind::Hopper);
    let mut cur = None;
    s[5] = st(1, 10);
    c.slot_click(&mut s, &mut cur, SURVIVAL, 5, 0, 1);
    assert_eq!(count(&s, 0), 10);
    let (mut c, mut s) = win(WinKind::Beacon);
    s[1] = st(388, 1);
    c.slot_click(&mut s, &mut cur, SURVIVAL, 1, 0, 1);
    assert_eq!(s[0].as_ref().map(|x| x.id), Some(388));
    s[2] = st(388, 2); // mais de 1: não entra no farol; vai para a hotbar (28..37)
    c.slot_click(&mut s, &mut cur, SURVIVAL, 2, 0, 1);
    assert_eq!(count(&s, 28), 2);
}

#[test]
fn horse_saddle_slot_shift_click() {
    let (mut c, mut s) = win(WinKind::Horse { chested: true });
    let mut cur = None;
    s[17] = st(329, 1);
    c.slot_click(&mut s, &mut cur, SURVIVAL, 17, 0, 1);
    assert_eq!(s[0].as_ref().map(|x| x.id), Some(329));
    s[18] = st(1, 5);
    c.slot_click(&mut s, &mut cur, SURVIVAL, 18, 0, 1);
    assert_eq!(count(&s, 2), 5); // baú do cavalo
}

#[test]
fn clicked_item_sent_to_server_is_the_slot_before_the_click() {
    let (mut c, mut s) = chest();
    let mut cur = None;
    s[0] = st(1, 9);
    let sent = c.slot_click(&mut s, &mut cur, SURVIVAL, 0, 1, 0);
    assert_eq!(sent.map(|x| x.count), Some(9));
    let none = c.slot_click(&mut s, &mut cur, SURVIVAL, 9, 0, 0);
    assert!(none.is_none());
}
