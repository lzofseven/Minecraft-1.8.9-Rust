//! Texturas das entidades: as do jar (mobs, skins padrão) e as skins baixadas de
//! textures.minecraft.net numa thread própria (como o `ThreadDownloadImageData`).

use std::collections::HashMap;
use std::io::Read;
use std::sync::mpsc;
use std::time::Duration;

use image::RgbaImage;
use mc_render::{Jar, Renderer};
use serde_json::Value;

use crate::entity_net::{ProfileProperty, Uuid};

/// Skin de um perfil: URL (vazia sem skin própria), se o modelo é o de braço fino ("slim") e a capa.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkinRef {
    pub url: String,
    pub slim: bool,
    /// `MinecraftProfileTexture.Type.CAPE`.
    pub cape: Option<String>,
}

/// Só baixa do servidor de texturas da Mojang (a URL vem do servidor do jogo).
fn allowed(url: &str) -> bool {
    ["http://textures.minecraft.net/", "https://textures.minecraft.net/"].iter().any(|p| url.starts_with(p))
}

/// Base64 padrão (com ou sem `=`); ignora espaços e quebras de linha.
pub fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        } as u32)
    };
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in s.bytes().filter(|c| !c.is_ascii_whitespace()) {
        if c == b'=' {
            break;
        }
        acc = (acc << 6) | val(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// Lê a propriedade `textures` (base64 de um JSON) de um perfil.
pub fn skin_from_properties(props: &[ProfileProperty]) -> Option<SkinRef> {
    let p = props.iter().find(|p| p.name == "textures")?;
    parse_textures(&p.value)
}

pub fn parse_textures(b64: &str) -> Option<SkinRef> {
    let json = base64_decode(b64)?;
    let v: Value = serde_json::from_slice(&json).ok()?;
    let tex = v.get("textures")?;
    let url_of = |k: &str| tex.get(k).and_then(|t| t.get("url")).and_then(Value::as_str).filter(|u| allowed(u)).map(str::to_string);
    let (url, cape) = (url_of("SKIN"), url_of("CAPE"));
    if url.is_none() && cape.is_none() {
        return None;
    }
    let slim = tex.get("SKIN").and_then(|s| s.get("metadata")).and_then(|m| m.get("model")).and_then(Value::as_str) == Some("slim");
    Some(SkinRef { url: url.unwrap_or_default(), slim, cape })
}

/// Skin padrão pelo UUID (`DefaultPlayerSkin`): hash ímpar = Alex (braço fino).
pub fn default_skin(uuid: Uuid) -> (&'static str, bool) {
    if uuid.java_hash() & 1 == 1 {
        ("entity/alex", true)
    } else {
        ("entity/steve", false)
    }
}

/// Copia um retângulo `w x h` de `(sx, sy)` para `(dx, dy)`, espelhando na horizontal se `flip`.
fn blit(img: &mut RgbaImage, sx: u32, sy: u32, dx: u32, dy: u32, w: u32, h: u32, flip: bool) {
    for y in 0..h {
        for x in 0..w {
            let p = *img.get_pixel(sx + x, sy + y);
            let tx = if flip { dx + w - 1 - x } else { dx + x };
            img.put_pixel(tx, dy + y, p);
        }
    }
}

/// `ImageBufferDownload.parseUserSkin`: skins antigas 64x32 viram 64x64 (perna e braço
/// esquerdos espelhados dos direitos) e as áreas de base ficam opacas / camadas sem fundo.
pub fn fix_skin(src: &RgbaImage) -> Option<RgbaImage> {
    if src.width() != 64 || !(src.height() == 32 || src.height() == 64) {
        return None;
    }
    let mut img = RgbaImage::new(64, 64);
    for (x, y, p) in src.enumerate_pixels() {
        img.put_pixel(x, y, *p);
    }
    if src.height() == 32 {
        // (destino x, y, origem x, y, largura, altura), todos espelhados.
        for (dx, dy, sx, sy, w, h) in [
            (20, 48, 4, 16, 4, 4),
            (24, 48, 8, 16, 4, 4),
            (16, 52, 8, 20, 4, 12),
            (20, 52, 4, 20, 4, 12),
            (24, 52, 0, 20, 4, 12),
            (28, 52, 12, 20, 4, 12),
            (36, 48, 44, 16, 4, 4),
            (40, 48, 48, 16, 4, 4),
            (32, 52, 48, 20, 4, 12),
            (36, 52, 44, 20, 4, 12),
            (40, 52, 40, 20, 4, 12),
            (44, 52, 52, 20, 4, 12),
        ] {
            blit(&mut img, sx, sy, dx, dy, w, h, true);
        }
    }
    let area = |img: &mut RgbaImage, x0: u32, y0: u32, x1: u32, y1: u32, opaque: bool| {
        if !opaque && (x0..x1).any(|x| (y0..y1).any(|y| img.get_pixel(x, y).0[3] < 128)) {
            return;
        }
        for x in x0..x1 {
            for y in y0..y1 {
                img.get_pixel_mut(x, y).0[3] = if opaque { 255 } else { 0 };
            }
        }
    };
    area(&mut img, 0, 0, 32, 16, true);
    area(&mut img, 32, 0, 64, 32, false);
    area(&mut img, 0, 16, 64, 32, true);
    area(&mut img, 0, 32, 16, 48, false);
    area(&mut img, 16, 32, 40, 48, false);
    area(&mut img, 40, 32, 56, 48, false);
    area(&mut img, 0, 48, 16, 64, false);
    area(&mut img, 16, 48, 48, 64, true);
    area(&mut img, 48, 48, 64, 64, false);
    Some(img)
}

/// Pasta do cache de skins (`~/.cache/mine-rs/skins`), como o `assets/skins` do `SkinManager`.
pub fn cache_dir() -> std::path::PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".cache"));
    base.join("mine-rs").join("skins")
}

