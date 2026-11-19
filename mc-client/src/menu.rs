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
use crate::lang::{tr, trf};
use crate::worlds::{Flat, Mode, NewWorld, WorldEntry, FLAT_PRESETS, WORLD_TYPES};
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
    /// Um jogador: o servidor local subindo (mensagem em `Menu.working.1`).
    LocalWorld,
    /// `GuiSelectWorld`.
    WorldList,
    /// `GuiCreateWorld` (as escolhas ficam em `Menu.create`).
    CreateWorld,
    /// `GuiRenameWorld` do mundo `i` da lista.
    RenameWorld(usize),
    /// `GuiYesNo` de excluir o mundo `i`.
    ConfirmDeleteWorld(usize),
    /// `GuiCreateFlatWorld` (camadas em `Menu.flat`).
    CustomizeFlat,
    /// `GuiFlatPresets`.
    FlatPresets,
    /// No lugar do `GuiShareToLan` (só um jogador): só a opção de permitir comandos.
    Cheats,
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
    /// Um jogador: abre o mundo da pasta.
    PlayWorld(String),
    /// Um jogador: cria o mundo na pasta e entra nele.
    CreateWorld(String, NewWorld),
    /// Um jogador: liga ou desliga os comandos do mundo aberto e volta ao jogo.
    SetCheats(bool),
    /// Cancelou a preparação do mundo local.
    CancelSingleplayer,
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
    /// A sessão atual (ou a última) é o modo Um jogador: "Salvar e sair" e a volta vai ao menu principal.
    pub local: bool,
    /// Comandos ligados no mundo aberto (um jogador) e a escolha na tela de cheats.
    pub local_cheats: bool,
    cheats_draft: bool,
    /// Mundos da `GuiSelectWorld`.
    pub worlds: Vec<WorldEntry>,
    /// Escolhas do `GuiCreateWorld`.
    create: CreateDraft,
    /// Superplano sendo editado no `GuiCreateFlatWorld`.
    flat: Flat,
    /// Texto do campo do `GuiFlatPresets`.
    preset_text: String,
    /// Ícones (vaga no atlas; `None` sem ícone) e nomes dos itens das telas do superplano, preenchidos pelo jogo.
    pub item_icons: std::collections::HashMap<(i16, i16), Option<u32>>,
    pub item_names: std::collections::HashMap<(i16, i16), String>,
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
            local: false,
            local_cheats: false,
            cheats_draft: false,
            worlds: Vec::new(),
            create: CreateDraft::new(),
            flat: Flat::parse(crate::worlds::DEFAULT_FLAT),
            preset_text: String::new(),
            item_icons: Default::default(),
            item_names: Default::default(),
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
    /// Uma opção mudou fora da tela (F11): os botões são refeitos com o valor novo.
    pub fn refresh_options(&mut self) {
        self.inited = None;
    }

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
        self.item_icons.clear();
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
            Screen::Disconnected(..) if self.local => self.stack.push(Screen::MainMenu),
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
                // GuiMainMenu.initGui + addSingleplayerMultiplayerButtons (um jogador: servidor oficial local).
                let j = h / 4 + 48;
                b.push(Button::new(1, w / 2 - 100, j, tr("menu.singleplayer")));
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
            Screen::Connecting(_) | Screen::LocalWorld => {
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
                let quit = if self.local { "menu.returnToMenu" } else { "menu.disconnect" };
                b.push(Button::new(1, w / 2 - 100, h / 4 + 120 + i, tr(quit)));
                b.push(Button::new(4, w / 2 - 100, h / 4 + 24 + i, tr("menu.returnToGame")));
                b.push(Button::sized(0, w / 2 - 100, h / 4 + 96 + i, 98, 20, tr("menu.options")));
                // "Abrir para LAN" vira a tela de cheats no mundo local (sem LAN: o servidor só ouve 127.0.0.1).
                let mut lan = Button::sized(7, w / 2 + 2, h / 4 + 96 + i, 98, 20, if self.local { "Ativar cheats".to_string() } else { tr("menu.shareToLan") });
                lan.enabled = self.local;
                b.push(lan);
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
            Screen::Cheats => {
                // Como o GuiShareToLan, só com o botão de comandos.
                let onoff = tr(if self.cheats_draft { "options.on" } else { "options.off" });
                b.push(Button::sized(103, w / 2 - 75, 100, 150, 20, format!("{} {onoff}", tr("selectWorld.allowCommands"))));
                b.push(Button::sized(101, w / 2 - 155, h - 28, 150, 20, tr("gui.done")));
                b.push(Button::sized(102, w / 2 + 5, h - 28, 150, 20, tr("gui.cancel")));
            }
            Screen::WorldList => {
                // GuiSelectWorld.initGui + addWorldSelectionButtons.
                let keep = self.inited.as_ref().is_some_and(|(sc, _, _)| *sc == Screen::WorldList);
                let mut list = SlotList::new(w, h, 32, h - 64, 36);
                list.selected = self.list.selected.min(self.worlds.len() as i32 - 1);
                if keep {
                    list.scroll = self.list.scroll;
                }
                self.list = list;
                b.push(Button::sized(1, w / 2 - 154, h - 52, 150, 20, tr("selectWorld.select")));
                b.push(Button::sized(3, w / 2 + 4, h - 52, 150, 20, tr("selectWorld.create")));
                b.push(Button::sized(6, w / 2 - 154, h - 28, 72, 20, tr("selectWorld.rename")));
                b.push(Button::sized(2, w / 2 - 76, h - 28, 72, 20, tr("selectWorld.delete")));
                b.push(Button::sized(7, w / 2 + 4, h - 28, 72, 20, tr("selectWorld.recreate")));
                b.push(Button::sized(0, w / 2 + 4 + 78, h - 28, 72, 20, tr("gui.cancel")));
                let sel = self.list.selected >= 0;
                for x in b.iter_mut().filter(|x| matches!(x.id, 1 | 2 | 6 | 7)) {
                    x.enabled = sel;
                }
            }
            Screen::CreateWorld => {
                // GuiCreateWorld.initGui + updateDisplayState.
                let d = self.create.clone();
                let more = d.more;
                let hard = d.mode == Mode::Hardcore;
                let onoff = |v: bool| tr(if v { "options.on" } else { "options.off" });
                let mut create = Button::sized(0, w / 2 - 155, h - 28, 150, 20, tr("selectWorld.create"));
                create.enabled = !d.name.trim().is_empty();
                b.push(create);
                b.push(Button::sized(1, w / 2 + 5, h - 28, 150, 20, tr("gui.cancel")));
                let mode = format!("{}: {}", tr("selectWorld.gameMode"), tr(&format!("selectWorld.gameMode.{}", d.mode.key())));
                let mut gm = Button::sized(2, w / 2 - 75, 115, 150, 20, mode);
                gm.visible = !more;
                b.push(gm);
                b.push(Button::sized(3, w / 2 - 75, 187, 150, 20, tr(if more { "gui.done" } else { "selectWorld.moreWorldOptions" })));
                let mut feat = Button::sized(4, w / 2 - 155, 100, 150, 20, format!("{} {}", tr("selectWorld.mapFeatures"), onoff(d.structures)));
                feat.visible = more;
                b.push(feat);
                // Baú bônus: o servidor dedicado não gera, então fica desligado.
                let mut bonus = Button::sized(7, w / 2 + 5, 151, 150, 20, format!("{} {}", tr("selectWorld.bonusItems"), onoff(false)));
                bonus.visible = more;
                bonus.enabled = false;
                b.push(bonus);
                let ty = WORLD_TYPES[d.world_type];
                let mut map = Button::sized(5, w / 2 + 5, 100, 150, 20, format!("{} {}", tr("selectWorld.mapType"), tr(ty.1)));
                map.visible = more;
                b.push(map);
                let mut cmd = Button::sized(6, w / 2 - 155, 151, 150, 20, format!("{} {}", tr("selectWorld.allowCommands"), onoff(d.cheats && !hard)));
                cmd.visible = more;
                cmd.enabled = !hard;
                b.push(cmd);
                // Personalizar: o do superplano; o do "Personalizado" (GuiCustomizeWorldScreen) não foi portado.
                let mut custom = Button::sized(8, w / 2 + 5, 120, 150, 20, tr("selectWorld.customizeType"));
                custom.visible = more && matches!(ty.0, "FLAT" | "CUSTOMIZED");
                custom.enabled = ty.0 == "FLAT";
                b.push(custom);
                let mut f = TextField::new(w / 2 - 100, 60, 200, 20);
                f.focused = true;
                f.set_text(ui, if more { &d.seed } else { &d.name });
                self.fields = vec![f];
            }
            Screen::RenameWorld(i) => {
                // GuiRenameWorld.initGui.
                let name = self.worlds.get(i).map_or(String::new(), |x| x.name.clone());
                let mut ok = Button::new(0, w / 2 - 100, h / 4 + 96 + 12, tr("selectWorld.renameButton"));
                ok.enabled = !name.trim().is_empty();
                b.push(ok);
                b.push(Button::new(1, w / 2 - 100, h / 4 + 120 + 12, tr("gui.cancel")));
                let mut f = TextField::new(w / 2 - 100, 60, 200, 20);
                f.focused = true;
                f.set_text(ui, &name);
                self.fields = vec![f];
            }
            Screen::ConfirmDeleteWorld(_) => {
                b.push(Button::sized(0, w / 2 - 155, h / 6 + 96, 150, 20, tr("selectWorld.deleteButton")));
                b.push(Button::sized(1, w / 2 - 155 + 160, h / 6 + 96, 150, 20, tr("gui.cancel")));
            }
            Screen::CustomizeFlat => {
                // GuiCreateFlatWorld.initGui: "Adicionar" e "Editar camada" ficam invisíveis na 1.8.9.
                let keep = self.inited.as_ref().is_some_and(|(sc, _, _)| *sc == Screen::CustomizeFlat);
                let mut list = SlotList::new(w, h, 43, h - 60, 24);
                list.selected = self.list.selected.min(self.flat.layers.len() as i32 - 1);
                if keep {
                    list.scroll = self.list.scroll;
                }
                self.list = list;
                let mut remove = Button::sized(4, w / 2 - 155, h - 52, 150, 20, tr("createWorld.customize.flat.removeLayer"));
                remove.enabled = self.list.selected >= 0;
                b.push(remove);
                b.push(Button::sized(0, w / 2 - 155, h - 28, 150, 20, tr("gui.done")));
                b.push(Button::sized(5, w / 2 + 5, h - 52, 150, 20, tr("createWorld.customize.presets")));
                b.push(Button::sized(1, w / 2 + 5, h - 28, 150, 20, tr("gui.cancel")));
            }
            Screen::FlatPresets => {
                // GuiFlatPresets.initGui.
                let keep = self.inited.as_ref().is_some_and(|(sc, _, _)| *sc == Screen::FlatPresets);
                let mut list = SlotList::new(w, h, 80, h - 37, 24);
                list.selected = self.list.selected.min(FLAT_PRESETS.len() as i32 - 1);
                if keep {
                    list.scroll = self.list.scroll;
                }
                self.list = list;
                let mut ok = Button::sized(0, w / 2 - 155, h - 28, 150, 20, tr("createWorld.customize.presets.select"));
                ok.enabled = self.preset_text.len() > 1;
                b.push(ok);
                b.push(Button::sized(1, w / 2 + 5, h - 28, 150, 20, tr("gui.cancel")));
                let mut f = TextField::new(50, 40, w - 100, 20);
                f.set_max_len(1230);
                let t = self.preset_text.clone();
                f.set_text(ui, &t);
                self.fields = vec![f];
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
            Screen::LocalWorld => {
                gui::default_background(ui, w, h, in_game);
                centered(ui, &tr("menu.loadingLevel"), w / 2, h / 2 - 50, 0xFFFFFF);
                let msg = self.working.1.clone();
                centered(ui, &msg, w / 2, h / 2 - 30, 0xAAAAAA);
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
            Screen::Cheats => {
                gui::default_background(ui, w, h, in_game);
                centered(ui, "Cheats", w / 2, 50, 0xFFFFFF);
                centered(ui, &tr("selectWorld.allowCommands.info"), w / 2, 82, 0xFFFFFF);
            }
            Screen::WorldList => {
                // GuiSelectWorld.drawScreen + List.drawSlot.
                gui::default_background(ui, w, h, in_game);
                let n = self.worlds.len();
                self.list.draw_background(ui, n);
                let x = self.list.entry_x(220);
                for i in 0..n {
                    let y = self.list.entry_y(i);
                    if self.list.selected == i as i32 {
                        self.list.draw_selection(ui, i, 220);
                    }
                    let wd = self.worlds[i].clone();
                    let name = if wd.name.is_empty() { format!("{} {}", tr("selectWorld.world"), i + 1) } else { wd.name.clone() };
                    let date = chrono::DateTime::from_timestamp_millis(wd.last_played as i64)
                        .map(|d| d.with_timezone(&chrono::Local).format("%d/%m/%y %H:%M").to_string())
                        .unwrap_or_default();
                    let mut mode = match wd.mode {
                        Mode::Hardcore => format!("§4{}§r", tr("gameMode.hardcore")),
                        Mode::Creative => tr("gameMode.creative"),
                        Mode::Survival => tr("gameMode.survival"),
                    };
                    if wd.cheats {
                        mode = format!("{mode}, {}", tr("selectWorld.cheats"));
                    }
                    ui.text((x + 2) as f32, (y + 1) as f32, &name, 0xFFFFFF, false);
                    ui.text((x + 2) as f32, (y + 12) as f32, &format!("{} ({date})", wd.folder), 0x808080, false);
                    ui.text((x + 2) as f32, (y + 12 + 10) as f32, &mode, 0x808080, false);
                }
                self.list.draw_foreground(ui, n, w / 2 + 124);
                centered(ui, &tr("selectWorld.title"), w / 2, 20, 0xFFFFFF);
            }
            Screen::CreateWorld => {
                // GuiCreateWorld.drawScreen.
                gui::default_background(ui, w, h, in_game);
                centered(ui, &tr("selectWorld.create"), w / 2, 20, 0xFFFFFF);
                let d = self.create.clone();
                if d.more {
                    string(ui, &tr("selectWorld.enterSeed"), w / 2 - 100, 47, 0xA0A0A0);
                    string(ui, &tr("selectWorld.seedInfo"), w / 2 - 100, 85, 0xA0A0A0);
                    string(ui, &tr("selectWorld.mapFeatures.info"), w / 2 - 150, 122, 0xA0A0A0);
                    string(ui, &tr("selectWorld.allowCommands.info"), w / 2 - 150, 172, 0xA0A0A0);
                    if WORLD_TYPES[d.world_type].0 == "AMPLIFIED" {
                        let mut y = 100 + 22;
                        for l in gui::wrap(ui, &tr("generator.amplified.info"), 150) {
                            ui.text((w / 2 + 5 + 2) as f32, y as f32, &l, 0xA0A0A0, false);
                            y += 9;
                        }
                    }
                } else {
                    string(ui, &tr("selectWorld.enterName"), w / 2 - 100, 47, 0xA0A0A0);
                    string(ui, &format!("{} {}", tr("selectWorld.resultFolder"), d.folder), w / 2 - 100, 85, 0xA0A0A0);
                    string(ui, &tr(&format!("selectWorld.gameMode.{}.line1", d.mode.key())), w / 2 - 100, 137, 0xA0A0A0);
                    string(ui, &tr(&format!("selectWorld.gameMode.{}.line2", d.mode.key())), w / 2 - 100, 149, 0xA0A0A0);
                }
                for f in &self.fields {
                    f.draw(ui);
                }
            }
            Screen::RenameWorld(_) => {
                gui::default_background(ui, w, h, in_game);
                centered(ui, &tr("selectWorld.renameTitle"), w / 2, 20, 0xFFFFFF);
                string(ui, &tr("selectWorld.enterName"), w / 2 - 100, 47, 0xA0A0A0);
                for f in &self.fields {
                    f.draw(ui);
                }
            }
            Screen::ConfirmDeleteWorld(i) => {
                gui::default_background(ui, w, h, in_game);
                centered(ui, &tr("selectWorld.deleteQuestion"), w / 2, 70, 0xFFFFFF);
                let name = self.worlds.get(i).map_or(String::new(), |s| s.name.clone());
                let msg = format!("'{name}' {}", tr("selectWorld.deleteWarning"));
                let mut y = 90;
                for l in gui::wrap(ui, &msg, w - 50) {
                    centered(ui, &l, w / 2, y, 0xFFFFFF);
                    y += 9;
                }
            }
            Screen::CustomizeFlat => {
                // GuiCreateFlatWorld.drawScreen + Details.drawSlot: a lista vai de cima (última camada) para baixo.
                gui::default_background(ui, w, h, in_game);
                let n = self.flat.layers.len();
                self.list.draw_background(ui, n);
                let x = self.list.entry_x(220);
                for i in 0..n {
                    let y = self.list.entry_y(i);
                    if self.list.selected == i as i32 {
                        self.list.draw_selection(ui, i, 220);
                    }
                    let layer = self.flat.layers[n - i - 1].clone();
                    let item = layer.icon_item();
                    if let Some(slot) = item.and_then(|k| self.item_icons.get(&k).copied().flatten()) {
                        ui.item_icon(slot, (x + 2) as f32, (y + 2) as f32, 16.0);
                    }
                    let name = match item {
                        None => "Air".to_string(),
                        Some(k) => self.item_names.get(&k).cloned().unwrap_or_else(|| layer.block.clone()),
                    };
                    ui.text((x + 18 + 5) as f32, (y + 3) as f32, &name, 0xFFFFFF, false);
                    let key = if i == 0 {
                        "createWorld.customize.flat.layer.top"
                    } else if i == n - 1 {
                        "createWorld.customize.flat.layer.bottom"
                    } else {
                        "createWorld.customize.flat.layer"
                    };
                    let hs = trf(key, &[&layer.count.to_string()]);
                    let hw = gui::width(ui, &hs);
                    ui.text((x + 2 + 213 - hw) as f32, (y + 3) as f32, &hs, 0xFFFFFF, false);
                }
                self.list.draw_foreground(ui, n, w / 2 + 124);
                centered(ui, &tr("createWorld.customize.flat.title"), w / 2, 8, 0xFFFFFF);
                let i = w / 2 - 92 - 16;
                string(ui, &tr("createWorld.customize.flat.tile"), i, 32, 0xFFFFFF);
                let hs = tr("createWorld.customize.flat.height");
                let hw = gui::width(ui, &hs);
                string(ui, &hs, i + 2 + 213 - hw, 32, 0xFFFFFF);
            }
            Screen::FlatPresets => {
                // GuiFlatPresets.drawScreen + ListSlot.drawSlot.
                gui::default_background(ui, w, h, in_game);
                let n = FLAT_PRESETS.len();
                self.list.draw_background(ui, n);
                let x = self.list.entry_x(220);
                for (i, (key, icon, _)) in FLAT_PRESETS.iter().enumerate() {
                    let y = self.list.entry_y(i);
                    if self.list.selected == i as i32 {
                        self.list.draw_selection(ui, i, 220);
                    }
                    if let Some(slot) = self.item_icons.get(icon).copied().flatten() {
                        ui.item_icon(slot, (x + 2) as f32, (y + 2) as f32, 16.0);
                    }
                    ui.text((x + 18 + 5) as f32, (y + 6) as f32, key, 0xFFFFFF, false);
                }
                self.list.draw_foreground(ui, n, w / 2 + 124);
                centered(ui, &tr("createWorld.customize.presets.title"), w / 2, 8, 0xFFFFFF);
                string(ui, &tr("createWorld.customize.presets.share"), 50, 30, 0xA0A0A0);
                string(ui, &tr("createWorld.customize.presets.list"), 50, 70, 0xA0A0A0);
                for f in &self.fields {
                    f.draw(ui);
                }
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
            Screen::WorldList => self.worlds.len(),
            Screen::CustomizeFlat => self.flat.layers.len(),
            Screen::FlatPresets => FLAT_PRESETS.len(),
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
        if matches!(
            self.screen,
            Screen::ServerList | Screen::Video | Screen::Controls | Screen::Language | Screen::WorldList | Screen::CustomizeFlat | Screen::FlatPresets
        ) {
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
            Screen::WorldList => {
                let n = self.worlds.len();
                if let Some((_, double)) = self.list.press(mx, my, 220, n, self.w / 2 + 124) {
                    // GuiSelectWorld.List.elementClicked: seleciona; o clique duplo entra.
                    self.inited = None;
                    if double {
                        return self.play_selected();
                    }
                }
            }
            Screen::CustomizeFlat => {
                let n = self.flat.layers.len();
                if self.list.press(mx, my, 220, n, self.w / 2 + 124).is_some() {
                    self.inited = None;
                }
            }
            Screen::FlatPresets => {
                let n = FLAT_PRESETS.len();
                if let Some((i, _)) = self.list.press(mx, my, 220, n, self.w / 2 + 124) {
                    // GuiFlatPresets.ListSlot.elementClicked: o texto da predefinição vai para o campo.
                    self.preset_text = FLAT_PRESETS[i].2.to_string();
                    self.inited = None;
                }
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

    /// O mundo escolhido na `GuiSelectWorld`.
    fn selected_world(&self) -> Option<usize> {
        usize::try_from(self.list.selected).ok().filter(|i| *i < self.worlds.len())
    }

    /// `GuiSelectWorld.func_146615_e`: entra no mundo escolhido.
    fn play_selected(&mut self) -> Option<Action> {
        let folder = self.worlds.get(self.selected_world()?)?.folder.clone();
        self.local = true;
        self.working.1.clear();
        self.show(Screen::LocalWorld);
        Some(Action::PlayWorld(folder))
    }

    /// Itens com ícone nas telas do superplano (o jogo prepara os ícones e os nomes).
    pub fn wanted_items(&self) -> Vec<(i16, i16)> {
        match self.screen {
            Screen::CustomizeFlat => self.flat.layers.iter().filter_map(|l| l.icon_item()).collect(),
            Screen::FlatPresets => FLAT_PRESETS.iter().map(|p| p.1).collect(),
            _ => Vec::new(),
        }
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
                1 => {
                    self.worlds = crate::worlds::list();
                    self.list.selected = -1;
                    self.open(Screen::WorldList);
                }
                2 => {
                    self.local = false;
                    self.open(Screen::ServerList);
                }
                14 => {
                    self.login_status.clear();
                    self.open(Screen::Login);
                }
                0 => self.open(Screen::Options),
                4 => return Some(Action::Quit),
                5 => self.open(Screen::Language),
                _ => {}
            },
            Screen::Cheats => match id {
                103 => {
                    self.cheats_draft = !self.cheats_draft;
                    self.inited = None;
                }
                101 => {
                    // GuiShareToLan: confirmar volta direto ao jogo.
                    self.stack.clear();
                    self.screen = Screen::None;
                    self.inited = None;
                    return Some(Action::SetCheats(self.cheats_draft));
                }
                102 => self.back(),
                _ => {}
            },
            Screen::WorldList => match id {
                1 => return self.play_selected(),
                3 => {
                    self.create = CreateDraft::new();
                    self.open(Screen::CreateWorld);
                }
                6 => {
                    if let Some(i) = self.selected_world() {
                        self.open(Screen::RenameWorld(i));
                    }
                }
                2 => {
                    if let Some(i) = self.selected_world() {
                        self.open(Screen::ConfirmDeleteWorld(i));
                    }
                }
                7 => {
                    // GuiCreateWorld.recreateFromExistingWorld: cópia com a mesma semente e as mesmas opções.
                    if let Some(i) = self.selected_world() {
                        self.create = CreateDraft::recreate(&self.worlds[i]);
                        self.open(Screen::CreateWorld);
                    }
                }
                0 => self.back(),
                _ => {}
            },
            Screen::CreateWorld => {
                let d = &mut self.create;
                match id {
                    0 => {
                        let name = d.name.trim().to_string();
                        if name.is_empty() {
                            return None;
                        }
                        let new = NewWorld {
                            name,
                            seed: d.seed.trim().to_string(),
                            mode: d.mode,
                            structures: d.structures,
                            world_type: d.world_type,
                            cheats: d.cheats && d.mode != Mode::Hardcore,
                            generator: d.generator.clone(),
                        };
                        let folder = crate::worlds::folder_for(&new.name);
                        self.local = true;
                        self.working.1.clear();
                        self.show(Screen::LocalWorld);
                        return Some(Action::CreateWorld(folder, new));
                    }
                    1 => {
                        self.back();
                        return None;
                    }
                    2 => {
                        d.mode = d.mode.next();
                        if !d.cheats_set {
                            d.cheats = d.mode == Mode::Creative;
                        }
                    }
                    3 => d.more = !d.more,
                    4 => d.structures = !d.structures,
                    5 => d.world_type = (d.world_type + 1) % WORLD_TYPES.len(),
                    6 => {
                        d.cheats = !d.cheats;
                        d.cheats_set = true;
                    }
                    8 => {
                        self.flat = Flat::parse(&self.create.generator);
                        self.list.selected = -1;
                        self.open(Screen::CustomizeFlat);
                        return None;
                    }
                    _ => {}
                }
                self.inited = None;
            }
            Screen::RenameWorld(i) => {
                if id == 0 {
                    let name = self.fields.first().map(|f| f.text.trim().to_string()).unwrap_or_default();
                    if let Some(wd) = self.worlds.get_mut(i).filter(|_| !name.is_empty()) {
                        wd.name = name;
                        let _ = wd.save();
                    }
                }
                self.worlds = crate::worlds::list();
                self.back();
            }
            Screen::ConfirmDeleteWorld(i) => {
                if id == 0 {
                    if let Some(wd) = self.worlds.get(i) {
                        if let Err(e) = crate::worlds::delete(&wd.folder) {
                            eprintln!("não deu para excluir o mundo {}: {e}", wd.folder);
                        }
                    }
                    self.list.selected = -1;
                }
                self.worlds = crate::worlds::list();
                self.back();
            }
            Screen::CustomizeFlat => match id {
                4 => {
                    // GuiCreateFlatWorld.func_146375_g: tira a camada escolhida (a lista vai de cima para baixo).
                    let n = self.flat.layers.len();
                    if let Ok(sel) = usize::try_from(self.list.selected) {
                        if sel < n {
                            self.flat.layers.remove(n - sel - 1);
                            self.list.selected = self.list.selected.min(n as i32 - 2);
                        }
                    }
                    self.inited = None;
                }
                0 => {
                    self.create.generator = self.flat.to_text();
                    self.back();
                }
                5 => {
                    self.preset_text = self.flat.to_text();
                    self.list.selected = -1;
                    self.open(Screen::FlatPresets);
                }
                1 => self.back(),
                _ => {}
            },
            Screen::FlatPresets => {
                if id == 0 {
                    self.flat = Flat::parse(&self.preset_text);
                }
                if id <= 1 {
                    self.list.selected = -1;
                    self.back();
                }
            }
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
                self.show(if self.local { Screen::MainMenu } else { Screen::ServerList });
                return Some(Action::Disconnect);
            }
            Screen::LocalWorld => {
                self.show(Screen::MainMenu);
                return Some(Action::CancelSingleplayer);
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
                self.show(if self.local { Screen::MainMenu } else { Screen::ServerList });
            }
            Screen::Pause => match id {
                7 if self.local => {
                    self.cheats_draft = self.local_cheats;
                    self.open(Screen::Cheats);
                }
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
            Screen::CreateWorld | Screen::RenameWorld(_) | Screen::FlatPresets => {
                if code == KeyCode::Escape {
                    self.back();
                    return None;
                }
                if matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) && self.screen != Screen::FlatPresets {
                    if self.buttons.first().is_some_and(|b| b.enabled) {
                        return self.action(ui, 0, Kind::Plain);
                    }
                    return None;
                }
                for f in &mut self.fields {
                    f.key(ui, code, text, ctrl, shift);
                }
                let t = self.fields.first().map(|f| f.text.clone()).unwrap_or_default();
                let ok = match self.screen {
                    Screen::CreateWorld if self.create.more => {
                        self.create.seed = t;
                        !self.create.name.trim().is_empty()
                    }
                    Screen::CreateWorld => {
                        // GuiCreateWorld.calcSaveDirName a cada tecla.
                        self.create.folder = crate::worlds::folder_for(&t);
                        self.create.name = t;
                        !self.create.name.trim().is_empty()
                    }
                    Screen::FlatPresets => {
                        self.preset_text = t;
                        self.preset_text.len() > 1
                    }
                    _ => !t.trim().is_empty(),
                };
                if let Some(b) = self.buttons.first_mut() {
                    b.enabled = ok;
                }
                None
            }
            Screen::WorldList | Screen::CustomizeFlat | Screen::Cheats => {
                if code == KeyCode::Escape {
                    self.back();
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
            Screen::MainMenu | Screen::Connecting(_) | Screen::LocalWorld | Screen::DownloadTerrain | Screen::None | Screen::ServerPack(..) | Screen::Working | Screen::GameOver => None,
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
    let opener = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
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

/// Escolhas do `GuiCreateWorld`.
#[derive(Clone, Debug)]
struct CreateDraft {
    name: String,
    /// `saveDirName` mostrado embaixo do nome.
    folder: String,
    seed: String,
    mode: Mode,
    structures: bool,
    world_type: usize,
    cheats: bool,
    /// `allowCheatsWasSetByUser`: trocar o modo não mexe mais nos comandos.
    cheats_set: bool,
    /// `inMoreWorldOptionsMode`.
    more: bool,
    generator: String,
}

impl CreateDraft {
    fn new() -> Self {
        let name = tr("selectWorld.newWorld");
        CreateDraft {
            folder: crate::worlds::folder_for(&name),
            name,
            seed: String::new(),
            mode: Mode::Survival,
            structures: true,
            world_type: 0,
            cheats: false,
            cheats_set: false,
            more: false,
            generator: crate::worlds::DEFAULT_FLAT.to_string(),
        }
    }

    fn recreate(w: &WorldEntry) -> Self {
        let name = trf("selectWorld.newWorld.copyOf", &[&w.name]);
        CreateDraft {
            folder: crate::worlds::folder_for(&name),
            name,
            seed: w.seed().map(|s| s.to_string()).unwrap_or_default(),
            mode: w.mode,
            structures: w.structures,
            world_type: w.world_type,
            cheats: w.cheats,
            cheats_set: true,
            more: false,
            generator: if w.generator.is_empty() { crate::worlds::DEFAULT_FLAT.to_string() } else { w.generator.clone() },
        }
    }
}
