//! `JsonToNBT`: o NBT em texto (`{id:"minecraft:stone",Count:1b}`) que os eventos `show_item` e
//! `show_entity` do chat carregam. Porte linha a linha de `net.minecraft.nbt.JsonToNBT` (1.8.9),
//! inclusive as manias dele (`[d|D]` aceita `|`, listas de tipos misturados descartam o intruso).

use std::collections::HashMap;

use mc_protocol::nbt::Nbt;

#[derive(Debug, PartialEq)]
pub struct NbtError(pub String);

type Res<T> = Result<T, NbtError>;

fn err<T>(msg: impl Into<String>) -> Res<T> {
    Err(NbtError(msg.into()))
}

fn text(s: &[char]) -> String {
    s.iter().collect()
}

/// `String.trim`: tira tudo que é <= espaço.
fn trim(s: &[char]) -> &[char] {
    let a = s.iter().position(|c| *c > ' ').unwrap_or(s.len());
    let b = s.iter().rposition(|c| *c > ' ').map_or(a, |p| p + 1);
    &s[a..b]
}

/// `func_179271_b`: a aspa em `i` está escapada por um número ímpar de barras?
fn escaped(s: &[char], i: usize) -> bool {
    i > 0 && s[i - 1] == '\\' && !escaped(s, i - 1)
}

/// `func_150310_b`: quantas tags de topo o texto tem (e se os colchetes fecham).
fn top_tags(s: &[char]) -> Res<i32> {
    let (mut count, mut in_quote) = (0, false);
    let mut stack: Vec<char> = Vec::new();
    for (i, &c) in s.iter().enumerate() {
        if c == '"' {
            if escaped(s, i) {
                if !in_quote {
                    return err(format!("Illegal use of \\\": {}", text(s)));
                }
            } else {
                in_quote = !in_quote;
            }
        } else if !in_quote {
            if c != '{' && c != '[' {
                if c == '}' && stack.pop() != Some('{') {
                    return err(format!("Unbalanced curly brackets {{}}: {}", text(s)));
                }
                if c == ']' && stack.pop() != Some('[') {
                    return err(format!("Unbalanced square brackets []: {}", text(s)));
                }
            } else {
                if stack.is_empty() {
                    count += 1;
                }
                stack.push(c);
            }
        }
    }
    if in_quote {
        return err(format!("Unbalanced quotation: {}", text(s)));
    }
    if !stack.is_empty() {
        return err(format!("Unbalanced brackets: {}", text(s)));
    }
    if count == 0 && !s.is_empty() {
        count = 1;
    }
    Ok(count)
}

/// `func_150312_a`: índice de `ch` fora de aspas e antes de qualquer `{`/`[`, ou -1.
fn find_outside(s: &[char], ch: char) -> i32 {
    let mut outside = true;
    for (i, &c) in s.iter().enumerate() {
        if c == '"' {
            if !escaped(s, i) {
                outside = !outside;
            }
        } else if outside {
            if c == ch {
                return i as i32;
            }
            if c == '{' || c == '[' {
                return -1;
            }
        }
    }
    -1
}

/// `func_179269_a`: o trecho do próximo elemento (até a vírgula de nível zero).
fn element_end(s: &[char], colon: i32) -> Res<usize> {
    let mut stack: Vec<char> = Vec::new();
    let mut k = (colon + 1) as usize;
    let (mut in_quote, mut quote_started, mut seen_text) = (false, false, false);
    let mut last_quote_end = 0usize;
    while k < s.len() {
        let c = s[k];
        if c == '"' {
            if escaped(s, k) {
                if !in_quote {
                    return err(format!("Illegal use of \\\": {}", text(s)));
                }
            } else {
                in_quote = !in_quote;
                if in_quote && !seen_text {
                    quote_started = true;
                }
                if !in_quote {
                    last_quote_end = k;
                }
            }
        } else if !in_quote {
            if c != '{' && c != '[' {
                if c == '}' && stack.pop() != Some('{') {
                    return err(format!("Unbalanced curly brackets {{}}: {}", text(s)));
                }
                if c == ']' && stack.pop() != Some('[') {
                    return err(format!("Unbalanced square brackets []: {}", text(s)));
                }
                if c == ',' && stack.is_empty() {
                    return Ok(k);
                }
            } else {
                stack.push(c);
            }
        }
        if !c.is_whitespace() {
            if !in_quote && quote_started && last_quote_end != k {
                return Ok(last_quote_end + 1);
            }
            seen_text = true;
        }
        k += 1;
    }
    Ok(k)
}

