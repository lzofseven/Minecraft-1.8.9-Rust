//! Mundos do modo Um jogador (`GuiSelectWorld`, `GuiCreateWorld`, `GuiCreateFlatWorld`, `GuiFlatPresets`).
//!
//! Cada mundo é uma pasta em `<dados>/singleplayer/<pasta>` com o `server.properties` do servidor local, o
//! mundo em `world/` e o `mundo.json` (nome, modo de jogo, comandos, datas) que a lista mostra.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

pub fn root() -> PathBuf {
    crate::setup::data_dir().join("singleplayer")
}

pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// `WorldSettings.GameType` mais o hardcore, como no botão de modo do `GuiCreateWorld`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Survival,
    Hardcore,
    Creative,
}

impl Mode {
    pub fn key(self) -> &'static str {
        match self {
            Mode::Survival => "survival",
            Mode::Hardcore => "hardcore",
            Mode::Creative => "creative",
        }
    }

    fn from_key(s: &str) -> Mode {
        match s {
            "hardcore" => Mode::Hardcore,
            "creative" => Mode::Creative,
            _ => Mode::Survival,
        }
    }

    /// Ordem do botão: sobrevivência → hardcore → criativo.
    pub fn next(self) -> Mode {
        match self {
            Mode::Survival => Mode::Hardcore,
            Mode::Hardcore => Mode::Creative,
            Mode::Creative => Mode::Survival,
        }
    }
}

/// `WorldType` do botão "Tipo de mundo" (o `debug_all_block_states` só aparece com Shift no jogo e não entra).
pub const WORLD_TYPES: [(&str, &str); 5] = [
    ("DEFAULT", "generator.default"),
    ("FLAT", "generator.flat"),
    ("LARGEBIOMES", "generator.largeBiomes"),
    ("AMPLIFIED", "generator.amplified"),
    ("CUSTOMIZED", "generator.customized"),
];

/// Superplano clássico (`FlatGeneratorInfo.getDefaultFlatGenerator`).
pub const DEFAULT_FLAT: &str = "3;minecraft:bedrock,2*minecraft:dirt,minecraft:grass;1;village";

/// Um mundo na lista.
#[derive(Clone, Debug)]
pub struct WorldEntry {
    pub folder: String,
    pub name: String,
    pub mode: Mode,
    pub cheats: bool,
    pub last_played: u64,
    /// Para o "Recriar": tipo, opções do gerador e estruturas.
    pub world_type: usize,
    pub generator: String,
    pub structures: bool,
}

impl WorldEntry {
    pub fn load(folder: &str) -> Option<WorldEntry> {
        let v: Value = serde_json::from_slice(&std::fs::read(root().join(folder).join("mundo.json")).ok()?).ok()?;
        Some(WorldEntry {
            folder: folder.to_string(),
            name: v["nome"].as_str().unwrap_or(folder).to_string(),
            mode: Mode::from_key(v["modo"].as_str().unwrap_or("")),
            cheats: v["comandos"].as_bool().unwrap_or(false),
            last_played: v["jogado"].as_u64().unwrap_or(0),
            world_type: v["tipo"].as_str().and_then(|t| WORLD_TYPES.iter().position(|(k, _)| *k == t)).unwrap_or(0),
            generator: v["gerador"].as_str().unwrap_or("").to_string(),
            structures: v["estruturas"].as_bool().unwrap_or(true),
        })
    }

    pub fn save(&self) -> std::io::Result<()> {
        let v = json!({
            "nome": self.name, "modo": self.mode.key(), "comandos": self.cheats, "jogado": self.last_played,
            "tipo": WORLD_TYPES[self.world_type.min(WORLD_TYPES.len() - 1)].0, "gerador": self.generator,
            "estruturas": self.structures,
        });
        let dir = root().join(&self.folder);
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("mundo.json"), serde_json::to_vec_pretty(&v)?)
    }

    /// A semente gravada no `level.dat` (para o "Recriar").
    pub fn seed(&self) -> Option<i64> {
        use std::io::Read;
        let raw = std::fs::read(root().join(&self.folder).join("world/level.dat")).ok()?;
        let mut data = Vec::new();
        flate2::read::GzDecoder::new(&raw[..]).read_to_end(&mut data).ok()?;
        let nbt = mc_protocol::nbt::read_root(&mut mc_protocol::Reader::new(&data)).ok()??;
        match nbt.get("Data")?.get("RandomSeed")? {
            mc_protocol::nbt::Nbt::Long(s) => Some(*s),
            _ => None,
        }
    }
}

/// `SaveFormatOld.getSaveList` ordenado como o `GuiSelectWorld` (jogado por último primeiro).
pub fn list() -> Vec<WorldEntry> {
    let mut v: Vec<WorldEntry> = std::fs::read_dir(root())
        .map(|rd| rd.flatten().filter(|e| e.path().is_dir()).filter_map(|e| WorldEntry::load(&e.file_name().to_string_lossy())).collect())
        .unwrap_or_default();
    v.sort_by(|a, b| b.last_played.cmp(&a.last_played).then_with(|| a.folder.cmp(&b.folder)));
    v
}

