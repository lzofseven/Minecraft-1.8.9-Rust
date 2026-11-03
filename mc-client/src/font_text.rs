//! Texto da GUI que o Java faz no `FontRenderer` e no `GuiUtilRenderComponents`: largura, corte,
//! quebra de linhas com códigos `§`, `splitText` de componentes de chat e o JSON de livro.
//! Tudo trabalha por caractere como o original (`char` do Java), com a largura dada por [`Metrics`].

use mc_render::ui::UiBatch;
use serde_json::Value;

/// Largura de um caractere (`FontRenderer.getCharWidth`): `-1` para o `§`.
pub trait Metrics {
    fn char_width(&mut self, c: char) -> i32;
}

impl Metrics for UiBatch {
    fn char_width(&mut self, c: char) -> i32 {
        if c == '§' {
            return -1;
        }
        let mut b = [0u8; 4];
        // O avanço dos glifos unicode é (fim - início) / 2 + 1 com divisão inteira no Java.
        self.text_width(c.encode_utf8(&mut b)) as i32
    }
}

/// `ChatAllowedCharacters.isAllowedCharacter`.
pub fn is_allowed_char(c: char) -> bool {
    c != '§' && c >= ' ' && c != '\u{7f}'
}

/// `EnumChatFormatting.getTextWithoutFormattingCodes`: tira `§` seguido de `0-9a-fk-or`.
pub fn strip_formatting(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < c.len() {
        if c[i] == '§' && i + 1 < c.len() && matches!(c[i + 1].to_ascii_lowercase(), '0'..='9' | 'a'..='f' | 'k'..='o' | 'r') {
            i += 2;
        } else {
            out.push(c[i]);
            i += 1;
        }
    }
    out
}

/// `FontRenderer.getStringWidth`.
pub fn string_width<M: Metrics>(m: &mut M, s: &str) -> i32 {
    let c: Vec<char> = s.chars().collect();
    let (mut total, mut j, mut bold) = (0, 0, false);
    while j < c.len() {
        let mut k = m.char_width(c[j]);
        if k < 0 && j < c.len() - 1 {
            j += 1;
            match c[j] {
                'l' | 'L' => bold = true,
                'r' | 'R' => bold = false,
                _ => {}
            }
            k = 0;
        }
        total += k;
        if bold && k > 0 {
            total += 1;
        }
        j += 1;
    }
    total
}

/// `FontRenderer.trimStringToWidth(texto, largura)`: o maior prefixo que cabe.
pub fn trim_to_width<M: Metrics>(m: &mut M, s: &str, max: i32) -> String {
    let mut out = String::new();
    let (mut width, mut after_section, mut bold) = (0, false, false);
    for c in s.chars() {
        if width >= max {
            break;
        }
        let w = m.char_width(c);
        if after_section {
            after_section = false;
            match c {
                'l' | 'L' => bold = true,
                'r' | 'R' => bold = false,
                _ => {}
            }
        } else if w < 0 {
            after_section = true;
        } else {
            width += w;
            if bold {
                width += 1;
            }
        }
        if width > max {
            break;
        }
        out.push(c);
    }
    out
}

fn is_format_color(c: char) -> bool {
    c.is_ascii_digit() || ('a'..='f').contains(&c) || ('A'..='F').contains(&c)
}

fn is_format_special(c: char) -> bool {
    ('k'..='o').contains(&c) || ('K'..='O').contains(&c) || c == 'r' || c == 'R'
}

/// `FontRenderer.getFormatFromString`: o que continua valendo no fim do texto.
pub fn format_from(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let mut out = String::new();
    for i in 0..c.len() {
        if c[i] == '§' && i + 1 < c.len() {
            let n = c[i + 1];
            if is_format_color(n) {
                out = format!("§{n}");
            } else if is_format_special(n) {
                out.push('§');
                out.push(n);
            }
        }
    }
    out
}

