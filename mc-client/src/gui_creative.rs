//! Inventário criativo (`GuiContainerCreative` + `ContainerCreative`): abas, grade rolável, busca,
//! lixeira e a aba de inventário. As abas e os itens de cada uma vêm de `gen_creative.rs`
//! (gerado rodando `CreativeTabs` do original). Os pacotes são os C10 do `PlayerControllerMP`
//! (`sendSlotPacket` e `sendPacketDropItem`).

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use mc_protocol::nbt::{read_root, Nbt, Slot};
use mc_protocol::Reader;
use mc_render::container_tex as ct;
use mc_render::ui::UiBatch;
use winit::keyboard::KeyCode;

use crate::container_net::creative_slot;
use crate::containers::{item_equal, max_stack, same_tags, Container, WinKind};
use crate::gen_creative::{Stack, ITEMS, SEARCH, TABS};
use crate::gui::{self, TextField};
use crate::inventory::*;
use crate::lang::{tr, Lang};

const XS: i32 = 195;
const YS: i32 = 136;
const SEARCH_TAB: usize = 5;
const INVENTORY_TAB: usize = 11;
/// Slot da lixeira na aba de inventário (`field_147064_C`, depois dos 45 do jogador).
const BIN: usize = 45;

/// `GuiContainerCreative.selectedTabIndex` é estático: a aba fica entre aberturas.
static SELECTED: AtomicUsize = AtomicUsize::new(0);

pub struct Creative {
    /// `ContainerCreative.itemList`.
    pub items: Vec<Slot>,
    /// `ContainerCreative`: 45 da grade (`field_147060_v`) + 9 da hotbar.
    pub c: Container,
    pub slots: Vec<Option<Slot>>,
    pub scroll: f32,
    scrolling: bool,
    was_clicking: bool,
    mouse_down: bool,
    pub search: TextField,
    /// `field_147057_D`: depois de um clique, a próxima tecla limpa a busca.
    clear_search: bool,
    /// A busca mudou e falta refiltrar (o filtro precisa do idioma, que só chega no desenho).
    search_dirty: bool,
    /// Cópia do que o `inventoryContainer` já mandou (`Container.inventoryItemStacks`), para o
    /// `detectAndSendChanges` com o `CreativeCrafting` da aba de inventário.
    sent: Vec<Option<Slot>>,
}

fn stack(s: &Stack) -> Slot {
    let (id, damage, raw) = *s;
    let nbt = if raw.is_empty() { None } else { read_root(&mut Reader::new(raw)).ok().flatten() };
    Slot { id, count: 1, damage, nbt, raw_nbt: raw.to_vec() }
}

pub fn selected() -> usize {
    SELECTED.load(Ordering::Relaxed)
}

/// Só para testes: aba com que o próximo `initGui` abre.
pub fn set_selected(tab: usize) {
    SELECTED.store(tab.min(TABS.len() - 1), Ordering::Relaxed);
}

impl Creative {
    /// `initGui`: reabre a última aba (`setCurrentCreativeTab` com a anterior = -1).
    pub fn new(inv: &Inventory) -> Self {
        let mut search = TextField::new(0, 0, 89, 9);
        search.max_len = 15;
        search.background = false;
        search.color = 0xFFFFFF;
        let mut c = Self {
            items: Vec::new(),
            c: Container::new(WinKind::Creative),
            slots: vec![None; 54],
            scroll: 0.0,
            scrolling: false,
            was_clicking: false,
            mouse_down: false,
            search,
            clear_search: false,
            search_dirty: false,
            sent: inv.player.clone(),
        };
        c.set_tab(selected().min(TABS.len() - 1));
        c
    }

    /// Itens com ícone nesta tela (grade e ícones das abas).
    pub fn visible_items(&self) -> Vec<Slot> {
        let mut v: Vec<Slot> = self.slots.iter().flatten().cloned().collect();
        v.extend(TABS.iter().map(|t| Slot { id: t.icon.0, count: 1, damage: t.icon.1, nbt: None, raw_nbt: Vec::new() }));
        v
    }

    /// `setCurrentCreativeTab`.
    fn set_tab(&mut self, tab: usize) {
        SELECTED.store(tab, Ordering::Relaxed);
        self.items = ITEMS[tab].iter().map(stack).collect();
        let t = &TABS[tab];
        if t.search {
            self.search.focused = true;
            self.search.text.clear();
            self.search.w = t.search_width;
            self.search_dirty = true;
        } else {
            self.search.focused = false;
        }
        self.scroll = 0.0;
        self.scroll_to(0.0);
    }

