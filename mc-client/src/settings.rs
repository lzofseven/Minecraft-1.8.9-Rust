//! Porte do `GameSettings` da 1.8.9: o `options.txt`, as opções das telas e as teclas.
//!
//! O arquivo é lido e gravado no mesmo formato do Java (mesma ordem, `true`/`false`, floats
//! como o `Float.toString`). Linhas que o Java não conhece (de mods) são guardadas e regravadas
//! no fim para não perder nada da cópia do dono.

use std::path::{Path, PathBuf};

use winit::keyboard::KeyCode;

use crate::lang::{tr, trf};

/// `GameSettings.Options`, na mesma ordem (o ordinal é o id dos botões; as de transmissão
/// e saturação existem só para manter a numeração).
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opt {
    InvertMouse,
    Sensitivity,
    Fov,
    Gamma,
    Saturation,
    RenderDistance,
    ViewBobbing,
    Anaglyph,
    FramerateLimit,
    FboEnable,
    RenderClouds,
    Graphics,
    AmbientOcclusion,
    GuiScale,
    Particles,
    ChatVisibility,
    ChatColor,
    ChatLinks,
    ChatOpacity,
    ChatLinksPrompt,
    SnooperEnabled,
    UseFullscreen,
    EnableVsync,
    UseVbo,
    Touchscreen,
    ChatScale,
    ChatWidth,
    ChatHeightFocused,
    ChatHeightUnfocused,
    MipmapLevels,
    ForceUnicodeFont,
    StreamBytesPerPixel,
    StreamVolumeMic,
    StreamVolumeSystem,
    StreamKbps,
    StreamFps,
    StreamCompression,
    StreamSendMetadata,
    StreamChatEnabled,
    StreamChatUserFilter,
    StreamMicToggleBehavior,
    BlockAlternatives,
    ReducedDebugInfo,
    EntityShadows,
    RealmsNotifications,
}

impl Opt {
    /// (chave de tradução, é float, é booleano, mínimo, máximo, passo)
    fn meta(self) -> (&'static str, bool, bool, f32, f32, f32) {
        use Opt::*;
        match self {
            InvertMouse => ("options.invertMouse", false, true, 0.0, 1.0, 0.0),
            Sensitivity => ("options.sensitivity", true, false, 0.0, 1.0, 0.0),
            Fov => ("options.fov", true, false, 30.0, 110.0, 1.0),
            Gamma => ("options.gamma", true, false, 0.0, 1.0, 0.0),
            Saturation => ("options.saturation", true, false, 0.0, 1.0, 0.0),
            // 32 com Java 64 bits e 1 GB ou mais (o caso do dono).
            RenderDistance => ("options.renderDistance", true, false, 2.0, 32.0, 1.0),
            ViewBobbing => ("options.viewBobbing", false, true, 0.0, 1.0, 0.0),
            Anaglyph => ("options.anaglyph", false, true, 0.0, 1.0, 0.0),
            FramerateLimit => ("options.framerateLimit", true, false, 10.0, 260.0, 10.0),
            FboEnable => ("options.fboEnable", false, true, 0.0, 1.0, 0.0),
            RenderClouds => ("options.renderClouds", false, false, 0.0, 1.0, 0.0),
            Graphics => ("options.graphics", false, false, 0.0, 1.0, 0.0),
            AmbientOcclusion => ("options.ao", false, false, 0.0, 1.0, 0.0),
            GuiScale => ("options.guiScale", false, false, 0.0, 1.0, 0.0),
            Particles => ("options.particles", false, false, 0.0, 1.0, 0.0),
            ChatVisibility => ("options.chat.visibility", false, false, 0.0, 1.0, 0.0),
            ChatColor => ("options.chat.color", false, true, 0.0, 1.0, 0.0),
            ChatLinks => ("options.chat.links", false, true, 0.0, 1.0, 0.0),
            ChatOpacity => ("options.chat.opacity", true, false, 0.0, 1.0, 0.0),
            ChatLinksPrompt => ("options.chat.links.prompt", false, true, 0.0, 1.0, 0.0),
            SnooperEnabled => ("options.snooper", false, true, 0.0, 1.0, 0.0),
            UseFullscreen => ("options.fullscreen", false, true, 0.0, 1.0, 0.0),
            EnableVsync => ("options.vsync", false, true, 0.0, 1.0, 0.0),
            UseVbo => ("options.vbo", false, true, 0.0, 1.0, 0.0),
            Touchscreen => ("options.touchscreen", false, true, 0.0, 1.0, 0.0),
            ChatScale => ("options.chat.scale", true, false, 0.0, 1.0, 0.0),
            ChatWidth => ("options.chat.width", true, false, 0.0, 1.0, 0.0),
            ChatHeightFocused => ("options.chat.height.focused", true, false, 0.0, 1.0, 0.0),
            ChatHeightUnfocused => ("options.chat.height.unfocused", true, false, 0.0, 1.0, 0.0),
            MipmapLevels => ("options.mipmapLevels", true, false, 0.0, 4.0, 1.0),
            ForceUnicodeFont => ("options.forceUnicodeFont", false, true, 0.0, 1.0, 0.0),
            StreamBytesPerPixel => ("options.stream.bytesPerPixel", true, false, 0.0, 1.0, 0.0),
            StreamVolumeMic => ("options.stream.micVolumne", true, false, 0.0, 1.0, 0.0),
            StreamVolumeSystem => ("options.stream.systemVolume", true, false, 0.0, 1.0, 0.0),
            StreamKbps => ("options.stream.kbps", true, false, 0.0, 1.0, 0.0),
            StreamFps => ("options.stream.fps", true, false, 0.0, 1.0, 0.0),
            StreamCompression => ("options.stream.compression", false, false, 0.0, 1.0, 0.0),
            StreamSendMetadata => ("options.stream.sendMetadata", false, true, 0.0, 1.0, 0.0),
            StreamChatEnabled => ("options.stream.chat.enabled", false, false, 0.0, 1.0, 0.0),
            StreamChatUserFilter => ("options.stream.chat.userFilter", false, false, 0.0, 1.0, 0.0),
            StreamMicToggleBehavior => ("options.stream.micToggleBehavior", false, false, 0.0, 1.0, 0.0),
            BlockAlternatives => ("options.blockAlternatives", false, true, 0.0, 1.0, 0.0),
            ReducedDebugInfo => ("options.reducedDebugInfo", false, true, 0.0, 1.0, 0.0),
            EntityShadows => ("options.entityShadows", false, true, 0.0, 1.0, 0.0),
            RealmsNotifications => ("options.realmsNotifications", false, true, 0.0, 1.0, 0.0),
        }
    }

