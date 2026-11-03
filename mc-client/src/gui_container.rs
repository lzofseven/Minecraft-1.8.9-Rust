//! Desenho e widgets das telas de container (`GuiContainer` e subclasses): fundo de cada tela, slots,
//! texto, barras de progresso, runas do encantamento, botões do farol e da troca, campo de nome da bigorna.

use std::collections::HashMap;

use mc_protocol::nbt::Slot;
use mc_render::container_tex as ct;
use mc_render::ui::{UiBatch, CHEST, INVENTORY};

use crate::containers::{Role, WinKind};
use crate::enchant::BookAnim;
use crate::gen_container_items::BEACON_EFFECTS;
use crate::gui::{self, TextField};
use crate::inventory::*;
use crate::lang::Lang;

const TEXT: u32 = 0x404040;

/// Botão do farol (`GuiBeacon.Button`): confirmar, cancelar ou um efeito.
#[derive(Clone, Debug)]
pub struct BeaconButton {
    pub id: i32,
    pub x: i32,
    pub y: i32,
    pub enabled: bool,
    pub selected: bool,
    /// Ícone 18x18 (origem no atlas).
    pub icon: (u32, u32),
    /// Texto da dica.
    pub tip: String,
}

#[derive(Default)]
pub struct UiState {
    pub last_ms: u64,
    pub acc_ms: f64,
    pub partial: f32,
    // bigorna
    pub anvil_field: Option<TextField>,
    pub anvil_last0: Option<Slot>,
    /// Nome padrão (traduzido) do item do slot 0, para `renameItem`.
    pub anvil_default: String,
    pub anvil_init: bool,
    // encantamento
    pub book: BookAnim,
    // farol
    pub beacon_buttons: Vec<BeaconButton>,
    pub beacon_not_drawn: bool,
    // aldeão
    pub trade_selected: i32,
    pub trade_next: bool,
    pub trade_prev: bool,
    pub trade_press: Option<bool>,
    // criativo
    pub creative: Option<crate::gui_creative::Creative>,
}

// ---------------------------------------------------------------- ganchos chamados pelo Inventory

pub fn on_open(inv: &mut Inventory) {
    inv.ui.last_ms = now_ms();
    // GuiInventory.initGui: no criativo abre o GuiContainerCreative.
    if inv.own_screen && inv.env.creative {
        inv.ui.creative = Some(crate::gui_creative::Creative::new(inv));
    }
    if let Some(w) = &inv.open {
        match w.c.kind {
            WinKind::Anvil => {
                // GuiRepair.initGui: campo em (guiLeft+62, guiTop+24), 103x12, sem fundo, branco, máximo 30.
                let mut f = TextField::new(0, 0, 103, 12);
                f.background = false;
                f.color = 0xFFFFFF;
                f.max_len = 30;
                inv.ui.anvil_field = Some(f);
                inv.ui.anvil_init = false;
            }
            WinKind::Beacon => inv.ui.beacon_not_drawn = true,
            _ => {}
        }
    }
}

pub fn on_property(_w: &mut Window, _id: i16, _value: i16) {}

/// Itens que a tela desenha fora dos slots (prévia da troca, pagamento do farol, criativo).
pub fn extra_items(inv: &Inventory) -> Vec<Slot> {
    let mut v = Vec::new();
    let item = |id: i16| Slot { id, count: 1, damage: 0, nbt: None, raw_nbt: Vec::new() };
    if let Some(w) = &inv.open {
        match w.c.kind {
            WinKind::Beacon => v.extend([388, 264, 266, 265].map(item)),
            WinKind::Merchant => {
                if let Some(t) = w.c.trades.get(inv.ui.trade_selected.max(0) as usize) {
                    v.push(t.buy1.clone());
                    v.extend(t.buy2.clone());
                    v.push(t.sell.clone());
                }
            }
            _ => {}
        }
    }
    if let Some(c) = &inv.ui.creative {
        v.extend(c.visible_items());
    }
    v
}

// ---------------------------------------------------------------- entradas

fn gui_mouse(inv: &Inventory) -> (i32, i32) {
    (inv.mouse.0 as i32, inv.mouse.1 as i32)
}

fn in_rect(mx: i32, my: i32, x: i32, y: i32, w: i32, h: i32) -> bool {
    mx >= x && my >= y && mx < x + w && my < y + h
}

