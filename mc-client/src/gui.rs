//! Widgets da GUI da 1.8.9: `GuiButton`, `GuiOptionButton`, `GuiOptionSlider`, slider de som,
//! `GuiButtonLanguage`, `GuiTextField`, `GuiSlot`/`GuiListExtended` e as funções de desenho do
//! `Gui`/`GuiScreen` (texto centralizado, fundo de terra, degradê, tooltip).
//!
//! Coordenadas em pixels de GUI inteiros, como o Java (`ScaledResolution`).

use std::time::Instant;

use mc_render::ui::{UiBatch, BACKGROUND, WIDGETS};

use crate::settings::{Opt, Settings};

/// `FontRenderer.getStringWidth`.
pub fn width(ui: &mut UiBatch, s: &str) -> i32 {
    ui.text_width(s) as i32
}

/// `drawCenteredString`: texto com sombra centrado em `cx`.
pub fn centered(ui: &mut UiBatch, s: &str, cx: i32, y: i32, color: u32) {
    let w = width(ui, s);
    ui.text((cx - w / 2) as f32, y as f32, s, color, true);
}

/// `drawString` (com sombra).
pub fn string(ui: &mut UiBatch, s: &str, x: i32, y: i32, color: u32) {
    ui.text(x as f32, y as f32, s, color, true);
}

/// `Gui.drawRect` com cor ARGB literal.
pub fn rect(ui: &mut UiBatch, x0: i32, y0: i32, x1: i32, y1: i32, argb: u32) {
    let (x0, x1) = (x0.min(x1), x0.max(x1));
    let (y0, y1) = (y0.min(y1), y0.max(y1));
    ui.gradient(x0 as f32, y0 as f32, (x1 - x0) as f32, (y1 - y0) as f32, argb, argb);
}

/// `drawGradientRect`.
pub fn gradient(ui: &mut UiBatch, x0: i32, y0: i32, x1: i32, y1: i32, top: u32, bottom: u32) {
    ui.gradient(x0 as f32, y0 as f32, (x1 - x0) as f32, (y1 - y0) as f32, top, bottom);
}

/// Ladrilhos do `options_background` (16 px na textura, 32 px na tela) cobrindo
/// `x0..x1, y0..y1`, com a textura deslocada `voff` px para cima e tingida com `rgb`.
#[allow(clippy::too_many_arguments)]
pub fn tile(ui: &mut UiBatch, x0: i32, y0: i32, x1: i32, y1: i32, voff: i32, rgb: u32, alpha: u8) {
    let (bx, by) = (BACKGROUND.0 as f32, BACKGROUND.1 as f32);
    let argb = ((alpha as u32) << 24) | (rgb & 0xFFFFFF);
    let mut y = y0;
    while y < y1 {
        let ty = (y + voff).rem_euclid(32);
        let h = (32 - ty).min(y1 - y);
        let mut x = x0;
        while x < x1 {
            let tx = x.rem_euclid(32);
            let w = (32 - tx).min(x1 - x);
            ui.sprite_alpha(x as f32, y as f32, w as f32, h as f32, bx + tx as f32 / 2.0, by + ty as f32 / 2.0, w as f32 / 2.0, h as f32 / 2.0, argb);
            x += w;
        }
        y += h;
    }
}

/// `GuiScreen.drawDefaultBackground`: degradê escuro sobre o mundo, senão terra.
pub fn default_background(ui: &mut UiBatch, w: i32, h: i32, in_game: bool) {
    if in_game {
        gradient(ui, 0, 0, w, h, 0xC010_1010, 0xD010_1010);
    } else {
        tile(ui, 0, 0, w, h, 0, 0x404040, 255);
    }
}

