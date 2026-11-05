//! Pilha de recursos do jogo: `SimpleReloadableResourceManager` + `FallbackResourceManager`,
//! `ResourcePackRepository` e os pacotes (`FileResourcePack`, `FolderResourcePack`,
//! `DefaultResourcePack`) da 1.8.9.
//!
//! A pilha tem o pacote padrão por baixo (jar do jogo + índice de assets do launcher), os pacotes
//! escolhidos pelo jogador acima (na ordem de `resourcePacks` do `options.txt`, o último vence) e o
//! pacote baixado do servidor no topo (`Minecraft.refreshResources`).

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

/// `PackMetadataSection`: `pack_format` e a descrição já com os códigos de formatação.
#[derive(Clone, Debug, PartialEq)]
pub struct PackMeta {
    pub pack_format: i32,
    pub description: String,
}

/// Versão de pacote que a 1.8.9 aceita sem avisar (`ResourcePackRepository.Entry.func_183027_f`).
pub const PACK_FORMAT: i32 = 1;

/// `PackMetadataSection` a partir do `pack.mcmeta`. `Err` = JSON inválido; `Ok(None)` = sem a seção `pack`.
pub fn parse_pack_meta(bytes: &[u8]) -> Result<Option<PackMeta>, String> {
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim_start_matches('\u{feff}');
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let obj = v.as_object().ok_or("pack.mcmeta não é um objeto")?;
    let Some(pack) = obj.get("pack").and_then(|p| p.as_object()) else { return Ok(None) };
    let Some(format) = pack.get("pack_format").and_then(|f| f.as_i64()) else { return Ok(None) };
    let description = pack.get("description").map(component_text).unwrap_or_default();
    Ok(Some(PackMeta { pack_format: format as i32, description }))
}

/// Texto de um componente de chat JSON (`IChatComponent.getFormattedText` sem estilos além dos § do texto).
fn component_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(a) => a.iter().map(component_text).collect(),
        serde_json::Value::Object(o) => {
            let mut s = String::new();
            if let Some(c) = o.get("color").and_then(|c| c.as_str()) {
                if let Some(code) = color_code(c) {
                    s.push('§');
                    s.push(code);
                }
            }
            if let Some(t) = o.get("text").and_then(|t| t.as_str()) {
                s.push_str(t);
            }
            if let Some(extra) = o.get("extra") {
                s.push_str(&component_text(extra));
            }
            s
        }
        _ => String::new(),
    }
}

fn color_code(name: &str) -> Option<char> {
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
        _ => return None,
    })
}

// ---------------------------------------------------------------------------------------------
// Pacotes
// ---------------------------------------------------------------------------------------------

enum Source {
    Zip(zip::ZipArchive<BufReader<File>>),
    Dir(PathBuf),
}

/// Um pacote de recursos em .zip (`FileResourcePack`) ou pasta (`FolderResourcePack`).
pub struct Pack {
    pub name: String,
    pub path: PathBuf,
    src: Source,
}

impl Pack {
    pub fn open(path: &Path) -> Result<Self, String> {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let src = if path.is_dir() {
            Source::Dir(path.to_path_buf())
        } else {
            let f = File::open(path).map_err(|e| format!("não abri {}: {e}", path.display()))?;
            Source::Zip(zip::ZipArchive::new(BufReader::new(f)).map_err(|e| format!("{}: {e}", path.display()))?)
        };
        Ok(Self { name, path: path.to_path_buf(), src })
    }

    /// `getInputStreamByName`: nome relativo à raiz do pacote (`assets/minecraft/...`).
    pub fn bytes(&mut self, name: &str) -> Option<Vec<u8>> {
        match &mut self.src {
            Source::Zip(z) => {
                let mut f = z.by_name(name).ok()?;
                if f.is_dir() {
                    return None;
                }
                let mut buf = Vec::with_capacity(f.size() as usize);
                f.read_to_end(&mut buf).ok()?;
                Some(buf)
            }
            Source::Dir(root) => std::fs::read(safe_join(root, name)?).ok(),
        }
    }

    /// `hasResourceName`.
    pub fn has(&mut self, name: &str) -> bool {
        match &mut self.src {
            Source::Zip(z) => z.by_name(name).is_ok_and(|f| !f.is_dir()),
            Source::Dir(root) => safe_join(root, name).is_some_and(|p| p.is_file()),
        }
    }