    /// `ContainerCreative.scrollTo`: copia a janela de 5 linhas da lista para a grade.
    pub fn scroll_to(&mut self, f: f32) {
        let i = (self.items.len() as i32 + 9 - 1) / 9 - 5;
        let j = ((f * i as f32) as f64 + 0.5) as i32;
        let j = j.max(0);
        for k in 0..5 {
            for l in 0..9 {
                let i1 = l + (k + j) * 9;
                self.slots[(l + k * 9) as usize] = self.items.get(i1 as usize).cloned();
            }
        }
    }

    /// `func_148328_e`.
    fn has_more(&self) -> bool {
        self.items.len() > 45
    }

    /// `updateCreativeSearch` + `updateFilteredItems` (a aba de busca é a única com campo).
    fn update_search(&mut self, lang: &Lang) {
        let q = self.search.text.to_lowercase();
        self.items = SEARCH.iter().map(stack).filter(|s| tooltip_text(s, lang).iter().any(|l| strip(l).to_lowercase().contains(&q))).collect();
        self.scroll = 0.0;
        self.scroll_to(0.0);
    }
}

/// Texto sem os códigos `§x` (`EnumChatFormatting.getTextWithoutFormattingCodes`).
fn strip(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '§' {
            it.next();
        } else {
            out.push(c);
        }
    }
    out
}

/// Encantamentos (`ench` ou `StoredEnchantments` dos livros) como (id, nível).
fn enchantments(s: &Slot) -> Vec<(u8, i32)> {
    let n = s.nbt.as_ref();
    let list = n.and_then(|n| n.get("ench")).or_else(|| n.and_then(|n| n.get("StoredEnchantments")));
    let Some(Nbt::List(v)) = list else { return Vec::new() };
    v.iter()
        .filter_map(|e| {
            let num = |k: &str| match e.get(k)? {
                Nbt::Short(x) => Some(*x as i32),
                Nbt::Int(x) => Some(*x),
                Nbt::Byte(x) => Some(*x as i32),
                _ => None,
            };
            Some((num("id")? as u8, num("lvl")?))
        })
        .collect()
}

/// Linhas da dica que entram na busca (`ItemStack.getTooltip`): nome, encantamentos e lore.
pub fn tooltip_text(s: &Slot, lang: &Lang) -> Vec<String> {
    let mut lines = crate::gui_container::item_tooltip(s, lang);
    for (id, lvl) in enchantments(s) {
        if let Some(n) = crate::enchant::translated_name(id, lvl) {
            lines.insert(1, n);
        }
    }
    lines
}

// ---------------------------------------------------------------- geometria

/// guiLeft/guiTop: no centro mesmo com efeitos de poção (sem o deslocamento do `InventoryEffectRenderer`).
fn frame(_inv: &Inventory, w: f32, h: f32) -> (i32, i32) {
    let left = (w as i32 - XS) / 2;
    (left, ((h as i32) - YS) / 2)
}

/// Posição de cada slot mostrado (índice do `slotNumber` da tela) na aba atual.
fn positions() -> Vec<(i32, i32)> {
    if selected() == INVENTORY_TAB {
        // setCurrentCreativeTab(tabInventory): os 45 do inventoryContainer reposicionados + lixeira.
        let mut v = Vec::with_capacity(46);
        for j in 0..45i32 {
            v.push(if (5..9).contains(&j) {
                let j1 = j - 5;
                (9 + j1 / 2 * 54, 6 + j1 % 2 * 27)
            } else if j < 5 {
                (-2000, -2000)
            } else {
                let k = j - 9;
                (9 + k % 9 * 18, if j >= 36 { 112 } else { 54 + k / 9 * 18 })
            });
        }
        v.push((173, 112));
        v
    } else {
        crate::containers::defs_for(WinKind::Creative).iter().map(|d| (d.x, d.y)).collect()
    }
}

/// `getSlotAtPosition` (`isMouseOverSlot` = região de 16x16 com 1 px de folga).
fn slot_at(inv: &Inventory, w: f32, h: f32) -> Option<usize> {
    let (l, t) = frame(inv, w, h);
    let (mx, my) = (inv.mouse.0 as i32 - l, inv.mouse.1 as i32 - t);
    positions().iter().position(|&(x, y)| mx >= x - 1 && mx < x + 16 + 1 && my >= y - 1 && my < y + 16 + 1)
}

/// Coluna/linha da aba em coordenadas da tela (`func_147049_a`/`func_147051_a`).
fn tab_origin(tab: usize) -> (i32, i32) {
    let t = &TABS[tab];
    let i = t.column;
    let mut j = 28 * i;
    if i == 5 {
        j = XS - 28 + 2;
    } else if i > 0 {
        j += i;
    }
    (j, if t.first_row { -32 } else { YS })
}

fn tab_hit(tab: usize, mx: i32, my: i32) -> bool {
    let (j, k) = tab_origin(tab);
    mx >= j && mx <= j + 28 && my >= k && my <= k + 32
}

fn needs_scroll_bars(c: &Creative) -> bool {
    let s = selected();
    s != INVENTORY_TAB && TABS[s].hide_player_inventory && c.has_more()
}