/// `GuiScreen.drawHoveringText`.
pub fn tooltip(ui: &mut UiBatch, lines: &[String], x: i32, y: i32, sw: i32, sh: i32) {
    if lines.is_empty() {
        return;
    }
    let tw = lines.iter().map(|l| width(ui, l)).max().unwrap_or(0);
    let mut l1 = x + 12;
    let mut i2 = y - 12;
    let mut k = 8;
    if lines.len() > 1 {
        k += 2 + (lines.len() as i32 - 1) * 10;
    }
    if l1 + tw > sw {
        l1 -= 28 + tw;
    }
    if i2 + k + 6 > sh {
        i2 = sh - k - 6;
    }
    let bg = 0xF010_0010;
    gradient(ui, l1 - 3, i2 - 4, l1 + tw + 3, i2 - 3, bg, bg);
    gradient(ui, l1 - 3, i2 + k + 3, l1 + tw + 3, i2 + k + 4, bg, bg);
    gradient(ui, l1 - 3, i2 - 3, l1 + tw + 3, i2 + k + 3, bg, bg);
    gradient(ui, l1 - 4, i2 - 3, l1 - 3, i2 + k + 3, bg, bg);
    gradient(ui, l1 + tw + 3, i2 - 3, l1 + tw + 4, i2 + k + 3, bg, bg);
    let b1 = 0x5050_00FF_u32;
    let b2 = ((b1 & 0x00FE_FEFE) >> 1) | (b1 & 0xFF00_0000);
    gradient(ui, l1 - 3, i2 - 2, l1 - 2, i2 + k + 2, b1, b2);
    gradient(ui, l1 + tw + 2, i2 - 2, l1 + tw + 3, i2 + k + 2, b1, b2);
    gradient(ui, l1 - 3, i2 - 3, l1 + tw + 3, i2 - 2, b1, b1);
    gradient(ui, l1 - 3, i2 + k + 2, l1 + tw + 3, i2 + k + 3, b2, b2);
    for (n, l) in lines.iter().enumerate() {
        string(ui, l, l1, i2, 0xFFFFFF);
        if n == 0 {
            i2 += 2;
        }
        i2 += 10;
    }
}

/// `FontRenderer.trimStringToWidth`: o maior prefixo (ou sufixo, com `reverse`) que cabe em `max`.
pub fn trim_to_width(ui: &mut UiBatch, s: &str, max: i32, reverse: bool) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out: Vec<char> = Vec::new();
    let (mut total, mut after_code, mut bold) = (0, false, false);
    let mut l: i64 = if reverse { chars.len() as i64 - 1 } else { 0 };
    let step: i64 = if reverse { -1 } else { 1 };
    while l >= 0 && (l as usize) < chars.len() && total < max {
        let c = chars[l as usize];
        let w = ui.char_width(c);
        if after_code {
            after_code = false;
            if c == 'l' || c == 'L' {
                bold = true;
            } else if c == 'r' || c == 'R' {
                bold = false;
            }
        } else if w < 0 {
            after_code = true;
        } else {
            total += w;
            if bold {
                total += 1;
            }
        }
        if total > max {
            break;
        }
        if reverse {
            out.insert(0, c);
        } else {
            out.push(c);
        }
        l += step;
    }
    out.into_iter().collect()
}

fn is_format_color(c: char) -> bool {
    c.is_ascii_digit() || ('a'..='f').contains(&c) || ('A'..='F').contains(&c)
}

fn is_format_special(c: char) -> bool {
    ('k'..='o').contains(&c) || ('K'..='O').contains(&c) || c == 'r' || c == 'R'
}

/// `FontRenderer.sizeStringToWidth`: quantos caracteres cabem em `max` (quebrando no último espaço).
fn size_to_width(ui: &mut UiBatch, s: &[char], max: i32, sga: bool) -> usize {
    let n = s.len();
    let (mut width, mut k, mut last_space) = (0, 0usize, -1i64);
    let mut bold = false;
    while k < n {
        let c = s[k];
        if c == '\n' {
            // `--k` + `++k` + `l = k`: pára no próprio \n.
            last_space = k as i64;
            break;
        }
        if c == '§' {
            if k + 1 < n {
                k += 1;
                let c1 = s[k];
                if c1 == 'l' || c1 == 'L' {
                    bold = true;
                } else if c1 == 'r' || c1 == 'R' || is_format_color(c1) {
                    bold = false;
                }
            }
        } else {
            if c == ' ' {
                last_space = k as i64;
            }
            width += if sga { ui.sga_char_width(c) } else { ui.char_width(c) };
            if bold {
                width += 1;
            }
        }
        if width > max {
            break;
        }
        k += 1;
    }
    if k != n && last_space != -1 && (last_space as usize) < k { last_space as usize } else { k }
}

/// `FontRenderer.getFormatFromString`: a última cor e os estilos ativos.
pub fn format_from_string(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    for i in 0..chars.len() {
        if chars[i] == '§' && i + 1 < chars.len() {
            let c = chars[i + 1];
            if is_format_color(c) {
                out = format!("§{c}");
            } else if is_format_special(c) {
                out.push('§');
                out.push(c);
            }
        }
    }
    out
}

