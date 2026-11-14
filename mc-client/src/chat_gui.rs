//! `GuiNewChat` (as linhas do chat), `GuiChat` (a tela com o campo de texto) e os cliques e dicas
//! de `GuiScreen` (`handleComponentClick` / `handleComponentHover`), portados da 1.8.9.

use mc_protocol::nbt::{Nbt, Slot};
use mc_render::ui::UiBatch;
use winit::keyboard::KeyCode;

use crate::chat::{ClickAction, Component, HoverAction, HoverEvent, Kind, Span, Style};
use crate::gui::{self, TextField};
use crate::lang::Lang;

/// Linhas que o chat guarda (`drawnChatLines` e `chatLines`).
pub const KEEP_LINES: usize = 100;
/// Ticks até a linha sumir quando o chat está fechado.
pub const FADE_TICKS: i32 = 200;
/// `FontRenderer.FONT_HEIGHT`.
const FONT_HEIGHT: i32 = 9;

/// O que o `GuiNewChat` lê do `GameSettings`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChatCfg {
    /// `EnumChatVisibility`: 0 completo, 1 só comandos, 2 oculto.
    pub visibility: i32,
    pub opacity: f32,
    pub scale: f32,
    pub width: f32,
    pub height_focused: f32,
    pub height_unfocused: f32,
    /// `chatColours`.
    pub colors: bool,
    /// `chatLinks`.
    pub links: bool,
    /// `chatLinksPrompt`.
    pub links_prompt: bool,
}

impl Default for ChatCfg {
    fn default() -> Self {
        Self {
            visibility: 0,
            opacity: 1.0,
            scale: 1.0,
            width: 1.0,
            height_focused: 1.0,
            height_unfocused: 0.443_661_96,
            colors: true,
            links: true,
            links_prompt: true,
        }
    }
}

impl ChatCfg {
    /// `GuiNewChat.getChatWidth`.
    pub fn chat_width(&self) -> i32 {
        crate::settings::chat_width(self.width)
    }

    /// `GuiNewChat.getChatHeight`.
    pub fn chat_height(&self, open: bool) -> i32 {
        crate::settings::chat_height(if open { self.height_focused } else { self.height_unfocused })
    }

    /// `GuiNewChat.getLineCount`.
    pub fn line_count(&self, open: bool) -> i32 {
        self.chat_height(open) / FONT_HEIGHT
    }
}