/// Botões e campos antes do clique de slot. Devolve true se o clique foi consumido.
pub fn click_widgets(inv: &mut Inventory, ui: &mut UiBatch, button: u8, w: f32, h: f32) -> bool {
    let (l, t, _, _) = inv.frame(w, h);
    let (mx, my) = gui_mouse(inv);
    let kind = inv.kind();
    if inv.open.is_none() {
        return false;
    }
    match kind {
        WinKind::Anvil => {
            if let Some(f) = inv.ui.anvil_field.as_mut() {
                f.x = l + 62;
                f.y = t + 24;
                f.click(ui, mx, my);
            }
        }
        WinKind::Enchant if button == 0 => {
            let props = inv.container().props;
            let lapis = inv.slots().get(1).and_then(|s| s.as_ref()).map_or(0, |s| s.count as i32);
            let have_item = inv.slots().first().is_some_and(|s| s.is_some());
            for i in 0..3 {
                if in_rect(mx - (l + 60), my - (t + 14 + 19 * i), 0, 0, 108, 19) && true {
                    let (dx, dy) = (mx - (l + 60), my - (t + 14 + 19 * i));
                    if dx >= 0 && dy >= 0 && dx < 108 && dy < 19 {
                        let creative = inv.env.creative;
                        let need = i + 1;
                        let ok = !((lapis < need) && !creative)
                            && props[i as usize] > 0
                            && have_item
                            && ((inv.env.level >= need && inv.env.level >= props[i as usize]) || creative);
                        if ok {
                            let id = inv.window_id();
                            inv.out.push(crate::container_net::enchant_item(id, i as u8));
                        }
                    }
                }
            }
        }
        WinKind::Merchant if button == 0 => {
            let n = inv.container().trades.len() as i32;
            let st = &mut inv.ui;
            let mut changed = false;
            if st.trade_next && in_rect(mx, my, l + 120 + 27, t + 24 - 1, 12, 19) {
                st.trade_selected = (st.trade_selected + 1).min(n - 1);
                changed = true;
            } else if st.trade_prev && in_rect(mx, my, l + 36 - 19, t + 24 - 1, 12, 19) {
                st.trade_selected = (st.trade_selected - 1).max(0);
                changed = true;
            }
            if changed {
                let sel = st.trade_selected;
                if let Some(wd) = inv.open.as_mut() {
                    wd.c.trade_index = sel as usize;
                    wd.c.after_change(&mut wd.slots);
                }
                inv.out.push(crate::container_net::trade_select(sel));
            }
        }
        WinKind::Beacon if button == 0 => {
            let hit = inv.ui.beacon_buttons.iter().position(|b| b.enabled && in_rect(mx, my, l + b.x, t + b.y, 22, 22));
            if let Some(i) = hit {
                let b = inv.ui.beacon_buttons[i].clone();
                if b.id == -2 {
                    close_gui(inv);
                } else if b.id == -1 {
                    let (p, s) = (inv.container().props[1], inv.container().props[2]);
                    inv.out.push(crate::container_net::beacon(p, s));
                    close_gui(inv);
                } else if !b.selected {
                    let (eff, level) = (b.id & 255, b.id >> 8);
                    if let Some(wd) = inv.open.as_mut() {
                        wd.c.props[if level < 3 { 1 } else { 2 }] = eff;
                    }
                    inv.ui.beacon_buttons.clear();
                    inv.ui.beacon_not_drawn = true;
                    beacon_update(inv);
                }
            }
        }
        _ => {}
    }
    false
}

fn close_gui(inv: &mut Inventory) {
    if let Some(p) = inv.close() {
        inv.out.push(p);
    }
    inv.closed = true;
}

pub fn moved_widgets(inv: &mut Inventory, _w: f32, _h: f32) {
    let _ = inv;
}

pub fn release_widgets(inv: &mut Inventory, button: u8) {
    let _ = (inv, button);
}

pub fn key_widgets(inv: &mut Inventory, ui: &mut UiBatch, ev: &KeyEvent) -> bool {
    if inv.container().kind == WinKind::Anvil && inv.open.is_some() {
        let has_item = inv.slots().first().is_some_and(|s| s.is_some());
        let consumed = match inv.ui.anvil_field.as_mut() {
            Some(f) if has_item => f.key(ui, ev.code, ev.text, ev.ctrl, ev.shift),
            _ => false,
        };
        if consumed {
            // GuiRepair.renameItem; o texto do campo vai para o servidor.
            anvil_rename(inv, ui);
            return true;
        }
    }
    false
}

pub fn wheel(inv: &mut Inventory, up: bool) {
    crate::gui_creative::wheel(inv, up);
}

fn anvil_rename(inv: &mut Inventory, _ui: &mut UiBatch) {
    let Some(f) = inv.ui.anvil_field.as_ref() else { return };
    let mut name = f.text.clone();
    if let Some(Some(s)) = inv.slots().first() {
        if s.display_name().is_none() && name == inv.ui.anvil_default {
            name.clear();
        }
    }
    inv.out.push(crate::container_net::item_name(&name));
}

// ---------------------------------------------------------------- tique (20 Hz)

/// `GuiScreen.updateScreen` desde o último desenho (cada 50 ms).
fn advance(inv: &mut Inventory) -> f32 {
    let now = now_ms();
    if inv.ui.last_ms == 0 {
        inv.ui.last_ms = now;
    }
    inv.ui.acc_ms += now.saturating_sub(inv.ui.last_ms) as f64;
    inv.ui.last_ms = now;
    let mut ticks = 0;
    while inv.ui.acc_ms >= 50.0 && ticks < 10 {
        inv.ui.acc_ms -= 50.0;
        ticks += 1;
        tick(inv);
    }
    if ticks == 10 {
        inv.ui.acc_ms = 0.0;
    }
    inv.ui.partial = (inv.ui.acc_ms / 50.0) as f32;
    inv.ui.partial
}