/// `func_150314_a`.
fn next_element(s: &[char], compound: bool) -> Res<String> {
    let mut colon = find_outside(s, ':');
    let comma = find_outside(s, ',');
    if compound {
        if colon == -1 {
            return err(format!("Unable to locate name/value separator for string: {}", text(s)));
        }
        if comma != -1 && comma < colon {
            return err(format!("Name error at: {}", text(s)));
        }
    } else if colon == -1 || colon > comma {
        colon = -1;
    }
    Ok(text(&s[..element_end(s, colon)?]))
}

/// `func_150313_b`: o nome (antes do `:`).
fn name_of(s: &[char], list_item: bool) -> Res<String> {
    let mut s = s;
    if list_item {
        s = trim(s);
        if s.first() == Some(&'{') || s.first() == Some(&'[') {
            return Ok(String::new());
        }
    }
    let colon = find_outside(s, ':');
    if colon == -1 {
        if list_item {
            return Ok(String::new());
        }
        return err(format!("Unable to locate name/value separator for string: {}", text(s)));
    }
    Ok(text(trim(&s[..colon as usize])))
}

/// `func_150311_c`: o valor (depois do `:`).
fn value_of(s: &[char], list_item: bool) -> Res<String> {
    let mut s = s;
    if list_item {
        s = trim(s);
        if s.first() == Some(&'{') || s.first() == Some(&'[') {
            return Ok(text(s));
        }
    }
    let colon = find_outside(s, ':');
    if colon == -1 {
        if list_item {
            return Ok(text(s));
        }
        return err(format!("Unable to locate name/value separator for string: {}", text(s)));
    }
    Ok(text(trim(&s[colon as usize + 1..])))
}

enum Any {
    Compound(String, Vec<Any>),
    List(String, Vec<Any>),
    Primitive(String, String),
}

impl Any {
    fn name(&self) -> &str {
        match self {
            Any::Compound(n, _) | Any::List(n, _) | Any::Primitive(n, _) => n,
        }
    }
}

fn is_separator(c: char) -> bool {
    matches!(c, ',' | '{' | '}' | '[' | ']')
}

/// `func_150316_a`.
fn build(name: &str, value: &str) -> Res<Any> {
    let chars: Vec<char> = value.chars().collect();
    let v = trim(&chars);
    if v.first() == Some(&'{') {
        if v.len() < 2 {
            return err("String index out of range");
        }
        let mut rest = &v[1..v.len() - 1];
        let mut fields = Vec::new();
        while !rest.is_empty() {
            let first: Vec<char> = next_element(rest, true)?.chars().collect();
            if !first.is_empty() {
                fields.push(build_pair(&first, false)?);
            }
            if rest.len() < first.len() + 1 {
                break;
            }
            let c = rest[first.len()];
            if !is_separator(c) {
                return err(format!("Unexpected token '{c}' at: {}", text(&rest[first.len()..])));
            }
            rest = &rest[first.len() + 1..];
        }
        Ok(Any::Compound(name.to_string(), fields))
    } else if v.first() == Some(&'[') && !is_int_array_literal(v) {
        if v.len() < 2 {
            return err("String index out of range");
        }
        let mut rest = &v[1..v.len() - 1];
        let mut items = Vec::new();
        while !rest.is_empty() {
            let first: Vec<char> = next_element(rest, false)?.chars().collect();
            if !first.is_empty() {
                items.push(build_pair(&first, true)?);
            }
            if rest.len() < first.len() + 1 {
                break;
            }
            let c = rest[first.len()];
            if !is_separator(c) {
                return err(format!("Unexpected token '{c}' at: {}", text(&rest[first.len()..])));
            }
            rest = &rest[first.len() + 1..];
        }
        Ok(Any::List(name.to_string(), items))
    } else {
        Ok(Any::Primitive(name.to_string(), text(v)))
    }
}

/// `func_179270_a`.
fn build_pair(s: &[char], list_item: bool) -> Res<Any> {
    let name = name_of(s, list_item)?;
    let value = value_of(s, list_item)?;
    build(&name, &value)
}