    /// `getPackMetadata("pack")`.
    pub fn meta(&mut self) -> Result<Option<PackMeta>, String> {
        let b = self.bytes("pack.mcmeta").ok_or("sem pack.mcmeta")?;
        parse_pack_meta(&b)
    }

    /// `getPackImage`: bytes do `pack.png`.
    pub fn icon(&mut self) -> Option<Vec<u8>> {
        self.bytes("pack.png")
    }
}

/// Junta `name` a `root` sem deixar escapar da pasta do pacote.
fn safe_join(root: &Path, name: &str) -> Option<PathBuf> {
    if name.split(['/', '\\']).any(|c| c == "..") || name.starts_with('/') {
        return None;
    }
    Some(root.join(name))
}

/// `AssetIndex`/`ResourceIndex`: chave (`minecraft/sounds.json`) -> arquivo em `objects/`.
pub struct AssetIndex {
    map: HashMap<String, PathBuf>,
}

/// Nome do índice da 1.8 (`assets/indexes/1.8.json`).
pub const ASSET_INDEX: &str = "1.8";

impl AssetIndex {
    pub fn load(dir: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(dir.join("indexes").join(format!("{ASSET_INDEX}.json"))).ok()?;
        let v: serde_json::Value = serde_json::from_str(&text).ok()?;
        let objects = dir.join("objects");
        let map = v["objects"]
            .as_object()?
            .iter()
            .filter_map(|(k, o)| {
                let h = o["hash"].as_str()?;
                Some((k.clone(), objects.join(h.get(..2)?).join(h)))
            })
            .collect();
        Some(Self { map })
    }

    pub fn file(&self, key: &str) -> Option<&PathBuf> {
        self.map.get(key)
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.map.keys()
    }
}

fn env_dir(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// Pasta do launcher oficial: `~/.minecraft` (no Windows, `%APPDATA%\.minecraft`).
pub fn minecraft_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        env_dir("APPDATA").map(|d| d.join(".minecraft"))
    } else {
        env_dir("HOME").map(|h| h.join(".minecraft"))
    }
}

/// Pasta do Prism Launcher: `~/.local/share/PrismLauncher` (no Windows, `%APPDATA%\PrismLauncher`).
pub fn prism_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        env_dir("APPDATA").map(|d| d.join("PrismLauncher"))
    } else {
        env_dir("HOME").map(|h| h.join(".local/share/PrismLauncher"))
    }
}

/// Pasta de assets do launcher: `MINE_RS_ASSETS`, Prism Launcher e `~/.minecraft/assets`.
pub fn find_assets_dir() -> Option<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(d) = std::env::var_os("MINE_RS_ASSETS") {
        dirs.push(PathBuf::from(d));
    }
    dirs.extend(prism_dir().map(|d| d.join("assets")));
    dirs.extend(minecraft_dir().map(|d| d.join("assets")));
    dirs.into_iter().find(|d| d.join("indexes").join(format!("{ASSET_INDEX}.json")).is_file())
}

/// `DefaultResourcePack`: o jar do jogo e, para o que não está nele (sons, idiomas), o índice de assets.
pub struct Base {
    jar: Option<zip::ZipArchive<BufReader<File>>>,
    index: Option<AssetIndex>,
}

impl Base {
    fn bytes(&mut self, name: &str) -> Option<Vec<u8>> {
        if let Some(z) = &mut self.jar {
            if let Ok(mut f) = z.by_name(name) {
                if !f.is_dir() {
                    let mut buf = Vec::with_capacity(f.size() as usize);
                    if f.read_to_end(&mut buf).is_ok() {
                        return Some(buf);
                    }
                }
            }
        }
        let key = name.strip_prefix("assets/").unwrap_or(name);
        let path = self.index.as_ref()?.file(key)?;
        std::fs::read(path).ok()
    }

    fn has(&mut self, name: &str) -> bool {
        if self.jar.as_mut().is_some_and(|z| z.by_name(name).is_ok_and(|f| !f.is_dir())) {
            return true;
        }
        let key = name.strip_prefix("assets/").unwrap_or(name);
        self.index.as_ref().is_some_and(|i| i.file(key).is_some_and(|p| p.is_file()))
    }
}