fn tick(inv: &mut Inventory) {
    match inv.container().kind {
        WinKind::Enchant if inv.open.is_some() => {
            let any = inv.container().props[..3].iter().any(|v| *v != 0);
            let item = inv.slots().first().cloned().flatten();
            inv.ui.book.tick(item.as_ref(), any);
        }
        WinKind::Merchant if inv.open.is_some() => {
            let n = inv.container().trades.len() as i32;
            let st = &mut inv.ui;
            if n > 0 {
                st.trade_next = st.trade_selected < n - 1;
                st.trade_prev = st.trade_selected > 0;
            }
        }
        WinKind::Beacon if inv.open.is_some() => beacon_update(inv),
        _ => {}
    }
}

/// `GuiBeacon.updateScreen`.
fn beacon_update(inv: &mut Inventory) {
    let (levels, primary, secondary) = {
        let p = &inv.container().props;
        (p[0], p[1], p[2])
    };
    let has_payment = inv.slots().first().is_some_and(|s| s.is_some());
    if inv.ui.beacon_buttons.is_empty() && !inv.ui.beacon_not_drawn {
        inv.ui.beacon_not_drawn = true;
    }
    if inv.ui.beacon_not_drawn && levels >= 0 {
        inv.ui.beacon_not_drawn = false;
        let mut v = vec![
            BeaconButton { id: -1, x: 164, y: 107, enabled: false, selected: false, icon: (ct::BEACON.0 + 90, ct::BEACON.1 + 220), tip: crate::lang::tr("gui.done") },
            BeaconButton { id: -2, x: 190, y: 107, enabled: true, selected: false, icon: (ct::BEACON.0 + 112, ct::BEACON.1 + 220), tip: crate::lang::tr("gui.cancel") },
        ];
        let power = |id: i32, x: i32, y: i32, icon: u32, name: &str, tier: i32| {
            let mut tip = crate::lang::tr(name);
            if tier >= 3 && id != 10 {
                tip.push_str(" II");
            }
            BeaconButton {
                id: (tier << 8) | id,
                x,
                y,
                enabled: true,
                selected: false,
                icon: (INVENTORY.0 + (icon % 8) * 18, INVENTORY.1 + 198 + (icon / 8) * 18),
                tip,
            }
        };
        for tier in 0..=2usize {
            let row = BEACON_EFFECTS[tier];
            let n = row.len() as i32;
            let width = n * 22 + (n - 1) * 2;
            for (i, (id, icon, name)) in row.iter().enumerate() {
                let mut b = power(*id as i32, 76 + i as i32 * 24 - width / 2, 22 + tier as i32 * 25, *icon as u32, name, tier as i32);
                if tier as i32 >= levels {
                    b.enabled = false;
                } else if *id as i32 == primary {
                    b.selected = true;
                }
                v.push(b);
            }
        }
        let tier = 3usize;
        let row = BEACON_EFFECTS[tier];
        let n = row.len() as i32 + 1;
        let width = n * 22 + (n - 1) * 2;
        for (i, (id, icon, name)) in row.iter().enumerate() {
            let mut b = power(*id as i32, 167 + i as i32 * 24 - width / 2, 47, *icon as u32, name, 3);
            if 3 >= levels {
                b.enabled = false;
            } else if *id as i32 == secondary {
                b.selected = true;
            }
            v.push(b);
        }
        if primary > 0 {
            // O efeito primário também vira opção do secundário (nível II).
            if let Some((id, icon, name)) = BEACON_EFFECTS.iter().flat_map(|r| r.iter()).find(|e| e.0 as i32 == primary) {
                let mut b = power(*id as i32, 167 + (n - 1) * 24 - width / 2, 47, *icon as u32, name, 3);
                if 3 >= levels {
                    b.enabled = false;
                } else if primary == secondary {
                    b.selected = true;
                }
                v.push(b);
            }
        }
        inv.ui.beacon_buttons = v;
    }
    if let Some(b) = inv.ui.beacon_buttons.iter_mut().find(|b| b.id == -1) {
        b.enabled = has_payment && primary > 0;
    }
}

// ---------------------------------------------------------------- desenho

fn rarity_color_name(s: &Slot, lang: &Lang) -> String {
    match s.display_name() {
        Some(n) => format!("§o{n}"),
        None => {
            let base = lang.item_name(s.id, s.damage).unwrap_or_else(|| format!("#{}:{}", s.id, s.damage));
            if s.enchanted() { format!("§b{base}") } else { base }
        }
    }
}

