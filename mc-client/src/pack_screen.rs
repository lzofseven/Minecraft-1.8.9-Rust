//! `GuiScreenResourcePacks` (+ `GuiResourcePackAvailable`/`GuiResourcePackSelected`,
//! `ResourcePackListEntry*`): duas listas de pacotes (disponíveis e selecionados), setas para mover
//! e a lógica de composição. O desenho segue o Java; a lista de selecionados mostra o pacote de
//! maior prioridade no topo e o pacote padrão ("Default") por último.

use std::collections::HashMap;

use mc_render::resources::{Entry, PACK_FORMAT};
use mc_render::ui::{UiBatch, RESOURCE_PACKS};

use crate::gui::{self, SlotList};
use crate::lang::tr;

/// Uma linha das listas: pacote do jogador ou o padrão (que não se move).
#[derive(Clone, Debug)]
pub enum Item {
    Pack(Entry),
    Default { description: String },
}

impl Item {
    fn id(&self) -> String {
        match self {
            Item::Pack(e) => e.id(),
            Item::Default { .. } => "default".to_string(),
        }
    }

    /// `func_183019_a`: -1/0 pacote antigo, 1 compatível, 2 mais novo que o jogo.
    fn compat(&self) -> i32 {
        match self {
            Item::Pack(e) => (e.format() - PACK_FORMAT).signum() + 1,
            Item::Default { .. } => 1,
        }
    }

    fn movable(&self) -> bool {
        matches!(self, Item::Pack(_))
    }

    fn title(&self) -> String {
        match self {
            Item::Pack(e) => e.name.clone(),
            Item::Default { .. } => "Default".to_string(),
        }
    }

    fn description(&self) -> String {
        match self {
            Item::Pack(e) => e.description(),
            Item::Default { description } => description.clone(),
        }
    }
}

/// Resultado de um clique.
#[derive(Debug, PartialEq)]
pub enum Click {
    None,
    /// Pacote incompatível sendo ativado: o `GuiYesNo` de confirmação (id da entrada, `true` = mais novo).
    Confirm(String, bool),
}

/// Estado da tela.
pub struct PackScreen {
    pub available: Vec<Item>,
    /// Do maior para o menor prioridade, com o padrão no fim.
    pub selected: Vec<Item>,
    pub changed: bool,
    avail_list: SlotList,
    sel_list: SlotList,
    w: i32,
    h: i32,
    /// Vaga do ícone de cada entrada no atlas da GUI (chave = `Item::id`).
    pub icons: HashMap<String, u32>,
    /// PNGs ainda não gravados no atlas (chave, bytes).
    pending: Vec<(String, Vec<u8>)>,
    asked: std::collections::HashSet<String>,
}

const LIST_W: i32 = 200;

fn make_list(w: i32, h: i32, left: i32) -> SlotList {
    // GuiResourcePackList: topo 32, fundo height - 55 + 4, entradas de 36, cabeçalho 1,5 linha.
    let mut l = SlotList::new(LIST_W, h, 32, h - 55 + 4, 36);
    l.left = left;
    l.header = (9.0_f32 * 1.5) as i32;
    let _ = w;
    l
}

impl PackScreen {
    /// `GuiScreenResourcePacks.initGui` (primeira vez): `all` = todos os pacotes da pasta,
    /// `selected_low_to_high` = os ativos do menos para o mais prioritário.
    pub fn new(w: i32, h: i32, all: &[Entry], selected_low_to_high: &[Entry], default_description: String) -> Self {
        let ids: Vec<String> = selected_low_to_high.iter().map(Entry::id).collect();
        let available = all.iter().filter(|e| !ids.contains(&e.id())).cloned().map(Item::Pack).collect();
        let mut selected: Vec<Item> = selected_low_to_high.iter().rev().cloned().map(Item::Pack).collect();
        selected.push(Item::Default { description: default_description });
        let mut s = Self {
            available,
            selected,
            changed: false,
            avail_list: make_list(w, h, 0),
            sel_list: make_list(w, h, 0),
            w,
            h,
            icons: HashMap::new(),
            pending: Vec::new(),
            asked: Default::default(),
        };
        s.layout(w, h);
        s
    }