enum Layer {
    Base(Base),
    Pack(Pack),
}

impl Layer {
    fn bytes(&mut self, name: &str) -> Option<Vec<u8>> {
        match self {
            Layer::Base(b) => b.bytes(name),
            Layer::Pack(p) => p.bytes(name),
        }
    }

    fn has(&mut self, name: &str) -> bool {
        match self {
            Layer::Base(b) => b.has(name),
            Layer::Pack(p) => p.has(name),
        }
    }

    fn name(&self) -> String {
        match self {
            Layer::Base(_) => "Default".to_string(),
            Layer::Pack(p) => p.name.clone(),
        }
    }
}

/// A pilha: índice 0 = pacote padrão; o último vence (`FallbackResourceManager.getResource`).
pub struct Stack {
    layers: Vec<Layer>,
    /// Sobe a cada `set_packs`: quem guarda cópias (atlas, ícones, texturas) sabe que precisa recarregar.
    pub generation: u32,
}

impl Stack {
    pub fn bytes(&mut self, name: &str) -> Option<Vec<u8>> {
        self.layers.iter_mut().rev().find_map(|l| l.bytes(name))
    }

    pub fn has(&mut self, name: &str) -> bool {
        self.layers.iter_mut().rev().any(|l| l.has(name))
    }

    /// Nome do pacote que fornece `name` (o mais alto que o tem).
    pub fn provider(&mut self, name: &str) -> Option<String> {
        self.layers.iter_mut().rev().find_map(|l| l.has(name).then(|| l.name()))
    }

    /// Recurso e o `.mcmeta` que o acompanha, como `FallbackResourceManager.getResource`: o
    /// `.mcmeta` vem do pacote mais alto que o tem, entre o topo e o pacote que fornece o recurso.
    pub fn resource(&mut self, name: &str) -> Option<(Vec<u8>, Option<Vec<u8>>)> {
        let meta_name = format!("{name}.mcmeta");
        let mut meta_layer: Option<usize> = None;
        for i in (0..self.layers.len()).rev() {
            if meta_layer.is_none() && self.layers[i].has(&meta_name) {
                meta_layer = Some(i);
            }
            if self.layers[i].has(name) {
                let data = self.layers[i].bytes(name)?;
                let meta = meta_layer.and_then(|m| self.layers[m].bytes(&meta_name));
                return Some((data, meta));
            }
        }
        None
    }

    /// Todos os recursos de `name`, do pacote mais baixo ao mais alto (`getAllResources`).
    pub fn all(&mut self, name: &str) -> Vec<Vec<u8>> {
        self.layers.iter_mut().filter_map(|l| if l.has(name) { l.bytes(name) } else { None }).collect()
    }

    /// Nomes dos pacotes acima do padrão, de baixo para cima.
    pub fn pack_names(&self) -> Vec<String> {
        self.layers.iter().skip(1).map(Layer::name).collect()
    }

    pub fn len(&self) -> usize {
        self.layers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }
}

/// Acesso compartilhado à pilha. `Clone` entrega outro punho para a mesma pilha (som, ícones, skins).
#[derive(Clone)]
pub struct Jar {
    stack: Arc<Mutex<Stack>>,
}

impl Jar {
    /// Abre o jar do jogo como pacote padrão, com o índice de assets do launcher (se achar).
    pub fn open(path: &Path) -> Result<Self, String> {
        let file = File::open(path).map_err(|e| format!("não abri {}: {e}", path.display()))?;
        let zip = zip::ZipArchive::new(BufReader::new(file)).map_err(|e| e.to_string())?;
        let index = find_assets_dir().and_then(|d| AssetIndex::load(&d));
        let base = Base { jar: Some(zip), index };
        Ok(Self { stack: Arc::new(Mutex::new(Stack { layers: vec![Layer::Base(base)], generation: 0 })) })
    }

    fn lock(&self) -> MutexGuard<'_, Stack> {
        self.stack.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Bytes de `assets/minecraft/...` vindos do pacote mais alto que o tem.
    pub fn bytes(&mut self, path: &str) -> Option<Vec<u8>> {
        self.lock().bytes(path)
    }

    pub fn has(&mut self, path: &str) -> bool {
        self.lock().has(path)
    }

