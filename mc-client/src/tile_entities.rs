//! Tile entities do cliente: pacotes 0x35 (`S35PacketUpdateTileEntity`), 0x33
//! (`S33PacketUpdateSign`) e 0x24 (`S24PacketBlockAction`), guarda dos dados por posição
//! (somem com o chunk ou com o bloco) e desenho como o `TileEntityRendererDispatcher`:
//! baús (`TileEntityChestRenderer`, `TileEntityEnderChestRenderer`), placas
//! (`TileEntitySignRenderer`), cabeças (`TileEntitySkullRenderer`) e estandartes
//! (`TileEntityBannerRenderer`). O JSON de modelo desses blocos é vazio no jogo: o desenho
//! vem só daqui.

use std::collections::HashMap;
use std::f32::consts::PI;

use glam::{DVec3, Mat4, Vec3};
use mc_protocol::nbt::{self, Nbt};
use mc_protocol::{Reader, Result};
use mc_render::ui::UiBatch;
use mc_render::{Camera, DrawItem, Jar, Renderer, Tex};
use mc_world::mesh::{Geometry, Vertex};
use mc_world::World;

use crate::banner::BannerData;
use crate::chat;
use crate::entities::light_at;
use crate::entity_net::{ProfileProperty, Uuid};
use crate::models::{diffuse, Part, Sink};
use crate::skins::{default_skin, skin_from_properties, SkinRef, Textures};
use crate::sound::SoundEffect;

pub type Pos = (i32, i32, i32);

/// Ids de bloco com renderer de tile entity.
pub const CHEST: u16 = 54;
pub const STANDING_SIGN: u16 = 63;
pub const WALL_SIGN: u16 = 68;
pub const NOTE_BLOCK: u16 = 25;
pub const ENDER_CHEST: u16 = 130;
pub const FLOWER_POT: u16 = 140;
pub const SKULL: u16 = 144;
pub const TRAPPED_CHEST: u16 = 146;
pub const STANDING_BANNER: u16 = 176;
pub const WALL_BANNER: u16 = 177;
pub const ENCHANTING_TABLE: u16 = 116;

/// Placa em pé ou na parede.
pub fn is_sign(id: u16) -> bool {
    matches!(id, STANDING_SIGN | WALL_SIGN)
}

/// `Block.hasTileEntity` para os blocos que este módulo guarda.
pub fn is_tile_block(id: u16) -> bool {
    matches!(id, CHEST | STANDING_SIGN | WALL_SIGN | ENDER_CHEST | FLOWER_POT | SKULL | TRAPPED_CHEST | STANDING_BANNER | WALL_BANNER | ENCHANTING_TABLE)
}

/// Distância máxima de desenho (`TileEntity.getMaxRenderDistanceSquared`).
const MAX_RENDER_DISTANCE_SQ: f32 = 4096.0;

// ---------------------------------------------------------------------------------------------
// Pacotes
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum TileEvent {
    /// 0x35: tipo 4 cabeça, 5 vaso, 6 estandarte (os demais não têm renderer aqui).
    Update { pos: Pos, action: u8, nbt: Option<Nbt> },
    /// 0x33: as 4 linhas como JSON de chat.
    Sign { pos: Pos, lines: [String; 4] },
    /// 0x24: bloco, evento e parâmetro.
    Action { pos: Pos, event: u8, param: u8, block: u16 },
}

/// Pacotes que este módulo trata.
pub fn handles(id: i32) -> bool {
    matches!(id, 0x24 | 0x33 | 0x35)
}

pub fn parse_packet(id: i32, body: &[u8]) -> Result<TileEvent> {
    let mut r = Reader::new(body);
    Ok(match id {
        0x35 => {
            let pos = r.position()?;
            let action = r.u8()?;
            TileEvent::Update { pos, action, nbt: nbt::read_root(&mut r)? }
        }
        0x33 => {
            let pos = r.position()?;
            TileEvent::Sign { pos, lines: [r.string()?, r.string()?, r.string()?, r.string()?] }
        }
        _ => {
            let pos = r.position()?;
            let (event, param) = (r.u8()?, r.u8()?);
            TileEvent::Action { pos, event, param, block: (r.varint()? & 4095) as u16 }
        }
    })
}

// ---------------------------------------------------------------------------------------------
// Dados
// ---------------------------------------------------------------------------------------------

/// Dono da cabeça (`Owner`): perfil com a skin das propriedades, se houver.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Owner {
    pub id: Option<Uuid>,
    pub name: Option<String>,
    pub skin: Option<SkinRef>,
}

#[derive(Clone, Debug, Default)]
pub struct Tile {
    // Baús e baú do ender (`TileEntityChest`, `TileEntityEnderChest`).
    pub lid: f32,
    pub prev_lid: f32,
    pub players_using: i32,
    // Placa: linhas já com códigos de formatação.
    pub text: [String; 4],
    // Cabeça.
    pub skull_type: u8,
    pub skull_rot: u8,
    pub owner: Option<Owner>,
    // Estandarte.
    pub banner: BannerData,
    // Vaso de planta (`Item` e `Data`).
    pub pot_item: Option<String>,
    pub pot_data: i32,
    // Mesa de encantamento (`TileEntityEnchantmentTable`): livro.
    pub book: TableBook,
}

/// Campos do livro da `TileEntityEnchantmentTable` (`field_145932_k` é o alvo da folha, `field_145929_l` a
/// velocidade e `field_145924_q` o ângulo para o jogador).
#[derive(Clone, Debug, Default)]
pub struct TableBook {
    pub tick_count: i32,
    pub page_flip: f32,
    pub page_flip_prev: f32,
    pub flip_target: f32,
    pub flip_speed: f32,
    pub spread: f32,
    pub spread_prev: f32,
    pub rotation: f32,
    pub rotation_prev: f32,
    pub target_rotation: f32,
}

/// Leva o ângulo para [-π, π) com os laços do Java.
fn wrap_pi(mut a: f32) -> f32 {
    while a >= std::f32::consts::PI {
        a -= std::f32::consts::TAU;
    }
    while a < -std::f32::consts::PI {
        a += std::f32::consts::TAU;
    }
    a
}

impl TableBook {
    /// `TileEntityEnchantmentTable.update`; `closest` é o jogador mais perto a até 3 blocos do centro.
    pub fn tick(&mut self, centre: DVec3, closest: Option<DVec3>, rand: &mut crate::enchant::JavaRandom) {
        self.spread_prev = self.spread;
        self.rotation_prev = self.rotation;
        match closest {
            Some(p) => {
                self.target_rotation = (p.z - centre.z).atan2(p.x - centre.x) as f32;
                self.spread += 0.1;
                if self.spread < 0.5 || rand.next_int(40) == 0 {
                    let before = self.flip_target;
                    loop {
                        self.flip_target += (rand.next_int(4) - rand.next_int(4)) as f32;
                        if before != self.flip_target {
                            break;
                        }
                    }
                }
            }
            None => {
                self.target_rotation += 0.02;
                self.spread -= 0.1;
            }
        }
        self.rotation = wrap_pi(self.rotation);
        self.target_rotation = wrap_pi(self.target_rotation);
        let d = wrap_pi(self.target_rotation - self.rotation);
        self.rotation += d * 0.4;
        self.spread = self.spread.clamp(0.0, 1.0);
        self.tick_count += 1;
        self.page_flip_prev = self.page_flip;
        let f = ((self.flip_target - self.page_flip) * 0.4).clamp(-0.2, 0.2);
        self.flip_speed += (f - self.flip_speed) * 0.9;
        self.page_flip += self.flip_speed;
    }

    /// `TileEntityEnchantmentTableRenderer.renderTileEntityAt`: matriz e argumentos do `ModelBook.render`
    /// (tempo, folha direita, folha esquerda, abertura).
    pub fn pose(&self, p: Pos, partial: f32) -> (Mat4, [f32; 4]) {
        let time = self.tick_count as f32 + partial;
        let d = wrap_pi(self.rotation - self.rotation_prev);
        let rot = self.rotation_prev + d * partial;
        let m = Mat4::from_translation(Vec3::new(p.0 as f32 + 0.5, p.1 as f32 + 0.75, p.2 as f32 + 0.5))
            * Mat4::from_translation(Vec3::new(0.0, 0.1 + crate::mth::sin(time * 0.1) * 0.01, 0.0))
            * Mat4::from_rotation_y(-rot)
            * Mat4::from_rotation_z(80f32.to_radians());
        let page = |o: f32| {
            let v = self.page_flip_prev + (self.page_flip - self.page_flip_prev) * partial + o;
            ((v - v.trunc()) * 1.6 - 0.3).clamp(0.0, 1.0)
        };
        let spread = self.spread_prev + (self.spread - self.spread_prev) * partial;
        (m, [time, page(0.25), page(0.75), spread])
    }
}

fn parse_uuid(s: &str) -> Option<Uuid> {
    let hex: String = s.chars().filter(|c| *c != '-').collect();
    (hex.len() == 32).then(|| u128::from_str_radix(&hex, 16).ok()).flatten().map(Uuid)
}

fn byte(n: &Nbt, key: &str) -> u8 {
    match n.get(key) {
        Some(Nbt::Byte(v)) => *v as u8,
        Some(Nbt::Short(v)) => *v as u8,
        Some(Nbt::Int(v)) => *v as u8,
        _ => 0,
    }
}