    /// Reposiciona as listas (mudança de tamanho da janela), mantendo a rolagem.
    pub fn layout(&mut self, w: i32, h: i32) {
        self.w = w;
        self.h = h;
        let (sa, ss) = (self.avail_list.scroll, self.sel_list.scroll);
        self.avail_list = make_list(w, h, w / 2 - 4 - LIST_W);
        self.sel_list = make_list(w, h, w / 2 + 4);
        self.avail_list.scroll = sa;
        self.sel_list.scroll = ss;
    }

    /// Pacotes escolhidos, do menos para o mais prioritário (como `repositoryEntries`).
    pub fn chosen(&self) -> Vec<Entry> {
        self.selected
            .iter()
            .rev()
            .filter_map(|i| match i {
                Item::Pack(e) => Some(e.clone()),
                Item::Default { .. } => None,
            })
            .collect()
    }

    /// O atlas da GUI foi refeito: todos os ícones precisam subir de novo.
    pub fn reset_icons(&mut self) {
        self.icons.clear();
        self.asked.clear();
        self.pending.clear();
    }

    /// Ícones que ainda precisam ir para o atlas: `(chave, png)`. Cada um é pedido uma vez.
    pub fn take_icon_requests(&mut self, default_icon: Option<&[u8]>) -> Vec<(String, Vec<u8>)> {
        let mut out = std::mem::take(&mut self.pending);
        for item in self.available.iter().chain(self.selected.iter()) {
            let id = item.id();
            if self.icons.contains_key(&id) || self.asked.contains(&id) {
                continue;
            }
            self.asked.insert(id.clone());
            let png = match item {
                Item::Pack(e) => e.icon.clone(),
                Item::Default { .. } => default_icon.map(<[u8]>::to_vec),
            };
            if let Some(p) = png {
                out.push((id, p));
            }
        }
        out
    }

    fn lists(&self, selected: bool) -> (&Vec<Item>, &SlotList) {
        if selected {
            (&self.selected, &self.sel_list)
        } else {
            (&self.available, &self.avail_list)
        }
    }

    fn scrollbar(list: &SlotList) -> i32 {
        list.left + list.width - 6
    }

    // ------------------------------------------------------------------ desenho

    pub fn draw(&mut self, ui: &mut UiBatch, mx: i32, my: i32, touchscreen: bool) {
        for selected in [false, true] {
            self.draw_list(ui, selected, mx, my, touchscreen);
        }
    }

    fn draw_list(&mut self, ui: &mut UiBatch, selected: bool, mx: i32, my: i32, touchscreen: bool) {
        let n = if selected { self.selected.len() } else { self.available.len() };
        let (items, list) = if selected { (self.selected.clone(), &mut self.sel_list) } else { (self.available.clone(), &mut self.avail_list) };
        list.draw_background(ui, n);
        let x = list.entry_x(LIST_W);
        // drawListHeader: sublinhado + negrito, centrado na lista.
        let title = format!("§n§l{}", tr(if selected { "resourcePack.selected.title" } else { "resourcePack.available.title" }));
        let tw = gui::width(ui, &title);
        ui.text((x + LIST_W / 2 - tw / 2) as f32, (list.top + 3).min(list.top + 4 - list.scroll as i32) as f32, &title, 0xFFFFFF, false);
        let hovered = list.index_at(mx, my, LIST_W, n, Self::scrollbar(list));
        let list_snapshot = list.clone();
        for (i, item) in items.iter().enumerate() {
            let y = list_snapshot.entry_y(i);
            if y + list_snapshot.slot_height < list_snapshot.top || y > list_snapshot.bottom {
                continue;
            }
            let icon = self.icons.get(&item.id()).copied();
            Self::draw_entry(ui, item, icon, &items, i, x, y, mx, my, hovered == Some(i), touchscreen, selected);
        }
        list_snapshot.draw_foreground(ui, n, Self::scrollbar(&list_snapshot));
    }

