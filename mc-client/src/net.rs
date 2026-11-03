//! Rede numa thread própria: login, keep-alive e tradução de pacotes em eventos.

use std::sync::mpsc;

use mc_net::Connection;
use mc_protocol::{serverbound, Reader};
use mc_world::{parse_chunk_bulk, parse_chunk_data, ChunkData};
use tokio::sync::mpsc as tmpsc;

use crate::auth::{self, Session};
use crate::chat;
use crate::entity_net::{self, EntityPacket};

pub type BoxErr = Box<dyn std::error::Error + Send + Sync>;

pub enum NetEvent {
    Status(String),
    Joined { dimension: i32 },
    /// S07: dimensão e o modo de jogo do jogador novo.
    Respawn { dimension: i32, gamemode: u8 },
    /// S20 `generic.movementSpeed`: base e modificadores (uuid, valor, operação) como o servidor mandou.
    SpeedAttribute { entity: i32, base: f64, modifiers: Vec<(u128, f64, u8)> },
    Chunk(ChunkData),
    Chunks(Vec<ChunkData>),
    BlockChange { x: i32, y: i32, z: i32, state: u16 },
    MultiBlockChange(Vec<(i32, i32, i32, u16)>),
    PosLook { x: f64, y: f64, z: f64, yaw: f32, pitch: f32, flags: u8 },
    Chat { text: String, comp: chat::Component, position: u8 },
    Health { health: f32, food: i32, saturation: f32 },
    /// 0x01 Join Game: bit 8 do byte de modo de jogo (`WorldInfo.isHardcoreModeEnabled`).
    Hardcore(bool),
    /// 0x1D (efeito aplicado) e 0x1E (removido) em qualquer entidade.
    PotionEffect { entity: i32, effect: crate::potion::Effect },
    RemoveEffect { entity: i32, id: u8 },
    /// 0x20: valor final de `generic.maxHealth` da entidade (`generic.maxHealth` em 0x20).
    MaxHealth { entity: i32, value: f32 },
    /// 0x3A Tab-Complete: sugestões do servidor.
    TabComplete(Vec<String>),
    /// 0x37 Statistics: nome da estatística e valor (conquistas começam com `achievement.`).
    Statistics(Vec<(String, i32)>),
    OpenWindow { id: u8, kind: String, title: String, size: u8, entity: i32 },
    CloseWindow(u8),
    SetSlot { window: i8, slot: i16, item: Option<mc_protocol::nbt::Slot> },
    WindowItems { window: u8, items: Vec<Option<mc_protocol::nbt::Slot>> },
    /// Transação recusada numa janela que não é o inventário.
    Transaction { window: u8, action: u16 },
    /// 0x03: hora do dia e idade do mundo (`getTotalWorldTime`).
    Time { time: i64, age: i64 },
    /// Spawn Position (0x05): onde a bússola aponta.
    SpawnPos { x: i32, z: i32 },
    /// S2B Change Game State: chuva (1, 2, 7) e trovão (8).
    Weather { reason: u8, value: f32 },
    GameMode(u8),
    HeldSlot(u8),
    Experience { bar: f32, level: i32 },
    /// 0x45: 0 título, 1 subtítulo, 2 tempos, 3 limpar, 4 resetar.
    Title { action: i32, text: String, times: (i32, i32, i32) },
    TabHeader { header: String, footer: String },
    /// 0x3B; `hearts` é o tipo de exibição (`EnumRenderType.HEARTS`).
    /// `mode`: 0 criar, 1 remover, 2 atualizar.
    Objective { name: String, mode: u8, display: String, hearts: bool },
    /// A conexão ficou criptografada no login (`NetworkManager.getIsencrypted`).
    Encrypted(bool),
    /// 0x34 `S34PacketMaps`.
    Map(crate::maps::MapPacket),
    /// Dificuldade do mundo (0x01, 0x07 e 0x41).
    Difficulty(u8),
    /// `reducedDebugInfo` do 0x01.
    ReducedDebug(bool),
    Score { player: String, objective: String, value: Option<i32> },
    DisplayObjective { position: u8, name: String },
    /// 0x3E: modo 0 criar, 1 remover, 2 atualizar, 3 adicionar, 4 tirar jogadores.
    /// `prefix`: (prefixo, sufixo, visibilidade do nametag: 0 always, 1 never, 2 hideForOtherTeams, 3 hideForOwnTeam,
    /// cor: `EnumChatFormatting.func_175744_a`, None para RESET ou índice fora das 16 cores).
    Team { name: String, mode: u8, prefix: Option<(String, String, u8, Option<u8>)>, players: Vec<String> },
    /// S0D: item/orbe pego (`collected`) por `collector`.
    Collect { collected: i32, collector: i32 },
    /// S25: rachadura que outra entidade está fazendo num bloco (0..9; fora disso some).
    BreakAnim { entity: i32, pos: (i32, i32, i32), stage: i8 },
    /// S2B motivo 6: a flecha do jogador acertou outro jogador.
    ArrowHit,
    Abilities { flags: u8, fly_speed: f32, walk_speed: f32 },
    /// Entidades e lista de jogadores (`entity_net.rs`).
    Entity(EntityPacket),
    /// 0x29 Named Sound Effect e os sons do 0x28 Effect.
    Sound(crate::sound::SoundEffect),
    /// Tile entities: 0x35, 0x33 e 0x24 (`tile_entities.rs`).
    Tile(crate::tile_entities::TileEvent),
    /// 0x2A Particles.
    Particles(crate::particles::ParticlePacket),
    /// 0x27 Explosion.
    Explosion(crate::particles::ExplosionPacket),
    WorldBorder(crate::border::BorderPacket),
    GlobalSound { kind: i32, pos: (i32, i32, i32) },
    /// S43PacketCamera: a entidade de onde a câmera olha (`setRenderViewEntity`).
    Camera(i32),
    /// 0x28 Effect visual (2000 a 2005).
    Effect { kind: i32, pos: (i32, i32, i32), data: i32 },
    /// 0x48: URL e hash SHA-1 do pacote de recursos do servidor.
    ResourcePack { url: String, hash: String },
    Container(crate::container_net::ContainerEvent),
    Disconnected(String),
}