// ---------------------------------------------------------------- slots do jogador

/// A hotbar da grade criativa (slots 45..54) espelha `inv.player[36..45]`.
fn pull_hotbar(inv: &mut Inventory) {
    let hot: Vec<Option<Slot>> = inv.player[36..45].to_vec();
    if let Some(c) = inv.ui.creative.as_mut() {
        c.slots[45..54].clone_from_slice(&hot);
    }
}

fn push_hotbar(inv: &mut Inventory) {
    if let Some(c) = inv.ui.creative.as_ref() {
        let hot = c.slots[45..54].to_vec();
        inv.player[36..45].clone_from_slice(&hot);
    }
}

fn stacks_equal(a: &Option<Slot>, b: &Option<Slot>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a.id == b.id && a.count == b.count && a.damage == b.damage && same_tags(a, b),
        _ => false,
    }
}

/// `inventoryContainer.detectAndSendChanges` com o `CreativeCrafting`: C10 de cada slot que mudou.
fn detect_and_send(inv: &mut Inventory) {
    let player = inv.player.clone();
    let Some(c) = inv.ui.creative.as_mut() else { return };
    let mut out = Vec::new();
    for (i, s) in player.iter().enumerate() {
        if !stacks_equal(s, &c.sent[i]) {
            c.sent[i] = s.clone();
            out.push(creative_slot(i as i16, s.as_ref()));
        }
    }
    inv.out.extend(out);
}

/// `PlayerControllerMP.sendSlotPacket`.
fn send_slot(inv: &mut Inventory, s: Option<&Slot>, slot: usize) {
    inv.out.push(creative_slot(slot as i16, s));
    if let Some(c) = inv.ui.creative.as_mut() {
        c.sent[slot] = s.cloned();
    }
}

/// `PlayerControllerMP.sendPacketDropItem` (o item que cai no mundo vem do servidor).
fn send_drop(inv: &mut Inventory, s: &Slot) {
    inv.out.push(creative_slot(-1, Some(s)));
}

// ---------------------------------------------------------------- handleMouseClick

fn handle_click(inv: &mut Inventory, slot: Option<usize>, slot_number: i32, button: i32, mode: i32) {
    let tab = selected();
    if let Some(c) = inv.ui.creative.as_mut() {
        c.clear_search = true;
    }
    let flag = mode == 1;
    let mode = if slot_number == -999 && mode == 0 { 4 } else { mode };
    if slot.is_none() && tab != INVENTORY_TAB && mode != 5 {
        if let Some(mut cur) = inv.cursor.take() {
            if button == 0 {
                send_drop(inv, &cur);
            } else if button == 1 {
                let mut one = cur.clone();
                one.count = 1;
                send_drop(inv, &one);
                cur.count -= 1;
                if cur.count > 0 {
                    inv.cursor = Some(cur);
                }
            } else {
                inv.cursor = Some(cur);
            }
        }
    } else if tab == INVENTORY_TAB && slot == Some(BIN) && flag {
        // Shift na lixeira apaga o inventário todo (o servidor manda os slots de volta).
        for j in 0..inv.player.len() {
            send_slot(inv, None, j);
        }
    } else if tab == INVENTORY_TAB {
        if slot == Some(BIN) {
            inv.cursor = None;
        } else if mode == 4 && slot.is_some_and(|s| inv.player[s].is_some()) {
            let s = slot.unwrap();
            let n = if button == 0 { 1 } else { max_stack(inv.player[s].as_ref().unwrap()) };
            let cur = inv.player[s].as_mut().unwrap();
            let taken = if cur.count <= n {
                inv.player[s].take().unwrap()
            } else {
                cur.count -= n;
                Slot { count: n, ..cur.clone() }
            };
            send_drop(inv, &taken);
        } else if mode == 4 && inv.cursor.is_some() {
            let cur = inv.cursor.take().unwrap();
            send_drop(inv, &cur);
        } else {
            // inventoryContainer.slotClick com o slot de verdade + detectAndSendChanges.
            let env = inv.env;
            let n = slot.map_or(slot_number, |s| s as i32);
            let mut pc = inv.pc.clone();
            pc.slot_click(&mut inv.player, &mut inv.cursor, env, n, button, mode);
            inv.pc = pc;
            detect_and_send(inv);
        }
    } else if mode != 5 && slot.is_some_and(|s| s < 45) {
        let s = slot.unwrap();
        let in_slot = inv.ui.creative.as_ref().and_then(|c| c.slots[s].clone());
        match mode {
            2 => {
                if let Some(mut it) = in_slot.filter(|_| (0..9).contains(&button)) {
                    it.count = max_stack(&it);
                    inv.player[36 + button as usize] = Some(it);
                    pull_hotbar(inv);
                    detect_and_send(inv);
                }
                return;
            }
            3 => {
                if inv.cursor.is_none() {
                    if let Some(mut it) = in_slot {
                        it.count = max_stack(&it);
                        inv.cursor = Some(it);
                    }
                }
                return;
            }
            4 => {
                if let Some(mut it) = in_slot {
                    it.count = if button == 0 { 1 } else { max_stack(&it) };
                    send_drop(inv, &it);
                }
                return;
            }
            _ => {}
        }
        match (inv.cursor.as_mut(), in_slot) {
            (Some(cur), Some(it)) if item_equal(cur, &it) && same_tags(cur, &it) => {
                if button == 0 {
                    if flag {
                        cur.count = max_stack(cur);
                    } else if cur.count < max_stack(cur) {
                        cur.count += 1;
                    }
                } else if cur.count <= 1 {
                    inv.cursor = None;
                } else {
                    cur.count -= 1;
                }
            }
            (None, Some(mut it)) => {
                if flag {
                    it.count = max_stack(&it);
                }
                inv.cursor = Some(it);
            }
            _ => inv.cursor = None,
        }
    } else {
        // ContainerCreative.slotClick (hotbar e arrastar) e os C10 da hotbar.
        pull_hotbar(inv);
        let env = inv.env;
        let n = slot.map_or(slot_number, |s| s as i32);
        if let Some(c) = inv.ui.creative.as_mut() {
            let mut cont = c.c.clone();
            cont.slot_click(&mut c.slots, &mut inv.cursor, env, n, button, mode);
            c.c = cont;
        }
        push_hotbar(inv);
        if (button >> 2) & 3 == 2 && mode == 5 {
            for i in 0..9 {
                let s = inv.player[36 + i].clone();
                send_slot(inv, s.as_ref(), 36 + i);
            }
        } else if let Some(s) = slot {
            // Slots 45–53 da tela criativa são a hotbar, que no inventário do jogador é 36–44.
            // A conta tem de ser feita nesta ordem: `s - 54 + 9 + 36` estoura em `usize` (45 < 54).
            if (45..54).contains(&s) {
                let player_slot = 36 + (s - 45);
                let it = inv.player.get(player_slot).cloned().flatten();
                send_slot(inv, it.as_ref(), player_slot);
            }
        }
    }
}