/// Dica do item (`renderToolTip`): nome (itálico se tem nome próprio) e lore.
pub(crate) fn item_tooltip(s: &Slot, lang: &Lang) -> Vec<String> {
    let mut lines = vec![rarity_color_name(s, lang)];
    for l in s.lore() {
        lines.push(format!("§5§o{l}"));
    }
    lines
}

/// Ícone e quantidade (como RenderItem.renderItemOverlayIntoGUI).
pub fn draw_stack(ui: &mut UiBatch, s: &Slot, x: f32, y: f32, icons: &HashMap<IconKey, u32>) {
    draw_stack_label(ui, s, x, y, icons, None);
}

/// Como `draw_stack`, mas com um texto no lugar da quantidade (`renderItemOverlayIntoGUI(..., text)`).
pub fn draw_stack_label(ui: &mut UiBatch, s: &Slot, x: f32, y: f32, icons: &HashMap<IconKey, u32>, label: Option<&str>) {
    if let Some(slot) = icons.get(&icon_key(s)) {
        ui.item_icon(*slot, x, y, 16.0);
    }
    let text = label.map(str::to_string).or_else(|| (s.count != 1).then(|| s.count.to_string()));
    if let Some(t) = text {
        let tw = ui.text_width(&t);
        ui.text(x + 19.0 - 2.0 - tw, y + 6.0 + 3.0, &t, 0xFFFFFF, true);
    }
}

fn blit(ui: &mut UiBatch, at: (u32, u32), x: i32, y: i32, u: i32, v: i32, w: i32, h: i32) {
    ui.gui(at, x as f32, y as f32, u as f32, v as f32, w as f32, h as f32);
}

fn label(ui: &mut UiBatch, x: i32, y: i32, s: &str, color: u32) {
    ui.text(x as f32, y as f32, s, color, false);
}

fn centered_label(ui: &mut UiBatch, cx: i32, y: i32, s: &str, color: u32) {
    let w = ui.text_width(s) as i32;
    label(ui, cx - w / 2, y, s, color);
}

/// Fundo (`drawGuiContainerBackgroundLayer`) de cada tela. `l, t` = guiLeft/guiTop.
fn background(inv: &Inventory, ui: &mut UiBatch, l: i32, t: i32, xs: i32, ys: i32) {
    let c = inv.container();
    let p = &c.props;
    match c.kind {
        WinKind::Player => blit(ui, INVENTORY, l, t, 0, 0, 176, 166),
        WinKind::Chest { .. } => {
            let rows = (c.len() - 36) / 9;
            blit(ui, CHEST, l, t, 0, 0, xs, rows as i32 * 18 + 17);
            blit(ui, CHEST, l, t + rows as i32 * 18 + 17, 0, 126, xs, 96);
        }
        WinKind::Workbench => blit(ui, ct::CRAFTING_TABLE, l, t, 0, 0, xs, ys),
        WinKind::Dispenser => blit(ui, ct::DISPENSER, l, t, 0, 0, xs, ys),
        WinKind::Hopper => blit(ui, ct::HOPPER, l, t, 0, 0, xs, ys),
        WinKind::Furnace => {
            blit(ui, ct::FURNACE, l, t, 0, 0, xs, ys);
            // TileEntityFurnace.isBurning: campo 0 (tempo de queima restante) > 0.
            if p[0] > 0 {
                let total = if p[1] == 0 { 200 } else { p[1] };
                let k = p[0] * 13 / total;
                blit(ui, ct::FURNACE, l + 56, t + 36 + 12 - k, 176, 12 - k, 14, k + 1);
            }
            let cook = if p[3] != 0 && p[2] != 0 { p[2] * 24 / p[3] } else { 0 };
            blit(ui, ct::FURNACE, l + 79, t + 34, 176, 14, cook + 1, 16);
        }
        WinKind::Brewing => {
            blit(ui, ct::BREWING_STAND, l, t, 0, 0, xs, ys);
            let brew = p[0];
            if brew > 0 {
                let mut k = (28.0 * (1.0 - brew as f32 / 400.0)) as i32;
                if k > 0 {
                    blit(ui, ct::BREWING_STAND, l + 97, t + 16, 176, 0, 9, k);
                }
                k = match brew / 2 % 7 {
                    0 => 29,
                    1 => 24,
                    2 => 20,
                    3 => 16,
                    4 => 11,
                    5 => 6,
                    _ => 0,
                };
                if k > 0 {
                    blit(ui, ct::BREWING_STAND, l + 65, t + 14 + 29 - k, 185, 29 - k, 12, k);
                }
            }
        }
        WinKind::Anvil => {
            blit(ui, ct::ANVIL, l, t, 0, 0, xs, ys);
            let s0 = inv.slots()[0].is_some();
            let s1 = inv.slots()[1].is_some();
            let s2 = inv.slots()[2].is_some();
            blit(ui, ct::ANVIL, l + 59, t + 20, 0, ys + if s0 { 0 } else { 16 }, 110, 16);
            if (s0 || s1) && !s2 {
                blit(ui, ct::ANVIL, l + 99, t + 45, xs, 0, 28, 21);
            }
        }
        WinKind::Beacon => {
            blit(ui, ct::BEACON, l, t, 0, 0, xs, ys);
        }
        WinKind::Merchant => {
            blit(ui, ct::VILLAGER, l, t, 0, 0, xs, ys);
            if let Some(tr) = c.trades.get(inv.ui.trade_selected.max(0) as usize) {
                if tr.disabled {
                    blit(ui, ct::VILLAGER, l + 83, t + 21, 212, 0, 28, 21);
                    blit(ui, ct::VILLAGER, l + 83, t + 51, 212, 0, 28, 21);
                }
            }
        }
        WinKind::Horse { chested } => {
            blit(ui, ct::HORSE, l, t, 0, 0, xs, ys);
            if chested {
                blit(ui, ct::HORSE, l + 79, t + 17, 0, ys, 90, 54);
            }
            if c.horse_armor {
                blit(ui, ct::HORSE, l + 7, t + 35, 0, ys + 54, 18, 18);
            }
        }
        WinKind::Enchant => blit(ui, ct::ENCHANTING_TABLE, l, t, 0, 0, xs, ys),
        WinKind::Creative => {}
    }
}