fn int(n: &Nbt, key: &str) -> i32 {
    match n.get(key) {
        Some(Nbt::Byte(v)) => *v as i32,
        Some(Nbt::Short(v)) => *v as i32,
        Some(Nbt::Int(v)) => *v,
        _ => 0,
    }
}

/// `NBTUtil.readGameProfileFromNBT`: nome, id e propriedades (a skin vem de `textures`).
fn read_owner(n: &Nbt) -> Owner {
    let name = n.get("Name").and_then(Nbt::as_str).filter(|s| !s.is_empty()).map(str::to_string);
    let id = n.get("Id").and_then(Nbt::as_str).and_then(parse_uuid);
    let mut props = Vec::new();
    if let Some(Nbt::Compound(p)) = n.get("Properties") {
        for (key, list) in p {
            if let Nbt::List(l) = list {
                for e in l {
                    let value = e.get("Value").and_then(Nbt::as_str).unwrap_or_default().to_string();
                    let signature = e.get("Signature").and_then(Nbt::as_str).map(str::to_string);
                    props.push(ProfileProperty { name: key.clone(), value, signature });
                }
            }
        }
    }
    Owner { id, name, skin: skin_from_properties(&props) }
}

impl Tile {
    fn read_skull(&mut self, n: &Nbt) {
        self.skull_type = byte(n, "SkullType");
        self.skull_rot = byte(n, "Rot");
        self.owner = None;
        if self.skull_type == 3 {
            if let Some(o @ Nbt::Compound(_)) = n.get("Owner") {
                self.owner = Some(read_owner(o));
            } else if let Some(extra) = n.get("ExtraType").and_then(Nbt::as_str).filter(|s| !s.is_empty()) {
                self.owner = Some(Owner { name: Some(extra.to_string()), ..Default::default() });
            }
        }
    }

    fn read_pot(&mut self, n: &Nbt) {
        self.pot_item = match n.get("Item") {
            Some(Nbt::String(s)) => Some(s.clone()),
            Some(Nbt::Int(i)) => Some(i.to_string()),
            _ => None,
        };
        self.pot_data = int(n, "Data");
    }

    /// Ângulo da tampa suavizado (a fórmula comum do baú e do baú do ender).
    pub fn lid_angle(&self, partial: f32) -> f32 {
        let a = self.prev_lid + (self.lid - self.prev_lid) * partial;
        let a = 1.0 - a;
        -((1.0 - a * a * a) * PI / 2.0)
    }

    /// `update()` do baú e do baú do ender: a tampa anda 0,1 por tick. `centre` é onde o som toca;
    /// `silent_close` é verdadeiro no lado que não toca som (metade de baú duplo).
    fn tick_lid(&mut self, centre: DVec3, quiet: bool, sounds: &mut Vec<SoundEffect>) {
        self.prev_lid = self.lid;
        let step = 0.1;
        let mut play = |name: &str| {
            if !quiet {
                sounds.push(SoundEffect { name: name.into(), pos: centre, volume: 0.5, pitch: rand::random::<f32>() * 0.1 + 0.9 });
            }
        };
        if self.players_using > 0 && self.lid == 0.0 {
            play("random.chestopen");
        }
        if (self.players_using == 0 && self.lid > 0.0) || (self.players_using > 0 && self.lid < 1.0) {
            let before = self.lid;
            self.lid += if self.players_using > 0 { step } else { -step };
            if self.lid > 1.0 {
                self.lid = 1.0;
            }
            if self.lid < 0.5 && before >= 0.5 {
                play("random.chestclosed");
            }
            if self.lid < 0.0 {
                self.lid = 0.0;
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Armazenamento
// ---------------------------------------------------------------------------------------------

#[derive(Default)]
pub struct TileEntities {
    pub tiles: HashMap<Pos, Tile>,
    /// Ticks de mundo (`getTotalWorldTime` para o balanço do estandarte).
    pub world_time: i64,
    /// O `rand` estático da `TileEntityEnchantmentTable`.
    book_rand: Option<crate::enchant::JavaRandom>,
}

impl TileEntities {
    pub fn clear(&mut self) {
        self.tiles.clear();
    }

    /// Chunk novo ou reenviado: cria as tile entities dos blocos que têm uma (como o `getTileEntity`
    /// sob demanda do cliente) e descarta as que não têm mais bloco.
    pub fn scan_column(&mut self, world: &World, cx: i32, cz: i32) {
        let mut found: Vec<Pos> = Vec::new();
        if let Some(col) = world.columns.get(&(cx, cz)) {
            for (sy, s) in col.sections.iter().enumerate() {
                let Some(s) = s else { continue };
                for (i, st) in s.blocks.iter().enumerate() {
                    if is_tile_block(st >> 4) {
                        let (x, y, z) = ((i & 15) as i32, (i >> 8) as i32, ((i >> 4) & 15) as i32);
                        found.push((cx * 16 + x, sy as i32 * 16 + y, cz * 16 + z));
                    }
                }
            }
        }
        self.tiles.retain(|p, _| (p.0 >> 4, p.2 >> 4) != (cx, cz) || found.contains(p));
        for p in found {
            self.tiles.entry(p).or_default();
        }
    }

    /// Coluna descarregada: as tile entities somem junto.
    pub fn unload_column(&mut self, cx: i32, cz: i32) {
        self.tiles.retain(|p, _| (p.0 >> 4, p.2 >> 4) != (cx, cz));
    }

    /// `Chunk.setBlockState`: trocar o bloco por outro id remove a tile entity; um bloco com
    /// tile entity cria a dele.
    pub fn block_changed(&mut self, x: i32, y: i32, z: i32, state: u16) {
        let id = state >> 4;
        match self.tiles.get(&(x, y, z)) {
            Some(_) if is_tile_block(id) => {}
            Some(_) => {
                self.tiles.remove(&(x, y, z));
            }
            None if is_tile_block(id) => {
                self.tiles.insert((x, y, z), Tile::default());
            }
            None => {}
        }
    }

    /// Aplica um pacote. Sons gerados (nota) vão para `sounds`.
    pub fn handle(&mut self, ev: TileEvent, world: &World, sounds: &mut Vec<SoundEffect>) {
        match ev {
            TileEvent::Update { pos, action, nbt } => {
                let id = world.block(pos.0, pos.1, pos.2) >> 4;
                let (Some(t), Some(n)) = (self.tiles.get_mut(&pos), nbt) else { return };
                match (action, id) {
                    (4, SKULL) => t.read_skull(&n),
                    (5, FLOWER_POT) => t.read_pot(&n),
                    (6, STANDING_BANNER | WALL_BANNER) => t.banner = BannerData::from_nbt(&n),
                    _ => {}
                }
            }
            TileEvent::Sign { pos, lines } => {
                if let Some(t) = self.tiles.get_mut(&pos) {
                    if matches!(world.block(pos.0, pos.1, pos.2) >> 4, STANDING_SIGN | WALL_SIGN) {
                        t.text = lines.map(|l| chat::to_legacy(&l));
                    }
                }
            }
            TileEvent::Action { pos, event, param, block } => match block {
                // TileEntityChest/EnderChest.receiveClientEvent: evento 1 = jogadores usando.
                CHEST | TRAPPED_CHEST | ENDER_CHEST => {
                    if event == 1 {
                        if let Some(t) = self.tiles.get_mut(&pos) {
                            t.players_using = param as i32;
                        }
                    }
                }
                // BlockNote.onBlockEventReceived.
                NOTE_BLOCK => {
                    const INSTRUMENTS: [&str; 5] = ["harp", "bd", "snare", "hat", "bassattack"];
                    let name = format!("note.{}", INSTRUMENTS[(event as usize).min(INSTRUMENTS.len() - 1)]);
                    let pitch = 2f64.powf((param as f64 - 12.0) / 12.0) as f32;
                    sounds.push(SoundEffect {
                        name,
                        pos: DVec3::new(pos.0 as f64 + 0.5, pos.1 as f64 + 0.5, pos.2 as f64 + 0.5),
                        volume: 3.0,
                        pitch,
                    });
                }
                _ => {}
            },
        }
    }

    /// Um tick de jogo: anima a tampa dos baús e o livro das mesas de encantamento. `players` são os pés
    /// dos jogadores que não são espectadores (`getClosestPlayer`).
    pub fn tick(&mut self, world: &World, players: &[DVec3], sounds: &mut Vec<SoundEffect>) {
        self.world_time += 1;
        let rand = self.book_rand.get_or_insert_with(|| crate::enchant::JavaRandom::new(crate::inventory::now_ms() as i64));
        let positions: Vec<Pos> = self.tiles.keys().copied().collect();
        for p in positions {
            let id = world.block(p.0, p.1, p.2) >> 4;
            let Some(t) = self.tiles.get_mut(&p) else { continue };
            let mut centre = DVec3::new(p.0 as f64 + 0.5, p.1 as f64 + 0.5, p.2 as f64 + 0.5);
            match id {
                CHEST | TRAPPED_CHEST => {
                    let adj = Adjacent::of(world, p, id);
                    if adj.z_pos {
                        centre.z += 0.5;
                    }
                    if adj.x_pos {
                        centre.x += 0.5;
                    }
                    // Só a metade "positiva" toca o som.
                    t.tick_lid(centre, adj.x_neg || adj.z_neg, sounds);
                }
                ENDER_CHEST => t.tick_lid(centre, false, sounds),
                ENCHANTING_TABLE => {
                    let closest = players.iter().map(|q| (q.distance_squared(centre), *q)).filter(|(d, _)| *d < 9.0).min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, q)| q);
                    t.book.tick(centre, closest, rand);
                }
                _ => {}
            }
        }
    }
}

/// Baús vizinhos do mesmo tipo (`checkForAdjacentChests`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Adjacent {
    pub x_neg: bool,
    pub x_pos: bool,
    pub z_neg: bool,
    pub z_pos: bool,
}

impl Adjacent {
    pub fn of(world: &World, p: Pos, id: u16) -> Self {
        let at = |dx: i32, dz: i32| world.block(p.0 + dx, p.1, p.2 + dz) >> 4 == id;
        Self { x_neg: at(-1, 0), x_pos: at(1, 0), z_neg: at(0, -1), z_pos: at(0, 1) }
    }