// ---------------------------------------------------------------- mouse

/// `GuiContainerCreative.mouseClicked` + `GuiContainer.mouseClicked`.
pub fn click(inv: &mut Inventory, button: u8, shift: bool, w: f32, h: f32) {
    let (l, t) = frame(inv, w, h);
    let (mx, my) = (inv.mouse.0 as i32, inv.mouse.1 as i32);
    if button == 0 {
        if let Some(c) = inv.ui.creative.as_mut() {
            c.mouse_down = true;
        }
        if (0..TABS.len()).any(|tab| tab_hit(tab, mx - l, my - t)) {
            return;
        }
    }
    let slot = slot_at(inv, w, h);
    let button = button as i32;
    let now = now_ms();
    inv.ms.double_click = inv.ms.last_click_slot == slot && now.saturating_sub(inv.ms.last_click_time) < 250 && inv.ms.last_click_button == button;
    inv.ms.ignore_mouse_up = false;
    let outside = slot.is_none() && (mx < l || my < t || mx >= l + XS || my >= t + YS);
    let n = if outside { -999 } else { slot.map_or(-1, |s| s as i32) };
    if n != -1 && !inv.ms.drag_splitting {
        if inv.cursor.is_none() {
            if button == 2 {
                handle_click(inv, slot, n, button, 3);
            } else {
                let mode = if n != -999 && shift {
                    inv.ms.shift_clicked = slot.and_then(|s| shown_stack(inv, s));
                    1
                } else if n == -999 {
                    4
                } else {
                    0
                };
                handle_click(inv, slot, n, button, mode);
            }
            inv.ms.ignore_mouse_up = true;
        } else {
            inv.ms.drag_splitting = true;
            inv.ms.drag_button = button;
            inv.ms.drag_slots.clear();
            inv.ms.drag_limit = match button {
                0 => 0,
                1 => 1,
                _ => 2,
            };
        }
    }
    inv.ms.last_click_slot = slot;
    inv.ms.last_click_time = now;
    inv.ms.last_click_button = button;
}

/// O que o slot mostrado `s` tem (grade, hotbar ou inventário do jogador).
fn shown_stack(inv: &Inventory, s: usize) -> Option<Slot> {
    if selected() == INVENTORY_TAB {
        inv.player.get(s).cloned().flatten()
    } else if s >= 45 {
        inv.player[36 + s - 45].clone()
    } else {
        inv.ui.creative.as_ref().and_then(|c| c.slots[s].clone())
    }
}

