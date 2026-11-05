//! Janela, loop de jogo e ligação entre rede, mundo, física e renderização.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use glam::DVec3;
use mc_protocol::serverbound;
use mc_render::ui::{build_ui_atlas, load_unicode_page, Font, UiBatch};
use mc_render::anim::{celestial_angle, Animations, CompassWorld};
use mc_render::{build_atlas, AtlasImage, Camera, Frame, Jar, Renderer, SkyTex};
use mc_world::blocks::BlockTable;
use mc_world::visgraph::SetVisibility;
use mc_world::mesh::{mesh_section, MeshData};
use mc_world::World;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Fullscreen, Window, WindowId};

use crate::auth::Session;
use crate::culling::{visible_sections, Scene};
use crate::hud::{DebugInfo, Hud};
use crate::interact::{self, Hit};
use crate::gui_container::draw_stack;
use crate::inventory::{icon_key, IconKey, Inventory, KeyEvent};
use mc_render::items::{icon_origin, ItemIcons};
use mc_render::ui::white_uv;
use mc_render::{DrawItem, Tex};
use crate::entities::Entities;
use crate::tile_entities::TileEntities;
use crate::net::{self, BoxErr, Net, NetEvent};
use crate::physics::{Keys, MovePacket, Player, UseAction};
use crate::sound::Sound;

const TICK: f64 = 0.05;
/// Seções mandadas para gerar malha por atualização.
const MESH_BUDGET: usize = 64;
/// Ticks atrasados recuperados de uma vez (como o jogo original).
const MAX_CATCH_UP: u32 = 10;

pub struct Options {
    /// `None` abre o menu de servidores.
    pub server: Option<String>,
    pub name: String,
    pub session: Option<Session>,
    pub jar: PathBuf,
    /// `--test-world`: mundo plano local, sem rede.
    pub test_world: bool,
    /// Sem conta salva nem `--session`/`--offline`: abre a tela de conta primeiro.
    pub login: bool,
    /// Aviso do login que falhou ao abrir (token vencido sem renovação, por exemplo).
    pub login_error: Option<String>,
}

/// Automação do mundo de teste (ver `test_world`).
#[derive(Default)]
struct TestOpts {
    keys: Keys,
    swing: bool,
    use_item: bool,
}

type Key = (i32, i32, i32);

/// Monta o `Env` das partículas (mundo, blocos, jogador, entidades) sem pegar `self` inteiro.
macro_rules! penv {
    ($g:expr, $env:ident, $body:block) => {{
        let ent = |id: i32| {
            $g.entities.list.get(&id).map(|e| {
                let (w, h) = e.size();
                crate::particles::EntView { pos: e.pos, width: w, height: h }
            })
        };
        let $env = crate::particles::Env { world: &$g.world, table: &*$g.table, player: ($g.player.pos, $g.player.vel), entity: &ent };
        $body
    }};
}

struct MeshDone {
    key: Key,
    version: u32,
    mesh: MeshData,
}

/// Seções medidas por quadro (índices de `Prof::us`).
const SECOES: [&str; 12] = [
    "ticks", "atmosfera", "hud", "entidades", "mao", "tiles", "particulas", "gui", "visiveis", "render", "espera_gpu",
    // parte de "ticks": o `EffectRenderer.updateEffects`
    "tick_particulas",
];

/// MINE_RS_STATS=1: tempo médio por quadro de cada seção, em ms, impresso a cada segundo.
struct Prof {
    on: bool,
    since: Instant,
    quadros: u32,
    total_us: u128,
    us: [u128; SECOES.len()],
}

impl Prof {
    fn new() -> Self {
        Self { on: std::env::var_os("MINE_RS_STATS").is_some(), since: Instant::now(), quadros: 0, total_us: 0, us: [0; SECOES.len()] }
    }

    /// Soma o tempo da seção `secao` desde `t`.
    fn add(&mut self, secao: usize, t: Instant) {
        if self.on {
            self.us[secao] += t.elapsed().as_micros();
        }
    }

    /// Passa `us` da seção `de` para a seção `para` (uma parte medida dentro da outra).
    fn mover(&mut self, de: usize, para: usize, us: u128) {
        if self.on {
            let us = us.min(self.us[de]);
            self.us[de] -= us;
            self.us[para] += us;
        }
    }

    /// Fecha um quadro que começou em `inicio`; a cada segundo imprime a média.
    fn fim_quadro(&mut self, inicio: Instant) {
        if !self.on {
            return;
        }
        self.quadros += 1;
        self.total_us += inicio.elapsed().as_micros();
        let t = self.since.elapsed().as_secs_f32();
        if t >= 1.0 {
            let n = self.quadros.max(1) as f32 * 1000.0;
            let mut linha = format!("[prof] {:.1} quadros/s, {:.2} ms/quadro:", self.quadros as f32 / t, self.total_us as f32 / n);
            for (nome, us) in SECOES.iter().zip(self.us) {
                linha += &format!(" {nome} {:.2}", us as f32 / n);
            }
            println!("{linha}");
            self.since = Instant::now();
            self.quadros = 0;
            self.total_us = 0;
            self.us = [0; SECOES.len()];
        }
    }
}

struct Game {
    server: String,
    name: String,
    session: Option<Session>,
    /// Login Microsoft em andamento (código de dispositivo).
    login_rx: Option<mpsc::Receiver<crate::msa::Event>>,
    menu: crate::menu::Menu,
    stdin_rx: mpsc::Receiver<String>,
    next_favicon: u32,
    table: Arc<BlockTable>,
    atlas: Option<(AtlasImage, Vec<u8>)>,
    jar: Jar,
    ui: UiBatch,
    hud: Hud,
    fps: u32,
    inv: Inventory,
    lang: crate::lang::Lang,
    icons: ItemIcons,
    /// Texturas animadas do atlas (`.mcmeta`), bússola e relógio.
    anim: Animations,
    /// Spawn Position (x, z): a bússola aponta para cá.
    spawn_pos: (i32, i32),
    icon_map: HashMap<IconKey, u32>,
    /// Ícones de cabeça com skin ainda baixando: regravados quando a skin chega.
    heads_pending: Vec<(IconKey, crate::tile_entities::EntityItem)>,
    /// Vagas do atlas das cabeças do tab (uma por linha, até 80) e o que cada uma tem desenhado:
    /// skin (URL ou caminho da padrão), chapéu e de cabeça para baixo.
    tab_heads: Vec<(u32, Option<(String, bool, bool)>)>,
    /// Bloco mirado.
    target: Option<Hit>,
    /// Entidade mirada: id e ponto atingido (relativo aos pés dela).
    target_entity: Option<(i32, DVec3)>,
    /// Quebrando: posição, face e progresso (0..1).
    digging: Option<((i32, i32, i32), u8, f32)>,
    dig_delay: u32,
    left_down: bool,
    right_down: bool,
    /// `KeyBinding.pressTime` do ataque e do uso: cliques ainda não processados pelo `runTick`.
    attack_presses: u32,
    use_presses: u32,
    /// `keyBindPickBlock.pressTime` (clique do meio por padrão).
    pick_presses: u32,
    /// OptiFine (optionsof.txt) do jogador.
    visuals: crate::visuals::Visuals,
    /// `WorldClient.getWorldBorder` (S44).
    border: crate::border::WorldBorder,
    /// `renderViewEntity` vinda do S43 (`None` = o próprio jogador).
    view_entity: Option<i32>,
    /// `PlayerControllerMP.currentBlock`, `currentItemHittingBlock` e `stepSoundTickCounter`.
    current_block: (i32, i32, i32),
    dig_item: Option<mc_protocol::nbt::Slot>,
    dig_step: f32,
    /// `PlayerControllerMP.currentPlayerItem`: o slot que o servidor conhece (C09 só no sync).
    current_player_item: u8,
    /// Slot e item (id, dano) do uso em andamento: trocar a mão cancela o uso (`clearItemInUse`).
    use_slot: Option<(u8, i16, i16)>,
    /// Rachaduras de outras entidades (S25): entidade -> (bloco, estágio, tick em que chegou).
    break_anims: HashMap<i32, ((i32, i32, i32), u8, u64)>,
    /// O placar mudou e os nametags precisam do `NameBoard` novo.
    board_dirty: bool,
    /// `MINE_RS_PRINT_DIR`: pasta onde o teste pede capturas da GPU.
    print_dir: Option<std::path::PathBuf>,
    right_delay: u32,
    /// `Minecraft.leftClickCounter`: depois de bater no ar, 10 ticks sem bater de novo segurando o botão.
    left_click_counter: u32,
    /// Hora do dia (ticks); negativa = parada pelo servidor.
    time_of_day: i64,
    /// `thePlayer.dimension`: o S07 na mesma dimensão não troca o mundo.
    dimension: i32,
    atmos: crate::atmosphere::Atmosphere,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    net: Option<Net>,
    world: World,
    player: Player,
    /// Entidades, lista de jogadores e skins.
    entities: Entities,
    /// Baús, placas, cabeças e estandartes (`tile_entities.rs`).
    tiles: TileEntities,
    keys: HashSet<KeyCode>,
    grabbed: bool,
    shift_held: bool,
    ctrl_held: bool,
    /// `EntityPlayer.isPlayerSleeping` do jogador local (S0A dele liga, animação 2 desliga).
    sleeping: bool,
    /// O `GuiSleepMP` está aberto (é o chat do `hud` mais o botão de acordar).
    sleep_gui: bool,
    spawned: bool,
    /// GuiDownloadTerrain.progress.
    terrain_ticks: u32,
    last_update: Instant,
    tick_acc: f64,
    next_frame: Instant,
    occluded: bool,
    /// MINE_RS_STATS=1: ticks e quadros por segundo no terminal.
    stats: Option<(Instant, u32, u32)>,
    prof: Prof,
    mesh_tx: mpsc::Sender<MeshDone>,
    mesh_rx: mpsc::Receiver<MeshDone>,
    versions: HashMap<Key, u32>,
    /// Visibilidade de cada seção com malha (`MeshData::visibility`); a BFS de `culling` lê daqui.
    vis: HashMap<Key, SetVisibility>,
    fps_frames: u32,
    fps_timer: Instant,
    /// `RenderChunk.renderChunksUpdated`: malhas enviadas neste segundo e no anterior (F3).
    chunk_updates: u32,
    chunk_updates_shown: u32,
    /// Malhas sendo montadas nos trabalhadores.
    mesh_in_flight: usize,
    /// Início do quadro anterior (`FrameTimer`).
    last_frame: Instant,
    /// Dificuldade do mundo (0x01, 0x07, 0x41).
    difficulty: u8,
    /// `GameSettings.hideGUI` (F1; não vai para o options.txt).
    hide_gui: bool,
    /// `RenderManager.debugBoundingBox` (F3+B).
    debug_boxes: bool,
    exit_reason: Option<String>,
    sound: Sound,
    particles: crate::particles::Particles,
    /// Opções já aplicadas (para notar mudanças feitas nas telas).
    applied: Option<crate::settings::Settings>,
    camera: crate::camera::CameraState,
    first_person: crate::first_person::FirstPerson,
    items: crate::first_person::ItemModels,
    test: Option<TestOpts>,
    /// Pacote de recursos baixado do servidor (topo da pilha).
    server_pack: Option<std::path::PathBuf>,
    pack_tx: mpsc::Sender<PackEvent>,
    pack_rx: mpsc::Receiver<PackEvent>,
    /// Sobe a cada saída de servidor: downloads antigos são ignorados.
    pack_gen: u32,
}

/// Andamento do download do pacote do servidor (thread de download -> jogo).
enum PackEvent {
    Progress { generation: u32, message: String, percent: i32 },
    Done { generation: u32, hash: String, result: Result<std::path::PathBuf, String> },
}

pub fn run(opts: Options) -> Result<(), BoxErr> {
    let mut jar = Jar::open(&opts.jar)?;
    crate::res::install(jar.clone());
    let dir = crate::settings::config_dir();
    let mut settings = crate::settings::Settings::load(&dir.join("options.txt"));
    // Pacotes de recursos do options.txt por cima do padrão, antes de ler qualquer textura.
    let chosen = crate::packs::selected(&mut settings);
    for e in jar.set_packs(&crate::packs::stack_paths(&chosen, None)) {
        eprintln!("pacote de recursos ignorado: {e}");
    }
    if !chosen.is_empty() {
        println!("pacotes de recursos: {}", chosen.iter().map(|e| e.name.as_str()).collect::<Vec<_>>().join(", "));
    }
    let mut table = BlockTable::with_models(&mut |p| jar.bytes(p));
    table.set_fancy_leaves(settings.fancy_graphics);
    let table = Arc::new(table);
    let (atlas, missing) = build_atlas(&mut jar, &table.textures)?;
    let atlas_tile = atlas.tile;
    let (ui_rgba, font) = build_ui_atlas(&mut jar);
    if !missing.is_empty() {
        eprintln!("texturas não encontradas no jar: {}", missing.join(", "));
    }
    println!("{} texturas carregadas de {}", table.textures.len(), opts.jar.display());

    crate::lang::install(&settings.language, crate::lang::en_us_text().as_deref());
    let view = settings.render_distance.clamp(2, 32) as u8;
    let net = if opts.test_world { None } else { opts.server.as_ref().map(|s| net::start(s.clone(), opts.name.clone(), opts.session.clone(), view)) };
    let screen = match &opts.server {
        Some(s) => crate::menu::Screen::Connecting(s.clone()),
        None if opts.login => crate::menu::Screen::Login,
        None => crate::menu::Screen::MainMenu,
    };
    let lang_code = settings.language.clone();
    let mut menu = crate::menu::Menu::new(screen.clone(), settings, dir.join("servers.dat"));
    menu.show(screen);
    menu.account = if opts.login { String::new() } else { opts.session.as_ref().map_or(opts.name.clone(), |s| s.name.clone()) };
    menu.login_status = opts.login_error.iter().cloned().collect();
    // Só para prints das telas sem entrar em servidor: MINE_RS_TELA=pausa|conectando|opcoes.
    match std::env::var("MINE_RS_TELA").as_deref() {
        Ok("pausa") => menu.show(crate::menu::Screen::Pause),
        Ok("conectando") => menu.show(crate::menu::Screen::Connecting(String::new())),
        Ok("opcoes") => menu.show(crate::menu::Screen::Options),
        Ok("pacotes") => {
            menu.show(crate::menu::Screen::Options);
            menu.open_packs();
        }
        _ => {}
    }
    if let Some(t) = jar.bytes("assets/minecraft/texts/splashes.txt") {
        menu.splash = crate::menu::pick_splash(&String::from_utf8_lossy(&t));
    }

    // Chat pelo terminal: cada linha digitada vai para o servidor conectado.
    let (stdin_tx, stdin_rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in std::io::stdin().lines().map_while(Result::ok) {
            let msg: String = line.trim().chars().take(100).collect();
            if !msg.is_empty() && stdin_tx.send(msg).is_err() {
                break;
            }
        }
    });

    let (mesh_tx, mesh_rx) = mpsc::channel();
    let (pack_tx, pack_rx) = mpsc::channel();
    let anim = Animations::load(&mut jar, &table.textures, atlas_tile);
    let sound = Sound::start(jar.clone());
    let mut game = Game {
        server: opts.server.unwrap_or_default(),
        // Com sessão, o nome é o do perfil (o mesmo do login), não o --name.
        name: opts.session.as_ref().map_or(opts.name, |s| s.name.clone()),
        session: opts.session,
        login_rx: None,
        menu,
        stdin_rx,
        next_favicon: mc_render::ui::ITEM_SLOTS - 1,
        table,
        atlas: Some((atlas, ui_rgba)),
        jar,
        ui: UiBatch::new(font),
        hud: Hud::default(),
        fps: 0,
        inv: Inventory::default(),
        lang: crate::lang::Lang::load(&lang_code),
        icons: ItemIcons::default(),
        anim,
        spawn_pos: (0, 0),
        icon_map: HashMap::new(),
        heads_pending: Vec::new(),
        tab_heads: Vec::new(),
        target: None,
        target_entity: None,
        digging: None,
        dig_delay: 0,
        left_down: false,
        right_down: false,
        attack_presses: 0,
        use_presses: 0,
        pick_presses: 0,
        print_dir: std::env::var_os("MINE_RS_PRINT_DIR").map(std::path::PathBuf::from),
        visuals: crate::visuals::Visuals::load(),
        border: Default::default(),
        view_entity: None,
        current_block: (-1, -1, -1),
        break_anims: HashMap::new(),
        board_dirty: true,
        dig_item: None,
        dig_step: 0.0,
        current_player_item: 0,
        use_slot: None,
        left_click_counter: 0,
        right_delay: 0,
        time_of_day: 6000,
        dimension: 0,
        atmos: Default::default(),
        window: None,
        renderer: None,
        net,
        world: World::new(true),
        player: Player::default(),
        entities: Entities::default(),
        tiles: TileEntities::default(),
        keys: HashSet::new(),
        grabbed: false,
        shift_held: false,
        ctrl_held: false,
        sleeping: false,
        sleep_gui: false,
        spawned: false,
        terrain_ticks: 0,
        last_update: Instant::now(),
        tick_acc: 0.0,
        next_frame: Instant::now(),
        occluded: false,
        stats: std::env::var_os("MINE_RS_STATS").map(|_| (Instant::now(), 0, 0)),
        prof: Prof::new(),
        mesh_tx,
        mesh_rx,
        versions: HashMap::new(),
        vis: HashMap::new(),
        fps_frames: 0,
        fps_timer: Instant::now(),
        chunk_updates: 0,
        chunk_updates_shown: 0,
        mesh_in_flight: 0,
        last_frame: Instant::now(),
        difficulty: 2,
        hide_gui: false,
        debug_boxes: false,
        exit_reason: None,
        sound,
        particles: crate::particles::Particles::default(),
        applied: None,
        camera: Default::default(),
        first_person: Default::default(),
        items: Default::default(),
        test: None,
        server_pack: None,
        pack_tx,
        pack_rx,
        pack_gen: 0,
    };
    if opts.test_world {
        game.start_test_world();
    }
    // Só para prints dos tile entities sem servidor: MINE_RS_MUNDO=tiles.
    if std::env::var("MINE_RS_MUNDO").as_deref() == Ok("tiles") {
        crate::tile_entities::demo(&mut game.world, &mut game.tiles, &mut game.player);
        game.spawned = true;
        game.menu.screen = crate::menu::Screen::None;
    }
    // OptiFine do jogador vale para o céu, a câmera e o HUD.
    game.atmos.vis = game.visuals;
    game.hud.vis = game.visuals;
    game.camera.dynamic_fov = game.visuals.dynamic_fov;
    // Só para prints: mundo plano local com blocos que soltam partículas, sem servidor.
    if std::env::var_os("MINE_RS_DEMO_PARTICULAS").is_some() {
        game.demo_particles_world();
    }
    // MINE_RS_DEMO=1: mundo local de demonstração, sem servidor (só para prints).
    if std::env::var_os("MINE_RS_DEMO").is_some() {
        game.world = crate::demo::world();
        game.player.pos = glam::DVec3::new(0.5, 67.0, 12.0);
        game.player.prev = game.player.pos;
        game.player.yaw = 180.0;
        game.player.pitch = 45.0;
        game.menu.screen = crate::menu::Screen::None;
        game.spawned = true;
    }
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now()));
    event_loop.run_app(&mut game)?;
    if let Some(r) = game.exit_reason {
        println!("desconectado: {}", crate::hud::plain(&r));
    }
    Ok(())
}