    /// `ResourcePackListEntry.drawEntry`.
    #[allow(clippy::too_many_arguments)]
    fn draw_entry(ui: &mut UiBatch, item: &Item, icon: Option<u32>, list: &[Item], i: usize, x: i32, y: i32, mx: i32, my: i32, hovered: bool, touchscreen: bool, selected: bool) {
        let compat = item.compat();
        if compat != 1 {
            gui::rect(ui, x - 1, y - 1, x + LIST_W - 9, y + 36 + 1, 0xFF77_0000);
        }
        match icon {
            Some(slot) => ui.item_icon(slot, x as f32, y as f32, 32.0),
            None => ui.rect(x as f32, y as f32, 32.0, 32.0, 0xFF40_4040),
        }
        let mut title = item.title();
        let mut desc = item.description();
        if (touchscreen || hovered) && item.movable() {
            gui::rect(ui, x, y, x + 32, y + 32, 0xA090_9090);
            let (rx, ry) = (mx - x, my - y);
            if compat < 1 {
                title = tr("resourcePack.incompatible");
                desc = tr("resourcePack.incompatible.old");
            } else if compat > 1 {
                title = tr("resourcePack.incompatible");
                desc = tr("resourcePack.incompatible.new");
            }
            let at = |ui: &mut UiBatch, u: f32, v: f32| ui.gui(RESOURCE_PACKS, x as f32, y as f32, u, v, 32.0, 32.0);
            if !selected {
                // func_148309_e: seta para ativar.
                at(ui, 0.0, if rx < 32 { 32.0 } else { 0.0 });
            } else {
                at(ui, 32.0, if rx < 16 { 32.0 } else { 0.0 });
                if i > 0 && list[i - 1].movable() {
                    at(ui, 96.0, if rx < 32 && rx > 16 && ry < 16 { 32.0 } else { 0.0 });
                }
                if i + 1 < list.len() && list[i + 1].movable() {
                    at(ui, 64.0, if rx < 32 && rx > 16 && ry > 16 { 32.0 } else { 0.0 });
                }
            }
        }
        if gui::width(ui, &title) > 157 {
            let dots = gui::width(ui, "...");
            let mut t = title.clone();
            while gui::width(ui, &t) > 157 - dots && !t.is_empty() {
                t.pop();
            }
            title = t + "...";
        }
        ui.text((x + 32 + 2) as f32, (y + 1) as f32, &title, 0xFFFFFF, true);
        for (n, line) in gui::wrap(ui, &desc, 157).iter().take(2).enumerate() {
            ui.text((x + 32 + 2) as f32, (y + 12 + 10 * n as i32) as f32, line, 0x80_8080, true);
        }
    }

    // ------------------------------------------------------------------ entrada

    /// Clique do botão esquerdo (`mouseClicked` das duas listas).
    pub fn click(&mut self, mx: i32, my: i32) -> Click {
        let mut result = Click::None;
        for selected in [false, true] {
            let (n, sb) = {
                let (items, list) = self.lists(selected);
                (items.len(), Self::scrollbar(list))
            };
            let hit = {
                let list = if selected { &mut self.sel_list } else { &mut self.avail_list };
                list.press(mx, my, LIST_W, n, sb)
            };
            let Some((i, _)) = hit else { continue };
            let (ex, ey) = {
                let list = if selected { &self.sel_list } else { &self.avail_list };
                (list.entry_x(LIST_W), list.entry_y(i))
            };
            let c = self.entry_pressed(selected, i, mx - ex, my - ey);
            if c != Click::None {
                result = c;
            }
        }
        result
    }

    /// `ResourcePackListEntry.mousePressed`.
    fn entry_pressed(&mut self, selected: bool, i: usize, rx: i32, ry: i32) -> Click {
        let item = if selected { self.selected[i].clone() } else { self.available[i].clone() };
        if !item.movable() || rx > 32 {
            return Click::None;
        }
        if !selected {
            self.changed = true;
            let c = item.compat();
            if c != 1 {
                return Click::Confirm(item.id(), c > 1);
            }
            self.available.remove(i);
            self.selected.insert(0, item);
            return Click::None;
        }
        if rx < 16 {
            self.selected.remove(i);
            self.available.insert(0, item);
            self.changed = true;
        } else if rx > 16 && ry < 16 && i > 0 && self.selected[i - 1].movable() {
            self.selected.swap(i, i - 1);
            self.changed = true;
        } else if rx > 16 && ry > 16 && i + 1 < self.selected.len() && self.selected[i + 1].movable() {
            self.selected.swap(i, i + 1);
            self.changed = true;
        }
        Click::None
    }

    /// Resposta do `GuiYesNo` de pacote incompatível.
    pub fn confirm(&mut self, id: &str, yes: bool) {
        if !yes {
            return;
        }
        if let Some(pos) = self.available.iter().position(|i| i.id() == id) {
            let item = self.available.remove(pos);
            self.selected.insert(0, item);
        }
    }