    /// Só a metade sem vizinho em -X/-Z desenha (o baú duplo inteiro sai da outra metade).
    pub fn draws(&self) -> bool {
        !self.z_neg && !self.x_neg
    }

    pub fn large(&self) -> bool {
        self.x_pos || self.z_pos
    }
}

// ---------------------------------------------------------------------------------------------
// Modelos
// ---------------------------------------------------------------------------------------------

/// `ModelChest` / `ModelLargeChest`: tampa, trinco e corpo. Textura 64x64 ou 128x64.
pub struct ChestModel {
    pub lid: Part,
    pub knob: Part,
    pub below: Part,
    pub tex: (f32, f32),
}

impl ChestModel {
    pub fn new(large: bool) -> Self {
        let w = if large { 30 } else { 14 };
        Self {
            lid: Part::new([1.0, 7.0, 15.0]).cube((0, 0), [0.0, -5.0, -14.0], [w, 5, 14], 0.0, false),
            knob: Part::new([if large { 16.0 } else { 8.0 }, 7.0, 15.0]).cube((0, 0), [-1.0, -2.0, -15.0], [2, 4, 1], 0.0, false),
            below: Part::new([1.0, 6.0, 1.0]).cube((0, 19), [0.0, 0.0, 0.0], [w, 10, 14], 0.0, false),
            tex: (if large { 128.0 } else { 64.0 }, 64.0),
        }
    }

    /// `renderAll` com a tampa em `angle` (rotateAngleX; o trinco acompanha).
    pub fn emit(&mut self, sink: &mut Sink, m: &Mat4, angle: f32) {
        self.lid.rot.x = angle;
        self.knob.rot.x = angle;
        sink.tex = self.tex;
        sink.part(&self.lid, m);
        sink.part(&self.knob, m);
        sink.part(&self.below, m);
    }
}

/// Matriz do `TileEntityChestRenderer` / `TileEntityEnderChestRenderer` (a posição é o canto do bloco).
pub fn chest_matrix(pos: Pos, meta: u16, adj: Adjacent) -> Mat4 {
    let rot = match meta {
        2 => 180.0f32,
        4 => 90.0,
        5 => -90.0,
        _ => 0.0,
    };
    let mut m = Mat4::from_translation(Vec3::new(pos.0 as f32, pos.1 as f32 + 1.0, pos.2 as f32 + 1.0))
        * Mat4::from_scale(Vec3::new(1.0, -1.0, -1.0))
        * Mat4::from_translation(Vec3::splat(0.5));
    if meta == 2 && adj.x_pos {
        m *= Mat4::from_translation(Vec3::X);
    }
    if meta == 5 && adj.z_pos {
        m *= Mat4::from_translation(Vec3::new(0.0, 0.0, -1.0));
    }
    m * Mat4::from_rotation_y(rot.to_radians()) * Mat4::from_translation(Vec3::splat(-0.5))
}

/// Textura do baú: 24 a 26 de dezembro é a de Natal (`Calendar`; aqui na data UTC).
fn is_christmas() -> bool {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    let (_, month, day) = civil_from_days(secs.div_euclid(86400));
    month == 12 && (24..=26).contains(&day)
}

/// Data civil (ano, mês, dia) a partir de dias desde 1970-01-01.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + (m <= 2) as i64, m, d)
}

/// Escala comum da placa e do estandarte.
const SIGN_SCALE: f32 = 0.6666667;

/// `ModelSign`: tábua e poste (textura 64x32).
pub struct SignModel {
    pub board: Part,
    pub stick: Part,
}

impl SignModel {
    pub fn new() -> Self {
        Self {
            board: Part::new([0.0; 3]).cube((0, 0), [-12.0, -14.0, -1.0], [24, 12, 2], 0.0, false),
            stick: Part::new([0.0; 3]).cube((0, 14), [-1.0, -2.0, -1.0], [2, 14, 2], 0.0, false),
        }
    }
}

impl Default for SignModel {
    fn default() -> Self {
        Self::new()
    }
}

/// Giro da placa na parede (metadado 2 a 5), em graus.
pub fn wall_rotation(meta: u16) -> f32 {
    match meta {
        2 => 180.0,
        4 => 90.0,
        5 => -90.0,
        _ => 0.0,
    }
}

/// Matriz da placa antes da escala do modelo: em pé gira `-meta * 22,5°`; na parede recua.
pub fn sign_matrix(pos: Pos, standing: bool, meta: u16) -> Mat4 {
    let base = Mat4::from_translation(Vec3::new(pos.0 as f32 + 0.5, pos.1 as f32 + 0.75 * SIGN_SCALE, pos.2 as f32 + 0.5));
    if standing {
        base * Mat4::from_rotation_y((-(meta as f32 * 360.0) / 16.0).to_radians())
    } else {
        base * Mat4::from_rotation_y((-wall_rotation(meta)).to_radians()) * Mat4::from_translation(Vec3::new(0.0, -0.3125, -0.4375))
    }
}

/// Primeira linha que o `GuiUtilRenderComponents.splitText(texto, 90, ...)` produz.
pub fn first_sign_line(ui: &mut UiBatch, s: &str) -> String {
    if ui.text_width(s) <= 90.0 {
        return s.to_string();
    }
    // trimStringToWidth(s, 90): o maior prefixo que cabe.
    let mut end = 0;
    for (i, c) in s.char_indices() {
        let next = i + c.len_utf8();
        if ui.text_width(&s[..next]) > 90.0 {
            break;
        }
        end = next;
    }
    let mut line = &s[..end];
    if end < s.len() {
        if let Some(l) = line.rfind(' ') {
            if ui.text_width(&s[..l]) > 0.0 {
                line = &s[..l];
            }
        }
    }
    line.to_string()
}

/// Quadriláteros do texto da placa: `ui.text` desenha em pixels de fonte e `m` leva para o mundo
/// (`translate(0, 0.5s, 0.07s); scale(k, -k, k)` depois da matriz da placa).
pub fn sign_text(ui: &mut UiBatch, geo: &mut Geometry, lines: &[String; 4], base: &Mat4, light: [u8; 4]) {
    let k = 0.015625 * SIGN_SCALE;
    let m = *base * Mat4::from_translation(Vec3::new(0.0, 0.5 * SIGN_SCALE, 0.07 * SIGN_SCALE)) * Mat4::from_scale(Vec3::new(k, -k, k));
    // glNormal3f(0, 0, -k): a luz direcional das entidades vale também para o texto.
    let shade = diffuse(m.transform_vector3(-Vec3::Z).normalize_or_zero());
    let c = (shade * 255.0) as u8;
    let (v0, i0) = (ui.vertices.len(), ui.indices.len());
    for (i, line) in lines.iter().enumerate() {
        if line.is_empty() {
            continue;
        }
        let text = first_sign_line(ui, line);
        let w = (ui.text_width(&text) as i32 / 2) as f32;
        ui.text(-w, (i as i32 * 10 - 4 * 5) as f32, &text, 0xFF000000, false);
    }
    for q in ui.vertices[v0..].chunks_exact(4) {
        let v = |i: usize| Vertex {
            pos: m.transform_point3(Vec3::new(q[i].pos[0], q[i].pos[1], 0.0)).to_array(),
            uv: q[i].uv,
            color: [(q[i].color[0] as u32 * c as u32 / 255) as u8, (q[i].color[1] as u32 * c as u32 / 255) as u8, (q[i].color[2] as u32 * c as u32 / 255) as u8, q[i].color[3]],
            light,
        };
        geo.quad([v(0), v(1), v(2), v(3)]);
    }
    ui.vertices.truncate(v0);
    ui.indices.truncate(i0);
}

/// `ModelSkeletonHead(0, 0, 64, 32)` (esqueleto, wither, creeper) ou `ModelHumanoidHead`
/// (zumbi e jogador: cabeça em 64x64 com a camada externa inflada 0,25).
pub struct SkullModel {
    pub head: Part,
    pub hat: Option<Part>,
    pub tex: (f32, f32),
}

impl SkullModel {
    pub fn new(humanoid: bool) -> Self {
        Self {
            head: Part::new([0.0; 3]).cube((0, 0), [-4.0, -8.0, -4.0], [8, 8, 8], 0.0, false),
            hat: humanoid.then(|| Part::new([0.0; 3]).cube((32, 0), [-4.0, -8.0, -4.0], [8, 8, 8], 0.25, false)),
            tex: (64.0, if humanoid { 64.0 } else { 32.0 }),
        }
    }