/// `EnumChatFormatting.getTextWithoutFormattingCodes`: tira `§` + `[0-9A-FK-OR]` (sem distinguir maiúsculas).
pub fn strip_formatting(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '§' && chars.get(i + 1).is_some_and(|c| c.is_ascii_hexdigit() || ('k'..='o').contains(&c.to_ascii_lowercase()) || c.eq_ignore_ascii_case(&'r')) {
            i += 2;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// Uma linha já quebrada: o `ChatComponentText("")` raiz com um irmão por trecho.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Line {
    pub spans: Vec<Span>,
}

impl Line {
    /// `getFormattedText` da raiz: `§r` da própria raiz e, por trecho, estilo + texto + `§r`.
    pub fn formatted_text(&self) -> String {
        let mut s = String::from("§r");
        for sp in &self.spans {
            s.push_str(&sp.style.code());
            s.push_str(&sp.text);
            s.push_str("§r");
        }
        s
    }

    #[allow(dead_code)]
    pub fn plain(&self) -> String {
        strip_formatting(&self.spans.iter().map(|s| s.text.as_str()).collect::<String>())
    }
}

/// `GuiUtilRenderComponents.splitText`: quebra o componente em linhas de até `width` pixels.
/// `keep_space` conserva o espaço da quebra; `force_colors` ignora a opção `chatColours`.
pub fn split_text(ui: &mut UiBatch, comp: &Component, width: i32, keep_space: bool, force_colors: bool, chat_colors: bool) -> Vec<Line> {
    let strip = |s: String| if !force_colors && !chat_colors { strip_formatting(&s) } else { s };
    let mut used = 0;
    let mut cur = Line::default();
    let mut lines = Vec::new();
    let mut list: Vec<Span> = comp.spans();
    let mut j = 0;
    let mut guard = 0u32;
    while j < list.len() {
        // Largura menor que um caractere nunca avança (no Java também giraria sem parar).
        guard += 1;
        if guard > 200_000 {
            break;
        }
        let span = list[j].clone();
        let mut s = span.text.clone();
        let mut flag = false;
        if let Some(k) = s.find('\n') {
            let rest = s[k + 1..].to_string();
            s.truncate(k + 1);
            list.insert(j + 1, Span { text: rest, style: span.style.clone() });
            flag = true;
        }
        let s4 = strip(format!("{}{}", span.style.code(), s));
        let s5 = s4.strip_suffix('\n').unwrap_or(&s4).to_string();
        let mut w = ui.string_width(&s5);
        let mut piece = Span { text: s5, style: span.style.clone() };
        if used + w > width {
            let mut s2 = gui::trim_to_width(ui, &s4, width - used, false);
            let s4c: Vec<char> = s4.chars().collect();
            let n2 = s2.chars().count();
            if n2 < s4c.len() && s4c.len() > n2 {
                let mut s3: String = s4c[n2..].iter().collect();
                let s2c: Vec<char> = s2.chars().collect();
                match s2c.iter().rposition(|c| *c == ' ') {
                    Some(l) if ui.string_width(&s4c[..l].iter().collect::<String>()) > 0 => {
                        s2 = s4c[..l].iter().collect();
                        s3 = s4c[if keep_space { l + 1 } else { l }..].iter().collect();
                    }
                    _ if used > 0 && !s4.contains(' ') => {
                        s2 = String::new();
                        s3 = s4.clone();
                    }
                    _ => {}
                }
                s3 = gui::format_from_string(&s2) + &s3;
                list.insert(j + 1, Span { text: s3, style: span.style.clone() });
            }
            w = ui.string_width(&s2);
            piece = Span { text: s2, style: span.style.clone() };
            flag = true;
        }
        if used + w <= width {
            used += w;
            cur.spans.push(piece);
        } else {
            flag = true;
        }
        if flag {
            lines.push(std::mem::take(&mut cur));
            used = 0;
        }
        j += 1;
    }
    lines.push(cur);
    lines
}

/// `ChatLine` da lista bruta (`chatLines`): o componente original, para refazer a quebra.
struct Stored {
    counter: i32,
    comp: Component,
    id: i32,
}

/// `ChatLine` da lista desenhada (`drawnChatLines`).
pub struct Drawn {
    pub counter: i32,
    pub line: Line,
    pub id: i32,
}

/// `GuiNewChat`. O índice 0 de cada lista é a linha mais nova.
#[derive(Default)]
pub struct NewChat {
    /// `sentMessages`: o histórico do ↑/↓.
    pub sent: Vec<String>,
    lines: Vec<Stored>,
    pub drawn: Vec<Drawn>,
    pub scroll_pos: i32,
    pub is_scrolled: bool,
}

impl NewChat {
    /// `clearChatMessages` (F3+D; o `Hud` é recriado ao sair do servidor).
    pub fn clear(&mut self) {
        self.drawn.clear();
        self.lines.clear();
        self.sent.clear();
    }

    /// `addToSentMessages`: não repete a última.
    pub fn add_sent(&mut self, msg: &str) {
        if self.sent.last().map(String::as_str) != Some(msg) {
            self.sent.push(msg.to_string());
        }
    }

    /// `resetScroll`.
    pub fn reset_scroll(&mut self) {
        self.scroll_pos = 0;
        self.is_scrolled = false;
    }

    /// `scroll`.
    pub fn scroll(&mut self, n: i32, cfg: &ChatCfg, open: bool) {
        self.scroll_pos += n;
        let len = self.drawn.len() as i32;
        let count = cfg.line_count(open);
        if self.scroll_pos > len - count {
            self.scroll_pos = len - count;
        }
        if self.scroll_pos <= 0 {
            self.scroll_pos = 0;
            self.is_scrolled = false;
        }
    }

    /// `printChatMessage`.
    pub fn print(&mut self, ui: &mut UiBatch, cfg: &ChatCfg, open: bool, comp: &Component, counter: i32) {
        self.print_with_id(ui, cfg, open, comp, 0, counter);
    }

    /// `printChatMessageWithOptionalDeletion`: com `id != 0` troca a linha anterior de mesmo id.
    pub fn print_with_id(&mut self, ui: &mut UiBatch, cfg: &ChatCfg, open: bool, comp: &Component, id: i32, counter: i32) {
        self.set_chat_line(ui, cfg, open, comp, id, counter, false);
    }

    #[allow(clippy::too_many_arguments)]
    fn set_chat_line(&mut self, ui: &mut UiBatch, cfg: &ChatCfg, open: bool, comp: &Component, id: i32, counter: i32, refresh: bool) {
        if id != 0 {
            self.delete_line(id);
        }
        let width = (cfg.chat_width() as f32 / cfg.scale).floor() as i32;
        for line in split_text(ui, comp, width, false, false, cfg.colors) {
            if open && self.scroll_pos > 0 {
                self.is_scrolled = true;
                self.scroll(1, cfg, open);
            }
            self.drawn.insert(0, Drawn { counter, line, id });
        }
        self.drawn.truncate(KEEP_LINES);
        if !refresh {
            self.lines.insert(0, Stored { counter, comp: comp.clone(), id });
            self.lines.truncate(KEEP_LINES);
        }
    }

    /// `refreshChat`: refaz a quebra (mudou a largura, a escala ou as cores).
    pub fn refresh(&mut self, ui: &mut UiBatch, cfg: &ChatCfg, open: bool) {
        self.drawn.clear();
        self.reset_scroll();
        let stored: Vec<(Component, i32, i32)> = self.lines.iter().rev().map(|l| (l.comp.clone(), l.id, l.counter)).collect();
        for (comp, id, counter) in stored {
            self.set_chat_line(ui, cfg, open, &comp, id, counter, true);
        }
    }

    /// `deleteChatLine`: todas as linhas desenhadas com o id e a primeira da lista bruta.
    pub fn delete_line(&mut self, id: i32) {
        self.drawn.retain(|l| l.id != id);
        if let Some(p) = self.lines.iter().position(|l| l.id == id) {
            self.lines.remove(p);
        }
    }

    /// `drawChat`. `h` é a altura da tela em pixels de GUI (o jogo translada `h - 48`).
    pub fn draw(&mut self, ui: &mut UiBatch, cfg: &ChatCfg, open: bool, tick: i32, h: f32) {
        if cfg.visibility == 2 {
            return;
        }
        let count = cfg.line_count(open);
        let total = self.drawn.len() as i32;
        if total == 0 {
            return;
        }
        let opacity = cfg.opacity * 0.9 + 0.1;
        let scale = cfg.scale;
        let width = (cfg.chat_width() as f32 / scale).ceil() as i32;
        let saved = ui.transform;
        ui.transform = Some([scale, 0.0, 0.0, scale, 2.0, h - 48.0 + 20.0]);
        let mut shown = 0;
        let mut i = 0;
        while i + self.scroll_pos < total && i < count {
            let d = &self.drawn[(i + self.scroll_pos) as usize];
            let age = tick - d.counter;
            if age < FADE_TICKS || open {
                let mut k = age as f64 / 200.0;
                k = 1.0 - k;
                k *= 10.0;
                k = k.clamp(0.0, 1.0);
                k *= k;
                let mut alpha = (255.0 * k) as i32;
                if open {
                    alpha = 255;
                }
                alpha = (alpha as f32 * opacity) as i32;
                shown += 1;
                if alpha > 3 {
                    let y = -i * FONT_HEIGHT;
                    gui::rect(ui, 0, y - 9, width + 4, y, ((alpha / 2) as u32) << 24);
                    ui.text(0.0, (y - 8) as f32, &d.line.formatted_text(), ((alpha as u32) << 24) | 0xFFFFFF, true);
                }
            }
            i += 1;
        }
        if open {
            // translate(-3, 0): barra de rolagem à esquerda do texto.
            let total_h = total * FONT_HEIGHT + total;
            let shown_h = shown * FONT_HEIGHT + shown;
            let off = self.scroll_pos * shown_h / total;
            let bar = shown_h * shown_h / total_h;
            if total_h != shown_h {
                let alpha: u32 = if off > 0 { 170 } else { 96 };
                let color: u32 = if self.is_scrolled { 13382451 } else { 3355562 };
                gui::rect(ui, -3, -off, -1, -off - bar, color + (alpha << 24));
                gui::rect(ui, -1, -off, -2, -off - bar, 13421772 + (alpha << 24));
            }
        }
        ui.transform = saved;
    }

    /// `getChatComponent(Mouse.getX(), Mouse.getY())`: o trecho sob o mouse. `raw_x` e `raw_y` são
    /// pixels da janela com a origem embaixo à esquerda (LWJGL); `factor` é o fator de escala da GUI.
    pub fn component_at(&self, ui: &mut UiBatch, cfg: &ChatCfg, open: bool, raw_x: i32, raw_y: i32, factor: i32) -> Option<Span> {
        if !open || factor <= 0 {
            return None;
        }
        let scale = cfg.scale;
        let x = (raw_x / factor - 3) as f32 / scale;
        let y = (raw_y / factor - 27) as f32 / scale;
        let (x, y) = (x.floor() as i32, y.floor() as i32);
        if x < 0 || y < 0 {
            return None;
        }
        let n = cfg.line_count(open).min(self.drawn.len() as i32);
        if x > (cfg.chat_width() as f32 / scale).floor() as i32 || y >= FONT_HEIGHT * n + n {
            return None;
        }
        let idx = y / FONT_HEIGHT + self.scroll_pos;
        if idx < 0 || idx as usize >= self.drawn.len() {
            return None;
        }
        let mut acc = 0;
        for span in &self.drawn[idx as usize].line.spans {
            let shown = if cfg.colors { span.text.clone() } else { strip_formatting(&span.text) };
            acc += ui.string_width(&shown);
            if acc > x {
                return Some(span.clone());
            }
        }
        None
    }
}

/// Efeito que a tela do chat pede ao jogo (rede, janela, navegador).
#[derive(Clone, Debug, PartialEq)]
pub enum ChatEffect {
    /// `displayGuiScreen(null)`.
    Close,
    /// `sendChatMessage(msg, addToHistory)`.
    Send(String, bool),
    /// C14 Tab-Complete com o texto até o cursor.
    TabComplete(String),
    /// `GuiConfirmOpenLink` para a URL.
    ConfirmLink(String),
    /// `openWebLink` direto (sem confirmação).
    OpenLink(String),
}

/// `String.trim`.
fn java_trim(s: &str) -> &str {
    s.trim_matches(|c: char| c <= ' ')
}

/// `java.net.URI`: devolve o esquema se `s` é um URI válido; `Err` com o motivo, como o `URISyntaxException`.
pub fn uri_scheme(s: &str) -> Result<String, String> {
    let colon = s.find(':').filter(|i| {
        let head = &s[..*i];
        head.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) && head.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    });
    let Some(colon) = colon else {
        // Sem esquema: a URI relativa também é válida, mas o chat exige o protocolo.
        return Err("Missing protocol".to_string());
    };
    let rest = &s[colon + 1..];
    if rest.is_empty() {
        return Err("Expected scheme-specific part".to_string());
    }
    let chars: Vec<char> = rest.chars().collect();
    let mut fragment = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '%' => {
                if !(chars.get(i + 1).is_some_and(char::is_ascii_hexdigit) && chars.get(i + 2).is_some_and(char::is_ascii_hexdigit)) {
                    return Err("Malformed escape pair".to_string());
                }
                i += 2;
            }
            '#' if fragment => return Err("Illegal character in fragment".to_string()),
            '#' => fragment = true,
            ' ' | '"' | '<' | '>' | '\\' | '^' | '`' | '{' | '|' | '}' => return Err(format!("Illegal character: {c:?}")),
            c if c.is_control() => return Err("Illegal character".to_string()),
            _ => {}
        }
        i += 1;
    }
    Ok(s[..colon].to_string())
}