/// `GuiContainer.mouseClickMove` (só a hotbar aceita arrastar).
pub fn moved(inv: &mut Inventory, w: f32, h: f32) {
    if !inv.ms.drag_splitting || selected() == INVENTORY_TAB {
        return;
    }
    let Some(slot) = slot_at(inv, w, h) else { return };
    let Some(cur) = inv.cursor.clone() else { return };
    if slot < 45 || inv.ms.drag_slots.contains(&slot) || cur.count as usize <= inv.ms.drag_slots.len() {
        return;
    }
    pull_hotbar(inv);
    let ok = inv.ui.creative.as_ref().is_some_and(|c| can_add(&c.slots, slot, &cur, true));
    if ok {
        inv.ms.drag_slots.push(slot);
    }
}

/// `GuiContainerCreative.mouseReleased` + `GuiContainer.mouseReleased`.
pub fn release(inv: &mut Inventory, button: u8, shift: bool, w: f32, h: f32) {
    let (l, t) = frame(inv, w, h);
    let (mx, my) = (inv.mouse.0 as i32, inv.mouse.1 as i32);
    if button == 0 {
        if let Some(c) = inv.ui.creative.as_mut() {
            c.mouse_down = false;
        }
        if let Some(tab) = (0..TABS.len()).find(|&tab| tab_hit(tab, mx - l, my - t)) {
            inv.ms.drag_splitting = false;
            inv.ms.drag_slots.clear();
            if let Some(c) = inv.ui.creative.as_mut() {
                c.set_tab(tab);
            }
            return;
        }
    }
    let slot = slot_at(inv, w, h);
    let button = button as i32;
    let outside = slot.is_none() && (mx < l || my < t || mx >= l + XS || my >= t + YS);
    let k = if outside { -999 } else { slot.map_or(-1, |s| s as i32) };
    // canMergeSlot: na aba de inventário vale o ContainerPlayer; nas outras só a hotbar (y > 90).
    let can_merge = slot.is_some_and(|s| if selected() == INVENTORY_TAB { s != BIN && s != 0 } else { s >= 45 });
    if inv.ms.double_click && slot.is_some() && button == 0 && can_merge {
        if shift {
            if let Some(sc) = inv.ms.shift_clicked.clone() {
                let s = slot.unwrap();
                let list: Vec<usize> = (0..positions().len())
                    .filter(|&i| {
                        let here = shown_stack(inv, i);
                        here.is_some() && same_half(i, s) && can_add_shown(inv, i, &sc)
                    })
                    .collect();
                for i in list {
                    handle_click(inv, Some(i), i as i32, button, 1);
                }
            }
        } else {
            handle_click(inv, slot, k, button, 6);
        }
        inv.ms.double_click = false;
        inv.ms.last_click_time = 0;
    } else {
        if inv.ms.drag_splitting && inv.ms.drag_button != button {
            inv.ms.drag_splitting = false;
            inv.ms.drag_slots.clear();
            inv.ms.ignore_mouse_up = true;
            return;
        }
        if inv.ms.ignore_mouse_up {
            inv.ms.ignore_mouse_up = false;
            return;
        }
        if inv.ms.drag_splitting && !inv.ms.drag_slots.is_empty() {
            let lim = inv.ms.drag_limit;
            let db = |e: i32| (e & 3) | ((lim & 3) << 2);
            handle_click(inv, None, -999, db(0), 5);
            for s in inv.ms.drag_slots.clone() {
                handle_click(inv, Some(s), s as i32, db(1), 5);
            }
            handle_click(inv, None, -999, db(2), 5);
        } else if inv.cursor.is_some() {
            if button == 2 {
                handle_click(inv, slot, k, button, 3);
            } else {
                let flag1 = k != -999 && shift;
                if flag1 {
                    inv.ms.shift_clicked = slot.and_then(|s| shown_stack(inv, s));
                }
                handle_click(inv, slot, k, button, if flag1 { 1 } else { 0 });
            }
        }
    }
    if inv.cursor.is_none() {
        inv.ms.last_click_time = 0;
    }
    inv.ms.drag_splitting = false;
}

/// `slot2.inventory == slot.inventory` na coleta com shift + duplo clique.
fn same_half(a: usize, b: usize) -> bool {
    if selected() == INVENTORY_TAB {
        (a >= 5) == (b >= 5) && a != BIN && b != BIN
    } else {
        (a >= 45) == (b >= 45)
    }
}

fn can_add_shown(inv: &Inventory, i: usize, s: &Slot) -> bool {
    match shown_stack(inv, i) {
        None => true,
        Some(c) => item_equal(s, &c) && same_tags(&c, s) && c.count <= max_stack(s),
    }
}