    pub fn key(self) -> &'static str {
        self.meta().0
    }
    pub fn is_float(self) -> bool {
        self.meta().1
    }
    pub fn is_bool(self) -> bool {
        self.meta().2
    }
    pub fn min(self) -> f32 {
        self.meta().3
    }
    pub fn max(self) -> f32 {
        self.meta().4
    }

    fn snap(self, v: f32) -> f32 {
        let step = self.meta().5;
        let v = if step > 0.0 { step * (v / step).round() } else { v };
        v.clamp(self.min(), self.max())
    }

    /// `normalizeValue`: valor real -> posição do slider (0..1).
    pub fn normalize(self, v: f32) -> f32 {
        ((self.snap(v) - self.min()) / (self.max() - self.min())).clamp(0.0, 1.0)
    }

    /// `denormalizeValue`: posição do slider -> valor real (já no passo).
    pub fn denormalize(self, f: f32) -> f32 {
        self.snap(self.min() + (self.max() - self.min()) * f.clamp(0.0, 1.0))
    }
}

/// Categorias de som (`SoundCategory`), na ordem do Java.
pub const SOUND_CATEGORIES: [&str; 9] = ["master", "music", "record", "weather", "block", "hostile", "neutral", "player", "ambient"];

/// Partes do modelo (`EnumPlayerModelParts`): id = bit da máscara.
pub const MODEL_PARTS: [&str; 7] = ["cape", "jacket", "left_sleeve", "right_sleeve", "left_pants_leg", "right_pants_leg", "hat"];

#[derive(Clone, Debug, PartialEq)]
pub struct KeyBind {
    pub desc: &'static str,
    pub code: i32,
    pub default: i32,
    pub category: &'static str,
}

/// `GameSettings.keyBindings`: ordem do arquivo.
const KEYS: [(&str, i32, &str); 33] = [
    ("key.attack", -100, "key.categories.gameplay"),
    ("key.use", -99, "key.categories.gameplay"),
    ("key.forward", 17, "key.categories.movement"),
    ("key.left", 30, "key.categories.movement"),
    ("key.back", 31, "key.categories.movement"),
    ("key.right", 32, "key.categories.movement"),
    ("key.jump", 57, "key.categories.movement"),
    ("key.sneak", 42, "key.categories.movement"),
    ("key.sprint", 29, "key.categories.movement"),
    ("key.drop", 16, "key.categories.gameplay"),
    ("key.inventory", 18, "key.categories.inventory"),
    ("key.chat", 20, "key.categories.multiplayer"),
    ("key.playerlist", 15, "key.categories.multiplayer"),
    ("key.pickItem", -98, "key.categories.gameplay"),
    ("key.command", 53, "key.categories.multiplayer"),
    ("key.screenshot", 60, "key.categories.misc"),
    ("key.togglePerspective", 63, "key.categories.misc"),
    ("key.smoothCamera", 0, "key.categories.misc"),
    ("key.streamStartStop", 64, "key.categories.stream"),
    ("key.streamPauseUnpause", 65, "key.categories.stream"),
    ("key.streamCommercial", 0, "key.categories.stream"),
    ("key.streamToggleMic", 0, "key.categories.stream"),
    ("key.fullscreen", 87, "key.categories.misc"),
    ("key.spectatorOutlines", 0, "key.categories.misc"),
    ("key.hotbar.1", 2, "key.categories.inventory"),
    ("key.hotbar.2", 3, "key.categories.inventory"),
    ("key.hotbar.3", 4, "key.categories.inventory"),
    ("key.hotbar.4", 5, "key.categories.inventory"),
    ("key.hotbar.5", 6, "key.categories.inventory"),
    ("key.hotbar.6", 7, "key.categories.inventory"),
    ("key.hotbar.7", 8, "key.categories.inventory"),
    ("key.hotbar.8", 9, "key.categories.inventory"),
    ("key.hotbar.9", 10, "key.categories.inventory"),
];

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub invert_mouse: bool,
    pub mouse_sensitivity: f32,
    /// Graus (o arquivo guarda `(fov - 70) / 40`).
    pub fov: f32,
    pub gamma: f32,
    pub saturation: f32,
    pub render_distance: i32,
    pub gui_scale: i32,
    pub particles: i32,
    pub view_bobbing: bool,
    pub anaglyph: bool,
    pub max_fps: i32,
    pub fbo_enable: bool,
    pub difficulty: i32,
    pub fancy_graphics: bool,
    pub ambient_occlusion: i32,
    pub clouds: i32,
    pub resource_packs: Vec<String>,
    pub incompatible_resource_packs: Vec<String>,
    pub last_server: String,
    pub language: String,
    pub chat_visibility: i32,
    pub chat_colors: bool,
    pub chat_links: bool,
    pub chat_links_prompt: bool,
    pub chat_opacity: f32,
    pub snooper_enabled: bool,
    pub fullscreen: bool,
    pub enable_vsync: bool,
    pub use_vbo: bool,
    pub hide_server_address: bool,
    pub advanced_item_tooltips: bool,
    pub pause_on_lost_focus: bool,
    pub touchscreen: bool,
    pub override_width: i32,
    pub override_height: i32,
    pub held_item_tooltips: bool,
    pub chat_height_focused: f32,
    pub chat_height_unfocused: f32,
    pub chat_scale: f32,
    pub chat_width: f32,
    pub show_inventory_achievement_hint: bool,
    pub mipmap_levels: i32,
    pub stream_bytes_per_pixel: f32,
    pub stream_mic_volume: f32,
    pub stream_system_volume: f32,
    pub stream_kbps: f32,
    pub stream_fps: f32,
    pub stream_compression: i32,
    pub stream_send_metadata: bool,
    pub stream_preferred_server: String,
    pub stream_chat_enabled: i32,
    pub stream_chat_user_filter: i32,
    pub stream_mic_toggle_behavior: i32,
    pub force_unicode_font: bool,
    pub allow_block_alternatives: bool,
    pub reduced_debug_info: bool,
    pub use_native_transport: bool,
    pub entity_shadows: bool,
    pub realms_notifications: bool,
    pub keys: Vec<KeyBind>,
    /// Volume por categoria, na ordem de `SOUND_CATEGORIES`.
    pub sound: [f32; 9],
    /// Bits de `MODEL_PARTS` ligados.
    pub model_parts: u8,
    /// Linhas desconhecidas (mods), regravadas no fim.
    pub extra: Vec<(String, String)>,
    /// Onde gravar.
    pub path: Option<PathBuf>,
}