/// `FontRenderer.wrapFormattedStringToWidth`.
fn wrap_formatted(ui: &mut UiBatch, text: &str, max: i32, sga: bool) -> String {
    let mut out = String::new();
    let mut rest: Vec<char> = text.chars().collect();
    loop {
        let i = size_to_width(ui, &rest, max, sga);
        if rest.len() <= i {
            out.extend(rest);
            return out;
        }
        let head: String = rest[..i].iter().collect();
        let c = rest[i];
        let skip = c == ' ' || c == '\n';
        let tail: String = rest[i + skip as usize..].iter().collect();
        out.push_str(&head);
        out.push('\n');
        rest = (format_from_string(&head) + &tail).chars().collect();
    }
}

/// `FontRenderer.listFormattedStringToWidth`: quebra por largura levando a cor para a linha seguinte
/// (`Arrays.asList(wrap.split("\n"))`: o `split` do Java descarta as linhas vazias do fim).
pub fn wrap(ui: &mut UiBatch, text: &str, max: i32) -> Vec<String> {
    wrap_font(ui, text, max, false)
}

/// `wrap` com as larguras da fonte galáctica (`standardGalacticFontRenderer.drawSplitString`).
pub fn wrap_sga(ui: &mut UiBatch, text: &str, max: i32) -> Vec<String> {
    wrap_font(ui, text, max, true)
}

fn wrap_font(ui: &mut UiBatch, text: &str, max: i32, sga: bool) -> Vec<String> {
    let wrapped = wrap_formatted(ui, text, max, sga);
    if !wrapped.contains('\n') {
        return vec![wrapped];
    }
    let mut lines: Vec<String> = wrapped.split('\n').map(str::to_string).collect();
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// `GuiButton`.
    Plain,
    /// `GuiOptionButton` ligado a uma opção (alterna no clique).
    Toggle(Opt),
    /// `GuiOptionSlider`.
    Slider(Opt),
    /// Slider de volume do `GuiScreenOptionsSounds` (índice da categoria).
    Sound(usize),
    /// `GuiButtonLanguage` (ícone do globo).
    Language,
    /// Parte da skin (`GuiCustomizeSkin.ButtonPart`).
    Part(usize),
}

#[derive(Clone, Debug)]
pub struct Button {
    pub id: i32,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub text: String,
    pub enabled: bool,
    pub visible: bool,
    pub kind: Kind,
    /// Posição do slider (0..1).
    pub value: f32,
    pub dragging: bool,
}

impl Button {
    /// `new GuiButton(id, x, y, text)`: 200x20.
    pub fn new(id: i32, x: i32, y: i32, text: impl Into<String>) -> Self {
        Self::sized(id, x, y, 200, 20, text)
    }

    pub fn sized(id: i32, x: i32, y: i32, w: i32, h: i32, text: impl Into<String>) -> Self {
        Self { id, x, y, w, h, text: text.into(), enabled: true, visible: true, kind: Kind::Plain, value: 0.0, dragging: false }
    }