    pub fn provider(&mut self, path: &str) -> Option<String> {
        self.lock().provider(path)
    }

    /// Recurso e `.mcmeta` (ver `Stack::resource`).
    pub fn resource(&mut self, path: &str) -> Option<(Vec<u8>, Option<Vec<u8>>)> {
        self.lock().resource(path)
    }

    /// `.mcmeta` de `textures/<path>.png` (ex.: animação), do pacote que vale para a textura.
    pub fn texture_mcmeta(&mut self, path: &str) -> Option<Vec<u8>> {
        self.lock().resource(&format!("assets/minecraft/textures/{path}.png"))?.1
    }

    pub fn all(&mut self, path: &str) -> Vec<Vec<u8>> {
        self.lock().all(path)
    }

    /// `textures/...` sem o prefixo `assets/minecraft/` e sem `.png`.
    pub fn texture(&mut self, path: &str) -> Option<image::RgbaImage> {
        let buf = self.bytes(&format!("assets/minecraft/textures/{path}.png"))?;
        Some(image::load_from_memory_with_format(&buf, image::ImageFormat::Png).ok()?.to_rgba8())
    }

    /// Troca os pacotes acima do padrão (`Minecraft.refreshResources`): `paths` de baixo para cima.
    /// Pacotes que não abrem são ignorados (e listados no retorno).
    pub fn set_packs(&self, paths: &[PathBuf]) -> Vec<String> {
        let mut failed = Vec::new();
        let mut st = self.lock();
        st.layers.truncate(1);
        for p in paths {
            match Pack::open(p) {
                Ok(pack) => st.layers.push(Layer::Pack(pack)),
                Err(e) => failed.push(e),
            }
        }
        st.generation = st.generation.wrapping_add(1);
        failed
    }

    pub fn pack_names(&self) -> Vec<String> {
        self.lock().pack_names()
    }

    pub fn generation(&self) -> u32 {
        self.lock().generation
    }

    /// Pilha só com pacotes (sem jar), para testes e ferramentas.
    pub fn from_packs(paths: &[PathBuf]) -> Self {
        let base = Base { jar: None, index: None };
        let jar = Self { stack: Arc::new(Mutex::new(Stack { layers: vec![Layer::Base(base)], generation: 0 })) };
        jar.set_packs(paths);
        jar
    }

    /// Arquivo do pacote padrão, ignorando os pacotes de cima (`pack.png`, `pack.mcmeta`).
    pub fn default_bytes(&mut self, name: &str) -> Option<Vec<u8>> {
        self.lock().layers.first_mut()?.bytes(name)
    }

    /// Imagem do pacote padrão (`pack.png` do jar).
    pub fn default_icon(&mut self) -> Option<Vec<u8>> {
        self.default_bytes("pack.png")
    }

    /// `pack.mcmeta` do pacote padrão (`DefaultResourcePack.getPackMetadata`).
    pub fn default_meta(&mut self) -> Option<PackMeta> {
        parse_pack_meta(&self.default_bytes("pack.mcmeta")?).ok().flatten()
    }
}

// ---------------------------------------------------------------------------------------------
// Repositório (ResourcePackRepository)
// ---------------------------------------------------------------------------------------------

/// `ResourcePackRepository.Entry`.
#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    /// `File.lastModified()` em milissegundos.
    pub modified: u64,
    pub meta: Option<PackMeta>,
    /// PNG do `pack.png` (ou o do pacote padrão).
    pub icon: Option<Vec<u8>>,
}

impl Entry {
    /// `Entry.toString()`: identidade usada em `equals`.
    pub fn id(&self) -> String {
        format!("{}:{}:{}", self.name, if self.is_dir { "folder" } else { "zip" }, self.modified)
    }

    /// `func_183027_f`: `pack_format` (0 sem a seção `pack`).
    pub fn format(&self) -> i32 {
        self.meta.as_ref().map_or(0, |m| m.pack_format)
    }

    /// `getTexturePackDescription`.
    pub fn description(&self) -> String {
        match &self.meta {
            Some(m) => m.description.clone(),
            None => "§cInvalid pack.mcmeta (or missing 'pack' section)".to_string(),
        }
    }
}