impl Game {
    fn start_test_world(&mut self) {
        use crate::test_world::{env, held};
        self.world = crate::test_world::build();
        // Id do próprio jogador no mundo de teste (o anzol de MINE_RS_TEST_OBJ aponta para ele).
        self.entities.own_id = Some(-2);
        let cols: Vec<(i32, i32)> = self.world.columns.keys().copied().collect();
        for (cx, cz) in cols {
            self.tiles.scan_column(&self.world, cx, cz);
        }
        self.spawned = true;
        self.menu.screen = crate::menu::Screen::None;
        let (yaw, pitch) = env("LOOK")
            .and_then(|v| v.split_once(',').and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?))))
            .unwrap_or((0.0, 10.0));
        self.player.teleport(DVec3::new(0.5, 64.0, 0.5), yaw, pitch);
        self.player.allow_flying = true;
        self.hud.health = 20.0;
        // MINE_RS_TEST_PARTICULAS: semente fixa para as execuções darem a mesma imagem.
        if crate::particles::Particles::test_hook().is_some() {
            self.particles = crate::particles::Particles::new(0x5eed);
        }
        // Servidor integrado: o tab desenha as cabeças.
        self.hud.tab.heads = true;
        if let Some(v) = env("TAB") {
            self.test_tab(&v);
        }
        // MINE_RS_TEST_MAPA=id: um 0x34 de teste com faixas de cores e dois ícones (tipo 0 e tipo 1).
        if let Some(id) = env("MAPA").and_then(|v| v.parse().ok()) {
            let data: Vec<u8> = (0..16384usize).map(|i| if (i % 128) < 8 { 0 } else { ((((i % 128) / 16 + (i / 128) / 16) % 35 + 1) * 4 + (i / 128 / 32) % 4) as u8 }).collect();
            self.entities.apply_map(crate::maps::MapPacket { id, scale: 0, icons: vec![[0, 0, 0, 0], [1, 40, -40, 4]], patch: Some((128, 128, 0, 0, data)) });
        }
        if env("FOGO").is_some() {
            self.hud.track.flags |= 1;
        }
        // Com fome (10) para dar para comer no teste.
        self.hud.food = 10;
        self.player.food_level = 10;
        self.hud.gamemode = 1;
        // MINE_RS_TEST_MORTE=pontos: jogador morto, para ver o GuiGameOver.
        if let Some(score) = env("MORTE") {
            self.player.set_health(0.0);
            self.hud.track.score = score.parse().unwrap_or(0);
            self.hud.hardcore = env("HARDCORE").is_some();
        }
        self.inv.player[36] = held();
        for (i, s) in crate::test_world::hotbar().into_iter().take(8).enumerate() {
            self.inv.player[37 + i] = Some(s);
        }
        if let Some(a) = env("ARMOR") {
            for (i, id) in a.split(',').enumerate().take(4) {
                let id: i16 = id.parse().unwrap_or(0);
                self.inv.player[5 + i] = (id > 0).then(|| mc_protocol::nbt::Slot { id, count: 1, damage: 0, nbt: None, raw_nbt: Vec::new() });
            }
        }
        if let Some(m) = env("MOBS") {
            self.entities.spawn_test_mobs(&m);
        }
        if let Some(m) = env("OBJ") {
            self.entities.spawn_test_objects(&m);
        }
        // MINE_RS_TEST_LACO=bx,by,bz: nó de coleira no bloco, uma vaca presa a ele e um porco preso ao jogador.
        if let Some(m) = env("LACO") {
            self.entities.spawn_test_leash(&m);
        }
        // MINE_RS_TEST_JANELA=tipo[@tamanho[@prop=valor,...]]: abre a janela (S2D) e aplica as propriedades (S31).
        if let Some(v) = env("JANELA") {
            let mut parts = v.split('@');
            let kind = parts.next().unwrap_or("").to_string();
            let size = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            self.inv.env = self.inv_env();
            self.inv.open_window(1, &kind, kind.clone(), size, -1);
            for kv in parts.next().unwrap_or("").split(',').filter_map(|kv| kv.split_once('=')) {
                if let (Ok(p), Ok(x)) = (kv.0.parse(), kv.1.parse()) {
                    self.inv.set_property(1, p, x);
                }
            }
        }
        // MINE_RS_TEST_CAVALO=1: abre o inventário do primeiro cavalo de MINE_RS_TEST_MOBS (com 100).
        if env("CAVALO").is_some() {
            let horse = self.entities.list.iter().find(|(_, e)| e.kind == crate::entities::Kind::Mob(100)).map(|(id, _)| *id);
            if let Some(id) = horse {
                self.inv.env = self.inv_env();
                self.inv.open_window(1, "EntityHorse", "Cavalo".into(), 2, id);
            }
        }
        // MINE_RS_TEST_EFEITOS=id:amplificador,...: efeitos de poção no jogador (o painel e a posição do inventário).
        for e in env("EFEITOS").unwrap_or_default().split(',') {
            let mut p = e.split(':').map(|v| v.trim().parse::<i32>().unwrap_or(0));
            if let (Some(id), amp) = (p.next().filter(|&id| id > 0), p.next().unwrap_or(0)) {
                self.hud.effects.add(crate::potion::Effect { id: id as u8, amplifier: amp as i8, duration: 1200, show_particles: true, duration_max: false });
            }
        }
        // MINE_RS_TEST_INV=aba: abre o inventário (criativo na aba dada, ex. 5 = busca, 11 = inventário).
        if let Some(tab) = env("INV") {
            crate::gui_creative::set_selected(tab.parse().unwrap_or(0));
            self.inv.env = self.inv_env();
            self.inv.open_own();
        }
        // MINE_RS_TELA_ESPECIAL=placa|placa_parede|livro|livro_edit|livro_assinar|cama: abre a tela no mundo de teste.
        if let Ok(name) = std::env::var("MINE_RS_TELA_ESPECIAL") {
            let hand = 36 + self.hud.selected as usize;
            if crate::gui_special::test_hook(&mut self.inv, &name, &self.name, hand) {
                self.sleeping = true;
            }
        }
        if let Some(spec) = env("HUD") {
            self.hud.apply_test(&spec);
        }
        // MINE_RS_TEST_CHAT=demo|@arquivo: mensagens JSON do servidor; MINE_RS_TEST_CHAT_OPEN=texto abre o chat.
        for json in crate::test_world::chat_messages() {
            let comp = crate::chat::Component::from_json(&json);
            self.hud.add_chat(&mut self.ui, &comp);
        }
        if let Some(prefix) = env("CHAT_OPEN") {
            self.hud.open_chat(if prefix == "1" { "" } else { &prefix });
        }
        self.camera.third_person = env("VIEW").and_then(|v| v.parse().ok()).unwrap_or(0);
        let keys = env("KEYS").unwrap_or_default();
        let has = |k: &str| keys.split(',').any(|x| x == k);
        let k = Keys { forward: has("w"), back: has("s"), left: has("a"), right: has("d"), jump: has("space"), sneak: has("shift"), sprint: has("ctrl") };
        self.test = Some(TestOpts { keys: k, swing: env("SWING").is_some(), use_item: env("USE").is_some() });
    }

    fn send(&self, p: Vec<u8>) {
        if let Some(n) = &self.net {
            let _ = n.out.send(p);
        }
    }

    fn set_grab(&mut self, on: bool) {
        let Some(w) = &self.window else { return };
        // Minecraft.setIngameFocus: só com a janela ativa (Display.isActive); senão o clique captura depois.
        if on && !w.has_focus() {
            return;
        }
        if on {
            if w.set_cursor_grab(CursorGrabMode::Locked).is_err() {
                let _ = w.set_cursor_grab(CursorGrabMode::Confined);
            }
            w.set_cursor_visible(false);
        } else {
            // MouseHelper.ungrabMouseCursor (só ao sair do jogo, como setIngameNotInFocus): cursor no centro.
            if self.grabbed {
                let size = w.inner_size();
                let center = winit::dpi::PhysicalPosition::new(size.width as f64 / 2.0, size.height as f64 / 2.0);
                let _ = w.set_cursor_position(center);
                let s = self.gui_scale() as f64;
                self.inv.mouse = ((center.x / s) as f32, (center.y / s) as f32);
                self.menu.mouse_move(self.inv.mouse.0, self.inv.mouse.1);
            }
            let _ = w.set_cursor_grab(CursorGrabMode::None);
            w.set_cursor_visible(true);
            // setIngameNotInFocus -> KeyBinding.unPressAllKeys: nada fica apertado com a tela aberta.
            self.keys.clear();
            self.left_down = false;
            self.right_down = false;
            self.attack_presses = 0;
            self.use_presses = 0;
            self.pick_presses = 0;
        }
        self.grabbed = on;
    }

    fn reset_world(&mut self, dimension: i32) {
        self.world = World::new(dimension == 0);
        self.border = Default::default();
        self.view_entity = None;
        self.atmos.reset(dimension);
        self.versions.clear();
        self.vis.clear();
        self.entities.clear();
        self.tiles.clear();
        self.particles.clear();
        self.spawned = false;
        if let Some(r) = &mut self.renderer {
            r.clear();
        }
    }

    fn send_abilities(&self) {
        // C13PacketPlayerAbilities: invulnerável, voando, pode voar, criativo.
        let p = &self.player;
        let flags = p.invulnerable as u8 | (p.flying as u8) << 1 | (p.allow_flying as u8) << 2 | (p.creative as u8) << 3;
        self.send(serverbound::abilities(flags, p.fly_speed, p.walk_speed));
    }

    /// Processa os eventos da rede. Devolve o motivo se a conexão caiu.
    fn pump_network(&mut self) -> Option<String> {
        for _ in 0..4000 {
            let Some(net) = &self.net else { break };
            let Ok(ev) = net.events.try_recv() else { break };
            match ev {
                NetEvent::Status(s) => println!("{s}"),
                NetEvent::Encrypted(on) => self.hud.tab.heads = on,
                NetEvent::Map(p) => self.entities.apply_map(p),
                NetEvent::Difficulty(d) => self.difficulty = d,
                NetEvent::ReducedDebug(on) => self.player.reduced_debug = on,
                // handleJoinGame: mundo novo sempre, jogador novo, C15 e o MC|Brand.
                NetEvent::Joined { dimension } => {
                    self.dimension = dimension;
                    self.reset_world(dimension);
                    // WorldClient novo, Scoreboard novo (o S07 de outra dimensão passa o antigo adiante).
                    self.hud.scoreboard = crate::hud::Scoreboard { me: self.name.clone(), ..Default::default() };
                    self.board_dirty = true;
                    self.hud.on_respawn();
                    self.new_player();
                    self.download_terrain();
                    self.send_client_settings();
                    self.send(serverbound::brand("vanilla"));
                }
                // handleRespawn: só troca o mundo se a dimensão mudou (o S07 na mesma dimensão dos plugins de skin/nick
                // mantém chunks e entidades); o jogador é sempre novo, com o modo de jogo do pacote.
                NetEvent::Respawn { dimension, gamemode } => {
                    let other = dimension != self.dimension;
                    if other {
                        self.dimension = dimension;
                        self.reset_world(dimension);
                    }
                    self.hud.on_respawn();
                    self.hud.gamemode = gamemode;
                    self.new_player();
                    if other {
                        self.download_terrain();
                    }
                }
                // handleEntityProperties no próprio jogador: base e modificadores do servidor (removeAllModifiers).
                NetEvent::SpeedAttribute { entity, base, modifiers } => {
                    if Some(entity) == self.entities.own_id {
                        self.player.speed.base = base;
                        self.player.speed.modifiers = modifiers;
                    }
                }
                NetEvent::Hardcore(h) => self.hud.hardcore = h,
                NetEvent::PotionEffect { entity, effect } => {
                    if Some(entity) == self.entities.own_id {
                        self.hud.effects.add(effect);
                    }
                }
                NetEvent::RemoveEffect { entity, id } => {
                    if Some(entity) == self.entities.own_id {
                        self.hud.effects.remove(id);
                    }
                }
                NetEvent::MaxHealth { entity, value } => {
                    if Some(entity) == self.entities.own_id {
                        self.hud.max_health = value;
                    }
                    self.hud.track.max_health.insert(entity, value);
                }
                NetEvent::Statistics(v) => {
                    // A descrição da dica leva a tecla do inventário (o IStatStringFormat do Minecraft.startGame).
                    let key = crate::settings::key_name(self.menu.settings.key("key.inventory"));
                    let show = self.menu.settings.show_inventory_achievement_hint;
                    if self.hud.achievements.statistics(&v, show, || crate::lang::trf("achievement.openInventory.desc", &[&key])) {
                        self.menu.settings.show_inventory_achievement_hint = false;
                        self.menu.settings.save();
                    }
                }
                NetEvent::Chunk(c) => {
                    let at = (c.x, c.z);
                    self.world.apply(c);
                    self.tiles.scan_column(&self.world, at.0, at.1);
                }
                NetEvent::Chunks(v) => {
                    for c in v {
                        let at = (c.x, c.z);
                        self.world.load(c);
                        self.tiles.scan_column(&self.world, at.0, at.1);
                    }
                }
                NetEvent::BlockChange { x, y, z, state } => {
                    self.world.set_block(x, y, z, state);
                    self.tiles.block_changed(x, y, z, state);
                }
                NetEvent::MultiBlockChange(v) => {
                    for (x, y, z, s) in v {
                        self.world.set_block(x, y, z, s);
                        self.tiles.block_changed(x, y, z, s);
                    }
                }
                // `handleUpdateSign`: a placa em edição (`setEditable(false)`) ignora o servidor.
                NetEvent::Tile(crate::tile_entities::TileEvent::Sign { pos, .. }) if self.inv.editing_sign().is_some_and(|(p, _)| p == pos) => {}
                NetEvent::Tile(t) => {
                    let mut sounds = Vec::new();
                    self.tiles.handle(t, &self.world, &mut sounds);
                    for s in sounds {
                        self.sound.play(&s.name, s.volume, s.pitch, s.pos);
                    }
                }
                NetEvent::PosLook { x, y, z, yaw, pitch, flags } => {
                    // NetHandlerPlayClient.handlePlayerPosLook: eixo absoluto zera a velocidade.
                    let p = &mut self.player;
                    let mut d = p.pos;
                    if flags & 1 != 0 { d.x += x } else { d.x = x; p.vel.x = 0.0 }
                    if flags & 2 != 0 { d.y += y } else { d.y = y; p.vel.y = 0.0 }
                    if flags & 4 != 0 { d.z += z } else { d.z = z; p.vel.z = 0.0 }
                    let yaw = if flags & 8 != 0 { p.yaw + yaw } else { yaw };
                    let pitch = if flags & 16 != 0 { p.pitch + pitch } else { pitch };
                    p.teleport(d, yaw, pitch);
                    // Entity.setRotation: yaw e pitch % 360 (o prevRotation fica com o valor cheio).
                    p.yaw = yaw % 360.0;
                    p.pitch = pitch % 360.0;
                    let (pos, rot, feet) = (p.pos, (p.yaw, p.pitch), p.bb.min.y);
                    self.send(serverbound::player_pos_look(pos.x, feet, pos.z, rot.0, rot.1, false));
                    // !doneLoadingTerrain: displayGuiScreen(null), feche o que estiver aberto.
                    if !self.spawned {
                        println!("no mundo em {:.1} {:.1} {:.1}", pos.x, pos.y, pos.z);
                        if matches!(self.menu.screen, crate::menu::Screen::Connecting(_) | crate::menu::Screen::DownloadTerrain) {
                            self.menu.screen = crate::menu::Screen::None;
                        }
                        self.inv.close_silently();
                        self.hud.close_chat();
                        if self.menu.screen == crate::menu::Screen::None {
                            self.set_grab(true);
                        }
                    }
                    self.spawned = true;
                }
                NetEvent::Chat { text, comp, position } => {
                    if position == 2 {
                        // setRecordPlaying(getUnformattedText()): a cor só vem dos códigos § dentro do texto.
                        let _ = text;
                        self.hud.action_bar = Some((comp.unformatted_text(), self.hud.ticks));
                    } else {
                        if !text.trim().is_empty() {
                            println!("[chat] {}", crate::hud::plain(&text));
                        }
                        self.hud.add_chat(&mut self.ui, &comp);
                    }
                }
                NetEvent::TabComplete(names) => self.hud.chat_tab_response(&mut self.ui, &names),
                NetEvent::Health { health, food, saturation } => {
                    self.hud.set_health(health, food, saturation, self.player.has_valid_health());
                    self.player.set_health(health);
                    self.player.food_level = food as i32;
                }
                NetEvent::GameMode(m) => {
                    self.hud.gamemode = m;
                    self.player.spectator = m == 3;
                }
                NetEvent::OpenWindow { id, kind, title, size, entity } => {
                    self.inv.open_window(id, &kind, title, size as usize, entity);
                    self.set_grab(false);
                }
                NetEvent::Container(e) => self.container_event(e),
                // handleCloseWindow -> closeScreenAndDropStack: fecha a tela aberta sem olhar o id da janela.
                NetEvent::CloseWindow(_) => {
                    let had_screen = self.inv.is_open() || self.hud.chat_open;
                    if self.inv.is_open() {
                        self.inv.close_silently();
                    }
                    if self.hud.chat_open {
                        self.hud.close_chat();
                    }
                    if had_screen && self.menu.screen == crate::menu::Screen::None {
                        self.set_grab(true);
                    }
                }
                NetEvent::SetSlot { window, slot, item } => self.inv.set_slot(window, slot, item),
                NetEvent::WindowItems { window, items } => self.inv.set_items(window, items),
                // handleConfirmTransaction: janela 0 (inventário) ou a aberta, recusada -> C0F aceito.
                NetEvent::Transaction { window, action } => {
                    if window == 0 || window == self.inv.window_id() {
                        self.send(serverbound::confirm_transaction(window, action));
                    }
                }
                // handleHeldItemChange: só índices da hotbar; o sync do tick ecoa o C09 como o Java.
                NetEvent::HeldSlot(s) if s < 9 => self.hud.selected = s,
                NetEvent::HeldSlot(_) => {}
                NetEvent::Experience { bar, level } => {
                    self.hud.xp_bar = bar;
                    self.hud.xp_level = level;
                }
                // handleTitle: os tempos do pacote (0 fora do TIMES) vão junto; o CLEAR do 1.8.9 só zera os tempos;
                // o RESET mostra o título vazio pelo tempo atual e volta aos tempos padrão.
                NetEvent::Title { action, text, times } => match action {
                    0 => self.hud.display_title(Some(text), None, times.0, times.1, times.2),
                    1 => self.hud.display_title(None, Some(text), times.0, times.1, times.2),
                    2 | 3 => self.hud.display_title(None, None, times.0, times.1, times.2),
                    4 => {
                        self.hud.display_title(Some(String::new()), Some(String::new()), -1, -1, -1);
                        self.hud.title_times = (10, 70, 20);
                    }
                    _ => {}
                },
                NetEvent::TabHeader { header, footer } => {
                    self.hud.tab_header = header;
                    self.hud.tab_footer = footer;
                }
                NetEvent::Objective { name, mode, display, hearts } => {
                    // handleScoreboardObjective: addScoreObjective de um nome que já existe lança exceção e o pacote
                    // se perde; atualizar ou remover um que não existe também (getObjective nulo).
                    self.board_dirty = true;
                    let sb = &mut self.hud.scoreboard;
                    let exists = sb.objectives.contains_key(&name);
                    match mode {
                        0 | 2 if exists == (mode == 2) => {
                            if hearts {
                                sb.hearts.insert(name.clone());
                            } else {
                                sb.hearts.remove(&name);
                            }
                            sb.objectives.insert(name, display);
                        }
                        1 if exists => {
                            sb.objectives.remove(&name);
                            sb.scores.remove(&name);
                            sb.hearts.remove(&name);
                            // Scoreboard.removeObjective: sai também dos 19 espaços de exibição.
                            for slot in [&mut sb.list, &mut sb.sidebar, &mut sb.below_name].into_iter().chain(sb.team_sidebar.iter_mut()) {
                                if slot.as_ref() == Some(&name) {
                                    *slot = None;
                                }
                            }
                        }
                        _ => {}
                    }
                }
                NetEvent::Score { player, objective, value } => {
                    self.board_dirty = true;
                    let sb = &mut self.hud.scoreboard;
                    match value {
                        // getValueFromObjective com objetivo nulo guarda o ponto fora de qualquer objetivo: nunca aparece.
                        Some(v) => {
                            if sb.objectives.contains_key(&objective) {
                                sb.scores.entry(objective).or_default().insert(player, v);
                            }
                        }
                        // Remover sem objetivo tira o jogador de todos.
                        None if objective.is_empty() => sb.scores.values_mut().for_each(|m| {
                            m.remove(&player);
                        }),
                        None => {
                            if let Some(m) = sb.scores.get_mut(&objective) {
                                m.remove(&player);
                            }
                        }
                    }
                }
                NetEvent::DisplayObjective { position, name } => {
                    // getObjective de um nome desconhecido dá nulo: o espaço fica vazio.
                    let slot = (!name.is_empty() && self.hud.scoreboard.objectives.contains_key(&name)).then_some(name);
                    match position {
                        0 => self.hud.scoreboard.list = slot,
                        1 => self.hud.scoreboard.sidebar = slot,
                        2 => self.hud.scoreboard.below_name = slot,
                        3..=18 => self.hud.scoreboard.team_sidebar[position as usize - 3] = slot,
                        _ => {}
                    }
                    self.board_dirty = true;
                }
                NetEvent::Team { name, mode, prefix, players } => {
                    // handleTeams: createTeam de um time que já existe lança exceção (o pacote inteiro se perde);
                    // os outros modos num time que não existe também não fazem nada.
                    let sb = &mut self.hud.scoreboard;
                    let exists = sb.teams.contains_key(&name);
                    match mode {
                        0 | 2 if exists == (mode == 2) => {
                            let team = sb.teams.entry(name.clone()).or_default();
                            if let Some((p, s, vis, color)) = prefix {
                                team.prefix = p;
                                team.suffix = s;
                                team.visibility = vis;
                                team.color = color;
                            }
                            if mode == 0 {
                                for pl in players {
                                    sb.player_team.insert(pl, name.clone());
                                }
                            }
                        }
                        3 if exists => {
                            for pl in players {
                                sb.player_team.insert(pl, name.clone());
                            }
                        }
                        // removePlayerFromTeam: jogador em outro time (ou em nenhum) lança exceção e para o laço.
                        4 => {
                            for pl in players {
                                let team = sb.player_team.get(&pl);
                                if team.map(|t| t == &name) != Some(true) {
                                    if team.is_some() || exists {
                                        break;
                                    }
                                    continue;
                                }
                                sb.player_team.remove(&pl);
                            }
                        }
                        1 if exists => {
                            sb.teams.remove(&name);
                            sb.player_team.retain(|_, t| *t != name);
                        }
                        _ => {}
                    }
                    self.board_dirty = true;
                }
                // handleCollectItem: som do item (ou do orbe) e a entidade sai do mundo.
                NetEvent::Collect { collected, collector } => {
                    if let Some(e) = self.entities.list.get_mut(&collected).filter(|e| e.obj.pickup.is_none()) {
                        let orb = matches!(e.kind, crate::entities::Kind::Object { kind: crate::naovivas::XP_ORB, .. });
                        let mut rng = rand::thread_rng();
                        use rand::Rng;
                        let pitch = ((rng.gen::<f32>() - rng.gen::<f32>()) * 0.7 + 1.0) * 2.0;
                        self.sound.play(if orb { "random.orb" } else { "random.pop" }, 0.2, pitch, e.pos);
                        // EntityPickupFX: sai do mundo (para de andar) e voa até quem coletou por 3 ticks.
                        e.obj.pickup = Some((collector, 0));
                        e.prev = e.pos;
                    }
                }
                // World.sendBlockBreakProgress das outras entidades (RenderGlobal.damagedBlocks).
                NetEvent::BreakAnim { entity, pos, stage } => {
                    if (0..10).contains(&stage) {
                        self.break_anims.insert(entity, (pos, stage as u8, self.hud.ticks));
                    } else {
                        self.break_anims.remove(&entity);
                    }
                }
                NetEvent::ArrowHit => {
                    let at = self.eye();
                    self.sound.play("random.successful_hit", 0.18, 0.45, at);
                }
                NetEvent::Abilities { flags, fly_speed, walk_speed } => {
                    self.player.invulnerable = flags & 1 != 0;
                    self.player.flying = flags & 2 != 0;
                    self.player.allow_flying = flags & 4 != 0;
                    self.player.creative = flags & 8 != 0;
                    self.player.fly_speed = fly_speed;
                    self.player.walk_speed = walk_speed;
                }
                NetEvent::Time { time, age } => {
                    self.entities.world_age = age;
                    self.time_of_day = time;
                    self.atmos.set_time(time);
                }
                NetEvent::Weather { reason, value } => self.atmos.game_state(reason, value),
                // `handleAnimation` tipo 2: `wakeUpPlayer` (o `GuiSleepMP` fecha no próximo tick).
                NetEvent::Entity(crate::entity_net::EntityPacket::Animation { id, kind: 2 }) if Some(id) == self.entities.own_id => self.sleeping = false,
                NetEvent::Entity(crate::entity_net::EntityPacket::Animation { id, kind }) if kind == 4 || kind == 5 => {
                    self.particles.setting = self.menu.settings.particles;
                    penv!(self, env, { self.particles.emit_at_entity(&env, id, if kind == 4 { 9 } else { 10 }) });
                }
                NetEvent::Particles(p) => {
                    self.particles.setting = self.menu.settings.particles;
                    self.particles.prepare_item_sprite(&mut self.jar, &p);
                    penv!(self, env, { self.particles.handle_particles(&env, &p) });
                }
                NetEvent::Effect { kind, pos, data } => {
                    self.particles.setting = self.menu.settings.particles;
                    penv!(self, env, { self.particles.effect(&env, kind, pos, data) });
                }
                NetEvent::Explosion(e) => {
                    self.particles.setting = self.menu.settings.particles;
                    penv!(self, env, { self.particles.explosion(&env, &e) });
                    // `Explosion.doExplosionB`: os blocos afetados somem no cliente; depois o empurrão.
                    for &(x, y, z) in &e.blocks {
                        self.world.set_block(x, y, z, 0);
                        self.tiles.block_changed(x, y, z, 0);
                    }
                    self.player.vel += DVec3::new(e.push[0] as f64, e.push[1] as f64, e.push[2] as f64);
                }
                NetEvent::SpawnPos { x, z } => self.spawn_pos = (x, z),
                NetEvent::WorldBorder(p) => self.border.apply(&p),
                // handleCamera: só troca se a entidade existe; o próprio id volta ao jogador.
                NetEvent::Camera(id) => {
                    if Some(id) == self.entities.own_id {
                        self.view_entity = None;
                    } else if self.entities.list.contains_key(&id) {
                        self.view_entity = Some(id);
                    }
                }
                // RenderGlobal.broadcastSound: a 2 blocos do jogador, na direção da fonte.
                NetEvent::GlobalSound { kind, pos } => {
                    let me = self.player.pos;
                    let d = DVec3::new(pos.0 as f64, pos.1 as f64, pos.2 as f64) - me;
                    let len = d.length();
                    let at = if len > 0.0 { me + d / len * 2.0 } else { me };
                    if kind == 1013 {
                        self.sound.play("mob.wither.spawn", 1.0, 1.0, at);
                    } else {
                        self.sound.play("mob.enderdragon.end", 5.0, 1.0, at);
                    }
                }
                NetEvent::Entity(e) => {
                    // handleEntityMetadata no próprio jogador: o DataWatcher troca a flag de corrida (bit 3) sem
                    // mexer no modificador de velocidade (que vem pelo S20); o eco do servidor pode religar a corrida.
                    if let crate::entity_net::EntityPacket::Metadata { id, meta } = &e {
                        if Some(*id) == self.entities.own_id {
                            if let Some((_, crate::entity_net::MetaValue::Byte(b))) = meta.iter().find(|(k, _)| *k == 0) {
                                self.player.sprinting = b & 8 != 0;
                            }
                        }
                    }
                    self.hud.entity_packet(&e, self.entities.own_id);
                    // EntityLivingBase.handleStatusUpdate(2): o cliente toca o som de dano da própria entidade (além do
                    // que o servidor mandar), com o tom (rand - rand) * 0.2 + 1.
                    if let crate::entity_net::EntityPacket::Status { id, status: 2 } = &e {
                        let own = Some(*id) == self.entities.own_id;
                        let hurt = if own { Some(("game.player.hurt".to_string(), 1.0, self.player.pos)) } else { self.entities.list.get(id).and_then(|en| crate::entities::hurt_sound(en).map(|(n, v)| (n, v, en.pos))) };
                        if let Some((name, vol, at)) = hurt {
                            let pitch = (rand::random::<f32>() - rand::random::<f32>()) * 0.2 + 1.0;
                            self.sound.play(&name, vol, pitch, at);
                        }
                    }
                    self.entities.handle(e, &mut self.player);
                }
                NetEvent::Sound(s) => self.sound.play(&s.name, s.volume, s.pitch, s.pos),
                NetEvent::ResourcePack { url, hash } => self.on_resource_pack(url, hash),
                NetEvent::Disconnected(r) => return Some(r),
            }
        }
        for (cx, cz) in self.world.removed.drain(..) {
            self.tiles.unload_column(cx, cz);
            self.vis.retain(|k, _| (k.0, k.2) != (cx, cz));
            if let Some(r) = &mut self.renderer {
                r.remove_column(cx, cz);
            }
        }
        None
    }

    /// `TextureMap.updateAnimations`: um passo das texturas animadas, no ritmo dos ticks do jogo.
    fn tick_animations(&mut self) {
        // OptiFine ofAnimatedWater/Lava/Fire desligados: a textura fica no primeiro quadro.
        let tile = |n: &str| self.table.textures.iter().position(|t| t == n).map(|i| i as u16);
        let mut parados = Vec::new();
        for (on, names) in [(self.visuals.anim_water, ["water_still", "water_flow"]), (self.visuals.anim_lava, ["lava_still", "lava_flow"]), (self.visuals.anim_fire, ["fire_layer_0", "fire_layer_1"])] {
            if !on {
                parados.extend(names.iter().filter_map(|n| tile(n)));
            }
        }
        if let Some(r) = &mut self.renderer {
            for (tile, levels) in self.anim.tick_blocks() {
                if parados.contains(&tile) {
                    continue;
                }
                r.write_atlas_tile(tile, levels);
            }
        }
        // Math.random() só é usado fora da superfície (nether e end).
        let random = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.5, |d| (d.subsec_nanos() % 1_000_000) as f64 / 1_000_000.0);
        let surface = self.world.has_sky;
        let compass_world = self.spawned.then(|| CompassWorld { spawn: self.spawn_pos, x: self.player.pos.x, z: self.player.pos.z, yaw: self.player.yaw as f64, surface });
        let celestial = self.spawned.then(|| celestial_angle(self.time_of_day.abs()));
        let mut live: Vec<(i16, Vec<u8>)> = Vec::new();
        if let Some(c) = &mut self.anim.compass {
            if let Some(f) = c.update(compass_world, random) {
                live.push((345, c.frame_rgba(f).to_vec()));
            }
        }
        if let Some(c) = &mut self.anim.clock {
            if let Some(f) = c.update(celestial, surface, random) {
                live.push((347, c.frame_rgba(f).to_vec()));
            }
        }
        for (id, rgba) in live {
            let side = ((rgba.len() / 4) as f64).sqrt() as u32;
            let Some(frame) = image::RgbaImage::from_raw(side, side, rgba) else { continue };
            // O ícone ocupa ITEM_PX lógicos no atlas da GUI: o quadro cobre a vaga inteira.
            let px = mc_render::ui::ITEM_PX;
            let icon = if side == px { frame.clone() } else { image::imageops::resize(&frame, px, px, image::imageops::FilterType::Nearest) };
            for s in self.icons.slots_of(id) {
                let (x, y) = icon_origin(s);
                if let Some(r) = &mut self.renderer {
                    r.write_ui(x, y, px, px, icon.as_raw());
                }
            }
            let img = icon;
            if let Some(r) = &mut self.renderer {
                self.items.set_live(id, frame, r);
            }
            self.icons.live.insert(id, img);
        }
    }

    /// `InventoryPlayer.getTotalArmorValue`: soma `damageReduceAmount` do capacete às botas.
    fn armor_value(&self) -> i32 {
        (5..=8)
            .filter_map(|i| self.inv.player.get(i).and_then(Option::as_ref))
            .filter_map(|s| crate::gen_hud::ARMOR.iter().find(|a| a.0 == s.id))
            .map(|a| a.2)
            .sum()
    }

    /// `Entity.doBlockCollisions` sobre `BlockPortal`: o corpo (reduzido de 0,001) toca um bloco de portal (id 90).
    fn touching_portal(&self) -> bool {
        let b = &self.player.bb;
        let (x0, x1) = (crate::mth::floor(b.min.x + 0.001), crate::mth::floor(b.max.x - 0.001));
        let (y0, y1) = (crate::mth::floor(b.min.y + 0.001), crate::mth::floor(b.max.y - 0.001));
        let (z0, z1) = (crate::mth::floor(b.min.z + 0.001), crate::mth::floor(b.max.z - 0.001));
        self.player.riding.is_none() && (x0..=x1).any(|x| (y0..=y1).any(|y| (z0..=z1).any(|z| self.world.block(x, y, z) >> 4 == 90)))
    }

    /// `setDimensionAndSpawnPlayer`/`handleRespawn`: `EntityPlayerSP` novo; a tela de morte fecha.
    /// `Minecraft.setDimensionAndSpawnPlayer`: um `EntityPlayerSP` novo (sem corrida/agachar no servidor, sem
    /// movimento, sem item em uso, sem montaria) com as capacidades do modo de jogo (`setPlayerCapabilities`).
    fn new_player(&mut self) {
        let reduced = self.player.reduced_debug;
        self.player = Player::default();
        self.player.reduced_debug = reduced;
        let gm = self.hud.gamemode;
        self.player.spectator = gm == 3;
        self.player.creative = gm == 1;
        self.player.allow_flying = gm == 1 || gm == 3;
        self.player.invulnerable = gm == 1 || gm == 3;
        self.player.flying = gm == 3;
        self.use_slot = None;
        self.digging = None;
        // O EntityPlayerSP novo traz um inventoryContainer novo: o transactionID da janela 0 volta a 0.
        self.inv.pc.tx = 0;
        self.player.reset_health();
        self.sleeping = false;
        self.menu.dead = false;
        if matches!(self.menu.screen, crate::menu::Screen::GameOver | crate::menu::Screen::GameOverConfirm) {
            self.menu.screen = crate::menu::Screen::None;
            self.set_grab(true);
        }
    }

    /// `Minecraft.runTick`: sem tela aberta e com o jogador morto, `displayGuiScreen(null)` abre o `GuiGameOver`.
    fn tick_screens(&mut self) {
        self.menu.dead = self.spawned && self.player.has_valid_health() && self.player.health <= 0.0;
        self.menu.hardcore = self.hud.hardcore;
        self.menu.score = self.hud.track.score;
        self.menu.tick();
        // GuiDownloadTerrain.updateScreen: C00PacketKeepAlive (chave 0) a cada 20 ticks.
        if self.menu.screen == crate::menu::Screen::DownloadTerrain {
            self.terrain_ticks += 1;
            if self.terrain_ticks % 20 == 0 {
                self.send(serverbound::keep_alive(0));
            }
        }
        // GuiContainer.updateScreen: morto com container aberto, closeScreen (C0D) e o GameOver abre em seguida.
        if self.menu.dead && self.inv.is_open() {
            if let Some(p) = self.inv.close() {
                self.send(p);
            }
        }
        let no_screen = self.menu.screen == crate::menu::Screen::None && !self.inv.is_open() && !self.hud.chat_open;
        if no_screen && self.menu.dead {
            self.menu.in_game = true;
            self.menu.show(crate::menu::Screen::GameOver);
            self.set_grab(false);
        }
    }

    fn tick(&mut self) {
        self.tick_screens();
        self.tick_sleep();
        let in_portal = self.touching_portal();
        let held = self.held().map(|s| {
            let name = match s.display_name() {
                Some(n) => format!("§o{n}"),
                None => self.lang.item_name(s.id, s.damage).unwrap_or_default(),
            };
            (s, name)
        });
        self.hud.held_item_tooltips = self.menu.settings.held_item_tooltips;
        if self.hud.tick(in_portal, held) {
            self.sound.play_ui("portal.trigger", 0.25, 0.8 + (self.hud.ticks % 40) as f32 / 100.0);
        }
        self.hud.armor = self.armor_value();
        self.tick_animations();
        // Minecraft.runTick: o lastLightningBolt cai antes do updateEntities, onde um raio aceso o põe em 2.
        if self.atmos.world.lightning > 0 {
            self.atmos.world.lightning -= 1;
        }
        let mut sounds = Vec::new();
        // World.getClosestPlayer ignora espectadores.
        let ents = &self.entities;
        let mut players: Vec<DVec3> = ents
            .list
            .values()
            .filter(|e| matches!(&e.kind, crate::entities::Kind::Player { uuid, .. } if ents.players.get(uuid).is_none_or(|p| p.gamemode != 3)))
            .map(|e| e.pos)
            .collect();
        if !self.player.spectator {
            players.push(self.player.pos);
        }
        self.tiles.tick(&self.world, &players, &mut sounds);
        for s in sounds {
            self.sound.play(&s.name, s.volume, s.pitch, s.pos);
        }
        if std::env::var_os("MINE_RS_DEMO_PARTICULAS").is_some() && self.hud.ticks % 100 == 20 {
            self.demo_particles_burst();
        }
        self.tick_particles();
        if self.time_of_day >= 0 {
            self.time_of_day += 1;
        }
        self.atmos.set_time(self.time_of_day);
        {
            let p = &self.player;
            self.atmos.tick(&self.world, [p.pos.x, p.pos.y, p.pos.z], self.menu.settings.render_distance);
        }
        // EntityRenderer.updateRenderer -> addRainParticles: gotas, fumaça e som da chuva.
        let feet = [self.player.pos.x, self.player.pos.y, self.player.pos.z];
        let rain = self.atmos.rain_fx(&self.world, &self.table, feet, self.menu.settings.fancy_graphics, self.menu.settings.particles);
        if !rain.is_empty() {
            self.particles.setting = self.menu.settings.particles;
            penv!(self, env, {
                for fx in &rain {
                    match *fx {
                        crate::rain::RainFx::Particle { id, pos } => {
                            self.particles.spawn(&env, id, false, glam::DVec3::from_array(pos), glam::DVec3::ZERO, &[]);
                        }
                        crate::rain::RainFx::Sound { volume, pitch, pos } => {
                            self.sound.play(crate::rain::RAIN_SOUND, volume, pitch, glam::DVec3::from_array(pos));
                        }
                    }
                }
            });
        }
        if !self.spawned {
            return;
        }
        // KeyBinding.isKeyDown: com chat/inventário aberto as teclas ficam soltas.
        let keys = if let Some(t) = &self.test {
            t.keys
        } else if self.grabbed && !self.hud.chat_open {
            // Teclas das keybinds do options.txt (GameSettings.isKeyDown).
            let k = |desc: &str| self.bind_down(desc);
            Keys {
                forward: k("key.forward"),
                back: k("key.back"),
                left: k("key.left"),
                right: k("key.right"),
                jump: k("key.jump"),
                sneak: k("key.sneak"),
                sprint: k("key.sprint"),
            }
        } else {
            Keys::default()
        };
        // Minecraft.runTick: soltar o botão direito encerra o uso do item (C07 RELEASE_USE_ITEM).
        // MINE_RS_TEST_CONQUISTA: no tick 40, com a janela já na tela (o aviso dura 3 s).
        if self.test.is_some() && self.hud.ticks == 40 {
            use crate::test_world::env;
            if let Some(v) = env("CONQUISTA") {
                let key = crate::settings::key_name(self.menu.settings.key("key.inventory"));
                let hint = || crate::lang::trf("achievement.openInventory.desc", &[&key]);
                let first: Vec<(String, i32)> = vec![("stat.leaveGame".into(), 1)];
                self.hud.achievements.statistics(&first, v == "dica", hint);
                if v != "dica" {
                    self.hud.achievements.statistics(&[(v, 1)], false, hint);
                }
            }
        }
        if let Some(t) = &self.test {
            let (swing, use_item) = (t.swing, t.use_item);
            if swing && self.hud.ticks % 10 == 0 {
                self.player.swing_item();
            }
            // Sem servidor: o fim do uso (status 9) é simulado quando a contagem acaba.
            if self.player.item_use.is_some_and(|u| u.count <= 0) {
                self.player.item_use = None;
            }
            if use_item && !self.player.is_using_item() {
                if let Some(h) = self.held() {
                    self.start_item_use(&h);
                }
            }
        }
        // Minecraft.runTick: com item em uso na hora das teclas, soltar o direito manda o C07 RELEASE e os cliques
        // desse tick são descartados (isUsingItem é lido antes do onStoppedUsingItem).
        let using_at_input = self.player.is_using_item();
        if self.test.is_none() && using_at_input && !self.right_down {
            self.player.item_use = None;
            self.use_slot = None;
            self.send(serverbound::digging(5, (0, 0, 0), 0));
        }
        self.tick_interaction(using_at_input);
        // EntityRenderer.updateRenderer roda antes do updateEntities.
        self.camera.tick(&self.player);
        self.camera.portal = (self.hud.portal.prev, self.hud.portal.time);
        self.camera.confusion = self.hud.effects.has(crate::potion::CONFUSION);
        let held = self.held();
        self.first_person.update_equipped(held.as_ref(), self.hud.selected);
        self.player.mount = self.player.riding.and_then(|id| self.entities.mount_point(id));
        // Os efeitos do próprio jogador (S1D/S1E) valem para a física: pulo, cegueira, braço.
        let amp = |id| self.hud.effects.get(id).map(|e| e.amplifier);
        self.player.jump_boost = amp(8);
        self.player.haste = amp(3);
        self.player.depth_strider = [self.inv.hotbar(self.hud.selected), self.inv.player.get(5).and_then(Option::as_ref), self.inv.player.get(6).and_then(Option::as_ref), self.inv.player.get(7).and_then(Option::as_ref), self.inv.player.get(8).and_then(Option::as_ref)]
            .iter()
            .flatten()
            .map(|s| s.enchantment_level(8))
            .max()
            .unwrap_or(0);
        self.player.mining_fatigue = amp(4);
        self.player.blindness = self.hud.effects.has(crate::potion::BLINDNESS);
        // EntityPlayer.onUpdate: o item em uso tem que continuar sendo o da mão; senão clearItemInUse (sem pacote).
        if self.player.is_using_item() {
            let now = (self.hud.selected, self.held().map(|h| (h.id, h.damage)));
            if self.use_slot.map(|(s, id, d)| (s, Some((id, d)))) != Some(now) {
                self.player.item_use = None;
                self.use_slot = None;
            }
        }
        self.player.world_border = Some(self.border.bounds());
        let out = self.player.tick(&self.world, &self.table, &keys);
        let p = &mut self.player;
        crate::physics::chase(&mut p.prev_chasing, &mut p.chasing, p.pos);
        // World.updateEntities: as outras entidades andam depois do jogador (e depois da mira e dos cliques do
        // runTick, que usam as posições do fim do tick anterior).
        self.entities.tick_in(&self.world);
        for e in self.entities.list.values_mut() {
            if let Some((thunder, explode)) = e.obj.thunder.take() {
                self.sound.play("ambient.weather.thunder", 10000.0, thunder, e.pos);
                self.sound.play("random.explode", 2.0, explode, e.pos);
            }
            if e.obj.flash && matches!(e.kind, crate::entities::Kind::Object { kind: crate::naovivas::LIGHTNING, .. }) {
                self.atmos.world.lightning = 2;
            }
        }
        self.entities.tick_players();
        self.entities.tick_guardians();
        self.entities.tick_wither_heads((self.player.pos, self.player.eye_height()));
        // WorldClient.tick: setTotalWorldTime(+1).
        self.entities.world_age += 1;
        self.entities.tick_objects(&self.world, &self.table);
        if out.abilities {
            self.send_abilities();
        }
        if out.item_finished {
            self.finish_item_use();
        }
        let own = self.entities.own_id.unwrap_or(0);
        let riding_horse = self.player.riding.is_some_and(|id| self.hud.track.is_horse_saddled(id));
        if let Some(power) = self.hud.horse_jump.tick(riding_horse, keys.jump) {
            self.send(serverbound::entity_action(own, 5, power));
        }
        for a in &out.actions {
            self.send(serverbound::entity_action(own, *a as i32, 0));
        }
        let p = &self.player;
        let (x, y, z, g) = (p.pos.x, p.bb.min.y, p.pos.z, p.on_ground);
        match out.movement {
            Some(MovePacket::PosLook) => self.send(serverbound::player_pos_look(x, y, z, p.yaw, p.pitch, g)),
            Some(MovePacket::Pos) => self.send(serverbound::player_pos(x, y, z, g)),
            Some(MovePacket::Look) => self.send(serverbound::player_look(p.yaw, p.pitch, g)),
            Some(MovePacket::Ground) => self.send(serverbound::player(g)),
            None => {}
        }
        if let Some((strafe, forward, jump, sneak)) = out.steer {
            self.send(serverbound::steer_vehicle(strafe, forward, jump, sneak));
        }
        self.sound.player_tick(&self.world, &self.player);
    }

    /// MINE_RS_DEMO_PARTICULAS=1: chão de grama, tocha, fornalha acesa, portal, mesa de encantamento
    /// e, a cada 5 s, uma amostra de cada tipo de partícula.
    fn demo_particles_world(&mut self) {
        for cx in -3..3 {
            for cz in -3..3 {
                self.world.columns.entry((cx, cz)).or_default();
            }
        }
        for x in -32..32 {
            for z in -32..32 {
                self.world.set_block(x, 62, z, 1 << 4);
                self.world.set_block(x, 63, z, 2 << 4);
            }
        }
        let put = |w: &mut World, x, y, z, st: u16| w.set_block(x, y, z, st);
        put(&mut self.world, 2, 64, 3, 50 << 4 | 5);
        put(&mut self.world, 4, 64, 3, 62 << 4 | 3);
        put(&mut self.world, 6, 64, 3, 76 << 4 | 5);
        for y in 64..67 {
            put(&mut self.world, 9, y, 3, 90 << 4 | 1);
        }
        put(&mut self.world, 12, 64, 3, 116 << 4);
        for x in 10..15 {
            put(&mut self.world, x, 64, 5, 47 << 4);
        }
        put(&mut self.world, -2, 64, 3, 110 << 4);
        self.player.pos = DVec3::new(6.5, 64.0, 11.0);
        self.player.prev = self.player.pos;
        self.player.yaw = 180.0;
        self.player.pitch = 12.0;
        self.spawned = true;
        self.menu.screen = crate::menu::Screen::None;
    }

    fn demo_particles_burst(&mut self) {
        let types = [9, 10, 13, 14, 15, 17, 23, 34, 20, 29, 30, 21, 22, 11, 12, 26, 27, 5, 4, 24, 25, 0, 32, 31];
        penv!(self, env, {
            for (i, id) in types.iter().enumerate() {
                let p = DVec3::new(-3.0 + i as f64 * 1.0, 65.0, 6.0);
                self.particles.spawn(&env, *id, true, p, DVec3::new(0.0, 0.05, 0.0), &[]);
            }
            self.particles.block_destroy(&env, (7, 64, 5), 18 << 4);
            self.particles.block_destroy(&env, (9, 64, 6), 1 << 4);
            self.particles.effect(&env, 2004, (11, 64, 7), 0);
            self.particles.effect(&env, 2001, (5, 64, 6), 17 << 12 | 3);
        });
    }

    /// `EffectRenderer.updateEffects` e as fontes de partículas do cliente (blocos, corrida, poções).
    fn tick_particles(&mut self) {
        self.particles.setting = self.menu.settings.particles;
        if let Some((n, t)) = crate::particles::Particles::test_hook() {
            if self.hud.ticks >= t {
                return;
            }
            let tick = self.hud.ticks;
            penv!(self, env, { self.particles.test_fill(&env, n, tick) });
        }
        let t = Instant::now();
        penv!(self, env, { self.particles.tick(&env) });
        self.prof.add(11, t);
        if !self.spawned {
            return;
        }
        let p = &self.player;
        let feet = (p.pos.x.floor() as i32, p.pos.y.floor() as i32, p.pos.z.floor() as i32);
        penv!(self, env, {
            // Corrida do próprio jogador e dos outros que correm (bit 3 do metadado 0).
            let water = |x: f64, y: f64, z: f64| matches!(env.world.block(x.floor() as i32, y.floor() as i32, z.floor() as i32) >> 4, 8 | 9);
            if self.player.sprinting && !water(self.player.pos.x, self.player.pos.y, self.player.pos.z) {
                self.particles.running(&env, self.player.pos, self.player.pos.y, self.player.vel, 0.6);
            }
            for (id, e) in &self.entities.list {
                if Some(*id) == self.entities.own_id {
                    continue;
                }
                let (w, h) = e.size();
                if e.is_living() {
                    self.particles.potion_swirls(&env, e.meta_int(7), e.meta_byte(8) > 0, e.invisible(), e.pos, w, h);
                }
                if e.meta_byte(0) & 8 != 0 && !water(e.pos.x, e.pos.y, e.pos.z) {
                    self.particles.running(&env, e.pos, e.pos.y, e.vel, w);
                }
            }
            self.particles.random_display_ticks(&env, feet);
        });
    }

    fn schedule_meshes(&mut self) {
        if self.world.dirty.is_empty() {
            return;
        }
        // RenderGlobal.setupTerrain ("build near"): seções a até 16 blocos da seção dos pés são refeitas na hora
        // (updateChunkNow), então o bloco colocado ou quebrado perto do jogador aparece no mesmo quadro.
        let feet = (
            (self.player.pos.x.floor() as i32) >> 4,
            (self.player.pos.y.floor() as i32) >> 4,
            (self.player.pos.z.floor() as i32) >> 4,
        );
        let near: Vec<Key> = self.world.dirty.iter().copied().filter(|k| (k.0 - feet.0).abs() <= 1 && (k.1 - feet.1).abs() <= 1 && (k.2 - feet.2).abs() <= 1).collect();
        for key in near {
            self.world.dirty.remove(&key);
            // Versão nova: um resultado em paralelo mais velho para a mesma seção é descartado.
            *self.versions.entry(key).or_insert(0) += 1;
            match self.world.snapshot(key.0, key.1, key.2) {
                None => {
                    self.vis.remove(&key);
                    if let Some(r) = &mut self.renderer {
                        r.remove(key);
                    }
                }
                Some(p) => {
                    let mesh = mesh_section(&p, [key.0 * 16, key.1 * 16, key.2 * 16], &self.table);
                    self.vis.insert(key, mesh.visibility);
                    if let Some(r) = &mut self.renderer {
                        r.upload(key, mesh);
                        self.chunk_updates += 1;
                    }
                }
            }
        }
        let pc = (
            (self.player.pos.x.floor() as i32) >> 4,
            (self.player.pos.y.floor() as i32) >> 4,
            (self.player.pos.z.floor() as i32) >> 4,
        );
        let mut keys: Vec<Key> = self.world.dirty.iter().copied().collect();
        keys.sort_by_key(|k| (k.0 - pc.0).pow(2) + (k.1 - pc.1).pow(2) + (k.2 - pc.2).pow(2));
        for key in keys.into_iter().take(MESH_BUDGET) {
            self.world.dirty.remove(&key);
            let version = {
                let v = self.versions.entry(key).or_insert(0);
                *v += 1;
                *v
            };
            match self.world.snapshot(key.0, key.1, key.2) {
                None => {
                    self.vis.remove(&key);
                    if let Some(r) = &mut self.renderer {
                        r.remove(key);
                    }
                }
                Some(p) => {
                    let (tx, table) = (self.mesh_tx.clone(), self.table.clone());
                    self.mesh_in_flight += 1;
                    rayon::spawn(move || {
                        let mesh = mesh_section(&p, [key.0 * 16, key.1 * 16, key.2 * 16], &table);
                        let _ = tx.send(MeshDone { key, version, mesh });
                    });
                }
            }
        }
    }

    fn collect_meshes(&mut self) {
        while let Ok(d) = self.mesh_rx.try_recv() {
            self.mesh_in_flight = self.mesh_in_flight.saturating_sub(1);
            if self.versions.get(&d.key) != Some(&d.version) || !self.world.columns.contains_key(&(d.key.0, d.key.2)) {
                continue;
            }
            self.vis.insert(d.key, d.mesh.visibility);
            if let Some(r) = &mut self.renderer {
                r.upload(d.key, d.mesh);
                self.chunk_updates += 1;
            }
        }
    }

    /// Rede, ticks e malhas. Roda a cada volta do loop, mesmo com a janela
    /// coberta ou minimizada: o servidor precisa receber os 20 pacotes de
    /// movimento por segundo ou expulsa por "lag".
    fn update(&mut self, el: &ActiveEventLoop) {
        self.poll_server_pack();
        self.poll_login();
        if let Some(reason) = self.pump_network() {
            println!("desconectado: {}", crate::hud::plain(&reason));
            // GuiDisconnected: "Conexão perdida" em jogo, "Falha ao conectar" antes de entrar.
            let title = if self.spawned { "disconnect.lost" } else { "connect.failed" };
            self.leave(crate::menu::Screen::Disconnected(title.to_string(), reason));
        }
        while let Ok(msg) = self.stdin_rx.try_recv() {
            self.send(serverbound::chat(&msg));
        }
        let _ = el;
        let now = Instant::now();
        self.tick_acc += (now - self.last_update).as_secs_f64();
        self.last_update = now;
        let mut ran = 0;
        while self.tick_acc >= TICK && ran < MAX_CATCH_UP {
            self.tick();
            self.tick_acc -= TICK;
            ran += 1;
        }
        if self.tick_acc >= TICK {
            self.tick_acc %= TICK;
        }
        if let Some((_, ticks, _)) = &mut self.stats {
            *ticks += ran;
        }
        self.schedule_meshes();
        self.collect_meshes();
    }

    fn print_stats(&mut self) {
        if let Some((since, ticks, frames)) = &mut self.stats {
            let t = since.elapsed().as_secs_f32();
            if t >= 5.0 {
                println!(
                    "[stats] {:.1} ticks/s, {:.0} quadros/s, coberta: {}, colunas: {}, seções por montar: {}, desenhadas: {}, jogador: {:.2} {:.2} {:.2} olhando {:.1} {:.1}",
                    *ticks as f32 / t,
                    *frames as f32 / t,
                    self.occluded,
                    self.world.columns.len(),
                    self.world.dirty.len() + self.mesh_in_flight,
                    self.renderer.as_ref().map_or(0, |r| r.drawn_sections.get()),
                    self.player.pos.x,
                    self.player.pos.y,
                    self.player.pos.z,
                    self.player.yaw,
                    self.player.pitch
                );
                // Jogadores vistos pelo cliente (testes de nametag e de rastreio).
                for e in self.entities.list.values() {
                    if let crate::entities::Kind::Player { name, .. } = &e.kind {
                        println!("[stats] jogador {name} em {:.1} {:.1} {:.1}", e.pos.x, e.pos.y, e.pos.z);
                    }
                    if std::env::var_os("MINE_RS_STATS_ENT").is_some() {
                        println!("[stats] ent {} {:?} em {:.1} {:.1} {:.1} meta {:?}", e.id, e.kind, e.pos.x, e.pos.y, e.pos.z, e.meta);
                    }
                }
                *since = Instant::now();
                *ticks = 0;
                *frames = 0;
            }
        }
    }

    /// Só para prints do céu sem servidor: MINE_RS_CEU=hora[,chuva[,trovão[,yaw[,pitch[,y]]]]].
    fn ceu_de_teste(&mut self) {
        let Ok(v) = std::env::var("MINE_RS_CEU") else { return };
        let n: Vec<f64> = v.split(',').filter_map(|s| s.trim().parse().ok()).collect();
        let get = |i: usize, d: f64| n.get(i).copied().unwrap_or(d);
        self.spawned = true;
        self.menu.screen = crate::menu::Screen::None;
        self.time_of_day = get(0, 6000.0) as i64;
        self.atmos.set_time(self.time_of_day);
        self.atmos.open_sky_for_test();
        self.atmos.world.rain = get(1, 0.0) as f32;
        self.atmos.world.thunder = get(2, 0.0) as f32;
        self.player.yaw = get(3, 0.0) as f32;
        self.player.pitch = get(4, 0.0) as f32;
        self.player.pos.y = get(5, 70.0);
        self.player.prev = self.player.pos;
    }

    fn frame(&mut self, el: &ActiveEventLoop) {
        let t_quadro = Instant::now();
        self.ceu_de_teste();
        if let Some((_, _, frames)) = &mut self.stats {
            *frames += 1;
        }
        // Brilho de encantamento animado (RenderItem.renderEffect usa Minecraft.getSystemTime()).
        let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
        for (slot, img) in self.icons.animate(now_ms) {
            let (x, y) = icon_origin(slot);
            if let Some(r) = &mut self.renderer {
                r.write_ui(x, y, img.width(), img.height(), img.as_raw());
            }
        }
        let p = &self.player;
        let alpha = (self.tick_acc / TICK).min(1.0);
        let pos = p.prev.lerp(p.pos, alpha);
        // S43: a câmera nos olhos de outra entidade (o espectador vendo um jogador).
        let watched = self.view_entity.and_then(|id| self.entities.list.get(&id)).map(|e| {
            let mut q = Player::default();
            (q.pos, q.prev, q.yaw, q.prev_yaw, q.pitch, q.prev_pitch) = (e.pos, e.prev, e.yaw, e.prev_yaw, e.pitch, e.prev_pitch);
            q.sneaking = e.sneaking();
            let eye = if matches!(e.kind, crate::entities::Kind::Player { .. }) { q.eye_height() as f32 } else { e.eye_height() };
            (q, eye)
        });
        self.camera.eye_override = watched.as_ref().map(|w| w.1);
        self.entities.hidden_view = self.view_entity.filter(|_| watched.is_some() && self.camera.third_person == 0);
        let p = watched.as_ref().map_or(p, |w| &w.0);
        let view = self.camera.view(p, &self.world, &self.table, alpha as f32);
        let (look_yaw, look_pitch) = (p.yaw, p.pitch);
        let cam = Camera {
            eye: view.eye.as_vec3(),
            yaw: look_yaw,
            pitch: look_pitch,
            fov_deg: view.fov,
            view_distance: self.menu.settings.render_distance.clamp(2, 32) as f32 * 16.0,
            view: Some(view.view),
        };
        let t = Instant::now();
        let atm = self.atmos.frame(
            &self.world,
            &crate::atmosphere::FrameIn {
                pt: alpha as f32,
                feet: [pos.x, pos.y, pos.z],
                pos: [p.pos.x, p.pos.y, p.pos.z],
                eye_height: p.eye_height(),
                yaw: p.yaw,
                pitch: p.pitch,
                render_distance: self.menu.settings.render_distance,
                fancy: self.menu.settings.fancy_graphics,
                clouds: self.menu.settings.clouds,
                gamma: self.menu.settings.gamma,
                fx: Default::default(),
            },
        );
        self.prof.add(1, t);
        let sky = atm.sky;
        self.ui.clear();
        let size = self.renderer.as_ref().map_or((1280, 720), |r| r.size());
        let gui_scale = self.gui_scale();
        let new_cfg = {
            let s = &self.menu.settings;
            crate::hud::ChatCfg {
                visibility: s.chat_visibility,
                opacity: s.chat_opacity,
                scale: s.chat_scale,
                width: s.chat_width,
                height_focused: s.chat_height_focused,
                height_unfocused: s.chat_height_unfocused,
                colors: s.chat_colors,
                links: s.chat_links,
                links_prompt: s.chat_links_prompt,
            }
        };
        if new_cfg != self.hud.chat_cfg {
            // GameSettings.setOptionFloatValue/ChatColor: refreshChat refaz a quebra das linhas.
            self.hud.chat_cfg = new_cfg;
            let open = self.hud.chat_open;
            self.hud.chat.refresh(&mut self.ui, &new_cfg, open);
        }
        let t = Instant::now();
        if self.spawned {
            let debug = if self.hud.show_debug { self.debug_info() } else { DebugInfo::default() };
            if self.hud.show_tab {
                self.update_tab();
            }
            let partial = (self.tick_acc / TICK) as f32;
            let eye = self.player.pos + DVec3::new(0.0, 1.188, 0.0);
            let l = crate::entities::light_at(&self.world, eye);
            self.hud.env = crate::hud::Env {
                third_person: self.camera.third_person as u8,
                helmet_pumpkin: self.inv.player.get(5).and_then(Option::as_ref).is_some_and(|s| s.id == 86),
                eyes_in_water: crate::camera::eyes_in_water(&self.world, &self.player),
                // light_at devolve [céu, bloco, ...]; brightness_at recebe (bloco, céu).
                brightness: crate::hud_stats::brightness_at((l[1] / 17) as i32, (l[0] / 17) as i32, self.time_of_day.abs()),
                fancy: self.menu.settings.fancy_graphics,
                border_warn: self.border.warning(self.player.pos.x, self.player.pos.z),
                riding_horse: self.player.riding.is_some_and(|id| self.hud.track.is_horse_saddled(id)),
                yaw: self.player.yaw,
                pitch: self.player.pitch,
                now_ms,
                spectator: self.player.spectator,
            };
            // EntityRenderer.updateCameraAndRender: renderGameOverlay com a GUI à mostra ou uma tela aberta.
            if self.gui_visible() {
                self.hud.draw(&mut self.ui, size.0 as f32 / gui_scale, size.1 as f32 / gui_scale, partial, &debug);
            }
            if self.sleep_gui {
                // `GuiSleepMP`: o botão `multiplayer.stopSleeping` por cima do chat.
                let (w, h) = (size.0 as f32 / gui_scale, size.1 as f32 / gui_scale);
                crate::gui_special::sleep_button(w, h).draw(&mut self.ui, self.inv.mouse.0 as i32, self.inv.mouse.1 as i32);
            }
            if self.hud.chat_open {
                let ctx = crate::chat_gui::HoverCtx {
                    advanced_tooltips: self.menu.settings.advanced_item_tooltips,
                    lang: &self.lang,
                    inventory_key: crate::settings::key_name(self.menu.settings.key("key.inventory")),
                };
                self.hud.draw_chat_hover(&mut self.ui, (size.0 as f32 / gui_scale) as i32, (size.1 as f32 / gui_scale) as i32, &ctx);
            }
        }
        self.prof.add(2, t);
        // WorldClient.setWorldTime: horário negativo é o ciclo do dia parado; o mundo usa o valor absoluto.
        self.entities.world_time = self.time_of_day.abs();
        self.entities.gui_enabled = !self.hide_gui;
        self.entities.entity_shadows = self.menu.settings.entity_shadows;
        self.entities.viewer_spectator = self.player.spectator;
        self.entities.front_view = self.camera.third_person == 2;
        let t = Instant::now();
        let mut front = 0..0;
        let mut models = match &mut self.renderer {
            Some(r) => {
                self.entities.world_time = self.time_of_day.abs();
                self.entities.draw(&cam, alpha as f32, &self.world, r, &mut self.jar, &mut self.ui, cam.view_distance, &mut self.items, &self.table, Some(crate::entities::Anchor::of_player(&self.player)))
            }
            None => Vec::new(),
        };
        self.prof.add(3, t);
        // RenderGlobal.renderWorldBorder: a parede vai com a mistura (770, 1) e o desvio de profundidade do TNT.
        if self.spawned {
            let p = self.player.prev.lerp(self.player.pos, alpha);
            let view = self.menu.settings.render_distance as f64 * 16.0;
            let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
            if let (Some(geo), Some(r)) = (crate::border::wall(&self.border, (p.x, p.z), view, ms), &mut self.renderer) {
                if let Some(tex) = self.entities.textures_mut().jar(r, &mut self.jar, "misc/forcefield") {
                    self.entities.additive.push(DrawItem { tex: Tex::Custom(tex), geo, translucent: true });
                }
            }
        }
        // Mão em primeira pessoa (renderHand) ou o próprio jogador na terceira pessoa.
        let t = Instant::now();
        let mut hand = None;
        if self.spawned && !self.player.spectator {
            let light = crate::entities::light_at(&self.world, self.player.pos + DVec3::new(0.0, self.player.eye_height(), 0.0));
            if self.camera.third_person == 0 {
                if let Some(r) = &mut self.renderer {
                    let session_uuid = self.session.as_ref().map(|s| s.uuid.clone());
                    let skin = self.entities.local_skin(r, &mut self.jar, &self.name, session_uuid.as_deref());
                    let (w, h) = r.size();
                    let far = self.menu.settings.render_distance.clamp(2, 32) as f32 * 16.0;
                    let proj = glam::Mat4::perspective_rh(view.hand_fov.to_radians(), w as f32 / h.max(1) as f32, 0.05, far * 2.0);
                    let parts = self.menu.settings.model_parts;
                    // renderHand: o item na mão some com o F1; os overlays (fogo) não.
                    let models = if self.hide_gui {
                        Vec::new()
                    } else {
                        let fishing = self.entities.list.values().any(|e| matches!(e.kind, crate::entities::Kind::Object { kind: crate::naovivas::FISH_HOOK, data } if Some(data) == self.entities.own_id));
                        // renderPlayerArm/renderPlayerArms: nada de braço com o jogador invisível (bit 5 do metadado 0).
                        let arms = skin.filter(|_| self.hud.track.flags & 0x20 == 0).map(|(t, s)| (t, s, parts));
                        self.first_person.build(&self.player, alpha as f32, view.hand_base, light, arms, fishing, &mut self.items, &mut self.jar, &self.table, r, self.entities.maps_textures())
                    };
                    // renderOverlays: no cliente o `fire` é zerado, então `isBurning` é só o bit 0 do metadado.
                    let mut overlays = Vec::new();
                    if self.hud.track.flags & 1 != 0 {
                        if let Some(geo) = crate::fx::first_person_fire(&self.table) {
                            overlays.push(mc_render::DrawItem { tex: mc_render::Tex::Blocks, geo, translucent: true });
                        }
                    }
                    hand = Some(mc_render::HandPass { proj, models, overlays });
                }
            } else {
                let start = models.len();
                models.extend(self.local_model(alpha as f32, None, light));
                front = start..models.len();
                // Render.doRenderShadowAndFire do próprio jogador (entityShadows e não invisível).
                if self.entities.entity_shadows && self.hud.track.flags & 0x20 == 0 {
                    let pos = self.player.prev.lerp(self.player.pos, alpha);
                    let d2 = (pos.as_vec3() - cam.eye).length_squared() as f64;
                    let (size, op) = crate::gen_shadows::PLAYER_SHADOW;
                    let strength = crate::fx::shadow_strength(d2, op);
                    if let Some(r) = self.renderer.as_mut().filter(|_| strength > 0.0) {
                        if let Some(tex) = self.entities.textures_mut().jar(r, &mut self.jar, "misc/shadow") {
                            let mut geo = mc_world::mesh::Geometry::default();
                            crate::fx::shadow(&mut geo, &self.world, &self.table, pos, size, strength, self.entities.world_time, light);
                            models.push(DrawItem { tex: Tex::Custom(tex), geo, translucent: true });
                        }
                    }
                }
            }
        }
        self.prof.add(4, t);
        let t = Instant::now();
        if let Some(r) = &mut self.renderer {
            let partial = (self.tick_acc / TICK) as f32;
            models.extend(self.tiles.draw(&cam, partial, &self.world, r, &mut self.jar, self.entities.textures_mut(), &mut self.ui));
        }
        self.prof.add(5, t);
        let t = Instant::now();
        self.particles.setting = self.menu.settings.particles;
        let particles = match &mut self.renderer {
            Some(r) => {
                let jar = &mut self.jar;
                let mut load = |name: &str| jar.texture(name).map(|img| r.create_texture(img.width(), img.height(), img.as_raw()));
                let pt = match crate::particles::Particles::test_hook() {
                    Some((_, t)) if self.hud.ticks >= t => 0.5,
                    _ => alpha as f32,
                };
                penv!(self, env, { self.particles.draw(&env, &cam, pt, &mut load) })
            }
            None => Default::default(),
        };
        self.prof.add(6, t);
        let t = Instant::now();
        if self.debug_boxes {
            models.push(DrawItem { tex: Tex::Ui, geo: self.debug_boxes_geo(alpha as f32), translucent: true });
        }
        models.extend(self.guardian_beams(alpha as f32));
        let lines = self.fishing_lines(alpha as f32);
        if !lines.indices.is_empty() {
            models.push(DrawItem { tex: Tex::Ui, geo: lines, translucent: false });
        }
        // isDrawBlockOutline: só com a GUI à mostra.
        // EntityRenderer.renderWorld: getMouseOver a cada quadro, com o olhar deste quadro.
        if self.spawned && self.grabbed {
            self.refresh_target();
        }
        if let Some(hit) = self.target.as_ref().filter(|_| !self.hide_gui) {
            models.push(DrawItem { tex: Tex::Ui, geo: interact::outline(&self.world, &self.table, hit, white_uv()), translucent: true });
        }
        if let Some((pos, _, progress)) = self.digging {
            let stage = (progress * 10.0) as usize;
            models.push(DrawItem { tex: Tex::Blocks, geo: interact::cracks(&self.table, pos, stage), translucent: true });
        }
        // RenderGlobal.drawBlockDamageTexture: as rachaduras das outras entidades (some depois de 400 ticks).
        let now = self.hud.ticks;
        self.break_anims.retain(|_, (_, _, t)| now.saturating_sub(*t) <= 400);
        for (pos, stage, _) in self.break_anims.values() {
            if self.world.block(pos.0, pos.1, pos.2) != 0 {
                models.push(DrawItem { tex: Tex::Blocks, geo: interact::cracks(&self.table, *pos, *stage as usize), translucent: true });
            }
        }
        // Nametags: o placar visto por eles muda junto com os pacotes de placar.
        if self.board_dirty {
            self.board_dirty = false;
            let sb = &self.hud.scoreboard;
            self.entities.board = crate::entities::NameBoard {
                player_team: sb.player_team.clone(),
                teams: sb.teams.iter().map(|(k, t)| (k.clone(), (t.prefix.clone(), t.suffix.clone(), t.visibility))).collect(),
                own_team: sb.player_team.get(&self.name).cloned(),
                below: sb.below_name.as_ref().and_then(|o| Some((sb.objectives.get(o)?.clone(), sb.scores.get(o).cloned().unwrap_or_default()))),
            };
        }
        if self.spawned {
            let (w, h) = (size.0 as f32 / gui_scale, size.1 as f32 / gui_scale);
            let mut wanted: Vec<IconKey> = (0..9).filter_map(|i| self.inv.hotbar(i).map(icon_key)).collect();
            if self.inv.is_open() {
                wanted.extend(self.inv.visible_items());
            }
            self.refresh_heads();
            for key in wanted {
                if !self.icon_map.contains_key(&key) {
                    self.ensure_icon(key);
                }
            }
            if self.hud.gamemode != 3 && self.gui_visible() {
                let cx = (w / 2.0).floor();
                for i in 0..9u8 {
                    if let Some(s) = self.inv.hotbar(i) {
                        draw_stack(&mut self.ui, s, cx - 90.0 + i as f32 * 20.0 + 2.0, h - 16.0 - 3.0, &self.icon_map);
                    }
                }
            }
            if self.inv.is_open() {
                self.inv.effects = self.hud.effects.clone();
                self.inv.draw(&mut self.ui, w, h, &self.icon_map, &self.lang);
                self.drain_inv();
            }
        }
        if self.menu.screen != crate::menu::Screen::None {
            self.update_favicons();
            let (w, h) = (size.0 as f32 / gui_scale, size.1 as f32 / gui_scale);
            self.menu.in_game = self.spawned;
            self.menu.draw(&mut self.ui, w, h);
        }
        // Minecraft.runGameLoop: updateAchievementWindow depois de tudo, com o jogador no mundo.
        if self.spawned {
            if let Some(s) = self.hud.achievements.icon() {
                let key = icon_key(&s);
                if !self.icon_map.contains_key(&key) {
                    self.ensure_icon(key);
                }
            }
            let w = (size.0 as f32 / gui_scale).ceil() as i32;
            self.hud.achievements.draw(&mut self.ui, w, &self.icon_map);
        }
        for name in std::mem::take(&mut self.menu.sounds) {
            self.sound.play_ui(name, 0.25, 1.0);
        }
        let crosshair = self.spawned && !self.inv.is_open() && self.menu.screen == crate::menu::Screen::None && !self.hide_gui;
        let crosshair = crosshair && self.camera.third_person == 0;
        let gui_models = self.gui_models(size.0 as f32 / gui_scale, size.1 as f32 / gui_scale, &cam);
        self.prof.add(7, t);
        // `setupTerrain`: a BFS a partir da seção do olho só passa por faces ligadas pelo ar.
        let t = Instant::now();
        let visible = self.renderer.as_ref().map(|r| {
            let sc = Scene { world: &self.world, vis: &self.vis, table: &self.table, cam: &cam, planes: r.cam_planes(&cam), rear_view: self.camera.third_person == 2, spectator: self.hud.gamemode == 3 };
            visible_sections(&sc)
        });
        self.prof.add(8, t);
        let frame = Frame { cam: &cam, hand: hand.as_ref(), sky: &sky, sky_draws: &atm.sky_draws, weather_draws: &atm.weather_draws, late_draws: &atm.late_draws, models: &models, additive: &self.entities.additive, overlay: &self.entities.overlay, front: front.clone(), particles: &particles, ui: &self.ui, gui_scale, crosshair, gui_models: gui_models.as_ref(), visible: visible.as_ref() };
        let t = Instant::now();
        // MINE_RS_PRINT_DIR (testes no modo fora da tela): o arquivo "pedido" com um nome vira <nome>.png lido da GPU.
        if let (Some(dir), Some(r)) = (&self.print_dir, &mut self.renderer) {
            if let Ok(nome) = std::fs::read_to_string(dir.join("pedido")) {
                let _ = std::fs::remove_file(dir.join("pedido"));
                r.pedido_print = Some(dir.join(format!("{}.png", nome.trim())));
            }
        }
        if let Some(r) = &mut self.renderer {
            if let Err(e) = r.render(&frame) {
                self.exit_reason = Some(format!("erro de vídeo: {e}"));
                el.exit();
                return;
            }
        }
        // render = só o trabalho da CPU; o tempo parado esperando a GPU vai para espera_gpu.
        let espera = self.renderer.as_ref().map_or(0, |r| r.espera_gpu.as_micros());
        self.prof.add(9, t);
        self.prof.mover(9, 10, espera);
        self.load_font_pages();

        self.prof.fim_quadro(t_quadro);
        self.hud.frames.add(self.last_frame.elapsed().as_nanos() as u64);
        self.last_frame = Instant::now();
        self.fps_frames += 1;
        let elapsed = self.fps_timer.elapsed().as_secs_f32();
        if elapsed >= 1.0 {
            let fps = (self.fps_frames as f32 / elapsed).round();
            self.fps = fps as u32;
            // Título: só os quadros por segundo e a memória do processo (VmRSS).
            if let Some(w) = &self.window {
                let ram = crate::debug_overlay::memory().0 / (1024 * 1024);
                w.set_title(&format!("{fps} FPS · {ram} MB RAM"));
            }
            self.fps_frames = 0;
            self.chunk_updates_shown = self.chunk_updates;
            self.chunk_updates = 0;
            self.fps_timer = Instant::now();
        }
    }
}


