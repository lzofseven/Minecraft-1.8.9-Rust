//! Estado das telas de container: inventário do jogador, janelas do servidor (S2D), cliques e teclas
//! (porte de `GuiContainer`, `Container.slotClick` em `containers.rs`). O desenho de cada tela está em
//! `gui_container.rs`; as telas sem slots (placa, livro, morte, cama) em `gui_special.rs`.

use mc_protocol::nbt::{Nbt, Slot};
use mc_protocol::serverbound;
use mc_render::items::{normalize_damage, ENCHANTED};
use mc_render::ui::UiBatch;
use winit::keyboard::KeyCode;

use crate::containers::{self, Container, Env, Trade, WinKind};

pub struct Window {
    pub id: u8,
    pub title: String,
    /// Slots do container (sem contar os 36 do jogador).
    pub size: usize,
    /// Container + 36 do jogador (27 + 9 da hotbar).
    pub slots: Vec<Option<Slot>>,
    pub c: Container,
    /// Entidade do cavalo (`EntityHorse`).
    pub entity: i32,
}

/// Estado do mouse de `GuiContainer` (arrastar para dividir, duplo clique, shift).
#[derive(Default)]
pub struct MouseState {
    pub ignore_mouse_up: bool,
    pub drag_splitting: bool,
    pub drag_slots: Vec<usize>,
    pub drag_limit: i32,
    pub drag_button: i32,
    pub drag_remnant: i32,
    pub last_click_time: u64,
    pub last_click_slot: Option<usize>,
    pub last_click_button: i32,
    pub double_click: bool,
    pub shift_clicked: Option<Slot>,
}

/// Tecla pressionada, como o `Keyboard` do LWJGL entrega.
pub struct KeyEvent<'a> {
    pub code: KeyCode,
    pub text: Option<&'a str>,
    pub ctrl: bool,
    pub shift: bool,
    pub repeat: bool,
}

pub struct Inventory {
    /// Janela 0: 0 resultado, 1-4 grade, 5-8 armadura, 9-35 inventário, 36-44 hotbar.
    pub player: Vec<Option<Slot>>,
    /// Container da janela 0 (layout, `slotClick`).
    pub pc: Container,
    pub open: Option<Window>,
    /// Tela do inventário do jogador (tecla E).
    pub own_screen: bool,
    pub cursor: Option<Slot>,
    pub mouse: (f32, f32),
    /// Modo de jogo e nível de xp (setados pelo jogo).
    pub env: Env,
    pub ms: MouseState,
    /// Pacotes a enviar (o jogo esvazia com `take_packets`).
    pub out: Vec<Vec<u8>>,
    /// A própria tela pediu para fechar (Esc, Concluído...): o jogo devolve o mouse ao mundo.
    pub closed: bool,
    pub ui: crate::gui_container::UiState,
    pub special: Option<crate::gui_special::Special>,
    /// Placa que acabou de ser editada (posição e linhas): o jogo aplica o texto na tile entity.
    pub sign_done: Option<((i32, i32, i32), [String; 4])>,
    /// Sons de interface (cliques de botão) que o jogo toca.
    pub sounds: Vec<&'static str>,
    /// Teclas de hotbar e de largar, como as opções (`key.hotbar.N`, `key.drop`).
    pub binds: Binds,
    /// Efeitos do jogador local (`InventoryEffectRenderer`), copiados do HUD a cada quadro.
    pub effects: crate::potion::Effects,
}

/// Teclas configuráveis usadas pelas telas de container.
#[derive(Clone)]
pub struct Binds {
    /// Códigos LWJGL das opções (`key.hotbar.N`, `key.drop`, `key.inventory`).
    pub hotbar: [i32; 9],
    pub drop: i32,
    pub inventory: i32,
    /// `key.chat`: no criativo sem busca, abre a aba de busca.
    pub chat: i32,
}