/// Arquivo do cache: `<2 primeiros>/<hash>` (o hash é o fim da URL da textura).
pub fn cache_path(dir: &std::path::Path, url: &str) -> Option<std::path::PathBuf> {
    let hash = url.rsplit('/').next().filter(|h| !h.is_empty() && h.chars().all(|c| c.is_ascii_alphanumeric()))?;
    Some(dir.join(&hash[..2.min(hash.len())]).join(hash))
}

/// `raw`: a capa vai como veio (o `ImageBufferDownload` de 64x64 é só das skins).
fn decode(bytes: &[u8], raw: bool) -> Option<RgbaImage> {
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok()?.to_rgba8();
    if raw { Some(img) } else { fix_skin(&img) }
}

/// Baixa a skin e grava os bytes originais no cache.
fn download(url: &str, dir: &std::path::Path, raw: bool) -> Option<RgbaImage> {
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(15)).build();
    let resp = agent.get(url).call().ok()?;
    let mut buf = Vec::new();
    resp.into_reader().take(1 << 20).read_to_end(&mut buf).ok()?;
    let img = decode(&buf, raw)?;
    if let Some(path) = cache_path(dir, url) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, &buf);
    }
    Some(img)
}

/// Perfil no sessionserver (`MinecraftSessionService.fillProfileProperties`): a skin do próprio jogador
/// quando o servidor não manda as propriedades.
fn fetch_profile(uuid: &str) -> Option<SkinRef> {
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(15)).build();
    let url = format!("https://sessionserver.mojang.com/session/minecraft/profile/{uuid}");
    let v: Value = agent.get(&url).call().ok()?.into_json().ok()?;
    let props = v.get("properties")?.as_array()?;
    let tex = props.iter().find(|p| p.get("name").and_then(Value::as_str) == Some("textures"))?;
    parse_textures(tex.get("value")?.as_str()?)
}

enum SkinState {
    Loading,
    Ready(u32),
    Failed,
}

enum Job {
    Skin(String, bool),
    Profile(String),
}

enum Done {
    Skin(String, Option<RgbaImage>),
    Profile(String, Option<SkinRef>),
}