impl Game {
    /// Carrega no atlas as páginas unicode que o texto pediu.
    fn load_font_pages(&mut self) {
        let wanted: Vec<u8> = self.ui.font.wanted.iter().copied().collect();
        for page in wanted {
            let Some(slot) = self.ui.font.page_slot() else { break };
            let (x, y) = Font::page_origin(slot);
            match load_unicode_page(&mut self.jar, page) {
                Some(img) if img.width() == 256 && img.height() == 256 => {
                    if let Some(r) = &mut self.renderer {
                        r.write_ui(x, y, 256, 256, img.as_raw());
                    }
                    self.ui.font.page_loaded(page, slot);
                }
                _ => {
                    self.ui.font.wanted.remove(&page);
                }
            }
        }
    }
}

impl Game {
    fn eye(&self) -> DVec3 {
        self.player.pos + DVec3::new(0.0, self.player.eye_height(), 0.0)
    }

    fn look(&self) -> DVec3 {
        let (y, p) = ((self.player.yaw as f64).to_radians(), (self.player.pitch as f64).to_radians());
        DVec3::new(-y.sin() * p.cos(), -p.sin(), y.cos() * p.cos())
    }

    /// `EntityPlayerSP.swingItem`: anima o braço e manda o C0A.
    fn swing(&mut self) {
        self.player.swing_item();
        self.send(serverbound::swing());
    }

