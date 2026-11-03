//! Lista de jogadores do Tab (porte de `GuiPlayerTabOverlay`).

use std::cmp::Ordering;
use std::collections::HashMap;
use std::time::Instant;

use image::RgbaImage;
use mc_render::ui::{UiBatch, ICONS, ITEM_PX};

use crate::entity_net::Uuid;
use crate::gui;
use crate::hud::Scoreboard;

/// O que o tab usa de um `NetworkPlayerInfo`.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub uuid: Uuid,
    /// `GameProfile.getName`.
    pub profile: String,
    /// `getPlayerName`: nome de exibição ou nome com prefixo e sufixo do time.
    pub name: String,
    /// `getPlayerTeam().getRegisteredName()`, vazio sem time.
    pub team: String,
    pub ping: i32,
    pub spectator: bool,
    /// Vaga do atlas da GUI com a cabeça (rosto e chapéu) já composta.
    pub head: Option<u32>,
}

/// `PlayerComparator`: quem não é espectador primeiro, depois o time e o nome do perfil
/// (`String.compareTo` compara unidades UTF-16).
pub fn compare(a: &Entry, b: &Entry) -> Ordering {
    a.spectator
        .cmp(&b.spectator)
        .then_with(|| a.team.encode_utf16().cmp(b.team.encode_utf16()))
        .then_with(|| a.profile.encode_utf16().cmp(b.profile.encode_utf16()))
}

/// Campos de corações do `NetworkPlayerInfo` (`func_178835_l` e vizinhos).
#[derive(Clone, Copy, Debug, Default)]
struct Hearts {
    last_health: i32,
    display_health: i32,
    last_health_time: u64,
    health_blink_time: u64,
    render_visibility_id: u64,
}

pub struct TabList {
    /// Já ordenada por `compare`.
    pub entries: Vec<Entry>,
    /// `isIntegratedServerRunning() || getIsencrypted()`: com cabeças.
    pub heads: bool,
    last_time_opened: u64,
    being_rendered: bool,
    hearts: HashMap<Uuid, Hearts>,
    epoch: Instant,
}

impl Default for TabList {
    fn default() -> Self {
        Self { entries: Vec::new(), heads: false, last_time_opened: 0, being_rendered: false, hearts: HashMap::new(), epoch: Instant::now() }
    }
}

/// Objetivo do espaço 0 (lista): nome e se o tipo de exibição é `hearts`.
pub struct ListObjective<'a> {
    pub name: &'a str,
    pub hearts: bool,
}

fn score(sb: &Scoreboard, objective: &str, player: &str) -> i32 {
    sb.scores.get(objective).and_then(|m| m.get(player)).copied().unwrap_or(0)
}

fn width(ui: &mut UiBatch, s: &str) -> i32 {
    ui.text_width(s) as i32
}

/// `"" + float` do Java para meios inteiros (`10.0`, `9.5`).
fn java_half(v: i32) -> String {
    format!("{:.1}", v as f32 / 2.0)
}

impl TabList {
    /// `Minecraft.getSystemTime` (só diferenças e igualdades importam).
    fn now(&self) -> u64 {
        self.epoch.elapsed().as_millis() as u64 + 1
    }

    /// `updatePlayerList`.
    pub fn update(&mut self, shown: bool) {
        if shown && !self.being_rendered {
            self.last_time_opened = self.now();
        }
        self.being_rendered = shown;
    }

