//! Telas sem slots: `GuiEditSign` (placa) e `GuiScreenBook` (livro), mais o botão do `GuiSleepMP`
//! (a cama é um chat com um botão; o chat em si fica no `hud`). Em construção: morte está no `menu.rs`,
//! e os desenhos 3D dentro da GUI (livro do encantamento, jogador, cavalo) ainda são ganchos vazios.

use std::collections::HashMap;

use mc_protocol::nbt::{Nbt, Slot};
use mc_render::container_tex::{BOOK, SIGN};
use mc_render::ui::UiBatch;
use winit::keyboard::KeyCode;

use crate::font_text::{self as ft, Metrics, Piece};
use crate::gui::{self, Button};
use crate::inventory::*;
use crate::lang::{tr, trf, Lang};

/// Id do item `writable_book` (livro e pena) e `written_book` (livro escrito).
pub const WRITABLE_BOOK: i16 = 386;
pub const WRITTEN_BOOK: i16 = 387;
/// Id do bloco `standing_sign`.
const STANDING_SIGN: u16 = 63;

pub enum Special {
    Sign(SignGui),
    Book(Box<BookGui>),
}

/// O que uma tela especial pediu ao fechar/agir.
#[derive(Default)]
struct Outcome {
    close: bool,
    /// `MC|BEdit` ou `MC|BSign` com o livro inteiro (e onde guardá-lo no inventário).
    send_book: Option<(&'static str, Slot)>,
    /// Mensagem de chat (`ClickEvent.RUN_COMMAND`).
    chat: Option<String>,
    sound: bool,
}

/// `GuiScreen.updateScreen` desde a última chamada (cada 50 ms), usando o relógio de `inv.ui`.
fn ticks(inv: &mut Inventory) -> u32 {
    let now = now_ms();
    if inv.ui.last_ms == 0 {
        inv.ui.last_ms = now;
    }
    inv.ui.acc_ms += now.saturating_sub(inv.ui.last_ms) as f64;
    inv.ui.last_ms = now;
    let mut n = 0;
    while inv.ui.acc_ms >= 50.0 && n < 10 {
        inv.ui.acc_ms -= 50.0;
        n += 1;
    }
    if n == 10 {
        inv.ui.acc_ms = 0.0;
    }
    n
}

fn dims(w: f32, h: f32) -> (i32, i32) {
    // `ScaledResolution`: o tamanho em pixels de GUI é arredondado para cima.
    (w.ceil() as i32, h.ceil() as i32)
}

fn mouse(inv: &Inventory) -> (i32, i32) {
    (inv.mouse.0 as i32, inv.mouse.1 as i32)
}

// ---------------------------------------------------------------------------------------------
// GuiEditSign
// ---------------------------------------------------------------------------------------------

/// Escala do `TileEntitySignRenderer` (0,6666667) e da GUI (93,75 px por bloco).
const SIGN_SCALE: f32 = 0.6666667;
const GUI_BLOCK: f32 = 93.75;

pub struct SignGui {
    pub pos: (i32, i32, i32),
    /// O bloco em `pos` é `standing_sign` (tem poste); qualquer outro desenha como placa de parede.
    pub standing: bool,
    /// As quatro linhas (texto com códigos `§` das que ainda não foram editadas).
    pub lines: [String; 4],
    edit_line: usize,
    counter: u32,
}

/// Retângulos em pixels de GUI de uma placa desenhada pelo `GuiEditSign`.
#[derive(Debug, PartialEq)]
struct SignLayout {
    /// x do centro (`width / 2`).
    cx: f32,
    board: [f32; 4],
    stick: Option<[f32; 4]>,
    /// Origem do texto (depois do `translate`) e a escala do `scale(k, -k, k)` composta com a da GUI.
    text_origin: (f32, f32),
    text_scale: f32,
}

impl SignLayout {
    /// A cadeia de matrizes do `drawScreen` e do `renderTileEntityAt` reduzida à GUI ortográfica:
    /// as rotações em Y se cancelam, `scale(-93,75)` com o `scale(s, -s, -s)` do modelo e o `rotate(180)`
    /// deixam x e y crescendo para a direita e para baixo, 16 unidades do modelo = 1 bloco.
    fn new(width: i32, standing: bool) -> Self {
        let cx = (width / 2) as f32;
        // Pixels de tela por unidade do modelo: 93,75 * 0,0625 * 0,6666667.
        let unit = GUI_BLOCK * (0.0625 * SIGN_SCALE);
        // y do ponto (0,0) do modelo: -93,75 * (frame_y) com
        // frame_y = -1,0625 (GuiEditSign) + (-0,75 + 0,75 * s) (renderTileEntityAt) [- 0,3125 na parede].
        let frame_y = -1.0625 + (-0.75 + 0.75 * SIGN_SCALE) - if standing { 0.0 } else { 0.3125 };
        let oy = -GUI_BLOCK * frame_y;
        let rect = |x0: f32, y0: f32, x1: f32, y1: f32| [cx + unit * x0, oy + unit * y0, unit * (x1 - x0), unit * (y1 - y0)];
        // ModelSign: tábua addBox(-12,-14,-1, 24,12,2) e poste addBox(-1,-2,-1, 2,14,2).
        let board = rect(-12.0, -14.0, 12.0, -2.0);
        let stick = standing.then(|| rect(-1.0, -2.0, 1.0, 12.0));
        // translate(0, 0,5 * s, 0,07 * s) e scale(0,015625 * s, ...) no mesmo referencial.
        let text_y = -GUI_BLOCK * (frame_y + 0.5 * SIGN_SCALE);
        Self { cx, board, stick, text_origin: (cx, text_y), text_scale: GUI_BLOCK * (0.015625 * SIGN_SCALE) }
    }
}

impl SignGui {
    pub fn new(pos: (i32, i32, i32), standing: bool, lines: [String; 4]) -> Self {
        Self { pos, standing, lines, edit_line: 0, counter: 0 }
    }