    /// `ItemStack.useItemRightClick` dos itens com ação (comer, beber, defender, arco).
    fn start_item_use(&mut self, held: &mc_protocol::nbt::Slot) {
        // `ItemWritableBook.onItemRightClick` -> `displayGUIBook`: o livro e pena abre o editor no cliente.
        if held.id == crate::gui_special::WRITABLE_BOOK && !self.player.spectator {
            let slot = 36 + self.hud.selected as usize;
            self.inv.open_book(held.clone(), true, self.name.clone(), slot);
            self.set_grab(false);
            return;
        }
        let Some(&(_, action, dur, always)) = crate::item_use::ITEM_USE.iter().find(|u| u.0 == held.id) else { return };
        let p = &self.player;
        let ok = match action {
            // ItemFood: `canEat` (fome ou sempre comestível, e não invulnerável).
            UseAction::Eat => (always || p.food_level < 20) && !p.invulnerable,
            // Poção de arremesso é atirada, não bebida.
            UseAction::Drink => !(held.id == 373 && held.damage & 0x4000 != 0),
            UseAction::Block => true,
            // ItemBow: criativo ou com flecha no inventário.
            UseAction::Bow => p.creative || self.inv.player.iter().flatten().any(|s| s.id == 262),
        };
        if ok {
            self.player.item_use = Some(crate::physics::ItemUse { action, max_duration: dur, count: dur });
            self.use_slot = Some((self.hud.selected, held.id, held.damage));
        }
    }

    /// `EntityPlayer.onItemUseFinish` no cliente: comida some uma da pilha, poção vira frasco (fora do criativo).
    fn finish_item_use(&mut self) {
        self.use_slot = None;
        if self.hud.gamemode == 1 {
            return;
        }
        let slot = 36 + self.hud.selected as usize;
        let Some(Some(s)) = self.inv.player.get_mut(slot) else { return };
        let Some(&(_, action, _, _)) = crate::item_use::ITEM_USE.iter().find(|u| u.0 == s.id) else { return };
        match action {
            UseAction::Eat => {
                s.count = s.count.saturating_sub(1);
                if s.count == 0 {
                    self.inv.player[slot] = None;
                }
            }
            // ItemPotion: a pilha de uma vira o frasco vazio; o leite (balde) vira balde.
            UseAction::Drink => {
                if s.count <= 1 {
                    let (id, damage) = if s.id == 335 { (325, 0) } else { (374, 0) };
                    *s = mc_protocol::nbt::Slot { id, count: 1, damage, nbt: None, raw_nbt: Vec::new() };
                } else {
                    s.count -= 1;
                }
            }
            _ => {}
        }
    }

    fn held(&self) -> Option<mc_protocol::nbt::Slot> {
        self.inv.hotbar(self.hud.selected).cloned()
    }

    /// Mira, quebra contínua e uso repetido (porte de Minecraft.clickMouse/rightClickMouse e PlayerControllerMP).
    fn tick_interaction(&mut self, using_at_input: bool) {
        // Minecraft.runTick: rightClickDelayTimer cai antes de tudo; updateController (C09) e getMouseOver(1.0F)
        // antes das teclas.
        self.right_delay = self.right_delay.saturating_sub(1);
        self.sync_current_play_item();
        self.refresh_target();
        let active = self.grabbed && !self.hud.chat_open && !self.inv.is_open();
        let using = using_at_input;
        let attacks = std::mem::take(&mut self.attack_presses);
        let uses = std::mem::take(&mut self.use_presses);
        let picks = std::mem::take(&mut self.pick_presses);
        // Com item em uso (espada defendendo, comendo, arco puxado) os cliques da fila são descartados.
        if active && !using {
            for _ in 0..attacks {
                self.click_mouse();
            }
            for _ in 0..uses {
                self.right_click_mouse();
            }
            for _ in 0..picks {
                self.middle_click_mouse();
            }
        }
        if self.right_down && active && self.right_delay == 0 && !self.player.is_using_item() {
            self.right_click_mouse();
        }
        // sendClickBlockToController(currentScreen == null && attack.isKeyDown() && inGameHasFocus).
        self.send_click_block_to_controller(self.left_down && active);
    }