/// Downloads em paralelo, como os `ThreadDownloadImageData` (limitados a este número).
const WORKERS: usize = 4;

/// Cache das texturas de entidade na GPU.
pub struct Textures {
    jar: HashMap<String, Option<u32>>,
    skins: HashMap<String, SkinState>,
    profiles: HashMap<String, Option<Option<SkinRef>>>,
    requests: Option<mpsc::Sender<Job>>,
    done_tx: mpsc::Sender<Done>,
    done_rx: mpsc::Receiver<Done>,
    /// Lidas do cache em disco, esperando subir para a GPU no próximo `poll`.
    cached: Vec<(String, RgbaImage)>,
    /// Cópia na CPU das skins já na GPU (ícones de cabeça rasterizados por software).
    images: HashMap<String, RgbaImage>,
    /// Texturas dos estandartes por desenho (tile entities e itens).
    pub banners: crate::banner::BannerTextures,
    dir: std::path::PathBuf,
    white: Option<u32>,
}

impl Default for Textures {
    fn default() -> Self {
        let (done_tx, done_rx) = mpsc::channel();
        Self {
            jar: HashMap::new(),
            skins: HashMap::new(),
            profiles: HashMap::new(),
            requests: None,
            done_tx,
            done_rx,
            cached: Vec::new(),
            images: HashMap::new(),
            banners: Default::default(),
            dir: cache_dir(),
            white: None,
        }
    }
}

/// `"texture": {"clamp": true}` do .mcmeta: `TextureMetadataSection.getTextureClamp` (`misc/shadow`).
pub fn texture_clamp(meta: &[u8]) -> bool {
    serde_json::from_slice::<Value>(meta).ok().and_then(|v| v["texture"]["clamp"].as_bool()).unwrap_or(false)
}

impl Textures {
    /// Textura do jar (`entity/zombie/zombie`...), carregada uma vez.
    pub fn jar(&mut self, r: &mut Renderer, jar: &mut Jar, path: &str) -> Option<u32> {
        if let Some(t) = self.jar.get(path) {
            return *t;
        }
        let clamp = jar.texture_mcmeta(path).is_some_and(|m| texture_clamp(&m));
        let id = jar.texture(path).map(|img| {
            // Skins padrão antigas (64x32) passam pela mesma conversão das baixadas.
            let img = if img.width() == 64 && img.height() == 32 && path.starts_with("entity/") && (path.ends_with("steve") || path.ends_with("alex")) {
                fix_skin(&img).unwrap_or(img)
            } else {
                img
            };
            r.create_texture_clamp(img.width(), img.height(), img.as_raw(), clamp)
        });
        if id.is_none() {
            eprintln!("textura de entidade não encontrada no jar: {path}");
        }
        self.jar.insert(path.to_string(), id);
        id
    }

    /// Recarga de recursos: solta as texturas lidas dos pacotes (mobs, baús, estandartes) para
    /// serem lidas de novo na próxima vez que forem usadas. Skins baixadas ficam.
    pub fn reload_jar(&mut self, r: &mut Renderer) {
        for id in self.jar.values().flatten().chain(self.banners.ids().iter()) {
            r.remove_texture(*id);
        }
        self.jar.clear();
        self.banners = Default::default();
    }

    /// Imagem de uma textura do jar na CPU, com a mesma conversão das skins padrão de `jar`.
    pub fn jar_image(&self, jar: &mut Jar, path: &str) -> Option<RgbaImage> {
        let img = jar.texture(path)?;
        let old_skin = img.width() == 64 && img.height() == 32 && path.starts_with("entity/") && (path.ends_with("steve") || path.ends_with("alex"));
        Some(if old_skin { fix_skin(&img).unwrap_or(img) } else { img })
    }

    /// Skin baixada vista da CPU (`None` enquanto não chegou).
    pub fn skin_image(&self, url: &str) -> Option<&RgbaImage> {
        self.images.get(url)
    }

    /// Pixel branco para caixas coloridas.
    pub fn white(&mut self, r: &mut Renderer) -> u32 {
        *self.white.get_or_insert_with(|| r.create_texture(1, 1, &[255; 4]))
    }