    pub fn drag(&mut self, my: i32) {
        let (na, ns) = (self.available.len(), self.selected.len());
        self.avail_list.drag(my, na);
        self.sel_list.drag(my, ns);
    }

    pub fn release(&mut self) {
        self.avail_list.release();
        self.sel_list.release();
    }

    pub fn wheel(&mut self, up: bool, mx: i32, my: i32) {
        let _ = mx;
        for selected in [false, true] {
            let n = if selected { self.selected.len() } else { self.available.len() };
            let list = if selected { &mut self.sel_list } else { &mut self.avail_list };
            let inside = mx >= list.left && mx <= list.left + list.width;
            if inside && list.y_inside(my) {
                list.wheel(up, n);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_render::resources::PackMeta;
    use std::path::PathBuf;

    fn entry(name: &str, format: i32) -> Entry {
        Entry {
            name: name.to_string(),
            path: PathBuf::from(name),
            is_dir: false,
            modified: 1,
            meta: Some(PackMeta { pack_format: format, description: "d".into() }),
            icon: None,
        }
    }

    fn names(v: &[Item]) -> Vec<String> {
        v.iter().map(Item::title).collect()
    }

    fn screen() -> PackScreen {
        let all = vec![entry("a", 1), entry("b", 1), entry("c", 1), entry("novo", 3)];
        PackScreen::new(427, 240, &all, &[all[1].clone(), all[2].clone()], "padrão".into())
    }

    #[test]
    fn initial_lists_follow_priority() {
        let s = screen();
        // Disponíveis: o que não está ativo. Ativos: o último da lista do options (mais prioritário) no topo.
        assert_eq!(names(&s.available), vec!["a", "novo"]);
        assert_eq!(names(&s.selected), vec!["c", "b", "Default"]);
        assert_eq!(s.chosen().iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), vec!["b", "c"]);
        assert!(!s.changed);
    }

    #[test]
    fn move_up_down_enable_and_disable() {
        let mut s = screen();
        // Ativar "a" (seta à esquerda da disponível): vai para o topo dos selecionados.
        assert_eq!(s.entry_pressed(false, 0, 10, 5), Click::None);
        assert_eq!(names(&s.selected), vec!["a", "c", "b", "Default"]);
        assert!(s.changed);
        // Descer "a" (metade de baixo da seta direita).
        s.entry_pressed(true, 0, 24, 24);
        assert_eq!(names(&s.selected), vec!["c", "a", "b", "Default"]);
        // Subir "b" duas vezes: a segunda não faz nada no topo... aqui b sobe uma posição.
        s.entry_pressed(true, 2, 24, 4);
        assert_eq!(names(&s.selected), vec!["c", "b", "a", "Default"]);
        // O padrão não se move e nada passa por baixo dele.
        s.entry_pressed(true, 2, 24, 24);
        assert_eq!(names(&s.selected), vec!["c", "b", "a", "Default"]);
        s.entry_pressed(true, 3, 24, 4);
        assert_eq!(names(&s.selected), vec!["c", "b", "a", "Default"]);
        // Desativar "b" (x < 16): volta ao topo dos disponíveis.
        s.entry_pressed(true, 1, 5, 5);
        assert_eq!(names(&s.available), vec!["b", "novo"]);
        assert_eq!(s.chosen().iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), vec!["a", "c"]);
        // Clique fora da faixa dos 32 px não faz nada.
        s.entry_pressed(false, 0, 40, 5);
        assert_eq!(names(&s.available), vec!["b", "novo"]);
    }

    #[test]
    fn incompatible_pack_asks_before_enabling() {
        let mut s = screen();
        let id = s.available[1].id();
        assert_eq!(s.entry_pressed(false, 1, 10, 5), Click::Confirm(id.clone(), true));
        assert_eq!(names(&s.selected), vec!["c", "b", "Default"], "nada muda antes da resposta");
        s.confirm(&id, false);
        assert_eq!(names(&s.available), vec!["a", "novo"]);
        s.confirm(&id, true);
        assert_eq!(names(&s.selected), vec!["novo", "c", "b", "Default"]);
        assert_eq!(names(&s.available), vec!["a"]);
    }

    #[test]
    fn compat_levels() {
        assert_eq!(Item::Pack(entry("x", 0)).compat(), 0);
        assert_eq!(Item::Pack(entry("x", 1)).compat(), 1);
        assert_eq!(Item::Pack(entry("x", 2)).compat(), 2);
    }
}