/// `StringUtils.getCommonPrefix`.
fn common_prefix(strs: &[String]) -> String {
    let Some(first) = strs.first() else { return String::new() };
    let first: Vec<char> = first.chars().collect();
    let mut n = first.len();
    for s in &strs[1..] {
        let s: Vec<char> = s.chars().collect();
        n = n.min(s.len()).min(first.iter().zip(&s).take_while(|(a, b)| a == b).count());
    }
    first[..n].iter().collect()
}

/// `GuiChat`.
pub struct ChatScreen {
    pub input: TextField,
    /// `defaultInputFieldText` (o `/` do atalho de comando).
    #[allow(dead_code)]
    pub default_text: String,
    /// Texto a aplicar quando houver `UiBatch` (o `initGui` do Java mede a fonte).
    init_text: Option<String>,
    history_buffer: String,
    /// `sentHistoryCursor`.
    sent_cursor: i32,
    names_found: bool,
    waiting: bool,
    autocomplete_index: usize,
    pub found_names: Vec<String>,
}

impl ChatScreen {
    /// `initGui`. `sent` é o tamanho do histórico enviado.
    pub fn new(default_text: &str, sent: usize) -> Self {
        let mut input = TextField::new(4, 0, 0, 12);
        input.max_len = 100;
        input.background = false;
        input.focused = true;
        input.can_lose_focus = false;
        Self {
            input,
            default_text: default_text.to_string(),
            init_text: Some(default_text.to_string()),
            history_buffer: String::new(),
            sent_cursor: sent as i32,
            names_found: false,
            waiting: false,
            autocomplete_index: 0,
            found_names: Vec::new(),
        }
    }

    fn ready(&mut self, ui: &mut UiBatch) {
        if let Some(t) = self.init_text.take() {
            self.input.set_text(ui, &t);
        }
    }

    /// Posição e largura do campo (`new GuiTextField(0, font, 4, height - 12, width - 4, 12)`).
    pub fn layout(&mut self, w: i32, h: i32) {
        self.input.x = 4;
        self.input.y = h - 12;
        self.input.w = w - 4;
        self.input.h = 12;
    }

    #[allow(dead_code)]
    pub fn text(&self) -> &str {
        &self.input.text
    }

    /// `setText(text, true)` / `setText(text, false)` (substitui ou escreve no cursor).
    pub fn set_text(&mut self, ui: &mut UiBatch, text: &str, replace: bool) {
        self.ready(ui);
        if replace {
            self.input.set_text(ui, text);
        } else {
            self.input.write_text(ui, text);
        }
    }

    /// `keyTyped`.
    #[allow(clippy::too_many_arguments)]
    pub fn key(&mut self, ui: &mut UiBatch, chat: &mut NewChat, cfg: &ChatCfg, tick: i32, code: KeyCode, text: Option<&str>, ctrl: bool, shift: bool) -> Vec<ChatEffect> {
        self.ready(ui);
        let mut fx = Vec::new();
        self.waiting = false;
        if code == KeyCode::Tab {
            self.autocomplete_player_names(ui, chat, cfg, tick, &mut fx);
        } else {
            self.names_found = false;
        }
        match code {
            KeyCode::Escape => fx.push(ChatEffect::Close),
            KeyCode::Enter | KeyCode::NumpadEnter => {
                let s = java_trim(&self.input.text).to_string();
                if !s.is_empty() {
                    fx.push(ChatEffect::Send(s, true));
                }
                fx.push(ChatEffect::Close);
            }
            KeyCode::ArrowUp => self.get_sent_history(ui, chat, -1),
            KeyCode::ArrowDown => self.get_sent_history(ui, chat, 1),
            KeyCode::PageUp => chat.scroll(cfg.line_count(true) - 1, cfg, true),
            KeyCode::PageDown => chat.scroll(-cfg.line_count(true) + 1, cfg, true),
            KeyCode::Tab => {}
            _ => {
                self.input.key(ui, code, text, ctrl, shift);
            }
        }
        fx
    }

    /// `getSentHistory`.
    pub fn get_sent_history(&mut self, ui: &mut UiBatch, chat: &NewChat, dir: i32) {
        self.ready(ui);
        let j = chat.sent.len() as i32;
        let i = (self.sent_cursor + dir).clamp(0, j);
        if i != self.sent_cursor {
            if i == j {
                self.sent_cursor = j;
                let buf = self.history_buffer.clone();
                self.input.set_text(ui, &buf);
            } else {
                if self.sent_cursor == j {
                    self.history_buffer = self.input.text.clone();
                }
                self.input.set_text(ui, &chat.sent[i as usize].clone());
                self.sent_cursor = i;
            }
        }
    }