    fn send(&mut self, job: Job) -> bool {
        let done = self.done_tx.clone();
        let dir = self.dir.clone();
        let tx = self.requests.get_or_insert_with(|| {
            let (tx, rx) = mpsc::channel::<Job>();
            let rx = std::sync::Arc::new(std::sync::Mutex::new(rx));
            for i in 0..WORKERS {
                let (rx, done, dir) = (rx.clone(), done.clone(), dir.clone());
                let _ = std::thread::Builder::new().name(format!("skins-{i}")).spawn(move || loop {
                    let job = match rx.lock() {
                        Ok(r) => r.recv(),
                        Err(_) => break,
                    };
                    let msg = match job {
                        Ok(Job::Skin(url, raw)) => {
                            let img = download(&url, &dir, raw);
                            Done::Skin(url, img)
                        }
                        Ok(Job::Profile(uuid)) => {
                            let skin = fetch_profile(&uuid);
                            Done::Profile(uuid, skin)
                        }
                        Err(_) => break,
                    };
                    if done.send(msg).is_err() {
                        break;
                    }
                });
            }
            tx
        });
        tx.send(job).is_ok()
    }

    /// Skin baixada; `None` enquanto não chegou (ou se falhou). O cache em disco é usado na hora.
    pub fn skin(&mut self, url: &str) -> Option<u32> {
        self.download_texture(url, false)
    }

    /// Capa baixada (`getLocationCape`), sem a conversão de skin.
    pub fn cape(&mut self, url: &str) -> Option<u32> {
        self.download_texture(url, true)
    }

    fn download_texture(&mut self, url: &str, raw: bool) -> Option<u32> {
        match self.skins.get(url) {
            Some(SkinState::Ready(id)) => return Some(*id),
            Some(_) => return None,
            None => {}
        }
        if !allowed(url) {
            self.skins.insert(url.to_string(), SkinState::Failed);
            return None;
        }
        if let Some(img) = cache_path(&self.dir, url).and_then(|p| std::fs::read(p).ok()).and_then(|b| decode(&b, raw)) {
            self.cached.push((url.to_string(), img));
            self.skins.insert(url.to_string(), SkinState::Loading);
            return None;
        }
        let state = if self.send(Job::Skin(url.to_string(), raw)) { SkinState::Loading } else { SkinState::Failed };
        self.skins.insert(url.to_string(), state);
        None
    }

    /// Skin de um perfil pelo sessionserver (UUID sem hífens); `None` enquanto não respondeu.
    pub fn profile(&mut self, uuid: &str) -> Option<SkinRef> {
        match self.profiles.get(uuid) {
            Some(Some(s)) => return s.clone(),
            Some(None) => return None,
            None => {}
        }
        let sent = self.send(Job::Profile(uuid.to_string()));
        self.profiles.insert(uuid.to_string(), if sent { None } else { Some(None) });
        None
    }

