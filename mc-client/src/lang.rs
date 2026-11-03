//! Traduções do jogo (pt_BR.lang dos assets do Prism) e nomes padrão dos itens.

use std::collections::HashMap;
use std::sync::RwLock;

use mc_world::item_keys::ITEM_KEYS;

const COLORS: [&str; 16] = [
    "white", "orange", "magenta", "lightBlue", "yellow", "lime", "pink", "gray", "silver", "cyan", "purple", "blue",
    "brown", "green", "red", "black",
];
const WOODS: [&str; 6] = ["oak", "spruce", "birch", "jungle", "acacia", "big_oak"];

#[derive(Default)]
pub struct Lang {
    map: HashMap<String, String>,
    keys: HashMap<u16, &'static str>,
}

impl Lang {
    /// Carrega `minecraft/lang/<código>.lang` de toda a pilha de recursos (pacotes por cima).
    pub fn load(code: &str) -> Self {
        let mut lang = Lang { map: HashMap::new(), keys: ITEM_KEYS.iter().copied().collect() };
        for text in crate::res::all_text(&format!("assets/minecraft/lang/{code}.lang")) {
            for line in text.lines() {
                if let Some((k, v)) = line.split_once('=') {
                    lang.map.insert(k.to_string(), v.to_string());
                }
            }
        }
        lang
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.map.get(key).map(String::as_str)
    }

    /// Junta outro idioma por cima (o Locale da 1.8 carrega en_US e depois o escolhido).
    fn overlay(&mut self, text: &str) {
        for line in text.lines() {
            if line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                self.map.insert(k.to_string(), v.to_string());
            }
        }
    }
}

/// Idioma das telas (I18n.format). Trocado pela tela de idiomas.
static CURRENT: RwLock<Option<Lang>> = RwLock::new(None);

/// Texto do `.lang` de `code` em toda a pilha (`Locale.loadLocaleData`: um por pacote, em ordem).
fn lang_text(code: &str) -> Option<String> {
    let all = crate::res::all_text(&format!("assets/minecraft/lang/{code}.lang"));
    (!all.is_empty()).then(|| all.join("\n"))
}

/// O en_US de toda a pilha (jar e pacotes), a base de todo idioma.
pub fn en_us_text() -> Option<String> {
    lang_text("en_US")
}

/// Liga o idioma `code`, com o en_US do jar por baixo.
pub fn install(code: &str, en_us: Option<&str>) {
    let mut l = Lang::default();
    if let Some(t) = en_us {
        l.overlay(t);
    }
    if let Some(t) = lang_text(code) {
        l.overlay(&t);
    }
    if let Ok(mut g) = CURRENT.write() {
        *g = Some(l);
    }
}

/// `I18n.format(key)`: a chave crua quando não há tradução.
pub fn tr(key: &str) -> String {
    trf(key, &[])
}

/// O padrão cru da chave, sem formatar (`LanguageMap.translateKey` quando existe).
pub fn raw(key: &str) -> Option<String> {
    CURRENT.read().ok().and_then(|g| g.as_ref().and_then(|l| l.get(key).map(str::to_string)))
}

/// `I18n.format(key, args...)` com `%s`, `%d` e `%1$s`.
pub fn trf(key: &str, args: &[&str]) -> String {
    let pattern = CURRENT.read().ok().and_then(|g| g.as_ref().and_then(|l| l.get(key).map(str::to_string)));
    match pattern {
        Some(p) => format_java(&p, args),
        None => key.to_string(),
    }
}

/// O subconjunto de `String.format` que os `.lang` usam.
pub fn format_java(p: &str, args: &[&str]) -> String {
    let mut out = String::new();
    let mut next = 0;
    let mut chars = p.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let mut spec = String::new();
        while let Some(&d) = chars.peek() {
            spec.push(d);
            chars.next();
            if d.is_ascii_alphabetic() || d == '%' {
                break;
            }
        }
        if spec == "%" {
            out.push('%');
            continue;
        }
        let idx = match spec.split_once('$') {
            Some((n, _)) => n.parse::<usize>().unwrap_or(1).saturating_sub(1),
            None => {
                next += 1;
                next - 1
            }
        };
        out.push_str(args.get(idx).copied().unwrap_or(""));
    }
    out
}

/// Idiomas listados no `pack.mcmeta` dos assets que têm arquivo: (código, "Nome (Região)").
pub fn available() -> Vec<(String, String)> {
    let Some(mut jar) = crate::res::jar() else { return Vec::new() };
    let Some(meta) = jar.default_bytes("pack.mcmeta") else { return Vec::new() };
    let Ok(m) = serde_json::from_slice::<serde_json::Value>(&meta) else { return Vec::new() };
    let mut out: Vec<(String, String)> = m["language"]
        .as_object()
        .map(|o| {
            o.iter()
                .filter(|(code, _)| code.as_str() == "en_US" || jar.has(&format!("assets/minecraft/lang/{code}.lang")))
                .map(|(code, l)| {
                    (code.clone(), format!("{} ({})", l["name"].as_str().unwrap_or(code), l["region"].as_str().unwrap_or("")))
                })
                .collect()
        })
        .unwrap_or_default();
    // LanguageManager devolve um TreeSet ordenado pelo código.
    out.sort();
    out
}

impl Lang {

    /// Nome padrão do item (como `ItemStack.getDisplayName` sem nome customizado).
    pub fn item_name(&self, id: i16, damage: i16) -> Option<String> {
        let base = *self.keys.get(&(id as u16))?;
        let d = damage.max(0) as usize;
        let variant = match base {
            "tile.cloth" | "tile.stainedGlass" | "tile.thinStainedGlass" | "tile.clayHardenedStained" | "tile.woolCarpet" => {
                Some(COLORS[d & 15].to_string())
            }
            "item.dyePowder" => Some(COLORS[15 - (d & 15)].to_string()),
            "tile.wood" | "tile.sapling" | "tile.woodSlab" => Some(WOODS[d.min(5)].to_string()),
            "tile.log" | "tile.leaves" => Some(WOODS[d & 3].to_string()),
            "tile.log2" | "tile.leaves2" => Some(WOODS[4 + (d & 1)].to_string()),
            "tile.stone" => {
                Some(["stone", "granite", "graniteSmooth", "diorite", "dioriteSmooth", "andesite", "andesiteSmooth"][d.min(6)].to_string())
            }
            "tile.sponge" => Some(if d == 1 { "wet" } else { "dry" }.to_string()),
            "item.skull" => Some(["skeleton", "wither", "zombie", "char", "creeper"][d.min(4)].to_string()),
            _ => None,
        };
        let with_variant = variant.and_then(|v| self.get(&format!("{base}.{v}.name")));
        with_variant.or_else(|| self.get(&format!("{base}.name"))).map(str::to_string)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "precisa dos assets do Prism nesta máquina"]
    fn portuguese_names() {
        let l = Lang::load("pt_BR");
        assert_eq!(l.item_name(345, 0).as_deref(), Some("Bússola"));
        assert_eq!(l.item_name(35, 3).as_deref(), Some("Lã Azul Claro"));
        assert_eq!(l.item_name(159, 14).as_deref(), Some("Argila Tingida de Vermelho"));
        assert_eq!(l.item_name(58, 0).as_deref(), Some("Bancada de Trabalho"));
        assert_eq!(l.item_name(351, 0).as_deref(), Some("Bolsa de Tinta"));
    }
}