    /// `GuiOptionButton`/`GuiOptionSlider` para a opção `o` (150x20), como `GuiOptions` cria.
    pub fn option(x: i32, y: i32, o: Opt, s: &Settings) -> Self {
        let mut b = Self::sized(o as i32, x, y, 150, 20, s.display(o));
        if o.is_float() {
            b.kind = Kind::Slider(o);
            b.value = o.normalize(s.float(o));
        } else {
            b.kind = Kind::Toggle(o);
        }
        b
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn hovered(&self, mx: i32, my: i32) -> bool {
        mx >= self.x && my >= self.y && mx < self.x + self.w && my < self.y + self.h
    }

    /// `GuiButton.mousePressed`.
    pub fn pressed(&self, mx: i32, my: i32) -> bool {
        self.enabled && self.visible && self.hovered(mx, my)
    }

    /// `GuiButton.drawButton` (com o `mouseDragged` dos sliders).
    pub fn draw(&self, ui: &mut UiBatch, mx: i32, my: i32) {
        if !self.visible {
            return;
        }
        let hovered = self.hovered(mx, my);
        if self.kind == Kind::Language {
            let v = if hovered { 126.0 } else { 106.0 };
            ui.gui(WIDGETS, self.x as f32, self.y as f32, 0.0, v, self.w as f32, self.h as f32);
            return;
        }
        let state = match self.kind {
            Kind::Slider(_) | Kind::Sound(_) => 0,
            _ if !self.enabled => 0,
            _ if hovered => 2,
            _ => 1,
        };
        let v = 46.0 + state as f32 * 20.0;
        let half = self.w / 2;
        ui.gui(WIDGETS, self.x as f32, self.y as f32, 0.0, v, half as f32, self.h as f32);
        ui.gui(WIDGETS, (self.x + half) as f32, self.y as f32, (200 - half) as f32, v, half as f32, self.h as f32);
        if matches!(self.kind, Kind::Slider(_) | Kind::Sound(_)) {
            let kx = self.x + (self.value * (self.w - 8) as f32) as i32;
            ui.gui(WIDGETS, kx as f32, self.y as f32, 0.0, 66.0, 4.0, 20.0);
            ui.gui(WIDGETS, (kx + 4) as f32, self.y as f32, 196.0, 66.0, 4.0, 20.0);
        }
        let color = if !self.enabled {
            0xA0A0A0
        } else if hovered {
            0xFFFFA0
        } else {
            0xE0E0E0
        };
        centered(ui, &self.text, self.x + self.w / 2, self.y + (self.h - 8) / 2, color);
    }

    /// Posição do slider para o mouse em `mx` (`(mx - (x + 4)) / (w - 8)`).
    pub fn slider_at(&self, mx: i32) -> f32 {
        ((mx - (self.x + 4)) as f32 / (self.w - 8) as f32).clamp(0.0, 1.0)
    }

    /// Slider de opção: aplica o valor do mouse e atualiza o texto.
    pub fn slide(&mut self, mx: i32, s: &mut Settings) {
        match self.kind {
            Kind::Slider(o) => {
                let v = o.denormalize(self.slider_at(mx));
                s.set_float(o, v);
                self.value = o.normalize(v);
                self.text = s.display(o);
            }
            Kind::Sound(cat) => {
                self.value = self.slider_at(mx);
                s.sound[cat] = self.value;
                s.save();
                self.text = s.sound_display(cat);
            }
            _ => {}
        }
    }
}

/// `ChatAllowedCharacters.isAllowedCharacter`.
pub fn allowed_char(c: char) -> bool {
    c != '§' && c >= ' ' && c != '\u{7f}'
}

/// `GuiTextField`.
#[derive(Clone, Debug)]
pub struct TextField {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub text: String,
    pub max_len: usize,
    pub focused: bool,
    /// `setEnableBackgroundDrawing`.
    pub background: bool,
    /// `setTextColor`.
    pub color: u32,
    /// `setCanLoseFocus` (o campo do chat nunca perde o foco).
    pub can_lose_focus: bool,
    cursor: usize,
    sel_end: usize,
    scroll: usize,
    born: Instant,
}

impl TextField {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self {
            x,
            y,
            w,
            h,
            text: String::new(),
            max_len: 32,
            focused: false,
            background: true,
            color: 0xE0E0E0,
            can_lose_focus: true,
            cursor: 0,
            sel_end: 0,
            scroll: 0,
            born: Instant::now(),
        }
    }

    fn len(&self) -> usize {
        self.text.chars().count()
    }

    fn slice(&self, a: usize, b: usize) -> String {
        self.text.chars().skip(a).take(b.saturating_sub(a)).collect()
    }

    /// `setFocused`: ganhar o foco zera o contador do cursor.
    pub fn set_focused(&mut self, on: bool) {
        if on && !self.focused {
            self.born = Instant::now();
        }
        self.focused = on;
    }

    /// `getCursorPosition`.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// `setText`: corta no máximo e põe o cursor no fim.
    pub fn set_text(&mut self, ui: &mut UiBatch, t: &str) {
        self.text = t.chars().take(self.max_len).collect();
        self.set_cursor(ui, self.len());
    }

    /// `setMaxStringLength`.
    #[allow(dead_code)]
    pub fn set_max_len(&mut self, n: usize) {
        self.max_len = n;
        if self.len() > n {
            self.text = self.slice(0, n);
        }
    }

    /// `getWidth` com fundo: largura útil.
    fn inner(&self) -> i32 {
        if self.background { self.w - 8 } else { self.w }
    }

    /// `setCursorPosition` + `setSelectionPos`.
    pub fn set_cursor(&mut self, ui: &mut UiBatch, pos: usize) {
        self.set_cursor_i(ui, pos as i64);
    }

    fn set_cursor_i(&mut self, ui: &mut UiBatch, pos: i64) {
        self.cursor = pos.clamp(0, self.len() as i64) as usize;
        self.set_selection(ui, self.cursor as i64);
    }

    /// `moveCursorBy`: relativo ao fim da seleção.
    fn move_cursor_by(&mut self, ui: &mut UiBatch, d: i64) {
        self.set_cursor_i(ui, self.sel_end as i64 + d);
    }