    /// `GuiEditSign.keyTyped`. Devolve true quando pediu para concluir (Esc).
    fn key<M: Metrics>(&mut self, m: &mut M, code: KeyCode, text: Option<&str>) -> bool {
        if code == KeyCode::ArrowUp {
            self.edit_line = (self.edit_line + 3) & 3;
        }
        if matches!(code, KeyCode::ArrowDown | KeyCode::Enter | KeyCode::NumpadEnter) {
            self.edit_line = (self.edit_line + 1) & 3;
        }
        // `getUnformattedText()`: a linha sem cores; ela vira texto simples depois de qualquer tecla.
        let mut line = ft::strip_formatting(&self.lines[self.edit_line]);
        if code == KeyCode::Backspace && !line.is_empty() {
            line.pop();
        }
        if let Some(t) = text.filter(|_| code != KeyCode::Backspace) {
            for c in t.chars() {
                if ft::is_allowed_char(c) {
                    let longer = format!("{line}{c}");
                    if ft::string_width(m, &longer) <= 90 {
                        line = longer;
                    }
                }
            }
        }
        self.lines[self.edit_line] = line;
        code == KeyCode::Escape
    }

    /// As linhas que vão no C12: texto sem formatação (o servidor também descarta os códigos).
    fn plain_lines(&self) -> [String; 4] {
        self.lines.clone().map(|l| ft::strip_formatting(&l))
    }

    fn done_button(&self, w: i32, h: i32) -> Button {
        Button::new(0, w / 2 - 100, h / 4 + 120, tr("gui.done"))
    }