/// `FontRenderer.sizeStringToWidth`: quantos caracteres cabem, preferindo quebrar no último espaço.
fn size_to_width<M: Metrics>(m: &mut M, s: &[char], max: i32) -> usize {
    let n = s.len();
    let (mut j, mut k, mut last_space, mut bold) = (0, 0usize, None::<usize>, false);
    while k < n {
        let c = s[k];
        match c {
            '\n' => {
                // `--k` e depois `++k`: fica no próprio '\n', que passa a ser o ponto de quebra.
                last_space = Some(k);
                break;
            }
            '§' => {
                if k + 1 < n {
                    k += 1;
                    let c1 = s[k];
                    if c1 == 'l' || c1 == 'L' {
                        bold = true;
                    } else if c1 == 'r' || c1 == 'R' || is_format_color(c1) {
                        bold = false;
                    }
                }
            }
            _ => {
                if c == ' ' {
                    last_space = Some(k);
                }
                j += m.char_width(c);
                if bold {
                    j += 1;
                }
            }
        }
        if j > max {
            break;
        }
        k += 1;
    }
    match last_space {
        Some(l) if k != n && l < k => l,
        _ => k,
    }
}

/// `FontRenderer.wrapFormattedStringToWidth`, sem a recursão.
fn wrap_formatted<M: Metrics>(m: &mut M, s: &str, max: i32) -> String {
    let mut out = String::new();
    let mut rest: Vec<char> = s.chars().collect();
    loop {
        let i = size_to_width(m, &rest, max);
        if rest.len() <= i {
            out.extend(rest.iter());
            return out;
        }
        let first: String = rest[..i].iter().collect();
        let skip = rest[i] == ' ' || rest[i] == '\n';
        let next = format!("{}{}", format_from(&first), rest[i + skip as usize..].iter().collect::<String>());
        out.push_str(&first);
        out.push('\n');
        rest = next.chars().collect();
    }
}

/// `FontRenderer.listFormattedStringToWidth` (o `String.split("\n")` do Java descarta as vazias do fim).
pub fn list_formatted<M: Metrics>(m: &mut M, s: &str, max: i32) -> Vec<String> {
    let wrapped = wrap_formatted(m, s, max);
    if !wrapped.contains('\n') {
        return vec![wrapped];
    }
    let mut parts: Vec<String> = wrapped.split('\n').map(str::to_string).collect();
    while parts.last().is_some_and(String::is_empty) {
        parts.pop();
    }
    parts
}

/// `FontRenderer.splitStringWidth`: altura em pixels do texto quebrado (9 por linha).
pub fn split_string_height<M: Metrics>(m: &mut M, s: &str, max: i32) -> i32 {
    9 * list_formatted(m, s, max).len() as i32
}

/// `FontRenderer.drawSplitString` (sem sombra): `trimStringNewline`, quebra e uma linha a cada 9 px.
pub fn draw_split_string(ui: &mut UiBatch, s: &str, x: i32, y: i32, max: i32, argb: u32) {
    let s = s.trim_end_matches('\n');
    let mut y = y;
    for line in list_formatted(ui, s, max) {
        ui.text(x as f32, y as f32, &line, argb, false);
        y += 9;
    }
}

// ---------------------------------------------------------------------------------------------
// Componentes de chat de livro
// ---------------------------------------------------------------------------------------------

/// `ClickEvent` (só as ações permitidas no chat).
#[derive(Clone, Debug, PartialEq)]
pub struct Click {
    pub action: String,
    pub value: String,
}

/// Um componente achatado como o `Iterable<IChatComponent>` entrega: texto próprio, o código de
/// formatação do estilo efetivo (`ChatStyle.getFormattingCode`) e o clique efetivo.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub text: String,
    pub fmt: String,
    pub click: Option<Click>,
}

/// Pedaço de uma linha: texto com o código de formatação na frente e o clique do estilo.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub text: String,
    pub click: Option<Click>,
}

#[derive(Clone, Default)]
struct Style {
    color: Option<char>,
    bold: bool,
    italic: bool,
    underlined: bool,
    obfuscated: bool,
    strikethrough: bool,
    click: Option<Click>,
}

impl Style {
    fn code(&self) -> String {
        let mut s = String::new();
        if let Some(c) = self.color {
            s.push('§');
            s.push(c);
        }
        for (on, k) in [(self.bold, 'l'), (self.italic, 'o'), (self.underlined, 'n'), (self.obfuscated, 'k'), (self.strikethrough, 'm')] {
            if on {
                s.push('§');
                s.push(k);
            }
        }
        s
    }

