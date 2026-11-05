//! Telas fora do jogo e de opções, portadas das classes `Gui*` da 1.8.9:
//! `GuiMainMenu`, `GuiMultiplayer` (+ `ServerSelectionList`, `ServerListEntryNormal`,
//! `ServerListEntryLanScan`), `GuiScreenAddServer`, `GuiScreenServerList`, `GuiYesNo`,
//! `GuiConnecting`, `GuiDisconnected`, `GuiIngameMenu`, `GuiOptions`, `GuiVideoSettings`,
//! `GuiControls` (+ `GuiKeyBindingList`), `GuiScreenOptionsSounds`, `ScreenChatOptions`,
//! `GuiCustomizeSkin`, `GuiLanguage` e `GuiGameOver`.
//!
//! Coordenadas, ids de botão e chaves de tradução são as do Java; os textos vêm do `.lang`.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Instant;

use mc_protocol::nbt::{read_root, Nbt};
use mc_protocol::Reader;
use mc_render::ui::{UiBatch, ICONS, PANORAMA, SERVER_SELECTION, TITLE, UNKNOWN_SERVER};
use winit::keyboard::KeyCode;

use crate::chat;
use crate::gui::{self, centered, string, Button, Kind, SlotList, TextField};
use crate::lang::tr;
use crate::pack_screen::{Click, PackScreen};
use crate::settings::{self, Opt, Settings, MODEL_PARTS};

#[derive(Clone, Debug, PartialEq)]
pub enum Screen {
    None,
    MainMenu,
    ServerList,
    /// `GuiScreenAddServer`; `Some(i)` edita o servidor `i`.
    AddServer(Option<usize>),
    /// `GuiScreenServerList` (conexão direta).
    DirectConnect,
    /// `GuiYesNo` de excluir o servidor `i`.
    ConfirmDelete(usize),
    Connecting(String),
    /// `GuiDownloadTerrain`: do S01 (ou S07 de outra dimensão) até o primeiro S08.
    DownloadTerrain,
    /// Conta: login Microsoft ou um nick offline (sem launcher, o jogo pede na primeira vez).
    Login,
    /// Chave do título (`connect.failed`/`disconnect.lost`) e o motivo.
    Disconnected(String, String),
    Pause,
    Options,
    Video,
    Controls,
    Sounds,
    Chat,
    Skin,
    Language,
    /// `GuiScreenResourcePacks`.
    ResourcePacks,
    /// `GuiYesNo` de pacote incompatível: id da entrada e se é mais novo que o jogo.
    PackConfirm(String, bool),
    /// `GuiYesNo` do pacote do servidor: URL e hash do S48.
    ServerPack(String, String),
    /// `GuiConfirmOpenLink` (clique em link do chat): a URL.
    LinkConfirm(String),
    /// `GuiScreenWorking` (baixando o pacote do servidor): título, mensagem e progresso em `Menu.working`.
    Working,
    /// `GuiGameOver`.
    GameOver,
    /// `GuiYesNo` do botão "Tela inicial" da tela de morte.
    GameOverConfirm,
}

pub enum Action {
    Join(String),
    Resume,
    Disconnect,
    Quit,
    /// Terminou a tela de pacotes com mudanças: recarregar os recursos.
    ReloadResources,
    /// Resposta do `GuiYesNo` do pacote do servidor.
    ServerPackAnswer(String, String, bool),
    /// Resposta do `GuiConfirmOpenLink`: URL, abrir e copiar (o chat reabre em seguida).
    LinkAnswer(String, bool, bool),
    /// `EntityPlayerSP.respawnPlayer`: C16 PERFORM_RESPAWN.
    Respawn,
    /// Sair do servidor para o menu principal (tela de morte).
    ToTitle,
    /// Tela de conta: login Microsoft pelo código de dispositivo, ou jogar offline com o nick.
    LoginMicrosoft,
    LoginOffline(String),
}

pub struct Status {
    pub motd: String,
    pub players: (i64, i64),
    pub ping_ms: u64,
    /// Ícone em base64 (sem o prefixo `data:`).
    pub favicon: Option<String>,
    pub protocol: i64,
    pub version: String,
    /// Nomes da amostra de jogadores (tooltip).
    pub sample: Vec<String>,
}

pub struct ServerEntry {
    pub name: String,
    pub ip: String,
    /// Ícone em base64 como no servers.dat.
    pub icon_b64: Option<String>,
    /// PNG 64x64 ainda não gravado no atlas.
    pub icon_png: Option<Vec<u8>>,
    /// Vaga no atlas da GUI quando já foi gravado.
    pub icon_slot: Option<u32>,
    /// `ServerData.ServerResourceMode`: 0 ligado, 1 desligado, 2 perguntar.
    pub resource_mode: u8,
    pub status: Option<Result<Status, String>>,
    last_click: Option<Instant>,
}

impl ServerEntry {
    pub fn new(name: String, ip: String) -> Self {
        Self { name, ip, icon_b64: None, icon_png: None, icon_slot: None, resource_mode: 2, status: None, last_click: None }
    }
}

/// Uma linha do `GuiKeyBindingList`.
enum KeyRow {
    Category(String),
    Key(usize),
}

pub struct Menu {
    pub screen: Screen,
    /// Telas-mãe (o `parentScreen` do Java).
    stack: Vec<Screen>,
    pub settings: Settings,
    pub servers: Vec<ServerEntry>,
    servers_path: PathBuf,
    pub mouse: (f32, f32),
    /// Há um mundo carregado (`mc.theWorld != null`).
    pub in_game: bool,
    pub splash: String,
    /// Sons de interface a tocar (o jogo esvazia).
    pub sounds: Vec<&'static str>,
    /// O idioma mudou (o jogo recarrega os nomes).
    pub language_changed: bool,
    /// Estado da tela de pacotes de recursos, enquanto aberta.
    pub packs: Option<PackScreen>,
    /// `GuiScreenWorking`: (título, mensagem, progresso %).
    pub working: (String, String, i32),
    /// Jogador local morto (`thePlayer.getHealth() <= 0`): `displayGuiScreen(null)` vira `GuiGameOver`.
    pub dead: bool,
    /// `WorldInfo.isHardcoreModeEnabled` e `EntityPlayer.getScore`, para a tela de morte.
    pub hardcore: bool,
    pub score: i32,
    /// Ticks desde que a tela atual abriu (`enableButtonsTimer`, `ticksUntilEnable`).
    screen_ticks: i32,
    w: i32,
    h: i32,
    inited: Option<(Screen, i32, i32)>,
    buttons: Vec<Button>,
    /// Botões dentro da lista (linhas do vídeo; trocar/reiniciar dos controles).
    rows: Vec<Button>,
    list: SlotList,
    fields: Vec<TextField>,
    draft: Option<ServerEntry>,
    hovering: Option<String>,
    key_rows: Vec<KeyRow>,
    max_label: i32,
    /// Tecla esperando o novo valor (`GuiControls.buttonId`).
    binding: Option<usize>,
    langs: Vec<(String, String)>,
    mouse_down: bool,
    ping_tx: mpsc::Sender<(usize, String, Result<Status, String>)>,
    ping_rx: mpsc::Receiver<(usize, String, Result<Status, String>)>,
    /// Conta em uso (nick) e as linhas de aviso da tela de conta (o código do login Microsoft, erros).
    pub account: String,
    pub login_status: Vec<String>,
}

fn b64(s: &str) -> Option<Vec<u8>> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    };
    let bytes: Vec<u8> = s.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=').collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let mut n = 0u32;
        for (i, c) in chunk.iter().enumerate() {
            n |= val(*c)? << (18 - 6 * i);
        }
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    Some(out)
}

/// Lê o `servers.dat` (NBT sem compressão).
pub fn load_servers(path: &Path) -> Vec<ServerEntry> {
    let Ok(bytes) = std::fs::read(path) else { return Vec::new() };
    let Ok(Some(root)) = read_root(&mut Reader::new(&bytes)) else { return Vec::new() };
    let Some(Nbt::List(list)) = root.get("servers") else { return Vec::new() };
    list.iter()
        .filter_map(|s| {
            let ip = s.get("ip")?.as_str()?.to_string();
            let name = s.get("name").and_then(Nbt::as_str).unwrap_or(&ip).to_string();
            let mut e = ServerEntry::new(name, ip);
            e.icon_b64 = s.get("icon").and_then(Nbt::as_str).map(str::to_string);
            e.icon_png = e.icon_b64.as_deref().and_then(b64);
            e.resource_mode = match s.get("acceptTextures") {
                Some(Nbt::Byte(0)) => 1,
                Some(Nbt::Byte(_)) => 0,
                _ => 2,
            };
            Some(e)
        })
        .collect()
}

/// `ServerList.saveServerList`: compound raiz com a lista `servers`.
pub fn servers_nbt(servers: &[ServerEntry]) -> Vec<u8> {
    fn put_str(out: &mut Vec<u8>, s: &str) {
        out.extend_from_slice(&(s.len() as u16).to_be_bytes());
        out.extend_from_slice(s.as_bytes());
    }
    fn tag(out: &mut Vec<u8>, kind: u8, name: &str) {
        out.push(kind);
        put_str(out, name);
    }
    let mut out = Vec::new();
    tag(&mut out, 10, "");
    tag(&mut out, 9, "servers");
    out.push(10);
    out.extend_from_slice(&(servers.len() as i32).to_be_bytes());
    for s in servers {
        // ServerData.getNBTCompound: name, ip, icon, acceptTextures.
        tag(&mut out, 8, "name");
        put_str(&mut out, &s.name);
        tag(&mut out, 8, "ip");
        put_str(&mut out, &s.ip);
        if let Some(icon) = &s.icon_b64 {
            tag(&mut out, 8, "icon");
            put_str(&mut out, icon);
        }
        if s.resource_mode != 2 {
            tag(&mut out, 1, "acceptTextures");
            out.push((s.resource_mode == 0) as u8);
        }
        out.push(0);
    }
    out.push(0);
    out
}