/// Texto fixo (`drawGuiContainerForegroundLayer`).
fn foreground(inv: &Inventory, ui: &mut UiBatch, l: i32, t: i32, xs: i32, ys: i32) {
    let title = inv.open.as_ref().map(|w| w.title.clone()).unwrap_or_default();
    let inventory = crate::lang::tr("container.inventory");
    let inv_label = |ui: &mut UiBatch| label(ui, l + 8, t + ys - 96 + 2, &inventory, TEXT);
    match inv.kind() {
        WinKind::Player => label(ui, l + 86, t + 16, &crate::lang::tr("container.crafting"), TEXT),
        WinKind::Chest { .. } | WinKind::Hopper | WinKind::Horse { .. } => {
            label(ui, l + 8, t + 6, &title, TEXT);
            inv_label(ui);
        }
        WinKind::Furnace | WinKind::Dispenser | WinKind::Brewing | WinKind::Merchant => {
            centered_label(ui, l + xs / 2, t + 6, &title, TEXT);
            inv_label(ui);
        }
        WinKind::Workbench => {
            label(ui, l + 28, t + 6, &crate::lang::tr("container.crafting"), TEXT);
            inv_label(ui);
        }
        WinKind::Enchant => {
            label(ui, l + 12, t + 5, &title, TEXT);
            inv_label(ui);
        }
        WinKind::Anvil => anvil_foreground(inv, ui, l, t, xs),
        WinKind::Beacon => {
            centered_label(ui, l + 62, t + 10, &crate::lang::tr("tile.beacon.primary"), 14737632);
            centered_label(ui, l + 169, t + 10, &crate::lang::tr("tile.beacon.secondary"), 14737632);
        }
        WinKind::Creative => {}
    }
}

fn anvil_foreground(inv: &Inventory, ui: &mut UiBatch, l: i32, t: i32, xs: i32) {
    label(ui, l + 60, t + 6, &crate::lang::tr("container.repair"), TEXT);
    let cost = inv.container().props[0];
    if cost > 0 {
        let mut color: u32 = 8453920;
        let mut show = true;
        let mut text = crate::lang::format_java(&crate::lang::tr("container.repair.cost"), &[&cost.to_string()]);
        let out = inv.slots()[2].is_some();
        if cost >= 40 && !inv.env.creative {
            text = crate::lang::tr("container.repair.expensive");
            color = 16736352;
        } else if !out {
            show = false;
        } else if !inv.container().can_take(inv.slots(), 2, inv.env) {
            color = 16736352;
        }
        if show {
            let shadow = 0xFF000000 | ((color & 16579836) >> 2);
            let x = xs - 8 - ui.text_width(&text) as i32;
            let y = 67;
            // `fontRendererObj.getUnicodeFlag()` é falso: sombra em três deslocamentos.
            label(ui, l + x, t + y + 1, &text, shadow);
            label(ui, l + x + 1, t + y, &text, shadow);
            label(ui, l + x + 1, t + y + 1, &text, shadow);
            label(ui, l + x, t + y, &text, color);
        }
    }
}

