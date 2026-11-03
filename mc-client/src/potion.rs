//! Efeitos de poção ativos do jogador (S1D/S1E) e o painel do inventário
//! (`InventoryEffectRenderer`). Tabela de poções gerada de `Potion.java` (`gen_hud.rs`).

use crate::gen_hud::POTIONS;

pub const SPEED: u8 = 1;
pub const SLOWNESS: u8 = 2;
pub const CONFUSION: u8 = 9;
pub const REGENERATION: u8 = 10;
pub const BLINDNESS: u8 = 15;
pub const HUNGER: u8 = 17;
pub const POISON: u8 = 19;
pub const WITHER: u8 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Effect {
    pub id: u8,
    pub amplifier: i8,
    pub duration: i32,
    pub show_particles: bool,
    /// `S1D.func_149429_c()` (duração 32767): mostra `**:**`.
    pub duration_max: bool,
}

/// Dados de `Potion.potionTypes[id]`.
pub fn potion(id: u8) -> Option<&'static (u8, &'static str, bool, u32, &'static str, i32)> {
    POTIONS.iter().find(|p| p.0 == id)
}

/// `Potion.getStatusIconIndex` (−1 sem ícone, como poções instantâneas).
pub fn icon_index(id: u8) -> i32 {
    potion(id).map_or(-1, |p| p.5)
}

/// Pacote 0x1D: id da entidade (varint), efeito, amplificador, duração (varint), esconder partículas.
pub fn parse_add(body: &[u8]) -> mc_protocol::Result<(i32, Effect)> {
    let mut r = mc_protocol::Reader::new(body);
    let entity = r.varint()?;
    let id = r.u8()?;
    let amplifier = r.u8()? as i8;
    let duration = r.varint()?;
    let hide = r.bool()?;
    Ok((entity, Effect { id, amplifier, duration, show_particles: !hide, duration_max: duration == 32767 }))
}

/// Pacote 0x1E: id da entidade (varint), efeito.
pub fn parse_remove(body: &[u8]) -> mc_protocol::Result<(i32, u8)> {
    let mut r = mc_protocol::Reader::new(body);
    Ok((r.varint()?, r.u8()?))
}

/// `activePotionsMap` do jogador local.
#[derive(Default, Debug, Clone)]
pub struct Effects {
    list: Vec<Effect>,
}

impl Effects {
    /// `EntityLivingBase.addPotionEffect` + `PotionEffect.combine` (só quando o efeito já existe).
    pub fn add(&mut self, e: Effect) {
        if let Some(cur) = self.list.iter_mut().find(|c| c.id == e.id) {
            if e.amplifier > cur.amplifier {
                cur.amplifier = e.amplifier;
                cur.duration = e.duration;
            } else if e.amplifier == cur.amplifier && cur.duration < e.duration {
                cur.duration = e.duration;
            }
            cur.show_particles = e.show_particles;
            cur.duration_max = e.duration_max;
        } else {
            self.list.push(e);
        }
    }

    /// `removePotionEffectClient`.
    pub fn remove(&mut self, id: u8) {
        self.list.retain(|e| e.id != id);
    }

    pub fn clear(&mut self) {
        self.list.clear();
    }

    pub fn has(&self, id: u8) -> bool {
        self.list.iter().any(|e| e.id == id)
    }