    /// `setSelectionPos`, com a rolagem horizontal do campo.
    pub fn set_selection(&mut self, ui: &mut UiBatch, pos: i64) {
        let len = self.len() as i64;
        let pos = pos.clamp(0, len);
        self.sel_end = pos as usize;
        let mut scroll = (self.scroll as i64).min(len);
        let w = self.inner();
        let visible = trim_to_width(ui, &self.slice(scroll as usize, self.len()), w, false);
        let k = visible.chars().count() as i64 + scroll;
        if pos == scroll {
            scroll -= trim_to_width(ui, &self.text, w, true).chars().count() as i64;
        }
        if pos > k {
            scroll += pos - k;
        } else if pos <= scroll {
            scroll -= scroll - pos;
        }
        self.scroll = scroll.clamp(0, len) as usize;
    }

    /// `getSelectedText`.
    pub fn selected_text(&self) -> String {
        self.slice(self.cursor.min(self.sel_end), self.cursor.max(self.sel_end))
    }

    /// `writeText`: troca a seleção pelo texto filtrado, respeitando o máximo.
    pub fn write_text(&mut self, ui: &mut UiBatch, s: &str) {
        let filtered: Vec<char> = s.chars().filter(|c| allowed_char(*c)).collect();
        let (a, b) = (self.cursor.min(self.sel_end), self.cursor.max(self.sel_end));
        let room = (self.max_len as i64 - self.len() as i64 + (b - a) as i64).max(0) as usize;
        let take = room.min(filtered.len());
        let ins: String = filtered[..take].iter().collect();
        self.text = self.slice(0, a) + &ins + &self.slice(b, self.len());
        self.set_cursor_i(ui, (a + take) as i64);
    }

    /// `deleteFromCursor`.
    pub fn delete_from_cursor(&mut self, ui: &mut UiBatch, n: i32) {
        if self.text.is_empty() {
            return;
        }
        if self.sel_end != self.cursor {
            self.write_text(ui, "");
            return;
        }
        let back = n < 0;
        let a = if back { self.cursor as i64 + n as i64 } else { self.cursor as i64 };
        let b = if back { self.cursor as i64 } else { self.cursor as i64 + n as i64 };
        let mut res = String::new();
        if a >= 0 {
            res = self.slice(0, a as usize);
        }
        if b < self.len() as i64 {
            res += &self.slice(b.max(0) as usize, self.len());
        }
        self.text = res;
        if back {
            self.move_cursor_by(ui, n as i64);
        }
    }

    /// `deleteWords`.
    pub fn delete_words(&mut self, ui: &mut UiBatch, n: i32) {
        if self.text.is_empty() {
            return;
        }
        if self.sel_end != self.cursor {
            self.write_text(ui, "");
        } else {
            let to = self.nth_word(n, self.cursor, true) as i32;
            self.delete_from_cursor(ui, to - self.cursor as i32);
        }
    }

    /// `func_146197_a` (`getNthWordFromPos`): posição da `n`-ésima palavra a partir de `pos`
    /// (`n < 0` anda para trás); `skip_spaces` pula os espaços depois da palavra.
    pub fn nth_word(&self, n: i32, pos: usize, skip_spaces: bool) -> usize {
        let chars: Vec<char> = self.text.chars().collect();
        let len = chars.len();
        let mut p = pos.min(len);
        for _ in 0..n.unsigned_abs() {
            if n >= 0 {
                match chars[p..].iter().position(|c| *c == ' ') {
                    None => p = len,
                    Some(i) => {
                        p += i;
                        while skip_spaces && p < len && chars[p] == ' ' {
                            p += 1;
                        }
                    }
                }
            } else {
                while skip_spaces && p > 0 && chars[p - 1] == ' ' {
                    p -= 1;
                }
                while p > 0 && chars[p - 1] != ' ' {
                    p -= 1;
                }
            }
        }
        p
    }