/// Itens das runas e botões que precisam do mouse (`drawGuiContainerBackgroundLayer` do encantamento).
fn enchant_options(inv: &Inventory, ui: &mut UiBatch, l: i32, t: i32) {
    let c = inv.container();
    let seed = c.props[3];
    let names = crate::enchant::names_for_seed(seed);
    let lapis = inv.slots().get(1).and_then(|s| s.as_ref()).map_or(0, |s| s.count as i32);
    let (mx, my) = gui_mouse(inv);
    for i in 0..3 {
        let x = l + 60;
        let x2 = x + 20;
        let lvl = c.props[i];
        let y = t + 14 + 19 * i as i32;
        if lvl == 0 {
            blit(ui, ct::ENCHANTING_TABLE, x, y, 0, 185, 108, 19);
            continue;
        }
        let mut color: u32 = 6839882;
        let creative = inv.env.creative;
        let split_color;
        if (lapis < i as i32 + 1 || inv.env.level < lvl) && !creative {
            blit(ui, ct::ENCHANTING_TABLE, x, y, 0, 185, 108, 19);
            blit(ui, ct::ENCHANTING_TABLE, x + 1, y + 1, 16 * i as i32, 239, 16, 16);
            split_color = (color & 16711422) >> 1;
            color = 4226832;
        } else {
            let (dx, dy) = (mx - x, my - y);
            if dx >= 0 && dy >= 0 && dx < 108 && dy < 19 {
                blit(ui, ct::ENCHANTING_TABLE, x, y, 0, 204, 108, 19);
                color = 16777088;
            } else {
                blit(ui, ct::ENCHANTING_TABLE, x, y, 0, 166, 108, 19);
            }
            blit(ui, ct::ENCHANTING_TABLE, x + 1, y + 1, 16 * i as i32, 223, 16, 16);
            split_color = color;
            color = 8453920;
        }
        // drawSplitString(nome, x2, y+2, 86): linhas de 9 px na fonte galáctica.
        let mut ly = y + 2;
        for line in gui::wrap_sga(ui, &names[i], 86) {
            ui.text_sga(x2 as f32, ly as f32, &line, 0xFF000000 | split_color);
            ly += 9;
        }
        let s = lvl.to_string();
        let w = ui.text_width(&s) as i32;
        ui.text((x2 + 86 - w) as f32, (y + 2 + 7) as f32, &s, color, true);
    }
}

/// Dica das opções do encantamento (`GuiEnchantment.drawScreen`).
fn enchant_tooltip(inv: &Inventory, l: i32, t: i32) -> Option<Vec<String>> {
    let c = inv.container();
    let (mx, my) = gui_mouse(inv);
    let lapis = inv.slots().get(1).and_then(|s| s.as_ref()).map_or(0, |s| s.count as i32);
    for i in 0..3usize {
        let lvl = c.props[i];
        let eid = c.props[4 + i];
        let n = i as i32 + 1;
        let (dx, dy) = (mx - (l + 60), my - (t + 14 + 19 * i as i32));
        if dx >= 0 && dy >= 0 && dx < 108 + 1 && dy < 17 + 1 && lvl > 0 && eid >= 0 {
            let mut lines = Vec::new();
            if let Some(name) = crate::enchant::translated_name((eid & 255) as u8, (eid & 0xFF00) >> 8) {
                lines.push(format!("§f§o{}", crate::lang::format_java(&crate::lang::tr("container.enchant.clue"), &[&name])));
            }
            if !inv.env.creative {
                if eid >= 0 {
                    lines.push(String::new());
                }
                if inv.env.level < lvl {
                    lines.push(format!("§cLevel Requirement: {lvl}"));
                } else {
                    let s = if n == 1 {
                        crate::lang::tr("container.enchant.lapis.one")
                    } else {
                        crate::lang::format_java(&crate::lang::tr("container.enchant.lapis.many"), &[&n.to_string()])
                    };
                    lines.push(format!("{}{s}", if lapis >= n { "§7" } else { "§c" }));
                    let s = if n == 1 {
                        crate::lang::tr("container.enchant.level.one")
                    } else {
                        crate::lang::format_java(&crate::lang::tr("container.enchant.level.many"), &[&n.to_string()])
                    };
                    lines.push(format!("§7{s}"));
                }
            }
            return Some(lines);
        }
    }
    None
}