    pub fn get(&self, id: u8) -> Option<&Effect> {
        self.list.iter().find(|e| e.id == id)
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Effect> {
        self.list.iter()
    }

    /// Na ordem do `activePotionsMap` (HashMap de ids pequenos: a dos ids).
    pub fn sorted(&self) -> Vec<&Effect> {
        let mut l: Vec<&Effect> = self.list.iter().collect();
        l.sort_by_key(|e| e.id);
        l
    }

    /// `updatePotionEffects` no cliente: a duração desce até 0 e o servidor manda remover.
    pub fn tick(&mut self) {
        for e in &mut self.list {
            if e.duration > 0 {
                e.duration -= 1;
            }
        }
    }
}

/// `StringUtils.ticksToElapsedTime`.
pub fn ticks_to_elapsed(ticks: i32) -> String {
    let mut s = ticks / 20;
    let m = s / 60;
    s %= 60;
    if s < 10 {
        format!("{m}:0{s}")
    } else {
        format!("{m}:{s}")
    }
}

/// `Potion.getDurationString`.
pub fn duration_string(e: &Effect) -> String {
    if e.duration_max {
        "**:**".to_string()
    } else {
        ticks_to_elapsed(e.duration)
    }
}

/// Uma linha do painel do `drawActivePotionEffects`.
#[derive(Clone, Debug, PartialEq)]
pub struct PanelRow {
    pub x: i32,
    pub y: i32,
    /// Ícone 18x18 em `inventory.png` (u, v), se a poção tem ícone.
    pub icon: Option<(i32, i32)>,
    pub name_key: &'static str,
    pub level_key: Option<&'static str>,
    pub duration: String,
}

/// `InventoryEffectRenderer.guiLeft` quando há efeitos (160 + (largura - xSize - 200) / 2).
pub fn gui_left(effects: &Effects, screen_w: i32, x_size: i32) -> i32 {
    if effects.is_empty() {
        (screen_w - x_size) / 2
    } else {
        160 + (screen_w - x_size - 200) / 2
    }
}

/// Posição das linhas do painel (`drawActivePotionEffects`): à esquerda do inventário.
pub fn panel(effects: &Effects, gui_left: i32, gui_top: i32) -> Vec<PanelRow> {
    let i = gui_left - 124;
    let mut j = gui_top;
    let n = effects.list.len() as i32;
    let step = if n > 5 { 132 / (n - 1) } else { 33 };
    let mut rows = Vec::new();
    // activePotionsMap é um HashMap<Integer, _>: com ids pequenos a ordem é a dos ids.
    let mut list: Vec<&Effect> = effects.list.iter().collect();
    list.sort_by_key(|e| e.id);
    for e in list {
        let Some(p) = potion(e.id) else { continue };
        let icon = (p.5 >= 0).then(|| (p.5 % 8 * 18, 198 + p.5 / 8 * 18));
        let level_key = match e.amplifier {
            1 => Some("enchantment.level.2"),
            2 => Some("enchantment.level.3"),
            3 => Some("enchantment.level.4"),
            _ => None,
        };
        rows.push(PanelRow { x: i, y: j, icon, name_key: p.4, level_key, duration: duration_string(e) });
        j += step;
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fx(id: u8, amp: i8, dur: i32) -> Effect {
        Effect { id, amplifier: amp, duration: dur, show_particles: true, duration_max: dur == 32767 }
    }

    #[test]
    fn parse_s1d_e_s1e() {
        // entidade 5, velocidade (1), amplificador 1, duração 1200 (varint 0xB0 0x09), partículas escondidas.
        let (e, fxs) = parse_add(&[5, 1, 1, 0xB0, 0x09, 1]).unwrap();
        assert_eq!(e, 5);
        assert_eq!(fxs, Effect { id: 1, amplifier: 1, duration: 1200, show_particles: false, duration_max: false });
        // 32767 = 0xFF 0xFF 0x01.
        let (_, m) = parse_add(&[5, 10, 0, 0xFF, 0xFF, 0x01, 0]).unwrap();
        assert!(m.duration_max);
        assert_eq!(parse_remove(&[5, 19]).unwrap(), (5, 19));
        assert!(parse_add(&[5, 1]).is_err());
    }

    #[test]
    fn tabela_gerada_do_java() {
        assert_eq!(potion(1).unwrap().4, "potion.moveSpeed");
        assert_eq!(icon_index(1), 0);
        assert_eq!(icon_index(10), 7);
        assert_eq!(icon_index(6), -1, "cura instantânea não tem ícone");
        assert!(potion(19).unwrap().2, "veneno é efeito ruim");
        assert_eq!(potion(20).unwrap().4, "potion.wither");
    }

    #[test]
    fn combine_e_tick() {
        let mut e = Effects::default();
        e.add(fx(1, 0, 100));
        e.add(fx(1, 0, 50)); // menor, não troca
        assert_eq!(e.get(1).unwrap().duration, 100);
        e.add(fx(1, 0, 300));
        assert_eq!(e.get(1).unwrap().duration, 300);
        e.add(fx(1, 1, 20)); // amplificador maior vence
        assert_eq!((e.get(1).unwrap().amplifier, e.get(1).unwrap().duration), (1, 20));
        e.tick();
        assert_eq!(e.get(1).unwrap().duration, 19);
        e.remove(1);
        assert!(e.is_empty());
        e.add(fx(9, 0, 1));
        e.tick();
        e.tick();
        assert_eq!(e.get(9).unwrap().duration, 0, "fica em 0 até o servidor remover");
    }

    #[test]
    fn duracao_em_texto() {
        assert_eq!(ticks_to_elapsed(1200), "1:00");
        assert_eq!(ticks_to_elapsed(1180), "0:59");
        assert_eq!(ticks_to_elapsed(20 * 125), "2:05");
        assert_eq!(duration_string(&fx(1, 0, 32767)), "**:**");
    }

    #[test]
    fn painel_do_inventario() {
        let mut e = Effects::default();
        assert_eq!(gui_left(&e, 427, 176), (427 - 176) / 2);
        e.add(fx(1, 0, 600));
        e.add(fx(10, 1, 400));
        assert_eq!(gui_left(&e, 427, 176), 160 + (427 - 176 - 200) / 2);
        let left = gui_left(&e, 427, 176);
        let rows = panel(&e, left, 40);
        assert_eq!(rows.len(), 2);
        assert_eq!((rows[0].x, rows[0].y), (left - 124, 40));
        assert_eq!(rows[1].y, 40 + 33);
        assert_eq!(rows[0].icon, Some((0, 198)));
        // Regeneração: ícone 7 -> (7*18, 198), nível II.
        assert_eq!(rows[1].icon, Some((126, 198)));
        assert_eq!(rows[1].level_key, Some("enchantment.level.2"));
        assert_eq!(rows[0].duration, "0:30");
        // Mais de 5 efeitos: espaçamento 132 / (n - 1).
        for id in [2, 3, 4, 8] {
            e.add(fx(id, 0, 100));
        }
        let rows = panel(&e, left, 0);
        assert_eq!(rows[1].y, 132 / 5);
    }
}