/// `\[[-+\d|,\s]+\]`.
fn is_int_array_literal(v: &[char]) -> bool {
    v.len() >= 3
        && v[0] == '['
        && v[v.len() - 1] == ']'
        && v[1..v.len() - 1].iter().all(|c| matches!(c, '-' | '+' | '|' | ',') || c.is_ascii_digit() || c.is_whitespace())
}

/// `[-+]?[0-9]*\.?[0-9]+`.
fn is_decimal(s: &str) -> bool {
    let body = s.strip_prefix(['-', '+']).unwrap_or(s);
    let dots = body.matches('.').count();
    !body.is_empty()
        && dots <= 1
        && body.chars().all(|c| c.is_ascii_digit() || c == '.')
        && body.ends_with(|c: char| c.is_ascii_digit())
}

/// `[-+]?[0-9]+`.
fn is_integer(s: &str) -> bool {
    let body = s.strip_prefix(['-', '+']).unwrap_or(s);
    !body.is_empty() && body.chars().all(|c| c.is_ascii_digit())
}

/// Sufixo do tipo (`[x|X]` aceita também a barra, como as regex do Java).
fn typed<'a>(s: &'a str, lower: char) -> Option<&'a str> {
    let last = s.chars().last()?;
    (last == lower || last == lower.to_ascii_uppercase() || last == '|').then(|| &s[..s.len() - 1])
}

fn unescape_quotes(s: &str) -> String {
    s.replace("\\\"", "\"")
}