    /// `textboxKeyTyped`. Devolve true se a tecla foi usada.
    pub fn key(&mut self, ui: &mut UiBatch, code: winit::keyboard::KeyCode, text: Option<&str>, ctrl: bool, shift: bool) -> bool {
        use winit::keyboard::KeyCode::*;
        if !self.focused {
            return false;
        }
        match code {
            KeyA if ctrl && !shift => {
                self.set_cursor(ui, self.len());
                self.set_selection(ui, 0);
            }
            KeyC if ctrl && !shift => {
                crate::desktop::set(&self.selected_text());
            }
            KeyV if ctrl && !shift => {
                let t = crate::desktop::get();
                self.write_text(ui, &t);
            }
            KeyX if ctrl && !shift => {
                crate::desktop::set(&self.selected_text());
                self.write_text(ui, "");
            }
            Backspace => {
                if ctrl {
                    self.delete_words(ui, -1);
                } else {
                    self.delete_from_cursor(ui, -1);
                }
            }
            Delete => {
                if ctrl {
                    self.delete_words(ui, 1);
                } else {
                    self.delete_from_cursor(ui, 1);
                }
            }
            Home => {
                if shift {
                    self.set_selection(ui, 0);
                } else {
                    self.set_cursor(ui, 0);
                }
            }
            End => {
                if shift {
                    self.set_selection(ui, self.len() as i64);
                } else {
                    self.set_cursor(ui, self.len());
                }
            }
            ArrowLeft => {
                if shift {
                    let to = if ctrl { self.nth_word(-1, self.sel_end, true) as i64 } else { self.sel_end as i64 - 1 };
                    self.set_selection(ui, to);
                } else if ctrl {
                    let to = self.nth_word(-1, self.cursor, true);
                    self.set_cursor(ui, to);
                } else {
                    self.move_cursor_by(ui, -1);
                }
            }
            ArrowRight => {
                if shift {
                    let to = if ctrl { self.nth_word(1, self.sel_end, true) as i64 } else { self.sel_end as i64 + 1 };
                    self.set_selection(ui, to);
                } else if ctrl {
                    let to = self.nth_word(1, self.cursor, true);
                    self.set_cursor(ui, to);
                } else {
                    self.move_cursor_by(ui, 1);
                }
            }
            _ => match text {
                Some(t) if !ctrl && t.chars().any(allowed_char) => self.write_text(ui, t),
                _ => return false,
            },
        }
        true
    }

    /// `mouseClicked`: foca (se puder perder o foco) e posiciona o cursor com o botão esquerdo.
    pub fn click(&mut self, ui: &mut UiBatch, mx: i32, my: i32) {
        let inside = mx >= self.x && mx < self.x + self.w && my >= self.y && my < self.y + self.h;
        if self.can_lose_focus {
            self.set_focused(inside);
        }
        if self.focused && inside {
            let rel = mx - self.x - if self.background { 4 } else { 0 };
            let visible = trim_to_width(ui, &self.slice(self.scroll, self.len()), self.inner(), false);
            let n = trim_to_width(ui, &visible, rel, false).chars().count();
            self.set_cursor(ui, n + self.scroll);
        }
    }

    /// `drawTextBox`.
    pub fn draw(&self, ui: &mut UiBatch) {
        if self.background {
            rect(ui, self.x - 1, self.y - 1, self.x + self.w + 1, self.y + self.h + 1, 0xFFA0_A0A0);
            rect(ui, self.x, self.y, self.x + self.w, self.y + self.h, 0xFF00_0000);
        }
        let color = self.color;
        let c = self.cursor as i32 - self.scroll as i32;
        let mut sel = self.sel_end as i32 - self.scroll as i32;
        let visible = trim_to_width(ui, &self.slice(self.scroll, self.len()), self.inner(), false);
        let vlen = visible.chars().count() as i32;
        let cursor_in = c >= 0 && c <= vlen;
        let ticks = self.born.elapsed().as_millis() / 50;
        let blink = self.focused && (ticks / 6) % 2 == 0 && cursor_in;
        let x0 = if self.background { self.x + 4 } else { self.x };
        let y0 = if self.background { self.y + (self.h - 8) / 2 } else { self.y };
        let mut x = x0;
        if sel > vlen {
            sel = vlen;
        }
        let vis_slice = |a: i32, b: i32| -> String { visible.chars().skip(a.max(0) as usize).take((b - a).max(0) as usize).collect() };
        if vlen > 0 {
            let first = if cursor_in { vis_slice(0, c) } else { visible.clone() };
            // drawStringWithShadow devolve o fim da sombra (um pixel à frente).
            x = x0 + ui.text(x0 as f32, y0 as f32, &first, color, true) as i32 + 1;
        }
        let at_limit = self.cursor < self.len() || self.len() >= self.max_len;
        let mut cx = x;
        if !cursor_in {
            cx = if c > 0 { x0 + self.w } else { x0 };
        } else if at_limit {
            cx = x - 1;
            x -= 1;
        }
        if vlen > 0 && cursor_in && c < vlen {
            ui.text(x as f32, y0 as f32, &vis_slice(c, vlen), color, true);
        }
        if blink {
            if at_limit {
                rect(ui, cx, y0 - 1, cx + 1, y0 + 1 + 9, 0xFFD0_D0D0);
            } else {
                ui.text(cx as f32, y0 as f32, "_", color, true);
            }
        }
        if sel != c {
            let sx = x0 + width(ui, &vis_slice(0, sel));
            // drawCursorVertical inverte as cores com azul; aqui um véu azul.
            rect(ui, cx, y0 - 1, sx - 1, y0 + 1 + 9, 0x8000_00FF);
        }
    }
}