fn parse_status(json: &str, ping_ms: u64) -> Status {
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
    let motd = chat::to_legacy(&v["description"].to_string());
    let players = (v["players"]["online"].as_i64().unwrap_or(0), v["players"]["max"].as_i64().unwrap_or(0));
    let favicon = v["favicon"].as_str().and_then(|f| f.strip_prefix("data:image/png;base64,")).map(|s| s.replace('\n', ""));
    let protocol = v["version"]["protocol"].as_i64().unwrap_or(47);
    let version = v["version"]["name"].as_str().unwrap_or("").to_string();
    let sample = v["players"]["sample"]
        .as_array()
        .map(|a| a.iter().filter_map(|p| p["name"].as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    Status { motd, players, ping_ms, favicon, protocol, version, sample }
}

/// Frase do menu principal: uma linha aleatória do `texts/splashes.txt` (`GuiMainMenu`).
pub fn pick_splash(text: &str) -> String {
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    if lines.is_empty() {
        return "missingno".into();
    }
    // O Java sorteia de novo enquanto cai na frase de hash 125780783 ("This message will never appear...").
    loop {
        let l = lines[rand::random::<usize>() % lines.len()];
        if java_hash(l) != 125_780_783 || lines.len() == 1 {
            return l.to_string();
        }
    }
}

fn java_hash(s: &str) -> i32 {
    s.encode_utf16().fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(c as i32))
}

/// Ordem das teclas no `GuiKeyBindingList` (`KeyBinding.compareTo` pelos textos traduzidos).
fn key_order(s: &Settings) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..s.keys.len()).collect();
    idx.sort_by(|&a, &b| {
        let (ka, kb) = (&s.keys[a], &s.keys[b]);
        tr(ka.category).cmp(&tr(kb.category)).then_with(|| tr(ka.desc).cmp(&tr(kb.desc)))
    });
    idx
}

impl Menu {
    pub fn new(screen: Screen, settings: Settings, servers_path: PathBuf) -> Self {
        let (ping_tx, ping_rx) = mpsc::channel();
        let servers = load_servers(&servers_path);
        let mut m = Menu {
            screen,
            stack: Vec::new(),
            settings,
            servers,
            servers_path,
            mouse: (0.0, 0.0),
            in_game: false,
            splash: String::new(),
            sounds: Vec::new(),
            language_changed: false,
            packs: None,
            working: (String::new(), String::new(), 0),
            dead: false,
            hardcore: false,
            score: 0,
            screen_ticks: 0,
            w: 0,
            h: 0,
            inited: None,
            buttons: Vec::new(),
            rows: Vec::new(),
            list: SlotList::new(0, 0, 0, 0, 1),
            fields: Vec::new(),
            draft: None,
            hovering: None,
            key_rows: Vec::new(),
            max_label: 0,
            binding: None,
            langs: Vec::new(),
            mouse_down: false,
            ping_tx,
            ping_rx,
            account: String::new(),
            login_status: Vec::new(),
        };
        if m.screen == Screen::ServerList {
            m.stack.push(Screen::MainMenu);
        }
        m.refresh();
        m
    }