    /// `render(null, 0, 0, 0, yaw, 0, 0.0625)`: gira a cabeça em Y por `yaw` graus.
    pub fn emit(&mut self, sink: &mut Sink, m: &Mat4, yaw: f32) {
        sink.tex = self.tex;
        self.head.rot = Vec3::new(0.0, yaw / 57.295776, 0.0);
        sink.part(&self.head, m);
        if let Some(h) = &mut self.hat {
            h.rot = self.head.rot;
            sink.part(h, m);
        }
    }
}

/// Matriz e giro final da cabeça (`renderSkull`): no chão fica no centro do bloco; na parede,
/// encostada pelo lado indicado em `meta & 7`.
pub fn skull_matrix(pos: Pos, meta: u16, rot: u8) -> (Mat4, f32) {
    let (x, y, z) = (pos.0 as f32, pos.1 as f32, pos.2 as f32);
    let mut yaw = rot as f32 * 360.0 / 16.0;
    let at = match meta & 7 {
        1 => Vec3::new(x + 0.5, y, z + 0.5),
        2 => Vec3::new(x + 0.5, y + 0.25, z + 0.74),
        3 => {
            yaw = 180.0;
            Vec3::new(x + 0.5, y + 0.25, z + 0.26)
        }
        4 => {
            yaw = 270.0;
            Vec3::new(x + 0.74, y + 0.25, z + 0.5)
        }
        _ => {
            yaw = 90.0;
            Vec3::new(x + 0.26, y + 0.25, z + 0.5)
        }
    };
    (Mat4::from_translation(at) * Mat4::from_scale(Vec3::new(-1.0, -1.0, 1.0)), yaw)
}

/// Textura (caminho no jar) da cabeça pelo `SkullType`; o jogador (3) usa skin à parte.
pub fn skull_texture(skull_type: u8) -> &'static str {
    match skull_type {
        1 => "entity/skeleton/wither_skeleton",
        2 => "entity/zombie/zombie",
        4 => "entity/creeper/creeper",
        _ => "entity/skeleton/skeleton",
    }
}

// ---------------------------------------------------------------------------------------------
// Desenho
// ---------------------------------------------------------------------------------------------

impl TileEntities {
    /// Geometria do quadro: modelos por textura e o texto das placas (translúcido, no atlas da GUI).
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        cam: &Camera,
        partial: f32,
        world: &World,
        r: &mut Renderer,
        jar: &mut Jar,
        textures: &mut Textures,
        ui: &mut UiBatch,
    ) -> Vec<DrawItem> {
        let mut opaque: HashMap<Tex, Geometry> = HashMap::new();
        let mut text = Geometry::default();
        let christmas = is_christmas();
        let time = self.world_time as f32 + partial;
        let Self { tiles, .. } = self;
        let mut sign_model = SignModel::new();
        for (&p, tile) in tiles.iter() {
            let centre = Vec3::new(p.0 as f32 + 0.5, p.1 as f32 + 0.5, p.2 as f32 + 0.5);
            if (centre - cam.eye).length_squared() >= MAX_RENDER_DISTANCE_SQ {
                continue;
            }
            let st = world.block(p.0, p.1, p.2);
            let (id, meta) = (st >> 4, st & 15);
            if !is_tile_block(id) {
                continue;
            }
            let light = light_at(world, DVec3::new(centre.x as f64, centre.y as f64, centre.z as f64));
            let mut put = |tex: u32, f: &mut dyn FnMut(&mut Sink)| {
                let mut s = Sink { geo: opaque.entry(Tex::Custom(tex)).or_default(), tex: (64.0, 64.0), tint: [1.0; 3], alpha: 255, light, lights: crate::models::WORLD_LIGHTS };
                f(&mut s);
            };
            match id {
                CHEST | TRAPPED_CHEST | ENDER_CHEST => {
                    let adj = if id == ENDER_CHEST { Adjacent::default() } else { Adjacent::of(world, p, id) };
                    if !adj.draws() {
                        continue;
                    }
                    let large = adj.large();
                    let path = match (id, large) {
                        (ENDER_CHEST, _) => "entity/chest/ender",
                        (_, false) if christmas => "entity/chest/christmas",
                        (_, true) if christmas => "entity/chest/christmas_double",
                        (TRAPPED_CHEST, false) => "entity/chest/trapped",
                        (TRAPPED_CHEST, true) => "entity/chest/trapped_double",
                        (_, false) => "entity/chest/normal",
                        (_, true) => "entity/chest/normal_double",
                    };
                    let Some(tex) = textures.jar(r, jar, path) else { continue };
                    let mut model = ChestModel::new(large);
                    let m = chest_matrix(p, meta, adj);
                    let angle = tile.lid_angle(partial);
                    put(tex, &mut |s| model.emit(s, &m, angle));
                }
                ENCHANTING_TABLE => {
                    let Some(tex) = textures.jar(r, jar, "entity/enchanting_table_book") else { continue };
                    let (m, [time, right, left, spread]) = tile.book.pose(p, partial);
                    put(tex, &mut |s| {
                        s.tex = (64.0, 32.0);
                        crate::enchant::emit_book(s, &m, time, right, left, spread);
                    });
                }
                STANDING_SIGN | WALL_SIGN => {
                    let Some(tex) = textures.jar(r, jar, "entity/sign") else { continue };
                    let base = sign_matrix(p, id == STANDING_SIGN, meta);
                    let m = base * Mat4::from_scale(Vec3::new(SIGN_SCALE, -SIGN_SCALE, -SIGN_SCALE));
                    sign_model.stick.show = id == STANDING_SIGN;
                    put(tex, &mut |s| {
                        s.tex = (64.0, 32.0);
                        s.part(&sign_model.board, &m);
                        s.part(&sign_model.stick, &m);
                    });
                    if tile.text.iter().any(|l| !l.is_empty()) {
                        sign_text(ui, &mut text, &tile.text, &base, light);
                    }
                }
                SKULL => {
                    let tex = match tile.skull_type {
                        3 => {
                            let owner = tile.owner.as_ref();
                            let downloaded = owner.and_then(|o| o.skin.as_ref()).and_then(|s| textures.skin(&s.url));
                            match downloaded {
                                Some(id) => Some(id),
                                None => {
                                    // Sem skin (ainda): `DefaultPlayerSkin` pelo UUID, ou o Steve legado.
                                    let (path, _) = owner.and_then(|o| o.id).map_or(("entity/steve", false), default_skin);
                                    textures.jar(r, jar, path)
                                }
                            }
                        }
                        t => textures.jar(r, jar, skull_texture(t)),
                    };
                    let Some(tex) = tex else { continue };
                    let humanoid = matches!(tile.skull_type, 2 | 3);
                    let (m, yaw) = skull_matrix(p, meta, tile.skull_rot);
                    let mut model = SkullModel::new(humanoid);
                    put(tex, &mut |s| model.emit(s, &m, yaw));
                }
                STANDING_BANNER | WALL_BANNER => {
                    let Some(tex) = textures.banners.texture(r, jar, &tile.banner) else { continue };
                    let wall = id == WALL_BANNER;
                    let m = crate::banner::banner_matrix(p, wall, meta);
                    let sway = crate::banner::sway(p, time);
                    let mut model = crate::banner::BannerModel::new();
                    model.stand.show = !wall;
                    put(tex, &mut |s| model.emit(s, &m, sway));
                }
                _ => {}
            }
        }
        let mut items: Vec<DrawItem> = opaque.into_iter().filter(|(_, g)| !g.is_empty()).map(|(tex, geo)| DrawItem { tex, geo, translucent: false }).collect();
        if !text.is_empty() {
            items.push(DrawItem { tex: Tex::Ui, geo: text, translucent: true });
        }
        items
    }
}

// ---------------------------------------------------------------------------------------------
// Itens desenhados por código (`TileEntityItemStackRenderer`, modelos `builtin/entity`)
// ---------------------------------------------------------------------------------------------

/// Item cujo modelo é um tile entity: baús, cabeça e estandarte.
#[derive(Clone, Debug, PartialEq)]
pub enum EntityItem {
    Chest,
    TrappedChest,
    EnderChest,
    Skull { skull_type: u8, owner: Option<Owner> },
    Banner(BannerData),
}

/// Marca (bit 25) da variante de ícone que aponta para a tabela de `EntityItem`; o resto é o índice.
pub const ENTITY_VARIANT: u32 = 1 << 25;

/// `TileEntityItemStackRenderer.renderByItem` escolhe pelo item: 54, 146 e 130 (baús), 397 (cabeça),
/// 425 (estandarte). `SkullOwner` (perfil ou nome) e `BlockEntityTag` (`Base`, `Patterns`) vêm do NBT.
pub fn entity_item(s: &mc_protocol::nbt::Slot) -> Option<EntityItem> {
    entity_item_of(s.id, s.damage, s.nbt.as_ref())
}

/// Como `entity_item`, a partir de id, dano e a tag NBT do item.
pub fn entity_item_of(id: i16, damage: i16, tag: Option<&Nbt>) -> Option<EntityItem> {
    Some(match id {
        54 => EntityItem::Chest,
        146 => EntityItem::TrappedChest,
        130 => EntityItem::EnderChest,
        397 => {
            let owner = match tag.and_then(|t| t.get("SkullOwner")) {
                Some(o @ Nbt::Compound(_)) => Some(read_owner(o)),
                Some(Nbt::String(n)) if !n.is_empty() => Some(Owner { name: Some(n.clone()), ..Default::default() }),
                _ => None,
            };
            EntityItem::Skull { skull_type: damage.clamp(0, 255) as u8, owner }
        }
        425 => {
            // `setItemValues`: a base vem do `Base` da tag, ou do dano do item (& 15).
            let entity_tag = tag.and_then(|t| t.get("BlockEntityTag")).filter(|t| matches!(t, Nbt::Compound(_)));
            let mut data = entity_tag.map(BannerData::from_nbt).unwrap_or(BannerData { loaded: true, ..Default::default() });
            if !entity_tag.is_some_and(|t| t.get("Base").is_some()) {
                data.base = (damage & 15) as i32;
            }
            EntityItem::Banner(data)
        }
        _ => return None,
    })
}