pub struct Net {
    pub events: mpsc::Receiver<NetEvent>,
    pub out: tmpsc::UnboundedSender<Vec<u8>>,
}

/// Endereço -> (host para conectar, porta). Sem porta explícita, tenta o SRV `_minecraft._tcp`.
pub async fn resolve_addr(addr: &str) -> (String, u16) {
    if let Some((h, p)) = addr.rsplit_once(':') {
        return (h.to_string(), p.parse().unwrap_or(25565));
    }
    if let Ok(r) = hickory_resolver::TokioAsyncResolver::tokio_from_system_conf() {
        if let Ok(srv) = r.srv_lookup(format!("_minecraft._tcp.{addr}.")).await {
            if let Some(rec) = srv.iter().next() {
                let host = rec.target().to_utf8().trim_end_matches('.').to_string();
                return (host, rec.port());
            }
        }
    }
    (addr.to_string(), 25565)
}

/// Ping de status (lista de servidores): JSON do servidor e latência em ms.
pub async fn status(addr: &str) -> Result<(String, u64), BoxErr> {
    let (host, port) = resolve_addr(addr).await;
    let mut c = Connection::connect((host.as_str(), port)).await?;
    c.send(&serverbound::handshake(&host, port, 1)).await?;
    c.send(&serverbound::status_request()).await?;
    let json = Reader::new(&c.recv().await?.body).string()?;
    let t0 = std::time::Instant::now();
    c.send(&serverbound::status_ping(42)).await?;
    c.recv().await?;
    Ok((json, t0.elapsed().as_millis() as u64))
}