    /// Grava a lista de servidores.
    pub fn save_servers(&self) {
        if let Some(dir) = self.servers_path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&self.servers_path, servers_nbt(&self.servers)) {
            eprintln!("não gravei {}: {e}", self.servers_path.display());
        }
    }

    /// Pinga todos os servidores em segundo plano.
    pub fn refresh(&mut self) {
        for (i, s) in self.servers.iter_mut().enumerate() {
            s.status = None;
            let (tx, ip) = (self.ping_tx.clone(), s.ip.clone());
            std::thread::spawn(move || {
                let res = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| e.to_string())
                    .and_then(|rt| {
                        rt.block_on(async { tokio::time::timeout(std::time::Duration::from_secs(8), crate::net::status(&ip)).await })
                            .map_err(|_| "timeout".to_string())?
                            .map_err(|e| e.to_string())
                    })
                    .map(|(json, ms)| parse_status(&json, ms));
                let _ = tx.send((i, ip, res));
            });
        }
    }

    /// Respostas de ping; devolve ícones novos para gravar no atlas (índice, png).
    pub fn poll(&mut self) -> Vec<(usize, Vec<u8>)> {
        let mut icons = Vec::new();
        let mut save = false;
        while let Ok((i, ip, res)) = self.ping_rx.try_recv() {
            let Some(s) = self.servers.get_mut(i) else { continue };
            if s.ip != ip {
                continue;
            }
            if let Ok(st) = &res {
                // ServerListEntryNormal: ícone novo vai para o servers.dat.
                if st.favicon.is_some() && st.favicon != s.icon_b64 {
                    s.icon_b64 = st.favicon.clone();
                    if let Some(png) = st.favicon.as_deref().and_then(b64) {
                        icons.push((i, png));
                    }
                    save = true;
                }
            }
            s.status = Some(res);
        }
        if save {
            self.save_servers();
        }
        icons
    }

    fn click_sound(&mut self) {
        self.sounds.push("gui.button.press");
    }

    /// `mc.displayGuiScreen(child)` guardando a tela atual como mãe.
    pub fn open(&mut self, s: Screen) {
        let cur = std::mem::replace(&mut self.screen, s);
        self.stack.push(cur);
        self.inited = None;
    }

    /// Recarga de recursos: o atlas da GUI foi refeito, então favicons e ícones de pacote voltam à fila.
    pub fn reset_icons(&mut self) {
        for s in &mut self.servers {
            s.icon_slot = None;
            s.icon_png = s.icon_b64.as_deref().and_then(b64);
        }
        if let Some(p) = &mut self.packs {
            p.reset_icons();
        }
    }

    /// `GuiButton 105`: abre a tela de pacotes com o que há na pasta e o que está escolhido.
    pub fn open_packs(&mut self) {
        let mut jar = crate::res::jar();
        let default_icon = jar.as_mut().and_then(|j| j.default_icon());
        let description = jar.as_mut().and_then(|j| j.default_meta()).map(|m| m.description);
        let all = mc_render::resources::scan(&mc_render::resources::resourcepacks_dir(), default_icon.as_deref());
        let chosen = mc_render::resources::resolve_selected(&all, &self.settings.resource_packs, &self.settings.incompatible_resource_packs);
        let description = description.unwrap_or_else(|| "§cMissing pack.mcmeta :(".to_string());
        self.packs = Some(PackScreen::new(self.w, self.h, &all, &chosen, description));
        self.open(Screen::ResourcePacks);
    }

    /// Volta para a tela-mãe.
    fn back(&mut self) {
        self.screen = self.stack.pop().unwrap_or(if self.in_game { Screen::None } else { Screen::MainMenu });
        self.inited = None;
        if self.screen == Screen::ServerList {
            self.list.selected = -1;
        }
    }

    /// Troca a tela sem mãe (o jogo usa ao conectar, cair ou sair).
    pub fn show(&mut self, s: Screen) {
        self.stack.clear();
        match &s {
            Screen::ServerList | Screen::Connecting(_) => self.stack.push(Screen::MainMenu),
            Screen::Disconnected(..) => self.stack.extend([Screen::MainMenu, Screen::ServerList]),
            _ => {}
        }
        if s == Screen::ServerList {
            self.refresh();
        }
        self.screen = s;
        self.inited = None;
        self.screen_ticks = 0;
    }

    /// `updateScreen` das telas que contam ticks: os botões da tela de morte e do `GuiYesNo` dela
    /// só ligam depois de 20 ticks (`enableButtonsTimer == 20`, `setButtonDelay(20)`).
    pub fn tick(&mut self) {
        self.screen_ticks += 1;
        if matches!(self.screen, Screen::GameOver | Screen::GameOverConfirm) && self.screen_ticks == 20 {
            for b in &mut self.buttons {
                b.enabled = true;
            }
        }
    }

    /// `displayGuiScreen(null)`: volta ao jogo ou, sem mundo, ao menu principal.
    fn close(&mut self) -> Option<Action> {
        self.settings.save();
        if self.in_game && self.dead {
            // Minecraft.displayGuiScreen: null com o jogador morto abre o GuiGameOver.
            self.show(Screen::GameOver);
            return None;
        }
        if self.in_game {
            self.stack.clear();
            self.screen = Screen::None;
            self.inited = None;
            Some(Action::Resume)
        } else {
            self.show(Screen::MainMenu);
            None
        }
    }

    fn ints(&self) -> (i32, i32) {
        (self.mouse.0.floor() as i32, self.mouse.1.floor() as i32)
    }

    // ---------------------------------------------------------------- initGui

    fn init(&mut self, ui: &mut UiBatch) {
        let (w, h) = (self.w, self.h);
        let s = &self.settings;
        let mut b: Vec<Button> = Vec::new();
        self.rows.clear();
        self.fields.clear();
        self.hovering = None;
        match self.screen.clone() {
            Screen::None => {}
            Screen::MainMenu => {
                // GuiMainMenu.initGui + addSingleplayerMultiplayerButtons (sem mundo local: desativado).
                let j = h / 4 + 48;
                b.push(Button::new(1, w / 2 - 100, j, tr("menu.singleplayer")).disabled());
                b.push(Button::new(2, w / 2 - 100, j + 24, tr("menu.multiplayer")));
                // No lugar do Realms (desativado na 1.8.9 sem conta): a conta do jogador.
                b.push(Button::new(14, w / 2 - 100, j + 48, "Conta"));
                b.push(Button::sized(0, w / 2 - 100, j + 72 + 12, 98, 20, tr("menu.options")));
                b.push(Button::sized(4, w / 2 + 2, j + 72 + 12, 98, 20, tr("menu.quit")));
                let mut lang = Button::sized(5, w / 2 - 124, j + 72 + 12, 20, 20, "");
                lang.kind = Kind::Language;
                b.push(lang);
            }
            Screen::ServerList => {
                // GuiMultiplayer.createButtons.
                let mut list = SlotList::new(w, h, 32, h - 64, 36);
                list.selected = self.list.selected.min(self.servers.len() as i32);
                if self.inited.as_ref().is_some_and(|(sc, _, _)| *sc == Screen::ServerList) {
                    list.scroll = self.list.scroll;
                }
                self.list = list;
                b.push(Button::sized(7, w / 2 - 154, h - 28, 70, 20, tr("selectServer.edit")));
                b.push(Button::sized(2, w / 2 - 74, h - 28, 70, 20, tr("selectServer.delete")));
                b.push(Button::sized(1, w / 2 - 154, h - 52, 100, 20, tr("selectServer.select")));
                b.push(Button::sized(4, w / 2 - 50, h - 52, 100, 20, tr("selectServer.direct")));
                b.push(Button::sized(3, w / 2 + 4 + 50, h - 52, 100, 20, tr("selectServer.add")));
                b.push(Button::sized(8, w / 2 + 4, h - 28, 70, 20, tr("selectServer.refresh")));
                b.push(Button::sized(0, w / 2 + 4 + 76, h - 28, 75, 20, tr("gui.cancel")));
                self.buttons = b;
                self.select_server(self.list.selected);
                return;
            }
            Screen::AddServer(_) => {
                let d = self.draft.get_or_insert_with(|| ServerEntry::new(tr("selectServer.defaultName"), String::new()));
                let mode = ["addServer.resourcePack.enabled", "addServer.resourcePack.disabled", "addServer.resourcePack.prompt"][d.resource_mode as usize % 3];
                b.push(Button::new(0, w / 2 - 100, h / 4 + 96 + 18, tr("addServer.add")));
                b.push(Button::new(1, w / 2 - 100, h / 4 + 120 + 18, tr("gui.cancel")));
                b.push(Button::new(2, w / 2 - 100, h / 4 + 72, format!("{}: {}", tr("addServer.resourcePack"), tr(mode))));
                let mut name = TextField::new(w / 2 - 100, 66, 200, 20);
                name.focused = true;
                let (dn, di) = (d.name.clone(), d.ip.clone());
                name.set_text(ui, &dn);
                let mut ip = TextField::new(w / 2 - 100, 106, 200, 20);
                ip.max_len = 128;
                ip.set_text(ui, &di);
                self.fields = vec![name, ip];
            }
            Screen::Login => {
                let y = h / 4;
                b.push(Button::new(0, w / 2 - 100, y + 24, "Entrar com Microsoft"));
                b.push(Button::new(1, w / 2 - 100, y + 96, "Jogar offline"));
                if !self.stack.is_empty() {
                    b.push(Button::new(2, w / 2 - 100, y + 128, tr("gui.cancel")));
                }
                let mut f = TextField::new(w / 2 - 100, y + 70, 200, 20);
                f.max_len = 16;
                f.focused = true;
                let nick = if crate::msa::valid_name(&self.account) { self.account.clone() } else { String::new() };
                f.set_text(ui, &nick);
                self.fields = vec![f];
                if let Some(b) = b.get_mut(1) {
                    b.enabled = crate::msa::valid_name(&nick);
                }
            }
            Screen::DirectConnect => {
                b.push(Button::new(0, w / 2 - 100, h / 4 + 96 + 12, tr("selectServer.select")));
                b.push(Button::new(1, w / 2 - 100, h / 4 + 120 + 12, tr("gui.cancel")));
                let mut f = TextField::new(w / 2 - 100, 116, 200, 20);
                f.max_len = 128;
                f.focused = true;
                f.set_text(ui, &s.last_server);
                self.fields = vec![f];
            }
            Screen::ConfirmDelete(_) => {
                // GuiYesNo com GuiOptionButton (150 de largura).
                b.push(Button::sized(0, w / 2 - 155, h / 6 + 96, 150, 20, tr("selectServer.deleteButton")));
                b.push(Button::sized(1, w / 2 - 155 + 160, h / 6 + 96, 150, 20, tr("gui.cancel")));
            }
            Screen::Connecting(_) => {
                b.push(Button::new(0, w / 2 - 100, h / 4 + 120 + 12, tr("gui.cancel")));
            }
            Screen::Disconnected(_, reason) => {
                let lines = gui::wrap(ui, &reason, w - 50);
                let th = lines.len() as i32 * 9;
                b.push(Button::new(0, w / 2 - 100, h / 2 + th / 2 + 9, tr("gui.toMenu")));
            }
            Screen::Pause => {
                // GuiIngameMenu (1.8.9 sem Forge), i = -16.
                let i = -16;
                b.push(Button::new(1, w / 2 - 100, h / 4 + 120 + i, tr("menu.disconnect")));
                b.push(Button::new(4, w / 2 - 100, h / 4 + 24 + i, tr("menu.returnToGame")));
                b.push(Button::sized(0, w / 2 - 100, h / 4 + 96 + i, 98, 20, tr("menu.options")));
                b.push(Button::sized(7, w / 2 + 2, h / 4 + 96 + i, 98, 20, tr("menu.shareToLan")).disabled());
                // Conquistas e estatísticas dependem de dados do servidor ainda não portados.
                b.push(Button::sized(5, w / 2 - 100, h / 4 + 48 + i, 98, 20, tr("gui.achievements")).disabled());
                b.push(Button::sized(6, w / 2 + 2, h / 4 + 48 + i, 98, 20, tr("gui.stats")).disabled());
            }
            Screen::Options => {
                let mut n = 0;
                b.push(Button::option(w / 2 - 155 + n % 2 * 160, h / 6 - 12 + 24 * (n >> 1), Opt::Fov, s));
                n += 1;
                let (x, y) = (w / 2 - 155 + n % 2 * 160, h / 6 - 12 + 24 * (n >> 1));
                if self.in_game {
                    let d = ["options.difficulty.peaceful", "options.difficulty.easy", "options.difficulty.normal", "options.difficulty.hard"]
                        [s.difficulty.clamp(0, 3) as usize];
                    b.push(Button::sized(108, x, y, 150, 20, format!("{}: {}", tr("options.difficulty"), tr(d))).disabled());
                } else {
                    b.push(Button::option(x, y, Opt::RealmsNotifications, s));
                }
                b.push(Button::sized(110, w / 2 - 155, h / 6 + 48 - 6, 150, 20, tr("options.skinCustomisation")));
                b.push(Button::sized(8675309, w / 2 + 5, h / 6 + 48 - 6, 150, 20, "Super Secret Settings..."));
                b.push(Button::sized(106, w / 2 - 155, h / 6 + 72 - 6, 150, 20, tr("options.sounds")));
                b.push(Button::sized(107, w / 2 + 5, h / 6 + 72 - 6, 150, 20, tr("options.stream")).disabled());
                b.push(Button::sized(101, w / 2 - 155, h / 6 + 96 - 6, 150, 20, tr("options.video")));
                b.push(Button::sized(100, w / 2 + 5, h / 6 + 96 - 6, 150, 20, tr("options.controls")));
                b.push(Button::sized(102, w / 2 - 155, h / 6 + 120 - 6, 150, 20, tr("options.language")));
                b.push(Button::sized(103, w / 2 + 5, h / 6 + 120 - 6, 150, 20, tr("options.chat.title")));
                b.push(Button::sized(105, w / 2 - 155, h / 6 + 144 - 6, 150, 20, tr("options.resourcepack")));
                b.push(Button::sized(104, w / 2 + 5, h / 6 + 144 - 6, 150, 20, tr("options.snooper.view")).disabled());
                b.push(Button::new(200, w / 2 - 100, h / 6 + 168, tr("gui.done")));
            }
            Screen::Video => {
                b.push(Button::new(200, w / 2 - 100, h - 27, tr("gui.done")));
                // GuiOptionsRowList: pares nas colunas w/2-155 e w/2+5.
                const VIDEO: [Opt; 18] = [
                    Opt::Graphics,
                    Opt::RenderDistance,
                    Opt::AmbientOcclusion,
                    Opt::FramerateLimit,
                    Opt::Anaglyph,
                    Opt::ViewBobbing,
                    Opt::GuiScale,
                    Opt::Gamma,
                    Opt::RenderClouds,
                    Opt::Particles,
                    Opt::UseFullscreen,
                    Opt::EnableVsync,
                    Opt::MipmapLevels,
                    Opt::BlockAlternatives,
                    Opt::UseVbo,
                    Opt::EntityShadows,
                    Opt::Lod,
                    Opt::LodDistance,
                ];
                let scroll = if self.inited.as_ref().is_some_and(|(sc, _, _)| *sc == Screen::Video) { self.list.scroll } else { 0.0 };
                self.list = SlotList::new(w, h, 32, h - 32, 25);
                self.list.scroll = scroll;
                for (i, o) in VIDEO.iter().enumerate() {
                    self.rows.push(Button::option(w / 2 - 155 + (i as i32 % 2) * 160, 0, *o, s));
                }
            }
            Screen::Controls => {
                b.push(Button::sized(200, w / 2 - 155, h - 29, 150, 20, tr("gui.done")));
                b.push(Button::sized(201, w / 2 - 155 + 160, h - 29, 150, 20, tr("controls.resetAll")));
                for (n, o) in [Opt::InvertMouse, Opt::Sensitivity, Opt::Touchscreen].iter().enumerate() {
                    let n = n as i32;
                    b.push(Button::option(w / 2 - 155 + n % 2 * 160, 18 + 24 * (n >> 1), *o, s));
                }
                self.list = SlotList::new(w, h, 63, h - 32, 20);
                self.key_rows.clear();
                self.max_label = 0;
                let mut cat: Option<&str> = None;
                for k in key_order(s) {
                    let kb = &s.keys[k];
                    if cat != Some(kb.category) {
                        cat = Some(kb.category);
                        self.key_rows.push(KeyRow::Category(tr(kb.category)));
                    }
                    self.max_label = self.max_label.max(gui::width(ui, &tr(kb.desc)));
                    self.key_rows.push(KeyRow::Key(k));
                    // Botões da linha: trocar (75) e reiniciar (50); x/y vêm no desenho.
                    self.rows.push(Button::sized(0, 0, 0, 75, 20, tr(kb.desc)));
                    self.rows.push(Button::sized(0, 0, 0, 50, 20, tr("controls.reset")));
                }
            }
            Screen::Sounds => {
                let mut n = 0;
                let mut master = Button::sized(0, w / 2 - 155 + n % 2 * 160, h / 6 - 12 + 24 * (n >> 1), 310, 20, s.sound_display(0));
                master.kind = Kind::Sound(0);
                master.value = s.sound[0];
                b.push(master);
                n += 2;
                for cat in 1..settings::SOUND_CATEGORIES.len() {
                    let mut sb = Button::sized(cat as i32, w / 2 - 155 + n % 2 * 160, h / 6 - 12 + 24 * (n >> 1), 150, 20, s.sound_display(cat));
                    sb.kind = Kind::Sound(cat);
                    sb.value = s.sound[cat];
                    b.push(sb);
                    n += 1;
                }
                b.push(Button::new(200, w / 2 - 100, h / 6 + 168, tr("gui.done")));
            }
            Screen::Chat => {
                const CHAT: [Opt; 10] = [
                    Opt::ChatVisibility,
                    Opt::ChatColor,
                    Opt::ChatLinks,
                    Opt::ChatOpacity,
                    Opt::ChatLinksPrompt,
                    Opt::ChatScale,
                    Opt::ChatHeightFocused,
                    Opt::ChatHeightUnfocused,
                    Opt::ChatWidth,
                    Opt::ReducedDebugInfo,
                ];
                for (n, o) in CHAT.iter().enumerate() {
                    let n = n as i32;
                    b.push(Button::option(w / 2 - 155 + n % 2 * 160, h / 6 + 24 * (n >> 1), *o, s));
                }
                b.push(Button::new(200, w / 2 - 100, h / 6 + 120, tr("gui.done")));
            }
            Screen::Skin => {
                let mut n = 0;
                for part in 0..MODEL_PARTS.len() {
                    let mut pb = Button::sized(part as i32, w / 2 - 155 + n % 2 * 160, h / 6 + 24 * (n >> 1), 150, 20, s.part_display(part));
                    pb.kind = Kind::Part(part);
                    b.push(pb);
                    n += 1;
                }
                if n % 2 == 1 {
                    n += 1;
                }
                b.push(Button::new(200, w / 2 - 100, h / 6 + 24 * (n >> 1), tr("gui.done")));
            }
            Screen::ResourcePacks => {
                // GuiScreenResourcePacks.initGui: abrir pasta (2) e concluído (1), GuiOptionButton de 150.
                b.push(Button::sized(2, w / 2 - 154, h - 48, 150, 20, tr("resourcePack.openFolder")));
                b.push(Button::sized(1, w / 2 + 4, h - 48, 150, 20, tr("gui.done")));
                if let Some(p) = &mut self.packs {
                    p.layout(w, h);
                }
            }
            Screen::PackConfirm(..) => {
                // GuiYesNo: botões gui.yes / gui.no.
                b.push(Button::sized(0, w / 2 - 155, h / 6 + 96, 150, 20, tr("gui.yes")));
                b.push(Button::sized(1, w / 2 - 155 + 160, h / 6 + 96, 150, 20, tr("gui.no")));
            }
            Screen::LinkConfirm(_) => {
                // GuiConfirmOpenLink.initGui: sim (0), copiar (2) e não (1), 100x20.
                b.push(Button::sized(0, w / 2 - 50 - 105, h / 6 + 96, 100, 20, tr("gui.yes")));
                b.push(Button::sized(2, w / 2 - 50, h / 6 + 96, 100, 20, tr("chat.copy")));
                b.push(Button::sized(1, w / 2 - 50 + 105, h / 6 + 96, 100, 20, tr("gui.no")));
            }
            Screen::ServerPack(..) => {
                b.push(Button::sized(0, w / 2 - 155, h / 6 + 96, 150, 20, tr("gui.yes")));
                b.push(Button::sized(1, w / 2 - 155 + 160, h / 6 + 96, 150, 20, tr("gui.no")));
            }
            Screen::Working | Screen::DownloadTerrain => {}
            Screen::GameOver => {
                if self.hardcore {
                    // Sem servidor integrado: sempre "Sair do servidor".
                    b.push(Button::new(1, w / 2 - 100, h / 4 + 96, tr("deathScreen.leaveServer")));
                } else {
                    b.push(Button::new(0, w / 2 - 100, h / 4 + 72, tr("deathScreen.respawn")));
                    b.push(Button::new(1, w / 2 - 100, h / 4 + 96, tr("deathScreen.titleScreen")));
                }
                // initGui desliga todos; updateScreen religa no tick 20.
                for x in &mut b {
                    x.enabled = self.screen_ticks >= 20;
                }
            }
            Screen::GameOverConfirm => {
                b.push(Button::sized(0, w / 2 - 155, h / 6 + 96, 150, 20, tr("deathScreen.titleScreen")));
                b.push(Button::sized(1, w / 2 - 155 + 160, h / 6 + 96, 150, 20, tr("deathScreen.respawn")));
                for x in &mut b {
                    x.enabled = self.screen_ticks >= 20;
                }
            }
            Screen::Language => {
                b.push(Button::option(w / 2 - 155, h - 38, Opt::ForceUnicodeFont, s));
                b.push(Button::sized(6, w / 2 - 155 + 160, h - 38, 150, 20, tr("gui.done")));
                if self.langs.is_empty() {
                    self.langs = crate::lang::available();
                }
                self.list = SlotList::new(w, h, 32, h - 65 + 4, 18);
                self.list.selected = self.langs.iter().position(|(c, _)| *c == s.language).map_or(-1, |i| i as i32);
            }
        }
        self.buttons = b;
    }

    /// `GuiMultiplayer.selectServer`.
    fn select_server(&mut self, i: i32) {
        self.list.selected = i;
        let normal = i >= 0 && (i as usize) < self.servers.len();
        for b in &mut self.buttons {
            if matches!(b.id, 1 | 7 | 2) {
                b.enabled = normal;
            }
        }
    }

    // ---------------------------------------------------------------- drawScreen

    pub fn draw(&mut self, ui: &mut UiBatch, w: f32, h: f32) {
        self.w = w.ceil() as i32;
        self.h = h.ceil() as i32;
        let key = (self.screen.clone(), self.w, self.h);
        if self.inited.as_ref() != Some(&key) {
            self.init(ui);
            self.inited = Some(key);
        }
        let (w, h) = (self.w, self.h);
        let (mx, my) = self.ints();
        let in_game = self.in_game;
        match self.screen.clone() {
            Screen::None => return,
            Screen::MainMenu => self.draw_main_menu(ui),
            Screen::ServerList => {
                self.hovering = None;
                gui::default_background(ui, w, h, in_game);
                self.draw_server_list(ui);
                centered(ui, &tr("multiplayer.title"), w / 2, 20, 0xFFFFFF);
            }
            Screen::AddServer(_) => {
                gui::default_background(ui, w, h, in_game);
                centered(ui, &tr("addServer.title"), w / 2, 17, 0xFFFFFF);
                string(ui, &tr("addServer.enterName"), w / 2 - 100, 53, 0xA0A0A0);
                string(ui, &tr("addServer.enterIp"), w / 2 - 100, 94, 0xA0A0A0);
                for f in &self.fields {
                    f.draw(ui);
                }
            }
            Screen::Login => {
                gui::default_background(ui, w, h, in_game);
                centered(ui, "Conta", w / 2, 20, 0xFFFFFF);
                let atual = if self.account.is_empty() { "nenhuma".to_string() } else { self.account.clone() };
                centered(ui, &format!("Conta atual: {atual}"), w / 2, 40, 0xA0A0A0);
                string(ui, "Nick para jogar offline:", w / 2 - 100, h / 4 + 58, 0xA0A0A0);
                for f in &self.fields {
                    f.draw(ui);
                }
                // Abaixo do último botão, sem passar da borda de baixo.
                let lines = self.login_status.len() as i32;
                let below = h / 4 + if self.stack.is_empty() { 122 } else { 154 };
                let mut y = below.min(h - 12 * lines - 4);
                for l in self.login_status.clone() {
                    centered(ui, &l, w / 2, y, 0xFFFFFF);
                    y += 12;
                }
            }
            Screen::DirectConnect => {
                gui::default_background(ui, w, h, in_game);
                centered(ui, &tr("selectServer.direct"), w / 2, 20, 0xFFFFFF);
                string(ui, &tr("addServer.enterIp"), w / 2 - 100, 100, 0xA0A0A0);
                for f in &self.fields {
                    f.draw(ui);
                }
            }
            Screen::ConfirmDelete(i) => {
                gui::default_background(ui, w, h, in_game);
                centered(ui, &tr("selectServer.deleteQuestion"), w / 2, 70, 0xFFFFFF);
                let name = self.servers.get(i).map_or(String::new(), |s| s.name.clone());
                let msg = format!("'{name}' {}", tr("selectServer.deleteWarning"));
                let mut y = 90;
                for l in gui::wrap(ui, &msg, w - 50) {
                    centered(ui, &l, w / 2, y, 0xFFFFFF);
                    y += 9;
                }
            }
            Screen::Connecting(_) => {
                gui::default_background(ui, w, h, in_game);
                centered(ui, &tr("connect.connecting"), w / 2, h / 2 - 50, 0xFFFFFF);
            }
            // drawBackground(0): sempre a terra, mesmo com o mundo carregado.
            Screen::DownloadTerrain => {
                gui::default_background(ui, w, h, false);
                centered(ui, &tr("multiplayer.downloadingTerrain"), w / 2, h / 2 - 50, 0xFFFFFF);
            }
            Screen::Disconnected(title, reason) => {
                gui::default_background(ui, w, h, in_game);
                let lines = gui::wrap(ui, &reason, w - 50);
                let th = lines.len() as i32 * 9;
                centered(ui, &tr(&title), w / 2, h / 2 - th / 2 - 9 * 2, 0xAAAAAA);
                let mut y = h / 2 - th / 2;
                for l in &lines {
                    centered(ui, l, w / 2, y, 0xFFFFFF);
                    y += 9;
                }
            }
            Screen::Pause => {
                gui::default_background(ui, w, h, in_game);
                centered(ui, &tr("menu.game"), w / 2, 40, 0xFFFFFF);
            }
            Screen::Options => {
                gui::default_background(ui, w, h, in_game);
                centered(ui, &tr("options.title"), w / 2, 15, 0xFFFFFF);
            }
            Screen::Video => {
                gui::default_background(ui, w, h, in_game);
                let n = self.rows.len().div_ceil(2);
                self.list.draw_background(ui, n);
                for r in 0..n {
                    let y = self.list.entry_y(r);
                    for b in self.rows.iter_mut().skip(r * 2).take(2) {
                        b.y = y;
                    }
                }
                for b in &self.rows {
                    b.draw(ui, mx, my);
                }
                self.list.draw_foreground(ui, n, w / 2 + 124 + 32);
                centered(ui, &tr("options.videoTitle"), w / 2, 5, 0xFFFFFF);
            }
            Screen::Controls => {
                gui::default_background(ui, w, h, in_game);
                self.draw_key_list(ui, mx, my);
                centered(ui, &tr("controls.title"), w / 2, 8, 0xFFFFFF);
                let changed = self.settings.keys.iter().any(|k| k.code != k.default);
                if let Some(b) = self.buttons.iter_mut().find(|b| b.id == 201) {
                    b.enabled = changed;
                }
            }
            Screen::Sounds => {
                gui::default_background(ui, w, h, in_game);
                centered(ui, &tr("options.sounds.title"), w / 2, 15, 0xFFFFFF);
            }
            Screen::Chat => {
                gui::default_background(ui, w, h, in_game);
                centered(ui, &tr("options.chat.title"), w / 2, 20, 0xFFFFFF);
            }
            Screen::Skin => {
                gui::default_background(ui, w, h, in_game);
                centered(ui, &tr("options.skinCustomisation.title"), w / 2, 20, 0xFFFFFF);
            }
            Screen::ResourcePacks => {
                // drawBackground(0): terra sempre, mesmo com mundo.
                gui::tile(ui, 0, 0, w, h, 0, 0x404040, 255);
                if let Some(p) = &mut self.packs {
                    p.draw(ui, mx, my, self.settings.touchscreen);
                }
                centered(ui, &tr("resourcePack.title"), w / 2, 16, 0xFFFFFF);
                centered(ui, &tr("resourcePack.folderInfo"), w / 2 - 77, h - 26, 0x808080);
            }
            Screen::PackConfirm(_, newer) => {
                gui::default_background(ui, w, h, in_game);
                centered(ui, &tr("resourcePack.incompatible.confirm.title"), w / 2, 70, 0xFFFFFF);
                let key = format!("resourcePack.incompatible.confirm.{}", if newer { "new" } else { "old" });
                let mut y = 90;
                for l in gui::wrap(ui, &tr(&key), w - 50) {
                    centered(ui, &l, w / 2, y, 0xFFFFFF);
                    y += 9;
                }
            }
            Screen::LinkConfirm(url) => {
                gui::default_background(ui, w, h, in_game);
                centered(ui, &tr("chat.link.confirm"), w / 2, 70, 0xFFFFFF);
                let mut y = 90;
                for l in gui::wrap(ui, &url, w - 50) {
                    centered(ui, &l, w / 2, y, 0xFFFFFF);
                    y += 9;
                }
                centered(ui, &tr("chat.link.warning"), w / 2, 110, 16764108);
            }
            Screen::ServerPack(..) => {
                gui::default_background(ui, w, h, in_game);
                centered(ui, &tr("multiplayer.texturePrompt.line1"), w / 2, 70, 0xFFFFFF);
                let mut y = 90;
                for l in gui::wrap(ui, &tr("multiplayer.texturePrompt.line2"), w - 50) {
                    centered(ui, &l, w / 2, y, 0xFFFFFF);
                    y += 9;
                }
            }
            Screen::Working => {
                gui::default_background(ui, w, h, in_game);
                let (title, msg, pct) = self.working.clone();
                centered(ui, &title, w / 2, 70, 0xFFFFFF);
                centered(ui, &format!("{msg} {pct}%"), w / 2, 90, 0xFFFFFF);
            }
            Screen::GameOver => {
                // GuiGameOver.drawScreen: degradê vermelho, título em escala 2, pontuação.
                gui::gradient(ui, 0, 0, w, h, 0x6050_0000, 0xA080_3030);
                let title = tr(if self.hardcore { "deathScreen.title.hardcore" } else { "deathScreen.title" });
                ui.transform = Some([2.0, 0.0, 0.0, 2.0, 0.0, 0.0]);
                centered(ui, &title, w / 2 / 2, 30, 0xFFFFFF);
                ui.transform = None;
                if self.hardcore {
                    centered(ui, &tr("deathScreen.hardcoreInfo"), w / 2, 144, 0xFFFFFF);
                }
                centered(ui, &format!("{}: §e{}", tr("deathScreen.score"), self.score), w / 2, 100, 0xFFFFFF);
            }
            Screen::GameOverConfirm => {
                // GuiYesNo com a segunda linha vazia.
                gui::default_background(ui, w, h, in_game);
                centered(ui, &tr("deathScreen.quit.confirm"), w / 2, 70, 0xFFFFFF);
            }
            Screen::Language => {
                let n = self.langs.len();
                gui::default_background(ui, w, h, in_game);
                self.list.draw_background(ui, n);
                for i in 0..n {
                    if self.list.selected == i as i32 {
                        self.list.draw_selection(ui, i, 220);
                    }
                    let y = self.list.entry_y(i);
                    let name = self.langs[i].1.clone();
                    centered(ui, &name, w / 2, y + 1, 0xFFFFFF);
                }
                self.list.draw_foreground(ui, n, w / 2 + 124);
                centered(ui, &tr("options.language"), w / 2, 16, 0xFFFFFF);
                centered(ui, &format!("({})", tr("options.languageWarning")), w / 2, h - 56, 0x808080);
            }
        }
        for b in &self.buttons {
            b.draw(ui, mx, my);
        }
        if let Some(t) = self.hovering.clone() {
            let lines: Vec<String> = t.split('\n').map(str::to_string).collect();
            gui::tooltip(ui, &lines, mx, my, w, h);
        }
    }

    fn draw_main_menu(&mut self, ui: &mut UiBatch) {
        let (w, h) = (self.w, self.h);
        // Panorama: o Java gira um cubo de 6 faces com desfoque; aqui a face 0 cobrindo a tela.
        let side = w.max(h) as f32;
        ui.sprite(
            (w as f32 - side) / 2.0,
            (h as f32 - side) / 2.0,
            side,
            side,
            PANORAMA.0 as f32,
            PANORAMA.1 as f32,
            256.0,
            256.0,
            0xFFFFFF,
        );
        gui::gradient(ui, 0, 0, w, h, 0x80FF_FFFF, 0x00FF_FFFF);
        gui::gradient(ui, 0, 0, w, h, 0x0000_0000, 0x8000_0000);
        let j = w / 2 - 274 / 2;
        let k = 30;
        ui.gui(TITLE, j as f32, k as f32, 0.0, 0.0, 155.0, 44.0);
        ui.gui(TITLE, (j + 155) as f32, k as f32, 0.0, 45.0, 155.0, 44.0);
        // Frase amarela girada -20° (pushMatrix/translate/rotate/scale).
        let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
        let f = 1.8 - ((ms % 1000) as f32 / 1000.0 * std::f32::consts::PI * 2.0).sin().abs() * 0.1;
        let splash = self.splash.clone();
        let f = f * 100.0 / (gui::width(ui, &splash) + 32) as f32;
        let (sin, cos) = (-20f32).to_radians().sin_cos();
        ui.transform = Some([cos * f, sin * f, -sin * f, cos * f, (w / 2 + 90) as f32, 70.0]);
        centered(ui, &splash, 0, -8, 0xFFFF00);
        ui.transform = None;
        string(ui, "Minecraft 1.8.9", 2, h - 10, 0xFFFFFF);
        let c = "Copyright Mojang AB. Do not distribute!";
        let cw = gui::width(ui, c);
        string(ui, c, w - cw - 2, h - 10, 0xFFFFFF);
    }

    /// `ServerSelectionList.drawScreen` + `ServerListEntryNormal`/`ServerListEntryLanScan.drawEntry`.
    fn draw_server_list(&mut self, ui: &mut UiBatch) {
        let (w, (mx, my)) = (self.w, self.ints());
        let size = self.servers.len() + 1;
        let lw = 305;
        let sb = w / 2 + 124 + 30;
        self.list.draw_background(ui, size);
        let x = self.list.entry_x(lw);
        let hovered = self.list.index_at(mx, my, lw, size, sb).filter(|_| self.list.y_inside(my));
        let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
        let count = self.servers.len();
        for i in 0..size {
            let y = self.list.entry_y(i);
            if self.list.selected == i as i32 {
                self.list.draw_selection(ui, i, lw);
            }
            if i == count {
                // Procurando jogos na rede local.
                let ty = y + 32 / 2 - 9 / 2;
                let t = tr("lanServer.scanning");
                centered_plain(ui, &t, w / 2, ty, 0xFFFFFF);
                let dots = match (now_ms / 300) % 4 {
                    1 | 3 => "o O o",
                    2 => "o o O",
                    _ => "O o o",
                };
                centered_plain(ui, dots, w / 2, ty + 9, 0x808080);
                continue;
            }
            let s = &self.servers[i];
            ui.text((x + 35) as f32, (y + 1) as f32, &s.name, 0xFFFFFF, false);
            let (motd, population, l, k, ping_tip, players_tip) = match &s.status {
                None => {
                    let mut l = ((now_ms / 100 + i as u64 * 2) & 7) as i32;
                    if l > 4 {
                        l = 8 - l;
                    }
                    (String::new(), String::new(), l, 1, "Pinging...".to_string(), None)
                }
                Some(Err(e)) => {
                    let motd = if e.contains("resolve") || e.contains("lookup") || e.contains("Name or service") {
                        "§4Can't resolve hostname"
                    } else {
                        "§4Can't connect to server."
                    };
                    (motd.to_string(), String::new(), 5, 0, "(no connection)".to_string(), None)
                }
                Some(Ok(st)) => {
                    let pop = format!("§7{}§8/§7{}", st.players.0, st.players.1);
                    let tip = (!st.sample.is_empty()).then(|| st.sample.join("\n"));
                    if st.protocol != 47 {
                        let txt = if st.protocol > 47 { "Client out of date!" } else { "Server out of date!" };
                        (st.motd.clone(), format!("§4{}", st.version), 5, 0, txt.to_string(), tip)
                    } else {
                        let l = match st.ping_ms {
                            0..150 => 0,
                            150..300 => 1,
                            300..600 => 2,
                            600..1000 => 3,
                            _ => 4,
                        };
                        (st.motd.clone(), pop, l, 0, format!("{}ms", st.ping_ms), tip)
                    }
                }
            };
            let lines = gui::wrap(ui, &motd, lw - 48 - 2);
            for (n, line) in lines.iter().take(2).enumerate() {
                ui.text((x + 35) as f32, (y + 12 + 9 * n as i32) as f32, line, 0x808080, false);
            }
            let pw = gui::width(ui, &population);
            ui.text((x + lw - pw - 15 - 2) as f32, (y + 1) as f32, &population, 0x808080, false);
            ui.gui(ICONS, (x + lw - 15) as f32, y as f32, (k * 10) as f32, (176 + l * 8) as f32, 10.0, 8.0);
            match s.icon_slot {
                Some(slot) => ui.item_icon(slot, x as f32, y as f32, 32.0),
                None => ui.sprite(x as f32, y as f32, 32.0, 32.0, UNKNOWN_SERVER.0 as f32, UNKNOWN_SERVER.1 as f32, 64.0, 64.0, 0xFFFFFF),
            }
            let (rx, ry) = (mx - x, my - y);
            if hovered == Some(i) {
                if rx >= lw - 15 && rx <= lw - 5 && (0..=8).contains(&ry) {
                    self.hovering = Some(ping_tip);
                } else if rx >= lw - pw - 15 - 2 && rx <= lw - 15 - 2 && (0..=8).contains(&ry) {
                    self.hovering = players_tip;
                }
            }
            if self.settings.touchscreen || hovered == Some(i) {
                gui::rect(ui, x, y, x + 32, y + 32, 0xA090_9090);
                let v = |on: bool| if on { 32.0 } else { 0.0 };
                ui.gui(SERVER_SELECTION, x as f32, y as f32, 0.0, v(rx < 32 && rx > 16), 32.0, 32.0);
                if i > 0 {
                    ui.gui(SERVER_SELECTION, x as f32, y as f32, 96.0, v(rx < 16 && ry < 16), 32.0, 32.0);
                }
                if i + 1 < count {
                    ui.gui(SERVER_SELECTION, x as f32, y as f32, 64.0, v(rx < 16 && ry > 16), 32.0, 32.0);
                }
            }
        }
        self.list.draw_foreground(ui, size, sb);
    }

    /// `GuiKeyBindingList.drawScreen`.
    fn draw_key_list(&mut self, ui: &mut UiBatch, mx: i32, my: i32) {
        let n = self.key_rows.len();
        let lw = 220 + 32;
        self.list.draw_background(ui, n);
        let x = self.list.entry_x(lw);
        let mut btn = 0;
        for r in 0..n {
            let y = self.list.entry_y(r);
            match &self.key_rows[r] {
                KeyRow::Category(label) => {
                    let lwid = gui::width(ui, label);
                    ui.text((self.w / 2 - lwid / 2) as f32, (y + 16 - 9 - 1) as f32, label, 0xFFFFFF, false);
                }
                KeyRow::Key(k) => {
                    let kb = self.settings.keys[*k].clone();
                    ui.text((x + 90 - self.max_label) as f32, (y + 16 / 2 - 9 / 2) as f32, &tr(kb.desc), 0xFFFFFF, false);
                    let dup = kb.code != 0 && self.settings.keys.iter().enumerate().any(|(j, o)| j != *k && o.code == kb.code);
                    let mut name = settings::key_name(kb.code);
                    if self.binding == Some(*k) {
                        name = format!("§f> §e{name}§f <");
                    } else if dup {
                        name = format!("§c{name}");
                    }
                    let (change, reset) = (btn, btn + 1);
                    self.rows[reset].x = x + 190;
                    self.rows[reset].y = y;
                    self.rows[reset].enabled = kb.code != kb.default;
                    self.rows[reset].draw(ui, mx, my);
                    self.rows[change].x = x + 105;
                    self.rows[change].y = y;
                    self.rows[change].text = name;
                    self.rows[change].draw(ui, mx, my);
                    btn += 2;
                }
            }
        }
        self.list.draw_foreground(ui, n, self.w / 2 + 124 + 15);
    }

    // ---------------------------------------------------------------- entrada

    pub fn mouse_move(&mut self, x: f32, y: f32) {
        self.mouse = (x, y);
        if !self.mouse_down {
            return;
        }
        let (mx, my) = self.ints();
        let s = &mut self.settings;
        for b in self.buttons.iter_mut().chain(self.rows.iter_mut()) {
            if b.dragging {
                b.slide(mx, s);
            }
        }
        let size = self.list_size();
        self.list.drag(my, size);
        if let Some(p) = &mut self.packs {
            p.drag(my);
        }
    }

    fn list_size(&self) -> usize {
        match self.screen {
            Screen::ServerList => self.servers.len() + 1,
            Screen::Video => self.rows.len().div_ceil(2),
            Screen::Controls => self.key_rows.len(),
            Screen::Language => self.langs.len(),
            _ => 0,
        }
    }

    pub fn wheel(&mut self, up: bool) {
        if self.screen == Screen::ResourcePacks {
            let (mx, my) = self.ints();
            if let Some(p) = &mut self.packs {
                p.wheel(up, mx, my);
            }
            return;
        }
        if matches!(self.screen, Screen::ServerList | Screen::Video | Screen::Controls | Screen::Language) {
            let (_, my) = self.ints();
            if self.list.y_inside(my) {
                let size = self.list_size();
                self.list.wheel(up, size);
            }
        }
    }

    pub fn mouse_up(&mut self) {
        self.mouse_down = false;
        let mut sound = false;
        let mut save = false;
        for b in self.buttons.iter_mut().chain(self.rows.iter_mut()) {
            if b.dragging {
                b.dragging = false;
                // O slider de som toca o clique ao soltar; os de opção gravam na saída.
                sound |= matches!(b.kind, Kind::Sound(_));
                save = true;
            }
        }
        if sound {
            self.click_sound();
        }
        if save {
            self.settings.save();
        }
        self.list.release();
        if let Some(p) = &mut self.packs {
            p.release();
        }
    }

    /// Botão do mouse (0 esquerdo, 1 direito, 2 meio).
    pub fn mouse_down(&mut self, ui: &mut UiBatch, button: u8) -> Option<Action> {
        let (mx, my) = self.ints();
        if button == 0 {
            self.mouse_down = true;
        }
        // GuiControls.mouseClicked: esperando tecla, o clique vira o botão do mouse.
        if self.screen == Screen::Controls {
            if let Some(k) = self.binding.take() {
                self.settings.keys[k].code = -100 + button as i32;
                self.settings.save();
                return None;
            }
        }
        if button != 0 {
            return None;
        }
        for f in &mut self.fields {
            f.click(ui, mx, my);
        }
        // GuiScreen.mouseClicked: o primeiro botão sob o mouse.
        let hit = self.buttons.iter().position(|b| b.pressed(mx, my));
        if let Some(i) = hit {
            let kind = self.buttons[i].kind;
            if !matches!(kind, Kind::Sound(_)) {
                self.click_sound();
            }
            if matches!(kind, Kind::Slider(_) | Kind::Sound(_)) {
                let s = &mut self.settings;
                self.buttons[i].slide(mx, s);
                self.buttons[i].dragging = true;
                return None;
            }
            let id = self.buttons[i].id;
            return self.action(ui, id, kind);
        }
        self.list_click(ui, mx, my)
    }

    /// Cliques dentro das listas (`GuiListExtended.mouseClicked` + `GuiSlot.handleMouseInput`).
    fn list_click(&mut self, ui: &mut UiBatch, mx: i32, my: i32) -> Option<Action> {
        match self.screen.clone() {
            Screen::Video => {
                let n = self.rows.len().div_ceil(2);
                let sb = self.w / 2 + 124 + 32;
                if self.list.y_inside(my) {
                    if let Some(r) = self.list.index_at(mx, my, 400, n, sb) {
                        for b in self.rows.iter_mut().skip(r * 2).take(2) {
                            if b.pressed(mx, my) {
                                self.sounds.push("gui.button.press");
                                self.list.entry_consumed();
                                match b.kind {
                                    Kind::Toggle(o) => {
                                        self.settings.cycle(o);
                                        b.text = self.settings.display(o);
                                    }
                                    Kind::Slider(_) => {
                                        b.slide(mx, &mut self.settings);
                                        b.dragging = true;
                                    }
                                    _ => {}
                                }
                                break;
                            }
                        }
                    }
                }
                self.list.press(mx, my, 400, n, sb);
            }
            Screen::Controls => {
                let n = self.key_rows.len();
                let sb = self.w / 2 + 124 + 15;
                if self.list.y_inside(my) {
                    if let Some(r) = self.list.index_at(mx, my, 252, n, sb) {
                        if let KeyRow::Key(k) = self.key_rows[r] {
                            let idx = self.key_rows[..r].iter().filter(|e| matches!(e, KeyRow::Key(_))).count() * 2;
                            if self.rows[idx].pressed(mx, my) {
                                self.click_sound();
                                self.binding = Some(k);
                                self.list.entry_consumed();
                            } else if self.rows[idx + 1].pressed(mx, my) {
                                self.click_sound();
                                let kb = &mut self.settings.keys[k];
                                kb.code = kb.default;
                                self.settings.save();
                                self.list.entry_consumed();
                            }
                        }
                    }
                }
                self.list.press(mx, my, 252, n, sb);
            }
            Screen::Language => {
                let n = self.langs.len();
                if let Some((i, _)) = self.list.press(mx, my, 220, n, self.w / 2 + 124) {
                    // GuiLanguage.List.elementClicked.
                    let code = self.langs[i].0.clone();
                    if code != self.settings.language {
                        self.settings.language = code;
                        self.settings.save();
                        self.language_changed = true;
                    }
                    self.inited = None;
                }
            }
            Screen::ServerList => {
                let size = self.servers.len() + 1;
                let lw = 305;
                let sb = self.w / 2 + 124 + 30;
                let count = self.servers.len();
                if self.list.y_inside(my) {
                    if let Some(i) = self.list.index_at(mx, my, lw, size, sb) {
                        let (rx, ry) = (mx - self.list.entry_x(lw), my - self.list.entry_y(i));
                        if i < count {
                            let shift = false;
                            if rx <= 32 {
                                if rx < 32 && rx > 16 {
                                    self.select_server(i as i32);
                                    self.list.entry_consumed();
                                    return self.connect_selected();
                                }
                                if rx < 16 && ry < 16 && i > 0 {
                                    self.move_server(i, if shift { 0 } else { i - 1 });
                                    self.list.entry_consumed();
                                    return None;
                                }
                                if rx < 16 && ry > 16 && i + 1 < count {
                                    self.move_server(i, if shift { count - 1 } else { i + 1 });
                                    self.list.entry_consumed();
                                    return None;
                                }
                            }
                            self.select_server(i as i32);
                            let double = self.servers[i].last_click.is_some_and(|t| t.elapsed().as_millis() < 250);
                            self.servers[i].last_click = Some(Instant::now());
                            if double {
                                return self.connect_selected();
                            }
                        } else {
                            self.select_server(i as i32);
                        }
                    }
                }
                self.list.press(mx, my, lw, size, sb);
            }
            Screen::ResourcePacks => {
                if let Some(p) = &mut self.packs {
                    if let Click::Confirm(id, newer) = p.click(mx, my) {
                        self.sounds.push("gui.button.press");
                        self.open(Screen::PackConfirm(id, newer));
                    }
                }
            }
            _ => {}
        }
        let _ = ui;
        None
    }

    /// `func_175391_a`/`func_175393_b`: troca de lugar e grava.
    fn move_server(&mut self, from: usize, to: usize) {
        self.servers.swap(from, to);
        if self.list.selected == from as i32 {
            self.select_server(to as i32);
        }
        self.save_servers();
    }

    fn connect_selected(&mut self) -> Option<Action> {
        let i = self.list.selected;
        let ip = self.servers.get(i.max(0) as usize).filter(|_| i >= 0).map(|s| s.ip.clone())?;
        Some(Action::Join(ip))
    }

    /// `actionPerformed` de cada tela.
    fn action(&mut self, ui: &mut UiBatch, id: i32, kind: Kind) -> Option<Action> {
        match self.screen.clone() {
            Screen::MainMenu => match id {
                2 => self.open(Screen::ServerList),
                14 => {
                    self.login_status.clear();
                    self.open(Screen::Login);
                }
                0 => self.open(Screen::Options),
                4 => return Some(Action::Quit),
                5 => self.open(Screen::Language),
                _ => {}
            },
            Screen::ServerList => match id {
                2 => {
                    let i = self.list.selected;
                    if i >= 0 && (i as usize) < self.servers.len() {
                        self.open(Screen::ConfirmDelete(i as usize));
                    }
                }
                1 => return self.connect_selected(),
                4 => self.open(Screen::DirectConnect),
                3 => {
                    self.draft = None;
                    self.open(Screen::AddServer(None));
                }
                7 => {
                    let i = self.list.selected;
                    if let Some(s) = self.servers.get(i.max(0) as usize).filter(|_| i >= 0) {
                        let mut d = ServerEntry::new(s.name.clone(), s.ip.clone());
                        d.resource_mode = s.resource_mode;
                        self.draft = Some(d);
                        self.open(Screen::AddServer(Some(i as usize)));
                    }
                }
                0 => self.back(),
                8 => {
                    self.refresh();
                    self.inited = None;
                }
                _ => {}
            },
            Screen::AddServer(edit) => match id {
                2 => {
                    if let Some(d) = &mut self.draft {
                        d.resource_mode = (d.resource_mode + 1) % 3;
                        d.name = self.fields[0].text.clone();
                        d.ip = self.fields[1].text.clone();
                    }
                    self.inited = None;
                }
                1 => {
                    self.draft = None;
                    self.back();
                }
                0 => {
                    let mut d = self.draft.take().unwrap_or_else(|| ServerEntry::new(String::new(), String::new()));
                    d.name = self.fields[0].text.clone();
                    d.ip = self.fields[1].text.clone();
                    match edit {
                        Some(i) if i < self.servers.len() => {
                            let s = &mut self.servers[i];
                            s.name = d.name;
                            s.ip = d.ip;
                            s.resource_mode = d.resource_mode;
                            s.status = None;
                        }
                        _ => self.servers.push(d),
                    }
                    self.save_servers();
                    self.back();
                    self.list.selected = -1;
                    self.refresh();
                }
                _ => {}
            },
            Screen::Login => match id {
                0 => return Some(Action::LoginMicrosoft),
                1 => {
                    let nick = self.fields.first().map(|f| f.text.trim().to_string()).unwrap_or_default();
                    if crate::msa::valid_name(&nick) {
                        return Some(Action::LoginOffline(nick));
                    }
                }
                2 => self.back(),
                _ => {}
            },
            Screen::DirectConnect => match id {
                0 => {
                    let ip = self.fields[0].text.clone();
                    self.settings.last_server = ip.clone();
                    self.settings.save();
                    self.back();
                    return Some(Action::Join(ip));
                }
                1 => self.back(),
                _ => {}
            },
            Screen::ConfirmDelete(i) => {
                if id == 0 && i < self.servers.len() {
                    self.servers.remove(i);
                    self.save_servers();
                    for s in &mut self.servers {
                        s.icon_slot = None;
                        s.icon_png = s.icon_b64.as_deref().and_then(b64);
                    }
                }
                self.back();
                self.list.selected = -1;
            }
            Screen::ResourcePacks => match id {
                2 => open_folder(&mc_render::resources::resourcepacks_dir()),
                1 => {
                    let mut reload = false;
                    if let Some(p) = self.packs.take() {
                        if p.changed {
                            // GuiScreenResourcePacks.actionPerformed(1): grava as listas no options.txt.
                            let chosen = p.chosen();
                            self.settings.resource_packs = chosen.iter().map(|e| e.name.clone()).collect();
                            self.settings.incompatible_resource_packs =
                                chosen.iter().filter(|e| e.format() != mc_render::resources::PACK_FORMAT).map(|e| e.name.clone()).collect();
                            self.settings.save();
                            reload = true;
                        }
                    }
                    self.back();
                    if reload {
                        return Some(Action::ReloadResources);
                    }
                }
                _ => {}
            },
            Screen::PackConfirm(pack, _) => {
                if let Some(p) = &mut self.packs {
                    p.confirm(&pack, id == 0);
                }
                self.back();
            }
            Screen::LinkConfirm(url) => {
                self.stack.clear();
                self.screen = Screen::None;
                self.inited = None;
                return Some(Action::LinkAnswer(url, id == 0, id == 2));
            }
            Screen::ServerPack(url, hash) => {
                self.stack.clear();
                self.screen = Screen::None;
                self.inited = None;
                return Some(Action::ServerPackAnswer(url, hash, id == 0));
            }
            Screen::Connecting(_) => {
                self.show(Screen::ServerList);
                return Some(Action::Disconnect);
            }
            Screen::Working | Screen::DownloadTerrain => {}
            Screen::GameOver => match id {
                0 => {
                    // respawnPlayer + displayGuiScreen(null): ainda morto, a tela reabre até o S07.
                    self.show(Screen::GameOver);
                    return Some(Action::Respawn);
                }
                1 if self.hardcore => return Some(Action::ToTitle),
                1 => self.show(Screen::GameOverConfirm),
                _ => {}
            },
            Screen::GameOverConfirm => {
                if id == 0 {
                    return Some(Action::ToTitle);
                }
                self.show(Screen::GameOver);
                return Some(Action::Respawn);
            }
            Screen::Disconnected(..) => {
                self.show(Screen::ServerList);
            }
            Screen::Pause => match id {
                0 => self.open(Screen::Options),
                1 => return Some(Action::Disconnect),
                4 => {
                    self.stack.clear();
                    self.screen = Screen::None;
                    self.inited = None;
                    return Some(Action::Resume);
                }
                _ => {}
            },
            Screen::Options => match id {
                110 => self.open(Screen::Skin),
                106 => self.open(Screen::Sounds),
                101 => self.open(Screen::Video),
                100 => self.open(Screen::Controls),
                102 => self.open(Screen::Language),
                103 => self.open(Screen::Chat),
                105 => self.open_packs(),
                200 => {
                    self.settings.save();
                    self.back();
                }
                _ => {
                    if let Kind::Toggle(o) = kind {
                        self.toggle(o);
                    }
                }
            },
            Screen::Video | Screen::Sounds | Screen::Chat | Screen::Skin => {
                if id == 200 && kind == Kind::Plain {
                    self.settings.save();
                    self.back();
                } else if let Kind::Part(p) = kind {
                    self.settings.model_parts ^= 1 << p;
                    self.settings.save();
                    let t = self.settings.part_display(p);
                    if let Some(b) = self.buttons.iter_mut().find(|b| b.kind == kind) {
                        b.text = t;
                    }
                } else if let Kind::Toggle(o) = kind {
                    self.toggle(o);
                }
            }
            Screen::Controls => match id {
                200 => {
                    self.settings.save();
                    self.back();
                }
                201 => {
                    for k in &mut self.settings.keys {
                        k.code = k.default;
                    }
                    self.settings.save();
                }
                _ => {
                    if let Kind::Toggle(o) = kind {
                        self.toggle(o);
                    }
                }
            },
            Screen::Language => match id {
                6 => {
                    self.settings.save();
                    self.back();
                }
                _ => {
                    if let Kind::Toggle(o) = kind {
                        self.toggle(o);
                    }
                }
            },
            Screen::None => {}
        }
        let _ = ui;
        None
    }

    fn toggle(&mut self, o: Opt) {
        self.settings.cycle(o);
        let t = self.settings.display(o);
        if let Some(b) = self.buttons.iter_mut().find(|b| b.kind == Kind::Toggle(o)) {
            b.text = t;
        }
    }

    /// Tecla com uma tela aberta (`keyTyped`).
    pub fn key(&mut self, ui: &mut UiBatch, code: KeyCode, text: Option<&str>, ctrl: bool, shift: bool) -> Option<Action> {
        // GuiControls esperando a tecla nova.
        if self.screen == Screen::Controls {
            if let Some(k) = self.binding.take() {
                let lw = settings::lwjgl_code(code);
                let c = if code == KeyCode::Escape {
                    0
                } else if lw != 0 {
                    lw
                } else {
                    text.and_then(|t| t.chars().next()).map_or(0, |c| c as i32 + 256)
                };
                self.settings.keys[k].code = c;
                self.settings.save();
                return None;
            }
        }
        match self.screen.clone() {
            Screen::Login => {
                if code == KeyCode::Escape {
                    if !self.stack.is_empty() {
                        self.back();
                    }
                    return None;
                }
                if matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) {
                    if self.buttons.get(1).is_some_and(|b| b.enabled) {
                        return self.action(ui, 1, Kind::Plain);
                    }
                    return None;
                }
                for f in &mut self.fields {
                    f.key(ui, code, text, ctrl, shift);
                }
                let ok = self.fields.first().is_some_and(|f| crate::msa::valid_name(f.text.trim()));
                if let Some(b) = self.buttons.get_mut(1) {
                    b.enabled = ok;
                }
                None
            }
            Screen::AddServer(_) | Screen::DirectConnect => {
                if code == KeyCode::Escape {
                    return self.close();
                }
                if code == KeyCode::Tab && self.fields.len() == 2 {
                    let f = self.fields[0].focused;
                    self.fields[0].focused = !f;
                    self.fields[1].focused = f;
                }
                if matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) {
                    if self.buttons.first().is_some_and(|b| b.enabled) {
                        return self.action(ui, 0, Kind::Plain);
                    }
                    return None;
                }
                for f in &mut self.fields {
                    f.key(ui, code, text, ctrl, shift);
                }
                // Botão "Adicionar"/"Selecionar" só com endereço (e nome).
                let ip_ok = self.fields.last().is_some_and(|f| !f.text.is_empty());
                let name_ok = self.fields.len() < 2 || !self.fields[0].text.is_empty();
                if let Some(b) = self.buttons.first_mut() {
                    b.enabled = ip_ok && name_ok;
                }
                None
            }
            Screen::ServerList => {
                let count = self.servers.len() as i32;
                let i = self.list.selected;
                match code {
                    KeyCode::F5 => {
                        self.refresh();
                        None
                    }
                    KeyCode::ArrowUp if i >= 0 => {
                        if shift {
                            if i > 0 && i < count {
                                self.move_server(i as usize, i as usize - 1);
                            }
                        } else if i > 0 {
                            self.select_server(i - 1);
                            let size = self.list_size();
                            self.list.scroll_by(-36, size);
                        } else {
                            self.select_server(-1);
                        }
                        None
                    }
                    KeyCode::ArrowDown if i >= 0 => {
                        if shift {
                            if i < count - 1 {
                                self.move_server(i as usize, i as usize + 1);
                            }
                        } else if i < count - 1 {
                            self.select_server(i + 1);
                            let size = self.list_size();
                            self.list.scroll_by(36, size);
                        } else {
                            self.select_server(-1);
                        }
                        None
                    }
                    KeyCode::Enter | KeyCode::NumpadEnter if i >= 0 => self.connect_selected(),
                    KeyCode::Escape => self.close(),
                    _ => None,
                }
            }
            // GuiMainMenu, GuiConnecting e GuiGameOver ignoram o Esc.
            Screen::MainMenu | Screen::Connecting(_) | Screen::DownloadTerrain | Screen::None | Screen::ServerPack(..) | Screen::Working | Screen::GameOver => None,
            _ => {
                if code == KeyCode::Escape {
                    self.close()
                } else {
                    None
                }
            }
        }
    }
}