    /// `autocompletePlayerNames`.
    fn autocomplete_player_names(&mut self, ui: &mut UiBatch, chat: &mut NewChat, cfg: &ChatCfg, tick: i32, fx: &mut Vec<ChatEffect>) {
        let word_start = |f: &TextField| f.nth_word(-1, f.cursor(), false);
        if self.names_found {
            let back = word_start(&self.input) as i32 - self.input.cursor() as i32;
            self.input.delete_from_cursor(ui, back);
            if self.autocomplete_index >= self.found_names.len() {
                self.autocomplete_index = 0;
            }
        } else {
            let i = word_start(&self.input);
            self.found_names.clear();
            self.autocomplete_index = 0;
            let upto: String = self.input.text.chars().take(self.input.cursor()).collect();
            // sendAutocompleteRequest: só com pelo menos um caractere antes do cursor.
            if !upto.is_empty() {
                fx.push(ChatEffect::TabComplete(upto));
                self.waiting = true;
            }
            if self.found_names.is_empty() {
                return;
            }
            self.names_found = true;
            let back = i as i32 - self.input.cursor() as i32;
            self.input.delete_from_cursor(ui, back);
        }
        if self.found_names.len() > 1 {
            let joined = self.found_names.join(", ");
            chat.print_with_id(ui, cfg, true, &Component::text(joined), 1, tick);
        }
        let name = strip_formatting(&self.found_names[self.autocomplete_index]);
        self.autocomplete_index += 1;
        self.input.write_text(ui, &name);
    }

    /// `onAutocompleteResponse` (S3A).
    pub fn on_autocomplete_response(&mut self, ui: &mut UiBatch, chat: &mut NewChat, cfg: &ChatCfg, tick: i32, names: &[String]) {
        if !self.waiting {
            return;
        }
        self.ready(ui);
        self.names_found = false;
        self.found_names = names.iter().filter(|s| !s.is_empty()).cloned().collect();
        let start = self.input.nth_word(-1, self.input.cursor(), false);
        let s1: String = self.input.text.chars().skip(start).collect();
        let s2 = strip_formatting(&common_prefix(names));
        if !s2.is_empty() && !s1.eq_ignore_ascii_case(&s2) {
            let back = start as i32 - self.input.cursor() as i32;
            self.input.delete_from_cursor(ui, back);
            self.input.write_text(ui, &s2);
        } else if !self.found_names.is_empty() {
            self.names_found = true;
            let mut fx = Vec::new();
            self.autocomplete_player_names(ui, chat, cfg, tick, &mut fx);
        }
    }

    /// `handleMouseInput` da roda: ±1 (×7 sem Shift) linhas.
    pub fn wheel(&self, chat: &mut NewChat, cfg: &ChatCfg, up: bool, shift: bool) {
        let mut i = if up { 1 } else { -1 };
        if !shift {
            i *= 7;
        }
        chat.scroll(i, cfg, true);
    }

    /// `GuiScreen.handleComponentClick`. `None` = não tratou (o clique segue para o campo de texto).
    pub fn handle_component_click(&mut self, ui: &mut UiBatch, span: Option<&Span>, cfg: &ChatCfg, shift: bool) -> Option<Vec<ChatEffect>> {
        let span = span?;
        if shift {
            if let Some(ins) = &span.style.insertion {
                self.set_text(ui, ins, false);
            }
            return None;
        }
        let ev = span.style.click.as_ref()?;
        let mut fx = Vec::new();
        match ev.action {
            ClickAction::OpenUrl => {
                if !cfg.links {
                    return None;
                }
                match uri_scheme(&ev.value) {
                    Ok(scheme) if matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https") => {
                        fx.push(if cfg.links_prompt { ChatEffect::ConfirmLink(ev.value.clone()) } else { ChatEffect::OpenLink(ev.value.clone()) });
                    }
                    Ok(scheme) => eprintln!("Can't open url for {}: Unsupported protocol: {}", ev.value, scheme.to_ascii_lowercase()),
                    Err(e) => eprintln!("Can't open url for {}: {e}", ev.value),
                }
            }
            ClickAction::SuggestCommand => self.set_text(ui, &ev.value, true),
            ClickAction::RunCommand => fx.push(ChatEffect::Send(ev.value.clone(), false)),
            ClickAction::ChangePage => eprintln!("Don't know how to handle ClickEvent{{action=change_page, value='{}'}}", ev.value),
        }
        Some(fx)
    }

    /// `mouseClicked`: o clique no texto do chat, senão o campo. `mx`/`my` em pixels de GUI.
    #[allow(clippy::too_many_arguments)]
    pub fn mouse_clicked(&mut self, ui: &mut UiBatch, span: Option<&Span>, cfg: &ChatCfg, button: i32, shift: bool, mx: i32, my: i32) -> Vec<ChatEffect> {
        self.ready(ui);
        if button == 0 {
            if let Some(fx) = self.handle_component_click(ui, span, cfg, shift) {
                return fx;
            }
        }
        if button == 0 {
            self.input.click(ui, mx, my);
        }
        Vec::new()
    }

    /// `drawScreen` sem a dica: fundo do campo e o campo.
    pub fn draw(&mut self, ui: &mut UiBatch, w: i32, h: i32) {
        self.ready(ui);
        self.layout(w, h);
        gui::rect(ui, 2, h - 14, w - 2, h - 2, 0x8000_0000);
        self.input.draw(ui);
    }
}

// ---------------------------------------------------------------- dicas (handleComponentHover)

/// O que a dica precisa saber do jogo.
pub struct HoverCtx<'a> {
    /// `advancedItemTooltips` (o `show_entity` só aparece com ela).
    pub advanced_tooltips: bool,
    pub lang: &'a Lang,
    /// `GameSettings.getKeyDisplayString(keyBindInventory)`, para a descrição de `openInventory`.
    pub inventory_key: String,
}

/// `Item.getByNameOrId`: `ResourceLocation` (`minecraft:nome` ou `nome`, sem distinguir maiúsculas)
/// e, se não achar, o número.
pub fn item_by_name_or_id(name: &str) -> Option<i16> {
    let (domain, path) = match name.split_once(':') {
        Some((d, p)) => (if d.is_empty() { "minecraft" } else { d }, p),
        None => ("minecraft", name),
    };
    let found = (domain.eq_ignore_ascii_case("minecraft"))
        .then(|| crate::gen_chat_tables::ITEM_NAMES.iter().find(|(_, n)| *n == path.to_ascii_lowercase()).map(|(i, _)| *i as i16))
        .flatten();
    found.or_else(|| name.parse::<i32>().ok().and_then(item_by_id))
}