    /// `Minecraft.middleClickMouse` + `ForgeHooks.onPickBlock`: o item do bloco mirado (`getPickBlock`, tabela
    /// gerada do Java). Na hotbar, troca para ele; no criativo, sem ele na hotbar, vai para o primeiro slot vazio
    /// da hotbar (senão o da mão) e sai o C10. Mirar entidade (só no criativo) não foi portado.
    fn middle_click_mouse(&mut self) {
        let Some(hit) = self.target.filter(|_| self.target_entity.is_none()) else { return };
        let state = self.world.block(hit.pos.0, hit.pos.1, hit.pos.2);
        if state == 0 {
            return;
        }
        let pick = &crate::gen_pick_block::PICK_BLOCK;
        let Ok(k) = pick.binary_search_by_key(&state, |r| r.0) else { return };
        let (_, item, damage) = pick[k];
        // isItemEqual + areItemStackTagsEqual: mesmo item e dano, sem NBT.
        let same = |s: &Option<mc_protocol::nbt::Slot>| s.as_ref().is_some_and(|s| s.id == item && s.damage == damage && s.nbt.is_none());
        if let Some(x) = (0..9).find(|x| same(&self.inv.player[36 + x])) {
            self.hud.selected = x as u8;
            return;
        }
        if self.hud.gamemode != 1 {
            return;
        }
        // InventoryPlayer.getFirstEmptyStack: hotbar (36-44) e depois o resto (9-35); fora da hotbar, a mão.
        let empty = (36..45).chain(9..36).find(|&i| self.inv.player[i].is_none());
        let slot = match empty {
            Some(i) if i >= 36 => (i - 36) as u8,
            _ => self.hud.selected,
        };
        let stack = mc_protocol::nbt::Slot { id: item, count: 1, damage, nbt: None, raw_nbt: Vec::new() };
        self.inv.set_slot(0, 36 + slot as i16, Some(stack.clone()));
        self.hud.selected = slot;
        self.send(crate::container_net::creative_slot(36 + slot as i16, Some(&stack)));
    }

    /// `PlayerControllerMP.syncCurrentPlayItem`: o C09 só sai quando o slot mudou (no tick e antes das ações).
    fn sync_current_play_item(&mut self) {
        if self.hud.selected != self.current_player_item {
            self.current_player_item = self.hud.selected;
            self.send(serverbound::held_item_change(self.current_player_item as u16));
        }
    }

    /// `Minecraft.sendClickBlockToController`: soltar o botão zera o `leftClickCounter`.
    fn send_click_block_to_controller(&mut self, down: bool) {
        if !down {
            self.left_click_counter = 0;
        }
        if self.left_click_counter > 0 || self.player.is_using_item() {
            return;
        }
        match self.target.filter(|_| down && self.target_entity.is_none()) {
            Some(hit) => {
                if self.world.block(hit.pos.0, hit.pos.1, hit.pos.2) != 0 && self.on_player_damage_block(hit.pos, hit.face) {
                    self.particles.setting = self.menu.settings.particles;
                    penv!(self, env, { self.particles.block_hit(&env, hit.pos, hit.face as usize) });
                    self.swing();
                }
            }
            None => self.reset_block_removing(),
        }
    }

    /// `EntityPlayer.getBreakSpeed` / `ForgeHooks.blockStrength` do bloco para o jogador agora.
    fn relative_hardness(&self, state: u16) -> f32 {
        let held = self.held();
        let helmet_aqua = self.inv.player.get(5).and_then(|s| s.as_ref()).is_some_and(|h| h.enchantment_level(6) > 0);
        let in_water = crate::camera::eyes_in_water(&self.world, &self.player) && !helmet_aqua;
        let amp = |id| self.hud.effects.get(id).map(|e| e.amplifier);
        let mods = interact::DigMods { efficiency: held.as_ref().map_or(0, |h| h.enchantment_level(32)), haste: amp(3), fatigue: amp(4) };
        interact::break_speed(state >> 4, held.map(|h| h.id), self.player.on_ground, in_water, mods)
    }

    /// `PlayerControllerMP.isHittingPosition`: mesmo bloco e mesmo item (dano só conta em item sem durabilidade).
    fn is_hitting_position(&self, pos: (i32, i32, i32)) -> bool {
        let held = self.held();
        let same = match (&self.dig_item, &held) {
            (None, None) => true,
            (Some(a), Some(b)) => a.id == b.id && a.raw_nbt == b.raw_nbt && (damageable(b.id) || a.damage == b.damage),
            _ => false,
        };
        pos == self.current_block && same
    }

    /// `PlayerControllerMP.clickBlock`.
    fn click_block(&mut self, pos: (i32, i32, i32), face: u8) -> bool {
        // PlayerControllerMP.clickBlock: aventura (sem isAllowEdit) só quebra o que o CanDestroy do item lista;
        // fora da borda do mundo não quebra.
        if self.hud.gamemode == 3 {
            return false;
        }
        if self.hud.gamemode == 2 {
            let block = self.world.block(pos.0, pos.1, pos.2) >> 4;
            if !self.held().is_some_and(|h| crate::containers::tag_allows(&h, "CanDestroy", block)) {
                return false;
            }
        }
        if !self.border.contains(pos.0, pos.2) {
            return false;
        }
        match self.hud.gamemode {
            1 => {
                self.send(serverbound::digging(0, pos, face));
                self.click_block_creative(pos, face);
                self.dig_delay = 5;
                return true;
            }
            _ => {}
        }
        if self.digging.is_none() || !self.is_hitting_position(pos) {
            if self.digging.is_some() {
                self.send(serverbound::digging(1, self.current_block, face));
            }
            self.send(serverbound::digging(0, pos, face));
            let state = self.world.block(pos.0, pos.1, pos.2);
            if state != 0 && self.relative_hardness(state) >= 1.0 {
                self.destroy_block(pos, state);
            } else {
                self.digging = Some((pos, face, 0.0));
                self.current_block = pos;
                self.dig_item = self.held();
                self.dig_step = 0.0;
            }
        }
        true
    }

    /// `PlayerControllerMP.clickBlockCreative`: fogo na face clicada apaga (`World.extinguishFire`); senão quebra.
    fn click_block_creative(&mut self, pos: (i32, i32, i32), face: u8) {
        let d = [(0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1), (-1, 0, 0), (1, 0, 0)][face.min(5) as usize];
        let f = (pos.0 + d.0, pos.1 + d.1, pos.2 + d.2);
        if self.world.block(f.0, f.1, f.2) >> 4 == 51 {
            self.world.set_block(f.0, f.1, f.2, 0);
            self.tiles.block_changed(f.0, f.1, f.2, 0);
            if let Some(s) = crate::sound::effect_sound(1004, f, 0, &mut rand::thread_rng()) {
                self.sound.play(&s.name, s.volume, s.pitch, s.pos);
            }
            return;
        }
        let state = self.world.block(pos.0, pos.1, pos.2);
        self.destroy_block(pos, state);
    }

    /// `PlayerControllerMP.resetBlockRemoving`: ABORT com a face de baixo.
    fn reset_block_removing(&mut self) {
        if self.digging.take().is_some() {
            self.send(serverbound::digging(1, self.current_block, 0));
        }
    }

    /// `PlayerControllerMP.onPlayerDamageBlock`: um passo da quebra por tick com o botão segurado.
    fn on_player_damage_block(&mut self, pos: (i32, i32, i32), face: u8) -> bool {
        self.sync_current_play_item();
        if self.dig_delay > 0 {
            self.dig_delay -= 1;
            return true;
        }
        if self.hud.gamemode == 1 {
            self.dig_delay = 5;
            self.send(serverbound::digging(0, pos, face));
            self.click_block_creative(pos, face);
            return true;
        }
        if !self.is_hitting_position(pos) {
            return self.click_block(pos, face);
        }
        let state = self.world.block(pos.0, pos.1, pos.2);
        if state == 0 {
            self.digging = None;
            return false;
        }
        let dmg = self.relative_hardness(state);
        let total = {
            let d = self.digging.get_or_insert((pos, face, 0.0));
            d.2 += dmg;
            d.2
        };
        if self.dig_step % 4.0 == 0.0 {
            if let Some(t) = crate::sound::block_sound(state >> 4) {
                let at = DVec3::new(pos.0 as f64 + 0.5, pos.1 as f64 + 0.5, pos.2 as f64 + 0.5);
                self.sound.play(&t.step_sound(), (t.volume() + 1.0) / 8.0, t.frequency() * 0.5, at);
            }
        }
        self.dig_step += 1.0;
        if total >= 1.0 {
            self.digging = None;
            self.send(serverbound::digging(2, pos, face));
            self.destroy_block(pos, state);
            self.dig_step = 0.0;
            self.dig_delay = 5;
        }
        true
    }

    /// `PlayerControllerMP.getIsHittingBlock`: quebrando um bloco no modo sobrevivência.
    fn is_hitting_block(&self) -> bool {
        self.digging.is_some()
    }

    /// `Minecraft.clickMouse`: bater no ar (ou num bloco de ar) fora do criativo trava 10 ticks.
    fn click_mouse(&mut self) {
        if self.left_click_counter > 0 {
            return;
        }
        self.swing();
        let not_creative = self.hud.gamemode != 1;
        if let Some((id, _)) = self.target_entity {
            self.attack_entity(id);
        } else if let Some(hit) = self.target.filter(|h| self.world.block(h.pos.0, h.pos.1, h.pos.2) != 0) {
            self.click_block(hit.pos, hit.face);
        } else if not_creative {
            self.left_click_counter = 10;
        }
    }

    /// `Minecraft.rightClickMouse`: nada enquanto bate num bloco (`getIsHittingBlock`).
    fn right_click_mouse(&mut self) {
        if !self.is_hitting_block() {
            self.use_item();
        }
    }

    /// `PlayerControllerMP.attackEntity`: o C02 e, fora do espectador, o lado do cliente de
    /// `EntityPlayer.attackTargetEntityWithCurrentItem`.
    fn attack_entity(&mut self, id: i32) {
        use crate::entities::Kind;
        self.sync_current_play_item();
        self.send(serverbound::use_entity(id, 1, None));
        if self.hud.gamemode == 3 {
            return;
        }
        let Some(e) = self.entities.list.get(&id) else { return };
        let held = self.held();
        let level = |ench: i32| held.as_ref().map_or(0, |h| h.enchantment_level(ench));
        // EnchantmentHelper.getModifierForCreature: afiação sempre, julgamento em mortos-vivos, ruína em artrópodes.
        let (undead, arthropod) = match e.kind {
            Kind::Mob(51 | 54 | 57 | 64) => (true, false),
            Kind::Mob(52 | 59 | 60 | 67) => (false, true),
            _ => (false, false),
        };
        let f1 = level(16) as f32 * 1.25 + if undead { level(17) as f32 * 2.5 } else { 0.0 } + if arthropod { level(18) as f32 * 2.5 } else { 0.0 };
        // getKnockbackModifier + 1 correndo.
        let i = level(19) + self.player.sprinting as i32;
        let living = matches!(e.kind, Kind::Player { .. } | Kind::Mob(_));
        let p = &self.player;
        let crit = p.fall_distance > 0.0 && !p.on_ground && !p.on_ladder(&self.world) && !p.in_water && !p.blindness && p.riding.is_none() && living;
        // attackEntityFrom no mundo do cliente: EntityOtherPlayerMP, barco, carrinho, cristal, bola de fogo e os
        // pendurados devolvem true; os vivos (EntityLivingBase com isRemote) devolvem false.
        use crate::naovivas::{BOAT, CRYSTAL, ITEM_FRAME, LARGE_FIREBALL, LEASH_KNOT, MINECART, PAINTING};
        let hit = match e.kind {
            Kind::Player { .. } => true,
            Kind::Object { kind, .. } => matches!(kind, BOAT | MINECART | CRYSTAL | LARGE_FIREBALL | ITEM_FRAME | LEASH_KNOT | PAINTING),
            Kind::Mob(_) => false,
        };
        if !hit {
            return;
        }
        if i > 0 {
            self.player.vel.x *= 0.6;
            self.player.vel.z *= 0.6;
            self.player.set_sprinting(false);
        }
        // onCriticalHit / onEnchantmentCritical: as partículas no alvo, como o S0B 4 e 5.
        self.particles.setting = self.menu.settings.particles;
        if crit {
            penv!(self, env, { self.particles.emit_at_entity(&env, id, 9) });
        }
        if f1 > 0.0 {
            penv!(self, env, { self.particles.emit_at_entity(&env, id, 10) });
        }
    }

    /// `PlayerControllerMP.onPlayerDestroyBlock`: o bloco some no cliente na hora, com as partículas e o som
    /// do `playAuxSFX(2001)` (o servidor manda o 2001 para os outros, não para quem quebrou).
    fn destroy_block(&mut self, pos: (i32, i32, i32), state: u16) {
        // Criativo com espada não quebra.
        if self.hud.gamemode == 1 && self.held().is_some_and(|h| matches!(h.id, 267 | 268 | 272 | 276 | 283)) {
            return;
        }
        if state == 0 {
            return;
        }
        self.particles.setting = self.menu.settings.particles;
        penv!(self, env, { self.particles.block_destroy(&env, pos, state) });
        if let Some(t) = crate::sound::block_sound(state >> 4) {
            let at = DVec3::new(pos.0 as f64 + 0.5, pos.1 as f64 + 0.5, pos.2 as f64 + 0.5);
            self.sound.play(&t.break_sound(), (t.volume() + 1.0) / 2.0, t.frequency() * 0.8, at);
        }
        self.world.set_block(pos.0, pos.1, pos.2, 0);
        self.tiles.block_changed(pos.0, pos.1, pos.2, 0);
        self.current_block.1 = -1;
    }

    /// `ItemBlock.canPlaceBlockOnSide` + o destino do `onItemUse`, só para os blocos do caminho genérico
    /// (`getMetadata` + `getStateFromMeta`, sem `onBlockPlaced` próprio): `None` = bloco sem previsão (o servidor
    /// decide), `Some(None)` = não cabe, `Some(Some((pos, estado)))` = cabe ali.
    #[allow(clippy::type_complexity)]
    fn item_block_target(&self, hit: interact::Hit, held: &mc_protocol::nbt::Slot) -> Option<Option<((i32, i32, i32), u16)>> {
        if held.count == 0 || !(1..256).contains(&held.id) {
            return None;
        }
        let id = held.id as u16;
        let meta = place_meta(id, held.damage as u16 & 15, hit.face)?;
        // Block.isReplaceable do bloco clicado; senão vai para o lado da face.
        let clicked = self.world.block(hit.pos.0, hit.pos.1, hit.pos.2);
        if clicked >> 4 == 175 {
            return None;
        }
        let pos = if replaceable_clicked(clicked) {
            hit.pos
        } else {
            let d = [(0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1), (-1, 0, 0), (1, 0, 0)][hit.face.min(5) as usize];
            (hit.pos.0 + d.0, hit.pos.1 + d.1, hit.pos.2 + d.2)
        };
        // World.setBlockState fora de 0..256 falha; o servidor também recusa.
        if !(0..255).contains(&pos.1) {
            return Some(None);
        }
        // World.canBlockBePlaced: material substituível e nenhuma entidade (preventEntitySpawning) na caixa.
        if !replaceable_material(self.world.block(pos.0, pos.1, pos.2) >> 4) {
            return Some(None);
        }
        let (lo, hi) = (DVec3::new(pos.0 as f64, pos.1 as f64, pos.2 as f64), DVec3::new(pos.0 as f64 + 1.0, pos.1 as f64 + 1.0, pos.2 as f64 + 1.0));
        let hits = |min: DVec3, max: DVec3| max.x > lo.x && min.x < hi.x && max.y > lo.y && min.y < hi.y && max.z > lo.z && min.z < hi.z;
        if hits(self.player.bb.min, self.player.bb.max) {
            return Some(None);
        }
        for (eid, e) in &self.entities.list {
            if Some(*eid) == self.entities.own_id || e.dead || !prevents_spawning(&e.kind) {
                continue;
            }
            let (w, h) = e.size();
            let half = w as f64 / 2.0;
            if hits(e.pos - DVec3::new(half, 0.0, half), e.pos + DVec3::new(half, h as f64, half)) {
                return Some(None);
            }
        }
        Some(Some((pos, id << 4 | meta)))
    }

    /// O resto do `ItemBlock.onItemUse` no mundo do cliente: o bloco aparece na hora, o som de colocar toca e a
    /// pilha diminui (fora do criativo); o servidor confirma ou desfaz depois.
    fn apply_place(&mut self, pos: (i32, i32, i32), state: u16) {
        self.world.set_block(pos.0, pos.1, pos.2, state);
        self.tiles.block_changed(pos.0, pos.1, pos.2, state);
        if let Some(t) = crate::sound::block_sound(state >> 4) {
            let at = DVec3::new(pos.0 as f64 + 0.5, pos.1 as f64 + 0.5, pos.2 as f64 + 0.5);
            self.sound.play(&t.place_sound(), (t.volume() + 1.0) / 2.0, t.frequency() * 0.8, at);
        }
        if self.hud.gamemode != 1 {
            let slot = 36 + self.hud.selected as usize;
            if let Some(Some(s)) = self.inv.player.get_mut(slot) {
                s.count = s.count.saturating_sub(1);
                if s.count == 0 {
                    self.inv.player[slot] = None;
                }
            }
        }
    }

    /// `EntityRenderer.getMouseOver`: o bloco pelo raio e, por cima dele, a entidade colidível mais perto.
    /// Roda no começo do tick, a cada quadro e no clique, sempre com a direção atual do olhar.
    fn refresh_target(&mut self) {
        let creative = self.hud.gamemode == 1;
        let reach = if creative { 5.0 } else { 4.5 };
        let (eye, dir) = (self.eye(), self.look());
        self.target = if self.hud.gamemode == 3 { None } else { interact::raycast(&self.world, &self.table, eye, dir, reach) };
        // extendedReach (criativo): 6 blocos; senão o alcance do bloco, e entidade a mais de 3 vira erro.
        let d0 = if creative { 6.0 } else { reach };
        let far_miss = !creative && d0 > 3.0;
        let d1 = self.target.map_or(d0, |h| (h.point - eye).length());
        let mut pointed: Option<(i32, DVec3, DVec3)> = None; // id, ponto relativo à entidade, ponto no mundo
        let mut d2 = d1;
        for (id, e) in &self.entities.list {
            if Some(*id) == self.entities.own_id || !e.can_be_collided(&self.entities.players) {
                continue;
            }
            // getCollisionBorderSize: 0,1 para todos os lados.
            let (w, h) = e.size();
            let half = w as f64 / 2.0 + 0.1;
            let min = e.pos - DVec3::new(half, 0.1, half);
            let max = e.pos + DVec3::new(half, h as f64 + 0.1, half);
            let inside = eye.cmpge(min).all() && eye.cmple(max).all();
            if inside {
                if d2 >= 0.0 {
                    pointed = Some((*id, eye - e.pos, eye));
                    d2 = 0.0;
                }
            } else if let Some((d3, _)) = interact::ray_box(eye, dir, min, max).filter(|(t, _)| *t <= d0) {
                if d3 < d2 || d2 == 0.0 {
                    let hit = eye + dir * d3;
                    let at = hit - e.pos;
                    // Montaria do próprio jogador (canRiderInteract falso): só quando nada mais foi pego.
                    if self.player.riding == Some(*id) {
                        if d2 == 0.0 {
                            pointed = Some((*id, at, hit));
                        }
                    } else {
                        pointed = Some((*id, at, hit));
                        d2 = d3;
                    }
                }
            }
        }
        self.target_entity = None;
        if let Some((id, at, hit)) = pointed {
            if far_miss && (hit - eye).length() > 3.0 {
                // MovingObjectType.MISS: nem a entidade nem o bloco atrás.
                self.target = None;
            } else if d2 < d1 || self.target.is_none() {
                self.target_entity = Some((id, at));
                self.target = None;
            }
        }
    }

    /// `Minecraft.rightClickMouse` (quem chama confere o `getIsHittingBlock`).
    fn use_item(&mut self) {
        self.right_delay = 4;
        let held = self.held();
        if self.stats.is_some() {
            println!("[debug] usar item {:?} mirando {:?} entidade {:?}", held.as_ref().map(|h| h.id), self.target.map(|t| t.pos), self.target_entity.map(|t| t.0));
        }
        let mut flag = true;
        if let Some((id, at)) = self.target_entity {
            if self.right_click_entity(id, at, held.as_ref()) {
                flag = false;
            }
        } else if let Some(hit) = self.target {
            if self.world.block(hit.pos.0, hit.pos.1, hit.pos.2) != 0 {
                if self.on_player_right_click(hit, held.as_ref()) {
                    flag = false;
                    self.swing();
                }
                if held.is_none() {
                    return;
                }
            }
        }
        // Nem a entidade nem o bloco "pegaram" o clique: usa o item no ar (espada, comida, arco, balde...).
        if flag {
            if let Some(h) = &held {
                self.send_use_item(h);
            }
        }
    }

    /// `PlayerControllerMP.sendUseItem`: C08 sem posição e o `useItemRightClick`.
    fn send_use_item(&mut self, held: &mc_protocol::nbt::Slot) {
        if self.hud.gamemode == 3 {
            return;
        }
        self.sync_current_play_item();
        self.send(serverbound::block_placement(None, 255, Some(held), [0; 3]));
        self.start_item_use(held);
    }

    /// `isPlayerRightClickingOnEntity` + `interactWithEntitySendPacket`: true se a entidade ficou com o clique.
    fn right_click_entity(&mut self, id: i32, at: DVec3, held: Option<&mc_protocol::nbt::Slot>) -> bool {
        use crate::entities::Kind;
        use crate::naovivas::{BOAT, ITEM_FRAME, LEASH_KNOT, MINECART};
        self.sync_current_play_item();
        self.send(serverbound::use_entity(id, 2, Some([at.x as f32, at.y as f32, at.z as f32])));
        let spectator = self.hud.gamemode == 3;
        let Some(e) = self.entities.list.get(&id) else { return false };
        // EntityArmorStand.interactAt: true no cliente (menos o marcador).
        if !spectator && matches!(e.kind, Kind::Object { kind: crate::entities::ARMOR_STAND, .. }) && e.meta_byte(10) & 0x10 == 0 {
            return true;
        }
        // EntityPlayer.interactWith: interactFirst da entidade ou interactWithEntity do item (só os comuns).
        let sneaking = self.player.sneaking;
        let held_id = held.map_or(0, |h| h.id);
        let first = match e.kind {
            // EntityVillager: vivo, adulto, sem troca em andamento, sem agachar e sem ovo na mão.
            Kind::Mob(120) => !e.dead && !e.child() && !sneaking && held_id != 383,
            // Cavalo e porco selado montam; carrinho, barco e os pendurados ficam com o clique.
            Kind::Mob(100) => true,
            Kind::Mob(90) => e.meta_byte(16) & 1 != 0,
            Kind::Object { kind, .. } => matches!(kind, BOAT | MINECART | ITEM_FRAME | LEASH_KNOT),
            _ => false,
        };
        let item = matches!(e.kind, Kind::Mob(_))
            && match held_id {
                // Etiqueta com nome, sela no porco, tesoura e tinta na ovelha.
                421 => held.and_then(|h| h.display_name()).is_some(),
                329 => matches!(e.kind, Kind::Mob(90)),
                359 => matches!(e.kind, Kind::Mob(91 | 96)),
                351 => matches!(e.kind, Kind::Mob(91)),
                _ => false,
            };
        self.sync_current_play_item();
        self.send(serverbound::use_entity(id, 0, None));
        !spectator && (first || item)
    }

    /// `PlayerControllerMP.onPlayerRightClick`: o bloco reage ou o item é usado nele (C08 com posição).
    fn on_player_right_click(&mut self, hit: interact::Hit, held: Option<&mc_protocol::nbt::Slot>) -> bool {
        self.sync_current_play_item();
        // Fora da borda do mundo não usa o bloco nem coloca.
        if !self.border.contains(hit.pos.0, hit.pos.2) {
            return false;
        }
        let block = self.world.block(hit.pos.0, hit.pos.1, hit.pos.2) >> 4;
        let mut flag = false;
        let mut place = None;
        if self.hud.gamemode != 3 {
            if !self.player.sneaking || held.is_none() {
                flag = interactive_block(block);
            }
            if !flag {
                if let Some(h) = held.filter(|h| (1..256).contains(&h.id)) {
                    match self.item_block_target(hit, h) {
                        // canPlaceBlockOnSide falso: sem C08 posicionado (o rightClickMouse manda o do ar).
                        Some(None) => return false,
                        Some(Some(t)) => place = Some(t),
                        None => {}
                    }
                }
            }
        }
        // C08 com a posição do clique na face: (int)(hitX * 16), de 0 a 16.
        let rel = hit.point - DVec3::new(hit.pos.0 as f64, hit.pos.1 as f64, hit.pos.2 as f64);
        let cursor = [rel.x, rel.y, rel.z].map(|v| ((v as f32 * 16.0) as i32).clamp(0, 16) as u8);
        self.send(serverbound::block_placement(Some(hit.pos), hit.face, held, cursor));
        if !flag && self.hud.gamemode != 3 {
            let Some(h) = held else { return false };
            flag = if h.id < 256 {
                // ItemBlock.onItemUse: canPlayerEdit (aventura: CanPlaceOn com o bloco de apoio); colocou (previsto
                // aqui) ou é um bloco sem previsão, que o servidor decide.
                const STEP: [(i32, i32, i32); 6] = [(0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1), (-1, 0, 0), (1, 0, 0)];
                let editable = |g: &Self, pos: (i32, i32, i32)| {
                    let d = STEP[hit.face as usize % 6];
                    let under = g.world.block(pos.0 - d.0, pos.1 - d.1, pos.2 - d.2) >> 4;
                    g.hud.gamemode != 2 || crate::containers::tag_allows(h, "CanPlaceOn", under)
                };
                match place {
                    Some((pos, state)) if editable(self, pos) => {
                        self.apply_place(pos, state);
                        true
                    }
                    Some(_) => false,
                    None => true,
                }
            } else {
                item_uses_on_block(h.id, block, hit.face)
            };
        }
        flag
    }