/// Roda do mouse (`handleMouseInput`).
pub fn wheel(inv: &mut Inventory, up: bool) {
    let Some(c) = inv.ui.creative.as_mut() else { return };
    if !needs_scroll_bars(c) {
        return;
    }
    let j = c.items.len() as i32 / 9 - 5;
    let i = if up { 1.0 } else { -1.0 };
    c.scroll = ((c.scroll as f64 - i / j as f64) as f32).clamp(0.0, 1.0);
    let f = c.scroll;
    c.scroll_to(f);
}

// ---------------------------------------------------------------- teclado

/// `GuiContainerCreative.keyTyped`. Devolve true se consumiu (o resto vai para o `GuiContainer.keyTyped`).
pub fn key(inv: &mut Inventory, ui: &mut UiBatch, ev: &KeyEvent, w: f32, h: f32) -> bool {
    let tab = selected();
    let hover = slot_at(inv, w, h);
    let lw = crate::settings::lwjgl_code(ev.code);
    if !TABS[tab].search {
        if lw == inv.binds.chat {
            if let Some(c) = inv.ui.creative.as_mut() {
                c.set_tab(SEARCH_TAB);
            }
            return true;
        }
        return generic_key(inv, ev, lw, hover);
    }
    if let Some(c) = inv.ui.creative.as_mut() {
        if c.clear_search {
            c.clear_search = false;
            c.search.set_text(ui, "");
        }
    }
    if check_hotbar_keys(inv, lw, hover) {
        return true;
    }
    let typed = inv.ui.creative.as_mut().is_some_and(|c| ev.code != KeyCode::Escape && c.search.key(ui, ev.code, ev.text, ev.ctrl, ev.shift));
    if typed {
        if let Some(c) = inv.ui.creative.as_mut() {
            c.search_dirty = true;
        }
        return true;
    }
    generic_key(inv, ev, lw, hover)
}

/// `GuiContainer.checkHotbarKeys` sobre o slot sob o mouse.
fn check_hotbar_keys(inv: &mut Inventory, lw: i32, hover: Option<usize>) -> bool {
    if inv.cursor.is_some() {
        return false;
    }
    let Some(s) = hover else { return false };
    if let Some(n) = inv.binds.hotbar.iter().position(|k| *k == lw) {
        handle_click(inv, Some(s), s as i32, n as i32, 2);
        return true;
    }
    false
}

/// `GuiContainer.keyTyped`: fechar, teclas da hotbar e largar.
fn generic_key(inv: &mut Inventory, ev: &KeyEvent, lw: i32, hover: Option<usize>) -> bool {
    if ev.code == KeyCode::Escape || lw == inv.binds.inventory {
        if let Some(p) = inv.close() {
            inv.out.push(p);
        }
        inv.closed = true;
        return true;
    }
    if check_hotbar_keys(inv, lw, hover) {
        return true;
    }
    if let Some(s) = hover.filter(|s| shown_stack(inv, *s).is_some()) {
        if lw == inv.binds.drop {
            handle_click(inv, Some(s), s as i32, if ev.ctrl { 1 } else { 0 }, 4);
        }
    }
    true
}

// ---------------------------------------------------------------- desenho

fn blit(ui: &mut UiBatch, at: (u32, u32), x: i32, y: i32, u: i32, v: i32, w: i32, h: i32) {
    ui.gui(at, x as f32, y as f32, u as f32, v as f32, w as f32, h as f32);
}

fn background_tex(tab: usize) -> (u32, u32) {
    match TABS[tab].background {
        "item_search.png" => ct::TAB_ITEM_SEARCH,
        "inventory.png" => ct::TAB_INVENTORY,
        _ => ct::TAB_ITEMS,
    }
}

/// `func_147051_a`: aba e o ícone dela.
fn draw_tab(ui: &mut UiBatch, tab: usize, l: i32, t: i32, icons: &HashMap<IconKey, u32>) {
    let tb = &TABS[tab];
    let sel = tab == selected();
    let i = tb.column;
    let u = i * 28;
    let mut v = if sel { 32 } else { 0 };
    let mut x = l + 28 * i;
    let mut y = t;
    if i == 5 {
        x = l + XS - 28;
    } else if i > 0 {
        x += i;
    }
    if tb.first_row {
        y -= 28;
    } else {
        v += 64;
        y += YS - 4;
    }
    blit(ui, ct::TABS, x, y, u, v, 28, 32);
    let icon = Slot { id: tb.icon.0, count: 1, damage: tb.icon.1, nbt: None, raw_nbt: Vec::new() };
    crate::gui_container::draw_stack(ui, &icon, (x + 6) as f32, (y + 8 + if tb.first_row { 1 } else { -1 }) as f32, icons);
}