    /// Sobe para a GPU as skins que terminaram de baixar ou saíram do cache.
    pub fn poll(&mut self, r: &mut Renderer) {
        for (url, img) in self.cached.drain(..) {
            self.images.insert(url.clone(), img.clone());
            self.skins.insert(url, SkinState::Ready(r.create_texture(img.width(), img.height(), img.as_raw())));
        }
        while let Ok(done) = self.done_rx.try_recv() {
            match done {
                Done::Skin(url, img) => {
                    let state = match img {
                        Some(img) => {
                            self.images.insert(url.clone(), img.clone());
                            SkinState::Ready(r.create_texture(img.width(), img.height(), img.as_raw()))
                        }
                        None => {
                            eprintln!("skin não baixou: {url}");
                            SkinState::Failed
                        }
                    };
                    self.skins.insert(url, state);
                }
                Done::Profile(uuid, skin) => {
                    self.profiles.insert(uuid, Some(skin));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b64(data: &[u8]) -> String {
        const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut s = String::new();
        for c in data.chunks(3) {
            let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
            for i in 0..4 {
                if i <= c.len() {
                    s.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
                } else {
                    s.push('=');
                }
            }
        }
        s
    }

    #[test]
    fn base64_roundtrip() {
        assert_eq!(base64_decode("TWFu").unwrap(), b"Man");
        assert_eq!(base64_decode("TWE=").unwrap(), b"Ma");
        assert_eq!(base64_decode("TQ==").unwrap(), b"M");
        let data: Vec<u8> = (0..=255).collect();
        assert_eq!(base64_decode(&b64(&data)).unwrap(), data);
        assert!(base64_decode("@@").is_none());
    }

    #[test]
    fn textures_property() {
        let json = r#"{"timestamp":1,"profileName":"x","textures":{"SKIN":{"url":"http://textures.minecraft.net/texture/abc","metadata":{"model":"slim"}}}}"#;
        let props = vec![ProfileProperty { name: "textures".into(), value: b64(json.as_bytes()), signature: None }];
        assert_eq!(skin_from_properties(&props), Some(SkinRef { url: "http://textures.minecraft.net/texture/abc".into(), slim: true, cape: None }));
        let json = r#"{"textures":{"SKIN":{"url":"https://textures.minecraft.net/texture/def"}}}"#;
        assert_eq!(parse_textures(&b64(json.as_bytes())).unwrap().slim, false);
        // Fora do servidor de texturas: ignora.
        let json = r#"{"textures":{"SKIN":{"url":"http://exemplo.com/skin.png"}}}"#;
        assert!(parse_textures(&b64(json.as_bytes())).is_none());
    }

    #[test]
    fn legacy_skin_becomes_64x64() {
        let mut old = RgbaImage::from_pixel(64, 32, image::Rgba([10, 20, 30, 255]));
        // Perna direita, face da frente (x 4..8, y 20..32): coluna 4 vermelha.
        for y in 20..32 {
            old.put_pixel(4, y, image::Rgba([255, 0, 0, 255]));
        }
        let new = fix_skin(&old).unwrap();
        assert_eq!((new.width(), new.height()), (64, 64));
        // Vai para a perna esquerda (x 20..24, y 52..64) espelhada: coluna 23.
        assert_eq!(new.get_pixel(23, 52).0, [255, 0, 0, 255]);
        assert_eq!(new.get_pixel(20, 52).0[3], 255);
        // Base opaca, camada do chapéu (toda opaca no original) vira transparente.
        assert_eq!(new.get_pixel(0, 0).0[3], 255);
        assert_eq!(new.get_pixel(40, 8).0[3], 0);
        assert!(fix_skin(&RgbaImage::new(32, 32)).is_none());
    }

    #[test]
    fn cache_por_hash() {
        let dir = std::path::Path::new("/tmp/c");
        assert_eq!(cache_path(dir, "http://textures.minecraft.net/texture/abc123").unwrap(), dir.join("ab").join("abc123"));
        assert!(cache_path(dir, "http://textures.minecraft.net/texture/").is_none());
        assert!(cache_path(dir, "http://textures.minecraft.net/texture/a.b").is_none());
        // Um PNG gravado no cache volta igual (e passa pela conversão 64x64).
        let img = RgbaImage::from_pixel(64, 64, image::Rgba([1, 2, 3, 255]));
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(img).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        assert_eq!(decode(&png, false).unwrap().get_pixel(0, 0).0, [1, 2, 3, 255]);
    }

    #[test]
    fn default_skin_by_uuid() {
        assert_eq!(default_skin(Uuid(1)), ("entity/alex", true));
        assert_eq!(default_skin(Uuid(2)), ("entity/steve", false));
    }
}

#[cfg(test)]
mod clamp_tests {
    use super::texture_clamp;

    #[test]
    fn mcmeta_da_sombra_pede_clamp() {
        // misc/shadow.png.mcmeta do 1.8.9.
        assert!(texture_clamp(br#"{ "texture": { "clamp": true } }"#));
        assert!(!texture_clamp(br#"{ "texture": { "blur": true } }"#));
        assert!(!texture_clamp(b"nao e json"));
    }
}