/// `GuiSlot`: lista com rolagem, fundo de terra e barra.
#[derive(Clone, Debug)]
pub struct SlotList {
    /// X da borda esquerda (`GuiSlot.left`); 0 nas listas de tela cheia.
    pub left: i32,
    pub width: i32,
    pub height: i32,
    pub top: i32,
    pub bottom: i32,
    pub slot_height: i32,
    pub header: i32,
    pub scroll: f32,
    initial_click_y: i32,
    scroll_mult: f32,
    pub selected: i32,
    last_clicked: Option<Instant>,
    /// `GuiSlot.enabled`: falso enquanto um botão da lista está pressionado.
    enabled: bool,
}

impl SlotList {
    pub fn new(width: i32, height: i32, top: i32, bottom: i32, slot_height: i32) -> Self {
        Self {
            left: 0,
            width,
            height,
            top,
            bottom,
            slot_height,
            header: 0,
            scroll: 0.0,
            initial_click_y: -2,
            scroll_mult: 1.0,
            selected: -1,
            last_clicked: None,
            enabled: true,
        }
    }

    pub fn content_height(&self, size: usize) -> i32 {
        size as i32 * self.slot_height + self.header
    }

    pub fn max_scroll(&self, size: usize) -> i32 {
        (self.content_height(size) - (self.bottom - self.top - 4)).max(0)
    }

    pub fn bind(&mut self, size: usize) {
        self.scroll = self.scroll.clamp(0.0, self.max_scroll(size) as f32);
    }

    /// `scrollBy`.
    pub fn scroll_by(&mut self, d: i32, size: usize) {
        self.scroll += d as f32;
        self.bind(size);
        self.initial_click_y = -2;
    }

    /// X da coluna das entradas (`k` do `drawScreen`).
    pub fn entry_x(&self, list_width: i32) -> i32 {
        self.left + self.width / 2 - list_width / 2 + 2
    }

    /// Y do topo da entrada `i`.
    pub fn entry_y(&self, i: usize) -> i32 {
        self.top + 4 - self.scroll as i32 + i as i32 * self.slot_height + self.header
    }

    /// Fundo (`drawContainerBackground`). Chamar antes das entradas.
    pub fn draw_background(&mut self, ui: &mut UiBatch, size: usize) {
        self.bind(size);
        tile(ui, self.left, self.top, self.left + self.width, self.bottom, self.scroll as i32, 0x202020, 255);
    }

    /// Moldura do `GuiSlot.drawSelectionBox` para a entrada selecionada.
    pub fn draw_selection(&self, ui: &mut UiBatch, i: usize, list_width: i32) {
        let k = self.entry_y(i);
        let l = self.slot_height - 4;
        let i1 = self.left + self.width / 2 - list_width / 2;
        let j1 = self.left + self.width / 2 + list_width / 2;
        rect(ui, i1, k - 2, j1, k + l + 2, 0xFF80_8080);
        rect(ui, i1 + 1, k - 1, j1 - 1, k + l + 1, 0xFF00_0000);
    }

    /// Faixas de terra, sombras e barra de rolagem. Chamar depois das entradas.
    pub fn draw_foreground(&self, ui: &mut UiBatch, size: usize, scrollbar_x: i32) {
        let (l, r) = (self.left, self.left + self.width);
        tile(ui, l, 0, r, self.top, 0, 0x404040, 255);
        tile(ui, l, self.bottom, r, self.height, 0, 0x404040, 255);
        gradient(ui, l, self.top, r, self.top + 4, 0xFF00_0000, 0x0000_0000);
        gradient(ui, l, self.bottom - 4, r, self.bottom, 0x0000_0000, 0xFF00_0000);
        let max = self.max_scroll(size);
        if max > 0 {
            let span = self.bottom - self.top;
            let k1 = (span * span / self.content_height(size)).clamp(32, span - 8);
            let l1 = (self.scroll as i32 * (span - k1) / max + self.top).max(self.top);
            let (i, j) = (scrollbar_x, scrollbar_x + 6);
            rect(ui, i, self.top, j, self.bottom, 0xFF00_0000);
            rect(ui, i, l1, j, l1 + k1, 0xFF80_8080);
            rect(ui, i, l1, j - 1, l1 + k1 - 1, 0xFFC0_C0C0);
        }
    }

    pub fn y_inside(&self, my: i32) -> bool {
        my >= self.top && my <= self.bottom
    }