    /// O estilo `own` do componente sobre o do pai.
    fn over(&self, own: &Map) -> Style {
        let mut st = self.clone();
        if let Some(Value::String(c)) = own.get("color") {
            if let Some(code) = color_code(c) {
                st.color = Some(code);
            }
        }
        for (k, field) in [
            ("bold", &mut st.bold),
            ("italic", &mut st.italic),
            ("underlined", &mut st.underlined),
            ("strikethrough", &mut st.strikethrough),
            ("obfuscated", &mut st.obfuscated),
        ] {
            if let Some(v) = own.get(k) {
                *field = v.as_bool().unwrap_or(false);
            }
        }
        if let Some(Value::Object(ev)) = own.get("clickEvent") {
            let action = ev.get("action").and_then(Value::as_str);
            let value = ev.get("value").and_then(Value::as_str);
            // `ClickEvent.Action.shouldAllowInChat`: open_file e twitch_user_info ficam de fora.
            if let (Some(a), Some(v)) = (action, value) {
                if matches!(a, "open_url" | "run_command" | "suggest_command" | "change_page") {
                    st.click = Some(Click { action: a.to_string(), value: v.to_string() });
                }
            }
        }
        st
    }
}

type Map = serde_json::Map<String, Value>;

/// `EnumChatFormatting.getValueByName` das cores.
fn color_code(name: &str) -> Option<char> {
    Some(match name.to_ascii_lowercase().as_str() {
        "black" => '0',
        "dark_blue" => '1',
        "dark_green" => '2',
        "dark_aqua" => '3',
        "dark_red" => '4',
        "dark_purple" => '5',
        "gold" => '6',
        "gray" => '7',
        "dark_gray" => '8',
        "blue" => '9',
        "green" => 'a',
        "aqua" => 'b',
        "red" => 'c',
        "light_purple" => 'd',
        "yellow" => 'e',
        "white" => 'f',
        "obfuscated" => 'k',
        "bold" => 'l',
        "strikethrough" => 'm',
        "underline" => 'n',
        "italic" => 'o',
        "reset" => 'r',
        _ => return None,
    })
}

struct Comp {
    text: String,
    own: Map,
    siblings: Vec<Comp>,
}

fn primitive_text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// `IChatComponent.Serializer.deserialize`; `None` onde o Java lança `JsonParseException` ou devolve null.
fn component(v: &Value) -> Option<Comp> {
    match v {
        Value::Array(a) => {
            let mut result: Option<Comp> = None;
            for e in a {
                let c = component(e)?;
                match &mut result {
                    None => result = Some(c),
                    Some(r) => r.siblings.push(c),
                }
            }
            result
        }
        Value::Object(o) => {
            let text = if let Some(t) = o.get("text") {
                primitive_text(t)?
            } else if let Some(k) = o.get("translate") {
                let key = primitive_text(k)?;
                let with: Vec<String> = match o.get("with") {
                    Some(Value::Array(w)) => w.iter().map(|x| component(x).map(|c| plain_text(&c)).unwrap_or_default()).collect(),
                    _ => Vec::new(),
                };
                let args: Vec<&str> = with.iter().map(String::as_str).collect();
                crate::lang::trf(&key, &args)
            } else {
                return None;
            };
            let mut c = Comp { text, own: o.clone(), siblings: Vec::new() };
            if let Some(extra) = o.get("extra") {
                let Value::Array(extra) = extra else { return None };
                if extra.is_empty() {
                    return None;
                }
                for e in extra {
                    c.siblings.push(component(e)?);
                }
            }
            Some(c)
        }
        other => primitive_text(other).map(|text| Comp { text, own: Map::new(), siblings: Vec::new() }),
    }
}

/// `getUnformattedText` (texto próprio e dos irmãos).
fn plain_text(c: &Comp) -> String {
    let mut s = c.text.clone();
    for sib in &c.siblings {
        s.push_str(&plain_text(sib));
    }
    s
}

fn flatten(c: &Comp, parent: &Style, out: &mut Vec<Item>) {
    let st = parent.over(&c.own);
    out.push(Item { text: c.text.clone(), fmt: st.code(), click: st.click.clone() });
    for s in &c.siblings {
        flatten(s, &st, out);
    }
}