fn primitive(value: &str) -> Nbt {
    let fail = || Nbt::String(unescape_quotes(value));
    if let Some(n) = typed(value, 'd').filter(|n| is_decimal(n)) {
        return n.parse().map(Nbt::Double).unwrap_or_else(|_| fail());
    }
    if let Some(n) = typed(value, 'f').filter(|n| is_decimal(n)) {
        return n.parse().map(Nbt::Float).unwrap_or_else(|_| fail());
    }
    if let Some(n) = typed(value, 'b').filter(|n| is_integer(n)) {
        return n.parse().map(Nbt::Byte).unwrap_or_else(|_| fail());
    }
    if let Some(n) = typed(value, 'l').filter(|n| is_integer(n)) {
        return n.parse().map(Nbt::Long).unwrap_or_else(|_| fail());
    }
    if let Some(n) = typed(value, 's').filter(|n| is_integer(n)) {
        return n.parse().map(Nbt::Short).unwrap_or_else(|_| fail());
    }
    if is_integer(value) {
        return value.parse().map(Nbt::Int).unwrap_or_else(|_| fail());
    }
    if is_decimal(value) {
        return value.parse().map(Nbt::Double).unwrap_or_else(|_| fail());
    }
    if value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("false") {
        return Nbt::Byte(value.eq_ignore_ascii_case("true") as i8);
    }
    if value.starts_with('[') && value.ends_with(']') && value.len() >= 2 {
        let inner = &value[1..value.len() - 1];
        let parsed: Result<Vec<i32>, _> = inner.split(',').filter(|p| !p.is_empty()).map(|p| p.trim().parse::<i32>()).collect();
        return match parsed {
            Ok(v) => Nbt::IntArray(v),
            Err(_) => Nbt::String(value.to_string()),
        };
    }
    let mut v = value;
    if v.len() >= 2 && v.starts_with('"') && v.ends_with('"') {
        v = &v[1..v.len() - 1];
    }
    let v = unescape_quotes(v);
    let chars: Vec<char> = v.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if i + 1 < chars.len() && chars[i] == '\\' && chars[i + 1] == '\\' {
            out.push('\\');
            i += 2;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    Nbt::String(out)
}

/// ID da tag no formato NBT (para a regra de tipo único das listas).
fn tag_id(n: &Nbt) -> u8 {
    match n {
        Nbt::Byte(_) => 1,
        Nbt::Short(_) => 2,
        Nbt::Int(_) => 3,
        Nbt::Long(_) => 4,
        Nbt::Float(_) => 5,
        Nbt::Double(_) => 6,
        Nbt::ByteArray(_) => 7,
        Nbt::String(_) => 8,
        Nbt::List(_) => 9,
        Nbt::Compound(_) => 10,
        Nbt::IntArray(_) => 11,
    }
}

fn parse_any(a: &Any) -> Nbt {
    match a {
        Any::Compound(_, fields) => {
            let mut m = HashMap::new();
            for f in fields {
                m.insert(f.name().to_string(), parse_any(f));
            }
            Nbt::Compound(m)
        }
        Any::List(_, items) => {
            let mut v: Vec<Nbt> = Vec::new();
            for it in items {
                let tag = parse_any(it);
                // NBTTagList.appendTag: o primeiro define o tipo; outro tipo é descartado.
                if v.first().is_none_or(|f| tag_id(f) == tag_id(&tag)) {
                    v.push(tag);
                }
            }
            Nbt::List(v)
        }
        Any::Primitive(_, value) => primitive(value),
    }
}

/// `JsonToNBT.getTagFromJson`: o texto tem que ser um compound `{...}`.
pub fn parse(s: &str) -> Result<Nbt, NbtError> {
    let chars: Vec<char> = s.chars().collect();
    let t = trim(&chars);
    if t.first() != Some(&'{') {
        return err("Invalid tag encountered, expected '{' as first char.");
    }
    if top_tags(t)? != 1 {
        return err("Encountered multiple top tags, only one expected");
    }
    let any = build("tag", &text(t))?;
    Ok(parse_any(&any))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get<'a>(n: &'a Nbt, k: &str) -> &'a Nbt {
        n.get(k).unwrap_or_else(|| panic!("falta {k} em {n:?}"))
    }

    #[test]
    fn item_compound() {
        let n = parse(r#"{id:"minecraft:diamond_sword",Count:1b,Damage:5s,tag:{display:{Name:"Espada",Lore:["a","b"]},ench:[{id:16s,lvl:5s}]}}"#).unwrap();
        assert_eq!(get(&n, "id"), &Nbt::String("minecraft:diamond_sword".into()));
        assert_eq!(get(&n, "Count"), &Nbt::Byte(1));
        assert_eq!(get(&n, "Damage"), &Nbt::Short(5));
        let tag = get(&n, "tag");
        assert_eq!(get(get(tag, "display"), "Name"), &Nbt::String("Espada".into()));
        assert_eq!(get(get(tag, "display"), "Lore"), &Nbt::List(vec![Nbt::String("a".into()), Nbt::String("b".into())]));
        match get(tag, "ench") {
            Nbt::List(v) => assert_eq!(get(&v[0], "lvl"), &Nbt::Short(5)),
            o => panic!("{o:?}"),
        }
    }

    #[test]
    fn primitives_follow_the_java_patterns() {
        let n = parse(r#"{a:1,b:2.5,c:3d,d:4f,e:5L,f:true,g:[1,2,3],h:abc,i:"x,y",j:99999999999,k:-7b,l:300b}"#).unwrap();
        assert_eq!(get(&n, "a"), &Nbt::Int(1));
        assert_eq!(get(&n, "b"), &Nbt::Double(2.5));
        assert_eq!(get(&n, "c"), &Nbt::Double(3.0));
        assert_eq!(get(&n, "d"), &Nbt::Float(4.0));
        assert_eq!(get(&n, "e"), &Nbt::Long(5));
        assert_eq!(get(&n, "f"), &Nbt::Byte(1));
        assert_eq!(get(&n, "g"), &Nbt::IntArray(vec![1, 2, 3]));
        assert_eq!(get(&n, "h"), &Nbt::String("abc".into()));
        assert_eq!(get(&n, "i"), &Nbt::String("x,y".into()));
        // Estouro de inteiro cai em string (NumberFormatException), byte fora da faixa também.
        assert_eq!(get(&n, "j"), &Nbt::String("99999999999".into()));
        assert_eq!(get(&n, "k"), &Nbt::Byte(-7));
        assert_eq!(get(&n, "l"), &Nbt::String("300b".into()));
    }

    #[test]
    fn escaped_quotes_and_backslashes() {
        let n = parse(r#"{s:"ele disse \"oi\" \\ fim"}"#).unwrap();
        assert_eq!(get(&n, "s"), &Nbt::String("ele disse \"oi\" \\ fim".into()));
    }

    #[test]
    fn invalid_inputs_are_errors() {
        assert!(parse("abc").is_err());
        assert!(parse("{a:1").is_err());
        assert!(parse("{a:1}{b:2}").is_err());
        assert!(parse("{a}").is_err());
    }

    #[test]
    fn mixed_list_drops_the_odd_tag() {
        let n = parse("{l:[1,2,abc]}").unwrap();
        assert_eq!(get(&n, "l"), &Nbt::List(vec![Nbt::Int(1), Nbt::Int(2)]));
    }
}
