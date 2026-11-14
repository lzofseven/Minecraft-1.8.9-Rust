//! Componentes de chat (`IChatComponent`, `ChatStyle`, `ClickEvent`, `HoverEvent`) lidos do JSON do
//! servidor, como `IChatComponent.Serializer` da 1.8.9.
//!
//! O estilo é herdado do pai (cor, negrito etc., e também click/hover/insertion): o Java guarda um
//! ponteiro `parentStyle` em cada `ChatStyle`; aqui o estilo de cada filho é resolvido contra o do pai
//! ao percorrer a árvore (`Component::spans`, o `iterator()` do Java com `createDeepCopy`).

use serde_json::{Map, Value};

/// `ClickEvent.Action` permitidas no chat (`shouldAllowInChat`); as outras o servidor não consegue enviar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClickAction {
    OpenUrl,
    RunCommand,
    SuggestCommand,
    ChangePage,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClickEvent {
    pub action: ClickAction,
    pub value: String,
}

/// `HoverEvent.Action`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoverAction {
    ShowText,
    ShowAchievement,
    ShowItem,
    ShowEntity,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HoverEvent {
    pub action: HoverAction,
    pub value: Box<Component>,
}

/// `ChatStyle`: cada campo `None` herda do pai.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Style {
    /// Código de `EnumChatFormatting` (`'0'..='f'`; o JSON aceita qualquer nome do enum).
    pub color: Option<char>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underlined: Option<bool>,
    pub strikethrough: Option<bool>,
    pub obfuscated: Option<bool>,
    pub click: Option<ClickEvent>,
    pub hover: Option<HoverEvent>,
    pub insertion: Option<String>,
}