/// `Gson.fromJson(texto, ...)` do modo leniente: JSON, ou uma palavra solta sem aspas.
fn lenient_json(s: &str) -> Option<Value> {
    if let Ok(v) = serde_json::from_str::<Value>(s) {
        return Some(v);
    }
    let t = s.trim_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{c}'));
    if t.is_empty() {
        return None;
    }
    if t.len() >= 2 && t.starts_with('\'') && t.ends_with('\'') && !t[1..t.len() - 1].contains(['\'', '\\']) {
        return Some(Value::String(t[1..t.len() - 1].to_string()));
    }
    // Literal sem aspas: termina em qualquer destes (JsonReader.nextNonWhitespace/nextUnquotedValue).
    if t.contains(['/', '\\', ';', '#', '=', '{', '}', '[', ']', ':', ',', ' ', '\t', '\u{c}', '\r', '\n', '"', '\'']) {
        return None;
    }
    Some(Value::String(t.to_string()))
}

/// `jsonToComponent(json)` achatado. `None`: `JsonParseException` ou componente nulo.
pub fn parse_page(json: &str) -> Option<Vec<Item>> {
    let v = lenient_json(json)?;
    if v.is_null() {
        return None;
    }
    let c = component(&v)?;
    let mut out = Vec::new();
    flatten(&c, &Style::default(), &mut out);
    Some(out)
}

fn count(s: &str) -> usize {
    s.chars().count()
}

fn take(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn skip(s: &str, n: usize) -> String {
    s.chars().skip(n).collect()
}

/// `GuiUtilRenderComponents.splitText(componente, largura, fonte, manter_espaço, cores)` com as cores
/// ligadas (é como livro e placa chamam). Cada linha é a lista de pedaços de texto.
pub fn split_text<M: Metrics>(m: &mut M, items: Vec<Item>, max: i32, keep_space: bool) -> Vec<Vec<Piece>> {
    let mut width = 0;
    let mut line: Vec<Piece> = Vec::new();
    let mut lines: Vec<Vec<Piece>> = Vec::new();
    let mut list = items;
    let mut j = 0;
    while j < list.len() {
        let item = list[j].clone();
        let mut s = item.text.clone();
        let mut flag = false;
        if let Some(k) = s.find('\n') {
            let rest = s[k + 1..].to_string();
            s.truncate(k + 1);
            list.insert(j + 1, Item { text: rest, fmt: item.fmt.clone(), click: item.click.clone() });
            flag = true;
        }
        let s4 = format!("{}{}", item.fmt, s);
        let s5 = s4.strip_suffix('\n').unwrap_or(&s4).to_string();
        let mut w1 = string_width(m, &s5);
        let mut cur = s5;
        if width + w1 > max {
            let mut s2 = trim_to_width(m, &s4, max - width);
            let tail = if count(&s2) < count(&s4) { Some(skip(&s4, count(&s2))) } else { None };
            if let Some(mut s3) = tail.filter(|t| !t.is_empty()) {
                let space = s2.chars().collect::<Vec<_>>().iter().rposition(|c| *c == ' ');
                match space {
                    Some(l) if string_width(m, &take(&s4, l)) > 0 => {
                        s2 = take(&s4, l);
                        s3 = skip(&s4, if keep_space { l + 1 } else { l });
                    }
                    _ if width > 0 && !s4.contains(' ') => {
                        s2 = String::new();
                        s3 = s4.clone();
                    }
                    _ => {}
                }
                s3 = format!("{}{}", format_from(&s2), s3);
                list.insert(j + 1, Item { text: s3, fmt: item.fmt.clone(), click: item.click.clone() });
            }
            w1 = string_width(m, &s2);
            cur = s2;
            flag = true;
        }
        if width + w1 <= max {
            width += w1;
            line.push(Piece { text: cur, click: item.click.clone() });
        } else {
            flag = true;
        }
        if flag {
            lines.push(std::mem::take(&mut line));
            width = 0;
        }
        j += 1;
    }
    lines.push(line);
    lines
}

/// Texto da linha (`getUnformattedText` do componente da linha: os pedaços um atrás do outro).
pub fn line_text(line: &[Piece]) -> String {
    line.iter().map(|p| p.text.as_str()).collect()
}

/// `GuiScreenBook.func_175385_b`: o pedaço sob o ponto `(rx, ry)`, relativo ao canto do texto.
pub fn piece_at<'a, M: Metrics>(m: &mut M, lines: &'a [Vec<Piece>], rx: i32, ry: i32) -> Option<&'a Piece> {
    if rx < 0 || ry < 0 {
        return None;
    }
    let n = (128 / 9).min(lines.len() as i32);
    if rx <= 116 && ry < 9 * n + n {
        let idx = (ry / 9) as usize;
        let mut w = 0;
        for p in lines.get(idx)? {
            w += string_width(m, &p.text);
            if w > rx {
                return Some(p);
            }
        }
    }
    None
}