    /// Garante o ícone de (id, dano) no atlas da GUI.
    fn ensure_icon(&mut self, key: IconKey) {
        let mut variant = key.2;
        if let Some(item) = crate::tile_entities::entity_from_variant(key.2) {
            // Só é `builtin/entity` se o modelo do item (o do pacote de recursos, se houver) for: um pacote pode
            // trocar as cabeças por um ícone plano (`builtin/generated`), e aí vale o modelo normal.
            if self.icons.entity_display(key.0, key.1, &mut self.jar).is_some() {
                self.ensure_entity_icon(key, item);
                return;
            }
            variant &= mc_render::items::ENCHANTED;
        }
        let (slot, img) = self.icons.get(key.0, key.1, variant, &mut self.jar);
        if let (Some(s), Some(img)) = (slot, img) {
            let (x, y) = icon_origin(s);
            if let Some(r) = &mut self.renderer {
                r.write_ui(x, y, img.width(), img.height(), img.as_raw());
            }
        }
        if let Some(s) = slot {
            self.icon_map.insert(key, s);
        }
    }

    /// Ícone de item `builtin/entity` (`TileEntityItemStackRenderer`): a geometria do tile entity vista
    /// pelo `display.gui` do modelo do item, rasterizada na CPU. Devolve se a textura já é a definitiva.
    fn ensure_entity_icon(&mut self, key: IconKey, item: crate::tile_entities::EntityItem) -> bool {
        use crate::models::{Sink, FULL_LIGHT};
        let Some(gui) = self.icons.entity_display(key.0, key.1, &mut self.jar) else { return true };
        let Some((tex, done)) = crate::tile_entities::item_image(&item, self.entities.textures_mut(), &mut self.jar) else { return true };
        let lights = mc_render::items::gui_model_lights(gui).map(glam::Vec3::from);
        let mut geo = mc_world::mesh::Geometry::default();
        let mut sink = Sink { geo: &mut geo, tex: (64.0, 64.0), tint: [1.0; 3], alpha: 255, light: FULL_LIGHT, lights };
        // RenderItem.renderItem (isBuiltInRenderer): o giro de 180° em Y antes do renderByItem, como na mão e no chão.
        crate::tile_entities::emit_item(&item, &mut sink, &crate::first_person::builtin_matrix(glam::Mat4::IDENTITY));
        let img = mc_render::items::render_entity_icon(&geo, &tex, gui);
        if let Some(s) = self.icons.put(key.0, key.1, key.2) {
            let (x, y) = icon_origin(s);
            if let Some(r) = &mut self.renderer {
                r.write_ui(x, y, img.width(), img.height(), img.as_raw());
            }
            self.icon_map.insert(key, s);
        }
        if !done && !self.heads_pending.iter().any(|(k, _)| *k == key) {
            self.heads_pending.push((key, item));
        }
        done
    }

    /// `MINE_RS_TEST_TAB=n[:hearts|:integer]`: o próprio jogador e n-1 de mentira (um time com
    /// prefixo, um espectador, pings variados) e, com o sufixo, um objetivo no espaço 0.
    fn test_tab(&mut self, v: &str) {
        use crate::entities::PlayerInfo;
        use crate::entity_net::Uuid;
        let (n, kind) = v.split_once(':').map_or((v, None), |(a, b)| (a, Some(b)));
        let n: u128 = n.parse().unwrap_or(1);
        let sb = &mut self.hud.scoreboard;
        sb.teams.insert("a_vip".into(), crate::hud::Team { prefix: "§6[VIP] ".into(), suffix: String::new(), visibility: 0, color: None });
        for i in 0..n {
            let name = if i == 0 { self.name.clone() } else { format!("Jogador{i}") };
            if i % 3 == 1 {
                sb.player_team.insert(name.clone(), "a_vip".into());
            }
            let gamemode = if i == 2 { 3 } else { 0 };
            let ping = [40, 200, 450, 800, 1500, -1][i as usize % 6];
            if let Some(k) = kind {
                sb.scores.entry("lista".into()).or_default().insert(name.clone(), (i as i32 * 7) % 21);
                if k == "hearts" {
                    sb.hearts.insert("lista".into());
                }
            }
            self.entities.players.insert(Uuid(i * 0x1_0000_0001 + 1), PlayerInfo { name, display_name: None, ping, gamemode, skin: None, properties: Vec::new() });
        }
        if kind.is_some() {
            sb.objectives.insert("lista".into(), "Lista".into());
            sb.list = Some("lista".into());
        }
        self.hud.tab_header = "§bMundo de teste\n§7lista do tab".into();
        self.hud.tab_footer = "§erodapé".into();
        self.hud.show_tab = true;
    }

    /// O próprio jogador como o `RenderPlayer` desenha: no mundo (terceira pessoa) ou numa tela (`gui`),
    /// com o item da mão (`getHeldItem`), a armadura e as partes da skin das opções.
    fn local_model(&mut self, partial: f32, gui: Option<&crate::entities::GuiView>, light: [u8; 4]) -> Vec<DrawItem> {
        let use_ticks = self.player.item_use.map(|u| u.max_duration - u.count);
        let fishing = self.own_fishing();
        let Some(r) = &mut self.renderer else { return Vec::new() };
        let session_uuid = self.session.as_ref().map(|s| s.uuid.clone());
        let Some(skin) = self.entities.local_skin(r, &mut self.jar, &self.name, session_uuid.as_deref()) else { return Vec::new() };
        let worn = |s: Option<&mc_protocol::nbt::Slot>| {
            s.map(|s| crate::entities::Worn {
                id: s.id,
                damage: s.damage,
                model: None,
                color: s.nbt.as_ref().and_then(|n| n.get("display")).and_then(|d| d.get("color")).and_then(|c| match c {
                    mc_protocol::nbt::Nbt::Int(c) => Some(*c as u32 & 0xFFFFFF),
                    _ => None,
                }),
                entity: crate::tile_entities::entity_item(s).map(crate::tile_entities::entity_variant),
            })
        };
        // Inventário do jogador: 5 capacete, 6 peito, 7 calça, 8 botas.
        let slot = |i: usize| self.inv.player.get(i).and_then(Option::as_ref);
        let held = worn(self.inv.hotbar(self.hud.selected)).map(|w| crate::entities::Worn { model: crate::first_person::entity_override(w.id, fishing, use_ticks), ..w });
        let armor = [worn(slot(8)), worn(slot(7)), worn(slot(6)), worn(slot(5))];
        let parts = self.menu.settings.model_parts;
        // LayerCape: a parte CAPE das opções e o jogador visível.
        let cape = if parts & 1 != 0 && self.hud.track.flags & 0x20 == 0 { self.entities.local_cape(&self.name, session_uuid.as_deref()) } else { None };
        self.entities.draw_local(&self.player, partial, gui, skin, parts, held, armor, light, self.hud.ticks as u32, cape, &mut self.items, &self.table, r, &mut self.jar)
    }

    /// Atende o pedido de modelo 3D da GUI (`UiBatch::gui3d`) com a projeção da tela
    /// (`GuiScreen`: `ortho(0, w, h, 0, 1000, 3000)` e `translate(0, 0, -2000)`).
    fn gui_models(&mut self, w: f32, h: f32, cam: &mc_render::Camera) -> Option<mc_render::GuiModels> {
        use mc_render::ui::Gui3d;
        match self.ui.gui3d? {
            Gui3d::Entity { at, x, y, scale, mx, my, entity } => {
                let view = crate::entities::GuiView::new(x as f32, y as f32, scale as f32, mx, my);
                // O lightmap fica no máximo (o setLightmapTextureCoords(240, 240) do GuiContainer).
                let models = if entity == -1 { self.local_model(1.0, Some(&view), crate::models::FULL_LIGHT) } else { self.gui_entity_model(entity, &view, cam) };
                let proj = glam::Mat4::orthographic_rh(0.0, w, h, 0.0, 1000.0, 3000.0) * glam::Mat4::from_translation(glam::Vec3::new(0.0, 0.0, -2000.0));
                Some(mc_render::GuiModels { at, proj, viewport: None, models })
            }
            Gui3d::Book { at, partial, .. } => {
                use glam::{Mat4, Vec3};
                let b = self.inv.ui.book.clone();
                let open = b.o_open + (b.open - b.o_open) * partial;
                // (x - truncateDoubleToInt(x)) * 1,6 - 0,3, preso em 0..1.
                let page = |o: f32| {
                    let v = b.o_flip + (b.flip - b.o_flip) * partial + o;
                    ((v - v.trunc()) * 1.6 - 0.3).clamp(0.0, 1.0)
                };
                let k = 1.0 - open;
                let m = Mat4::from_translation(Vec3::new(0.0, 3.3, -16.0))
                    * Mat4::from_scale(Vec3::splat(5.0))
                    * Mat4::from_rotation_z(std::f32::consts::PI)
                    * Mat4::from_rotation_x(20f32.to_radians())
                    * Mat4::from_translation(Vec3::new(k * 0.2, k * 0.1, k * 0.25))
                    * Mat4::from_rotation_y((-k * 90.0 - 90.0).to_radians())
                    * Mat4::from_rotation_x(std::f32::consts::PI);
                let r = self.renderer.as_mut()?;
                let tex = self.entities.textures_mut().jar(r, &mut self.jar, "entity/enchanting_table_book")?;
                let mut geo = mc_world::mesh::Geometry::default();
                // enableStandardItemLighting com a modelview zerada: as luzes ficam como no mundo.
                let mut sink = crate::models::Sink { geo: &mut geo, tex: (64.0, 32.0), tint: [1.0; 3], alpha: 255, light: crate::models::FULL_LIGHT, lights: crate::models::WORLD_LIGHTS };
                crate::enchant::emit_book(&mut sink, &m, 0.0, page(0.25), page(0.75), open);
                let proj = Mat4::from_translation(Vec3::new(-0.34, 0.23, 0.0)) * Mat4::perspective_rh(90f32.to_radians(), 1.3333334, 9.0, 80.0);
                // glViewport((sw - 320) / 2 * escala, (sh - 240) / 2 * escala, 320 * escala, 240 * escala), y de baixo.
                let (dw, dh) = r.size();
                let scale = (dw as f32 / w).round() as i32;
                let (sw, sh) = (w.ceil() as i32, h.ceil() as i32);
                let (x, y_bottom, vw, vh) = ((sw - 320) / 2 * scale, (sh - 240) / 2 * scale, 320 * scale, 240 * scale);
                let viewport = Some([x as f32, (dh as i32 - y_bottom - vh) as f32, vw as f32, vh as f32]);
                Some(mc_render::GuiModels { at, proj, viewport, models: vec![DrawItem { tex: Tex::Custom(tex), geo, translucent: false }] })
            }
        }
    }

    /// Outra entidade numa tela (o cavalo do `GuiScreenHorseInventory`).
    fn gui_entity_model(&mut self, entity: i32, view: &crate::entities::GuiView, cam: &mc_render::Camera) -> Vec<DrawItem> {
        let Some(r) = &mut self.renderer else { return Vec::new() };
        self.entities.draw_in_gui(entity, view, cam, &self.world, r, &mut self.jar, &mut self.items, &self.table)
    }

    /// `RenderGuardian`: o raio do guardião até o alvo (metadado 17), opaco e com luz cheia, girando com a idade
    /// do mundo e mudando de cor com o tempo de ataque.
    fn guardian_beams(&mut self, t: f32) -> Vec<DrawItem> {
        use crate::entities::Kind;
        use mc_world::mesh::{Geometry, Vertex};
        let td = t as f64;
        let mut beams = Vec::new();
        for e in self.entities.list.values().filter(|e| e.kind == Kind::Mob(68) && e.attack_target != 0) {
            // getTargetedEntity: só EntityLivingBase; o próprio jogador não está na lista.
            let target = if Some(e.attack_target) == self.entities.own_id {
                Some((self.player.prev.lerp(self.player.pos, td), 1.8))
            } else {
                self.entities.list.get(&e.attack_target).filter(|o| matches!(o.kind, Kind::Player { .. } | Kind::Mob(_))).map(|o| (o.prev.lerp(o.pos, td), o.size().1 as f64))
            };
            let Some((tpos, height)) = target else { continue };
            let dur = if e.meta_int(16) & 4 != 0 { 60.0 } else { 80.0 };
            let progress = (e.attack_time as f32 + t) / dur;
            let eye = e.eye_height() as f64;
            let gpos = e.prev.lerp(e.pos, td);
            let from = gpos + DVec3::new(0.0, eye, 0.0);
            let d = tpos + DVec3::new(0.0, height * 0.5, 0.0) - from;
            let len = d.length() + 1.0;
            let n = d.normalize_or_zero();
            let (a, b) = (n.y.acos() as f32, n.z.atan2(n.x) as f32);
            let m = glam::Mat4::from_translation(from.as_vec3())
                * crate::camera::rot((std::f32::consts::FRAC_PI_2 - b) * 57.295776, 0.0, 1.0, 0.0)
                * crate::camera::rot(a * 57.295776, 1.0, 0.0, 0.0);
            let time = self.entities.world_age as f32 + t;
            let scroll = time * 0.5 % 1.0;
            let spin = time as f64 * 0.05 * (1.0 - 2.5);
            let p2 = progress * progress;
            // WorldRenderer.color(int r, g, b, a) monta `a << 24 | b << 16 | g << 8 | r`: o vermelho passa de 255 no
            // fim do ataque e o excesso cai no verde.
            let packed = (64 + (p2 * 240.0) as i32) | (32 + (p2 * 192.0) as i32) << 8 | (128 - (p2 * 64.0) as i32) << 16;
            let color = [packed as u8, (packed >> 8) as u8, (packed >> 16) as u8, 255];
            let (r0, r1) = (0.2f64, 0.2 * 1.41);
            let at = |ang: f64, r: f64| (ang.cos() * r, ang.sin() * r);
            let c = |k: f64| at(spin + k, r1);
            let s = |k: f64| at(spin + k, r0);
            let (v0, v1) = (-1.0 + scroll as f64, len * (0.5 / r0) + (-1.0 + scroll as f64));
            let full = crate::models::FULL_LIGHT;
            let v = |x: f64, y: f64, z: f64, u: f64, w: f64| Vertex { pos: m.transform_point3(glam::Vec3::new(x as f32, y as f32, z as f32)).to_array(), uv: [u as f32, w as f32], color, light: full };
            let mut geo = Geometry::default();
            let (q0, q1, q2, q3) = (s(std::f64::consts::PI), s(0.0), s(std::f64::consts::FRAC_PI_2), s(4.71238898038469));
            geo.quad([v(q0.0, len, q0.1, 0.4999, v1), v(q0.0, 0.0, q0.1, 0.4999, v0), v(q1.0, 0.0, q1.1, 0.0, v0), v(q1.0, len, q1.1, 0.0, v1)]);
            geo.quad([v(q2.0, len, q2.1, 0.4999, v1), v(q2.0, 0.0, q2.1, 0.4999, v0), v(q3.0, 0.0, q3.1, 0.0, v0), v(q3.0, len, q3.1, 0.0, v1)]);
            let cap = if e.age % 2 == 0 { 0.5 } else { 0.0 };
            let (c0, c1, c2, c3) = (c(2.356194490192345), c(0.7853981633974483), c(5.497787143782138), c(3.9269908169872414));
            geo.quad([v(c0.0, len, c0.1, 0.5, cap + 0.5), v(c1.0, len, c1.1, 1.0, cap + 0.5), v(c2.0, len, c2.1, 1.0, cap), v(c3.0, len, c3.1, 0.5, cap)]);
            beams.push(geo);
        }
        let Some(r) = self.renderer.as_mut() else { return Vec::new() };
        let Some(tex) = self.entities.textures_mut().jar(r, &mut self.jar, "entity/guardian_beam") else { return Vec::new() };
        beams.into_iter().map(|geo| DrawItem { tex: Tex::Custom(tex), geo, translucent: false }).collect()
    }

    /// O próprio jogador tem anzol na água (`fishEntity`).
    fn own_fishing(&self) -> bool {
        self.entities.list.values().any(|e| matches!(e.kind, crate::entities::Kind::Object { kind: crate::naovivas::FISH_HOOK, data } if Some(data) == self.entities.own_id))
    }

    /// Linhas dos anzóis (`RenderFish`): o pescador é a entidade do campo `data` do 0x0E. O próprio
    /// jogador em primeira pessoa segura a vara na frente da câmera; os outros, na mão pelo corpo.
    fn fishing_lines(&self, t: f32) -> mc_world::mesh::Geometry {
        use crate::entities::Kind;
        let mut geo = mc_world::mesh::Geometry::default();
        let td = t as f64;
        for e in self.entities.list.values() {
            let Kind::Object { kind: crate::naovivas::FISH_HOOK, data } = e.kind else { continue };
            let hook = e.prev.lerp(e.pos, td) + DVec3::new(0.0, 0.25, 0.0);
            // (posição interpolada, pitch, yaw, corpo, balanço, altura dos olhos, agachado, é o local em 1ª pessoa)
            let angler = if Some(data) == self.entities.own_id {
                let p = &self.player;
                let lerp = |a: f32, b: f32| a + (b - a) * t;
                Some((p.prev.lerp(p.pos, td), lerp(p.prev_pitch, p.pitch), lerp(p.prev_yaw, p.yaw), lerp(p.prev_render_yaw_offset, p.render_yaw_offset), p.swing(t), p.eye_height(), p.sneaking, self.camera.third_person == 0))
            } else {
                self.entities.list.get(&data).filter(|a| matches!(a.kind, Kind::Player { .. })).map(|a| {
                    let lerp = |x: f32, y: f32| x + (y - x) * t;
                    (a.prev.lerp(a.pos, td), lerp(a.prev_pitch, a.pitch), lerp(a.prev_yaw, a.yaw), lerp(a.prev_body_yaw, a.body_yaw), a.swing_at(t), a.eye_height() as f64, a.sneaking(), false)
                })
            };
            let Some((pos, pitch, yaw, body, swing, eye, sneak, first)) = angler else { continue };
            let (rod, dy) = if first {
                let f = crate::mth::sin(swing.sqrt() * std::f32::consts::PI);
                let mut v = DVec3::new(-0.36, 0.03, 0.35);
                v = interact::rotate_pitch(v, -pitch * std::f32::consts::PI / 180.0);
                v = interact::rotate_yaw(v, -yaw * std::f32::consts::PI / 180.0);
                v = interact::rotate_yaw(v, f * 0.5);
                v = interact::rotate_pitch(v, -f * 0.7);
                (pos + v, eye)
            } else {
                let b = body * std::f32::consts::PI / 180.0;
                let (s, c) = (crate::mth::sin(b) as f64, crate::mth::cos(b) as f64);
                (DVec3::new(pos.x - c * 0.35 - s * 0.8, pos.y + eye - 0.45, pos.z - s * 0.35 + c * 0.8), if sneak { -0.1875 } else { 0.0 })
            };
            interact::fishing_line(&mut geo, rod, hook, dy, white_uv());
        }
        geo
    }

    /// Caixas do F3+B de cada entidade desenhada (não invisível, dentro da distância) e do próprio
    /// jogador fora da primeira pessoa.
    fn debug_boxes_geo(&self, partial: f32) -> mc_world::mesh::Geometry {
        use crate::entities::{Kind, ARMOR_STAND};
        let mut geo = mc_world::mesh::Geometry::default();
        let max = self.menu.settings.render_distance.clamp(2, 32) as f64 * 16.0;
        let eye = self.player.pos + DVec3::new(0.0, self.player.eye_height(), 0.0);
        for e in self.entities.list.values() {
            let pos = e.prev.lerp(e.pos, partial as f64);
            if e.invisible() || pos.distance(eye) > max {
                continue;
            }
            let (w, h) = e.size();
            let living = matches!(e.kind, Kind::Player { .. } | Kind::Mob(_) | Kind::Object { kind: ARMOR_STAND, .. });
            // EntityLivingBase.getLook interpola o pitch e a cabeça.
            let look = living.then(|| {
                let lerp = |a: f32, b: f32| a + (b - a) * partial;
                (e.eye_height() as f64, interact::look_vector(lerp(e.prev_pitch, e.pitch), lerp(e.prev_head_yaw, e.head_yaw)))
            });
            interact::debug_box(&mut geo, pos, w as f64, h as f64, look, white_uv());
        }
        if self.camera.third_person != 0 {
            let p = &self.player;
            interact::debug_box(&mut geo, p.pos, 0.6, 1.8, Some((p.eye_height(), interact::look_vector(p.pitch, p.yaw))), white_uv());
        }
        geo
    }

    /// `!hideGUI || currentScreen != null` (chat, inventário e menus contam como tela).
    fn gui_visible(&self) -> bool {
        !self.hide_gui || self.hud.chat_open || self.inv.is_open() || self.menu.screen != crate::menu::Screen::None
    }

    /// Combinações com F3 de `Minecraft.runTick` (sem tela aberta). F3+F não faz nada na 1.8.9
    /// (`setOptionValue` não trata RENDER_DISTANCE).
    fn f3_combo(&mut self, code: KeyCode) {
        match code {
            KeyCode::KeyD => self.hud.chat.clear(),
            KeyCode::KeyS | KeyCode::KeyT => self.reload_resources(),
            KeyCode::KeyA => self.world.mark_all_dirty(),
            KeyCode::KeyH => {
                self.menu.settings.advanced_item_tooltips ^= true;
                self.menu.settings.save();
            }
            KeyCode::KeyP => {
                self.menu.settings.pause_on_lost_focus ^= true;
                self.menu.settings.save();
            }
            KeyCode::KeyB => self.debug_boxes = !self.debug_boxes,
            _ => {}
        }
    }

    /// O que a tela F3 mostra neste quadro.
    fn debug_info(&self) -> DebugInfo {
        use mc_world::states;
        let s = &self.menu.settings;
        let p = &self.player;
        let (bx, by, bz) = (p.pos.x.floor() as i32, p.pos.y.floor() as i32, p.pos.z.floor() as i32);
        let loaded = (0..256).contains(&by) && self.world.is_loaded(bx, bz);
        let light = crate::entities::light_at(&self.world, p.pos);
        let looking_at = self.target.as_ref().map(|h| {
            let (x, y, z) = h.pos;
            let st = self.world.block(x, y, z);
            let nb = |dx: i32, dy: i32, dz: i32| self.world.block(x + dx, y + dy, z + dz);
            let bits = if states::dyn_count(st >> 4) > 1 { states::actual_bits(&*self.table, st, [x, y, z], &nb) } else { 0 };
            let variant = states::location(st >> 4, st & 15, bits).map_or(String::new(), |l| l.1);
            ((x, y, z), st, variant)
        });
        let gl = self.renderer.as_ref().map_or(Default::default(), |r| {
            let i = &r.adapter_info;
            let vendor = if i.driver.is_empty() { format!("0x{:04X}", i.vendor) } else { i.driver.clone() };
            [vendor, i.name.clone(), format!("{:?} {}", i.backend, i.driver_info)]
        });
        DebugInfo {
            fps: self.fps,
            chunk_updates: self.chunk_updates_shown,
            max_fps: s.max_fps,
            vsync: s.enable_vsync,
            fancy: s.fancy_graphics,
            clouds: s.clouds,
            vbo: s.use_vbo,
            sections_drawn: self.renderer.as_ref().map_or(0, |r| r.drawn_sections.get()),
            render_distance: s.render_distance,
            pending_compile: self.world.dirty.len() + self.mesh_in_flight,
            free_builders: rayon::current_num_threads().saturating_sub(self.mesh_in_flight),
            // O próprio jogador conta no total e, fora da primeira pessoa, entre os desenhados.
            entities_rendered: self.entities.rendered + (self.camera.third_person != 0) as usize,
            entities_total: self.entities.list.len() + 1,
            particles: self.particles.count(),
            columns: self.world.columns.len(),
            pos: [p.pos.x, p.pos.y, p.pos.z],
            yaw: p.yaw,
            pitch: p.pitch,
            loaded,
            biome: mc_world::biome::biome(self.world.biome_at(bx, bz)).name.to_string(),
            sky: light[0] / 17,
            block: light[1] / 17,
            has_sky: self.world.has_sky,
            difficulty: self.difficulty,
            world_time: self.time_of_day.abs(),
            looking_at,
            reduced: p.reduced_debug || s.reduced_debug_info,
            display: self.window.as_ref().map_or((0, 0), |w| (w.inner_size().width, w.inner_size().height)),
            gl,
        }
    }