/// `resourcePackFilter`: .zip ou pasta com `pack.mcmeta`.
pub fn is_pack_file(path: &Path) -> bool {
    let zip = path.is_file() && path.file_name().is_some_and(|n| n.to_string_lossy().ends_with(".zip"));
    let dir = path.is_dir() && path.join("pack.mcmeta").is_file();
    zip || dir
}

/// `Entry.updateResourcePack`: `None` quando o `pack.mcmeta` falta ou é JSON inválido (o Java
/// descarta a entrada nesse caso).
pub fn read_entry(path: &Path, default_icon: Option<&[u8]>) -> Option<Entry> {
    let mut pack = Pack::open(path).ok()?;
    let meta = pack.meta().ok()?;
    let icon = pack.icon().or_else(|| default_icon.map(<[u8]>::to_vec));
    let modified = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as u64);
    Some(Entry {
        name: pack.name.clone(),
        path: path.to_path_buf(),
        is_dir: path.is_dir(),
        modified,
        meta,
        icon,
    })
}

/// `updateRepositoryEntriesAll`: todos os pacotes da pasta, em ordem de nome.
pub fn scan(dir: &Path, default_icon: Option<&[u8]>) -> Vec<Entry> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut files: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| is_pack_file(p)).collect();
    files.sort();
    files.iter().filter_map(|p| read_entry(p, default_icon)).collect()
}

/// Construtor do `ResourcePackRepository`: das `resourcePacks` do options, os que existem e são
/// compatíveis (ou já marcados como incompatíveis aceitos). Devolve os nomes mantidos, na ordem.
pub fn resolve_selected(all: &[Entry], names: &[String], incompatible: &[String]) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    for n in names {
        if let Some(e) = all.iter().find(|e| &e.name == n) {
            if e.format() == PACK_FORMAT || incompatible.contains(&e.name) {
                out.push(e.clone());
            }
        }
    }
    out
}