impl Default for Settings {
    /// Valores iniciais do construtor do `GameSettings` (Java 64 bits).
    fn default() -> Self {
        Self {
            invert_mouse: false,
            mouse_sensitivity: 0.5,
            fov: 70.0,
            gamma: 0.0,
            saturation: 0.0,
            render_distance: 12,
            gui_scale: 0,
            particles: 0,
            view_bobbing: true,
            anaglyph: false,
            max_fps: 120,
            fbo_enable: true,
            difficulty: 2,
            fancy_graphics: true,
            ambient_occlusion: 2,
            clouds: 2,
            resource_packs: Vec::new(),
            incompatible_resource_packs: Vec::new(),
            last_server: String::new(),
            language: "en_US".into(),
            chat_visibility: 0,
            chat_colors: true,
            chat_links: true,
            chat_links_prompt: true,
            chat_opacity: 1.0,
            snooper_enabled: true,
            fullscreen: false,
            enable_vsync: true,
            use_vbo: false,
            hide_server_address: false,
            advanced_item_tooltips: false,
            pause_on_lost_focus: true,
            touchscreen: false,
            override_width: 0,
            override_height: 0,
            held_item_tooltips: true,
            chat_height_focused: 1.0,
            chat_height_unfocused: 0.443_661_96,
            chat_scale: 1.0,
            chat_width: 1.0,
            show_inventory_achievement_hint: true,
            mipmap_levels: 4,
            stream_bytes_per_pixel: 0.5,
            stream_mic_volume: 1.0,
            stream_system_volume: 1.0,
            stream_kbps: 0.541_284_4,
            stream_fps: 0.316_901_42,
            stream_compression: 1,
            stream_send_metadata: true,
            stream_preferred_server: String::new(),
            stream_chat_enabled: 0,
            stream_chat_user_filter: 0,
            stream_mic_toggle_behavior: 0,
            force_unicode_font: false,
            allow_block_alternatives: true,
            reduced_debug_info: false,
            use_native_transport: true,
            entity_shadows: true,
            realms_notifications: true,
            keys: KEYS.iter().map(|&(desc, code, category)| KeyBind { desc, code, default: code, category }).collect(),
            sound: [1.0; 9],
            model_parts: 0x7F,
            extra: Vec::new(),
            path: None,
        }
    }
}

