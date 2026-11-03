//! Aviso de conquista (porte de `GuiAchievement` e da parte de conquistas de
//! `NetHandlerPlayClient.handleStatistics`).

use std::collections::HashMap;
use std::time::Instant;

use mc_protocol::nbt::Slot;
use mc_render::container_tex::ACHIEVEMENT_BG;
use mc_render::ui::UiBatch;

use crate::gen_achievements::ICONS;
use crate::lang::tr;

struct Toast {
    title: String,
    description: String,
    icon: (i16, i16),
    /// `notificationTime` em ms do relógio de `Achievements`.
    time: i64,
    permanent: bool,
}

/// Estado por conexão: o `StatFileWriter` das conquistas, o `field_147308_k` (já chegou o primeiro
/// 0x37) e o aviso na tela.
pub struct Achievements {
    unlocked: HashMap<String, i32>,
    received: bool,
    toast: Option<Toast>,
    epoch: Instant,
}

impl Default for Achievements {
    fn default() -> Self {
        Self { unlocked: HashMap::new(), received: false, toast: None, epoch: Instant::now() }
    }
}

fn icon_of(id: &str) -> (i16, i16) {
    ICONS.iter().find(|(n, _, _)| *n == id).map_or((0, 0), |&(_, i, m)| (i, m))
}

impl Achievements {
    /// `Minecraft.getSystemTime`.
    fn now(&self) -> i64 {
        self.epoch.elapsed().as_millis() as i64 + 1
    }

    /// `displayAchievement`.
    fn display(&mut self, id: &str) {
        self.toast = Some(Toast { title: tr("achievement.get"), description: tr(id), icon: icon_of(id), time: self.now(), permanent: false });
    }

    /// `displayUnformattedAchievement`: fica na tela, entrando depois de 2,5 s.
    fn display_unformatted(&mut self, id: &str, description: String) {
        self.toast = Some(Toast { title: tr(id), description, icon: icon_of(id), time: self.now() + 2500, permanent: true });
    }

    /// `handleStatistics`. Devolve se `showInventoryAchievementHint` deve ser desligado e gravado.
    /// `hint` é a descrição de `achievement.openInventory` com a tecla do inventário.
    pub fn statistics(&mut self, stats: &[(String, i32)], show_hint: bool, hint: impl Fn() -> String) -> bool {
        let mut any = false;
        let mut hint_off = false;
        for (name, value) in stats {
            if ICONS.iter().any(|(n, _, _)| n == name) && *value > 0 {
                if self.received && self.unlocked.get(name).copied().unwrap_or(0) == 0 {
                    self.display(name);
                    if name == "achievement.openInventory" {
                        hint_off = true;
                    }
                }
                any = true;
            }
            self.unlocked.insert(name.clone(), *value);
        }
        if !self.received && !any && show_hint {
            self.display_unformatted("achievement.openInventory", hint());
        }
        self.received = true;
        hint_off
    }

    /// Item do aviso, para o ícone ser montado antes do desenho.
    pub fn icon(&self) -> Option<Slot> {
        self.toast.as_ref().map(|t| Slot { id: t.icon.0, count: 1, damage: t.icon.1, nbt: None, raw_nbt: Vec::new() })
    }

    /// `updateAchievementWindow`, por cima de tudo (telas inclusive).
    pub fn draw(&mut self, ui: &mut UiBatch, w: i32, icons: &HashMap<crate::inventory::IconKey, u32>) {
        let now = self.now();
        let Some(t) = &self.toast else { return };
        let mut k = (now - t.time) as f64 / 3000.0;
        if !t.permanent {
            if !(0.0..=1.0).contains(&k) {
                self.toast = None;
                return;
            }
        } else if k > 0.5 {
            k = 0.5;
        }
        let mut d = k * 2.0;
        if d > 1.0 {
            d = 2.0 - d;
        }
        d = 1.0 - d * 4.0;
        if d < 0.0 {
            d = 0.0;
        }
        d = d * d;
        d = d * d;
        let x = w - 160;
        let y = -((d * 36.0) as i32);
        ui.gui(ACHIEVEMENT_BG, x as f32, y as f32, 96.0, 202.0, 160.0, 32.0);
        if t.permanent {
            // drawSplitString: linhas de 9 px sem sombra.
            for (i, l) in crate::gui::wrap(ui, &t.description, 120).iter().enumerate() {
                ui.text((x + 30) as f32, (y + 7 + 9 * i as i32) as f32, l, 0xFFFFFFFF, false);
            }
        } else {
            ui.text((x + 30) as f32, (y + 7) as f32, &t.title, 0xFFFFFF00, false);
            ui.text((x + 30) as f32, (y + 18) as f32, &t.description, 0xFFFFFFFF, false);
        }
        if let Some(s) = self.icon() {
            crate::gui_container::draw_stack(ui, &s, (x + 8) as f32, (y + 8) as f32, icons);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats(v: &[(&str, i32)]) -> Vec<(String, i32)> {
        v.iter().map(|(n, x)| (n.to_string(), *x)).collect()
    }

    #[test]
    fn primeiro_pacote_nao_avisa_e_os_seguintes_sim() {
        let mut a = Achievements::default();
        assert!(!a.statistics(&stats(&[("achievement.mineWood", 1)]), true, String::new));
        assert!(a.toast.is_none());
        // Já tinha: nada. Nova: aviso; a do inventário desliga a dica.
        a.statistics(&stats(&[("achievement.mineWood", 1)]), true, String::new);
        assert!(a.toast.is_none());
        assert!(a.statistics(&stats(&[("achievement.openInventory", 1)]), true, String::new));
        assert_eq!(a.toast.as_ref().map(|t| (t.icon, t.permanent)), Some(((340, 0), false)));
    }

    #[test]
    fn dica_do_inventario_sem_conquistas() {
        let mut a = Achievements::default();
        a.statistics(&stats(&[("stat.leaveGame", 3)]), true, || "abra".to_string());
        assert!(a.toast.as_ref().is_some_and(|t| t.permanent && t.description == "abra"));
        let mut b = Achievements::default();
        b.statistics(&stats(&[("stat.leaveGame", 3)]), false, String::new);
        assert!(b.toast.is_none());
    }
}