    /// Entradas do tab a partir da lista de jogadores (`getPlayerInfoMap` ordenado pelo
    /// `PlayerComparator`) e, com cabeças, a skin de cada uma das 80 primeiras no atlas.
    fn update_tab(&mut self) {
        use crate::entities::Kind;
        let sb = &self.hud.scoreboard;
        let mut entries: Vec<crate::tab_list::Entry> = self
            .entities
            .players
            .iter()
            .map(|(uuid, p)| crate::tab_list::Entry {
                uuid: *uuid,
                profile: p.name.clone(),
                name: match &p.display_name {
                    Some(j) => crate::chat::to_legacy(j),
                    None => sb.format_name(&p.name),
                },
                team: sb.player_team.get(&p.name).cloned().unwrap_or_default(),
                ping: p.ping,
                spectator: p.gamemode == 3,
                head: None,
            })
            .collect();
        entries.sort_by(crate::tab_list::compare);
        if self.hud.tab.heads {
            // getPlayerEntityByUUID: só quem está no mundo tem as partes do modelo (byte 10).
            let mut parts: HashMap<crate::entity_net::Uuid, u8> = HashMap::new();
            for e in self.entities.list.values() {
                if let Kind::Player { uuid, .. } = &e.kind {
                    parts.insert(*uuid, e.meta_byte(10) as u8);
                }
            }
            for (i, e) in entries.iter_mut().take(80).enumerate() {
                let parts = if e.profile == self.name { Some(self.menu.settings.model_parts) } else { parts.get(&e.uuid).copied() };
                // EnumPlayerModelParts: CAPE bit 0, HAT bit 6.
                let hat = parts.is_some_and(|p| p & 0x40 != 0);
                let flip = parts.is_some_and(|p| p & 0x01 != 0) && (e.profile == "Dinnerbone" || e.profile == "Grumm");
                let skin = self.entities.players.get(&e.uuid).and_then(|p| p.skin.clone());
                let textures = self.entities.textures_mut();
                // getLocationSkin: a padrão enquanto a baixada não chegou.
                let url = skin.filter(|s| textures.skin(&s.url).is_some()).map(|s| s.url);
                let source = url.clone().unwrap_or_else(|| crate::skins::default_skin(e.uuid).0.to_string());
                if self.tab_heads.len() <= i {
                    self.tab_heads.push((self.next_favicon, None));
                    self.next_favicon -= 1;
                }
                let key = Some((source.clone(), hat, flip));
                if self.tab_heads[i].1 != key {
                    let img = match &url {
                        Some(u) => textures.skin_image(u).cloned(),
                        None => textures.jar_image(&mut self.jar, &source),
                    };
                    if let (Some(img), Some(r)) = (img, &mut self.renderer) {
                        let (x, y) = icon_origin(self.tab_heads[i].0);
                        let head = crate::tab_list::head_image(&img, hat, flip);
                        r.write_ui(x, y, head.width(), head.height(), head.as_raw());
                        self.tab_heads[i].1 = key;
                    }
                }
                e.head = Some(self.tab_heads[i].0);
            }
        }
        self.hud.tab.entries = entries;
    }

    /// Regrava os ícones de cabeça cuja skin terminou de baixar (o jogo redesenha o ícone a cada quadro).
    fn refresh_heads(&mut self) {
        if self.heads_pending.is_empty() {
            return;
        }
        if let Some(r) = &mut self.renderer {
            self.entities.textures_mut().poll(r);
        }
        for (key, item) in std::mem::take(&mut self.heads_pending) {
            if !self.ensure_entity_icon(key, item.clone()) {
                // ensure_entity_icon já recolocou na fila.
            }
        }
    }

    /// Sai do servidor e mostra `screen` (lista ou motivo da desconexão).
    fn leave(&mut self, screen: crate::menu::Screen) {
        self.net = None;
        self.clear_server_pack();
        self.reset_world(0);
        self.hud = Hud::default();
        self.inv = Inventory::default();
        self.target = None;
        self.target_entity = None;
        self.digging = None;
        self.set_grab(false);
        self.menu.in_game = false;
        self.menu.show(screen);
    }

    fn join(&mut self, addr: String) {
        self.leave(crate::menu::Screen::Connecting(addr.clone()));
        self.server = addr.clone();
        let view = self.menu.settings.render_distance.clamp(2, 32) as u8;
        self.net = Some(net::start(addr, self.name.clone(), self.session.clone(), view));
    }

    /// Escala da GUI pelas opções (`ScaledResolution`).
    fn gui_scale(&self) -> f32 {
        let (w, h) = self.renderer.as_ref().map_or((1280, 720), |r| r.size());
        self.menu.settings.scale_for(w, h) as f32
    }

    /// Tamanho da tela em pixels de GUI.
    fn gui_size(&self) -> (f32, f32) {
        let (w, h) = self.renderer.as_ref().map_or((1280, 720), |r| r.size());
        let s = self.gui_scale();
        (w as f32 / s, h as f32 / s)
    }

    /// A keybind `desc` (`key.forward`...) está apertada? Botões do mouse usam códigos negativos.
    fn bind_down(&self, desc: &str) -> bool {
        let code = self.menu.settings.key(desc);
        match code {
            -100 => self.left_down,
            -99 => self.right_down,
            c if c > 0 => crate::settings::winit_key(c).is_some_and(|k| self.keys.contains(&k)),
            _ => false,
        }
    }

    /// A tecla `code` é a da keybind `desc`?
    fn is_bind(&self, desc: &str, code: KeyCode) -> bool {
        let c = self.menu.settings.key(desc);
        c > 0 && crate::settings::lwjgl_code(code) == c
    }

    /// C15 Client Settings com idioma, distância, chat e partes da skin.
    fn send_client_settings(&self) {
        let s = &self.menu.settings;
        self.send(serverbound::client_settings_full(
            &s.language,
            s.render_distance.clamp(2, 32) as u8,
            s.chat_visibility as u8,
            s.chat_colors,
            s.model_parts,
        ));
    }

    /// Aplica o que mudou nas opções: som, vsync, tela cheia, idioma e pacote de opções.
    fn apply_settings(&mut self) {
        if self.applied.as_ref() == Some(&self.menu.settings) {
            return;
        }
        let s = self.menu.settings.clone();
        let old = self.applied.take();
        self.sound.set_volumes(s.sound);
        // gameSettings.fovSetting e viewBobbing alimentam a câmera do EntityRenderer.
        self.camera.fov_setting = s.fov;
        self.camera.view_bobbing = s.view_bobbing;
        if let Some(r) = &mut self.renderer {
            r.set_vsync(s.enable_vsync);
        }
        if let Some(w) = &self.window {
            if old.as_ref().map(|o| o.fullscreen) != Some(s.fullscreen) && (w.fullscreen().is_some() != s.fullscreen) {
                w.set_fullscreen(if s.fullscreen { Some(Fullscreen::Borderless(None)) } else { None });
            }
        }
        // GameSettings.setOptionValue(GRAPHICS) -> RenderGlobal.loadRenderers: folhas novas e todas as malhas de novo.
        if old.as_ref().is_some_and(|o| o.fancy_graphics != s.fancy_graphics) {
            let mut table = (*self.table).clone();
            table.set_fancy_leaves(s.fancy_graphics);
            self.table = Arc::new(table);
            self.world.mark_all_dirty();
        }
        if self.menu.language_changed {
            self.menu.language_changed = false;
            crate::lang::install(&s.language, crate::lang::en_us_text().as_deref());
            self.lang = crate::lang::Lang::load(&s.language);
        }
        if let Some(o) = &old {
            let net_fields = |x: &crate::settings::Settings| (x.language.clone(), x.render_distance, x.chat_visibility, x.chat_colors, x.model_parts);
            if net_fields(o) != net_fields(&s) && self.spawned {
                self.send_client_settings();
            }
        }
        self.applied = Some(s);
    }

    /// O login Microsoft avisa o código e depois a conta (ou o erro).
    fn poll_login(&mut self) {
        let Some(rx) = &self.login_rx else { return };
        while let Ok(ev) = rx.try_recv() {
            match ev {
                crate::msa::Event::Code { url, code } => {
                    self.menu.login_status = vec![format!("Abra {url}"), format!("e digite o código: {code}"), "Esperando a aprovação...".to_string()];
                }
                crate::msa::Event::Done(Ok(acc)) => {
                    self.use_account(&acc);
                    self.login_rx = None;
                    return;
                }
                crate::msa::Event::Done(Err(e)) => {
                    self.menu.login_status = vec!["Não foi possível entrar:".to_string(), e];
                    self.login_rx = None;
                    return;
                }
            }
        }
    }

    /// A conta escolhida vale para as próximas conexões; volta ao menu principal.
    fn use_account(&mut self, acc: &crate::msa::Account) {
        self.session = acc.session();
        self.name = acc.name().to_string();
        self.menu.account = self.name.clone();
        self.menu.login_status.clear();
        self.menu.show(crate::menu::Screen::MainMenu);
    }

    /// Resultado de uma tela.
    fn menu_action(&mut self, a: Option<crate::menu::Action>, el: &ActiveEventLoop) {
        match a {
            Some(crate::menu::Action::LoginMicrosoft) => match crate::msa::client_id() {
                Some(id) => {
                    self.menu.login_status = vec!["Pedindo o código à Microsoft...".to_string()];
                    self.login_rx = Some(crate::msa::start_device_login(id));
                }
                None => {
                    self.menu.login_status = vec![
                        "Falta o Client ID do seu app Microsoft (Azure).".to_string(),
                        "Defina MINE_RS_MS_CLIENT_ID ou crie o arquivo".to_string(),
                        format!("{}", crate::settings::config_dir().join("microsoft-client-id").display()),
                    ];
                }
            },
            Some(crate::menu::Action::LoginOffline(nick)) => {
                let acc = crate::msa::Account::Offline(nick);
                let _ = acc.save();
                self.use_account(&acc);
            }
            Some(crate::menu::Action::Join(addr)) => self.join(addr),
            Some(crate::menu::Action::Resume) => {
                self.menu.screen = crate::menu::Screen::None;
                self.set_grab(true);
            }
            Some(crate::menu::Action::Disconnect) => self.leave(crate::menu::Screen::ServerList),
            Some(crate::menu::Action::Respawn) => self.send(serverbound::client_status(serverbound::STATUS_PERFORM_RESPAWN)),
            Some(crate::menu::Action::ToTitle) => self.leave(crate::menu::Screen::MainMenu),
            Some(crate::menu::Action::ReloadResources) => self.reload_resources(),
            Some(crate::menu::Action::ServerPackAnswer(url, hash, yes)) => {
                // ServerData.setResourceMode + ServerList.saveSingleServer.
                if let Some(i) = self.server_entry() {
                    self.menu.servers[i].resource_mode = if yes { 0 } else { 1 };
                    self.menu.save_servers();
                }
                self.set_grab(true);
                if yes {
                    self.accept_server_pack(url, hash);
                } else {
                    self.send_pack_status(&hash, crate::server_pack::Status::Declined);
                }
            }
            Some(crate::menu::Action::LinkAnswer(url, open, copy)) => {
                if copy {
                    crate::desktop::set(&url);
                }
                if open {
                    crate::desktop::open_url(&url);
                }
                // confirmClicked -> displayGuiScreen(GuiChat): o chat reabre com o texto inicial.
                self.hud.open_chat("");
                self.set_grab(false);
            }
            Some(crate::menu::Action::Quit) => el.exit(),
            None => {}
        }
    }

    /// Ícones dos servidores que chegaram pelo ping.
    fn update_favicons(&mut self) {
        // Ícones (pack.png) da tela de pacotes de recursos.
        if self.menu.packs.is_some() {
            let default_icon = self.jar.default_icon();
            let requests = self.menu.packs.as_mut().map(|p| p.take_icon_requests(default_icon.as_deref())).unwrap_or_default();
            for (id, png) in requests {
                let Ok(img) = image::load_from_memory(&png) else { continue };
                let img = image::imageops::resize(&img.to_rgba8(), 32, 32, image::imageops::FilterType::Nearest);
                let slot = self.next_favicon;
                self.next_favicon -= 1;
                let (x, y) = icon_origin(slot);
                if let Some(r) = &mut self.renderer {
                    r.write_ui(x, y, 32, 32, img.as_raw());
                    if let Some(p) = &mut self.menu.packs {
                        p.icons.insert(id, slot);
                    }
                }
            }
        }
        for (i, png) in self.menu.poll() {
            let Ok(img) = image::load_from_memory(&png) else { continue };
            let img = image::imageops::resize(&img.to_rgba8(), 32, 32, image::imageops::FilterType::Nearest);
            let slot = match self.menu.servers[i].icon_slot {
                Some(s) => s,
                None => {
                    let s = self.next_favicon;
                    self.next_favicon -= 1;
                    s
                }
            };
            let (x, y) = icon_origin(slot);
            if let Some(r) = &mut self.renderer {
                r.write_ui(x, y, 32, 32, img.as_raw());
                self.menu.servers[i].icon_slot = Some(slot);
            }
        }
        // Ícones salvos no servers.dat, uma vez.
        for i in 0..self.menu.servers.len() {
            if self.menu.servers[i].icon_slot.is_none() {
                if let Some(png) = self.menu.servers[i].icon_png.take() {
                    if let Ok(img) = image::load_from_memory(&png) {
                        let img = image::imageops::resize(&img.to_rgba8(), 32, 32, image::imageops::FilterType::Nearest);
                        let slot = self.next_favicon;
                        self.next_favicon -= 1;
                        let (x, y) = icon_origin(slot);
                        if let Some(r) = &mut self.renderer {
                            r.write_ui(x, y, 32, 32, img.as_raw());
                            self.menu.servers[i].icon_slot = Some(slot);
                        }
                    }
                }
            }
        }
    }