pub fn delete(folder: &str) -> std::io::Result<()> {
    if folder.is_empty() || folder.contains(['/', '\\']) || folder == ".." || folder == "." {
        return Ok(());
    }
    std::fs::remove_dir_all(root().join(folder))
}

/// `GuiCreateWorld.func_146317_a`: troca os caracteres proibidos por `_`, evita nomes reservados do Windows
/// e acrescenta `-` até a pasta não existir.
pub fn folder_for(name: &str) -> String {
    let mut s: String = name.trim().chars().map(|c| if "/\n\r\t\0\u{c}`?*\\<>|\":".contains(c) { '_' } else { c }).collect();
    if s.is_empty() || s.chars().all(|c| c == '.') {
        s = "World".to_string();
    }
    const RESERVED: [&str; 22] = [
        "CON", "COM", "PRN", "AUX", "CLOCK$", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7",
    ];
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(&s)) {
        s = format!("_{s}_");
    }
    while root().join(&s).exists() {
        s.push('-');
    }
    s
}

/// Um mundo novo pelo `GuiCreateWorld`.
#[derive(Clone, Debug)]
pub struct NewWorld {
    pub name: String,
    pub seed: String,
    pub mode: Mode,
    pub structures: bool,
    pub world_type: usize,
    pub cheats: bool,
    /// `generator-settings` (texto do superplano).
    pub generator: String,
}

impl NewWorld {
    /// Chaves do `server.properties` para gerar o mundo.
    pub fn properties(&self) -> Vec<(&'static str, String)> {
        let ty = WORLD_TYPES[self.world_type.min(WORLD_TYPES.len() - 1)].0;
        vec![
            ("level-seed", self.seed.clone()),
            ("level-type", ty.to_string()),
            ("generator-settings", if ty == "FLAT" { self.generator.clone() } else { String::new() }),
            ("generate-structures", self.structures.to_string()),
            ("gamemode", if self.mode == Mode::Creative { "1" } else { "0" }.to_string()),
            ("hardcore", (self.mode == Mode::Hardcore).to_string()),
            ("difficulty", if self.mode == Mode::Hardcore { "3" } else { "2" }.to_string()),
        ]
    }
}

// ------------------------------------------------------------------ superplano

/// Uma camada do superplano (`FlatLayerInfo`).
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub count: u32,
    /// Nome como no texto (`minecraft:stone` ou o número).
    pub block: String,
    pub meta: u8,
}

impl Layer {
    /// `Block.getBlockFromName`: nome de registro (`minecraft:` opcional) ou o número.
    pub fn block_id(&self) -> Option<i16> {
        let b = self.block.trim();
        let names = crate::gen_block_names::BLOCK_NAMES;
        if let Ok(n) = b.parse::<u16>() {
            return names.iter().find(|(i, _)| *i == n).map(|(i, _)| *i as i16);
        }
        let full = if b.contains(':') { b.to_ascii_lowercase() } else { format!("minecraft:{}", b.to_ascii_lowercase()) };
        names.iter().find(|(_, n)| *n == full).map(|(i, _)| *i as i16)
    }

    /// O item mostrado na lista (`GuiCreateFlatWorld.Details`): água e lava viram os baldes.
    pub fn icon_item(&self) -> Option<(i16, i16)> {
        match self.block_id()? {
            0 => None,
            8 | 9 => Some((326, 0)),
            10 | 11 => Some((327, 0)),
            id => Some((id, self.meta as i16)),
        }
    }

    fn to_text(&self) -> String {
        let mut s = String::new();
        if self.count > 1 {
            s.push_str(&format!("{}*", self.count));
        }
        s.push_str(&self.block);
        if self.meta > 0 {
            s.push_str(&format!(":{}", self.meta));
        }
        s
    }
}

/// `FlatGeneratorInfo` só com o que a tela mexe: as camadas (de baixo para cima); bioma e estruturas ficam como
/// vieram no texto.
#[derive(Clone, Debug, PartialEq)]
pub struct Flat {
    pub layers: Vec<Layer>,
    /// `bioma;estruturas` do texto original.
    pub rest: String,
}

impl Flat {
    /// `FlatGeneratorInfo.createFlatGeneratorFromString`: texto inválido vira o superplano clássico.
    pub fn parse(s: &str) -> Flat {
        Self::try_parse(s).unwrap_or_else(|| Self::try_parse(DEFAULT_FLAT).unwrap())
    }