/// `Item.getItemById`.
fn item_by_id(id: i32) -> Option<i16> {
    let id = u16::try_from(id).ok()?;
    crate::gen_chat_tables::ITEM_NAMES.binary_search_by_key(&id, |(i, _)| *i).ok().map(|_| id as i16)
}

fn nbt_number(n: &Nbt) -> Option<i64> {
    match n {
        Nbt::Byte(v) => Some(*v as i64),
        Nbt::Short(v) => Some(*v as i64),
        Nbt::Int(v) => Some(*v as i64),
        Nbt::Long(v) => Some(*v),
        _ => None,
    }
}

/// `ItemStack.loadItemStackFromNBT`: `None` se o item não existe.
pub fn item_from_nbt(nbt: &Nbt) -> Option<Slot> {
    let id = match nbt.get("id") {
        Some(Nbt::String(s)) => item_by_name_or_id(s)?,
        other => item_by_id(other.and_then(nbt_number).unwrap_or(0) as i16 as i32)?,
    };
    let count = nbt.get("Count").and_then(nbt_number).unwrap_or(0) as i8 as u8;
    let damage = (nbt.get("Damage").and_then(nbt_number).unwrap_or(0) as i16).max(0);
    let tag = match nbt.get("tag") {
        Some(t @ Nbt::Compound(_)) => Some(t.clone()),
        _ => None,
    };
    Some(Slot { id, count, damage, nbt: tag, raw_nbt: Vec::new() })
}

/// `EntityList.getIDFromString` (90 se não existe).
fn entity_id(name: &str) -> i32 {
    crate::gen_chat_tables::ENTITY_IDS.iter().find(|(n, _)| *n == name).map_or(90, |(_, i)| *i as i32)
}

/// `StatList.getOneShotStat` + `GuiScreen.handleComponentHover` para `show_achievement`.
fn achievement_lines(id: &str, ctx: &HoverCtx) -> Option<Vec<String>> {
    use crate::gen_chat_tables::{ACHIEVEMENTS, GENERAL_STATS};
    let achievement = ACHIEVEMENTS.iter().find(|(n, _)| *n == id);
    if achievement.is_none() && !GENERAL_STATS.contains(&id) {
        return None;
    }
    // Achievement.getStatName: verde, ou roxo escuro nas especiais; o statName é um ChatComponentTranslation.
    let mut name = Component { kind: Kind::Translate { key: id.to_string(), args: Vec::new() }, style: Style::default(), siblings: Vec::new() };
    if let Some((_, special)) = achievement {
        name.style.color = Some(if *special { '5' } else { 'a' });
    }
    let kind = if achievement.is_some() { "achievement" } else { "statistic" };
    let mut ty = Component { kind: Kind::Translate { key: format!("stats.tooltip.type.{kind}"), args: Vec::new() }, style: Style::default(), siblings: Vec::new() };
    ty.style.italic = Some(true);
    let mut lines = vec![name.formatted_text(), ty.formatted_text()];
    if achievement.is_some() {
        let desc_key = format!("{id}.desc");
        let mut desc = crate::lang::raw(&desc_key).unwrap_or(desc_key);
        if id == "achievement.openInventory" {
            // IStatStringFormat do Minecraft: String.format(texto, nome da tecla do inventário).
            desc = crate::lang::format_java(&desc, &[ctx.inventory_key.as_str()]);
        }
        lines.push(desc);
    }
    Some(lines)
}