/// `IChatComponent.Serializer.componentToJson(new ChatComponentText(texto))`: o texto como literal
/// JSON do Gson (que também escapa `< > & = '`).
pub fn component_to_json(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\u{c}' => out.push_str("\\f"),
            '<' => out.push_str("\\u003c"),
            '>' => out.push_str("\\u003e"),
            '&' => out.push_str("\\u0026"),
            '=' => out.push_str("\\u003d"),
            '\'' => out.push_str("\\u0027"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c if c < ' ' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Métrica de mentira para os testes: todo caractere mede 6, o espaço 4 e o `§` é -1.
#[cfg(test)]
pub(crate) mod testing {
    use super::Metrics;

    pub struct Mock;

    impl Metrics for Mock {
        fn char_width(&mut self, c: char) -> i32 {
            match c {
                '§' => -1,
                ' ' => 4,
                _ => 6,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::Mock;
    use super::*;

    #[test]
    fn width_skips_codes_and_counts_bold() {
        assert_eq!(string_width(&mut Mock, "abc"), 18);
        assert_eq!(string_width(&mut Mock, "§cab"), 12);
        assert_eq!(string_width(&mut Mock, "§lab"), 14);
        assert_eq!(string_width(&mut Mock, "§lab§rc"), 14 + 6);
        // `§` no fim conta -1, como no Java.
        assert_eq!(string_width(&mut Mock, "a§"), 5);
    }

    #[test]
    fn trim_cuts_at_width() {
        assert_eq!(trim_to_width(&mut Mock, "abcdef", 18), "abc");
        assert_eq!(trim_to_width(&mut Mock, "abcdef", 17), "ab");
        assert_eq!(trim_to_width(&mut Mock, "ab", 100), "ab");
        // Os códigos não gastam largura.
        assert_eq!(trim_to_width(&mut Mock, "§cabcd", 12), "§cab");
    }

    #[test]
    fn format_from_keeps_last_color_and_styles() {
        assert_eq!(format_from("§aab§lcd"), "§a§l");
        assert_eq!(format_from("§aab§cdd"), "§c");
        assert_eq!(format_from("sem"), "");
        assert_eq!(format_from("§"), "");
    }

    #[test]
    fn wrap_prefers_spaces_and_carries_format() {
        // Largura 30 = 5 letras.
        assert_eq!(list_formatted(&mut Mock, "abcde fghij", 30), vec!["abcde", "fghij"]);
        assert_eq!(list_formatted(&mut Mock, "abc defghij", 30), vec!["abc", "defgh", "ij"]);
        assert_eq!(list_formatted(&mut Mock, "§cabc defghij", 30), vec!["§cabc", "§cdefgh", "§cij"]);
        assert_eq!(list_formatted(&mut Mock, "a\nb", 30), vec!["a", "b"]);
        assert_eq!(list_formatted(&mut Mock, "", 30), vec![""]);
        assert_eq!(list_formatted(&mut Mock, "a\n\n", 30), vec!["a"]);
        assert_eq!(list_formatted(&mut Mock, "abcdefghij", 30), vec!["abcde", "fghij"]);
    }

    #[test]
    fn page_json_inherits_style_and_click() {
        let items = parse_page(r##"{"text":"","extra":[{"text":"ir ","color":"red","clickEvent":{"action":"change_page","value":"3"}},"fim"],"bold":true}"##).unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0], Item { text: String::new(), fmt: "§l".into(), click: None });
        assert_eq!(items[1].fmt, "§c§l");
        assert_eq!(items[1].click, Some(Click { action: "change_page".into(), value: "3".into() }));
        // O irmão seguinte herda negrito do pai, mas não o clique do vizinho.
        assert_eq!(items[2], Item { text: "fim".into(), fmt: "§l".into(), click: None });
    }

    #[test]
    fn page_json_variants() {
        // Texto simples, número, array (os demais viram irmãos do primeiro).
        assert_eq!(parse_page("\"oi\"").unwrap()[0].text, "oi");
        assert_eq!(parse_page("palavra").unwrap()[0].text, "palavra");
        assert_eq!(parse_page("123").unwrap()[0].text, "123");
        let a = parse_page(r##"[{"text":"a","color":"gold"},"b"]"##).unwrap();
        assert_eq!((a[0].fmt.as_str(), a[1].fmt.as_str(), a[1].text.as_str()), ("§6", "§6", "b"));
        // Onde o Gson falha ou devolve null, o livro cai no texto cru.
        assert!(parse_page("duas palavras").is_none());
        assert!(parse_page("").is_none());
        assert!(parse_page("null").is_none());
        assert!(parse_page("[]").is_none());
        assert!(parse_page(r##"{"extra":[]}"##).is_none());
        assert!(parse_page(r##"{"nada":1}"##).is_none());
        // `open_file` não é permitido no chat.
        let f = parse_page(r##"{"text":"x","clickEvent":{"action":"open_file","value":"/a"}}"##).unwrap();
        assert!(f[0].click.is_none());
    }

    fn texts(lines: &[Vec<Piece>]) -> Vec<String> {
        lines.iter().map(|l| line_text(l)).collect()
    }

    #[test]
    fn split_text_wraps_pieces_and_keeps_clicks() {
        let click = Some(Click { action: "run_command".into(), value: "/x".into() });
        let items = vec![
            Item { text: "abc ".into(), fmt: String::new(), click: None },
            Item { text: "defghi jkl".into(), fmt: "§c".into(), click: click.clone() },
        ];
        // 5 letras por linha (30). Como no Java, a primeira linha aproveita o resto com parte da palavra.
        let lines = split_text(&mut Mock, items, 30, true);
        assert_eq!(texts(&lines), vec!["abc §cd", "§c§cefghi", "§c§c jkl"]);
        assert_eq!(lines[1][0].click, click);
        // O texto todo continua lá, na ordem.
        assert_eq!(strip_formatting(&texts(&lines).concat()), "abc defghi jkl");
    }

    #[test]
    fn split_text_newline_ends_line() {
        let items = vec![Item { text: "ab\ncd".into(), fmt: String::new(), click: None }];
        let lines = split_text(&mut Mock, items, 100, true);
        assert_eq!(texts(&lines), vec!["ab", "cd"]);
        // Um item que cabe inteiro fica numa linha só.
        let lines = split_text(&mut Mock, vec![Item { text: "abc".into(), fmt: String::new(), click: None }], 100, false);
        assert_eq!(texts(&lines), vec!["abc"]);
    }

    #[test]
    fn piece_hit_test_uses_cumulative_width() {
        let click = Some(Click { action: "change_page".into(), value: "2".into() });
        let lines = vec![vec![
            Piece { text: "abc".into(), click: None },
            Piece { text: "§cde".into(), click: click.clone() },
        ]];
        // abc = 18 px, de = 12 px.
        assert!(piece_at(&mut Mock, &lines, 17, 0).unwrap().click.is_none());
        assert_eq!(piece_at(&mut Mock, &lines, 18, 8).unwrap().click, click);
        assert!(piece_at(&mut Mock, &lines, 30, 0).is_none());
        assert!(piece_at(&mut Mock, &lines, 0, 20).is_none());
        assert!(piece_at(&mut Mock, &lines, -1, 0).is_none());
    }

    #[test]
    fn strip_and_allowed() {
        assert_eq!(strip_formatting("§cola§lx§"), "olax§");
        assert_eq!(strip_formatting("a§zb"), "a§zb");
        assert!(is_allowed_char('a') && is_allowed_char('é') && !is_allowed_char('§') && !is_allowed_char('\n') && !is_allowed_char('\u{7f}'));
    }

    #[test]
    fn gson_escapes() {
        assert_eq!(component_to_json("a\"b\\c\nd"), "\"a\\\"b\\\\c\\nd\"");
        assert_eq!(component_to_json("<x>&='"), "\"\\u003cx\\u003e\\u0026\\u003d\\u0027\"");
        assert_eq!(component_to_json("\u{1}é"), "\"\\u0001é\"");
    }
}