    fn try_parse(s: &str) -> Option<Flat> {
        let parts: Vec<&str> = s.split(';').collect();
        let version: u32 = if parts.len() == 1 { 0 } else { parts[0].trim().parse().ok()? };
        if version > 3 {
            return None;
        }
        let j = if parts.len() == 1 { 0 } else { 1 };
        let mut layers = Vec::new();
        let mut height = 0u32;
        for l in parts.get(j)?.split(',') {
            let l = l.trim();
            let (count, block) = match l.split_once(if version >= 3 { '*' } else { 'x' }) {
                Some((c, b)) => (c.trim().parse::<u32>().ok()?.max(1), b),
                None => (1, l),
            };
            let pieces: Vec<&str> = block.split(':').collect();
            let (name, meta) = if version >= 3 && pieces.len() > 1 && pieces[0].parse::<i32>().is_err() {
                (format!("{}:{}", pieces[0], pieces[1]), pieces.get(2))
            } else {
                (pieces[0].to_string(), pieces.get(1))
            };
            let meta = match meta {
                Some(m) => m.parse::<u8>().ok().filter(|m| *m < 16)?,
                None => 0,
            };
            let layer = Layer { count, block: name, meta };
            layer.block_id()?;
            height += count;
            if height > 256 {
                return None;
            }
            layers.push(layer);
        }
        if layers.is_empty() {
            return None;
        }
        let rest = if parts.len() > j + 1 { parts[j + 1..].join(";") } else { "1;village".to_string() };
        // Versões antigas (0 a 2) sem estruturas no texto ganham a vila, como no jogo.
        let rest = if version == 0 && !rest.contains(';') { format!("{rest};village") } else { rest };
        Some(Flat { layers, rest })
    }

    /// `FlatGeneratorInfo.toString` (versão 3).
    pub fn to_text(&self) -> String {
        let layers: Vec<String> = self.layers.iter().map(Layer::to_text).collect();
        format!("3;{};{}", layers.join(","), self.rest)
    }
}

/// `GuiFlatPresets.FLAT_WORLD_PRESETS`: nome (fixo em inglês no jogo, sem tradução), ícone e texto.
pub const FLAT_PRESETS: [(&str, (i16, i16), &str); 8] = [
    ("Classic Flat", (2, 0), "3;minecraft:bedrock,2*minecraft:dirt,minecraft:grass;1;village"),
    (
        "Tunnelers' Dream",
        (1, 0),
        "3;minecraft:bedrock,230*minecraft:stone,5*minecraft:dirt,minecraft:grass;3;biome_1,dungeon,decoration,stronghold,mineshaft",
    ),
    (
        "Water World",
        (326, 0),
        "3;minecraft:bedrock,5*minecraft:stone,5*minecraft:dirt,5*minecraft:sand,90*minecraft:water;24;biome_1,oceanmonument",
    ),
    (
        "Overworld",
        (31, 1),
        "3;minecraft:bedrock,59*minecraft:stone,3*minecraft:dirt,minecraft:grass;1;village,biome_1,decoration,stronghold,mineshaft,dungeon,lake,lava_lake",
    ),
    ("Snowy Kingdom", (78, 0), "3;minecraft:bedrock,59*minecraft:stone,3*minecraft:dirt,minecraft:grass,minecraft:snow_layer;12;village,biome_1"),
    ("Bottomless Pit", (288, 0), "3;2*minecraft:cobblestone,3*minecraft:dirt,minecraft:grass;1;village,biome_1"),
    (
        "Desert",
        (12, 0),
        "3;minecraft:bedrock,3*minecraft:stone,52*minecraft:sandstone,8*minecraft:sand;2;village,biome_1,decoration,stronghold,mineshaft,dungeon",
    ),
    ("Redstone Ready", (331, 0), "3;minecraft:bedrock,3*minecraft:stone,52*minecraft:sandstone;2;"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn superplano_classico() {
        let f = Flat::parse(DEFAULT_FLAT);
        assert_eq!(f.layers.len(), 3);
        assert_eq!(f.layers[1], Layer { count: 2, block: "minecraft:dirt".into(), meta: 0 });
        assert_eq!(f.rest, "1;village");
        assert_eq!(f.to_text(), DEFAULT_FLAT);
    }

    #[test]
    fn predefinicoes_validas_e_iguais_de_volta() {
        for (key, _, text) in FLAT_PRESETS {
            let f = Flat::try_parse(text).unwrap_or_else(|| panic!("{key} inválida"));
            assert_eq!(f.to_text(), text, "{key}");
            assert!(f.layers.iter().all(|l| l.block_id().is_some()), "{key}");
        }
    }

    #[test]
    fn textos_estranhos() {
        // Inválido: volta ao clássico.
        assert_eq!(Flat::parse("lixo").to_text(), DEFAULT_FLAT);
        assert_eq!(Flat::parse("3;minecraft:naoexiste;1;").to_text(), DEFAULT_FLAT);
        assert_eq!(Flat::parse("3;300*minecraft:stone;1;").to_text(), DEFAULT_FLAT);
        // Versão 2 (números e `x`).
        let f = Flat::parse("2;7,2x3,2;1;village");
        assert_eq!(f.to_text(), "3;7,2*3,2;1;village");
        // Com meta (lã laranja).
        let f = Flat::parse("3;minecraft:wool:1;1;");
        assert_eq!(f.layers[0].meta, 1);
        assert_eq!(f.layers[0].icon_item(), Some((35, 1)));
        assert_eq!(Flat::parse("3;minecraft:water;1;").layers[0].icon_item(), Some((326, 0)));
    }

    #[test]
    fn pasta_do_mundo() {
        assert_eq!(folder_for("a/b:c"), "a_b_c");
        assert_eq!(folder_for("con"), "_con_");
        assert_eq!(folder_for("   "), "World");
    }
}