/// `GuiScreen.handleComponentHover`: as linhas da dica, ou `None` se não há o que mostrar.
pub fn hover_lines(ui: &mut UiBatch, ev: &HoverEvent, ctx: &HoverCtx) -> Option<Vec<String>> {
    let invalid = |what: &str| Some(vec![format!("§cInvalid {what}!")]);
    match ev.action {
        HoverAction::ShowItem => {
            let slot = crate::snbt::parse(&ev.value.unformatted_text()).ok().and_then(|n| item_from_nbt(&n));
            match slot {
                Some(s) => Some(crate::gui_creative::tooltip_text(&s, ctx.lang)),
                None => invalid("Item"),
            }
        }
        HoverAction::ShowEntity => {
            if !ctx.advanced_tooltips {
                return None;
            }
            match crate::snbt::parse(&ev.value.unformatted_text()) {
                Ok(n) => {
                    let get = |k: &str| n.get(k).and_then(Nbt::as_str).unwrap_or("").to_string();
                    let mut lines = vec![get("name")];
                    if let Some(Nbt::String(t)) = n.get("type") {
                        lines.push(format!("Type: {t} ({})", entity_id(t)));
                    }
                    lines.push(get("id"));
                    Some(lines)
                }
                Err(_) => invalid("Entity"),
            }
        }
        HoverAction::ShowText => Some(ev.value.formatted_text().split('\n').map(str::to_string).collect()),
        HoverAction::ShowAchievement => match achievement_lines(&ev.value.unformatted_text(), ctx) {
            Some(mut lines) => {
                // O Java quebra só a descrição (3º item) em 150 px.
                if lines.len() > 2 {
                    let desc = lines.split_off(2).remove(0);
                    lines.extend(gui::wrap(ui, &desc, 150));
                }
                Some(lines)
            }
            None => invalid("statistic/achievement"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_render::ui::{Font, UiBatch};

    /// Fonte de testes: toda célula com 6 de largura e espaço com 4 (`Font::fixed_width`).
    fn ui() -> UiBatch {
        UiBatch::new(Font::fixed_width(6))
    }

    fn comp(j: &str) -> Component {
        Component::from_json(j)
    }

    fn plain_lines(ui: &mut UiBatch, c: &Component, width: i32) -> Vec<String> {
        split_text(ui, c, width, false, false, true).iter().map(Line::plain).collect()
    }

    #[test]
    fn short_text_stays_on_one_line() {
        let mut u = ui();
        let l = split_text(&mut u, &comp(r#"{"text":"oi","color":"red"}"#), 100, false, false, true);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].plain(), "oi");
        assert_eq!(l[0].spans[0].text, "§coi");
        assert_eq!(l[0].formatted_text(), "§r§c§coi§r");
    }

    #[test]
    fn long_text_breaks_at_the_last_space() {
        let mut u = ui();
        // 6 px por letra, 4 por espaço. "aaaa bbbb" em 40 px: o corte cai dentro de "bbbb" e volta ao
        // último espaço; o espaço fica no começo da linha seguinte (keepSpace = false no GuiNewChat).
        let lines = plain_lines(&mut u, &comp("\"aaaa bbbb\""), 40);
        assert_eq!(lines, ["aaaa", " bbbb"]);
        // Quando o corte cai exatamente num espaço o `lastIndexOf(" ")` recua mais uma palavra
        // (comportamento do Java): "aaaa bbbb cccc" em 55 px.
        let lines = plain_lines(&mut u, &comp("\"aaaa bbbb cccc\""), 55);
        assert_eq!(lines, ["aaaa", " bbbb", " cccc"]);
        // Palavra maior que a linha é cortada onde cabe.
        let lines = plain_lines(&mut u, &comp("\"abcdefghij\""), 30);
        assert_eq!(lines, ["abcde", "fghij"]);
    }

    #[test]
    fn color_carries_to_the_next_line() {
        let mut u = ui();
        let l = split_text(&mut u, &comp(r#"{"text":"aaaa bbbb","color":"green"}"#), 40, false, false, true);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].spans[0].text, "§aaaaa");
        // O trecho que sobrou ganha a cor de novo (código do estilo + getFormatFromString).
        assert_eq!(l[1].spans[0].text, "§a§a bbbb");
        assert_eq!(l[1].plain(), " bbbb");
    }

    #[test]
    fn newline_forces_a_break_and_keeps_style() {
        let mut u = ui();
        let l = split_text(&mut u, &comp(r#"{"text":"ab\ncd","color":"red"}"#), 100, false, false, true);
        assert_eq!(l.iter().map(Line::plain).collect::<Vec<_>>(), ["ab", "cd"]);
        assert_eq!(l[1].spans[0].style.color, Some('c'));
    }

    #[test]
    fn spans_keep_click_events_after_breaking() {
        let mut u = ui();
        let j = r#"{"text":"vai para ","extra":[{"text":"https://example.com/muito/longo","clickEvent":{"action":"open_url","value":"https://example.com"}}]}"#;
        let l = split_text(&mut u, &comp(j), 120, false, false, true);
        assert!(l.len() >= 2);
        let with_click: Vec<_> = l.iter().flat_map(|l| &l.spans).filter(|s| s.style.click.is_some()).collect();
        assert!(with_click.len() >= 2, "o clique tem que sobreviver à quebra");
    }

    #[test]
    fn colors_off_strips_codes_before_measuring() {
        let mut u = ui();
        let l = split_text(&mut u, &comp(r#"{"text":"ab","color":"red"}"#), 100, false, false, false);
        assert_eq!(l[0].spans[0].text, "ab");
    }

    #[test]
    fn new_chat_keeps_only_100_lines_newest_first() {
        let mut u = ui();
        let cfg = ChatCfg::default();
        let mut chat = NewChat::default();
        for i in 0..130 {
            chat.print(&mut u, &cfg, false, &Component::text(format!("linha {i}")), i);
        }
        assert_eq!(chat.drawn.len(), 100);
        assert_eq!(chat.drawn[0].line.plain(), "linha 129");
        assert_eq!(chat.drawn[99].line.plain(), "linha 30");
    }

    #[test]
    fn lines_with_id_replace_the_previous_one() {
        let mut u = ui();
        let cfg = ChatCfg::default();
        let mut chat = NewChat::default();
        chat.print(&mut u, &cfg, false, &Component::text("a"), 0);
        chat.print_with_id(&mut u, &cfg, false, &Component::text("opções 1"), 1, 1);
        chat.print(&mut u, &cfg, false, &Component::text("b"), 2);
        chat.print_with_id(&mut u, &cfg, false, &Component::text("opções 2"), 1, 3);
        let all: Vec<String> = chat.drawn.iter().map(|d| d.line.plain()).collect();
        assert_eq!(all, ["opções 2", "b", "a"]);
        chat.delete_line(1);
        assert_eq!(chat.drawn.len(), 2);
    }

    #[test]
    fn scroll_is_clamped_and_follows_new_lines_when_open() {
        let mut u = ui();
        let cfg = ChatCfg::default();
        let mut chat = NewChat::default();
        for i in 0..40 {
            chat.print(&mut u, &cfg, true, &Component::text(format!("{i}")), 0);
        }
        // Aberto: height_focused = 1 -> 180 px / 9 = 20 linhas visíveis.
        assert_eq!(cfg.line_count(true), 20);
        chat.scroll(7, &cfg, true);
        assert_eq!(chat.scroll_pos, 7);
        // Uma linha nova com o chat aberto e rolado empurra a posição junto (a leitura não se mexe).
        chat.print(&mut u, &cfg, true, &Component::text("nova"), 0);
        assert_eq!(chat.scroll_pos, 8);
        assert!(chat.is_scrolled);
        // O teto é o total menos as linhas visíveis.
        chat.scroll(100, &cfg, true);
        assert_eq!(chat.scroll_pos, 41 - 20);
        chat.scroll(-100, &cfg, true);
        assert_eq!(chat.scroll_pos, 0);
        assert!(!chat.is_scrolled);
    }

    #[test]
    fn finds_the_span_under_the_mouse() {
        let mut u = ui();
        let cfg = ChatCfg::default();
        let mut chat = NewChat::default();
        let j = r#"{"text":"ab","extra":[{"text":"cd","clickEvent":{"action":"run_command","value":"/x"}}]}"#;
        chat.print(&mut u, &cfg, true, &comp(j), 0);
        // Escala da GUI 2: linha 0 em y (de baixo) entre 28*2 e 37*2; texto começa em x = 2 (+3 do deslocamento).
        let raw = |gx: i32, gy: i32| (gx * 2, gy * 2);
        let at = |u: &mut UiBatch, chat: &NewChat, gx: i32, gy: i32| {
            let (rx, ry) = raw(gx, gy);
            chat.component_at(u, &cfg, true, rx, ry, 2)
        };
        // x = 3 + 6 -> logo depois do "a": ainda no primeiro trecho.
        let first = at(&mut u, &chat, 3 + 5, 28).unwrap();
        assert!(first.style.click.is_none());
        // x = 3 + 14: no segundo trecho (12 px de "ab" já passaram).
        let second = at(&mut u, &chat, 3 + 14, 28).unwrap();
        assert_eq!(second.style.click.unwrap().value, "/x");
        // Fora do texto (além do último trecho), longe da linha, e com o chat fechado: nada.
        assert!(at(&mut u, &chat, 3 + 40, 28).is_none());
        assert!(at(&mut u, &chat, 3 + 5, 100).is_none());
        assert!(chat.component_at(&mut u, &cfg, false, 16, 56, 2).is_none());
    }

    #[test]
    fn history_walks_back_and_restores_the_draft() {
        let mut u = ui();
        let mut chat = NewChat::default();
        chat.add_sent("um");
        chat.add_sent("dois");
        chat.add_sent("dois");
        assert_eq!(chat.sent, ["um", "dois"]);
        let mut s = ChatScreen::new("", chat.sent.len());
        s.set_text(&mut u, "rascunho", true);
        s.get_sent_history(&mut u, &chat, -1);
        assert_eq!(s.text(), "dois");
        s.get_sent_history(&mut u, &chat, -1);
        assert_eq!(s.text(), "um");
        s.get_sent_history(&mut u, &chat, -1);
        assert_eq!(s.text(), "um");
        s.get_sent_history(&mut u, &chat, 1);
        s.get_sent_history(&mut u, &chat, 1);
        assert_eq!(s.text(), "rascunho");
    }

    #[test]
    fn enter_sends_the_trimmed_line_and_closes() {
        let mut u = ui();
        let cfg = ChatCfg::default();
        let mut chat = NewChat::default();
        let mut s = ChatScreen::new("", 0);
        s.set_text(&mut u, "  oi  ", true);
        let fx = s.key(&mut u, &mut chat, &cfg, 0, KeyCode::Enter, None, false, false);
        assert_eq!(fx, [ChatEffect::Send("oi".into(), true), ChatEffect::Close]);
        // Linha vazia: só fecha.
        let mut s = ChatScreen::new("", 0);
        s.set_text(&mut u, "   ", true);
        assert_eq!(s.key(&mut u, &mut chat, &cfg, 0, KeyCode::Enter, None, false, false), [ChatEffect::Close]);
        assert_eq!(s.key(&mut u, &mut chat, &cfg, 0, KeyCode::Escape, None, false, false), [ChatEffect::Close]);
    }

    #[test]
    fn typing_is_limited_to_100_chars_and_filters_section_sign() {
        let mut u = ui();
        let cfg = ChatCfg::default();
        let mut chat = NewChat::default();
        let mut s = ChatScreen::new("", 0);
        s.key(&mut u, &mut chat, &cfg, 0, KeyCode::KeyA, Some(&"a".repeat(150)), false, false);
        assert_eq!(s.text().chars().count(), 100);
        let mut s = ChatScreen::new("", 0);
        s.key(&mut u, &mut chat, &cfg, 0, KeyCode::KeyA, Some("a§b"), false, false);
        assert_eq!(s.text(), "ab");
    }

    #[test]
    fn default_text_for_the_command_key() {
        let mut u = ui();
        let mut s = ChatScreen::new("/", 0);
        s.draw(&mut u, 400, 240);
        assert_eq!(s.text(), "/");
        assert_eq!(s.input.cursor(), 1);
    }

    #[test]
    fn tab_requests_completion_and_applies_the_common_prefix() {
        let mut u = ui();
        let cfg = ChatCfg::default();
        let mut chat = NewChat::default();
        let mut s = ChatScreen::new("", 0);
        s.set_text(&mut u, "oi Lo", true);
        let fx = s.key(&mut u, &mut chat, &cfg, 0, KeyCode::Tab, None, false, false);
        assert_eq!(fx, [ChatEffect::TabComplete("oi Lo".into())]);
        // Resposta com dois nomes de prefixo comum "Loha": o campo ganha "Loha".
        s.on_autocomplete_response(&mut u, &mut chat, &cfg, 0, &["Lohan".to_string(), "Lohana".to_string()]);
        assert_eq!(s.text(), "oi Lohan");
        // Segundo Tab: cicla pelos nomes e lista as opções no chat (linha de id 1).
        let fx = s.key(&mut u, &mut chat, &cfg, 1, KeyCode::Tab, None, false, false);
        assert_eq!(fx, [ChatEffect::TabComplete("oi Lohan".into())]);
    }

    #[test]
    fn tab_with_a_single_answer_completes_it() {
        let mut u = ui();
        let cfg = ChatCfg::default();
        let mut chat = NewChat::default();
        let mut s = ChatScreen::new("", 0);
        s.set_text(&mut u, "/tp Lo", true);
        s.key(&mut u, &mut chat, &cfg, 0, KeyCode::Tab, None, false, false);
        s.on_autocomplete_response(&mut u, &mut chat, &cfg, 0, &["Lohan".to_string()]);
        assert_eq!(s.text(), "/tp Lohan");
        // Sem pedido pendente a resposta é ignorada.
        let mut s2 = ChatScreen::new("", 0);
        s2.set_text(&mut u, "abc", true);
        s2.on_autocomplete_response(&mut u, &mut chat, &cfg, 0, &["abcd".to_string()]);
        assert_eq!(s2.text(), "abc");
    }

    #[test]
    fn tab_cycles_when_the_prefix_already_matches() {
        let mut u = ui();
        let cfg = ChatCfg::default();
        let mut chat = NewChat::default();
        let mut s = ChatScreen::new("", 0);
        s.set_text(&mut u, "/msg ", true);
        // Uma palavra vazia depois do espaço: o pedido sai com o texto até o cursor.
        let fx = s.key(&mut u, &mut chat, &cfg, 0, KeyCode::Tab, None, false, false);
        assert_eq!(fx, [ChatEffect::TabComplete("/msg ".into())]);
        s.on_autocomplete_response(&mut u, &mut chat, &cfg, 0, &["Ana".to_string(), "Bia".to_string()]);
        // Sem prefixo comum: entra no ciclo, escreve o primeiro e lista as opções no chat.
        assert_eq!(s.text(), "/msg Ana");
        assert_eq!(chat.drawn[0].line.plain(), "Ana, Bia");
        assert_eq!(chat.drawn[0].id, 1);
        s.key(&mut u, &mut chat, &cfg, 1, KeyCode::Tab, None, false, false);
        assert_eq!(s.text(), "/msg Bia");
        s.key(&mut u, &mut chat, &cfg, 2, KeyCode::Tab, None, false, false);
        assert_eq!(s.text(), "/msg Ana");
        // A lista de opções foi trocada, não duplicada.
        assert_eq!(chat.drawn.iter().filter(|d| d.id == 1).count(), 1);
    }

    #[test]
    fn common_prefix_matches_apache_commons() {
        let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(common_prefix(&v(&["Lohan", "Lohana"])), "Lohan");
        assert_eq!(common_prefix(&v(&["abc", "xyz"])), "");
        assert_eq!(common_prefix(&v(&["solo"])), "solo");
        assert_eq!(common_prefix(&v(&["abc", ""])), "");
        assert_eq!(common_prefix(&[]), "");
    }

    #[test]
    fn click_events_follow_gui_screen_rules() {
        let mut u = ui();
        let cfg = ChatCfg::default();
        let mk = |action: &str, value: &str| {
            comp(&format!(r#"{{"text":"x","insertion":"INS","clickEvent":{{"action":"{action}","value":"{value}"}}}}"#)).spans().remove(0)
        };
        // open_url com confirmação.
        let mut s = ChatScreen::new("", 0);
        let fx = s.handle_component_click(&mut u, Some(&mk("open_url", "https://example.com/a")), &cfg, false).unwrap();
        assert_eq!(fx, [ChatEffect::ConfirmLink("https://example.com/a".into())]);
        // Sem a confirmação abre direto.
        let no_prompt = ChatCfg { links_prompt: false, ..cfg };
        let fx = s.handle_component_click(&mut u, Some(&mk("open_url", "HTTP://example.com")), &no_prompt, false).unwrap();
        assert_eq!(fx, [ChatEffect::OpenLink("HTTP://example.com".into())]);
        // chatLinks desligado: o clique não é tratado.
        let no_links = ChatCfg { links: false, ..cfg };
        assert!(s.handle_component_click(&mut u, Some(&mk("open_url", "https://example.com")), &no_links, false).is_none());
        // Protocolo que não é http/https, sem protocolo e URL malformada: tratado (true), sem efeito.
        for bad in ["ftp://example.com", "example.com", "http://exa mple.com", "javascript:alert(1)"] {
            assert_eq!(s.handle_component_click(&mut u, Some(&mk("open_url", bad)), &cfg, false), Some(vec![]), "{bad}");
        }
        // run_command manda sem entrar no histórico.
        let fx = s.handle_component_click(&mut u, Some(&mk("run_command", "/home casa")), &cfg, false).unwrap();
        assert_eq!(fx, [ChatEffect::Send("/home casa".into(), false)]);
        // suggest_command troca o texto do campo.
        s.set_text(&mut u, "antes", true);
        s.handle_component_click(&mut u, Some(&mk("suggest_command", "/tp ")), &cfg, false).unwrap();
        assert_eq!(s.text(), "/tp ");
        // Shift + clique escreve a insertion e deixa o clique seguir.
        assert!(s.handle_component_click(&mut u, Some(&mk("run_command", "/x")), &cfg, true).is_none());
        assert_eq!(s.text(), "/tp INS");
        // Sem trecho ou sem evento: não trata.
        assert!(s.handle_component_click(&mut u, None, &cfg, false).is_none());
        assert!(s.handle_component_click(&mut u, Some(&comp("\"x\"").spans().remove(0)), &cfg, false).is_none());
    }

    #[test]
    fn uri_validation() {
        assert_eq!(uri_scheme("https://example.com/a?b=c#d").unwrap(), "https");
        assert_eq!(uri_scheme("mailto:a@b.c").unwrap(), "mailto");
        assert!(uri_scheme("example.com").is_err());
        assert!(uri_scheme("http:").is_err());
        assert!(uri_scheme("http://a b").is_err());
        assert!(uri_scheme("http://a/%zz").is_err());
        assert!(uri_scheme("http://a/#x#y").is_err());
        assert!(uri_scheme("http://a/{x}").is_err());
    }

    #[test]
    fn hover_show_text_splits_on_newlines() {
        let mut u = ui();
        let lang = Lang::default();
        let ctx = HoverCtx { advanced_tooltips: false, lang: &lang, inventory_key: "E".into() };
        let s = comp(r#"{"text":"x","hoverEvent":{"action":"show_text","value":{"text":"um\ndois","color":"red"}}}"#).spans().remove(0);
        let lines = hover_lines(&mut u, s.style.hover.as_ref().unwrap(), &ctx).unwrap();
        assert_eq!(lines, ["§cum", "dois§r"]);
    }

    #[test]
    fn hover_show_item_reads_snbt() {
        let mut u = ui();
        let lang = Lang::default();
        let ctx = HoverCtx { advanced_tooltips: false, lang: &lang, inventory_key: "E".into() };
        let ev = |v: &str| {
            let j = serde_json::json!({"text":"x","hoverEvent":{"action":"show_item","value":{"text":v}}});
            Component::from_json(&j.to_string()).spans().remove(0).style.hover.unwrap()
        };
        let lines = hover_lines(&mut u, &ev(r#"{id:"minecraft:diamond_sword",Count:1b,Damage:0s,tag:{display:{Name:"Excalibur",Lore:["lenda"]}}}"#), &ctx).unwrap();
        assert_eq!(lines, ["§oExcalibur", "§5§olenda"]);
        // Item que não existe, id numérico inexistente e texto que não é NBT: "Invalid Item!".
        for bad in [r#"{id:"minecraft:nao_existe",Count:1b}"#, "{id:9999s,Count:1b}", "isso nao e nbt"] {
            assert_eq!(hover_lines(&mut u, &ev(bad), &ctx).unwrap(), ["§cInvalid Item!"], "{bad}");
        }
        // Número no lugar do nome: Item.getByNameOrId aceita "276".
        assert!(item_by_name_or_id("276").is_some());
        assert_eq!(item_by_name_or_id("minecraft:stone"), Some(1));
        assert_eq!(item_by_name_or_id("stone"), Some(1));
        assert_eq!(item_by_name_or_id("outro:stone"), None);
    }

    #[test]
    fn hover_show_entity_needs_advanced_tooltips() {
        let mut u = ui();
        let lang = Lang::default();
        let ev = || {
            let j = serde_json::json!({"text":"x","hoverEvent":{"action":"show_entity","value":{"text":"{id:\"abc-123\",name:\"Porquinho\",type:\"Pig\"}"}}});
            Component::from_json(&j.to_string()).spans().remove(0).style.hover.unwrap()
        };
        let off = HoverCtx { advanced_tooltips: false, lang: &lang, inventory_key: "E".into() };
        assert!(hover_lines(&mut u, &ev(), &off).is_none());
        let on = HoverCtx { advanced_tooltips: true, ..off };
        assert_eq!(hover_lines(&mut u, &ev(), &on).unwrap(), ["Porquinho", "Type: Pig (90)", "abc-123"]);
    }

    #[test]
    fn hover_show_achievement() {
        let mut u = ui();
        let lang = Lang::default();
        let ctx = HoverCtx { advanced_tooltips: false, lang: &lang, inventory_key: "E".into() };
        let ev = |id: &str| {
            let j = serde_json::json!({"text":"x","hoverEvent":{"action":"show_achievement","value":{"text":id}}});
            Component::from_json(&j.to_string()).spans().remove(0).style.hover.unwrap()
        };
        let lines = hover_lines(&mut u, &ev("achievement.mineWood"), &ctx).unwrap();
        assert_eq!(lines[0], format!("§a{}§r", crate::lang::tr("achievement.mineWood")));
        assert_eq!(lines[1], format!("§o{}§r", crate::lang::tr("stats.tooltip.type.achievement")));
        // Conquista especial: roxo escuro.
        let lines = hover_lines(&mut u, &ev("achievement.flyPig"), &ctx).unwrap();
        assert!(lines[0].starts_with("§5"));
        // Estatística geral: sem cor e sem descrição.
        let lines = hover_lines(&mut u, &ev("stat.leaveGame"), &ctx).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1], format!("§o{}§r", crate::lang::tr("stats.tooltip.type.statistic")));
        assert_eq!(hover_lines(&mut u, &ev("achievement.naoExiste"), &ctx).unwrap(), ["§cInvalid statistic/achievement!"]);
    }

    #[test]
    fn strip_formatting_only_removes_valid_codes() {
        assert_eq!(strip_formatting("§aoi §Lx§zy§"), "oi x§zy§");
        assert_eq!(strip_formatting("§1§Ra"), "a");
    }
}