/// `Float.toString` do Java (o suficiente para o options.txt).
pub fn java_float(f: f32) -> String {
    if f.is_nan() {
        return "NaN".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    let a = f.abs();
    if a != 0.0 && !(1e-3..1e7).contains(&a) {
        // Notação científica: 1.0E-4
        let s = format!("{f:e}");
        let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
        let m = if m.contains('.') { m.to_string() } else { format!("{m}.0") };
        return format!("{m}E{e}");
    }
    let s = format!("{f}");
    if s.contains('.') { s } else { format!("{s}.0") }
}

fn parse_float(s: &str) -> Option<f32> {
    match s {
        "true" => Some(1.0),
        "false" => Some(0.0),
        _ => s.trim().parse().ok(),
    }
}

impl Settings {
    /// Lê o `options.txt` (`GameSettings.loadOptions`). Sem arquivo, os padrões.
    pub fn load(path: &Path) -> Self {
        let mut s = Settings { path: Some(path.to_path_buf()), ..Default::default() };
        if let Ok(text) = std::fs::read_to_string(path) {
            s.parse(&text);
        }
        s
    }

    pub fn parse(&mut self, text: &str) {
        for line in text.lines() {
            if line.is_empty() {
                continue;
            }
            if !self.apply_line(line) {
                if let Some((k, v)) = line.split_once(':') {
                    self.extra.push((k.to_string(), v.to_string()));
                }
            }
        }
    }

    /// Uma linha `chave:valor`; false se a chave é desconhecida (ou o valor ruim).
    fn apply_line(&mut self, line: &str) -> bool {
        let Some((k, v)) = line.split_once(':') else { return false };
        // `astring[1]` no Java é o pedaço até o próximo ':'.
        let first = v.split(':').next().unwrap_or("");
        let b = first == "true";
        let f = || parse_float(first);
        let i = || first.trim().parse::<i32>().ok();
        macro_rules! set {
            ($field:expr, $val:expr) => {{
                match $val {
                    Some(x) => {
                        $field = x;
                        true
                    }
                    None => false,
                }
            }};
        }
        match k {
            "invertYMouse" => self.invert_mouse = b,
            "mouseSensitivity" => return set!(self.mouse_sensitivity, f()),
            "fov" => return set!(self.fov, f().map(|x| x * 40.0 + 70.0)),
            "gamma" => return set!(self.gamma, f()),
            "saturation" => return set!(self.saturation, f()),
            "renderDistance" => return set!(self.render_distance, i()),
            "guiScale" => return set!(self.gui_scale, i()),
            "particles" => return set!(self.particles, i()),
            "bobView" => self.view_bobbing = b,
            "anaglyph3d" => self.anaglyph = b,
            "maxFps" => return set!(self.max_fps, i()),
            "fboEnable" => self.fbo_enable = b,
            "difficulty" => return set!(self.difficulty, i()),
            "fancyGraphics" => self.fancy_graphics = b,
            "ao" => {
                self.ambient_occlusion = match first {
                    "true" => 2,
                    "false" => 0,
                    _ => return set!(self.ambient_occlusion, i()),
                }
            }
            "renderClouds" => match first {
                "true" => self.clouds = 2,
                "false" => self.clouds = 0,
                "fast" => self.clouds = 1,
                _ => {}
            },
            "resourcePacks" => self.resource_packs = serde_json::from_str(v).unwrap_or_default(),
            "incompatibleResourcePacks" => self.incompatible_resource_packs = serde_json::from_str(v).unwrap_or_default(),
            "lastServer" => self.last_server = v.to_string(),
            "lang" => self.language = first.to_string(),
            "chatVisibility" => return set!(self.chat_visibility, i()),
            "chatColors" => self.chat_colors = b,
            "chatLinks" => self.chat_links = b,
            "chatLinksPrompt" => self.chat_links_prompt = b,
            "chatOpacity" => return set!(self.chat_opacity, f()),
            "snooperEnabled" => self.snooper_enabled = b,
            "fullscreen" => self.fullscreen = b,
            "enableVsync" => self.enable_vsync = b,
            "useVbo" => self.use_vbo = b,
            "hideServerAddress" => self.hide_server_address = b,
            "advancedItemTooltips" => self.advanced_item_tooltips = b,
            "pauseOnLostFocus" => self.pause_on_lost_focus = b,
            "touchscreen" => self.touchscreen = b,
            "overrideWidth" => return set!(self.override_width, i()),
            "overrideHeight" => return set!(self.override_height, i()),
            "heldItemTooltips" => self.held_item_tooltips = b,
            "chatHeightFocused" => return set!(self.chat_height_focused, f()),
            "chatHeightUnfocused" => return set!(self.chat_height_unfocused, f()),
            "chatScale" => return set!(self.chat_scale, f()),
            "chatWidth" => return set!(self.chat_width, f()),
            "showInventoryAchievementHint" => self.show_inventory_achievement_hint = b,
            "mipmapLevels" => return set!(self.mipmap_levels, i()),
            "streamBytesPerPixel" => return set!(self.stream_bytes_per_pixel, f()),
            "streamMicVolume" => return set!(self.stream_mic_volume, f()),
            "streamSystemVolume" => return set!(self.stream_system_volume, f()),
            "streamKbps" => return set!(self.stream_kbps, f()),
            "streamFps" => return set!(self.stream_fps, f()),
            "streamCompression" => return set!(self.stream_compression, i()),
            "streamSendMetadata" => self.stream_send_metadata = b,
            "streamPreferredServer" => self.stream_preferred_server = v.to_string(),
            "streamChatEnabled" => return set!(self.stream_chat_enabled, i()),
            "streamChatUserFilter" => return set!(self.stream_chat_user_filter, i()),
            "streamMicToggleBehavior" => return set!(self.stream_mic_toggle_behavior, i()),
            "forceUnicodeFont" => self.force_unicode_font = b,
            "allowBlockAlternatives" => self.allow_block_alternatives = b,
            "reducedDebugInfo" => self.reduced_debug_info = b,
            "useNativeTransport" => self.use_native_transport = b,
            "entityShadows" => self.entity_shadows = b,
            "realmsNotifications" => self.realms_notifications = b,
            _ => {
                if let Some(desc) = k.strip_prefix("key_") {
                    if let Some(kb) = self.keys.iter_mut().find(|kb| kb.desc == desc) {
                        return set!(kb.code, i());
                    }
                    return false;
                }
                if let Some(cat) = k.strip_prefix("soundCategory_") {
                    if let Some(n) = SOUND_CATEGORIES.iter().position(|c| *c == cat) {
                        return set!(self.sound[n], f());
                    }
                    return false;
                }
                if let Some(part) = k.strip_prefix("modelPart_") {
                    if let Some(n) = MODEL_PARTS.iter().position(|p| *p == part) {
                        if b {
                            self.model_parts |= 1 << n;
                        } else {
                            self.model_parts &= !(1 << n);
                        }
                        return true;
                    }
                }
                return false;
            }
        }
        true
    }

    /// Texto do arquivo (`GameSettings.saveOptions`).
    pub fn to_text(&self) -> String {
        let jf = java_float;
        let clouds = match self.clouds {
            0 => "false",
            1 => "fast",
            _ => "true",
        };
        let mut out = vec![
            format!("invertYMouse:{}", self.invert_mouse),
            format!("mouseSensitivity:{}", jf(self.mouse_sensitivity)),
            format!("fov:{}", jf((self.fov - 70.0) / 40.0)),
            format!("gamma:{}", jf(self.gamma)),
            format!("saturation:{}", jf(self.saturation)),
            format!("renderDistance:{}", self.render_distance),
            format!("guiScale:{}", self.gui_scale),
            format!("particles:{}", self.particles),
            format!("bobView:{}", self.view_bobbing),
            format!("anaglyph3d:{}", self.anaglyph),
            format!("maxFps:{}", self.max_fps),
            format!("fboEnable:{}", self.fbo_enable),
            format!("difficulty:{}", self.difficulty),
            format!("fancyGraphics:{}", self.fancy_graphics),
            format!("ao:{}", self.ambient_occlusion),
            format!("renderClouds:{clouds}"),
            format!("resourcePacks:{}", serde_json::to_string(&self.resource_packs).unwrap_or_default()),
            format!("incompatibleResourcePacks:{}", serde_json::to_string(&self.incompatible_resource_packs).unwrap_or_default()),
            format!("lastServer:{}", self.last_server),
            format!("lang:{}", self.language),
            format!("chatVisibility:{}", self.chat_visibility),
            format!("chatColors:{}", self.chat_colors),
            format!("chatLinks:{}", self.chat_links),
            format!("chatLinksPrompt:{}", self.chat_links_prompt),
            format!("chatOpacity:{}", jf(self.chat_opacity)),
            format!("snooperEnabled:{}", self.snooper_enabled),
            format!("fullscreen:{}", self.fullscreen),
            format!("enableVsync:{}", self.enable_vsync),
            format!("useVbo:{}", self.use_vbo),
            format!("hideServerAddress:{}", self.hide_server_address),
            format!("advancedItemTooltips:{}", self.advanced_item_tooltips),
            format!("pauseOnLostFocus:{}", self.pause_on_lost_focus),
            format!("touchscreen:{}", self.touchscreen),
            format!("overrideWidth:{}", self.override_width),
            format!("overrideHeight:{}", self.override_height),
            format!("heldItemTooltips:{}", self.held_item_tooltips),
            format!("chatHeightFocused:{}", jf(self.chat_height_focused)),
            format!("chatHeightUnfocused:{}", jf(self.chat_height_unfocused)),
            format!("chatScale:{}", jf(self.chat_scale)),
            format!("chatWidth:{}", jf(self.chat_width)),
            format!("showInventoryAchievementHint:{}", self.show_inventory_achievement_hint),
            format!("mipmapLevels:{}", self.mipmap_levels),
            format!("streamBytesPerPixel:{}", jf(self.stream_bytes_per_pixel)),
            format!("streamMicVolume:{}", jf(self.stream_mic_volume)),
            format!("streamSystemVolume:{}", jf(self.stream_system_volume)),
            format!("streamKbps:{}", jf(self.stream_kbps)),
            format!("streamFps:{}", jf(self.stream_fps)),
            format!("streamCompression:{}", self.stream_compression),
            format!("streamSendMetadata:{}", self.stream_send_metadata),
            format!("streamPreferredServer:{}", self.stream_preferred_server),
            format!("streamChatEnabled:{}", self.stream_chat_enabled),
            format!("streamChatUserFilter:{}", self.stream_chat_user_filter),
            format!("streamMicToggleBehavior:{}", self.stream_mic_toggle_behavior),
            format!("forceUnicodeFont:{}", self.force_unicode_font),
            format!("allowBlockAlternatives:{}", self.allow_block_alternatives),
            format!("reducedDebugInfo:{}", self.reduced_debug_info),
            format!("useNativeTransport:{}", self.use_native_transport),
            format!("entityShadows:{}", self.entity_shadows),
            format!("realmsNotifications:{}", self.realms_notifications),
        ];
        out.extend(self.keys.iter().map(|k| format!("key_{}:{}", k.desc, k.code)));
        out.extend(SOUND_CATEGORIES.iter().zip(self.sound).map(|(c, v)| format!("soundCategory_{c}:{}", jf(v))));
        out.extend(MODEL_PARTS.iter().enumerate().map(|(i, p)| format!("modelPart_{p}:{}", self.model_parts & (1 << i) != 0)));
        out.extend(self.extra.iter().map(|(k, v)| format!("{k}:{v}")));
        let mut s = out.join("\n");
        s.push('\n');
        s
    }

    pub fn save(&self) {
        let Some(path) = &self.path else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(path, self.to_text()) {
            eprintln!("não gravei {}: {e}", path.display());
        }
    }

    pub fn float(&self, o: Opt) -> f32 {
        use Opt::*;
        match o {
            Fov => self.fov,
            Gamma => self.gamma,
            Saturation => self.saturation,
            Sensitivity => self.mouse_sensitivity,
            ChatOpacity => self.chat_opacity,
            ChatHeightFocused => self.chat_height_focused,
            ChatHeightUnfocused => self.chat_height_unfocused,
            ChatScale => self.chat_scale,
            ChatWidth => self.chat_width,
            FramerateLimit => self.max_fps as f32,
            MipmapLevels => self.mipmap_levels as f32,
            RenderDistance => self.render_distance as f32,
            StreamBytesPerPixel => self.stream_bytes_per_pixel,
            StreamVolumeMic => self.stream_mic_volume,
            StreamVolumeSystem => self.stream_system_volume,
            StreamKbps => self.stream_kbps,
            StreamFps => self.stream_fps,
            _ => 0.0,
        }
    }

    /// `setOptionFloatValue`.
    pub fn set_float(&mut self, o: Opt, v: f32) {
        use Opt::*;
        match o {
            Sensitivity => self.mouse_sensitivity = v,
            Fov => self.fov = v,
            Gamma => self.gamma = v,
            FramerateLimit => self.max_fps = v as i32,
            ChatOpacity => self.chat_opacity = v,
            ChatHeightFocused => self.chat_height_focused = v,
            ChatHeightUnfocused => self.chat_height_unfocused = v,
            ChatWidth => self.chat_width = v,
            ChatScale => self.chat_scale = v,
            MipmapLevels => self.mipmap_levels = v as i32,
            RenderDistance => self.render_distance = v as i32,
            StreamBytesPerPixel => self.stream_bytes_per_pixel = v,
            StreamVolumeMic => self.stream_mic_volume = v,
            StreamVolumeSystem => self.stream_system_volume = v,
            StreamKbps => self.stream_kbps = v,
            StreamFps => self.stream_fps = v,
            _ => {}
        }
    }

    pub fn bool(&self, o: Opt) -> bool {
        use Opt::*;
        match o {
            InvertMouse => self.invert_mouse,
            ViewBobbing => self.view_bobbing,
            Anaglyph => self.anaglyph,
            FboEnable => self.fbo_enable,
            ChatColor => self.chat_colors,
            ChatLinks => self.chat_links,
            ChatLinksPrompt => self.chat_links_prompt,
            SnooperEnabled => self.snooper_enabled,
            UseFullscreen => self.fullscreen,
            EnableVsync => self.enable_vsync,
            UseVbo => self.use_vbo,
            Touchscreen => self.touchscreen,
            StreamSendMetadata => self.stream_send_metadata,
            ForceUnicodeFont => self.force_unicode_font,
            BlockAlternatives => self.allow_block_alternatives,
            ReducedDebugInfo => self.reduced_debug_info,
            EntityShadows => self.entity_shadows,
            RealmsNotifications => self.realms_notifications,
            _ => false,
        }
    }

    /// `setOptionValue(option, 1)`: alterna ou avança e grava.
    pub fn cycle(&mut self, o: Opt) {
        use Opt::*;
        match o {
            InvertMouse => self.invert_mouse ^= true,
            GuiScale => self.gui_scale = (self.gui_scale + 1) & 3,
            Particles => self.particles = (self.particles + 1) % 3,
            ViewBobbing => self.view_bobbing ^= true,
            RenderClouds => self.clouds = (self.clouds + 1) % 3,
            ForceUnicodeFont => self.force_unicode_font ^= true,
            FboEnable => self.fbo_enable ^= true,
            Anaglyph => self.anaglyph ^= true,
            Graphics => self.fancy_graphics ^= true,
            AmbientOcclusion => self.ambient_occlusion = (self.ambient_occlusion + 1) % 3,
            ChatVisibility => self.chat_visibility = (self.chat_visibility + 1) % 3,
            StreamCompression => self.stream_compression = (self.stream_compression + 1) % 3,
            StreamSendMetadata => self.stream_send_metadata ^= true,
            StreamChatEnabled => self.stream_chat_enabled = (self.stream_chat_enabled + 1) % 3,
            StreamChatUserFilter => self.stream_chat_user_filter = (self.stream_chat_user_filter + 1) % 3,
            StreamMicToggleBehavior => self.stream_mic_toggle_behavior = (self.stream_mic_toggle_behavior + 1) % 2,
            ChatColor => self.chat_colors ^= true,
            ChatLinks => self.chat_links ^= true,
            ChatLinksPrompt => self.chat_links_prompt ^= true,
            SnooperEnabled => self.snooper_enabled ^= true,
            Touchscreen => self.touchscreen ^= true,
            UseFullscreen => self.fullscreen ^= true,
            EnableVsync => self.enable_vsync ^= true,
            UseVbo => self.use_vbo ^= true,
            BlockAlternatives => self.allow_block_alternatives ^= true,
            ReducedDebugInfo => self.reduced_debug_info ^= true,
            EntityShadows => self.entity_shadows ^= true,
            RealmsNotifications => self.realms_notifications ^= true,
            _ => {}
        }
        self.save();
    }

    /// Texto do botão (`GameSettings.getKeyBinding`).
    pub fn display(&self, o: Opt) -> String {
        use Opt::*;
        let s = format!("{}: ", tr(o.key()));
        if o.is_float() {
            let f1 = self.float(o);
            let f = o.normalize(f1);
            return match o {
                Sensitivity if f == 0.0 => s + &tr("options.sensitivity.min"),
                Sensitivity if f == 1.0 => s + &tr("options.sensitivity.max"),
                Sensitivity => format!("{s}{}%", (f * 200.0) as i32),
                Fov if f1 == 70.0 => s + &tr("options.fov.min"),
                Fov if f1 == 110.0 => s + &tr("options.fov.max"),
                Fov => format!("{s}{}", f1 as i32),
                FramerateLimit if f1 == o.max() => s + &tr("options.framerateLimit.max"),
                FramerateLimit => format!("{s}{} fps", f1 as i32),
                Gamma if f == 0.0 => s + &tr("options.gamma.min"),
                Gamma if f == 1.0 => s + &tr("options.gamma.max"),
                Gamma => format!("{s}+{}%", (f * 100.0) as i32),
                Saturation => format!("{s}{}%", (f * 400.0) as i32),
                ChatOpacity => format!("{s}{}%", (f * 90.0 + 10.0) as i32),
                ChatHeightUnfocused | ChatHeightFocused => format!("{s}{}px", chat_height(f)),
                ChatWidth => format!("{s}{}px", chat_width(f)),
                RenderDistance => format!("{s}{} chunks", f1 as i32),
                MipmapLevels if f1 == 0.0 => s + &tr("options.off"),
                MipmapLevels => format!("{s}{}", f1 as i32),
                _ if f == 0.0 => s + &tr("options.off"),
                _ => format!("{s}{}%", (f * 100.0) as i32),
            };
        }
        if o.is_bool() {
            return s + &tr(if self.bool(o) { "options.on" } else { "options.off" });
        }
        let pick = |keys: &[&str], i: i32| tr(keys.get(i as usize).copied().unwrap_or(keys[0]));
        match o {
            GuiScale => s + &pick(&["options.guiScale.auto", "options.guiScale.small", "options.guiScale.normal", "options.guiScale.large"], self.gui_scale),
            ChatVisibility => s + &pick(&["options.chat.visibility.full", "options.chat.visibility.system", "options.chat.visibility.hidden"], self.chat_visibility),
            Particles => s + &pick(&["options.particles.all", "options.particles.decreased", "options.particles.minimal"], self.particles),
            AmbientOcclusion => s + &pick(&["options.ao.off", "options.ao.min", "options.ao.max"], self.ambient_occlusion),
            StreamCompression => s + &pick(
                &["options.stream.compression.low", "options.stream.compression.medium", "options.stream.compression.high"],
                self.stream_compression,
            ),
            StreamChatEnabled => s + &pick(
                &["options.stream.chat.enabled.streaming", "options.stream.chat.enabled.always", "options.stream.chat.enabled.never"],
                self.stream_chat_enabled,
            ),
            StreamChatUserFilter => s + &pick(
                &["options.stream.chat.userFilter.all", "options.stream.chat.userFilter.subs", "options.stream.chat.userFilter.mods"],
                self.stream_chat_user_filter,
            ),
            StreamMicToggleBehavior => {
                s + &pick(&["options.stream.mic_toggle.mute", "options.stream.mic_toggle.talk"], self.stream_mic_toggle_behavior)
            }
            RenderClouds => s + &pick(&["options.off", "options.graphics.fast", "options.graphics.fancy"], self.clouds),
            Graphics => s + &tr(if self.fancy_graphics { "options.graphics.fancy" } else { "options.graphics.fast" }),
            _ => s,
        }
    }

    /// Texto do volume (`GuiScreenOptionsSounds.getSoundVolume`).
    pub fn sound_display(&self, cat: usize) -> String {
        let v = self.sound[cat];
        let val = if v == 0.0 { tr("options.off") } else { format!("{}%", (v * 100.0) as i32) };
        format!("{}: {val}", tr(&format!("soundCategory.{}", SOUND_CATEGORIES[cat])))
    }

    /// Texto do botão de parte da skin (`GuiCustomizeSkin.func_175358_a`).
    pub fn part_display(&self, part: usize) -> String {
        let on = self.model_parts & (1 << part) != 0;
        format!("{}: {}", tr(&format!("options.modelPart.{}", MODEL_PARTS[part])), tr(if on { "options.on" } else { "options.off" }))
    }

    pub fn key(&self, desc: &str) -> i32 {
        self.keys.iter().find(|k| k.desc == desc).map_or(0, |k| k.code)
    }

    /// Escala da GUI (`ScaledResolution`) para a janela `w x h`.
    pub fn scale_for(&self, w: u32, h: u32) -> u32 {
        let limit = if self.gui_scale == 0 { 1000 } else { self.gui_scale as u32 };
        let mut s = 1;
        while s < limit && w / (s + 1) >= 320 && h / (s + 1) >= 240 {
            s += 1;
        }
        s
    }
}

/// `GuiNewChat.calculateChatboxHeight`.
pub fn chat_height(f: f32) -> i32 {
    (f * 160.0 + 20.0).floor() as i32
}

/// `GuiNewChat.calculateChatboxWidth`.
pub fn chat_width(f: f32) -> i32 {
    (f * 280.0 + 40.0).floor() as i32
}

/// Códigos de tecla do LWJGL 2 (os do options.txt), a tecla do winit e o nome do `Keyboard.getKeyName`.
const KEYMAP: &[(i32, KeyCode, &str)] = {
    use KeyCode::*;
    &[
        (1, Escape, "ESCAPE"),
        (2, Digit1, "1"),
        (3, Digit2, "2"),
        (4, Digit3, "3"),
        (5, Digit4, "4"),
        (6, Digit5, "5"),
        (7, Digit6, "6"),
        (8, Digit7, "7"),
        (9, Digit8, "8"),
        (10, Digit9, "9"),
        (11, Digit0, "0"),
        (12, Minus, "MINUS"),
        (13, Equal, "EQUALS"),
        (14, Backspace, "BACK"),
        (15, Tab, "TAB"),
        (16, KeyQ, "Q"),
        (17, KeyW, "W"),
        (18, KeyE, "E"),
        (19, KeyR, "R"),
        (20, KeyT, "T"),
        (21, KeyY, "Y"),
        (22, KeyU, "U"),
        (23, KeyI, "I"),
        (24, KeyO, "O"),
        (25, KeyP, "P"),
        (26, BracketLeft, "LBRACKET"),
        (27, BracketRight, "RBRACKET"),
        (28, Enter, "RETURN"),
        (29, ControlLeft, "LCONTROL"),
        (30, KeyA, "A"),
        (31, KeyS, "S"),
        (32, KeyD, "D"),
        (33, KeyF, "F"),
        (34, KeyG, "G"),
        (35, KeyH, "H"),
        (36, KeyJ, "J"),
        (37, KeyK, "K"),
        (38, KeyL, "L"),
        (39, Semicolon, "SEMICOLON"),
        (40, Quote, "APOSTROPHE"),
        (41, Backquote, "GRAVE"),
        (42, ShiftLeft, "LSHIFT"),
        (43, Backslash, "BACKSLASH"),
        (44, KeyZ, "Z"),
        (45, KeyX, "X"),
        (46, KeyC, "C"),
        (47, KeyV, "V"),
        (48, KeyB, "B"),
        (49, KeyN, "N"),
        (50, KeyM, "M"),
        (51, Comma, "COMMA"),
        (52, Period, "PERIOD"),
        (53, Slash, "SLASH"),
        (54, ShiftRight, "RSHIFT"),
        (55, NumpadMultiply, "MULTIPLY"),
        (56, AltLeft, "LMENU"),
        (57, Space, "SPACE"),
        (58, CapsLock, "CAPITAL"),
        (59, F1, "F1"),
        (60, F2, "F2"),
        (61, F3, "F3"),
        (62, F4, "F4"),
        (63, F5, "F5"),
        (64, F6, "F6"),
        (65, F7, "F7"),
        (66, F8, "F8"),
        (67, F9, "F9"),
        (68, F10, "F10"),
        (69, NumLock, "NUMLOCK"),
        (70, ScrollLock, "SCROLL"),
        (71, Numpad7, "NUMPAD7"),
        (72, Numpad8, "NUMPAD8"),
        (73, Numpad9, "NUMPAD9"),
        (74, NumpadSubtract, "SUBTRACT"),
        (75, Numpad4, "NUMPAD4"),
        (76, Numpad5, "NUMPAD5"),
        (77, Numpad6, "NUMPAD6"),
        (78, NumpadAdd, "ADD"),
        (79, Numpad1, "NUMPAD1"),
        (80, Numpad2, "NUMPAD2"),
        (81, Numpad3, "NUMPAD3"),
        (82, Numpad0, "NUMPAD0"),
        (83, NumpadDecimal, "DECIMAL"),
        (86, IntlBackslash, "NONE"),
        (87, F11, "F11"),
        (88, F12, "F12"),
        (100, F13, "F13"),
        (101, F14, "F14"),
        (102, F15, "F15"),
        (115, IntlRo, "NONE"),
        (156, NumpadEnter, "NUMPADENTER"),
        (157, ControlRight, "RCONTROL"),
        (181, NumpadDivide, "DIVIDE"),
        (183, PrintScreen, "SYSRQ"),
        (184, AltRight, "RMENU"),
        (197, Pause, "PAUSE"),
        (199, Home, "HOME"),
        (200, ArrowUp, "UP"),
        (201, PageUp, "PRIOR"),
        (203, ArrowLeft, "LEFT"),
        (205, ArrowRight, "RIGHT"),
        (207, End, "END"),
        (208, ArrowDown, "DOWN"),
        (209, PageDown, "NEXT"),
        (210, Insert, "INSERT"),
        (211, Delete, "DELETE"),
        (219, SuperLeft, "LMETA"),
        (220, SuperRight, "RMETA"),
        (221, ContextMenu, "APPS"),
    ]
};

/// Teclas físicas já vistas -> código LWJGL pelo layout do teclado. O LWJGL do Linux traduz o keysym
/// sem modificadores (no ABNT2 o "]" fica na tecla que no americano é o "\\" e o "/" na tecla ao lado do Shift).
static LAYOUT: std::sync::Mutex<Vec<(KeyCode, i32)>> = std::sync::Mutex::new(Vec::new());

/// Registra a tecla física `k` com o que ela digita sem modificadores (chamado a cada tecla).
pub fn learn_key(k: KeyCode, unshifted: &winit::keyboard::Key) {
    let code = layout_code(k, unshifted);
    let Ok(mut l) = LAYOUT.lock() else { return };
    match l.iter_mut().find(|(p, _)| *p == k) {
        Some(e) => e.1 = code,
        None => l.push((k, code)),
    }
}

/// `LinuxKeycodes.mapKeySymToLWJGLKeyCode`: o caractere decide a tecla; o teclado numérico e as teclas sem
/// caractere ficam pela posição. Sem tecla LWJGL, o `Minecraft.runTick` usa caractere + 256.
fn layout_code(k: KeyCode, key: &winit::keyboard::Key) -> i32 {
    use KeyCode::*;
    let phys = KEYMAP.iter().find(|(_, kc, _)| *kc == k).map_or(0, |(c, _, _)| *c);
    let numpad = matches!(
        k,
        Numpad0 | Numpad1 | Numpad2 | Numpad3 | Numpad4 | Numpad5 | Numpad6 | Numpad7 | Numpad8 | Numpad9 | NumpadAdd | NumpadSubtract | NumpadMultiply | NumpadDivide | NumpadDecimal | NumpadEnter | NumpadEqual | NumpadComma
    );
    let winit::keyboard::Key::Character(s) = key else { return phys };
    let mut chars = s.chars();
    let (Some(c), None) = (chars.next(), chars.next()) else { return phys };
    if numpad || c == ' ' {
        return phys;
    }
    let name = match c.to_ascii_uppercase() {
        c @ ('A'..='Z' | '0'..='9') => return KEYMAP.iter().find(|(_, _, n)| n.len() == 1 && n.starts_with(c)).map_or(phys, |(code, _, _)| *code),
        '-' => "MINUS",
        '=' => "EQUALS",
        '[' => "LBRACKET",
        ']' => "RBRACKET",
        ';' => "SEMICOLON",
        '\'' => "APOSTROPHE",
        '`' => "GRAVE",
        '\\' => "BACKSLASH",
        ',' => "COMMA",
        '.' => "PERIOD",
        '/' => "SLASH",
        c if c.is_ascii() => return phys,
        c => return 256 + c as i32,
    };
    KEYMAP.iter().find(|(_, _, n)| *n == name).map_or(phys, |(code, _, _)| *code)
}

/// Código LWJGL de uma tecla do winit (0 = sem equivalente), pelo layout quando a tecla já foi vista.
pub fn lwjgl_code(k: KeyCode) -> i32 {
    if let Some(c) = LAYOUT.lock().ok().and_then(|l| l.iter().find(|(p, _)| *p == k).map(|e| e.1)) {
        return c;
    }
    KEYMAP.iter().find(|(_, kc, _)| *kc == k).map_or(0, |(c, _, _)| *c)
}

/// Tecla do winit de um código LWJGL (a do layout, senão a da posição americana que ainda não foi vista).
pub fn winit_key(code: i32) -> Option<KeyCode> {
    let l = LAYOUT.lock().map(|l| l.clone()).unwrap_or_default();
    if let Some((k, _)) = l.iter().find(|(_, c)| *c == code) {
        return Some(*k);
    }
    KEYMAP.iter().find(|(c, k, _)| *c == code && !l.iter().any(|(p, _)| p == k)).map(|(_, k, _)| *k)
}

/// `GameSettings.getKeyDisplayString`.
pub fn key_name(code: i32) -> String {
    if code < 0 {
        return trf("key.mouseButton", &[&(code + 101).to_string()]);
    }
    if code < 256 {
        return KEYMAP.iter().find(|(c, _, _)| *c == code).map_or("NONE", |(_, _, n)| n).to_string();
    }
    char::from_u32((code - 256) as u32).map(|c| c.to_uppercase().to_string()).unwrap_or_default()
}

/// Pasta de dados do mine-rs.
pub fn config_dir() -> PathBuf {
    // MINE_RS_CONFIG: pasta de configuração alternativa (testes e prints sem mexer na do dono).
    if let Some(d) = std::env::var_os("MINE_RS_CONFIG") {
        return PathBuf::from(d);
    }
    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config/mine-rs")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "invertYMouse:false\nmouseSensitivity:0.44366196\nfov:0.35\ngamma:10.0\nrenderDistance:10\nguiScale:2\n\
maxFps:260\nrenderClouds:false\nresourcePacks:[\"FruitFul_32x.zip\"]\nlastServer:mc.auratoons.com\nlang:pt_BR\nenableVsync:false\n\
key_key.sprint:33\nkey_key.hotbar.8:-96\nkey_of.key.zoom:0\nsoundCategory_master:0.16887417\nsoundCategory_music:0.0\nmodelPart_cape:false\n";

    #[test]
    fn parse_options() {
        let mut s = Settings::default();
        s.parse(SAMPLE);
        assert_eq!(s.mouse_sensitivity, 0.443_661_96);
        assert!((s.fov - 84.0).abs() < 1e-4);
        assert_eq!(s.gamma, 10.0);
        assert_eq!(s.render_distance, 10);
        assert_eq!(s.gui_scale, 2);
        assert_eq!(s.max_fps, 260);
        assert_eq!(s.clouds, 0);
        assert_eq!(s.resource_packs, vec!["FruitFul_32x.zip".to_string()]);
        assert_eq!(s.last_server, "mc.auratoons.com");
        assert!(!s.enable_vsync);
        assert_eq!(s.key("key.sprint"), 33);
        assert_eq!(s.key("key.hotbar.8"), -96);
        assert_eq!(s.sound[0], 0.168_874_17);
        assert_eq!(s.sound[1], 0.0);
        assert_eq!(s.model_parts & 1, 0);
        assert_eq!(s.extra, vec![("key_of.key.zoom".to_string(), "0".to_string())]);
    }

    #[test]
    fn options_round_trip() {
        let mut a = Settings::default();
        a.parse(SAMPLE);
        let text = a.to_text();
        let mut b = Settings::default();
        b.parse(&text);
        assert_eq!(a, b);
        assert_eq!(text, b.to_text());
        // Mesmo formato do Java.
        assert!(text.starts_with("invertYMouse:false\nmouseSensitivity:0.44366196\nfov:0.35\ngamma:10.0\n"));
        assert!(text.contains("\nrenderClouds:false\n"));
        assert!(text.contains("\nresourcePacks:[\"FruitFul_32x.zip\"]\n"));
        assert!(text.contains("\nkey_key.attack:-100\n"));
        assert!(text.contains("\nsoundCategory_ambient:1.0\n"));
        assert!(text.contains("\nmodelPart_cape:false\nmodelPart_jacket:true\n"));
        assert!(text.ends_with("modelPart_hat:true\nkey_of.key.zoom:0\n"));
    }

    #[test]
    fn real_options_round_trip() {
        let home = std::env::var("HOME").unwrap_or_default();
        let Ok(text) = std::fs::read_to_string(format!("{home}/.minecraft/options.txt")) else { return };
        let mut a = Settings::default();
        a.parse(&text);
        let mut b = Settings::default();
        b.parse(&a.to_text());
        assert_eq!(a, b);
        // Toda linha conhecida do original sai igual.
        let out = a.to_text();
        for line in text.lines().filter(|l| !l.is_empty()) {
            assert!(out.lines().any(|l| l == line), "linha mudou: {line}");
        }
    }

    #[test]
    fn java_floats() {
        assert_eq!(java_float(1.0), "1.0");
        assert_eq!(java_float(0.0), "0.0");
        assert_eq!(java_float(0.35), "0.35");
        assert_eq!(java_float(0.443_661_96), "0.44366196");
        assert_eq!(java_float(10.0), "10.0");
        assert_eq!(java_float(1e-4), "1.0E-4");
    }

    #[test]
    fn slider_value_mapping() {
        // Como GameSettings.Options.normalizeValue/denormalizeValue.
        assert_eq!(Opt::Fov.denormalize(0.5), 70.0);
        assert_eq!(Opt::Fov.normalize(70.0), 0.5);
        assert_eq!(Opt::Fov.denormalize(0.0), 30.0);
        assert_eq!(Opt::Fov.denormalize(1.0), 110.0);
        assert_eq!(Opt::FramerateLimit.denormalize(0.5), 140.0);
        assert_eq!(Opt::FramerateLimit.denormalize(0.52), 140.0);
        assert_eq!(Opt::FramerateLimit.denormalize(1.0), 260.0);
        assert_eq!(Opt::RenderDistance.denormalize(0.0), 2.0);
        assert_eq!(Opt::RenderDistance.denormalize(1.0), 32.0);
        assert_eq!(Opt::RenderDistance.normalize(17.0), 0.5);
        assert_eq!(Opt::MipmapLevels.denormalize(0.6), 2.0);
        assert_eq!(Opt::Sensitivity.denormalize(0.3), 0.3);
        assert_eq!(Opt::Gamma.normalize(10.0), 1.0);
    }

    #[test]
    fn display_strings() {
        let mut s = Settings::default();
        s.max_fps = 260;
        assert!(s.display(Opt::FramerateLimit).ends_with(": options.framerateLimit.max") || !s.display(Opt::FramerateLimit).contains("fps"));
        s.max_fps = 120;
        assert!(s.display(Opt::FramerateLimit).ends_with(": 120 fps"));
        s.render_distance = 10;
        assert!(s.display(Opt::RenderDistance).ends_with(": 10 chunks"));
        s.chat_width = 1.0;
        assert!(s.display(Opt::ChatWidth).ends_with(": 320px"));
        s.chat_height_unfocused = 0.443_661_96;
        assert!(s.display(Opt::ChatHeightUnfocused).ends_with(": 90px"));
        s.mouse_sensitivity = 0.5;
        assert!(s.display(Opt::Sensitivity).ends_with(": 100%"));
    }

    #[test]
    fn keys_and_scale() {
        assert_eq!(lwjgl_code(KeyCode::KeyW), 17);
        assert_eq!(winit_key(42), Some(KeyCode::ShiftLeft));
        // ABNT2: o "]" e o "/" pelo caractere, o "ç" vira caractere + 256 e o numérico fica pela posição.
        use winit::keyboard::Key;
        learn_key(KeyCode::Backslash, &Key::Character("]".into()));
        learn_key(KeyCode::BracketRight, &Key::Character("[".into()));
        learn_key(KeyCode::IntlRo, &Key::Character("/".into()));
        learn_key(KeyCode::Semicolon, &Key::Character("ç".into()));
        learn_key(KeyCode::NumpadDivide, &Key::Character("/".into()));
        assert_eq!(lwjgl_code(KeyCode::Backslash), 27);
        assert_eq!(lwjgl_code(KeyCode::BracketRight), 26);
        assert_eq!(winit_key(27), Some(KeyCode::Backslash));
        assert_eq!(lwjgl_code(KeyCode::IntlRo), 53);
        assert_eq!(lwjgl_code(KeyCode::Semicolon), 256 + 'ç' as i32);
        assert_eq!(lwjgl_code(KeyCode::NumpadDivide), 181);
        assert_eq!(key_name(17), "W");
        assert_eq!(key_name(57), "SPACE");
        assert_eq!(key_name(0), "NONE");
        let s = Settings { gui_scale: 2, ..Default::default() };
        assert_eq!(s.scale_for(1920, 1080), 2);
        let auto = Settings::default();
        assert_eq!(auto.scale_for(1920, 1080), 4);
        assert_eq!(auto.scale_for(1280, 720), 3);
        assert_eq!(auto.scale_for(800, 600), 2);
    }
}