    /// `Minecraft.refreshResources`: relê pacotes, modelos, atlas, GUI, fonte, idiomas, sons e
    /// animações sem reiniciar o jogo, e refaz as malhas do mundo.
    fn reload_resources(&mut self) {
        let t0 = Instant::now();
        let chosen = crate::packs::selected(&mut self.menu.settings);
        let paths = crate::packs::stack_paths(&chosen, self.server_pack.as_ref());
        for e in self.jar.set_packs(&paths) {
            eprintln!("pacote de recursos ignorado: {e}");
        }
        let mut table = BlockTable::with_models(&mut |p| self.jar.bytes(p));
        table.set_fancy_leaves(self.menu.settings.fancy_graphics);
        self.table = Arc::new(table);
        let (atlas, missing) = match build_atlas(&mut self.jar, &self.table.textures) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("recarga de recursos: {e}");
                return;
            }
        };
        if !missing.is_empty() {
            eprintln!("texturas não encontradas: {}", missing.join(", "));
        }
        let (ui_rgba, font) = build_ui_atlas(&mut self.jar);
        self.anim = Animations::load(&mut self.jar, &self.table.textures, atlas.tile);
        if let Some(r) = &mut self.renderer {
            r.replace_atlases(&atlas, &ui_rgba);
            self.entities.textures_mut().reload_jar(r);
        } else {
            self.atlas = Some((atlas, ui_rgba));
        }
        self.ui = UiBatch::new(font);
        self.entities.clear_names();
        // Tudo que foi gravado no atlas da GUI se perdeu: ícones de item, favicons e ícones de pacote.
        self.icons = ItemIcons::default();
        self.icon_map.clear();
        self.heads_pending.clear();
        self.tab_heads.clear();
        self.items = Default::default();
        self.next_favicon = mc_render::ui::ITEM_SLOTS - 1;
        self.menu.reset_icons();
        crate::lang::install(&self.menu.settings.language, crate::lang::en_us_text().as_deref());
        self.lang = crate::lang::Lang::load(&self.menu.settings.language);
        self.sound.reload(&mut self.jar);
        self.world.mark_all_dirty();
        println!("recursos recarregados em {} ms ({} pacotes)", t0.elapsed().as_millis(), self.jar.pack_names().len());
    }

    /// Entrada do servidor atual na lista (o `ServerData` do Java), pelo endereço.
    fn server_entry(&self) -> Option<usize> {
        self.menu.servers.iter().position(|s| s.ip.eq_ignore_ascii_case(&self.server))
    }

    fn send_pack_status(&self, hash: &str, status: crate::server_pack::Status) {
        self.send(crate::server_pack::status_packet(hash, status));
    }

    /// `NetHandlerPlayClient.handleResourcePack`.
    fn on_resource_pack(&mut self, url: String, hash: String) {
        use crate::server_pack::Status;
        if let Some(name) = url.strip_prefix("level://") {
            // Pacote de um mundo local: só existe em ~/.minecraft/saves/<nome>.
            let file = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".minecraft/saves").join(name);
            if !name.contains("..") && !name.contains('/') && file.is_file() {
                self.send_pack_status(&hash, Status::Accepted);
                self.server_pack = Some(file);
                self.reload_resources();
                self.send_pack_status(&hash, Status::SuccessfullyLoaded);
            } else {
                self.send_pack_status(&hash, Status::FailedDownload);
            }
            return;
        }
        // resourceMode: 0 ligado, 1 desligado, 2 perguntar (sem entrada na lista = perguntar).
        let mode = self.server_entry().map_or(2, |i| self.menu.servers[i].resource_mode);
        match mode {
            0 => self.accept_server_pack(url, hash),
            1 => self.send_pack_status(&hash, Status::Declined),
            _ => {
                self.set_grab(false);
                self.menu.show(crate::menu::Screen::ServerPack(url, hash));
            }
        }
    }

    /// Modo de jogo e nível de xp para as telas de container.
    fn inv_env(&self) -> crate::containers::Env {
        crate::containers::Env { creative: self.hud.gamemode == 1, level: self.hud.xp_level }
    }

    fn sync_inv_binds(&mut self) {
        let st = &self.menu.settings;
        let b = &mut self.inv.binds;
        for i in 0..9 {
            b.hotbar[i] = st.key(&format!("key.hotbar.{}", i + 1));
        }
        b.drop = st.key("key.drop");
        b.inventory = st.key("key.inventory");
        b.chat = st.key("key.chat");
        self.inv.env = self.inv_env();
    }

    /// Envia os pacotes das telas e devolve o mouse ao mundo quando a tela se fechou sozinha.
    fn drain_inv(&mut self) {
        self.inv.env = self.inv_env();
        for p in self.inv.take_packets() {
            self.send(p);
        }
        for name in std::mem::take(&mut self.inv.sounds) {
            self.sound.play_ui(name, 0.25, 1.0);
        }
        // A tile entity da placa muda junto com a digitação e fica com o texto final ao fechar.
        let edited = self.inv.editing_sign().map(|(p, l)| (p, l.clone())).or_else(|| self.inv.sign_done.take());
        if let Some((pos, lines)) = edited {
            if let Some(t) = self.tiles.tiles.get_mut(&pos) {
                t.text = lines;
            }
        }
        if self.inv.take_closed() {
            self.set_grab(true);
        }
    }

    fn container_event(&mut self, e: crate::container_net::ContainerEvent) {
        use crate::container_net::ContainerEvent as E;
        match e {
            E::Property { window, id, value } => self.inv.set_property(window, id, value),
            E::Trades { window, trades } => self.inv.set_trades(window, trades),
            E::SignEditor { pos } => {
                // `handleSignEditorOpen`: sem tile entity de placa no lugar, abre uma placa vazia.
                let block = (self.world.block(pos.0, pos.1, pos.2) >> 4) as u16;
                let lines = self.tiles.tiles.get(&pos).filter(|_| crate::tile_entities::is_sign(block)).map_or_else(Default::default, |t| t.text.clone());
                self.inv.open_sign(pos, block, lines);
                self.set_grab(false);
            }
            E::BookOpen => {
                // `MC|BOpen`: só abre com um livro escrito na mão.
                let slot = 36 + self.hud.selected as usize;
                if let Some(book) = self.inv.hotbar(self.hud.selected).filter(|b| b.id == crate::gui_special::WRITTEN_BOOK).cloned() {
                    self.inv.open_book(book, false, self.name.clone(), slot);
                    self.set_grab(false);
                }
            }
            E::UseBed { entity, .. } => {
                // `handleUseBed` -> `trySleep`: o `GuiSleepMP` abre pelo laço de `tick_sleep`.
                if Some(entity) == self.entities.own_id {
                    self.sleeping = true;
                }
            }
        }
    }

    /// `downloadResourcePack`: avisa o servidor, baixa (ou usa o cache) e aplica.
    fn accept_server_pack(&mut self, url: String, hash: String) {
        use crate::server_pack::Status;
        self.send_pack_status(&hash, Status::Accepted);
        // clearResourcePack: o pacote anterior sai logo.
        if self.server_pack.take().is_some() {
            self.reload_resources();
        }
        let (tx, generation) = (self.pack_tx.clone(), self.pack_gen);
        std::thread::spawn(move || {
            let mut last = (String::new(), -1);
            let result = crate::server_pack::prepare(&crate::server_pack::dir(), &url, &hash, &mut |msg, pct| {
                let message = if msg.is_empty() { last.0.clone() } else { msg.to_string() };
                if (message.clone(), pct) != last {
                    last = (message.clone(), pct);
                    let _ = tx.send(PackEvent::Progress { generation, message, percent: pct });
                }
            });
            let _ = tx.send(PackEvent::Done { generation, hash, result });
        });
    }

    /// Andamento dos downloads do pacote do servidor (`GuiScreenWorking`) e conclusão.
    fn poll_server_pack(&mut self) {
        use crate::server_pack::Status;
        while let Ok(ev) = self.pack_rx.try_recv() {
            match ev {
                PackEvent::Progress { generation, message, percent } if generation == self.pack_gen => {
                    self.menu.working = ("Downloading Resource Pack".to_string(), message, percent);
                    if self.menu.screen != crate::menu::Screen::Working {
                        self.set_grab(false);
                        self.menu.show(crate::menu::Screen::Working);
                    }
                }
                PackEvent::Done { generation, hash, result } if generation == self.pack_gen => {
                    // setDoneWorking: fecha a tela de progresso.
                    if self.menu.screen == crate::menu::Screen::Working {
                        self.menu.screen = crate::menu::Screen::None;
                        self.set_grab(true);
                    }
                    match result {
                        Ok(file) => {
                            self.server_pack = Some(file);
                            self.reload_resources();
                            self.send_pack_status(&hash, Status::SuccessfullyLoaded);
                        }
                        Err(e) => {
                            eprintln!("pacote do servidor: {e}");
                            self.send_pack_status(&hash, Status::FailedDownload);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// `clearResourcePack` ao sair do servidor.
    fn clear_server_pack(&mut self) {
        self.pack_gen = self.pack_gen.wrapping_add(1);
        if self.server_pack.take().is_some() {
            self.reload_resources();
        }
    }

    /// `displayGuiScreen(new GuiDownloadTerrain)`: substitui o container ou o chat aberto, sem C0D.
    fn download_terrain(&mut self) {
        self.inv.close_silently();
        self.hud.close_chat();
        self.menu.show(crate::menu::Screen::DownloadTerrain);
        self.terrain_ticks = 0;
        self.set_grab(false);
    }

    fn close_screen(&mut self) {
        if let Some(p) = self.inv.close() {
            self.send(p);
        }
        self.set_grab(true);
    }
}

/// Blocos que reagem ao clique direito (`Block.onBlockActivated` retorna true).
/// Metadata do bloco colocado pelo `ItemBlock.onItemUse`, só para os blocos sem `onBlockPlaced` próprio:
/// `getMetadata` devolve o dano nos `ItemMultiTexture`/`ItemCloth`/`ItemColored` e 0 no `ItemBlock` comum;
/// os troncos (`BlockLog.onBlockPlaced`) pegam o eixo da face clicada.
fn place_meta(id: u16, damage: u16, face: u8) -> Option<u16> {
    Some(match id {
        1 | 3 | 5 | 12 | 19 | 24 | 35 | 95 | 97 | 98 | 159 | 168 | 179 => damage,
        2 | 4 | 7 | 13..=16 | 20 | 21 | 22 | 41 | 42 | 45..=49 | 56 | 57 | 73 | 79 | 80 | 82 | 87 | 88 | 89 | 112 | 121 | 129 | 133 | 152 | 153 | 165 | 169
        | 172..=174 => 0,
        // EnumAxis: Y = 0, X = 4, Z = 8.
        17 | 162 => (damage & 3) | [0, 0, 8, 8, 4, 4][face.min(5) as usize],
        _ => return None,
    })
}

/// `Block.isReplaceable` do bloco clicado: ar, líquidos, mato, arbusto seco, fogo, cipó e neve de uma camada.
fn replaceable_clicked(state: u16) -> bool {
    match state >> 4 {
        78 => state & 15 == 0,
        id => replaceable_material(id),
    }
}

/// `Material.isReplaceable`: ar, água, lava, vine (mato, arbusto seco, cipó), fogo e camada de neve.
fn replaceable_material(id: u16) -> bool {
    matches!(id, 0 | 8..=11 | 31 | 32 | 51 | 78 | 106)
}

/// `Entity.preventEntitySpawning`: vivos, suporte de armadura, barco, carrinho, TNT acesa, bloco caindo e cristal.
fn prevents_spawning(kind: &crate::entities::Kind) -> bool {
    use crate::entities::Kind;
    use crate::naovivas::{BOAT, CRYSTAL, FALLING_BLOCK, MINECART, TNT};
    match kind {
        Kind::Player { .. } | Kind::Mob(_) => true,
        Kind::Object { kind, .. } => matches!(*kind, crate::entities::ARMOR_STAND | BOAT | MINECART | TNT | FALLING_BLOCK | CRYSTAL),
    }
}

/// `Block.onBlockActivated` que devolve true no cliente (o clique direito "pega" no bloco): inclui as cercas
/// (`BlockFence`: true no cliente por causa da coleira), o sensor de luz e o ovo do dragão.
fn interactive_block(id: u16) -> bool {
    matches!(
        id,
        23 | 25 | 26 | 54 | 58 | 61 | 62 | 64 | 69 | 71 | 77 | 84 | 85 | 92..=94 | 96 | 107 | 113 | 116..=118 | 122 | 130 | 137 | 138
            | 143 | 145 | 146 | 149 | 150 | 151 | 154 | 158 | 167 | 178 | 183..=197
    )
}

/// `ItemStack.isItemStackDamageable`: ferramentas, armas, armaduras, arco, vara, tesoura, isqueiro e cenoura na vara.
fn damageable(id: i16) -> bool {
    matches!(id, 256..=259 | 261 | 267..=279 | 283..=286 | 290..=294 | 298..=317 | 346 | 359 | 398)
}

/// `Item.onItemUse` que devolve true num bloco (o servidor também recebe o C08 posicionado e não o do ar).
/// Os condicionais seguem o bloco clicado; balde, comida, arco e espada devolvem false (vão para o C08 do ar).
fn item_uses_on_block(id: i16, block: u16, face: u8) -> bool {
    match id {
        // Placa, portas, redstone, açúcar, bolo, cama, repetidor, suporte de poções, caldeirão, vaso, cabeça,
        // comparador, bola de fogo, isqueiro, ovo de criação, moldura, pintura, suporte de armadura, fogos.
        323 | 324 | 330 | 331 | 338 | 354 | 355 | 356 | 379 | 380 | 390 | 397 | 404 | 385 | 259 | 383 | 389 | 321 | 416 | 401
        | 427..=431 => true,
        // Carrinhos só em trilho; enxada em terra/grama com a face de cima; sementes em terra arada.
        328 | 342 | 343 | 407 | 408 => matches!(block, 27 | 28 | 66 | 157),
        290..=294 => matches!(block, 2 | 3) && face == 1,
        295 | 361 | 362 | 391 | 392 => block == 60 && face == 1,
        // Olho do fim na moldura do portal; coleira na cerca.
        381 => block == 120,
        420 => matches!(block, 85 | 113 | 188..=192),
        _ => false,
    }
}

impl Game {
    /// `inventory.currentItem = slot`: o C09 sai no `syncCurrentPlayItem` do tick ou da próxima ação.
    fn select_slot(&mut self, slot: u8) {
        self.hud.selected = slot;
    }

    /// `Minecraft.runTick`: sem tela e dormindo abre o `GuiSleepMP`; ele fecha quando o jogador acorda.
    fn tick_sleep(&mut self) {
        if self.sleep_gui {
            if !self.sleeping {
                self.sleep_gui = false;
                self.hud.close_chat();
                self.set_grab(true);
            }
        } else if self.sleeping && self.spawned && !self.menu.dead && self.menu.screen == crate::menu::Screen::None && !self.inv.is_open() && !self.hud.chat_open {
            self.sleep_gui = true;
            self.hud.open_chat("");
            self.set_grab(false);
        }
    }

    /// `GuiSleepMP.wakeFromSleep`: C0B STOP_SLEEPING. Quem acorda é o servidor (animação 2); no mundo de
    /// teste, sem servidor, o próprio cliente faz esse papel.
    fn wake_from_sleep(&mut self) {
        let own = self.entities.own_id.unwrap_or(0);
        self.send(serverbound::entity_action(own, 2, 0));
        if self.test.is_some() {
            self.sleeping = false;
        }
    }

    /// `GuiSleepMP.keyTyped`: Esc acorda e Enter manda a mensagem sem fechar. Devolve se tratou a tecla.
    fn sleep_key(&mut self, code: KeyCode) -> bool {
        if !self.sleep_gui {
            return false;
        }
        match code {
            KeyCode::Escape => self.wake_from_sleep(),
            KeyCode::Enter | KeyCode::NumpadEnter => {
                let msg = self.hud.chat_screen.as_ref().map(|c| c.input.text.trim().to_string()).unwrap_or_default();
                if !msg.is_empty() {
                    self.send_chat_line(&msg, true);
                }
                // Limpa o texto e continua com o chat aberto.
                if let Some(c) = &mut self.hud.chat_screen {
                    c.input.set_text(&mut self.ui, "");
                }
                self.hud.chat.reset_scroll();
            }
            _ => return false,
        }
        true
    }

    /// Clique do mouse no `GuiSleepMP`: só o botão `multiplayer.stopSleeping` (id 1).
    fn sleep_click(&mut self) {
        let (w, h) = self.gui_size();
        let (mx, my) = (self.inv.mouse.0 as i32, self.inv.mouse.1 as i32);
        if crate::gui_special::sleep_button(w, h).pressed(mx, my) {
            self.sound.play_ui("gui.button.press", 0.25, 1.0);
            self.wake_from_sleep();
        }
    }

    /// Tecla com o chat aberto (`GuiChat.keyTyped`).
    fn chat_key(&mut self, code: KeyCode, text: Option<&str>) {
        let ctrl = self.keys.contains(&KeyCode::ControlLeft) || self.keys.contains(&KeyCode::ControlRight) || self.ctrl_held;
        let shift = self.keys.contains(&KeyCode::ShiftLeft) || self.shift_held;
        let fx = self.hud.chat_key(&mut self.ui, code, text, ctrl, shift);
        self.apply_chat_effects(fx);
    }

    /// `GuiScreen.sendChatMessage(msg, addToHistory)`: histórico e C01 (no máximo 100 caracteres).
    pub fn send_chat_line(&mut self, msg: &str, add_to_history: bool) {
        self.hud.remember_sent(msg, add_to_history);
        let msg: String = msg.chars().take(100).collect();
        self.send(serverbound::chat(&msg));
    }

    /// O que a tela do chat pediu: enviar, fechar, Tab, abrir link.
    fn apply_chat_effects(&mut self, fx: Vec<crate::chat_gui::ChatEffect>) {
        use crate::chat_gui::ChatEffect;
        for e in fx {
            match e {
                ChatEffect::Send(msg, history) => self.send_chat_line(&msg, history),
                ChatEffect::Close => {
                    self.hud.close_chat();
                    self.set_grab(true);
                }
                ChatEffect::TabComplete(text) => {
                    // Mirando um bloco, vai a posição dele (`objectMouseOver`).
                    let looking = self.target.as_ref().map(|h| h.pos);
                    self.send(serverbound::tab_complete(&text, looking));
                }
                ChatEffect::OpenLink(url) => {
                    crate::desktop::open_url(&url);
                }
                ChatEffect::ConfirmLink(url) => {
                    // displayGuiScreen(GuiConfirmOpenLink): o GuiChat fecha (onGuiClosed).
                    self.hud.close_chat();
                    self.menu.in_game = true;
                    self.menu.show(crate::menu::Screen::LinkConfirm(url));
                }
            }
        }
    }

    /// Clique do botão `button` com o chat aberto.
    fn chat_click(&mut self, button: i32) {
        let shift = self.keys.contains(&KeyCode::ShiftLeft) || self.shift_held;
        let fx = self.hud.chat_click(&mut self.ui, button, shift);
        self.apply_chat_effects(fx);
    }
}

impl ApplicationHandler for Game {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("mine-rs 1.8.9")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));
        let window = match el.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                self.exit_reason = Some(format!("não abri a janela: {e}"));
                el.exit();
                return;
            }
        };
        let (atlas, ui_rgba) = self.atlas.take().expect("atlas carregado");
        match pollster::block_on(Renderer::new(window.clone(), &atlas, &ui_rgba)) {
            Ok(mut r) => {
                println!("GPU: {}", r.adapter_name);
                // environment/*.png do jar: sol, lua, nuvens, céu do End, chuva e neve.
                for (tex, path) in [
                    (SkyTex::Sun, "environment/sun"),
                    (SkyTex::Moon, "environment/moon_phases"),
                    (SkyTex::Clouds, "environment/clouds"),
                    (SkyTex::EndSky, "environment/end_sky"),
                    (SkyTex::Rain, "environment/rain"),
                    (SkyTex::Snow, "environment/snow"),
                ] {
                    match self.jar.texture(path) {
                        Some(img) => r.set_sky_texture(tex, img.width(), img.height(), img.as_raw()),
                        None => eprintln!("textura {path} ausente no jar"),
                    }
                }
                self.renderer = Some(r);
            }
            Err(e) => {
                self.exit_reason = Some(format!("falha ao iniciar a GPU: {e}"));
                el.exit();
                return;
            }
        }
        self.window = Some(window);
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(s) => {
                if let Some(r) = &mut self.renderer {
                    r.resize(s.width, s.height);
                }
            }
            WindowEvent::Focused(false) => self.set_grab(false),
            WindowEvent::Occluded(o) => self.occluded = o,
            WindowEvent::CursorMoved { position, .. } => {
                let s = self.gui_scale() as f64;
                self.inv.mouse = ((position.x / s) as f32, (position.y / s) as f32);
                self.menu.mouse_move(self.inv.mouse.0, self.inv.mouse.1);
                let (pw, ph) = self.renderer.as_ref().map_or((1280, 720), |r| r.size());
                // Mouse.getX/getY do LWJGL: origem embaixo à esquerda.
                self.hud.chat_mouse = crate::hud::ChatMouse {
                    raw_x: position.x as i32,
                    raw_y: ph as i32 - 1 - position.y as i32,
                    factor: s as i32,
                    gx: self.inv.mouse.0 as i32,
                    gy: self.inv.mouse.1 as i32,
                };
                let _ = pw;
                if self.inv.is_open() {
                    let (gw, gh) = self.gui_size();
                    self.inv.moved(gw, gh);
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let pressed = state == ElementState::Pressed;
                // Soltar vale em qualquer tela: senão o botão solto com uma GUI aberta fica "preso" no jogo.
                if !pressed {
                    match button {
                        MouseButton::Left => self.left_down = false,
                        MouseButton::Right => self.right_down = false,
                        _ => {}
                    }
                }
                if self.stats.is_some() {
                    println!("[debug] mouse {button:?} {state:?} capturado={} tela={}", self.grabbed, self.inv.is_open());
                }
                if self.menu.screen != crate::menu::Screen::None {
                    let b = match button {
                        MouseButton::Left => 0,
                        MouseButton::Right => 1,
                        MouseButton::Middle => 2,
                        _ => return,
                    };
                    if pressed {
                        let a = self.menu.mouse_down(&mut self.ui, b);
                        self.menu_action(a, el);
                    } else if b == 0 {
                        self.menu.mouse_up();
                    }
                    return;
                }
                // Código LWJGL do botão (-100 + índice): os laterais são 3 (voltar) e 4 (avançar).
                let lw = match button {
                    MouseButton::Left => -100,
                    MouseButton::Right => -99,
                    MouseButton::Middle => -98,
                    MouseButton::Back => -97,
                    MouseButton::Forward => -96,
                    MouseButton::Other(n) => -100 + n as i32,
                };
                if self.inv.is_open() {
                    let b = match button {
                        MouseButton::Left => 0,
                        MouseButton::Right => 1,
                        MouseButton::Middle => 2,
                        _ => return,
                    };
                    let shift = self.keys.contains(&KeyCode::ShiftLeft) || self.shift_held;
                    let (gw, gh) = self.gui_size();
                    if pressed {
                        self.inv.click(&mut self.ui, b, shift, gw, gh);
                    } else {
                        self.inv.release(b, shift, gw, gh);
                    }
                    self.drain_inv();
                    return;
                }
                if self.hud.chat_open {
                    if pressed {
                        let b = match button {
                            MouseButton::Left => 0,
                            MouseButton::Right => 1,
                            MouseButton::Middle => 2,
                            _ => return,
                        };
                        // GuiSleepMP é um GuiChat com o botão de levantar.
                        let (gw, gh) = self.gui_size();
                        let (mx, my) = (self.inv.mouse.0 as i32, self.inv.mouse.1 as i32);
                        if self.sleep_gui && b == 0 && crate::gui_special::sleep_button(gw, gh).pressed(mx, my) {
                            self.sleep_click();
                        } else {
                            self.chat_click(b);
                        }
                    }
                    return;
                }
                // Minecraft.runTick: sem tela, qualquer botão apertado chama o setIngameFocus, e o KeyBinding.onTick
                // do mesmo clique já foi contado. O esquerdo não ataca (setIngameFocus põe o leftClickCounter em
                // 10000); o direito coloca o bloco na hora. Antes só o esquerdo capturava e o direito se perdia
                // (depois de fechar a loja, um baú ou o inventário).
                if !self.grabbed {
                    if pressed && !self.hud.chat_open {
                        self.set_grab(true);
                    }
                    if !self.grabbed || button == MouseButton::Left {
                        return;
                    }
                }
                // KeyBinding.setKeyBindState + onTick: o clique entra na fila (pressTime) e o runTick o processa.
                match button {
                    MouseButton::Left => {
                        self.left_down = pressed;
                        if pressed && self.spawned && !self.hud.chat_open {
                            self.attack_presses += 1;
                        }
                    }
                    MouseButton::Right => {
                        self.right_down = pressed;
                        if pressed && self.spawned && !self.hud.chat_open {
                            self.use_presses += 1;
                        }
                    }
                    // KeyBinding.onTick(botão - 100): os outros botões valem para as teclas ligadas a eles (hotbar).
                    _ => {
                        if pressed && self.spawned && !self.hud.chat_open && self.menu.settings.key("key.pickItem") == lw {
                            self.pick_presses += 1;
                        }
                        if pressed && self.spawned {
                            if let Some(n) = (1..=9).position(|i| self.menu.settings.key(&format!("key.hotbar.{i}")) == lw) {
                                if self.hud.gamemode != 3 {
                                    self.select_slot(n as u8);
                                }
                            }
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => (p.y / 40.0) as f32,
                };
                let step = if dy > 0.0 { 1 } else if dy < 0.0 { -1 } else { 0 };
                if self.menu.screen != crate::menu::Screen::None {
                    if step != 0 {
                        self.menu.wheel(step > 0);
                    }
                } else if self.inv.is_open() {
                    if step != 0 {
                        self.inv.wheel(step > 0);
                    }
                } else if self.hud.chat_open {
                    if step != 0 {
                        let shift = self.keys.contains(&KeyCode::ShiftLeft) || self.shift_held;
                        self.hud.chat_wheel(step > 0, shift);
                    }
                } else if self.grabbed && step != 0 {
                    self.select_slot((self.hud.selected as i32 - step).rem_euclid(9) as u8);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else { return };
                {
                    use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
                    crate::settings::learn_key(code, &event.key_without_modifiers());
                }
                if matches!(code, KeyCode::ShiftLeft | KeyCode::ShiftRight) {
                    self.shift_held = event.state == ElementState::Pressed;
                }
                // GuiScreen.isCtrlKeyDown lê o teclado de verdade: o Ctrl apertado antes de abrir a tela também conta.
                if matches!(code, KeyCode::ControlLeft | KeyCode::ControlRight) {
                    self.ctrl_held = event.state == ElementState::Pressed;
                }
                if self.hud.chat_open {
                    if matches!(code, KeyCode::ControlLeft | KeyCode::ControlRight) {
                        self.ctrl_held = event.state == ElementState::Pressed;
                    }
                    if event.state == ElementState::Pressed && !self.sleep_key(code) {
                        self.chat_key(code, event.text.as_deref());
                    }
                    return;
                }
                if self.menu.screen != crate::menu::Screen::None {
                    if event.state == ElementState::Pressed {
                        let ctrl = self.keys.contains(&KeyCode::ControlLeft) || self.ctrl_held;
                        let a = self.menu.key(&mut self.ui, code, event.text.as_deref(), ctrl, self.shift_held);
                        self.menu_action(a, el);
                    }
                    if matches!(code, KeyCode::ControlLeft | KeyCode::ControlRight) {
                        self.ctrl_held = event.state == ElementState::Pressed;
                    }
                    return;
                }
                if self.inv.is_open() {
                    if event.state == ElementState::Pressed {
                        self.sync_inv_binds();
                        let ctrl = self.keys.contains(&KeyCode::ControlLeft) || self.ctrl_held;
                        let (gw, gh) = self.gui_size();
                        let ev = KeyEvent { code, text: event.text.as_deref(), ctrl, shift: self.shift_held, repeat: event.repeat };
                        self.inv.key(&mut self.ui, &ev, gw, gh);
                        self.drain_inv();
                    }
                    if matches!(code, KeyCode::ControlLeft | KeyCode::ControlRight) {
                        self.ctrl_held = event.state == ElementState::Pressed;
                    }
                    return;
                }
                if event.state == ElementState::Pressed && code == KeyCode::Escape && !event.repeat {
                    if self.spawned {
                        self.menu.show(crate::menu::Screen::Pause);
                        self.set_grab(false);
                    }
                    return;
                }
                match event.state {
                    ElementState::Pressed => {
                        if self.keys.contains(&KeyCode::F3) && !event.repeat {
                            self.f3_combo(code);
                        }
                        if self.grabbed && self.spawned && !event.repeat {
                            if self.is_bind("key.pickItem", code) {
                                self.pick_presses += 1;
                            }
                        }
                        let chat = self.is_bind("key.chat", code);
                        let command = self.is_bind("key.command", code);
                        if self.spawned && !event.repeat && (chat || command) && self.menu.settings.chat_visibility != 2 {
                            self.hud.open_chat(if command && !chat { "/" } else { "" });
                            self.set_grab(false);
                            return;
                        } else if self.is_bind("key.inventory", code) && self.spawned && !event.repeat {
                            // PlayerControllerMP.isRidingHorse: montado em cavalo (com ou sem sela) manda o C0B
                            // OPEN_INVENTORY (sendHorseInventory) e o servidor abre a janela do cavalo.
                            let riding_horse = self.player.riding.is_some_and(|id| self.hud.track.is_horse(id));
                            match inventory_status(riding_horse) {
                                Some(action) => {
                                    self.send(serverbound::client_status(action));
                                    self.inv.env = self.inv_env();
                                    self.inv.open_own();
                                    self.set_grab(false);
                                }
                                None => self.send(serverbound::entity_action(self.entities.own_id.unwrap_or(0), 6, 0)),
                            }
                            return;
                        } else if self.is_bind("key.drop", code) && self.spawned && !event.repeat && self.hud.gamemode != 3 {
                            // Um aperto, um item (o jogo não recebe a repetição do sistema); Ctrl solta a pilha.
                            let status = if self.ctrl_held { 3 } else { 4 };
                            // EntityPlayerSP.dropOneItem: só o C07, sem balançar o braço.
                            self.send(serverbound::digging(status, (0, 0, 0), 0));
                        } else if code == KeyCode::F1 && !event.repeat {
                            self.hide_gui = !self.hide_gui;
                        } else if self.is_bind("key.togglePerspective", code) && !event.repeat {
                            // gameSettings.keyBindTogglePerspective: a tecla dos Controles (F5 por padrão), não o F5 fixo.
                            self.camera.cycle();
                        } else if code == KeyCode::F3 && !event.repeat {
                            // Minecraft.runTick (k == 61); o gráfico do profiler (Shift) não foi portado.
                            self.hud.show_debug = !self.hud.show_debug;
                            self.hud.lagometer = self.keys.contains(&KeyCode::AltLeft) || self.keys.contains(&KeyCode::AltRight);
                        } else if self.is_bind("key.playerlist", code) {
                            if self.stats.is_some() && !self.hud.show_tab {
                                println!("[debug] tab: {} jogadores na lista: {}", self.entities.players.len(), self.entities.players.values().map(|p| p.name.as_str()).collect::<Vec<_>>().join(" "));
                            }
                            self.hud.show_tab = true;
                        } else if let Some(n) = (1..=9).position(|i| self.is_bind(&format!("key.hotbar.{i}"), code)) {
                            self.select_slot(n as u8);
                        } else if self.is_bind("key.fullscreen", code) && !event.repeat {
                            // Minecraft.toggleFullscreen também grava a opção.
                            self.menu.settings.fullscreen ^= true;
                            self.menu.settings.save();
                        }
                        if self.grabbed {
                            self.keys.insert(code);
                        }
                    }
                    ElementState::Released => {
                        if self.is_bind("key.playerlist", code) {
                            self.hud.show_tab = false;
                        }
                        self.keys.remove(&code);
                    }
                }
            }
            WindowEvent::RedrawRequested => self.frame(el),
            _ => {}
        }
    }

    fn device_event(&mut self, _el: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
            if self.grabbed {
                // EntityRenderer.updateCameraAndRender: f = sens*0.6+0.2; f1 = f³*8; setAngles soma *0.15.
                let s = &self.menu.settings;
                let f = s.mouse_sensitivity * 0.6 + 0.2;
                let f1 = f * f * f * 8.0;
                let inv = if s.invert_mouse { -1.0 } else { 1.0 };
                // Entity.setAngles: conta em double e anda o prevRotation junto, então a câmera interpolada já
                // mostra o giro neste quadro (sem o atraso de até um tick).
                let (f2, f3) = (dx as f32 * f1, dy as f32 * f1 * inv);
                let (yaw0, pitch0) = (self.player.yaw, self.player.pitch);
                self.player.yaw = (self.player.yaw as f64 + f2 as f64 * 0.15) as f32;
                self.player.pitch = ((self.player.pitch as f64 + f3 as f64 * 0.15) as f32).clamp(-90.0, 90.0);
                self.player.prev_pitch += self.player.pitch - pitch0;
                self.player.prev_yaw += self.player.yaw - yaw0;
            }
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        self.apply_settings();
        let t = Instant::now();
        self.update(el);
        self.prof.add(0, t);
        self.print_stats();
        let now = Instant::now();
        if now >= self.next_frame {
            if let (Some(w), false) = (&self.window, self.occluded) {
                w.request_redraw();
            }
            // limitFramerate 260 = ilimitado; vsync fica com o Fifo.
            let fps = self.menu.settings.max_fps;
            let frame = if fps >= 260 { Duration::ZERO } else { Duration::from_nanos(1_000_000_000 / fps.max(10) as u64) };
            self.next_frame = now + frame;
        }
        // Acorda no próximo quadro ou no próximo tick, o que vier antes.
        let next_tick = now + Duration::from_secs_f64((TICK - self.tick_acc).max(0.0));
        el.set_control_flow(ControlFlow::WaitUntil(self.next_frame.min(next_tick)));
    }
}

/// `Minecraft.runTick`: a tecla do inventário manda C16 OPEN_INVENTORY_ACHIEVEMENT antes do `GuiInventory`.
/// Montado num cavalo o Java manda outro pacote (`sendHorseInventory`), então aqui não há C16.
fn inventory_status(riding_horse: bool) -> Option<i32> {
    (!riding_horse).then_some(serverbound::STATUS_OPEN_INVENTORY_ACHIEVEMENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_inventory_sends_client_status_2() {
        let action = inventory_status(false).expect("C16 fora do cavalo");
        assert_eq!(serverbound::client_status(action), vec![0x16, 0x02]);
    }

    #[test]
    fn riding_a_horse_sends_no_client_status() {
        assert_eq!(inventory_status(true), None);
    }
}

#[cfg(test)]
mod previsao_tests {
    use super::*;

    #[test]
    fn metadata_como_o_item_block() {
        // ItemCloth/ItemColored: o dano vira a cor; ItemBlock comum: 0.
        assert_eq!(place_meta(35, 14, 1), Some(14));
        assert_eq!(place_meta(159, 11, 1), Some(11));
        assert_eq!(place_meta(121, 5, 1), Some(0));
        // BlockLog.onBlockPlaced: eixo da face (Y 0, Z 8, X 4) com a madeira do dano.
        assert_eq!(place_meta(17, 2, 0), Some(2));
        assert_eq!(place_meta(17, 2, 3), Some(10));
        assert_eq!(place_meta(162, 1, 4), Some(5));
        // Escada, laje e tocha têm onBlockPlaced próprio: sem previsão.
        assert_eq!(place_meta(53, 0, 1), None);
        assert_eq!(place_meta(44, 0, 1), None);
        assert_eq!(place_meta(50, 0, 1), None);
    }

    #[test]
    fn substituiveis_como_o_material() {
        assert!(replaceable_clicked(31 << 4 | 1));
        assert!(replaceable_clicked(78 << 4));
        assert!(!replaceable_clicked(78 << 4 | 1), "neve com mais de uma camada não é substituída pelo clique");
        assert!(replaceable_material(78) && replaceable_material(9) && replaceable_material(0));
        assert!(!replaceable_material(1) && !replaceable_material(38));
    }
}