/// Conecta e faz o login (offline ou online, conforme o servidor pedir).
pub async fn login(addr: &str, name: &str, session: Option<&Session>, log: &dyn Fn(String)) -> Result<Connection, BoxErr> {
    let (host, port) = resolve_addr(addr).await;
    let mut c = Connection::connect((host.as_str(), port)).await?;
    c.send(&serverbound::handshake(&host, port, 2)).await?;
    let name = session.map_or(name, |s| s.name.as_str());
    c.send(&serverbound::login_start(name)).await?;

    // Login: 0x00 disconnect, 0x01 encryption request, 0x02 success, 0x03 set compression.
    loop {
        let p = c.recv().await?;
        match p.id {
            0x00 => return Err(format!("desconectado: {}", chat::to_plain(&Reader::new(&p.body).string()?)).into()),
            0x01 => {
                let session = session.ok_or("servidor em online-mode: use --session <auth.json>")?;
                let reply = auth::answer_encryption_request(&p.body)?;
                let (s, h) = (session.clone(), reply.hash.clone());
                tokio::task::spawn_blocking(move || auth::session_join(&s, &h)).await??;
                c.send(&reply.packet).await?;
                c.enable_encryption(&reply.secret);
                log("criptografia ligada".into());
            }
            0x03 => {
                let t = Reader::new(&p.body).varint()?;
                c.set_compression(t);
            }
            0x02 => {
                let mut r = Reader::new(&p.body);
                let (uuid, name) = (r.string()?, r.string()?);
                log(format!("login ok: {name} ({uuid})"));
                return Ok(c);
            }
            id => return Err(format!("pacote de login inesperado 0x{id:02x}").into()),
        }
    }
}

pub fn start(addr: String, name: String, session: Option<Session>, view_distance: u8) -> Net {
    let (ev_tx, ev_rx) = mpsc::channel();
    let (out_tx, out_rx) = tmpsc::unbounded_channel();
    let out = out_tx.clone();
    std::thread::Builder::new()
        .name("rede".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().expect("tokio");
            let res = rt.block_on(run(addr, name, session, view_distance, ev_tx.clone(), out, out_rx));
            let reason = match res {
                Ok(r) => r,
                Err(e) => e.to_string(),
            };
            let _ = ev_tx.send(NetEvent::Disconnected(reason));
        })
        .expect("thread de rede");
    Net { events: ev_rx, out: out_tx }
}