impl Default for Binds {
    fn default() -> Self {
        Self { hotbar: [2, 3, 4, 5, 6, 7, 8, 9, 10], drop: 16, inventory: 18, chat: 20 }
    }
}

impl Default for Inventory {
    fn default() -> Self {
        Self {
            player: vec![None; 45],
            pc: Container::new(WinKind::Player),
            open: None,
            own_screen: false,
            cursor: None,
            mouse: (0.0, 0.0),
            env: Env::default(),
            ms: MouseState { ignore_mouse_up: true, ..Default::default() },
            out: Vec::new(),
            closed: false,
            ui: Default::default(),
            special: None,
            sign_done: None,
            sounds: Vec::new(),
            binds: Binds::default(),
            effects: Default::default(),
        }
    }
}

/// Chave do ícone: id, dano que importa e variação (cor do couro, encantado).
pub type IconKey = (i16, i16, u32);

pub fn icon_key(s: &Slot) -> IconKey {
    let mut variant = 0;
    if (298..=301).contains(&s.id) {
        if let Some(Nbt::Int(c)) = s.nbt.as_ref().and_then(|n| n.get("display")).and_then(|d| d.get("color")) {
            variant = *c as u32 & 0xFFFFFF;
        }
    }
    if let Some(e) = crate::tile_entities::entity_item(s) {
        variant = crate::tile_entities::entity_variant(e);
    }
    if s.enchanted() {
        variant |= ENCHANTED;
    }
    (s.id, normalize_damage(s.id, s.damage), variant)
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Tamanho da janela do `GuiContainer` (xSize, ySize) de cada tela.
pub fn gui_size(kind: WinKind, slots: usize) -> (i32, i32) {
    match kind {
        WinKind::Chest { .. } => (176, 222 - 108 + (slots.saturating_sub(36) / 9).clamp(1, 6) as i32 * 18),
        WinKind::Hopper => (176, 133),
        WinKind::Beacon => (230, 219),
        WinKind::Creative => (195, 136),
        _ => (176, 166),
    }
}

/// Tipo de janela de S2D (`NetHandlerPlayClient.handleOpenWindow` + `EntityPlayerSP.displayGUI*`).
pub fn kind_for(kind: &str, size: usize, horse_chested: bool) -> WinKind {
    match kind {
        "minecraft:villager" => WinKind::Merchant,
        "EntityHorse" => WinKind::Horse { chested: horse_chested },
        "minecraft:crafting_table" => WinKind::Workbench,
        "minecraft:enchanting_table" => WinKind::Enchant,
        "minecraft:anvil" => WinKind::Anvil,
        "minecraft:hopper" => WinKind::Hopper,
        "minecraft:furnace" => WinKind::Furnace,
        "minecraft:brewing_stand" => WinKind::Brewing,
        "minecraft:beacon" => WinKind::Beacon,
        "minecraft:dispenser" | "minecraft:dropper" => WinKind::Dispenser,
        _ => WinKind::Chest { rows: (size / 9).clamp(1, 6) },
    }
}

impl Inventory {
    pub fn is_open(&self) -> bool {
        self.open.is_some() || self.own_screen || self.special.is_some()
    }

    pub fn hotbar(&self, i: u8) -> Option<&Slot> {
        self.player.get(36 + i as usize)?.as_ref()
    }

    /// `S2D`: abre a janela `id` do tipo `kind`. `size` = quantidade de slots do container (sem o jogador).
    pub fn open_window(&mut self, id: u8, kind: &str, title: String, size: usize, entity: i32) {
        self.own_screen = false;
        crate::gui_special::on_close(self);
        let k = kind_for(kind, size, size > 2);
        let mut c = Container::new(k);
        if let WinKind::Merchant = k {
            c.trades.clear();
        }
        let n = c.len();
        let mut slots: Vec<Option<Slot>> = vec![None; n];
        let tail = c.player_range();
        for (i, s) in slots[tail].iter_mut().enumerate() {
            *s = self.player[9 + i].clone();
        }
        self.cursor = None;
        self.ui = Default::default();
        self.ms = MouseState { ignore_mouse_up: true, ..Default::default() };
        self.open = Some(Window { id, title, size: n - 36, slots, c, entity });
        crate::gui_container::on_open(self);
    }

    /// Abre o inventário do jogador (tecla E).
    pub fn open_own(&mut self) {
        self.own_screen = true;
        crate::gui_special::on_close(self);
        self.ui = Default::default();
        self.ms = MouseState { ignore_mouse_up: true, ..Default::default() };
        crate::gui_container::on_open(self);
    }

    /// Fecha a tela atual (o servidor também pode mandar fechar). Devolve o pacote para enviar.
    pub fn close(&mut self) -> Option<Vec<u8>> {
        self.cursor = None;
        crate::gui_special::on_close(self);
        if let Some(w) = self.open.take() {
            self.copy_back(&w);
            return Some(serverbound::close_window(w.id));
        }
        if self.own_screen {
            self.own_screen = false;
            return Some(serverbound::close_window(0));
        }
        None
    }

    /// Fechou por ordem do servidor (S2E) ou por morte: sem pacote de volta.
    pub fn close_silently(&mut self) {
        self.open = None;
        self.own_screen = false;
        crate::gui_special::on_close(self);
        self.cursor = None;
    }

    /// Os 36 slots do jogador dentro de uma janela voltam para o inventário principal.
    fn copy_back(&mut self, w: &Window) {
        let r = w.c.player_range();
        for (i, s) in w.slots[r].iter().enumerate() {
            self.player[9 + i] = s.clone();
        }
    }

    fn sync_open(&mut self) {
        if let Some(w) = self.open.take() {
            self.copy_back(&w);
            self.open = Some(w);
        }
    }

    /// O slot `s` da janela 0 (9-44, inventário e hotbar) também na janela aberta, que divide o InventoryPlayer.
    fn mirror_player_slot(&mut self, s: usize, item: Option<Slot>) {
        if let Some(w) = self.open.as_mut().filter(|_| (9..45).contains(&s)) {
            let idx = w.c.player_range().start + s - 9;
            if idx < w.slots.len() {
                w.slots[idx] = item;
            }
        }
    }

    pub fn set_items(&mut self, window: u8, items: Vec<Option<Slot>>) {
        if window == 0 {
            for (i, it) in items.into_iter().enumerate().take(45) {
                self.player[i] = it.clone();
                self.mirror_player_slot(i, it);
            }
        } else if let Some(w) = self.open.as_mut().filter(|w| w.id == window) {
            for (i, it) in items.into_iter().enumerate() {
                if i < w.slots.len() {
                    w.slots[i] = it;
                }
            }
            w.c.after_change(&mut w.slots);
            self.sync_open();
        }
    }

    pub fn set_slot(&mut self, window: i8, slot: i16, item: Option<Slot>) {
        if window == -1 && slot == -1 {
            self.cursor = item;
        } else if window == 0 {
            // handleSetSlot: a hotbar (36-44) vai sempre para o inventoryContainer; o resto da janela 0 só sem outra
            // janela aberta. O InventoryPlayer é o mesmo da janela aberta, então a parte do jogador nela muda junto.
            let s = slot.max(0) as usize;
            if (36..45).contains(&s) || self.open.is_none() {
                if let Some(p) = self.player.get_mut(s) {
                    *p = item.clone();
                }
                self.pc.grid_changed(&mut self.player, s);
                self.mirror_player_slot(s, item);
            }
        } else if let Some(w) = self.open.as_mut().filter(|w| w.id as i8 == window) {
            let idx = slot.max(0) as usize;
            if idx < w.slots.len() {
                w.slots[idx] = item.clone();
                w.c.grid_changed(&mut w.slots, idx);
                w.c.after_change(&mut w.slots);
                if w.c.player_range().contains(&idx) {
                    let k = idx - w.c.player_range().start;
                    self.player[9 + k] = item;
                }
            }
        }
    }

    /// S31 Window Property.
    pub fn set_property(&mut self, window: u8, id: i16, value: i16) {
        if let Some(w) = self.open.as_mut().filter(|w| w.id == window) {
            if let Some(p) = w.c.props.get_mut(id.max(0) as usize) {
                *p = value as i32;
            }
            crate::gui_container::on_property(w, id, value);
        }
    }

    /// `MC|TrList`.
    pub fn set_trades(&mut self, window: i32, trades: Vec<Trade>) {
        if let Some(w) = self.open.as_mut().filter(|w| w.id as i32 == window && w.c.kind == WinKind::Merchant) {
            w.c.trades = trades;
            w.c.after_change(&mut w.slots);
            self.sync_open();
        }
    }

    pub fn slots(&self) -> &[Option<Slot>] {
        match &self.open {
            Some(w) => &w.slots,
            None => &self.player,
        }
    }

    pub fn container(&self) -> &Container {
        match &self.open {
            Some(w) => &w.c,
            None => &self.pc,
        }
    }

    /// Container, itens, cursor e id da janela atual (empréstimos separados).
    pub fn parts(&mut self) -> (&mut Container, &mut Vec<Option<Slot>>, &mut Option<Slot>, u8) {
        match &mut self.open {
            Some(w) => (&mut w.c, &mut w.slots, &mut self.cursor, w.id),
            None => (&mut self.pc, &mut self.player, &mut self.cursor, 0),
        }
    }

    pub fn window_id(&self) -> u8 {
        self.open.as_ref().map_or(0, |w| w.id)
    }

    pub fn kind(&self) -> WinKind {
        self.container().kind
    }

    /// Itens visíveis (para gerar os ícones antes de desenhar).
    pub fn visible_items(&self) -> Vec<IconKey> {
        let mut v: Vec<IconKey> = self.slots().iter().flatten().map(icon_key).collect();
        v.extend(self.cursor.iter().map(icon_key));
        v.extend(crate::gui_container::extra_items(self).iter().map(icon_key));
        v
    }

    pub fn take_packets(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.out)
    }

    pub fn take_closed(&mut self) -> bool {
        std::mem::take(&mut self.closed)
    }

    // ---------------------------------------------------------------- geometria

    /// (guiLeft, guiTop, xSize, ySize) da tela para uma janela de `w` x `h` pixels de GUI.
    pub fn frame(&self, w: f32, h: f32) -> (i32, i32, i32, i32) {
        let (xs, ys) = gui_size(self.kind(), self.container().len());
        // Sempre no centro, mesmo com efeitos (pedido do Lohan, como o `centralizar` do LzTweaks); o painel dos
        // efeitos continua à esquerda da tela.
        ((w as i32 - xs) / 2, ((h as i32) - ys) / 2, xs, ys)
    }

    /// `GuiInventory` e `GuiContainerCreative` são `InventoryEffectRenderer`: com efeitos o painel aparece à
    /// esquerda (a tela fica no centro, sem o deslocamento do vanilla).
    pub fn effect_screen(&self) -> bool {
        self.own_screen && self.open.is_none() && self.special.is_none()
    }

    /// `InventoryEffectRenderer.drawActivePotionEffects`.
    pub fn draw_effects(&self, ui: &mut UiBatch, left: i32, top: i32) {
        if !self.effect_screen() || self.effects.is_empty() {
            return;
        }
        for r in crate::potion::panel(&self.effects, left, top) {
            ui.gui(mc_render::ui::INVENTORY, r.x as f32, r.y as f32, 0.0, 166.0, 140.0, 32.0);
            if let Some((u, v)) = r.icon {
                ui.gui(mc_render::ui::INVENTORY, (r.x + 6) as f32, (r.y + 7) as f32, u as f32, v as f32, 18.0, 18.0);
            }
            let mut name = crate::lang::tr(r.name_key);
            if let Some(k) = r.level_key {
                name = format!("{name} {}", crate::lang::tr(k));
            }
            ui.text((r.x + 10 + 18) as f32, (r.y + 6) as f32, &name, 0xFFFFFF, true);
            ui.text((r.x + 10 + 18) as f32, (r.y + 6 + 10) as f32, &r.duration, 0x7F7F7F, true);
        }
    }

    /// `getSlotAtPosition`.
    pub fn slot_at(&self, w: f32, h: f32) -> Option<usize> {
        let (l, t, _, _) = self.frame(w, h);
        let (mx, my) = (self.mouse.0 as i32 - l, self.mouse.1 as i32 - t);
        let c = self.container();
        c.defs.iter().position(|d| mx >= d.x - 1 && mx < d.x + 17 && my >= d.y - 1 && my < d.y + 17)
    }

    fn outside(&self, w: f32, h: f32) -> bool {
        let (l, t, xs, ys) = self.frame(w, h);
        let (mx, my) = (self.mouse.0 as i32, self.mouse.1 as i32);
        mx < l || my < t || mx >= l + xs || my >= t + ys
    }

    // ---------------------------------------------------------------- cliques

    /// `GuiContainer.handleMouseClick` -> `PlayerControllerMP.windowClick`.
    pub fn handle_click(&mut self, slot: Option<usize>, slot_number: i32, button: i32, mode: i32) {
        let n = slot.map_or(slot_number, |s| s as i32);
        let env = self.env;
        let (c, slots, cursor, wid) = self.parts();
        c.tx = c.tx.wrapping_add(1);
        let tid = c.tx;
        let clicked = c.slot_click(slots, cursor, env, n, button, mode);
        self.out.push(serverbound::click_window(wid, n as i16, button as u8, tid, mode as u8, clicked.as_ref()));
        self.sync_open();
    }

    /// `GuiContainer.mouseClicked`. `button`: 0 esquerdo, 1 direito, 2 meio.
    pub fn click(&mut self, ui: &mut UiBatch, button: u8, shift: bool, w: f32, h: f32) {
        if self.special.is_some() {
            crate::gui_special::click(self, ui, button, shift, w, h);
            return;
        }
        if self.ui.creative.is_some() {
            crate::gui_creative::click(self, button, shift, w, h);
            return;
        }
        if crate::gui_container::click_widgets(self, ui, button, w, h) {
            return;
        }
        let button = button as i32;
        let slot = self.slot_at(w, h);
        let now = now_ms();
        self.ms.double_click = self.ms.last_click_slot == slot && now.saturating_sub(self.ms.last_click_time) < 250 && self.ms.last_click_button == button;
        self.ms.ignore_mouse_up = false;
        if button == 0 || button == 1 || button == 2 {
            let mut outside = self.outside(w, h);
            if slot.is_some() {
                outside = false;
            }
            let l = if outside { -999 } else { slot.map_or(-1, |s| s as i32) };
            if l != -1 && !self.ms.drag_splitting {
                if self.cursor.is_none() {
                    if button == 2 {
                        self.handle_click(slot, l, button, 3);
                    } else {
                        let flag2 = l != -999 && shift;
                        let mut mode = 0;
                        if flag2 {
                            self.ms.shift_clicked = slot.and_then(|s| self.slots()[s].clone());
                            mode = 1;
                        } else if l == -999 {
                            mode = 4;
                        }
                        self.handle_click(slot, l, button, mode);
                    }
                    self.ms.ignore_mouse_up = true;
                } else {
                    self.ms.drag_splitting = true;
                    self.ms.drag_button = button;
                    self.ms.drag_slots.clear();
                    self.ms.drag_limit = match button {
                        0 => 0,
                        1 => 1,
                        _ => 2,
                    };
                }
            }
        }
        self.ms.last_click_slot = slot;
        self.ms.last_click_time = now;
        self.ms.last_click_button = button;
    }

    /// `GuiContainer.mouseClickMove` (arrastar para dividir o cursor entre slots).
    pub fn moved(&mut self, w: f32, h: f32) {
        if self.special.is_some() {
            return;
        }
        if self.ui.creative.is_some() {
            crate::gui_creative::moved(self, w, h);
            return;
        }
        crate::gui_container::moved_widgets(self, w, h);
        if !self.ms.drag_splitting {
            return;
        }
        let Some(slot) = self.slot_at(w, h) else { return };
        let Some(c) = self.cursor.clone() else { return };
        let n_drag = self.ms.drag_slots.len();
        let ok = {
            let (cont, slots, _, _) = self.parts();
            c.count as usize > n_drag
                && can_add(slots, slot, &c, true)
                && cont.valid(slots, slot, &c)
        };
        if ok && !self.ms.drag_slots.contains(&slot) {
            self.ms.drag_slots.push(slot);
            self.update_drag_splitting();
        }
    }

    /// `GuiContainer.updateDragSplitting`.
    pub fn update_drag_splitting(&mut self) {
        let Some(c) = self.cursor.clone() else { return };
        if !self.ms.drag_splitting {
            return;
        }
        let mode = self.ms.drag_limit;
        let list = self.ms.drag_slots.clone();
        let (cont, slots, _, _) = self.parts();
        let mut remnant = c.count as i32;
        for &i in &list {
            let existing = slots[i].as_ref().map_or(0, |s| s.count as i32);
            let n = drag_stack_size(cont, i, &c, mode, existing, list.len());
            remnant -= n - existing;
        }
        self.ms.drag_remnant = remnant;
    }

    /// `GuiContainer.mouseReleased`.
    pub fn release(&mut self, button: u8, shift: bool, w: f32, h: f32) {
        if self.special.is_some() {
            crate::gui_special::release(self, button);
            return;
        }
        if self.ui.creative.is_some() {
            crate::gui_creative::release(self, button, shift, w, h);
            return;
        }
        crate::gui_container::release_widgets(self, button);
        let button = button as i32;
        let slot = self.slot_at(w, h);
        let mut outside = self.outside(w, h);
        if slot.is_some() {
            outside = false;
        }
        let k = if outside { -999 } else { slot.map_or(-1, |s| s as i32) };
        if self.ms.double_click && slot.is_some() && button == 0 && self.container().kind != WinKind::Creative {
            let s = slot.unwrap();
            if shift {
                if let Some(sc) = self.ms.shift_clicked.clone() {
                    let env = self.env;
                    let list: Vec<usize> = {
                        let (c, slots, _, _) = self.parts();
                        let inv_of = c.defs[s].player_inv;
                        (0..slots.len())
                            .filter(|&i| {
                                slots[i].is_some() && c.defs[i].player_inv == inv_of && same_inventory(c, i, s) && c.can_take(slots, i, env) && can_add(slots, i, &sc, true)
                            })
                            .collect()
                    };
                    for i in list {
                        self.handle_click(Some(i), i as i32, button, 1);
                    }
                }
            } else {
                self.handle_click(slot, k, button, 6);
            }
            self.ms.double_click = false;
            self.ms.last_click_time = 0;
        } else {
            if self.ms.drag_splitting && self.ms.drag_button != button {
                self.ms.drag_splitting = false;
                self.ms.drag_slots.clear();
                self.ms.ignore_mouse_up = true;
                return;
            }
            if self.ms.ignore_mouse_up {
                self.ms.ignore_mouse_up = false;
                return;
            }
            if self.ms.drag_splitting && !self.ms.drag_slots.is_empty() {
                let lim = self.ms.drag_limit;
                self.handle_click(None, -999, drag_button(0, lim), 5);
                for s in self.ms.drag_slots.clone() {
                    self.handle_click(Some(s), s as i32, drag_button(1, lim), 5);
                }
                self.handle_click(None, -999, drag_button(2, lim), 5);
            } else if self.cursor.is_some() {
                if button == 2 {
                    self.handle_click(slot, k, button, 3);
                } else {
                    let flag1 = k != -999 && shift;
                    if flag1 {
                        self.ms.shift_clicked = slot.and_then(|s| self.slots()[s].clone());
                    }
                    self.handle_click(slot, k, button, if flag1 { 1 } else { 0 });
                }
            }
        }
        if self.cursor.is_none() {
            self.ms.last_click_time = 0;
        }
        self.ms.drag_splitting = false;
    }

    // ---------------------------------------------------------------- teclado

    /// `GuiContainer.keyTyped`. Devolve true se a tecla foi usada (o jogo não deve repassá-la).
    pub fn key(&mut self, ui: &mut UiBatch, ev: &KeyEvent, w: f32, h: f32) -> bool {
        if self.special.is_some() {
            return crate::gui_special::key(self, ui, ev);
        }
        if self.ui.creative.is_some() {
            return crate::gui_creative::key(self, ui, ev, w, h);
        }
        if crate::gui_container::key_widgets(self, ui, ev) {
            return true;
        }
        // Keyboard.enableRepeatEvents só nas telas de texto (chat, placa, livro, criativo, bigorna): o resto das
        // telas de container não recebe a repetição do sistema.
        if ev.repeat {
            return true;
        }
        let lw = crate::settings::lwjgl_code(ev.code);
        if ev.code == KeyCode::Escape || lw == self.binds.inventory {
            if let Some(p) = self.close() {
                self.out.push(p);
            }
            self.closed = true;
            return true;
        }
        let slot = self.slot_at(w, h);
        // checkHotbarKeys
        if self.cursor.is_none() {
            if let (Some(s), Some(n)) = (slot, self.binds.hotbar.iter().position(|k| *k == lw)) {
                self.handle_click(Some(s), s as i32, n as i32, 2);
                return true;
            }
        }
        if let Some(s) = slot.filter(|s| self.slots()[*s].is_some()) {
            if lw == self.binds.drop {
                self.handle_click(Some(s), s as i32, if ev.ctrl { 1 } else { 0 }, 4);
            }
        }
        true
    }

    /// Roda do mouse (rolagem do criativo).
    pub fn wheel(&mut self, up: bool) {
        crate::gui_container::wheel(self, up);
    }
}