/// `EnumChatFormatting.getValueByName` pelo nome do JSON (`EnumTypeAdapterFactory`: nome em minúsculas).
fn formatting_by_name(name: &str) -> Option<char> {
    Some(match name {
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

impl Style {
    /// `ChatStyle.isEmpty` (o `insertion` não conta).
    pub fn is_empty(&self) -> bool {
        self.bold.is_none()
            && self.italic.is_none()
            && self.strikethrough.is_none()
            && self.underlined.is_none()
            && self.obfuscated.is_none()
            && self.color.is_none()
            && self.click.is_none()
            && self.hover.is_none()
    }

    /// O estilo efetivo deste sobre o do pai (o que `getBold()`, `getColor()` etc. devolvem).
    pub fn resolve(&self, parent: &Style) -> Style {
        Style {
            color: self.color.or(parent.color),
            bold: self.bold.or(parent.bold),
            italic: self.italic.or(parent.italic),
            underlined: self.underlined.or(parent.underlined),
            strikethrough: self.strikethrough.or(parent.strikethrough),
            obfuscated: self.obfuscated.or(parent.obfuscated),
            click: self.click.clone().or_else(|| parent.click.clone()),
            hover: self.hover.clone().or_else(|| parent.hover.clone()),
            insertion: self.insertion.clone().or_else(|| parent.insertion.clone()),
        }
    }

    /// `ChatStyle.getFormattingCode` de um estilo já resolvido: cor, negrito, itálico, sublinhado,
    /// ofuscado e tachado, nessa ordem.
    pub fn code(&self) -> String {
        let mut s = String::new();
        if let Some(c) = self.color {
            s.push('§');
            s.push(c);
        }
        for (on, k) in [(self.bold, 'l'), (self.italic, 'o'), (self.underlined, 'n'), (self.obfuscated, 'k'), (self.strikethrough, 'm')] {
            if on == Some(true) {
                s.push('§');
                s.push(k);
            }
        }
        s
    }

    /// `ChatStyle.Serializer.deserialize`.
    fn from_object(o: &Map<String, Value>) -> Style {
        let flag = |k: &str| o.get(k).map(as_bool);
        let mut st = Style {
            bold: flag("bold"),
            italic: flag("italic"),
            underlined: flag("underlined"),
            strikethrough: flag("strikethrough"),
            obfuscated: flag("obfuscated"),
            ..Style::default()
        };
        if let Some(v) = o.get("color") {
            st.color = v.as_str().and_then(formatting_by_name);
        }
        if let Some(v) = o.get("insertion") {
            st.insertion = primitive_string(v);
        }
        if let Some(Value::Object(c)) = o.get("clickEvent") {
            let action = c.get("action").and_then(Value::as_str).and_then(|a| match a {
                "open_url" => Some(ClickAction::OpenUrl),
                "run_command" => Some(ClickAction::RunCommand),
                "suggest_command" => Some(ClickAction::SuggestCommand),
                "change_page" => Some(ClickAction::ChangePage),
                // open_file e twitch_user_info existem, mas `shouldAllowInChat` é falso.
                _ => None,
            });
            let value = c.get("value").and_then(primitive_string);
            if let (Some(action), Some(value)) = (action, value) {
                st.click = Some(ClickEvent { action, value });
            }
        }
        if let Some(Value::Object(h)) = o.get("hoverEvent") {
            let action = h.get("action").and_then(Value::as_str).and_then(|a| match a {
                "show_text" => Some(HoverAction::ShowText),
                "show_achievement" => Some(HoverAction::ShowAchievement),
                "show_item" => Some(HoverAction::ShowItem),
                "show_entity" => Some(HoverAction::ShowEntity),
                _ => None,
            });
            let value = h.get("value").and_then(|v| Component::from_value(v).ok());
            if let (Some(action), Some(value)) = (action, value) {
                st.hover = Some(HoverEvent { action, value: Box::new(value) });
            }
        }
        st
    }
}

/// `JsonPrimitive.getAsString` (texto, número ou booleano).
fn primitive_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// `JsonElement.getAsBoolean`: booleano, ou o texto `true` (sem distinguir maiúsculas).
fn as_bool(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::String(s) => s.eq_ignore_ascii_case("true"),
        _ => false,
    }
}

/// Argumento de `ChatComponentTranslation`: texto puro ou outro componente.
#[derive(Clone, Debug, PartialEq)]
pub enum Arg {
    Str(String),
    Comp(Component),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// `ChatComponentText` (também `score`, com o valor do JSON, e `selector`, com o seletor cru).
    Text(String),
    /// `ChatComponentTranslation`.
    Translate { key: String, args: Vec<Arg> },
}

/// Trecho de texto com o estilo já resolvido (um item do `iterator()` do Java).
#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub text: String,
    pub style: Style,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Component {
    pub kind: Kind,
    pub style: Style,
    pub siblings: Vec<Component>,
}

/// Pedaço de um `ChatComponentTranslation` já separado pelo formato.
enum Piece {
    Lit(String),
    Arg(usize),
}

/// `ChatComponentTranslation.initializeFromFormat`: separa o padrão em texto e argumentos
/// (`%s`, `%1$s` e `%%`). Formato não suportado devolve `None`.
fn translation_pieces(pattern: &str, nargs: usize) -> Option<Vec<Piece>> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut pieces = Vec::new();
    let (mut next_arg, mut lit_start, mut i) = (0usize, 0usize, 0usize);
    let flush = |pieces: &mut Vec<Piece>, from: usize, to: usize| {
        if to > from {
            pieces.push(Piece::Lit(chars[from..to].iter().collect()));
        }
    };
    while i < chars.len() {
        if chars[i] != '%' {
            i += 1;
            continue;
        }
        // %(?:(\d+)\$)?([A-Za-z%]|$)
        let mut j = i + 1;
        let digits_end = (j..chars.len()).find(|k| !chars[*k].is_ascii_digit()).unwrap_or(chars.len());
        let mut index = None;
        if digits_end > j && chars.get(digits_end) == Some(&'$') {
            index = chars[j..digits_end].iter().collect::<String>().parse::<i64>().ok();
            j = digits_end + 1;
        }
        let conv = chars.get(j).copied();
        let matched = match conv {
            Some(c) if c.is_ascii_alphabetic() || c == '%' => Some(j + 1),
            None => Some(j),
            _ => None,
        };
        let Some(end) = matched else {
            i += 1;
            continue;
        };
        flush(&mut pieces, lit_start, i);
        let whole: String = chars[i..end].iter().collect();
        if conv == Some('%') && whole == "%%" {
            pieces.push(Piece::Lit("%".to_string()));
        } else {
            if conv != Some('s') {
                return None;
            }
            let idx = match index {
                Some(n) => n - 1,
                None => {
                    next_arg += 1;
                    next_arg as i64 - 1
                }
            };
            if idx >= 0 && (idx as usize) < nargs {
                pieces.push(Piece::Arg(idx as usize));
            }
        }
        lit_start = end;
        i = end.max(i + 1);
    }
    flush(&mut pieces, lit_start, chars.len());
    Some(pieces)
}

impl Component {
    /// `new ChatComponentText(s)`.
    pub fn text(s: impl Into<String>) -> Component {
        Component { kind: Kind::Text(s.into()), style: Style::default(), siblings: Vec::new() }
    }

    /// `IChatComponent.Serializer.jsonToComponent`; JSON inválido vira o texto cru.
    pub fn from_json(json: &str) -> Component {
        match serde_json::from_str::<Value>(json) {
            // jsonToComponent("null") devolve null: o servidor manda assim as linhas vazias de placa.
            Ok(Value::Null) => Component::text(""),
            Ok(v) => Component::from_value(&v).unwrap_or_else(|_| Component::text(json)),
            Err(_) => Component::text(json),
        }
    }

    /// `IChatComponent.Serializer.deserialize`.
    pub fn from_value(v: &Value) -> Result<Component, String> {
        match v {
            Value::String(_) | Value::Number(_) | Value::Bool(_) => Ok(Component::text(primitive_string(v).unwrap_or_default())),
            Value::Array(items) => {
                // O primeiro vira a base e os demais são irmãos dele (herdam o estilo do primeiro).
                let mut base: Option<Component> = None;
                for item in items {
                    let c = Component::from_value(item)?;
                    match &mut base {
                        None => base = Some(c),
                        Some(b) => b.siblings.push(c),
                    }
                }
                base.ok_or_else(|| "lista de componentes vazia".to_string())
            }
            Value::Object(o) => {
                let mut c = if let Some(t) = o.get("text") {
                    Component::text(primitive_string(t).unwrap_or_default())
                } else if let Some(k) = o.get("translate") {
                    let key = primitive_string(k).unwrap_or_default();
                    let mut args = Vec::new();
                    if let Some(w) = o.get("with") {
                        let Value::Array(with) = w else { return Err("\"with\" não é uma lista".into()) };
                        for a in with {
                            let comp = Component::from_value(a)?;
                            // Texto puro sem estilo e sem irmãos vira String (o estilo `insertion` se perde).
                            args.push(match comp {
                                Component { kind: Kind::Text(t), style, siblings } if style.is_empty() && siblings.is_empty() => Arg::Str(t),
                                other => Arg::Comp(other),
                            });
                        }
                    }
                    Component { kind: Kind::Translate { key, args }, style: Style::default(), siblings: Vec::new() }
                } else if let Some(Value::Object(sc)) = o.get("score") {
                    let (Some(_), Some(_)) = (sc.get("name").and_then(Value::as_str), sc.get("objective").and_then(Value::as_str)) else {
                        return Err("A score component needs a least a name and an objective".into());
                    };
                    Component::text(sc.get("value").and_then(primitive_string).unwrap_or_default())
                } else if let Some(sel) = o.get("selector") {
                    Component::text(primitive_string(sel).unwrap_or_default())
                } else {
                    return Err("Don't know how to turn the object into a Component".into());
                };
                if let Some(extra) = o.get("extra") {
                    let Value::Array(items) = extra else { return Err("\"extra\" não é uma lista".into()) };
                    if items.is_empty() {
                        return Err("Unexpected empty array of components".into());
                    }
                    for item in items {
                        c.siblings.push(Component::from_value(item)?);
                    }
                }
                c.style = Style::from_object(o);
                Ok(c)
            }
            Value::Null => Err("Don't know how to turn null into a Component".into()),
        }
    }

    /// O `iterator()` do Java: os trechos de texto com o estilo resolvido, na ordem em que aparecem.
    pub fn spans(&self) -> Vec<Span> {
        let mut out = Vec::new();
        self.collect(&Style::default(), &mut out);
        out
    }

    fn collect(&self, parent: &Style, out: &mut Vec<Span>) {
        let own = self.style.resolve(parent);
        match &self.kind {
            Kind::Text(t) => out.push(Span { text: t.clone(), style: own.clone() }),
            Kind::Translate { key, args } => {
                let pattern = crate::lang::raw(key).unwrap_or_else(|| key.clone());
                match translation_pieces(&pattern, args.len()) {
                    Some(pieces) => {
                        for p in pieces {
                            match p {
                                Piece::Lit(t) => out.push(Span { text: t, style: own.clone() }),
                                Piece::Arg(i) => match &args[i] {
                                    Arg::Str(s) => out.push(Span { text: s.clone(), style: own.clone() }),
                                    Arg::Comp(c) => c.collect(&own, out),
                                },
                            }
                        }
                    }
                    // Formato que o Java recusa (`ChatComponentTranslationFormatException`): mostra o padrão.
                    None => out.push(Span { text: pattern, style: own.clone() }),
                }
            }
        }
        for s in &self.siblings {
            s.collect(&own, out);
        }
    }

    /// `getUnformattedText`.
    pub fn unformatted_text(&self) -> String {
        self.spans().into_iter().map(|s| s.text).collect()
    }

    /// `getFormattedText`: código de estilo + texto + `§r` para cada trecho.
    pub fn formatted_text(&self) -> String {
        self.spans().into_iter().map(|s| format!("{}{}§r", s.style.code(), s.text)).collect()
    }
}

/// Texto com códigos `§` (cores e estilos), sem trechos vazios nem `§r` sobrando no fim.
pub fn legacy(c: &Component) -> String {
    let mut out: String = c
        .spans()
        .into_iter()
        .filter(|s| !s.text.is_empty())
        .map(|s| format!("{}{}§r", s.style.code(), s.text))
        .collect();
    while out.ends_with("§r") {
        out.truncate(out.len() - "§r".len());
    }
    out
}

/// Texto com códigos `§` (cores e estilos).
pub fn to_legacy(json: &str) -> String {
    legacy(&Component::from_json(json))
}

/// Texto sem cores (para o terminal).
pub fn to_plain(json: &str) -> String {
    mc_render::ui::strip_codes(&to_legacy(json))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_null_vira_texto_vazio() {
        assert_eq!(to_legacy("null"), "");
    }

    fn comp(j: &str) -> Component {
        Component::from_json(j)
    }

    #[test]
    fn nested_extra_and_codes() {
        let j = r#"{"extra":[{"color":"gold","text":"[AMIGOS] "},{"text":"§a_rd3 "},"está online."],"text":""}"#;
        assert_eq!(to_plain(j), "[AMIGOS] _rd3 está online.");
        assert_eq!(to_plain("\"oi\""), "oi");
        assert!(to_legacy(j).starts_with("§6[AMIGOS] "));
    }

    #[test]
    fn style_is_inherited_from_the_parent() {
        // O filho herda cor e negrito do pai e pode sobrescrever só o que declara.
        let c = comp(r#"{"text":"a","color":"red","bold":true,"extra":[{"text":"b"},{"text":"c","color":"blue","bold":false}]}"#);
        let spans = c.spans();
        assert_eq!(spans.iter().map(|s| s.text.as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);
        assert_eq!(spans[1].style.color, Some('c'));
        assert_eq!(spans[1].style.bold, Some(true));
        assert_eq!(spans[2].style.color, Some('9'));
        assert_eq!(spans[2].style.bold, Some(false));
        assert_eq!(spans[1].style.code(), "§c§l");
        assert_eq!(spans[2].style.code(), "§9");
    }

    #[test]
    fn array_elements_are_siblings_of_the_first() {
        // [A, B]: B vira irmão de A e herda o estilo dele (diferente de listas "independentes").
        let c = comp(r#"[{"text":"a","color":"green"},"b",{"text":"c","italic":true}]"#);
        let spans = c.spans();
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[1].style.color, Some('a'));
        assert_eq!(spans[2].style.color, Some('a'));
        assert_eq!(spans[2].style.italic, Some(true));
        assert_eq!(spans[1].style.italic, None);
    }

    #[test]
    fn code_order_matches_chat_style() {
        let c = comp(r#"{"text":"x","color":"gold","bold":true,"italic":true,"underlined":true,"obfuscated":true,"strikethrough":true}"#);
        // getFormattingCode: cor, negrito, itálico, sublinhado, ofuscado, tachado.
        assert_eq!(c.formatted_text(), "§6§l§o§n§k§mx§r");
    }

    #[test]
    fn click_hover_and_insertion_are_inherited() {
        let j = r#"{"text":"clique ","clickEvent":{"action":"open_url","value":"https://example.com"},
            "hoverEvent":{"action":"show_text","value":{"text":"dica","color":"red"}},"insertion":"ins",
            "extra":[{"text":"filho"},{"text":"outro","clickEvent":{"action":"run_command","value":"/home"}}]}"#;
        let spans = comp(j).spans();
        assert_eq!(spans.len(), 3);
        for s in &spans {
            assert_eq!(s.style.insertion.as_deref(), Some("ins"));
            assert_eq!(s.style.hover.as_ref().unwrap().action, HoverAction::ShowText);
        }
        assert_eq!(spans[1].style.click, Some(ClickEvent { action: ClickAction::OpenUrl, value: "https://example.com".into() }));
        assert_eq!(spans[2].style.click, Some(ClickEvent { action: ClickAction::RunCommand, value: "/home".into() }));
        assert_eq!(spans[0].style.hover.as_ref().unwrap().value.unformatted_text(), "dica");
    }

    #[test]
    fn disallowed_click_actions_are_dropped() {
        let c = comp(r#"{"text":"x","clickEvent":{"action":"open_file","value":"/etc/passwd"}}"#);
        assert!(c.spans()[0].style.click.is_none());
        let c = comp(r#"{"text":"x","clickEvent":{"action":"twitch_user_info","value":"a"}}"#);
        assert!(c.spans()[0].style.click.is_none());
        let c = comp(r#"{"text":"x","clickEvent":{"action":"suggest_command","value":"/tp"}}"#);
        assert_eq!(c.spans()[0].style.click.as_ref().unwrap().action, ClickAction::SuggestCommand);
    }

    /// Padrões mínimos das chaves que o teste formata. `install` ainda cobre com o `.lang`
    /// do jar, se ele existir; sem o jar, esta base evita falhar um clone limpo.
    fn install_test_lang() {
        crate::lang::install(
            "pt_BR",
            Some(
                "chat.type.text=<%s> %s\n\
                 multiplayer.player.joined=%s entrou no jogo\n\
                 commands.generic.notFound=Comando desconhecido.\n",
            ),
        );
    }

    #[test]
    fn translate_with_args() {
        install_test_lang();
        let j = r#"{"translate":"chat.type.text","with":["Lohan","oi"]}"#;
        assert_eq!(to_plain(j), "<Lohan> oi");
        // Chaves do .lang que antes estavam numa lista fixa, e chave desconhecida crua.
        let j = r#"{"translate":"multiplayer.player.joined","with":["Lohan"]}"#;
        assert_eq!(to_plain(j), crate::lang::trf("multiplayer.player.joined", &["Lohan"]));
        assert!(to_plain(j).contains("Lohan") && !to_plain(j).contains("%s"));
        assert_eq!(to_plain(r#"{"translate":"chave.inexistente.xyz"}"#), "chave.inexistente.xyz");
        assert!(to_plain(r#"{"translate":"commands.generic.notFound"}"#).len() > 10);
    }

    #[test]
    fn translation_pieces_follow_the_java_format_rules() {
        let p = |s: &str, n: usize| -> String {
            translation_pieces(s, n)
                .unwrap()
                .iter()
                .map(|p| match p {
                    Piece::Lit(t) => t.clone(),
                    Piece::Arg(i) => format!("[{i}]"),
                })
                .collect()
        };
        assert_eq!(p("<%s> %s", 2), "<[0]> [1]");
        assert_eq!(p("%2$s depois de %1$s", 2), "[1] depois de [0]");
        assert_eq!(p("100%% %s", 1), "100% [0]");
        // Argumento a mais no padrão que não foi enviado: some (`lvt_11_1_ < formatArgs.length`).
        assert_eq!(p("a %s b %s", 1), "a [0] b ");
        // Só `%s` é suportado (`%d` lança ChatComponentTranslationFormatException).
        assert!(translation_pieces("%d", 1).is_none());
        // `%` seguido de espaço não é especificador.
        assert_eq!(p("50% ok", 0), "50% ok");
    }

    #[test]
    fn translation_args_inherit_the_translation_style() {
        install_test_lang();
        let j = r#"{"translate":"chat.type.text","color":"yellow","with":[{"text":"Lohan","bold":true},"oi"]}"#;
        let spans = comp(j).spans();
        // "<" + nome + "> " + "oi"
        assert_eq!(spans.iter().map(|s| s.text.as_str()).collect::<String>(), "<Lohan> oi");
        let name = spans.iter().find(|s| s.text == "Lohan").unwrap();
        assert_eq!(name.style.color, Some('e'));
        assert_eq!(name.style.bold, Some(true));
        assert_eq!(spans[0].style.color, Some('e'));
        assert_eq!(spans[0].style.bold, None);
    }

    #[test]
    fn plain_translation_arg_drops_its_insertion() {
        // Texto sem estilo e sem irmãos vira String; `insertion` não conta como estilo (isEmpty).
        let c = comp(r#"{"translate":"x","with":[{"text":"a","insertion":"i"}]}"#);
        match c.kind {
            Kind::Translate { args, .. } => assert_eq!(args, vec![Arg::Str("a".into())]),
            o => panic!("{o:?}"),
        }
    }

    #[test]
    fn invalid_json_becomes_raw_text() {
        assert_eq!(comp("{isso não é json").unformatted_text(), "{isso não é json");
        assert_eq!(comp(r#"{"foo":1}"#).unformatted_text(), r#"{"foo":1}"#);
        assert_eq!(comp("42").unformatted_text(), "42");
    }

    #[test]
    fn formatted_text_closes_each_span_with_reset() {
        let c = comp(r#"{"text":"a","color":"red","extra":[{"text":"b","bold":true}]}"#);
        assert_eq!(c.formatted_text(), "§ca§r§c§lb§r");
        assert_eq!(c.unformatted_text(), "ab");
    }
}