impl Inventory {
    /// Desenha a tela aberta. `icons` traz a vaga no atlas de cada (id, dano).
    pub fn draw(&mut self, ui: &mut UiBatch, w: f32, h: f32, icons: &HashMap<IconKey, u32>, lang: &Lang) {
        if self.special.is_some() {
            crate::gui_special::draw(self, ui, w, h, icons, lang);
            return;
        }
        let partial = advance(self);
        if self.ui.creative.is_some() {
            crate::gui_creative::draw(self, ui, w, h, icons, lang);
            return;
        }
        gui::default_background(ui, w as i32, h as i32, true);
        let (l, t, xs, ys) = self.frame(w, h);
        anvil_sync(self, ui, lang, l, t);
        background(self, ui, l, t, xs, ys);
        let kind = self.kind();
        if kind == WinKind::Enchant && self.open.is_some() {
            crate::gui_special::draw_enchant_book(self, ui, l, t, partial);
            enchant_options(self, ui, l, t);
        }
        if kind == WinKind::Beacon {
            draw_beacon_buttons(self, ui, l, t);
        }
        if kind == WinKind::Merchant {
            draw_merchant_buttons(self, ui, l, t);
        }
        match kind {
            // GuiInventory.drawGuiContainerBackgroundLayer.
            WinKind::Player => crate::gui_special::draw_entity_in_gui(self, ui, l + 51, t + 75, 30, (l + 51, t + 75 - 50), -1),
            // GuiScreenHorseInventory.drawGuiContainerBackgroundLayer.
            WinKind::Horse { .. } => {
                let horse = self.open.as_ref().map_or(-1, |w| w.entity);
                crate::gui_special::draw_entity_in_gui(self, ui, l + 51, t + 60, 17, (l + 51, t + 75 - 50), horse)
            }
            _ => {}
        }
        let hovered = self.slot_at(w, h);
        let cursor = self.cursor.clone();
        let slots: Vec<Option<Slot>> = self.slots().to_vec();
        let c = self.container().clone();
        let ms_drag = (self.ms.drag_splitting, self.ms.drag_slots.clone(), self.ms.drag_limit, self.ms.drag_remnant);
        for (i, d) in c.defs.iter().enumerate() {
            let (x, y) = ((l + d.x) as f32, (t + d.y) as f32);
            let mut stack = slots[i].clone();
            let mut label_text: Option<String> = None;
            let mut highlight = false;
            let mut skip = false;
            if ms_drag.0 && ms_drag.1.contains(&i) && cursor.is_some() {
                if ms_drag.1.len() == 1 {
                    skip = true;
                } else {
                    let cur = cursor.clone().unwrap();
                    if can_add(&slots, i, &cur, true) {
                        let existing = slots[i].as_ref().map_or(0, |s| s.count as i32);
                        let mut it = cur.clone();
                        let max = crate::containers::max_stack(&cur) as i32;
                        let n = drag_stack_size(&c, i, &cur, ms_drag.2, existing, ms_drag.1.len());
                        // Dica amarela quando o limite cortou a pilha.
                        let mut raw = cur.count as i32;
                        match ms_drag.2 {
                            0 => raw = (raw as f32 / ms_drag.1.len() as f32).floor() as i32,
                            1 => raw = 1,
                            2 => raw = max,
                            _ => {}
                        }
                        raw += existing;
                        if raw > max {
                            label_text = Some(format!("§e{max}"));
                        } else if raw > c.limit(i, &cur) as i32 {
                            label_text = Some(format!("§e{}", c.limit(i, &cur)));
                        }
                        it.count = n.clamp(0, 255) as u8;
                        stack = Some(it);
                        highlight = true;
                    }
                }
            }
            if skip {
                continue;
            }
            if stack.is_none() {
                if let Role::Armor(k) = d.role {
                    ui.sprite(x, y, 16.0, 16.0, (ct::EMPTY_ARMOR.0 + k as u32 * 16) as f32, ct::EMPTY_ARMOR.1 as f32, 16.0, 16.0, 0xFFFFFF);
                }
            }
            if highlight {
                ui.rect(x, y, 16.0, 16.0, 0x80FFFFFF);
            }
            if let Some(s) = &stack {
                draw_stack_label(ui, s, x, y, icons, label_text.as_deref());
            }
            if hovered == Some(i) && !matches!(d.role, Role::HorseArmor if !c.horse_armor) {
                ui.rect(x, y, 16.0, 16.0, 0x80FFFFFF);
            }
        }
        foreground(self, ui, l, t, xs, ys);
        let (mx, my) = self.mouse;
        if let Some(cur) = &cursor {
            let mut it = cur.clone();
            let mut lab = None;
            if ms_drag.0 && ms_drag.1.len() > 1 {
                it.count = ms_drag.3.max(0) as u8;
                if ms_drag.3 == 0 {
                    lab = Some("§e0".to_string());
                }
            }
            draw_stack_label(ui, &it, mx - 8.0, my - 8.0, icons, lab.as_deref());
        } else if let Some(i) = hovered {
            if let Some(s) = slots[i].as_ref() {
                gui::tooltip(ui, &item_tooltip(s, lang), mx as i32, my as i32, w as i32, h as i32);
            }
        }
        match kind {
            WinKind::Enchant => {
                if let Some(lines) = enchant_tooltip(self, l, t) {
                    gui::tooltip(ui, &lines, mx as i32, my as i32, w as i32, h as i32);
                }
            }
            WinKind::Merchant => merchant_preview(self, ui, l, t, w, h, icons, lang),
            WinKind::Player => self.draw_effects(ui, l, t),
            WinKind::Beacon => {
                let (mx, my) = gui_mouse(self);
                if let Some(b) = self.ui.beacon_buttons.iter().find(|b| in_rect(mx, my, l + b.x, t + b.y, 22, 22)) {
                    gui::tooltip(ui, &[b.tip.clone()], mx, my, w as i32, h as i32);
                }
                for (i, id) in [388i16, 264, 266, 265].iter().enumerate() {
                    let it = Slot { id: *id, count: 1, damage: 0, nbt: None, raw_nbt: Vec::new() };
                    ui.item_icon(*icons.get(&icon_key(&it)).unwrap_or(&0), (l + 42 + 22 * i as i32) as f32, (t + 109) as f32, 16.0);
                }
            }
            WinKind::Anvil => {
                if let Some(f) = self.ui.anvil_field.as_mut() {
                    f.x = l + 62;
                    f.y = t + 24;
                    if self.open.as_ref().is_some_and(|w| w.slots[0].is_some()) {
                        f.draw(ui);
                    }
                }
            }
            _ => {}
        }
    }
}