fn variants() -> &'static std::sync::Mutex<Vec<EntityItem>> {
    static T: std::sync::OnceLock<std::sync::Mutex<Vec<EntityItem>>> = std::sync::OnceLock::new();
    T.get_or_init(Default::default)
}

/// Variante de ícone (campo `variant` da chave) que identifica este item e seus dados.
pub fn entity_variant(item: EntityItem) -> u32 {
    let mut t = variants().lock().unwrap_or_else(|e| e.into_inner());
    let i = t.iter().position(|v| *v == item).unwrap_or_else(|| {
        t.push(item);
        t.len() - 1
    });
    ENTITY_VARIANT | i as u32
}

/// Item de uma variante criada por `entity_variant`.
pub fn entity_from_variant(variant: u32) -> Option<EntityItem> {
    if variant & ENTITY_VARIANT == 0 {
        return None;
    }
    variants().lock().ok()?.get((variant & 0xFFFFFF) as usize).cloned()
}

/// Caminho no jar da skin (ou textura) de uma cabeça sem baixar nada: o perfil sem skin usa
/// `DefaultPlayerSkin` (pelo UUID) ou o Steve.
fn skull_path(skull_type: u8, owner: Option<&Owner>) -> &'static str {
    if skull_type == 3 {
        owner.and_then(|o| o.id).map_or("entity/steve", |id| default_skin(id).0)
    } else {
        skull_texture(skull_type)
    }
}

/// Textura de caminho no jar do baú de item (`TileEntityChestRenderer`, com a de Natal).
fn chest_path(item: &EntityItem) -> &'static str {
    match (item, is_christmas()) {
        (EntityItem::EnderChest, _) => "entity/chest/ender",
        (_, true) => "entity/chest/christmas",
        (EntityItem::TrappedChest, _) => "entity/chest/trapped",
        _ => "entity/chest/normal",
    }
}

/// Imagem na CPU da textura do item (para o ícone) e se já é a definitiva (a skin baixou).
pub fn item_image(item: &EntityItem, textures: &mut Textures, jar: &mut Jar) -> Option<(image::RgbaImage, bool)> {
    match item {
        EntityItem::Chest | EntityItem::TrappedChest | EntityItem::EnderChest => Some((textures.jar_image(jar, chest_path(item))?, true)),
        EntityItem::Banner(d) => Some((crate::banner::compose_banner(jar, d)?, true)),
        EntityItem::Skull { skull_type, owner } => {
            if *skull_type == 3 {
                if let Some(url) = owner.as_ref().and_then(|o| o.skin.as_ref()).map(|s| s.url.clone()) {
                    let _ = textures.skin(&url); // inicia o download
                    if let Some(img) = textures.skin_image(&url) {
                        return Some((img.clone(), true));
                    }
                    return Some((textures.jar_image(jar, skull_path(*skull_type, owner.as_ref()))?, false));
                }
            }
            Some((textures.jar_image(jar, skull_path(*skull_type, owner.as_ref()))?, true))
        }
    }
}

/// Textura na GPU do item (mão e terceira pessoa).
pub fn item_texture(item: &EntityItem, textures: &mut Textures, r: &mut Renderer, jar: &mut Jar) -> Option<u32> {
    match item {
        EntityItem::Chest | EntityItem::TrappedChest | EntityItem::EnderChest => textures.jar(r, jar, chest_path(item)),
        EntityItem::Banner(d) => textures.banners.texture(r, jar, d),
        EntityItem::Skull { skull_type, owner } => {
            if *skull_type == 3 {
                if let Some(id) = owner.as_ref().and_then(|o| o.skin.as_ref()).and_then(|s| textures.skin(&s.url)) {
                    return Some(id);
                }
            }
            textures.jar(r, jar, skull_path(*skull_type, owner.as_ref()))
        }
    }
}

/// Geometria do item em unidades de bloco, a partir de `m` (que já inclui o -0.5 do `RenderItem`):
/// os mesmos renderers dos blocos, com o tile entity na origem e sem mundo.
pub fn emit_item(item: &EntityItem, sink: &mut Sink, m: &Mat4) {
    match item {
        EntityItem::Chest | EntityItem::TrappedChest | EntityItem::EnderChest => {
            // `i = 0` sem mundo: sem giro, tampa fechada.
            ChestModel::new(false).emit(sink, &(*m * chest_matrix((0, 0, 0), 0, Adjacent::default())), 0.0);
        }
        EntityItem::Skull { skull_type, .. } => {
            // translate(-0.5, 0, -0.5), scale(2) e `renderSkull(0, 0, 0, UP, 0, tipo, perfil)`.
            let (sm, yaw) = skull_matrix((0, 0, 0), 1, 0);
            let m = *m * Mat4::from_translation(Vec3::new(-0.5, 0.0, -0.5)) * Mat4::from_scale(Vec3::splat(2.0)) * sm;
            SkullModel::new(matches!(skull_type, 2 | 3)).emit(sink, &m, yaw);
        }
        EntityItem::Banner(_) => {
            // Sem mundo: em pé, giro 0, posição e tempo 0 (balanço do `cos(0)`).
            let mut model = crate::banner::BannerModel::new();
            model.stand.show = true;
            model.emit(sink, &(*m * crate::banner::banner_matrix((0, 0, 0), false, 0)), crate::banner::sway((0, 0, 0), 0.0));
        }
    }
}