/// Pasta `resourcepacks` do jogo (`gameDir/resourcepacks`).
pub fn resourcepacks_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("MINE_RS_RESOURCEPACKS") {
        return PathBuf::from(d);
    }
    minecraft_dir().unwrap_or_else(|| PathBuf::from(".minecraft")).join("resourcepacks")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("mine-rs-res-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn make_zip(path: &Path, files: &[(&str, &[u8])]) {
        let mut z = zip::ZipWriter::new(File::create(path).unwrap());
        for (n, b) in files {
            z.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(b).unwrap();
        }
        z.finish().unwrap();
    }

    const META: &[u8] = br#"{"pack":{"pack_format":1,"description":"teste"}}"#;

    #[test]
    fn pack_meta_parsing() {
        let m = parse_pack_meta(META).unwrap().unwrap();
        assert_eq!((m.pack_format, m.description.as_str()), (1, "teste"));
        let m = parse_pack_meta("\u{feff}{\"pack\":{\"pack_format\":2,\"description\":{\"text\":\"a\",\"color\":\"red\",\"extra\":[\"b\"]}}}".as_bytes()).unwrap().unwrap();
        assert_eq!((m.pack_format, m.description.as_str()), (2, "§cab"));
        assert_eq!(parse_pack_meta(br#"{"x":1}"#).unwrap(), None);
        assert!(parse_pack_meta(b"{nope").is_err());
    }

    #[test]
    fn stack_order_and_override() {
        let d = tmp("ordem");
        let a = d.join("a.zip");
        make_zip(&a, &[("pack.mcmeta", META), ("assets/minecraft/textures/blocks/stone.png", b"A"), ("assets/minecraft/lang/x.lang", b"A")]);
        // Pasta: sobrescreve a pedra e traz uma textura nova.
        let b = d.join("b");
        std::fs::create_dir_all(b.join("assets/minecraft/textures/blocks")).unwrap();
        std::fs::write(b.join("pack.mcmeta"), META).unwrap();
        std::fs::write(b.join("assets/minecraft/textures/blocks/stone.png"), b"B").unwrap();
        std::fs::write(b.join("assets/minecraft/textures/blocks/dirt.png"), b"B").unwrap();
        let mut jar = Jar::from_packs(&[a.clone(), b.clone()]);
        let stone = "assets/minecraft/textures/blocks/stone.png";
        assert_eq!(jar.bytes(stone).unwrap(), b"B");
        assert_eq!(jar.provider(stone).as_deref(), Some("b"));
        assert_eq!(jar.bytes("assets/minecraft/lang/x.lang").unwrap(), b"A");
        assert_eq!(jar.all(stone), vec![b"A".to_vec(), b"B".to_vec()]);
        // Ordem invertida: o zip passa a vencer.
        jar.set_packs(&[b, a]);
        assert_eq!(jar.bytes(stone).unwrap(), b"A");
        assert_eq!(jar.pack_names(), vec!["b".to_string(), "a.zip".to_string()]);
        assert!(jar.bytes("assets/minecraft/nada.png").is_none());
        // Sem pacotes: volta ao padrão (aqui vazio).
        jar.set_packs(&[]);
        assert!(jar.bytes(stone).is_none());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn mcmeta_comes_from_topmost_pack_not_below_provider() {
        let d = tmp("mcmeta");
        let tex = "assets/minecraft/textures/blocks/water.png";
        let a = d.join("a.zip");
        make_zip(&a, &[("pack.mcmeta", META), (tex, b"A"), (&format!("{tex}.mcmeta"), b"META-A")]);
        let b = d.join("b.zip");
        make_zip(&b, &[("pack.mcmeta", META), (tex, b"B")]);
        let mut jar = Jar::from_packs(&[a.clone(), b.clone()]);
        // O pacote de cima fornece a imagem; o .mcmeta de baixo não vale (está abaixo do fornecedor).
        let (img, meta) = jar.resource(tex).unwrap();
        assert_eq!((img.as_slice(), meta), (b"B".as_slice(), None));
        // Se o de cima não tem imagem mas tem .mcmeta, ele vale sobre a imagem de baixo.
        let c = d.join("c.zip");
        make_zip(&c, &[("pack.mcmeta", META), (&format!("{tex}.mcmeta"), b"META-C")]);
        jar.set_packs(&[a, c]);
        let (img, meta) = jar.resource(tex).unwrap();
        assert_eq!((img.as_slice(), meta.as_deref()), (b"A".as_slice(), Some(b"META-C".as_slice())));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn folder_pack_blocks_path_escape() {
        let d = tmp("escape");
        std::fs::write(d.join("segredo.txt"), b"x").unwrap();
        let p = d.join("p");
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(p.join("pack.mcmeta"), META).unwrap();
        let mut pack = Pack::open(&p).unwrap();
        assert!(pack.bytes("../segredo.txt").is_none());
        assert!(!pack.has("../segredo.txt"));
        assert!(pack.has("pack.mcmeta"));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn repository_scan_and_selection() {
        let d = tmp("repo");
        make_zip(&d.join("bom.zip"), &[("pack.mcmeta", META), ("pack.png", b"PNG")]);
        make_zip(&d.join("novo.zip"), &[("pack.mcmeta", br#"{"pack":{"pack_format":3,"description":"n"}}"#)]);
        make_zip(&d.join("quebrado.zip"), &[("pack.mcmeta", b"{nao json")]);
        make_zip(&d.join("semmeta.zip"), &[("x.txt", b"x")]);
        std::fs::create_dir_all(d.join("pasta")).unwrap();
        std::fs::write(d.join("pasta/pack.mcmeta"), META).unwrap();
        std::fs::create_dir_all(d.join("lixo")).unwrap(); // pasta sem pack.mcmeta
        std::fs::write(d.join("notas.txt"), b"x").unwrap();
        let all = scan(&d, Some(b"PADRAO"));
        let names: Vec<_> = all.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["bom.zip", "novo.zip", "pasta"]);
        assert_eq!(all[0].icon.as_deref(), Some(b"PNG".as_slice()));
        assert_eq!(all[2].icon.as_deref(), Some(b"PADRAO".as_slice()));
        assert!(all[2].is_dir && !all[0].is_dir);
        // O pack de formato 3 só entra se estiver marcado como incompatível aceito.
        let want = vec!["novo.zip".to_string(), "bom.zip".to_string(), "sumiu.zip".to_string()];
        let sel = resolve_selected(&all, &want, &[]);
        assert_eq!(sel.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), vec!["bom.zip"]);
        let sel = resolve_selected(&all, &want, &["novo.zip".to_string()]);
        assert_eq!(sel.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), vec!["novo.zip", "bom.zip"]);
        let _ = std::fs::remove_dir_all(d);
    }
}