async fn run(
    addr: String,
    name: String,
    session: Option<Session>,
    _view_distance: u8,
    ev: mpsc::Sender<NetEvent>,
    out: tmpsc::UnboundedSender<Vec<u8>>,
    mut out_rx: tmpsc::UnboundedReceiver<Vec<u8>>,
) -> Result<String, BoxErr> {
    let log = |s: String| {
        let _ = ev.send(NetEvent::Status(s));
    };
    let conn = login(&addr, &name, session.as_ref(), &log).await?;
    let _ = ev.send(NetEvent::Encrypted(conn.encrypted()));
    let (mut reader, mut writer) = conn.into_split();
    tokio::spawn(async move {
        while let Some(p) = out_rx.recv().await {
            if writer.send(&p).await.is_err() {
                break;
            }
        }
    });

    let mut transactions_seen = false;
    loop {
        let p = reader.recv().await?;
        let mut r = Reader::new(&p.body);
        let event = match p.id {
            0x00 => {
                let _ = out.send(serverbound::keep_alive(r.varint()?));
                None
            }
            0x01 => {
                let entity_id = r.i32()?;
                let _ = ev.send(NetEvent::Entity(EntityPacket::JoinGame { entity_id }));
                let raw_mode = r.u8()?;
                let gamemode = raw_mode & 7;
                let _ = ev.send(NetEvent::Hardcore(raw_mode & 8 != 0));
                let _ = ev.send(NetEvent::GameMode(gamemode));
                let dimension = r.u8()? as i8 as i32;
                let _ = ev.send(NetEvent::Difficulty(r.u8()?));
                let (_max_players, _level_type) = (r.u8()?, r.string()?);
                let reduced = r.bool()?;
                // O jogador novo nasce no Joined (que manda o C15 e o MC|Brand, como o handleJoinGame); o
                // reducedDebugInfo vai nele depois.
                let _ = ev.send(NetEvent::Joined { dimension });
                Some(NetEvent::ReducedDebug(reduced))
            }
            0x02 => {
                let json = r.string()?;
                let comp = chat::Component::from_json(&json);
                Some(NetEvent::Chat { text: chat::legacy(&comp), comp, position: r.u8().unwrap_or(0) })
            }
            0x03 => {
                let age = r.i64()?;
                Some(NetEvent::Time { time: r.i64()?, age })
            }
            0x05 => {
                let (x, _y, z) = r.position()?;
                Some(NetEvent::SpawnPos { x, z })
            }
            0x06 => {
                let health = r.f32()?;
                let food = r.varint()?;
                Some(NetEvent::Health { health, food, saturation: r.f32()? })
            }
            0x09 => Some(NetEvent::HeldSlot(r.u8()?)),
            0x1D => {
                let (entity, effect) = crate::potion::parse_add(&p.body)?;
                Some(NetEvent::PotionEffect { entity, effect })
            }
            0x1E => {
                let (entity, id) = crate::potion::parse_remove(&p.body)?;
                Some(NetEvent::RemoveEffect { entity, id })
            }
            0x20 => {
                if let Some((entity, base, modifiers)) = crate::hud_track::parse_speed(&p.body)? {
                    let _ = ev.send(NetEvent::SpeedAttribute { entity, base, modifiers });
                }
                crate::hud_track::parse_max_health(&p.body)?.map(|(entity, value)| NetEvent::MaxHealth { entity, value })
            }
            0x37 => {
                let mut v = Vec::new();
                for _ in 0..r.varint()?.clamp(0, 4096) {
                    let name = r.string()?;
                    v.push((name, r.varint()?));
                }
                Some(NetEvent::Statistics(v))
            }
            0x3A => {
                let mut v = Vec::new();
                for _ in 0..r.varint()?.clamp(0, 4096) {
                    v.push(r.string()?);
                }
                Some(NetEvent::TabComplete(v))
            }
            0x1F => {
                let bar = r.f32()?;
                Some(NetEvent::Experience { bar, level: r.varint()? })
            }
            0x2B => {
                let reason = r.u8()?;
                let value = r.f32()?;
                match reason {
                    3 => Some(NetEvent::GameMode(value as u8)),
                    6 => Some(NetEvent::ArrowHit),
                    1 | 2 | 7 | 8 => Some(NetEvent::Weather { reason, value }),
                    _ => None,
                }
            }
            0x0D => Some(NetEvent::Collect { collected: r.varint()?, collector: r.varint()? }),
            0x25 => {
                let entity = r.varint()?;
                let pos = r.position()?;
                Some(NetEvent::BreakAnim { entity, pos, stage: r.u8()? as i8 })
            }
            0x34 => {
                use crate::entity_net::ReadExt;
                let id = r.varint()?;
                let scale = r.i8()?;
                let n = r.varint()?.max(0) as usize;
                let mut icons = Vec::with_capacity(n);
                for _ in 0..n {
                    let b = r.i8()? as i16;
                    icons.push([(b >> 4 & 15) as i8, r.i8()?, r.i8()?, (b & 15) as i8]);
                }
                let cols = r.u8()?;
                let patch = if cols > 0 {
                    let (rows, x, z) = (r.u8()?, r.u8()?, r.u8()?);
                    let len = r.varint()?.max(0) as usize;
                    Some((cols, rows, x, z, r.bytes(len)?.to_vec()))
                } else {
                    None
                };
                Some(NetEvent::Map(crate::maps::MapPacket { id, scale, icons, patch }))
            }
            0x3B => {
                let name = r.string()?;
                let mode = r.u8()?;
                let (display, hearts) = if mode == 0 || mode == 2 { (r.string()?, r.string()? == "hearts") } else { (String::new(), false) };
                Some(NetEvent::Objective { name, mode, display, hearts })
            }
            0x3C => {
                let player = r.string()?;
                let action = r.u8()?;
                let objective = r.string()?;
                let value = if action == 0 { Some(r.varint()?) } else { None };
                Some(NetEvent::Score { player, objective, value })
            }
            0x3D => {
                let position = r.u8()?;
                Some(NetEvent::DisplayObjective { position, name: r.string()? })
            }
            0x3E => {
                let name = r.string()?;
                let mode = r.u8()?;
                let mut prefix = None;
                if mode == 0 || mode == 2 {
                    let _display = r.string()?;
                    let p = r.string()?;
                    let s = r.string()?;
                    let _friendly = r.u8()?;
                    let vis = match r.string()?.as_str() {
                        "never" => 1,
                        "hideForOtherTeams" => 2,
                        "hideForOwnTeam" => 3,
                        _ => 0,
                    };
                    let color = r.u8()? as i8;
                    prefix = Some((p, s, vis, (0..16).contains(&color).then_some(color as u8)));
                }
                let mut players = Vec::new();
                if mode == 0 || mode == 3 || mode == 4 {
                    for _ in 0..r.varint()?.max(0) {
                        players.push(r.string()?);
                    }
                }
                Some(NetEvent::Team { name, mode, prefix, players })
            }
            0x43 => Some(NetEvent::Camera(r.varint()?)),
            // S44PacketWorldBorder.readPacketData.
            0x44 => {
                use crate::border::BorderPacket as B;
                let varlong = |r: &mut mc_protocol::Reader| -> mc_protocol::Result<i64> {
                    let mut v: i64 = 0;
                    for i in 0..10 {
                        let b = r.u8()?;
                        v |= ((b & 0x7F) as i64) << (7 * i);
                        if b & 0x80 == 0 {
                            break;
                        }
                    }
                    Ok(v)
                };
                let p = match r.varint()? {
                    0 => B::SetSize(r.f64()?),
                    1 => B::LerpSize { from: r.f64()?, to: r.f64()?, ms: varlong(&mut r)? },
                    2 => B::SetCenter(r.f64()?, r.f64()?),
                    3 => B::Initialize { x: r.f64()?, z: r.f64()?, from: r.f64()?, to: r.f64()?, ms: varlong(&mut r)?, size: r.varint()?, warning_distance: r.varint()?, warning_time: r.varint()? },
                    4 => B::WarningTime(r.varint()?),
                    _ => B::WarningBlocks(r.varint()?),
                };
                Some(NetEvent::WorldBorder(p))
            }
            0x45 => {
                let action = r.varint()?;
                let mut text = String::new();
                let mut times = (0, 0, 0);
                match action {
                    0 | 1 => text = chat::to_legacy(&r.string()?),
                    2 => times = (r.i32()?, r.i32()?, r.i32()?),
                    _ => {}
                }
                Some(NetEvent::Title { action, text, times })
            }
            0x47 => {
                let header = chat::to_legacy(&r.string()?);
                Some(NetEvent::TabHeader { header, footer: chat::to_legacy(&r.string()?) })
            }
            0x07 => {
                let dimension = r.i32()?;
                let _ = ev.send(NetEvent::Difficulty(r.u8()?));
                let gamemode = r.u8()? & 7;
                Some(NetEvent::Respawn { dimension, gamemode })
            }
            0x41 => Some(NetEvent::Difficulty(r.u8()?)),
            0x08 => Some(NetEvent::PosLook {
                x: r.f64()?,
                y: r.f64()?,
                z: r.f64()?,
                yaw: r.f32()?,
                pitch: r.f32()?,
                flags: r.u8()?,
            }),
            0x21 => match parse_chunk_data(&p.body) {
                Ok(c) => Some(NetEvent::Chunk(c)),
                Err(e) => Some(NetEvent::Status(format!("chunk inválido: {e}"))),
            },
            0x22 => {
                let (cx, cz) = (r.i32()?, r.i32()?);
                let n = r.varint()?.max(0);
                let mut v = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    let xz = r.u8()? as i32;
                    let y = r.u8()? as i32;
                    let state = r.varint()? as u16;
                    v.push((cx * 16 + (xz >> 4), y, cz * 16 + (xz & 15), state));
                }
                Some(NetEvent::MultiBlockChange(v))
            }
            0x23 => {
                let (x, y, z) = r.position()?;
                Some(NetEvent::BlockChange { x, y, z, state: r.varint()? as u16 })
            }
            0x26 => match parse_chunk_bulk(&p.body) {
                Ok(c) => Some(NetEvent::Chunks(c)),
                Err(e) => Some(NetEvent::Status(format!("chunk bulk inválido: {e}"))),
            },
            id if crate::container_net::handles(id) => match crate::container_net::parse(id, &p.body) {
                Ok(e) => e.map(NetEvent::Container),
                Err(e) => Some(NetEvent::Status(format!("pacote 0x{id:02x} inválido: {e}"))),
            },
            id if crate::tile_entities::handles(id) => match crate::tile_entities::parse_packet(id, &p.body) {
                Ok(t) => Some(NetEvent::Tile(t)),
                Err(e) => Some(NetEvent::Status(format!("pacote 0x{id:02x} inválido: {e}"))),
            },
            0x28 => {
                let (kind, pos, data) = (r.i32()?, r.position()?, r.i32()?);
                // isSoundServerwide: RenderGlobal.broadcastSound (só o wither e o fim do dragão tocam).
                if r.bool()? {
                    matches!(kind, 1013 | 1018).then_some(NetEvent::GlobalSound { kind, pos })
                } else {
                    if (2000..=2005).contains(&kind) {
                        let _ = ev.send(NetEvent::Effect { kind, pos, data });
                    }
                    crate::sound::parse_effect(&p.body)?.map(NetEvent::Sound)
                }
            }
            0x2A => Some(match crate::particles::parse_particles(&p.body) {
                Ok(p) => NetEvent::Particles(p),
                Err(e) => NetEvent::Status(format!("partículas inválidas: {e}")),
            }),
            0x27 => match crate::particles::parse_explosion(&p.body) {
                Ok(e) => {
                    let pitch = (1.0 + (rand::random::<f32>() - rand::random::<f32>()) * 0.2) * 0.7;
                    let _ = ev.send(NetEvent::Sound(crate::sound::SoundEffect { name: "random.explode".into(), pos: e.pos, volume: 4.0, pitch }));
                    Some(NetEvent::Explosion(e))
                }
                Err(e) => Some(NetEvent::Status(format!("explosão inválida: {e}"))),
            },
            0x29 => Some(NetEvent::Sound(crate::sound::parse_named_sound(&p.body)?)),
            0x32 => {
                // Como o cliente original: responde quando a janela é o inventário (0) e veio recusado.
                let (window, action, accepted) = (r.u8()?, r.u16()?, r.bool()?);
                // handleConfirmTransaction é enfileirado na thread principal: a resposta da janela 0 sai na ordem
                // dos pacotes (depois do S12/S08 que veio antes), como os anticheats de transação esperam.
                if window == 0 && !accepted && !transactions_seen {
                    transactions_seen = true;
                    let _ = ev.send(NetEvent::Status("servidor mede o lag por transações; respondendo".into()));
                }
                if !accepted {
                    Some(NetEvent::Transaction { window, action })
                } else {
                    None
                }
            }
            0x2D => {
                let id = r.u8()?;
                let kind = r.string()?;
                let title = chat::to_legacy(&r.string()?);
                let size = r.u8()?;
                let entity = if kind == "EntityHorse" { r.i32()? } else { 0 };
                Some(NetEvent::OpenWindow { id, kind, title, size, entity })
            }
            0x2E => Some(NetEvent::CloseWindow(r.u8()?)),
            0x2F => {
                let window = r.u8()? as i8;
                let slot = r.u16()? as i16;
                Some(NetEvent::SetSlot { window, slot, item: r.slot()? })
            }
            0x30 => {
                let window = r.u8()?;
                let n = r.u16()? as usize;
                let mut items = Vec::with_capacity(n);
                for _ in 0..n {
                    items.push(r.slot()?);
                }
                Some(NetEvent::WindowItems { window, items })
            }
            0x39 => Some(NetEvent::Abilities { flags: r.u8()?, fly_speed: r.f32()?, walk_speed: r.f32()? }),
            0x40 => return Ok(chat::to_legacy(&r.string()?)),
            0x46 => {
                reader.set_compression(r.varint()?);
                None
            }
            // S48PacketResourcePackSend: o jogo decide (modo do servidor, pergunta, download) e responde com o C19.
            0x48 => Some(NetEvent::ResourcePack { url: r.string()?, hash: r.string()? }),
            id if entity_net::handles(id) => Some(match entity_net::parse(id, &p.body) {
                Ok(e) => NetEvent::Entity(e),
                Err(e) => NetEvent::Status(format!("pacote de entidade 0x{id:02x} inválido: {e}")),
            }),
            _ => None,
        };
        if let Some(e) = event {
            if ev.send(e).is_err() {
                return Ok("janela fechada".into());
            }
        }
    }
}