pub fn draw(inv: &mut Inventory, ui: &mut UiBatch, w: f32, h: f32, icons: &HashMap<IconKey, u32>, lang: &Lang) {
    // updateScreen: fora do criativo vira o inventário comum.
    if !inv.env.creative {
        inv.ui.creative = None;
        return;
    }
    pull_hotbar(inv);
    let (l, t) = frame(inv, w, h);
    let (mx, my) = (inv.mouse.0 as i32, inv.mouse.1 as i32);
    let tab = selected();
    {
        let c = inv.ui.creative.as_mut().unwrap();
        if c.search_dirty {
            c.search_dirty = false;
            c.update_search(lang);
        }
        // drawScreen: arrastar a barra de rolagem.
        let (k, l1) = (l + 175, t + 18);
        let (i1, j1) = (k + 14, l1 + 112);
        if !c.was_clicking && c.mouse_down && mx >= k && my >= l1 && mx < i1 && my < j1 {
            c.scrolling = needs_scroll_bars(c);
        }
        if !c.mouse_down {
            c.scrolling = false;
        }
        c.was_clicking = c.mouse_down;
        if c.scrolling {
            c.scroll = (((my - l1) as f32 - 7.5) / ((j1 - l1) as f32 - 15.0)).clamp(0.0, 1.0);
            let f = c.scroll;
            c.scroll_to(f);
        }
    }
    gui::default_background(ui, w as i32, h as i32, true);
    // drawGuiContainerBackgroundLayer
    for i in 0..TABS.len() {
        if i != tab {
            draw_tab(ui, i, l, t, icons);
        }
    }
    blit(ui, background_tex(tab), l, t, 0, 0, XS, YS);
    let c = inv.ui.creative.as_mut().unwrap();
    if TABS[tab].search {
        c.search.x = l + 171 - c.search.w;
        c.search.y = t + 6;
        c.search.draw(ui);
    }
    if TABS[tab].hide_player_inventory {
        let (j, k) = (t + 18, t + 18 + 112);
        let u = 232 + if needs_scroll_bars(c) { 0 } else { 12 };
        blit(ui, ct::TABS, l + 175, j + ((k - j - 17) as f32 * c.scroll) as i32, u, 0, 12, 15);
    }
    draw_tab(ui, tab, l, t, icons);
    if tab == INVENTORY_TAB {
        // GuiContainerCreative.drawGuiContainerBackgroundLayer (aba do inventário).
        crate::gui_special::draw_entity_in_gui(inv, ui, l + 43, t + 45, 20, (l + 43, t + 45 - 30), -1);
    }
    // Slots.
    let pos = positions();
    let hovered = slot_at(inv, w, h);
    let drag: Vec<usize> = if inv.ms.drag_splitting { inv.ms.drag_slots.clone() } else { Vec::new() };
    for (i, &(x, y)) in pos.iter().enumerate() {
        if x < -1000 {
            continue;
        }
        let (fx, fy) = ((l + x) as f32, (t + y) as f32);
        let mut s = shown_stack(inv, i);
        if tab == INVENTORY_TAB && i == BIN {
            s = None;
        }
        if s.is_none() && tab == INVENTORY_TAB && (5..9).contains(&i) {
            ui.sprite(fx, fy, 16.0, 16.0, (ct::EMPTY_ARMOR.0 + (i as u32 - 5) * 16) as f32, ct::EMPTY_ARMOR.1 as f32, 16.0, 16.0, 0xFFFFFF);
        }
        if drag.contains(&i) && drag.len() > 1 {
            ui.rect(fx, fy, 16.0, 16.0, 0x80FFFFFF);
        }
        if let Some(s) = &s {
            crate::gui_container::draw_stack(ui, s, fx, fy, icons);
        }
        if hovered == Some(i) {
            ui.rect(fx, fy, 16.0, 16.0, 0x80FFFFFF);
        }
    }
    // drawGuiContainerForegroundLayer
    if TABS[tab].draw_title {
        ui.text((l + 8) as f32, (t + 6) as f32, &tr(TABS[tab].label), 0x404040, false);
    }
    let (fmx, fmy) = inv.mouse;
    if let Some(cur) = inv.cursor.clone() {
        crate::gui_container::draw_stack(ui, &cur, fmx - 8.0, fmy - 8.0, icons);
    } else if let Some(s) = hovered.and_then(|i| if tab == INVENTORY_TAB && i == BIN { None } else { shown_stack(inv, i) }) {
        gui::tooltip(ui, &tooltip_lines(&s, tab, lang), mx, my, w as i32, h as i32);
    }
    inv.draw_effects(ui, l, t);
    // Dicas das abas e da lixeira.
    let over_tab = |tab: usize| {
        let (j, k) = tab_origin(tab);
        let (rx, ry) = (mx - l, my - t);
        rx >= j + 3 - 1 && rx < j + 3 + 23 + 1 && ry >= k + 3 - 1 && ry < k + 3 + 27 + 1
    };
    if let Some(i) = (0..TABS.len()).find(|&i| over_tab(i)) {
        gui::tooltip(ui, &[tr(TABS[i].label)], mx, my, w as i32, h as i32);
    } else if tab == INVENTORY_TAB && hovered == Some(BIN) {
        gui::tooltip(ui, &[tr("inventory.binSlot")], mx, my, w as i32, h as i32);
    }
}