    /// `getSlotIndexFromScreenCoords`.
    pub fn index_at(&self, mx: i32, my: i32, list_width: i32, size: usize, scrollbar_x: i32) -> Option<usize> {
        let i = self.left + self.width / 2 - list_width / 2;
        let j = self.left + self.width / 2 + list_width / 2;
        let k = my - self.top - self.header + self.scroll as i32 - 4;
        let l = k / self.slot_height;
        (mx < scrollbar_x && mx >= i && mx <= j && l >= 0 && k >= 0 && (l as usize) < size).then_some(l as usize)
    }

    /// Botão da lista pressionado: trava o arraste (`GuiListExtended.mouseClicked`).
    pub fn entry_consumed(&mut self) {
        self.enabled = false;
    }

    /// Clique (`handleMouseInput` com o botão descendo). Devolve (entrada, clique duplo).
    pub fn press(&mut self, mx: i32, my: i32, list_width: i32, size: usize, scrollbar_x: i32) -> Option<(usize, bool)> {
        if !self.y_inside(my) {
            return None;
        }
        let i = self.left + (self.width - list_width) / 2;
        let j = self.left + (self.width + list_width) / 2;
        let k = my - self.top - self.header + self.scroll as i32 - 4;
        let l = k / self.slot_height;
        let mut hit = None;
        let mut drag = true;
        if (l as usize) < size && mx >= i && mx <= j && l >= 0 && k >= 0 {
            let double = l == self.selected && self.last_clicked.is_some_and(|t| t.elapsed().as_millis() < 250);
            hit = Some((l as usize, double));
            self.selected = l;
            self.last_clicked = Some(Instant::now());
        } else if mx >= i && mx <= j && k < 0 {
            drag = false;
        }
        if self.enabled {
            if mx >= scrollbar_x && mx <= scrollbar_x + 6 {
                let span = self.bottom - self.top;
                let max = self.max_scroll(size).max(1);
                let l1 = ((span * span) as f32 / self.content_height(size).max(1) as f32) as i32;
                let l1 = l1.clamp(32, span - 8);
                self.scroll_mult = -1.0 / ((span - l1) as f32 / max as f32);
            } else {
                self.scroll_mult = 1.0;
            }
            self.initial_click_y = if drag { my } else { -2 };
        }
        hit
    }

    /// Mouse movido com o botão segurado.
    pub fn drag(&mut self, my: i32, size: usize) {
        if self.enabled && self.initial_click_y >= 0 {
            self.scroll -= (my - self.initial_click_y) as f32 * self.scroll_mult;
            self.initial_click_y = my;
            self.bind(size);
        }
    }

    pub fn release(&mut self) {
        self.initial_click_y = -1;
        self.enabled = true;
    }

    /// Roda do mouse: meia entrada por passo.
    pub fn wheel(&mut self, up: bool, size: usize) {
        let d = if up { -1 } else { 1 };
        self.scroll += (d * self.slot_height / 2) as f32;
        self.bind(size);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_geometry() {
        // GuiOptionsRowList do vídeo em 427x240 (1280x720 na escala 3).
        let mut l = SlotList::new(427, 240, 32, 240 - 32, 25);
        assert_eq!(l.max_scroll(8), 8 * 25 - (176 - 4));
        assert_eq!(l.entry_y(0), 36);
        let sb = 427 / 2 + 124 + 32;
        assert_eq!(l.index_at(213, 37, 400, 8, sb), Some(0));
        assert_eq!(l.index_at(213, 62, 400, 8, sb), Some(1));
        assert_eq!(l.index_at(213, 34, 400, 8, sb), None);
        assert_eq!(l.index_at(sb, 37, 400, 8, sb), None);
        l.scroll = 10.0;
        assert_eq!(l.entry_y(1), 32 + 4 - 10 + 25);
        assert_eq!(l.index_at(213, 52, 400, 8, sb), Some(1));
        l.wheel(false, 8);
        assert_eq!(l.scroll, 22.0);
        l.wheel(false, 8);
        assert_eq!(l.scroll, 28.0);
    }

    #[test]
    fn slider_position() {
        let mut s = Settings::default();
        let mut b = Button::option(10, 0, Opt::Fov, &s);
        assert_eq!(b.w, 150);
        assert_eq!(b.value, 0.5);
        // Ponta esquerda (x + 4) = mínimo; ponta direita (x + w - 4) = máximo.
        b.slide(14, &mut s);
        assert_eq!(s.fov, 30.0);
        b.slide(10 + 150 - 4, &mut s);
        assert_eq!(s.fov, 110.0);
        b.slide(10 + 4 + 71, &mut s);
        assert_eq!(s.fov, 70.0);
        assert_eq!(b.value, 0.5);
    }
}