/// Mundo de teste sem servidor (`MINE_RS_MUNDO=tiles`): plataforma com baús, placas, cabeças e
/// estandartes para conferir os renderers no display virtual.
pub fn demo(world: &mut World, tiles: &mut TileEntities, player: &mut crate::physics::Player) {
    use mc_world::chunk::Column;
    let nbt = |pairs: Vec<(&str, Nbt)>| Nbt::Compound(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect());
    for (cx, cz) in [(-1, -1), (0, -1), (-1, 0), (0, 0)] {
        world.columns.insert((cx, cz), Column::default());
    }
    for x in -16..16 {
        for z in -16..16 {
            world.set_block(x, 63, z, 4 << 4);
        }
    }
    let put = |world: &mut World, tiles: &mut TileEntities, x: i32, z: i32, id: u16, meta: u16| {
        world.set_block(x, 64, z, id << 4 | meta);
        tiles.block_changed(x, 64, z, id << 4 | meta);
    };
    // Baús: normal, armadilha, ender, duplo e um aberto (todos virados para o sul).
    put(world, tiles, -7, 0, CHEST, 3);
    put(world, tiles, -5, 0, TRAPPED_CHEST, 3);
    put(world, tiles, -3, 0, ENDER_CHEST, 3);
    put(world, tiles, -1, 0, CHEST, 3);
    put(world, tiles, 0, 0, CHEST, 3);
    put(world, tiles, 3, 0, CHEST, 3);
    put(world, tiles, 5, 0, CHEST, 5);
    put(world, tiles, 5, 1, CHEST, 5);
    put(world, tiles, 7, 0, CHEST, 2);
    if let Some(t) = tiles.tiles.get_mut(&(3, 64, 0)) {
        t.players_using = 1;
        t.lid = 1.0;
        t.prev_lid = 1.0;
    }
    // Placas em pé com giros diferentes e na parede.
    for (i, meta) in [0u16, 2, 4, 6, 8, 12].into_iter().enumerate() {
        let x = -7 + 2 * i as i32;
        put(world, tiles, x, -4, STANDING_SIGN, meta);
        if let Some(t) = tiles.tiles.get_mut(&(x, 64, -4)) {
            t.text = ["Ola, mundo".into(), format!("giro {meta}"), "\u{a7}cvermelho".into(), "\u{a7}l\u{a7}9negrito".into()];
        }
    }
    world.set_block(5, 64, -6, 1 << 4);
    put(world, tiles, 5, -5, WALL_SIGN, 3);
    if let Some(t) = tiles.tiles.get_mut(&(5, 64, -5)) {
        t.text = ["na parede".into(), String::new(), "texto muito comprido da placa".into(), "fim".into()];
    }
    // Cabeças: esqueleto, wither, zumbi, jogador (sem skin), creeper; uma na parede.
    for ty in 0..5u8 {
        let x = -7 + 2 * ty as i32;
        put(world, tiles, x, 4, SKULL, 1);
        let n = nbt(vec![("SkullType", Nbt::Byte(ty as i8)), ("Rot", Nbt::Byte(ty as i8 * 3))]);
        tiles.tiles.get_mut(&(x, 64, 4)).unwrap().read_skull(&n);
    }
    world.set_block(5, 64, 3, 1 << 4);
    put(world, tiles, 5, 4, SKULL, 3);
    tiles.tiles.get_mut(&(5, 64, 4)).unwrap().read_skull(&nbt(vec![("SkullType", Nbt::Byte(1))]));
    // Estandartes: lisos e com padrões.
    let pat = |id: &str, color: i32| nbt(vec![("Pattern", Nbt::String(id.into())), ("Color", Nbt::Int(color))]);
    let banners = [
        nbt(vec![("Base", Nbt::Int(1))]),
        nbt(vec![("Base", Nbt::Int(15)), ("Patterns", Nbt::List(vec![pat("cr", 1), pat("bo", 0)]))]),
        nbt(vec![("Base", Nbt::Int(4)), ("Patterns", Nbt::List(vec![pat("ms", 15), pat("cbo", 1), pat("flo", 14)]))]),
    ];
    for (i, b) in banners.iter().enumerate() {
        let x = -3 + 2 * i as i32;
        put(world, tiles, x, 8, STANDING_BANNER, 0);
        tiles.tiles.get_mut(&(x, 64, 8)).unwrap().banner = BannerData::from_nbt(b);
    }
    world.set_block(6, 64, 9, 1 << 4);
    put(world, tiles, 6, 8, WALL_BANNER, 3);
    tiles.tiles.get_mut(&(6, 64, 8)).unwrap().banner = BannerData::from_nbt(&banners[2]);
    player.teleport(DVec3::new(0.0, 64.0, 13.0), 180.0, 12.0);
    player.flying = true;
    player.allow_flying = true;
    // MINE_RS_CAM=x,y,z,yaw,pitch muda a câmera do mundo de teste.
    if let Some(v) = std::env::var("MINE_RS_CAM").ok().map(|c| c.split(',').filter_map(|n| n.trim().parse::<f64>().ok()).collect::<Vec<_>>()).filter(|v| v.len() == 5) {
        player.teleport(DVec3::new(v[0], v[1], v[2]), v[3] as f32, v[4] as f32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn livro_da_mesa_abre_para_o_jogador_perto() {
        let mut b = TableBook::default();
        let mut rand = crate::enchant::JavaRandom::new(1);
        let centre = DVec3::new(0.5, 64.5, 0.5);
        for _ in 0..10 {
            b.tick(centre, Some(DVec3::new(2.5, 64.0, 0.5)), &mut rand);
        }
        // Abre de 0,1 em 0,1 até 1 e gira para +X (atan2(0, 2) = 0).
        assert_eq!(b.spread, 1.0);
        assert!(b.rotation.abs() < 1e-3, "{}", b.rotation);
        for _ in 0..10 {
            b.tick(centre, None, &mut rand);
        }
        assert_eq!(b.spread, 0.0);
    }
    use crate::models::FULL_LIGHT;
    use mc_world::chunk::Column;

    fn encode_pos(x: i32, y: i32, z: i32) -> [u8; 8] {
        let v = ((x as i64 & 0x3FFFFFF) << 38) | ((y as i64 & 0xFFF) << 26) | (z as i64 & 0x3FFFFFF);
        v.to_be_bytes()
    }

    fn string(s: &str) -> Vec<u8> {
        let mut v = vec![s.len() as u8];
        v.extend_from_slice(s.as_bytes());
        v
    }

    fn world_with(blocks: &[(i32, i32, i32, u16)]) -> World {
        let mut w = World::new(true);
        w.columns.insert((0, 0), Column::default());
        w.columns.insert((-1, 0), Column::default());
        for &(x, y, z, st) in blocks {
            w.set_block(x, y, z, st);
        }
        w
    }

    fn bounds(geo: &Geometry) -> ([f32; 3], [f32; 3]) {
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for v in &geo.vertices {
            for i in 0..3 {
                lo[i] = lo[i].min(v.pos[i]);
                hi[i] = hi[i].max(v.pos[i]);
            }
        }
        (lo, hi)
    }

    fn emit_chest(large: bool, pos: Pos, meta: u16, adj: Adjacent, angle: f32) -> Geometry {
        let mut geo = Geometry::default();
        let mut model = ChestModel::new(large);
        let mut sink = Sink { geo: &mut geo, tex: (64.0, 64.0), tint: [1.0; 3], alpha: 255, light: FULL_LIGHT, lights: crate::models::WORLD_LIGHTS };
        model.emit(&mut sink, &chest_matrix(pos, meta, adj), angle);
        geo
    }

    #[test]
    fn parses_tile_entity_packets() {
        // 0x35: posição, tipo 4 (cabeça) e NBT { SkullType: 3b } com nome vazio.
        let mut body = encode_pos(-5, 70, 12).to_vec();
        body.push(4);
        body.extend_from_slice(&[10, 0, 0, 1, 0, 9]);
        body.extend_from_slice(b"SkullType");
        body.extend_from_slice(&[3, 0]);
        match parse_packet(0x35, &body).unwrap() {
            TileEvent::Update { pos, action, nbt } => {
                assert_eq!((pos, action), ((-5, 70, 12), 4));
                assert_eq!(nbt.unwrap().get("SkullType"), Some(&Nbt::Byte(3)));
            }
            e => panic!("{e:?}"),
        }
        // 0x33: quatro linhas de JSON.
        let mut body = encode_pos(1, 2, 3).to_vec();
        for l in ["\"a\"", "\"b\"", "", "{\"text\":\"d\"}"] {
            body.extend(string(l));
        }
        let TileEvent::Sign { pos, lines } = parse_packet(0x33, &body).unwrap() else { panic!() };
        assert_eq!(pos, (1, 2, 3));
        assert_eq!(lines[3], "{\"text\":\"d\"}");
        // 0x24: evento 1, parâmetro 2, bloco 54 (VarInt).
        let mut body = encode_pos(0, 64, 0).to_vec();
        body.extend_from_slice(&[1, 2, 54]);
        assert_eq!(parse_packet(0x24, &body).unwrap(), TileEvent::Action { pos: (0, 64, 0), event: 1, param: 2, block: 54 });
        assert!(parse_packet(0x24, &body[..9]).is_err());
    }

    #[test]
    fn tiles_follow_chunk_and_block() {
        let mut w = world_with(&[(1, 64, 1, CHEST << 4 | 3), (2, 64, 2, 1 << 4), (-3, 70, 4, STANDING_SIGN << 4)]);
        let mut t = TileEntities::default();
        t.scan_column(&w, 0, 0);
        t.scan_column(&w, -1, 0);
        assert_eq!(t.tiles.len(), 2);
        // Sinal recebido fica na posição; trocar de bloco (outro id) remove, mesmo id mantém.
        let mut sounds = Vec::new();
        t.handle(TileEvent::Sign { pos: (-3, 70, 4), lines: ["\"oi\"".into(), String::new(), String::new(), "{\"text\":\"x\",\"color\":\"red\"}".into()] }, &w, &mut sounds);
        assert_eq!(t.tiles[&(-3, 70, 4)].text[0], "oi");
        assert_eq!(t.tiles[&(-3, 70, 4)].text[3], "§cx");
        t.block_changed(-3, 70, 4, STANDING_SIGN << 4 | 5);
        assert_eq!(t.tiles[&(-3, 70, 4)].text[0], "oi");
        t.block_changed(1, 64, 1, 0);
        assert!(!t.tiles.contains_key(&(1, 64, 1)));
        t.block_changed(2, 64, 2, WALL_SIGN << 4 | 2);
        assert!(t.tiles.contains_key(&(2, 64, 2)));
        // Chunk descarregado leva as tile entities junto.
        w.unload(-1, 0);
        t.unload_column(-1, 0);
        assert!(!t.tiles.contains_key(&(-3, 70, 4)));
        // Reenvio do chunk: valem só os blocos que o mundo tem (o baú em 1,64,1; a placa em 2,64,2 era só do evento).
        t.scan_column(&w, 0, 0);
        assert_eq!(t.tiles.keys().copied().collect::<Vec<_>>(), vec![(1, 64, 1)]);
    }

    #[test]
    fn chest_lid_animates_and_plays_sounds() {
        let w = world_with(&[(1, 64, 1, CHEST << 4 | 3)]);
        let mut t = TileEntities::default();
        t.scan_column(&w, 0, 0);
        let mut sounds = Vec::new();
        t.handle(TileEvent::Action { pos: (1, 64, 1), event: 1, param: 1, block: CHEST }, &w, &mut sounds);
        assert_eq!(t.tiles[&(1, 64, 1)].players_using, 1);
        for _ in 0..12 {
            t.tick(&w, &[], &mut sounds);
        }
        assert_eq!(t.tiles[&(1, 64, 1)].lid, 1.0);
        assert_eq!(sounds.len(), 1);
        assert_eq!(sounds[0].name, "random.chestopen");
        assert_eq!((sounds[0].pos.x, sounds[0].pos.y, sounds[0].pos.z), (1.5, 64.5, 1.5));
        assert!((0.9..1.0).contains(&sounds[0].pitch));
        t.handle(TileEvent::Action { pos: (1, 64, 1), event: 1, param: 0, block: CHEST }, &w, &mut sounds);
        for _ in 0..12 {
            t.tick(&w, &[], &mut sounds);
        }
        assert_eq!(t.tiles[&(1, 64, 1)].lid, 0.0);
        assert_eq!(sounds.iter().filter(|s| s.name == "random.chestclosed").count(), 1);
        // A tampa só sobe 0,1 por tick.
        let tile = Tile { lid: 0.5, prev_lid: 0.4, ..Default::default() };
        assert!((tile.lid_angle(1.0) + (1.0 - 0.5f32.powi(3)) * PI / 2.0).abs() < 1e-6);
        assert_eq!(Tile::default().lid_angle(0.5), 0.0 * -1.0);
    }

    #[test]
    fn note_block_action_plays_instrument() {
        let w = world_with(&[]);
        let mut t = TileEntities::default();
        let mut sounds = Vec::new();
        t.handle(TileEvent::Action { pos: (1, 64, 1), event: 2, param: 24, block: NOTE_BLOCK }, &w, &mut sounds);
        assert_eq!(sounds[0].name, "note.snare");
        assert_eq!((sounds[0].volume, sounds[0].pitch), (3.0, 2.0));
        t.handle(TileEvent::Action { pos: (1, 64, 1), event: 0, param: 12, block: NOTE_BLOCK }, &w, &mut sounds);
        assert_eq!((sounds[1].name.as_str(), sounds[1].pitch), ("note.harp", 1.0));
    }

    #[test]
    fn chest_model_geometry() {
        let m = ChestModel::new(false);
        // Tampa, trinco e corpo: 3 caixas, 6 faces cada.
        assert_eq!(m.lid.quad_count() + m.knob.quad_count() + m.below.quad_count(), 18);
        // UV do corpo (0,19) 14x10x14: frente (-Z) em u 14..28, v 33..43.
        let faces = m.below.cubes[0].faces();
        let (_, uv, normal) = faces[4];
        assert_eq!(normal, -Vec3::Z);
        assert_eq!((uv[1], uv[3]), ([14.0, 33.0], [28.0, 43.0]));
        // Topo da tampa (0,0) 14x5x14: u 14..28, v 0..14.
        let (_, uv, normal) = m.lid.cubes[0].faces()[2];
        assert_eq!(normal, -Vec3::Y);
        assert_eq!((uv[2], uv[0]), ([14.0, 14.0], [28.0, 0.0]));
        let big = ChestModel::new(true);
        assert_eq!(big.tex, (128.0, 64.0));
        assert_eq!(big.lid.cubes[0].size, [30, 5, 14]);
        assert_eq!(big.knob.pivot.x, 16.0);
        let faces = big.below.cubes[0].faces();
        // Corpo largo (0,19) 30x10x14: frente em u 14..44, v 33..43.
        assert_eq!((faces[4].1[1], faces[4].1[3]), ([14.0, 33.0], [44.0, 43.0]));
    }

    #[test]
    fn chest_matrix_places_model_in_block() {
        // Fechado, virado para o sul (meta 3): ocupa 1..15 em X/Z e 0..14 em altura (em 1/16).
        let g = emit_chest(false, (10, 64, 20), 3, Adjacent::default(), 0.0);
        let (lo, hi) = bounds(&g);
        let near = |a: f32, b: f32| (a - b).abs() < 1e-4;
        assert!(near(lo[0], 10.0 + 1.0 / 16.0) && near(hi[0], 10.0 + 15.0 / 16.0), "{lo:?} {hi:?}");
        assert!(near(lo[1], 64.0) && near(hi[1], 64.0 + 14.0 / 16.0), "{lo:?} {hi:?}");
        // O trinco (1 pixel) encosta na face da frente: z vai até 21,0.
        assert!(near(lo[2], 20.0 + 1.0 / 16.0) && near(hi[2], 21.0), "{lo:?} {hi:?}");
        // O trinco fica do lado da frente: para o sul (meta 3) é +Z; para o norte (meta 2), -Z.
        let knob = |meta: u16| {
            let mut geo = Geometry::default();
            let mut sink = Sink { geo: &mut geo, tex: (64.0, 64.0), tint: [1.0; 3], alpha: 255, light: FULL_LIGHT, lights: crate::models::WORLD_LIGHTS };
            sink.part(&ChestModel::new(false).knob, &chest_matrix((0, 0, 0), meta, Adjacent::default()));
            let (lo, hi) = bounds(&geo);
            [(lo[0] + hi[0]) / 2.0 - 0.5, (lo[2] + hi[2]) / 2.0 - 0.5]
        };
        let (s, n, w, e) = (knob(3), knob(2), knob(4), knob(5));
        assert!(s[1] > 0.4 && s[0].abs() < 1e-4, "{s:?}");
        assert!(n[1] < -0.4 && n[0].abs() < 1e-4, "{n:?}");
        assert!(w[0] < -0.4 && w[1].abs() < 1e-4, "{w:?}");
        assert!(e[0] > 0.4 && e[1].abs() < 1e-4, "{e:?}");
        // Tampa aberta (90°): sobe acima do bloco inteiro e a caixa passa de 14/16.
        let open = emit_chest(false, (0, 0, 0), 3, Adjacent::default(), -PI / 2.0);
        assert!(bounds(&open).1[1] > 14.0 / 16.0 + 0.05);
    }

    #[test]
    fn double_chest_joins_neighbours() {
        // Dois baús lado a lado em X, ambos virados para o sul: a metade em -X desenha o duplo.
        let w = world_with(&[(4, 64, 4, CHEST << 4 | 3), (5, 64, 4, CHEST << 4 | 3), (8, 64, 8, CHEST << 4 | 2), (8, 64, 9, CHEST << 4 | 2), (2, 64, 2, CHEST << 4 | 3), (3, 64, 2, TRAPPED_CHEST << 4 | 3)]);
        let a = Adjacent::of(&w, (4, 64, 4), CHEST);
        let b = Adjacent::of(&w, (5, 64, 4), CHEST);
        assert!(a.draws() && a.large() && a.x_pos);
        assert!(!b.draws() && b.x_neg);
        // Em Z: o de menor Z desenha.
        assert!(Adjacent::of(&w, (8, 64, 8), CHEST).draws() && Adjacent::of(&w, (8, 64, 8), CHEST).z_pos);
        assert!(!Adjacent::of(&w, (8, 64, 9), CHEST).draws());
        // Tipo diferente (baú armadilha) não junta.
        let c = Adjacent::of(&w, (2, 64, 2), CHEST);
        assert!(c.draws() && !c.large());
        // O duplo ocupa 2 blocos em X (1/16 a 31/16).
        let g = emit_chest(true, (4, 64, 4), 3, a, 0.0);
        let (lo, hi) = bounds(&g);
        assert!((lo[0] - (4.0 + 1.0 / 16.0)).abs() < 1e-4 && (hi[0] - (4.0 + 31.0 / 16.0)).abs() < 1e-4, "{lo:?} {hi:?}");
        // Virado para o norte (meta 2), o duplo anda 1 em X antes de girar e continua dentro dos 2 blocos.
        let g = emit_chest(true, (8, 64, 8), 2, Adjacent { x_pos: true, ..Default::default() }, 0.0);
        let (lo, hi) = bounds(&g);
        assert!((lo[0] - (8.0 + 1.0 / 16.0)).abs() < 1e-4 && (hi[0] - (8.0 + 31.0 / 16.0)).abs() < 1e-4, "{lo:?} {hi:?}");
        // Virado para o leste (meta 5) com vizinho em +Z: o duplo cobre os dois blocos em Z.
        let g = emit_chest(true, (8, 64, 8), 5, Adjacent { z_pos: true, ..Default::default() }, 0.0);
        let (lo, hi) = bounds(&g);
        assert!((lo[2] - (8.0 + 1.0 / 16.0)).abs() < 1e-4 && (hi[2] - (8.0 + 31.0 / 16.0)).abs() < 1e-4, "{lo:?} {hi:?}");
    }

    #[test]
    fn sign_rotation_matches_block_data() {
        let front = |m: &Mat4| m.transform_vector3(Vec3::Z);
        let near = |a: Vec3, b: Vec3| (a - b).length() < 1e-5;
        // Em pé: meta * 22,5° no sentido horário visto de cima; 0 sul, 4 oeste, 8 norte, 12 leste.
        assert!(near(front(&sign_matrix((0, 0, 0), true, 0)), Vec3::Z));
        assert!(near(front(&sign_matrix((0, 0, 0), true, 4)), -Vec3::X));
        assert!(near(front(&sign_matrix((0, 0, 0), true, 8)), -Vec3::Z));
        assert!(near(front(&sign_matrix((0, 0, 0), true, 12)), Vec3::X));
        // 22,5° entre dois valores seguidos.
        let a = front(&sign_matrix((0, 0, 0), true, 1));
        assert!(near(a, Vec3::new(-(22.5f32.to_radians()).sin(), 0.0, (22.5f32.to_radians()).cos())));
        // Na parede: 3 sul, 2 norte, 4 oeste, 5 leste.
        assert!(near(front(&sign_matrix((0, 0, 0), false, 3)), Vec3::Z));
        assert!(near(front(&sign_matrix((0, 0, 0), false, 2)), -Vec3::Z));
        assert!(near(front(&sign_matrix((0, 0, 0), false, 4)), -Vec3::X));
        assert!(near(front(&sign_matrix((0, 0, 0), false, 5)), Vec3::X));
        // Altura: 0,75 * 2/3 = meio bloco; na parede desce 0,3125 e recua 0,4375.
        let p = sign_matrix((3, 70, 5), true, 0).transform_point3(Vec3::ZERO);
        assert!(near(p, Vec3::new(3.5, 70.5, 5.5)));
        let p = sign_matrix((3, 70, 5), false, 3).transform_point3(Vec3::ZERO);
        assert!(near(p, Vec3::new(3.5, 70.5 - 0.3125, 5.5 - 0.4375)));
        // Modelo: tábua 24x12x2 e poste 2x14x2.
        let m = SignModel::new();
        assert_eq!(m.board.cubes[0].size, [24, 12, 2]);
        assert_eq!(m.stick.cubes[0].uv, (0, 14));
    }

    #[test]
    fn sign_geometry_matches_model_scale() {
        let mut geo = Geometry::default();
        let mut sink = Sink { geo: &mut geo, tex: (64.0, 32.0), tint: [1.0; 3], alpha: 255, light: FULL_LIGHT, lights: crate::models::WORLD_LIGHTS };
        let model = SignModel::new();
        let m = sign_matrix((0, 0, 0), true, 0) * Mat4::from_scale(Vec3::new(SIGN_SCALE, -SIGN_SCALE, -SIGN_SCALE));
        sink.part(&model.board, &m);
        sink.part(&model.stick, &m);
        assert_eq!(geo.vertices.len(), 12 * 4);
        let (lo, hi) = bounds(&geo);
        // Largura 24/16 * 2/3 = 1 bloco; do poste (embaixo) à ponta da tábua: 26/16 * 2/3.
        assert!((hi[0] - lo[0] - 1.0).abs() < 1e-4);
        assert!((hi[1] - lo[1] - 26.0 / 16.0 * SIGN_SCALE).abs() < 1e-4);
        assert!(lo[1] > -1e-4, "o poste fica sobre o chão: {lo:?}");
    }

    #[test]
    fn skull_placement_and_models() {
        // No chão: centro do bloco, giro Rot * 22,5°.
        let (m, yaw) = skull_matrix((1, 2, 3), 1, 4);
        assert_eq!(yaw, 90.0);
        assert!((m.transform_point3(Vec3::ZERO) - Vec3::new(1.5, 2.0, 3.5)).length() < 1e-6);
        // Na parede: para o norte (2) recua para z+0,74; sul (3) gira 180°; oeste (4) 270°; leste (5) 90°.
        let (m, yaw) = skull_matrix((1, 2, 3), 2, 7);
        assert_eq!(yaw, 7.0 * 22.5);
        assert!((m.transform_point3(Vec3::ZERO) - Vec3::new(1.5, 2.25, 3.74)).length() < 1e-6);
        assert_eq!(skull_matrix((0, 0, 0), 3, 9).1, 180.0);
        assert_eq!(skull_matrix((0, 0, 0), 4, 9).1, 270.0);
        assert_eq!(skull_matrix((0, 0, 0), 5, 9).1, 90.0);
        // Cabeça de esqueleto: 1 caixa em 64x32; de jogador/zumbi: cabeça + camada externa em 64x64.
        let mut geo = Geometry::default();
        let mut sink = Sink { geo: &mut geo, tex: (64.0, 64.0), tint: [1.0; 3], alpha: 255, light: FULL_LIGHT, lights: crate::models::WORLD_LIGHTS };
        SkullModel::new(false).emit(&mut sink, &Mat4::IDENTITY, 0.0);
        assert_eq!(sink.geo.vertices.len(), 24);
        let h = SkullModel::new(true);
        assert_eq!((h.tex, h.hat.as_ref().unwrap().cubes[0].inflate), ((64.0, 64.0), 0.25));
        assert_eq!(skull_texture(1), "entity/skeleton/wither_skeleton");
        assert_eq!(skull_texture(4), "entity/creeper/creeper");
        // O giro vira ângulo em Y: com 90° a frente (-Z) da cabeça aponta para outro eixo.
        let mut geo = Geometry::default();
        let mut sink = Sink { geo: &mut geo, tex: (64.0, 32.0), tint: [1.0; 3], alpha: 255, light: FULL_LIGHT, lights: crate::models::WORLD_LIGHTS };
        let mut model = SkullModel::new(false);
        model.emit(&mut sink, &Mat4::IDENTITY, 90.0);
        assert!((model.head.rot.y - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
    }

    #[test]
    fn skull_owner_from_nbt() {
        use std::collections::HashMap;
        let tex_json = r#"{"textures":{"SKIN":{"url":"http://textures.minecraft.net/texture/abc"}}}"#;
        let b64 = |s: &str| {
            const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let mut o = String::new();
            for c in s.as_bytes().chunks(3) {
                let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
                for i in 0..4 {
                    o.push(if i <= c.len() { T[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
                }
            }
            o
        };
        let prop = Nbt::Compound(HashMap::from([("Value".to_string(), Nbt::String(b64(tex_json)))]));
        let owner = Nbt::Compound(HashMap::from([
            ("Id".to_string(), Nbt::String("00000000-0000-0000-0000-000000000002".into())),
            ("Name".to_string(), Nbt::String("Alvo".into())),
            ("Properties".to_string(), Nbt::Compound(HashMap::from([("textures".to_string(), Nbt::List(vec![prop]))]))),
        ]));
        let nbt = Nbt::Compound(HashMap::from([
            ("SkullType".to_string(), Nbt::Byte(3)),
            ("Rot".to_string(), Nbt::Byte(5)),
            ("Owner".to_string(), owner),
        ]));
        let mut t = Tile::default();
        t.read_skull(&nbt);
        assert_eq!((t.skull_type, t.skull_rot), (3, 5));
        let o = t.owner.unwrap();
        assert_eq!(o.id, Some(Uuid(2)));
        assert_eq!(o.name.as_deref(), Some("Alvo"));
        assert_eq!(o.skin.unwrap().url, "http://textures.minecraft.net/texture/abc");
    }

    #[test]
    fn entity_items_read_nbt_and_round_trip_variants() {
        use mc_protocol::nbt::Slot;
        let slot = |id: i16, damage: i16, nbt: Option<Nbt>| Slot { id, count: 1, damage, nbt, raw_nbt: Vec::new() };
        let c = |pairs: Vec<(&str, Nbt)>| Nbt::Compound(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect());
        assert_eq!(entity_item(&slot(54, 0, None)), Some(EntityItem::Chest));
        assert_eq!(entity_item(&slot(146, 0, None)), Some(EntityItem::TrappedChest));
        assert_eq!(entity_item(&slot(130, 0, None)), Some(EntityItem::EnderChest));
        assert_eq!(entity_item(&slot(1, 0, None)), None);
        // Estandarte sem tag: a base é o dano & 15; com `Base` na tag, vale a tag.
        let Some(EntityItem::Banner(d)) = entity_item(&slot(425, 5, None)) else { panic!() };
        assert_eq!((d.base, d.layers.len()), (5, 0));
        let tag = c(vec![("BlockEntityTag", c(vec![("Base", Nbt::Int(2)), ("Patterns", Nbt::List(vec![c(vec![("Pattern", Nbt::String("cr".into())), ("Color", Nbt::Int(1))])]))]))]);
        let Some(EntityItem::Banner(d)) = entity_item(&slot(425, 5, Some(tag))) else { panic!() };
        assert_eq!((d.base, d.layers.len()), (2, 1));
        // Cabeça: `SkullOwner` por nome.
        let Some(EntityItem::Skull { skull_type, owner }) = entity_item(&slot(397, 3, Some(c(vec![("SkullOwner", Nbt::String("Notch".into()))])))) else { panic!() };
        assert_eq!((skull_type, owner.and_then(|o| o.name)), (3, Some("Notch".to_string())));
        let v = entity_variant(EntityItem::Banner(d.clone()));
        assert_eq!(entity_variant(EntityItem::Banner(d.clone())), v);
        assert_eq!(entity_from_variant(v), Some(EntityItem::Banner(d)));
        assert_eq!(entity_from_variant(5), None);
    }

    #[test]
    fn item_geometry_sits_in_block_space() {
        let mut geo = Geometry::default();
        let mut sink = Sink { geo: &mut geo, tex: (64.0, 64.0), tint: [1.0; 3], alpha: 255, light: FULL_LIGHT, lights: crate::models::WORLD_LIGHTS };
        emit_item(&EntityItem::Chest, &mut sink, &Mat4::IDENTITY);
        // Baú sem mundo: ocupa 14 x 14 x 10 pixels dentro do bloco, apoiado em y = 0.
        let (lo, hi) = bounds(&geo);
        assert!((lo[1] - 0.0).abs() < 1e-4 && (hi[1] - 0.875).abs() < 1e-4, "{lo:?} {hi:?}");
        assert!(lo[0] >= 0.0 && hi[0] <= 1.0 && lo[2] >= 0.0 && hi[2] <= 1.0, "{lo:?} {hi:?}");
        // Cabeça: 8 px de cubo, dobrada pelo scale(2): ocupa o bloco inteiro, centrada em (0,5; 0,5).
        let mut geo = Geometry::default();
        let mut sink = Sink { geo: &mut geo, tex: (64.0, 32.0), tint: [1.0; 3], alpha: 255, light: FULL_LIGHT, lights: crate::models::WORLD_LIGHTS };
        emit_item(&EntityItem::Skull { skull_type: 0, owner: None }, &mut sink, &Mat4::IDENTITY);
        let (lo, hi) = bounds(&geo);
        assert!((hi[0] - lo[0] - 1.0).abs() < 1e-4 && (hi[1] - lo[1] - 1.0).abs() < 1e-4, "{lo:?} {hi:?}");
        assert!((lo[0] + hi[0] - 1.0).abs() < 1e-4 && (lo[2] + hi[2] - 1.0).abs() < 1e-4, "{lo:?} {hi:?}");
    }

    #[test]
    fn christmas_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19358), (2023, 1, 1));
        assert_eq!(civil_from_days(19723 + 359), (2024, 12, 25));
    }
}