/// `renderToolTip`: na busca, a segunda linha diz a aba do item e o resto fica cinza.
fn tooltip_lines(s: &Slot, tab: usize, lang: &Lang) -> Vec<String> {
    let mut lines = tooltip_text(s, lang);
    if tab != SEARCH_TAB {
        return lines;
    }
    let item_tab = ITEMS.iter().position(|l| l.iter().any(|(id, d, raw)| *id == s.id && (*d == s.damage) && raw.is_empty() == s.raw_nbt.is_empty()));
    let item_tab = item_tab.or_else(|| {
        // Livro encantado sem aba: a aba que tem o tipo do encantamento.
        (s.id == 403).then(|| ITEMS.iter().position(|l| l.iter().any(|(id, _, raw)| *id == 403 && *raw == s.raw_nbt.as_slice()))).flatten()
    });
    if let Some(i) = item_tab {
        lines.insert(1.min(lines.len()), format!("§l§9{}", tr(TABS[i].label)));
    }
    for (i, l) in lines.iter_mut().enumerate() {
        if i > 0 {
            *l = format!("§7{l}");
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_and_scroll() {
        assert_eq!(TABS.len(), 12);
        assert_eq!(TABS[SEARCH_TAB].label, "itemGroup.search");
        assert_eq!(TABS[INVENTORY_TAB].label, "itemGroup.inventory");
        // Blocos de construção começam por pedra (id 1) como no jogo.
        assert_eq!(ITEMS[0][0].0, 1);
        let inv = Inventory::default();
        SELECTED.store(0, Ordering::Relaxed);
        let mut c = Creative::new(&inv);
        assert!(c.has_more());
        assert_eq!(c.slots[0].as_ref().unwrap().id, 1);
        c.scroll_to(1.0);
        let rows = (c.items.len() + 8) / 9;
        let first = (rows - 5) * 9;
        assert_eq!(c.slots[0].as_ref().map(|s| (s.id, s.damage)), c.items.get(first).map(|s| (s.id, s.damage)));
        // Livros encantados da busca trazem StoredEnchantments.
        let book = SEARCH.iter().map(stack).find(|s| s.id == 403).unwrap();
        assert!(!enchantments(&book).is_empty());
    }

    #[test]
    fn grid_click_takes_copy_and_shift_full_stack() {
        SELECTED.store(0, Ordering::Relaxed);
        let mut inv = Inventory::default();
        inv.env.creative = true;
        inv.own_screen = true;
        inv.ui.creative = Some(Creative::new(&inv));
        handle_click(&mut inv, Some(0), 0, 0, 0);
        assert_eq!(inv.cursor.as_ref().map(|s| (s.id, s.count)), Some((1, 1)));
        // Mesmo item: +1; com shift: pilha cheia.
        handle_click(&mut inv, Some(0), 0, 0, 0);
        assert_eq!(inv.cursor.as_ref().unwrap().count, 2);
        handle_click(&mut inv, Some(0), 0, 0, 1);
        assert_eq!(inv.cursor.as_ref().unwrap().count, 64);
        // Botão direito tira um.
        handle_click(&mut inv, Some(0), 0, 1, 0);
        assert_eq!(inv.cursor.as_ref().unwrap().count, 63);
        // Soltar na hotbar manda o C10 do slot 36.
        inv.out.clear();
        handle_click(&mut inv, Some(45), 45, 0, 0);
        assert_eq!(inv.player[36].as_ref().map(|s| (s.id, s.count)), Some((1, 63)));
        assert_eq!(inv.out.len(), 1);
        assert_eq!(&inv.out[0][..3], &[0x10, 0, 36]);
        // Tecla 2 sobre a grade põe a pilha cheia na hotbar 1.
        handle_click(&mut inv, Some(1), 1, 1, 2);
        assert_eq!(inv.player[37].as_ref().map(|s| s.count), Some(64));
    }

    #[test]
    fn drop_outside_sends_minus_one() {
        SELECTED.store(0, Ordering::Relaxed);
        let mut inv = Inventory::default();
        inv.env.creative = true;
        inv.ui.creative = Some(Creative::new(&inv));
        inv.cursor = Some(Slot { id: 1, count: 5, damage: 0, nbt: None, raw_nbt: Vec::new() });
        handle_click(&mut inv, None, -999, 1, 0);
        assert_eq!(inv.cursor.as_ref().unwrap().count, 4);
        assert_eq!(&inv.out[0][..3], &[0x10, 0xFF, 0xFF]);
        handle_click(&mut inv, None, -999, 0, 0);
        assert!(inv.cursor.is_none());
    }
}