/// `GuiScreenResourcePacks` botão 2: abre a pasta de pacotes no gerenciador de arquivos.
fn open_folder(dir: &Path) {
    let _ = std::fs::create_dir_all(dir);
    let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    let _ = std::process::Command::new(opener)
        .arg(dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

/// `drawString` sem sombra centralizado (entrada de LAN usa `drawString` comum).
fn centered_plain(ui: &mut UiBatch, s: &str, cx: i32, y: i32, color: u32) {
    let w = gui::width(ui, s);
    ui.text((cx - w / 2) as f32, y as f32, s, color, false);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_and_status() {
        assert_eq!(b64("TWFu").unwrap(), b"Man");
        assert_eq!(b64("TWE=").unwrap(), b"Ma");
        let st = parse_status(
            r#"{"description":{"text":"Oi","color":"green"},"players":{"online":5,"max":10,"sample":[{"name":"a","id":"x"}]},"version":{"name":"1.8.9","protocol":47}}"#,
            30,
        );
        assert_eq!(st.players, (5, 10));
        assert_eq!(st.protocol, 47);
        assert_eq!(st.sample, vec!["a".to_string()]);
        assert!(st.motd.contains("Oi"));
    }

    #[test]
    fn servers_dat_round_trip() {
        let dir = std::env::temp_dir().join(format!("mine-rs-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("servers.dat");
        let mut a = ServerEntry::new("Aura".into(), "mc.auratoons.com".into());
        a.icon_b64 = Some("TWFu".into());
        a.resource_mode = 0;
        let b = ServerEntry::new("Ação §aé".into(), "1.2.3.4:25566".into());
        let mut c = ServerEntry::new("C".into(), "c".into());
        c.resource_mode = 1;
        std::fs::write(&p, servers_nbt(&[a, b, c])).unwrap();
        let back = load_servers(&p);
        assert_eq!(back.len(), 3);
        assert_eq!(back[0].name, "Aura");
        assert_eq!(back[0].icon_b64.as_deref(), Some("TWFu"));
        assert_eq!(back[0].icon_png.as_deref(), Some(&b"Man"[..]));
        assert_eq!(back[0].resource_mode, 0);
        assert_eq!(back[1].name, "Ação §aé");
        assert_eq!(back[1].ip, "1.2.3.4:25566");
        assert_eq!(back[1].resource_mode, 2);
        assert_eq!(back[2].resource_mode, 1);
        // Gravar de novo dá os mesmos bytes.
        assert_eq!(std::fs::read(&p).unwrap(), servers_nbt(&back));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn real_servers_dat_round_trip() {
        let home = std::env::var("HOME").unwrap_or_default();
        let p = PathBuf::from(format!("{home}/.minecraft/servers.dat"));
        if !p.exists() {
            return;
        }
        let a = load_servers(&p);
        assert!(!a.is_empty());
        let dir = std::env::temp_dir().join(format!("mine-rs-real-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let q = dir.join("servers.dat");
        std::fs::write(&q, servers_nbt(&a)).unwrap();
        let b = load_servers(&q);
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(&b) {
            assert_eq!((&x.name, &x.ip, &x.icon_b64, x.resource_mode), (&y.name, &y.ip, &y.icon_b64, y.resource_mode));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn splash_hash() {
        assert_eq!(java_hash("This message will never appear on the splash screen, isn't that weird?"), 125_780_783);
        assert_eq!(pick_splash("a\nb\n").len(), 1);
    }

    fn layout(screen: Screen, in_game: bool, w: i32, h: i32) -> Vec<(i32, i32, i32, i32, i32)> {
        let font = mc_render::ui::Font::empty();
        let mut ui = UiBatch::new(font);
        let mut m = Menu::new(Screen::MainMenu, Settings::default(), PathBuf::from("/nonexistent/servers.dat"));
        m.in_game = in_game;
        m.screen = screen;
        m.draw(&mut ui, w as f32, h as f32);
        m.buttons.iter().map(|b| (b.id, b.x, b.y, b.w, b.h)).collect()
    }

    #[test]
    fn button_layouts_match_java() {
        // 1280x720 na escala automática 3 → 427x240.
        let (w, h) = (427, 240);
        let pause = layout(Screen::Pause, true, w, h);
        assert_eq!(
            pause,
            vec![
                (1, w / 2 - 100, h / 4 + 104, 200, 20),
                (4, w / 2 - 100, h / 4 + 8, 200, 20),
                (0, w / 2 - 100, h / 4 + 80, 98, 20),
                (7, w / 2 + 2, h / 4 + 80, 98, 20),
                (5, w / 2 - 100, h / 4 + 32, 98, 20),
                (6, w / 2 + 2, h / 4 + 32, 98, 20),
            ]
        );
        let opts = layout(Screen::Options, false, w, h);
        assert_eq!(opts[0], (Opt::Fov as i32, w / 2 - 155, h / 6 - 12, 150, 20));
        assert_eq!(opts[1], (Opt::RealmsNotifications as i32, w / 2 + 5, h / 6 - 12, 150, 20));
        assert_eq!(opts.last().unwrap(), &(200, w / 2 - 100, h / 6 + 168, 200, 20));
        assert!(opts.contains(&(101, w / 2 - 155, h / 6 + 90, 150, 20)));
        assert!(opts.contains(&(100, w / 2 + 5, h / 6 + 90, 150, 20)));
        let mp = layout(Screen::ServerList, false, w, h);
        assert_eq!(
            mp,
            vec![
                (7, w / 2 - 154, h - 28, 70, 20),
                (2, w / 2 - 74, h - 28, 70, 20),
                (1, w / 2 - 154, h - 52, 100, 20),
                (4, w / 2 - 50, h - 52, 100, 20),
                (3, w / 2 + 54, h - 52, 100, 20),
                (8, w / 2 + 4, h - 28, 70, 20),
                (0, w / 2 + 80, h - 28, 75, 20),
            ]
        );
        let main = layout(Screen::MainMenu, false, w, h);
        let j = h / 4 + 48;
        assert_eq!(main[1], (2, w / 2 - 100, j + 24, 200, 20));
        assert_eq!(main[3], (0, w / 2 - 100, j + 84, 98, 20));
        assert_eq!(main[4], (4, w / 2 + 2, j + 84, 98, 20));
        assert_eq!(main[5], (5, w / 2 - 124, j + 84, 20, 20));
        let sounds = layout(Screen::Sounds, false, w, h);
        assert_eq!(sounds[0], (0, w / 2 - 155, h / 6 - 12, 310, 20));
        assert_eq!(sounds[1], (1, w / 2 - 155, h / 6 + 12, 150, 20));
        assert_eq!(sounds[2], (2, w / 2 + 5, h / 6 + 12, 150, 20));
        assert_eq!(sounds.last().unwrap(), &(200, w / 2 - 100, h / 6 + 168, 200, 20));
        let skin = layout(Screen::Skin, false, w, h);
        assert_eq!(skin.len(), 8);
        assert_eq!(skin[6], (6, w / 2 - 155, h / 6 + 72, 150, 20));
        assert_eq!(skin[7], (200, w / 2 - 100, h / 6 + 96, 200, 20));
        let chat = layout(Screen::Chat, false, w, h);
        assert_eq!(chat[3], (Opt::ChatOpacity as i32, w / 2 + 5, h / 6 + 24, 150, 20));
        assert_eq!(chat.last().unwrap(), &(200, w / 2 - 100, h / 6 + 120, 200, 20));
        let ctl = layout(Screen::Controls, false, w, h);
        assert_eq!(ctl[0], (200, w / 2 - 155, h - 29, 150, 20));
        assert_eq!(ctl[1], (201, w / 2 + 5, h - 29, 150, 20));
        assert_eq!(ctl[3], (Opt::Sensitivity as i32, w / 2 + 5, 18, 150, 20));
        assert_eq!(ctl[4], (Opt::Touchscreen as i32, w / 2 - 155, 42, 150, 20));
        let video = layout(Screen::Video, false, w, h);
        assert_eq!(video, vec![(200, w / 2 - 100, h - 27, 200, 20)]);
        let add = layout(Screen::AddServer(None), false, w, h);
        assert_eq!(add[0], (0, w / 2 - 100, h / 4 + 114, 200, 20));
        assert_eq!(add[2], (2, w / 2 - 100, h / 4 + 72, 200, 20));
        let lang = layout(Screen::Language, false, w, h);
        assert_eq!(lang[1], (6, w / 2 + 5, h - 38, 150, 20));
    }
}