fn drag_button(event: i32, limit: i32) -> i32 {
    (event & 3) | ((limit & 3) << 2)
}

/// `slot.inventory == other.inventory`: mesma metade da janela (container x jogador).
fn same_inventory(c: &Container, a: usize, b: usize) -> bool {
    c.defs[a].player_inv == c.defs[b].player_inv
}

/// `Container.canAddItemToSlot` para um slot existente.
pub fn can_add(slots: &[Option<Slot>], i: usize, s: &Slot, ignore_size: bool) -> bool {
    match slots[i].as_ref() {
        None => true,
        Some(c) => containers::item_equal(s, c) && containers::same_tags(c, s) && c.count as i32 + if ignore_size { 0 } else { s.count as i32 } <= containers::max_stack(s) as i32,
    }
}

/// Tamanho que o slot `i` terminaria tendo ao dividir o cursor `c` (como em `drawSlot`/`updateDragSplitting`).
pub fn drag_stack_size(cont: &Container, i: usize, c: &Slot, mode: i32, existing: i32, n_slots: usize) -> i32 {
    let mut n = c.count as i32;
    match mode {
        0 => n = (n as f32 / n_slots as f32).floor() as i32,
        1 => n = 1,
        2 => n = containers::max_stack(c) as i32,
        _ => {}
    }
    n += existing;
    let probe = Slot { count: n.clamp(0, 255) as u8, ..c.clone() };
    n = n.min(containers::max_stack(&probe) as i32);
    n.min(cont.limit(i, &probe) as i32)
}

#[cfg(test)]
mod tests;