/// `GuiRepair.sendSlotContents`: quando o item do slot 0 muda, o campo recebe o nome e o servidor também.
fn anvil_sync(inv: &mut Inventory, ui: &mut UiBatch, lang: &Lang, l: i32, t: i32) {
    if inv.container().kind != WinKind::Anvil || inv.open.is_none() {
        return;
    }
    let cur = inv.slots()[0].clone();
    let same = match (&inv.ui.anvil_last0, &cur) {
        (None, None) => true,
        (Some(a), Some(b)) => a.id == b.id && a.damage == b.damage && a.count == b.count && a.raw_nbt == b.raw_nbt,
        _ => false,
    };
    if same && inv.ui.anvil_init {
        return;
    }
    inv.ui.anvil_init = true;
    inv.ui.anvil_last0 = cur.clone();
    let text = cur.as_ref().map_or(String::new(), |s| s.display_name().map(str::to_string).or_else(|| lang.item_name(s.id, s.damage)).unwrap_or_default());
    inv.ui.anvil_default = cur.as_ref().and_then(|s| lang.item_name(s.id, s.damage)).unwrap_or_default();
    if let Some(f) = inv.ui.anvil_field.as_mut() {
        f.x = l + 62;
        f.y = t + 24;
        f.set_text(ui, &text);
    }
    if cur.is_some() {
        anvil_rename(inv, ui);
    }
}

fn draw_beacon_buttons(inv: &Inventory, ui: &mut UiBatch, l: i32, t: i32) {
    let (mx, my) = gui_mouse(inv);
    for b in &inv.ui.beacon_buttons {
        let hovered = in_rect(mx, my, l + b.x, t + b.y, 22, 22);
        let mut u = 0;
        if !b.enabled {
            u += 22 * 2;
        } else if b.selected {
            u += 22;
        } else if hovered {
            u += 22 * 3;
        }
        blit(ui, ct::BEACON, l + b.x, t + b.y, u, 219, 22, 22);
        ui.sprite((l + b.x + 2) as f32, (t + b.y + 2) as f32, 18.0, 18.0, b.icon.0 as f32, b.icon.1 as f32, 18.0, 18.0, 0xFFFFFF);
    }
}

fn draw_merchant_buttons(inv: &Inventory, ui: &mut UiBatch, l: i32, t: i32) {
    let (mx, my) = gui_mouse(inv);
    for (next, x, enabled) in [(true, l + 120 + 27, inv.ui.trade_next), (false, l + 36 - 19, inv.ui.trade_prev)] {
        let y = t + 24 - 1;
        let hovered = in_rect(mx, my, x, y, 12, 19);
        let mut u = 176;
        if !enabled {
            u += 12 * 2;
        } else if hovered {
            u += 12;
        }
        let v = if next { 0 } else { 19 };
        blit(ui, ct::VILLAGER, x, y, u, v, 12, 19);
    }
}

#[allow(clippy::too_many_arguments)]
fn merchant_preview(inv: &Inventory, ui: &mut UiBatch, l: i32, t: i32, w: f32, h: f32, icons: &HashMap<IconKey, u32>, lang: &Lang) {
    let c = inv.container();
    let Some(tr) = c.trades.get(inv.ui.trade_selected.max(0) as usize) else { return };
    draw_stack(ui, &tr.buy1, (l + 36) as f32, (t + 24) as f32, icons);
    if let Some(b2) = &tr.buy2 {
        draw_stack(ui, b2, (l + 62) as f32, (t + 24) as f32, icons);
    }
    draw_stack(ui, &tr.sell, (l + 120) as f32, (t + 24) as f32, icons);
    let (mx, my) = gui_mouse(inv);
    let at = |x: i32, y: i32, ww: i32, hh: i32| in_rect(mx - l, my - t, x - 1, y - 1, ww + 2, hh + 2);
    let tip = if at(36, 24, 16, 16) {
        Some(item_tooltip(&tr.buy1, lang))
    } else if tr.buy2.is_some() && at(62, 24, 16, 16) {
        Some(item_tooltip(tr.buy2.as_ref().unwrap(), lang))
    } else if at(120, 24, 16, 16) {
        Some(item_tooltip(&tr.sell, lang))
    } else if tr.disabled && (at(83, 21, 28, 21) || at(83, 51, 28, 21)) {
        Some(vec![crate::lang::tr("merchant.deprecated")])
    } else {
        None
    };
    if let Some(lines) = tip {
        gui::tooltip(ui, &lines, mx, my, w as i32, h as i32);
    }
}