    /// `GuiEditSign.drawScreen`.
    fn draw(&self, ui: &mut UiBatch, w: i32, h: i32, mouse: (i32, i32)) {
        gui::default_background(ui, w, h, true);
        gui::centered(ui, &tr("sign.edit"), w / 2, 40, 0xFFFFFF);
        let l = SignLayout::new(w, self.standing);
        // Face da frente da tábua (UV 2,2 de 24x12) e do poste (UV 2,16 de 2x14) da textura 64x32.
        let [x, y, bw, bh] = l.board;
        ui.sprite(x, y, bw, bh, (SIGN.0 + 2) as f32, (SIGN.1 + 2) as f32, 24.0, 12.0, 0xFFFFFF);
        if let Some([x, y, sw, sh]) = l.stick {
            ui.sprite(x, y, sw, sh, (SIGN.0 + 2) as f32, (SIGN.1 + 16) as f32, 2.0, 14.0, 0xFFFFFF);
        }
        // O texto: `updateCounter / 6 % 2 == 0` mostra o cursor na linha editada.
        let k = l.text_scale;
        ui.transform = Some([k, 0.0, 0.0, k, l.text_origin.0, l.text_origin.1]);
        for (j, line) in self.lines.iter().enumerate() {
            let mut shown = crate::tile_entities::first_sign_line(ui, line);
            if j == self.edit_line && self.counter / 6 % 2 == 0 {
                shown = format!("> {shown} <");
            }
            let x = -(ft::string_width(ui, &shown) / 2);
            ui.text(x as f32, (j as i32 * 10 - 20) as f32, &shown, 0xFF000000, false);
        }
        ui.transform = None;
        self.done_button(w, h).draw(ui, mouse.0, mouse.1);
    }
}

// ---------------------------------------------------------------------------------------------
// GuiScreenBook
// ---------------------------------------------------------------------------------------------

pub struct BookGui {
    book: Slot,
    author: String,
    /// Casa do livro em `Inventory::player` (a mão), onde o resultado volta a ficar.
    slot_index: usize,
    unsigned: bool,
    modified: bool,
    signing: bool,
    counter: u32,
    total: i32,
    curr: i32,
    pages: Option<Vec<String>>,
    title: String,
    /// `field_175386_A`: as linhas da página escrita e a página a que se referem (`field_175387_B`).
    lines: Option<Vec<Vec<Piece>>>,
    lines_page: i32,
}

const BOOK_W: i32 = 192;
const BOOK_H: i32 = 192;

/// `ItemWritableBook.isNBTValid` + título e autor de `ItemEditableBook.validBookTagContents`.
fn valid_written_tag(tag: Option<&Nbt>) -> bool {
    let Some(tag) = tag else { return false };
    if !matches!(tag.get("pages"), Some(Nbt::List(_))) {
        return false;
    }
    // `getStringTagAt(i).length() > 32767`.
    if tag_pages(tag).iter().any(|p| p.encode_utf16().count() > 32767) {
        return false;
    }
    let Some(Nbt::String(title)) = tag.get("title") else { return false };
    title.encode_utf16().count() <= 32 && matches!(tag.get("author"), Some(Nbt::String(_)))
}

fn tag_pages(tag: &Nbt) -> Vec<String> {
    // `getTagList("pages", 8)`: lista de outro tipo vira lista vazia.
    match tag.get("pages") {
        Some(Nbt::List(v)) if v.iter().all(|p| matches!(p, Nbt::String(_))) => v.iter().filter_map(|p| p.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    }
}

impl BookGui {
    pub fn new(book: Slot, unsigned: bool, author: String, slot_index: usize) -> Self {
        let mut total = 1;
        let mut pages = None;
        if let Some(tag) = &book.nbt {
            let p = tag_pages(tag);
            total = (p.len() as i32).max(1);
            pages = Some(p);
        }
        if pages.is_none() && unsigned {
            pages = Some(vec![String::new()]);
            total = 1;
        }
        Self { book, author, slot_index, unsigned, modified: false, signing: false, counter: 0, total, curr: 0, pages, title: String::new(), lines: None, lines_page: -1 }
    }

    fn page_current(&self) -> String {
        match &self.pages {
            Some(p) if self.curr >= 0 && (self.curr as usize) < p.len() => p[self.curr as usize].clone(),
            _ => String::new(),
        }
    }

    fn page_set(&mut self, s: String) {
        if let Some(p) = &mut self.pages {
            if self.curr >= 0 && (self.curr as usize) < p.len() {
                p[self.curr as usize] = s;
                self.modified = true;
            }
        }
    }

    fn page_insert<M: Metrics>(&mut self, m: &mut M, add: &str) {
        let text = self.page_current() + add;
        let height = ft::split_string_height(m, &format!("{text}§0_"), 118);
        if height <= 128 && text.encode_utf16().count() < 256 {
            self.page_set(text);
        }
    }

    fn add_new_page(&mut self) {
        if let Some(p) = &mut self.pages {
            if p.len() < 50 {
                p.push(String::new());
                self.total += 1;
                self.modified = true;
            }
        }
    }

    fn next_visible(&self) -> bool {
        !self.signing && (self.curr < self.total - 1 || self.unsigned)
    }

    fn prev_visible(&self) -> bool {
        !self.signing && self.curr > 0
    }

    fn title_trimmed(&self) -> &str {
        self.title.trim_matches(|c: char| c <= ' ')
    }

    /// Os botões de texto na ordem de `buttonList`, só os visíveis (`updateButtons`).
    fn text_button(&self, id: i32, w: i32) -> Option<Button> {
        let y = 4 + BOOK_H;
        if !self.unsigned {
            return (id == 0).then(|| Button::sized(0, w / 2 - 100, y, 200, 20, tr("gui.done")));
        }
        match id {
            3 if !self.signing => Some(Button::sized(3, w / 2 - 100, y, 98, 20, tr("book.signButton"))),
            0 if !self.signing => Some(Button::sized(0, w / 2 + 2, y, 98, 20, tr("gui.done"))),
            5 if self.signing => {
                let b = Button::sized(5, w / 2 - 100, y, 98, 20, tr("book.finalizeButton"));
                Some(if self.title_trimmed().is_empty() { b.disabled() } else { b })
            }
            4 if self.signing => Some(Button::sized(4, w / 2 + 2, y, 98, 20, tr("gui.cancel"))),
            _ => None,
        }
    }

    /// Ordem de `buttonList`: [assinar, concluído, finalizar, cancelar] (ou só concluído), próxima, anterior.
    fn button_ids(&self) -> &'static [i32] {
        if self.unsigned {
            &[3, 0, 5, 4, 1, 2]
        } else {
            &[0, 1, 2]
        }
    }

    fn page_button_rect(&self, w: i32, next: bool) -> (i32, i32) {
        let left = (w - BOOK_W) / 2;
        if next {
            (left + 120, 2 + 154)
        } else {
            (left + 38, 2 + 154)
        }
    }

    /// `GuiScreenBook.sendBookToServer`: devolve o canal e o livro como vai na rede.
    fn send_book(&mut self, sign: bool) -> Option<(&'static str, Slot)> {
        if !(self.unsigned && self.modified) {
            return None;
        }
        let title = self.title_trimmed().to_string();
        let pages = self.pages.as_mut()?;
        while pages.len() > 1 && pages.last().is_some_and(String::is_empty) {
            pages.pop();
        }
        let mut tag = match &self.book.nbt {
            Some(Nbt::Compound(m)) => m.clone(),
            _ => HashMap::new(),
        };
        let mut channel = "MC|BEdit";
        let mut out_pages: Vec<Nbt> = pages.iter().cloned().map(Nbt::String).collect();
        if sign {
            channel = "MC|BSign";
            tag.insert("author".into(), Nbt::String(self.author.clone()));
            tag.insert("title".into(), Nbt::String(title));
            out_pages = pages.iter().map(|p| Nbt::String(ft::component_to_json(p))).collect();
            self.book.id = WRITTEN_BOOK;
        }
        tag.insert("pages".into(), Nbt::List(out_pages));
        let root = Nbt::Compound(tag);
        self.book.raw_nbt = root.write_root();
        self.book.nbt = Some(root);
        Some((channel, self.book.clone()))
    }

    /// `actionPerformed` do botão `id`.
    fn action<M: Metrics>(&mut self, _m: &mut M, id: i32) -> Outcome {
        let mut o = Outcome { sound: true, ..Default::default() };
        match id {
            0 => {
                o.close = true;
                o.send_book = self.send_book(false);
            }
            3 if self.unsigned => self.signing = true,
            1 => {
                if self.curr < self.total - 1 {
                    self.curr += 1;
                } else if self.unsigned {
                    self.add_new_page();
                    if self.curr < self.total - 1 {
                        self.curr += 1;
                    }
                }
            }
            2 => {
                if self.curr > 0 {
                    self.curr -= 1;
                }
            }
            5 if self.signing => {
                o.send_book = self.send_book(true);
                o.close = true;
            }
            4 if self.signing => self.signing = false,
            _ => {}
        }
        o
    }

    /// `GuiScreenBook.mouseClicked` (já em pixels de GUI) e o laço de botões de `GuiScreen.mouseClicked`.
    fn click<M: Metrics>(&mut self, m: &mut M, mx: i32, my: i32, button: u8, shift: bool, w: i32) -> Outcome {
        if button != 0 {
            return Outcome::default();
        }
        if let Some(c) = self.click_event_at(m, mx, my, w) {
            if let Some(o) = self.component_click(&c, shift) {
                return o;
            }
        }
        // `GuiScreen.mouseClicked`: percorre todos os botões na ordem, com a visibilidade do momento.
        let mut result = Outcome::default();
        for &id in self.button_ids() {
            let pressed = match id {
                1 | 2 => {
                    let next = id == 1;
                    let visible = if next { self.next_visible() } else { self.prev_visible() };
                    let (x, y) = self.page_button_rect(w, next);
                    visible && mx >= x && my >= y && mx < x + 23 && my < y + 13
                }
                _ => self.text_button(id, w).is_some_and(|b| b.pressed(mx, my)),
            };
            if pressed {
                let o = self.action(m, id);
                result.sound = true;
                if o.close {
                    return Outcome { sound: true, ..o };
                }
            }
        }
        result
    }

    /// O clique do pedaço de texto sob o mouse (`func_175385_b`), só na página escrita.
    fn click_event_at<M: Metrics>(&self, m: &mut M, mx: i32, my: i32, w: i32) -> Option<ft::Click> {
        let lines = self.lines.as_ref()?;
        let left = (w - BOOK_W) / 2;
        ft::piece_at(m, lines, mx - left - 36, my - 2 - 16 - 16)?.click.clone()
    }

    /// `GuiScreenBook.handleComponentClick`: `None` quando não tratou (o clique segue para os botões).
    fn component_click(&mut self, c: &ft::Click, shift: bool) -> Option<Outcome> {
        if c.action == "change_page" {
            let page = c.value.parse::<i32>().ok()? - 1;
            if page >= 0 && page < self.total && page != self.curr {
                self.curr = page;
                return Some(Outcome::default());
            }
            return None;
        }
        // `GuiScreen.handleComponentClick`: com shift só insere o texto de inserção (sem campo aqui).
        if shift {
            return None;
        }
        match c.action.as_str() {
            // Executa como mensagem de chat sem guardar no histórico e fecha o livro.
            "run_command" => Some(Outcome { close: true, chat: Some(c.value.clone()), ..Default::default() }),
            // `setText` da GuiScreen não faz nada; `open_url` abriria o GuiConfirmOpenLink (pendente).
            _ => Some(Outcome::default()),
        }
    }

    /// `keyTyped`: `super.keyTyped` (Esc fecha sem enviar) e depois a digitação no livro ou no título.
    fn key<M: Metrics>(&mut self, m: &mut M, ev: &KeyEvent) -> Outcome {
        if ev.code == KeyCode::Escape {
            return Outcome { close: true, ..Default::default() };
        }
        if !self.unsigned {
            return Outcome::default();
        }
        let enter = matches!(ev.code, KeyCode::Enter | KeyCode::NumpadEnter);
        if self.signing {
            if ev.code == KeyCode::Backspace {
                if !self.title.is_empty() {
                    self.title.pop();
                }
            } else if enter {
                if !self.title.is_empty() {
                    return Outcome { close: true, send_book: self.send_book(true), ..Default::default() };
                }
            } else if !ev.ctrl {
                for c in ev.text.unwrap_or("").chars() {
                    if self.title.encode_utf16().count() < 16 && ft::is_allowed_char(c) {
                        self.title.push(c);
                        self.modified = true;
                    }
                }
            }
        } else if ev.code == KeyCode::Backspace {
            let mut cur = self.page_current();
            if !cur.is_empty() {
                cur.pop();
                self.page_set(cur);
            }
        } else if enter {
            self.page_insert(m, "\n");
        } else if ev.ctrl {
            // Ctrl+V (`pageInsertIntoCurrent(getClipboardString())`) ainda não tem área de transferência.
        } else {
            for c in ev.text.unwrap_or("").chars() {
                if ft::is_allowed_char(c) {
                    self.page_insert(m, &c.to_string());
                }
            }
        }
        Outcome::default()
    }

    /// `GuiScreenBook.drawScreen`.
    fn draw(&mut self, ui: &mut UiBatch, w: i32, mouse: (i32, i32)) {
        let left = (w - BOOK_W) / 2;
        let top = 2;
        ui.gui(BOOK, left as f32, top as f32, 0.0, 0.0, BOOK_W as f32, BOOK_H as f32);
        let black = 0xFF000000;
        if self.signing {
            let mut title = self.title.clone();
            if self.unsigned {
                title.push_str(if self.counter / 6 % 2 == 0 { "§0_" } else { "§7_" });
            }
            let edit = tr("book.editTitle");
            let tw = ft::string_width(ui, &edit);
            ui.text((left + 36 + (116 - tw) / 2) as f32, (top + 16 + 16) as f32, &edit, black, false);
            let tw = ft::string_width(ui, &title);
            ui.text((left + 36 + (116 - tw) / 2) as f32, (top + 48) as f32, &title, black, false);
            let by = trf("book.byAuthor", &[&self.author]);
            let tw = ft::string_width(ui, &by);
            ui.text((left + 36 + (116 - tw) / 2) as f32, (top + 48 + 10) as f32, &format!("§8{by}"), black, false);
            ft::draw_split_string(ui, &tr("book.finalizeWarning"), left + 36, top + 80, 116, black);
        } else {
            let indicator = trf("book.pageIndicator", &[&(self.curr + 1).to_string(), &self.total.to_string()]);
            let mut text = match &self.pages {
                Some(p) if self.curr >= 0 && (self.curr as usize) < p.len() => p[self.curr as usize].clone(),
                _ => String::new(),
            };
            if self.unsigned {
                text.push_str(if self.counter / 6 % 2 == 0 { "§0_" } else { "§7_" });
            } else if self.lines_page != self.curr {
                self.lines = if valid_written_tag(self.book.nbt.as_ref()) {
                    ft::parse_page(&text).map(|items| ft::split_text(ui, items, 116, true))
                } else {
                    Some(vec![vec![Piece { text: "§4* Invalid book tag *".to_string(), click: None }]])
                };
                self.lines_page = self.curr;
            }
            let iw = ft::string_width(ui, &indicator);
            ui.text((left - iw + BOOK_W - 44) as f32, (top + 16) as f32, &indicator, black, false);
            match &self.lines {
                None => ft::draw_split_string(ui, &text, left + 36, top + 16 + 16, 116, black),
                Some(lines) => {
                    for (i, line) in lines.iter().take((128 / 9) as usize).enumerate() {
                        ui.text((left + 36) as f32, (top + 16 + 16 + i as i32 * 9) as f32, &ft::line_text(line), black, false);
                    }
                }
            }
        }
        for &id in self.button_ids() {
            match id {
                1 | 2 => {
                    let next = id == 1;
                    if if next { self.next_visible() } else { self.prev_visible() } {
                        let (x, y) = self.page_button_rect(w, next);
                        let hovered = mouse.0 >= x && mouse.1 >= y && mouse.0 < x + 23 && mouse.1 < y + 13;
                        let u = if hovered { 23.0 } else { 0.0 };
                        let v = if next { 192.0 } else { 192.0 + 13.0 };
                        ui.gui(BOOK, x as f32, y as f32, u, v, 23.0, 13.0);
                    }
                }
                _ => {
                    if let Some(b) = self.text_button(id, w) {
                        b.draw(ui, mouse.0, mouse.1);
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Ligação com o Inventory
// ---------------------------------------------------------------------------------------------

/// `onGuiClosed` ao fechar por qualquer caminho: a placa manda o C12 e guarda o texto.
pub fn on_close(inv: &mut Inventory) {
    if let Some(sp) = inv.special.take() {
        finish(inv, sp);
    }
}

fn finish(inv: &mut Inventory, sp: Special) {
    if let Special::Sign(s) = sp {
        inv.out.push(crate::container_net::update_sign(s.pos, &s.plain_lines()));
        inv.sign_done = Some((s.pos, s.lines));
    }
}

/// Aplica o resultado de uma ação: pacotes, o livro de volta na mão, chat, som e fechamento.
fn apply(inv: &mut Inventory, sp: Special, o: Outcome, book_slot: Option<usize>) {
    if o.sound {
        inv.sounds.push("gui.button.press");
    }
    if let Some((channel, book)) = o.send_book {
        inv.out.push(crate::container_net::book(channel, &book));
        if let Some(i) = book_slot.filter(|i| *i < inv.player.len()) {
            inv.player[i] = Some(book);
        }
    }
    if let Some(msg) = o.chat {
        inv.out.push(mc_protocol::serverbound::chat(&msg));
    }
    if o.close {
        finish(inv, sp);
        inv.closed = true;
    } else {
        inv.special = Some(sp);
    }
}

pub fn click(inv: &mut Inventory, ui: &mut UiBatch, button: u8, shift: bool, w: f32, h: f32) {
    let Some(mut sp) = inv.special.take() else { return };
    let (mx, my) = mouse(inv);
    let (wi, hi) = dims(w, h);
    let (o, slot) = match &mut sp {
        Special::Sign(s) => {
            let hit = button == 0 && s.done_button(wi, hi).pressed(mx, my);
            // `actionPerformed(doneBtn)`: fecha (e o onGuiClosed manda o C12).
            (Outcome { close: hit, sound: hit, ..Default::default() }, None)
        }
        Special::Book(b) => {
            let slot = b.slot_index;
            (b.click(ui, mx, my, button, shift, wi), Some(slot))
        }
    };
    apply(inv, sp, o, slot);
}

pub fn release(_inv: &mut Inventory, _button: u8) {}

pub fn key(inv: &mut Inventory, ui: &mut UiBatch, ev: &KeyEvent) -> bool {
    let Some(mut sp) = inv.special.take() else { return false };
    let (o, slot) = match &mut sp {
        Special::Sign(s) => (Outcome { close: s.key(ui, ev.code, ev.text), ..Default::default() }, None),
        Special::Book(b) => {
            let slot = b.slot_index;
            (b.key(ui, ev), Some(slot))
        }
    };
    apply(inv, sp, o, slot);
    true
}

pub fn draw(inv: &mut Inventory, ui: &mut UiBatch, w: f32, h: f32, _icons: &HashMap<IconKey, u32>, _lang: &Lang) {
    let n = ticks(inv);
    let m = mouse(inv);
    let (wi, hi) = dims(w, h);
    match inv.special.as_mut() {
        Some(Special::Sign(s)) => {
            s.counter += n;
            s.draw(ui, wi, hi, m);
        }
        Some(Special::Book(b)) => {
            b.counter += n;
            b.draw(ui, wi, m);
        }
        None => {}
    }
}

/// O livro do `GuiEnchantment.drawGuiContainerBackgroundLayer`: marca o ponto da GUI (depois do fundo, antes
/// das opções); o jogo monta o modelo com `inv.ui.book` (`Game::gui_models`).
pub fn draw_enchant_book(_inv: &Inventory, ui: &mut UiBatch, l: i32, t: i32, partial: f32) {
    let at = ui.indices.len() as u32;
    ui.gui3d = Some(mc_render::ui::Gui3d::Book { at, l, t, partial });
}
/// `GuiInventory.drawEntityOnScreen(x, y, scale, look.0 - mouseX, look.1 - mouseY, entidade)`: marca o ponto
/// da GUI onde o modelo entra; o jogo monta a geometria (`Game::gui_models`).
pub fn draw_entity_in_gui(inv: &Inventory, ui: &mut UiBatch, x: i32, y: i32, scale: i32, look: (i32, i32), entity: i32) {
    let (mx, my) = (inv.mouse.0.floor(), inv.mouse.1.floor());
    let at = ui.indices.len() as u32;
    ui.gui3d = Some(mc_render::ui::Gui3d::Entity { at, x, y, scale, mx: look.0 as f32 - mx, my: look.1 as f32 - my, entity });
}

impl Inventory {
    /// `NetHandlerPlayClient.handleSignEditorOpen` + `EntityPlayerSP.openEditSign`.
    pub fn open_sign(&mut self, pos: (i32, i32, i32), block_id: u16, lines: [String; 4]) {
        self.begin_special();
        self.special = Some(Special::Sign(SignGui::new(pos, block_id == STANDING_SIGN, lines)));
    }

    /// `GuiScreenBook`: `editable` é o livro e pena (abre ao clicar com ele na mão), senão o livro escrito
    /// (`MC|BOpen`). `slot_index` é a casa da mão em `player`.
    pub fn open_book(&mut self, book: Slot, editable: bool, author: String, slot_index: usize) {
        self.begin_special();
        self.special = Some(Special::Book(Box::new(BookGui::new(book, editable, author, slot_index))));
    }

    /// `displayGuiScreen(nova)`: a tela anterior fecha antes (o que ela precisar mandar vai junto).
    fn begin_special(&mut self) {
        if let Some(p) = self.close() {
            self.out.push(p);
        }
        self.own_screen = false;
        self.cursor = None;
        self.ui = Default::default();
        self.ui.last_ms = now_ms();
        self.ms = MouseState { ignore_mouse_up: true, ..Default::default() };
    }

    /// Texto atual da placa que está sendo editada (a tile entity muda enquanto se digita).
    pub fn editing_sign(&self) -> Option<((i32, i32, i32), &[String; 4])> {
        match &self.special {
            Some(Special::Sign(s)) => Some((s.pos, &s.lines)),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// GuiSleepMP
// ---------------------------------------------------------------------------------------------

/// O botão `multiplayer.stopSleeping` que o `GuiSleepMP` põe sobre o chat.
pub fn sleep_button(w: f32, h: f32) -> Button {
    let (w, h) = dims(w, h);
    Button::new(1, w / 2 - 100, h - 40, tr("multiplayer.stopSleeping"))
}

// ---------------------------------------------------------------------------------------------
// Ganchos de teste visual
// ---------------------------------------------------------------------------------------------

/// `MINE_RS_TELA_ESPECIAL=placa|placa_parede|livro|livro_edit|livro_assinar` no mundo de teste.
/// Devolve true para `cama` (o jogo marca o jogador como dormindo e deixa o laço normal abrir a tela).
pub fn test_hook(inv: &mut Inventory, name: &str, player: &str, hand_slot: usize) -> bool {
    let text = |s: &str| Nbt::String(s.to_string());
    let book = |id: i16, tag: Vec<(&str, Nbt)>| Slot {
        id,
        count: 1,
        damage: 0,
        nbt: {
            let t = Nbt::Compound(tag.into_iter().map(|(k, v)| (k.to_string(), v)).collect());
            Some(t)
        },
        raw_nbt: Vec::new(),
    };
    match name {
        "placa" | "placa_parede" => {
            let id = if name == "placa" { 63 } else { 68 };
            let lines = ["Primeira linha".to_string(), "§cLinha vermelha".to_string(), String::new(), "Fim".to_string()];
            inv.open_sign((3, 64, 3), id, lines);
        }
        "livro" => {
            let pages = vec![
                text(r##"{"text":"Capitulo Um\n\n","bold":true,"extra":[{"text":"Era uma vez um jogador que entrou no mundo e achou um livro escrito. ","bold":false},{"text":"Ir para a pagina 2","color":"dark_blue","underlined":true,"clickEvent":{"action":"change_page","value":"2"}}]}"##),
                text("\"Segunda pagina, so com texto simples.\""),
                text("Terceira pagina fora do formato JSON, mostrada como esta."),
            ];
            let mut b = book(WRITTEN_BOOK, vec![("title", text("Meu livro")), ("author", text(player)), ("pages", Nbt::List(pages))]);
            b.raw_nbt = b.nbt.as_ref().map(Nbt::write_root).unwrap_or_default();
            inv.open_book(b, false, player.to_string(), hand_slot);
        }
        "livro_edit" | "livro_assinar" => {
            let pages = vec![text("Texto que o jogador ja escreveu no livro e pena. Cada pagina cabe ate 14 linhas de 116 pixels."), text("")];
            let mut b = book(WRITABLE_BOOK, vec![("pages", Nbt::List(pages))]);
            b.raw_nbt = b.nbt.as_ref().map(Nbt::write_root).unwrap_or_default();
            inv.open_book(b, true, player.to_string(), hand_slot);
            if name == "livro_assinar" {
                if let Some(Special::Book(b)) = &mut inv.special {
                    b.signing = true;
                    b.title = "Aventuras".to_string();
                }
            }
        }
        "cama" => return true,
        _ => {}
    }
    false
}

#[cfg(test)]
mod tests;