    /// `renderPlayerlist`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&mut self, ui: &mut UiBatch, screen_w: i32, sb: &Scoreboard, objective: Option<ListObjective>, header: &str, footer: &str, update_counter: u64) {
        let entries = std::mem::take(&mut self.entries);
        let mut name_w = 0;
        let mut score_w = 0;
        for e in &entries {
            name_w = name_w.max(width(ui, &e.name));
            if let Some(o) = objective.as_ref().filter(|o| !o.hearts) {
                score_w = score_w.max(width(ui, &format!(" {}", score(sb, o.name, &e.profile))));
            }
        }
        let shown = &entries[..entries.len().min(80)];
        let n = shown.len() as i32;
        let mut rows = n;
        let mut cols = 1;
        while rows > 20 {
            cols += 1;
            rows = (n + cols - 1) / cols;
        }
        let heads = self.heads;
        let obj_w = match &objective {
            Some(o) if o.hearts => 90,
            Some(_) => score_w,
            None => 0,
        };
        let cell = (cols * ((if heads { 9 } else { 0 }) + name_w + obj_w + 13)).min(screen_w - 50) / cols;
        let left = screen_w / 2 - (cell * cols + (cols - 1) * 5) / 2;
        let mut y = 10;
        let mut total = cell * cols + (cols - 1) * 5;
        let header = (!header.is_empty()).then(|| gui::wrap(ui, header, screen_w - 50));
        let footer = (!footer.is_empty()).then(|| gui::wrap(ui, footer, screen_w - 50));
        for l in header.iter().chain(footer.iter()).flatten() {
            total = total.max(width(ui, l));
        }
        let (bx0, bx1) = (screen_w / 2 - total / 2 - 1, screen_w / 2 + total / 2 + 1);
        let rect = |ui: &mut UiBatch, x0: i32, y0: i32, x1: i32, y1: i32, c: u32| ui.rect(x0 as f32, y0 as f32, (x1 - x0) as f32, (y1 - y0) as f32, c);
        if let Some(lines) = &header {
            rect(ui, bx0, y - 1, bx1, y + lines.len() as i32 * 9, 0x80000000);
            for l in lines {
                let lw = width(ui, l);
                ui.text((screen_w / 2 - lw / 2) as f32, y as f32, l, 0xFFFFFFFF, true);
                y += 9;
            }
            y += 1;
        }
        rect(ui, bx0, y - 1, bx1, y + rows * 9, 0x80000000);
        for (i, e) in shown.iter().enumerate() {
            let i = i as i32;
            let (col, row) = (i / rows, i % rows);
            let mut x = left + col * cell + col * 5;
            let yy = y + row * 9;
            rect(ui, x, yy, x + cell, yy + 8, 0x20FFFFFF);
            if heads {
                if let Some(slot) = e.head {
                    ui.item_icon(slot, x as f32, yy as f32, 8.0);
                }
                x += 9;
            }
            if e.spectator {
                ui.text(x as f32, yy as f32, &format!("§o{}", e.name), 0x90FFFFFF, true);
            } else {
                ui.text(x as f32, yy as f32, &e.name, 0xFFFFFFFF, true);
            }
            if let Some(o) = objective.as_ref().filter(|_| !e.spectator) {
                let sx = x + name_w + 1;
                let ex = sx + obj_w;
                if ex - sx > 5 {
                    self.draw_score(ui, sb, o, yy, e, sx, ex, update_counter);
                }
            }
            draw_ping(ui, cell, x - if heads { 9 } else { 0 }, yy, e.ping);
        }
        if let Some(lines) = &footer {
            y = y + rows * 9 + 1;
            rect(ui, bx0, y - 1, bx1, y + lines.len() as i32 * 9, 0x80000000);
            for l in lines {
                let lw = width(ui, l);
                ui.text((screen_w / 2 - lw / 2) as f32, y as f32, l, 0xFFFFFFFF, true);
                y += 9;
            }
        }
        self.entries = entries;
    }

    /// `drawScoreboardValues`.
    #[allow(clippy::too_many_arguments)]
    fn draw_score(&mut self, ui: &mut UiBatch, sb: &Scoreboard, o: &ListObjective, y: i32, e: &Entry, sx: i32, ex: i32, update_counter: u64) {
        let points = score(sb, o.name, &e.profile);
        if !o.hearts {
            let s = format!("§e{points}");
            let w = width(ui, &s);
            ui.text((ex - w) as f32, y as f32, &s, 0xFFFFFFFF, true);
            return;
        }
        let now = self.now();
        let opened = self.last_time_opened;
        let h = self.hearts.entry(e.uuid).or_default();
        if opened == h.render_visibility_id {
            if points < h.last_health {
                h.last_health_time = now;
                h.health_blink_time = update_counter + 20;
            } else if points > h.last_health {
                h.last_health_time = now;
                h.health_blink_time = update_counter + 10;
            }
        }
        if now.saturating_sub(h.last_health_time) > 1000 || opened != h.render_visibility_id {
            h.last_health = points;
            h.display_health = points;
            h.last_health_time = now;
        }
        h.render_visibility_id = opened;
        h.last_health = points;
        let display = h.display_health;
        let full = (points.max(display) as f32 / 2.0).ceil() as i32;
        let max_hearts = ((points / 2) as f32).ceil().max(((display / 2) as f32).ceil()).max(10.0) as i32;
        let blink = h.health_blink_time > update_counter && (h.health_blink_time - update_counter) / 3 % 2 == 1;
        if full <= 0 {
            return;
        }
        let step = ((ex - sx - 4) as f32 / max_hearts as f32).min(9.0);
        if step > 3.0 {
            let at = |i: i32| sx as f32 + i as f32 * step;
            let bg = if blink { 25.0 } else { 16.0 };
            for i in full..max_hearts {
                ui.gui(ICONS, at(i), y as f32, bg, 0.0, 9.0, 9.0);
            }
            for i in 0..full {
                ui.gui(ICONS, at(i), y as f32, bg, 0.0, 9.0, 9.0);
                if blink {
                    if i * 2 + 1 < display {
                        ui.gui(ICONS, at(i), y as f32, 70.0, 0.0, 9.0, 9.0);
                    }
                    if i * 2 + 1 == display {
                        ui.gui(ICONS, at(i), y as f32, 79.0, 0.0, 9.0, 9.0);
                    }
                }
                if i * 2 + 1 < points {
                    ui.gui(ICONS, at(i), y as f32, if i >= 10 { 160.0 } else { 52.0 }, 0.0, 9.0, 9.0);
                }
                if i * 2 + 1 == points {
                    ui.gui(ICONS, at(i), y as f32, if i >= 10 { 169.0 } else { 61.0 }, 0.0, 9.0, 9.0);
                }
            }
        } else {
            let f = (points as f32 / 20.0).clamp(0.0, 1.0);
            let color = (((1.0 - f) * 255.0) as u32) << 16 | ((f * 255.0) as u32) << 8;
            let mut s = java_half(points);
            if ex - width(ui, &format!("{s}hp")) >= sx {
                s.push_str("hp");
            }
            let w = width(ui, &s);
            ui.text(((ex + sx) / 2 - w / 2) as f32, y as f32, &s, color, true);
        }
    }
}

/// `drawPing`.
fn draw_ping(ui: &mut UiBatch, cell: i32, x: i32, y: i32, ping: i32) {
    let bars = match ping {
        p if p < 0 => 5,
        p if p < 150 => 0,
        p if p < 300 => 1,
        p if p < 600 => 2,
        p if p < 1000 => 3,
        _ => 4,
    };
    ui.gui(ICONS, (x + cell - 11) as f32, y as f32, 0.0, 176.0 + bars as f32 * 8.0, 10.0, 8.0);
}

/// Cabeça do tab em `ITEM_PX`: o `drawScaledCustomSizeModalRect` do rosto (8, 8) e, com o chapéu
/// vestido, o da camada (40, 8), de cabeça para baixo para Dinnerbone/Grumm. Pixels com alfa até
/// 0,1 somem (`GL_GREATER 0.1`); as camadas se juntam com a mistura 770/771.
pub fn head_image(skin: &RgbaImage, hat: bool, flip: bool) -> RgbaImage {
    let k = (skin.width() / 64).max(1);
    let mut out = RgbaImage::new(ITEM_PX, ITEM_PX);
    let layers: &[u32] = if hat { &[8, 40] } else { &[8] };
    for &u in layers {
        for y in 0..ITEM_PX {
            let row = y * 8 * k / ITEM_PX;
            let sy = if flip { 16 * k - 1 - row } else { 8 * k + row };
            for x in 0..ITEM_PX {
                let sx = u * k + x * 8 * k / ITEM_PX;
                if sx >= skin.width() || sy >= skin.height() {
                    continue;
                }
                let src = *skin.get_pixel(sx, sy);
                if src[3] as f32 / 255.0 <= 0.1 {
                    continue;
                }
                let dst = out.get_pixel_mut(x, y);
                let (sa, da) = (src[3] as f32 / 255.0, dst[3] as f32 / 255.0);
                let oa = sa + da * (1.0 - sa);
                for c in 0..3 {
                    dst[c] = ((src[c] as f32 * sa + dst[c] as f32 * da * (1.0 - sa)) / oa).round() as u8;
                }
                dst[3] = (oa * 255.0).round() as u8;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(profile: &str, team: &str, spectator: bool) -> Entry {
        Entry { uuid: Uuid(0), profile: profile.into(), name: profile.into(), team: team.into(), ping: 0, spectator, head: None }
    }

    #[test]
    fn ordem_do_player_comparator() {
        let mut v = vec![entry("b", "", true), entry("z", "a", false), entry("a", "b", false), entry("c", "a", false)];
        v.sort_by(compare);
        let names: Vec<&str> = v.iter().map(|e| e.profile.as_str()).collect();
        assert_eq!(names, ["c", "z", "a", "b"]);
    }

    #[test]
    fn cabeca_com_chapeu_e_invertida() {
        let mut skin = RgbaImage::new(64, 64);
        // Rosto: linha 8 vermelha, linha 15 azul; chapéu cobre só o pixel (40, 8).
        for x in 8..16 {
            skin.put_pixel(x, 8, image::Rgba([255, 0, 0, 255]));
            skin.put_pixel(x, 15, image::Rgba([0, 0, 255, 255]));
        }
        skin.put_pixel(40, 8, image::Rgba([0, 255, 0, 255]));
        let px = ITEM_PX / 8;
        let normal = head_image(&skin, true, false);
        assert_eq!(normal.get_pixel(0, 0).0, [0, 255, 0, 255]);
        assert_eq!(normal.get_pixel(px, 0).0, [255, 0, 0, 255]);
        assert_eq!(normal.get_pixel(0, ITEM_PX - 1).0, [0, 0, 255, 255]);
        let sem_chapeu = head_image(&skin, false, false);
        assert_eq!(sem_chapeu.get_pixel(0, 0).0, [255, 0, 0, 255]);
        let invertida = head_image(&skin, false, true);
        assert_eq!(invertida.get_pixel(0, 0).0, [0, 0, 255, 255]);
    }

    #[test]
    fn meio_coracao_como_float_do_java() {
        assert_eq!(java_half(20), "10.0");
        assert_eq!(java_half(19), "9.5");
    }
}
