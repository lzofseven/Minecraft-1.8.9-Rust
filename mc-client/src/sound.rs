//! Sons, no comportamento do `SoundHandler`/`SoundManager` da 1.8.9.
//!
//! - Os arquivos vêm da pilha de recursos (`Jar`): pacotes do jogador por cima do índice de assets
//!   do launcher (`1.8.json` + `objects/<hash>`), que é o pacote padrão.
//! - `sounds.json` diz, para cada evento, a lista de arquivos (sorteio por peso), volume, pitch,
//!   "stream" e a categoria.
//! - Tocar: volume final = clamp(volume * volume_da_entrada, 0, 1) com atenuação linear até
//!   16 blocos (16 * volume quando volume > 1), pitch = clamp(pitch * pitch_da_entrada, 0.5, 2).
//! - A decodificação e a reprodução (rodio) ficam numa thread própria; sem dispositivo de áudio
//!   o jogo segue mudo.

use std::collections::{HashMap, VecDeque};
use std::io::Cursor;
use std::sync::{mpsc, Arc};
use std::time::Duration;

use glam::DVec3;
use mc_protocol::Reader;
use mc_render::Jar;
use mc_world::World;
use rand::Rng;

use crate::physics::Player;

/// Canais "normais" do paulscode SoundSystem usado pela 1.8 (28 normais + 4 de streaming).
pub const MAX_VOICES: usize = 28;
/// Distância de atenuação para volume <= 1.
pub const ATTENUATION_RANGE: f32 = 16.0;

// ---------------------------------------------------------------------------------------------
// sounds.json e índice de assets
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum EntryKind {
    /// Caminho dentro de `sounds/` sem extensão, ex. "random/click".
    File(String),
    /// Referência a outro evento (`"type": "event"`).
    Event(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub kind: EntryKind,
    pub volume: f32,
    pub pitch: f32,
    pub weight: u32,
    pub stream: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SoundEvent {
    pub category: String,
    pub entries: Vec<Entry>,
    /// `"replace": true` no `sounds.json`: troca o evento em vez de somar a ele.
    pub replace: bool,
}

/// Resultado do sorteio de um evento.
#[derive(Clone, Debug, PartialEq)]
pub struct Picked {
    /// Chave no índice, ex. "minecraft/sounds/random/click.ogg".
    pub key: String,
    pub volume: f32,
    pub pitch: f32,
    pub stream: bool,
}

/// Tira o domínio "minecraft:" dos nomes de evento (como o `ResourceLocation`).
fn event_name(name: &str) -> &str {
    name.strip_prefix("minecraft:").unwrap_or(name)
}

/// Lê o `sounds.json` seguindo as regras do `SoundListSerializer`.
pub fn parse_sounds_json(text: &str) -> Result<HashMap<String, SoundEvent>, String> {
    let root: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("sounds.json: {e}"))?;
    let obj = root.as_object().ok_or("sounds.json não é um objeto")?;
    let mut out = HashMap::with_capacity(obj.len());
    for (name, ev) in obj {
        let category = ev.get("category").and_then(|c| c.as_str()).unwrap_or("master").to_string();
        let mut entries = Vec::new();
        for s in ev.get("sounds").and_then(|s| s.as_array()).into_iter().flatten() {
            let mut e = Entry { kind: EntryKind::File(String::new()), volume: 1.0, pitch: 1.0, weight: 1, stream: false };
            let file_name = if let Some(n) = s.as_str() {
                n.to_string()
            } else {
                let n = s.get("name").and_then(|n| n.as_str()).ok_or_else(|| format!("{name}: entrada sem nome"))?;
                if let Some(v) = s.get("volume").and_then(|v| v.as_f64()) {
                    e.volume = v as f32;
                }
                if let Some(v) = s.get("pitch").and_then(|v| v.as_f64()) {
                    e.pitch = v as f32;
                }
                if let Some(v) = s.get("weight").and_then(|v| v.as_u64()) {
                    e.weight = v as u32;
                }
                e.stream = s.get("stream").and_then(|v| v.as_bool()).unwrap_or(false);
                if s.get("type").and_then(|t| t.as_str()) == Some("event") {
                    e.kind = EntryKind::Event(n.to_string());
                    entries.push(e);
                    continue;
                }
                n.to_string()
            };
            e.kind = EntryKind::File(file_name);
            entries.push(e);
        }
        let replace = ev.get("replace").and_then(|r| r.as_bool()).unwrap_or(false);
        out.insert(name.clone(), SoundEvent { category, entries, replace });
    }
    Ok(out)
}

/// Chave no índice para um arquivo de som ("random/click" -> "minecraft/sounds/random/click.ogg").
pub fn file_key(name: &str) -> String {
    let (domain, path) = name.split_once(':').unwrap_or(("minecraft", name));
    format!("{domain}/sounds/{path}.ogg")
}

pub struct SoundAssets {
    pub events: HashMap<String, SoundEvent>,
}

/// `SoundHandler.loadSoundResource`: evento novo, ou com `replace`, entra no lugar; senão soma entradas.
pub fn merge_events(base: &mut HashMap<String, SoundEvent>, add: HashMap<String, SoundEvent>) {
    for (name, ev) in add {
        match base.get_mut(&name) {
            Some(old) if !ev.replace => old.entries.extend(ev.entries),
            _ => {
                base.insert(name, ev);
            }
        }
    }
}

impl SoundAssets {
    /// `SoundHandler.onResourceManagerReload`: junta o `sounds.json` de todos os pacotes, do padrão ao topo.
    pub fn load(jar: &mut Jar) -> Result<Self, String> {
        let files = jar.all("assets/minecraft/sounds.json");
        if files.is_empty() {
            return Err("sounds.json não encontrado nos recursos".into());
        }
        let mut events = HashMap::new();
        for (i, f) in files.iter().enumerate() {
            match parse_sounds_json(&String::from_utf8_lossy(f)) {
                Ok(add) => merge_events(&mut events, add),
                Err(e) if i == 0 => return Err(e),
                Err(e) => eprintln!("sounds.json de pacote ignorado: {e}"),
            }
        }
        Ok(Self { events })
    }

    /// Peso de uma entrada: o próprio, ou a soma do evento referenciado.
    fn weight(&self, e: &Entry, depth: u8) -> u32 {
        match &e.kind {
            EntryKind::File(_) => e.weight,
            EntryKind::Event(n) if depth < 8 => {
                self.events.get(event_name(n)).map_or(0, |ev| ev.entries.iter().map(|x| self.weight(x, depth + 1)).sum())
            }
            EntryKind::Event(_) => 0,
        }
    }

    /// Sorteia o arquivo de um evento pelo peso (`SoundEventAccessorComposite.cloneEntry`).
    pub fn pick(&self, name: &str, rng: &mut impl Rng) -> Option<Picked> {
        self.pick_at(event_name(name), rng, 0)
    }

    fn pick_at(&self, name: &str, rng: &mut impl Rng, depth: u8) -> Option<Picked> {
        let ev = self.events.get(name)?;
        let total: u32 = ev.entries.iter().map(|e| self.weight(e, depth)).sum();
        if total == 0 || depth >= 8 {
            return None;
        }
        let mut r = rng.gen_range(0..total) as i64;
        for e in &ev.entries {
            r -= self.weight(e, depth) as i64;
            if r < 0 {
                return match &e.kind {
                    EntryKind::File(f) => Some(Picked { key: file_key(f), volume: e.volume, pitch: e.pitch, stream: e.stream }),
                    EntryKind::Event(n) => self.pick_at(event_name(n), rng, depth + 1),
                };
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------------------------
// Volume, pitch e posição
// ---------------------------------------------------------------------------------------------

/// Atenuação linear do paulscode: 1 no ouvinte, 0 a `16 * max(volume, 1)` blocos.
pub fn attenuation(distance: f32, volume: f32) -> f32 {
    let range = ATTENUATION_RANGE * volume.max(1.0);
    (1.0 - distance / range).clamp(0.0, 1.0)
}

/// `SoundManager.getNormalizedVolume` (categorias no volume padrão 1).
pub fn normalized_volume(volume: f32, entry_volume: f32) -> f32 {
    (volume * entry_volume).clamp(0.0, 1.0)
}

/// `SoundManager.getNormalizedPitch`.
pub fn normalized_pitch(pitch: f32, entry_pitch: f32) -> f32 {
    (pitch * entry_pitch).clamp(0.5, 2.0)
}

/// Lado do som em relação ao ouvinte: -1 esquerda, +1 direita (yaw no padrão do Minecraft).
pub fn pan(listener: DVec3, yaw: f32, src: DVec3) -> f32 {
    let d = src - listener;
    let h = (d.x * d.x + d.z * d.z).sqrt();
    if h < 1e-6 {
        return 0.0;
    }
    let r = (yaw as f64).to_radians();
    // Frente = (-sin, 0, cos); direita = frente x cima = (-cos, 0, -sin).
    let right = -r.cos() * d.x - r.sin() * d.z;
    (right / h).clamp(-1.0, 1.0) as f32
}

/// Ganho de cada lado: o som nunca some de um ouvido (como o panning do OpenAL em estéreo).
pub fn stereo_gains(gain: f32, pan: f32) -> (f32, f32) {
    (gain * (1.0 - 0.6 * pan.max(0.0)), gain * (1.0 + 0.6 * pan.min(0.0)))
}

// ---------------------------------------------------------------------------------------------
// Tipos de som dos blocos (Block.soundTypeXXX)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundType {
    /// `soundTypeStone` e `soundTypePiston` (iguais no som).
    Stone,
    Wood,
    Gravel,
    Grass,
    /// "stone" com pitch 1.5.
    Metal,
    /// "stone", mas quebra com "dig.glass".
    Glass,
    Cloth,
    Sand,
    Snow,
    /// "ladder", quebra com "dig.wood".
    Ladder,
    /// "anvil" com volume 0.3, quebra com "dig.stone".
    Anvil,
    Slime,
}

impl SoundType {
    pub fn volume(self) -> f32 {
        if self == SoundType::Anvil {
            0.3
        } else {
            1.0
        }
    }

    pub fn frequency(self) -> f32 {
        if self == SoundType::Metal {
            1.5
        } else {
            1.0
        }
    }

    fn name(self) -> &'static str {
        match self {
            SoundType::Stone | SoundType::Metal | SoundType::Glass => "stone",
            SoundType::Wood => "wood",
            SoundType::Gravel => "gravel",
            SoundType::Grass => "grass",
            SoundType::Cloth => "cloth",
            SoundType::Sand => "sand",
            SoundType::Snow => "snow",
            SoundType::Ladder => "ladder",
            SoundType::Anvil => "anvil",
            SoundType::Slime => "slime",
        }
    }

    pub fn step_sound(self) -> String {
        match self {
            SoundType::Slime => "mob.slime.small".into(),
            t => format!("step.{}", t.name()),
        }
    }

    pub fn break_sound(self) -> String {
        match self {
            SoundType::Glass => "dig.glass".into(),
            SoundType::Ladder => "dig.wood".into(),
            SoundType::Anvil => "dig.stone".into(),
            SoundType::Slime => "mob.slime.big".into(),
            t => format!("dig.{}", t.name()),
        }
    }

    /// `SoundType.getPlaceSound`.
    pub fn place_sound(self) -> String {
        match self {
            SoundType::Glass => "step.stone".into(),
            SoundType::Anvil => "random.anvil_land".into(),
            t => t.break_sound(),
        }
    }
}

/// Tipo de som de cada id de bloco, como em `Block.registerBlocks` (padrão do `Block`: pedra).
/// `None` para o ar.
pub fn block_sound(id: u16) -> Option<SoundType> {
    use SoundType::*;
    Some(match id {
        0 => return None,
        2 | 6 | 18 | 19 | 31 | 32 | 37..=40 | 46 | 59 | 83 | 106 | 110 | 111 | 141 | 142 | 161 | 170 | 175 => Grass,
        3 | 13 | 60 | 82 => Gravel,
        5 | 17 | 26 | 47 | 50 | 53 | 54 | 58 | 63 | 64 | 68 | 69 | 72 | 75 | 76 | 85 | 86 | 91 | 93 | 94 | 96 => Wood,
        99 | 100 | 103..=105 | 107 | 125..=127 | 134..=136 | 143 | 146..=151 | 162..=164 | 176..=178 | 183..=197 => Wood,
        12 | 88 => Sand,
        20 | 79 | 89 | 90 | 95 | 102 | 120 | 123 | 124 | 160 | 169 | 174 => Glass,
        27 | 28 | 41 | 42 | 52 | 57 | 66 | 71 | 101 | 133 | 152 | 154 | 157 | 167 => Metal,
        35 | 51 | 81 | 92 | 171 => Cloth,
        65 => Ladder,
        78 | 80 => Snow,
        145 => Anvil,
        165 => Slime,
        _ => Stone,
    })
}

fn is_liquid(id: u16) -> bool {
    (8..=11).contains(&id)
}

/// Cercas, muros e portões: contam como o bloco pisado quando o de baixo dos pés é ar.
fn is_fence_like(id: u16) -> bool {
    matches!(id, 85 | 107 | 113 | 139 | 183..=192)
}

// ---------------------------------------------------------------------------------------------
// Pacotes 0x29 (Named Sound Effect) e 0x28 (Effect)
// ---------------------------------------------------------------------------------------------

/// Um som posicionado vindo do servidor.
#[derive(Clone, Debug, PartialEq)]
pub struct SoundEffect {
    pub name: String,
    pub pos: DVec3,
    pub volume: f32,
    pub pitch: f32,
}

/// 0x29: nome, x/y/z inteiros * 8, volume float, pitch byte / 63.
pub fn parse_named_sound(body: &[u8]) -> mc_protocol::Result<SoundEffect> {
    let mut r = Reader::new(body);
    let name = r.string()?;
    let (x, y, z) = (r.i32()?, r.i32()?, r.i32()?);
    let volume = r.f32()?;
    let pitch = r.u8()? as f32 / 63.0;
    Ok(SoundEffect { name, pos: DVec3::new(x as f64, y as f64, z as f64) / 8.0, volume, pitch })
}

/// Som de um efeito do 0x28 (`RenderGlobal.playAuxSFX`), só os casos de som simples.
pub fn effect_sound(kind: i32, (x, y, z): (i32, i32, i32), data: i32, rng: &mut impl Rng) -> Option<SoundEffect> {
    let pos = DVec3::new(x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5);
    let mut wobble = || (rng.gen::<f32>() - rng.gen::<f32>()) * 0.2 + 1.0;
    let (name, volume, pitch): (String, f32, f32) = match kind {
        1000 => ("random.click".into(), 1.0, 1.0),
        1001 => ("random.click".into(), 1.0, 1.2),
        1002 => ("random.bow".into(), 1.0, 1.2),
        1003 => ("random.door_open".into(), 1.0, rng.gen::<f32>() * 0.1 + 0.9),
        1004 => ("random.fizz".into(), 0.5, 2.6 + (rng.gen::<f32>() - rng.gen::<f32>()) * 0.8),
        1006 => ("random.door_close".into(), 1.0, rng.gen::<f32>() * 0.1 + 0.9),
        1007 => ("mob.ghast.charge".into(), 10.0, wobble()),
        1008 => ("mob.ghast.fireball".into(), 10.0, wobble()),
        1009 => ("mob.ghast.fireball".into(), 2.0, wobble()),
        1010 => ("mob.zombie.wood".into(), 2.0, wobble()),
        1011 => ("mob.zombie.metal".into(), 2.0, wobble()),
        1012 => ("mob.zombie.woodbreak".into(), 2.0, wobble()),
        1014 => ("mob.wither.shoot".into(), 2.0, wobble()),
        1015 => ("mob.bat.takeoff".into(), 0.05, wobble()),
        1016 => ("mob.zombie.infect".into(), 2.0, wobble()),
        1017 => ("mob.zombie.unfect".into(), 2.0, wobble()),
        1020 => ("random.anvil_break".into(), 1.0, rng.gen::<f32>() * 0.1 + 0.9),
        1021 => ("random.anvil_use".into(), 1.0, rng.gen::<f32>() * 0.1 + 0.9),
        1022 => ("random.anvil_land".into(), 0.3, rng.gen::<f32>() * 0.1 + 0.9),
        // Quebra de bloco: dado = id de estado (id | meta << 12).
        2001 => {
            let t = block_sound((data & 0xFFF) as u16)?;
            (t.break_sound(), (t.volume() + 1.0) / 2.0, t.frequency() * 0.8)
        }
        2002 => ("game.potion.smash".into(), 1.0, rng.gen::<f32>() * 0.1 + 0.9),
        _ => return None,
    };
    Some(SoundEffect { name, pos, volume, pitch })
}

/// 0x28: tipo, posição, dado, "para todo o servidor" (sons globais de chefes ficam de fora).
pub fn parse_effect(body: &[u8]) -> mc_protocol::Result<Option<SoundEffect>> {
    let mut r = Reader::new(body);
    let kind = r.i32()?;
    let pos = r.position()?;
    let data = r.i32()?;
    let global = r.bool()?;
    Ok(if global { None } else { effect_sound(kind, pos, data, &mut rand::thread_rng()) })
}

// ---------------------------------------------------------------------------------------------
// Passos e queda do próprio jogador (Entity.moveEntity / EntityLivingBase.fall)
// ---------------------------------------------------------------------------------------------

/// Um som pedido pelo cliente: (evento, volume, pitch).
pub type LocalSound = (String, f32, f32);

pub struct StepTracker {
    walked: f32,
    next_step: i32,
    fall_distance: f32,
}

impl Default for StepTracker {
    fn default() -> Self {
        Self { walked: 0.0, next_step: 1, fall_distance: 0.0 }
    }
}

fn id_at(w: &World, x: i32, y: i32, z: i32) -> u16 {
    w.block(x, y, z) >> 4
}

impl StepTracker {
    /// Depois do tick de física: devolve os sons de passo, nado e queda deste tick.
    pub fn tick(&mut self, w: &World, p: &Player, rng: &mut impl Rng) -> Vec<LocalSound> {
        let mut out = Vec::new();
        let d = p.pos - p.prev;
        let (bx, bz) = (p.pos.x.floor() as i32, p.pos.z.floor() as i32);
        let mut by = (p.pos.y - 0.2).floor() as i32;
        let mut id = id_at(w, bx, by, bz);
        if id == 0 && is_fence_like(id_at(w, bx, by - 1, bz)) {
            by -= 1;
            id = id_at(w, bx, by, bz);
        }
        let feet = id_at(w, bx, p.pos.y.floor() as i32, bz);
        let in_water = [0.4, 1.4].iter().any(|dy| matches!(id_at(w, bx, (p.pos.y + dy).floor() as i32, bz), 8 | 9));

        // Queda: zera na água, em escada/trepadeira e voando; ao tocar o chão, toca o som se feriu.
        if in_water || p.flying || matches!(feet, 65 | 106) {
            self.fall_distance = 0.0;
        }
        if p.on_ground {
            if self.fall_distance > 0.0 {
                // Slime amortece tudo, exceto agachado.
                let mult = if id == 165 && !p.sneaking { 0.0 } else { 1.0 };
                let dmg = ((self.fall_distance - 3.0) * mult).ceil() as i32;
                if dmg > 0 && !p.allow_flying {
                    let name = if dmg > 4 { "game.player.hurt.fall.big" } else { "game.player.hurt.fall.small" };
                    out.push((name.to_string(), 1.0, 1.0));
                    if let Some(t) = block_sound(id) {
                        out.push((t.step_sound(), t.volume() * 0.5, t.frequency() * 0.75));
                    }
                }
                self.fall_distance = 0.0;
            }
        } else if d.y < 0.0 {
            self.fall_distance -= d.y as f32;
        }

        // Passos: não voando e não agachado no chão.
        if p.flying || (p.on_ground && p.sneaking) {
            return out;
        }
        let dy = if id == 65 { d.y } else { 0.0 };
        self.walked += ((d.x * d.x + dy * dy + d.z * d.z).sqrt() * 0.6) as f32;
        if self.walked > self.next_step as f32 && id != 0 {
            self.next_step = self.walked as i32 + 1;
            if in_water {
                let v = &p.vel;
                let f = ((v.x * v.x * 0.2 + v.y * v.y + v.z * v.z * 0.2).sqrt() as f32 * 0.35).min(1.0);
                out.push(("game.player.swim".into(), f, 1.0 + (rng.gen::<f32>() - rng.gen::<f32>()) * 0.4));
            }
            if id_at(w, bx, by + 1, bz) == 78 {
                out.push((SoundType::Snow.step_sound(), 0.15, 1.0));
            } else if !is_liquid(id) {
                if let Some(t) = block_sound(id) {
                    out.push((t.step_sound(), t.volume() * 0.15, t.frequency()));
                }
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------
// Reprodução
// ---------------------------------------------------------------------------------------------

struct Pcm {
    channels: u16,
    rate: u32,
    data: Vec<f32>,
}

fn decode(bytes: Vec<u8>) -> Result<Pcm, String> {
    use rodio::Source;
    let d = rodio::Decoder::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let (channels, rate) = (d.channels(), d.sample_rate());
    Ok(Pcm { channels, rate, data: d.convert_samples::<f32>().collect() })
}

/// Fonte estéreo com ganho por lado sobre amostras já decodificadas (compartilhadas pelo cache).
struct Voice {
    pcm: Arc<Pcm>,
    frame: usize,
    right: bool,
    gains: (f32, f32),
}

impl Iterator for Voice {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let c = self.pcm.channels.max(1) as usize;
        let i = self.frame * c + if self.right { 1.min(c - 1) } else { 0 };
        let s = *self.pcm.data.get(i)?;
        let g = if self.right { self.gains.1 } else { self.gains.0 };
        if self.right {
            self.frame += 1;
        }
        self.right = !self.right;
        Some(s * g)
    }
}

impl rodio::Source for Voice {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        self.pcm.rate
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

enum Cmd {
    Play(PlayCmd),
    /// Recarga de recursos: esquece os sons decodificados.
    Clear,
}

struct PlayCmd {
    /// Chave do som, ex. "minecraft/sounds/random/click.ogg".
    key: String,
    cache: bool,
    gains: (f32, f32),
    pitch: f32,
}

fn audio_thread(rx: mpsc::Receiver<Cmd>, mut jar: Jar) {
    use rodio::Source;
    let Ok((_stream, handle)) = rodio::OutputStream::try_default() else {
        eprintln!("sem dispositivo de áudio: jogo sem som");
        return;
    };
    let mut cache: HashMap<String, Arc<Pcm>> = HashMap::new();
    let mut voices: VecDeque<rodio::Sink> = VecDeque::new();
    for cmd in rx {
        let cmd = match cmd {
            Cmd::Play(c) => c,
            Cmd::Clear => {
                cache.clear();
                continue;
            }
        };
        let pcm = match cache.get(&cmd.key) {
            Some(p) => p.clone(),
            None => match jar.bytes(&format!("assets/{}", cmd.key)).ok_or_else(|| "arquivo não encontrado".to_string()).and_then(decode) {
                Ok(p) => {
                    let p = Arc::new(p);
                    if cmd.cache {
                        cache.insert(cmd.key.clone(), p.clone());
                    }
                    p
                }
                Err(e) => {
                    eprintln!("som {}: {e}", cmd.key);
                    continue;
                }
            },
        };
        voices.retain(|s| !s.empty());
        if voices.len() >= MAX_VOICES {
            // Como o paulscode: sem canal livre, o mais antigo dá lugar ao novo.
            if let Some(old) = voices.pop_front() {
                old.stop();
            }
        }
        let Ok(sink) = rodio::Sink::try_new(&handle) else { continue };
        sink.append(Voice { pcm, frame: 0, right: false, gains: cmd.gains }.speed(cmd.pitch));
        voices.push_back(sink);
    }
}

/// Sistema de som do cliente. Sem assets ou sem áudio, todas as chamadas viram nada.
pub struct Sound {
    tx: Option<mpsc::Sender<Cmd>>,
    assets: Option<SoundAssets>,
    listener: DVec3,
    yaw: f32,
    steps: StepTracker,
    /// Volumes do options.txt (ordem de `settings::SOUND_CATEGORIES`; 0 = master).
    volumes: [f32; 9],
}

impl Sound {
    /// Carrega os assets e abre o áudio numa thread própria.
    pub fn start(mut jar: Jar) -> Self {
        let assets = match SoundAssets::load(&mut jar) {
            Ok(a) => {
                println!("{} eventos de som carregados", a.events.len());
                Some(a)
            }
            Err(e) => {
                eprintln!("sons indisponíveis ({e}); jogo sem som");
                None
            }
        };
        let tx = assets.as_ref().and_then(|_| {
            let (tx, rx) = mpsc::channel();
            std::thread::Builder::new().name("som".into()).spawn(move || audio_thread(rx, jar)).ok().map(|_| tx)
        });
        Self { tx, assets, listener: DVec3::ZERO, yaw: 0.0, steps: StepTracker::default(), volumes: [1.0; 9] }
    }

    /// Recarga de recursos: relê o `sounds.json` de todos os pacotes e esquece os sons decodificados.
    pub fn reload(&mut self, jar: &mut Jar) {
        match SoundAssets::load(jar) {
            Ok(a) => self.assets = Some(a),
            Err(e) => eprintln!("sons: {e}"),
        }
        if let Some(tx) = &self.tx {
            let _ = tx.send(Cmd::Clear);
        }
    }

    /// Posição dos ouvidos (olhos do jogador) e para onde ele olha.
    pub fn set_listener(&mut self, eye: DVec3, yaw: f32) {
        self.listener = eye;
        self.yaw = yaw;
    }

    /// Volumes por categoria (`GameSettings.setSoundLevel`).
    pub fn set_volumes(&mut self, volumes: [f32; 9]) {
        self.volumes = volumes;
    }

    fn submit(&self, name: &str, volume: f32, pitch: f32, at: Option<DVec3>) {
        let (Some(tx), Some(a)) = (&self.tx, &self.assets) else { return };
        let Some(pick) = a.pick(name, &mut rand::thread_rng()) else { return };
        // SoundManager.getNormalizedVolume: volume * entrada * categoria; o master é o ganho do ouvinte.
        let category = a.events.get(name).map(|e| e.category.as_str()).unwrap_or("master");
        let cat = crate::settings::SOUND_CATEGORIES.iter().position(|c| *c == category).filter(|&i| i != 0).map_or(1.0, |i| self.volumes[i]);
        let mut gain = normalized_volume(volume * cat, pick.volume) * self.volumes[0];
        let mut side = 0.0;
        if let Some(pos) = at {
            gain *= attenuation(pos.distance(self.listener) as f32, volume);
            side = pan(self.listener, self.yaw, pos);
        }
        if gain <= 0.0 {
            return;
        }
        let pitch = normalized_pitch(pitch, pick.pitch);
        let _ = tx.send(Cmd::Play(PlayCmd { key: pick.key, cache: !pick.stream, gains: stereo_gains(gain, side), pitch }));
    }

    /// Som no mundo (`PositionedSoundRecord` com atenuação linear).
    pub fn play(&self, name: &str, volume: f32, pitch: f32, pos: DVec3) {
        self.submit(name, volume, pitch, Some(pos));
    }

    /// Som sem posição (interface).
    pub fn play_ui(&self, name: &str, volume: f32, pitch: f32) {
        self.submit(name, volume, pitch, None);
    }

    /// Clique de botão (`GuiButton.playPressSound`: volume 0.25, pitch 1). Para a interface usar.
    #[allow(dead_code)]
    pub fn button_click(&self) {
        self.play_ui("gui.button.press", 0.25, 1.0);
    }

    /// Chamar depois do tick de física: atualiza o ouvinte e toca passos/queda do jogador.
    pub fn player_tick(&mut self, w: &World, p: &Player) {
        self.set_listener(p.pos + DVec3::new(0.0, p.eye_height(), 0.0), p.yaw);
        if self.tx.is_none() {
            return;
        }
        for (name, volume, pitch) in self.steps.tick(w, p, &mut rand::thread_rng()) {
            self.play(&name, volume, pitch, p.pos);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named_sound_bytes(name: &str, x: i32, y: i32, z: i32, vol: f32, pitch: u8) -> Vec<u8> {
        let mut b = vec![name.len() as u8];
        b.extend_from_slice(name.as_bytes());
        for v in [x, y, z] {
            b.extend_from_slice(&v.to_be_bytes());
        }
        b.extend_from_slice(&vol.to_be_bytes());
        b.push(pitch);
        b
    }

    #[test]
    fn parse_0x29() {
        let b = named_sound_bytes("mob.villager.idle", 100 * 8 + 4, 64 * 8, -10 * 8 - 4, 0.8, 63);
        let s = parse_named_sound(&b).unwrap();
        assert_eq!(s.name, "mob.villager.idle");
        assert_eq!(s.pos, DVec3::new(100.5, 64.0, -10.5));
        assert_eq!(s.volume, 0.8);
        assert_eq!(s.pitch, 1.0);
        let b = named_sound_bytes("random.click", 0, 0, 0, 1.0, 126);
        assert_eq!(parse_named_sound(&b).unwrap().pitch, 2.0);
        // Pacote cortado é erro, não pânico.
        assert!(parse_named_sound(&b[..b.len() - 1]).is_err());
    }

    #[test]
    fn parse_0x28_break_block() {
        let mut b = 2001i32.to_be_bytes().to_vec();
        let (x, y, z) = (10i64, 70i64, -3i64);
        let packed = ((x & 0x3FFFFFF) << 38) | ((y & 0xFFF) << 26) | (z & 0x3FFFFFF);
        b.extend_from_slice(&packed.to_be_bytes());
        b.extend_from_slice(&20i32.to_be_bytes()); // vidro, meta 0
        b.push(0);
        let s = parse_effect(&b).unwrap().unwrap();
        assert_eq!(s.name, "dig.glass");
        assert_eq!(s.pos, DVec3::new(10.5, 70.5, -2.5));
        assert_eq!((s.volume, s.pitch), (1.0, 0.8));
        // Global (som de chefe) e tipos sem som: nada.
        *b.last_mut().unwrap() = 1;
        assert!(parse_effect(&b).unwrap().is_none());
        let mut rng = rand::thread_rng();
        assert!(effect_sound(2000, (0, 0, 0), 0, &mut rng).is_none());
        assert_eq!(effect_sound(1003, (0, 0, 0), 0, &mut rng).unwrap().name, "random.door_open");
        // Bigorna: volume (0.3 + 1) / 2.
        assert_eq!(effect_sound(2001, (0, 0, 0), 145, &mut rng).unwrap().volume, 0.65);
    }

    #[test]
    fn volume_by_distance() {
        assert_eq!(attenuation(0.0, 1.0), 1.0);
        assert_eq!(attenuation(8.0, 1.0), 0.5);
        assert_eq!(attenuation(16.0, 1.0), 0.0);
        assert_eq!(attenuation(40.0, 1.0), 0.0);
        // Volume < 1 não encolhe o alcance; volume > 1 aumenta (trovão, ghast).
        assert_eq!(attenuation(8.0, 0.15), 0.5);
        assert_eq!(attenuation(80.0, 10.0), 0.5);
        assert_eq!(normalized_volume(4.0, 1.0), 1.0);
        assert_eq!(normalized_volume(0.15, 0.5), 0.075);
        assert_eq!(normalized_pitch(3.0, 1.0), 2.0);
        assert_eq!(normalized_pitch(0.1, 1.0), 0.5);
        assert_eq!(normalized_pitch(1.2, 1.0), 1.2);
    }

    #[test]
    fn panning() {
        let o = DVec3::ZERO;
        // yaw 0 olha para +Z: a direita é -X.
        assert!((pan(o, 0.0, DVec3::new(-5.0, 0.0, 0.0)) - 1.0).abs() < 1e-6);
        assert!((pan(o, 0.0, DVec3::new(5.0, 0.0, 0.0)) + 1.0).abs() < 1e-6);
        assert!(pan(o, 0.0, DVec3::new(0.0, 0.0, 5.0)).abs() < 1e-6);
        // yaw 90 olha para -X: a direita é -Z.
        assert!((pan(o, 90.0, DVec3::new(0.0, 0.0, -5.0)) - 1.0).abs() < 1e-6);
        assert_eq!(pan(o, 0.0, DVec3::new(0.0, 3.0, 0.0)), 0.0);
        assert_eq!(stereo_gains(1.0, 0.0), (1.0, 1.0));
        let (l, r) = stereo_gains(1.0, 1.0);
        assert!(r == 1.0 && (l - 0.4).abs() < 1e-6);
    }

    #[test]
    fn material_table() {
        assert_eq!(block_sound(0), None);
        assert_eq!(block_sound(1), Some(SoundType::Stone));
        assert_eq!(block_sound(2).unwrap().step_sound(), "step.grass");
        assert_eq!(block_sound(3).unwrap().step_sound(), "step.gravel"); // terra
        assert_eq!(block_sound(5).unwrap().step_sound(), "step.wood");
        assert_eq!(block_sound(12).unwrap().step_sound(), "step.sand");
        assert_eq!(block_sound(17), Some(SoundType::Wood)); // tronco, no construtor
        assert_eq!(block_sound(18), Some(SoundType::Grass)); // folhas, no construtor
        assert_eq!(block_sound(20).unwrap().break_sound(), "dig.glass");
        assert_eq!(block_sound(20).unwrap().step_sound(), "step.stone");
        assert_eq!(block_sound(35).unwrap().step_sound(), "step.cloth");
        let iron = block_sound(42).unwrap();
        assert_eq!((iron.step_sound().as_str(), iron.frequency()), ("step.stone", 1.5));
        assert_eq!(block_sound(53), Some(SoundType::Wood)); // escada de carvalho herda a madeira
        assert_eq!(block_sound(67), Some(SoundType::Stone));
        assert_eq!(block_sound(65).unwrap().step_sound(), "step.ladder");
        assert_eq!(block_sound(65).unwrap().break_sound(), "dig.wood");
        assert_eq!(block_sound(78).unwrap().step_sound(), "step.snow");
        assert_eq!(block_sound(88), Some(SoundType::Sand)); // areia de almas
        assert_eq!(block_sound(145).unwrap().volume(), 0.3);
        assert_eq!(block_sound(145).unwrap().place_sound(), "random.anvil_land");
        assert_eq!(block_sound(165).unwrap().step_sound(), "mob.slime.small");
        assert_eq!(block_sound(171), Some(SoundType::Cloth)); // tapete
        assert_eq!(block_sound(197), Some(SoundType::Wood));
        // Todos os ids da 1.8 têm um tipo.
        assert!((1..=197).all(|id| block_sound(id).is_some()));
    }

    #[test]
    fn sounds_json_rules() {
        let j = r#"{
            "a": {"category": "block", "sounds": ["x/one", {"name": "x/two", "volume": 0.5, "pitch": 1.5, "weight": 3}]},
            "b": {"sounds": [{"name": "a", "type": "event"}, {"name": "m/song", "stream": true}]}
        }"#;
        let ev = parse_sounds_json(j).unwrap();
        assert_eq!(ev["a"].category, "block");
        assert_eq!(ev["b"].category, "master");
        assert_eq!(ev["a"].entries[1], Entry { kind: EntryKind::File("x/two".into()), volume: 0.5, pitch: 1.5, weight: 3, stream: false });
        assert_eq!(ev["b"].entries[0].kind, EntryKind::Event("a".into()));
        assert!(ev["b"].entries[1].stream);
        let a = SoundAssets { events: ev };
        // O peso do evento "a" (1 + 3) entra no sorteio de "b".
        assert_eq!(a.weight(&a.events["b"].entries[0], 0), 4);
        let mut rng = rand::thread_rng();
        let mut seen = HashMap::new();
        for _ in 0..2000 {
            *seen.entry(a.pick("minecraft:b", &mut rng).unwrap().key).or_insert(0) += 1;
        }
        assert_eq!(seen.len(), 3);
        assert!(seen["minecraft/sounds/x/two.ogg"] > seen["minecraft/sounds/x/one.ogg"]);
        assert!(a.pick("nada", &mut rng).is_none());
    }

    #[test]
    fn pack_sounds_json_merges_or_replaces() {
        let base = r#"{"a": {"category": "block", "sounds": ["x/one"]}, "b": {"sounds": ["y"]}}"#;
        let pack = r#"{"a": {"sounds": ["x/two"]}, "b": {"replace": true, "sounds": ["z"]}, "c": {"sounds": ["w"]}}"#;
        let mut ev = parse_sounds_json(base).unwrap();
        merge_events(&mut ev, parse_sounds_json(pack).unwrap());
        let names = |n: &str| ev[n].entries.iter().map(|e| e.kind.clone()).collect::<Vec<_>>();
        assert_eq!(names("a"), vec![EntryKind::File("x/one".into()), EntryKind::File("x/two".into())]);
        assert_eq!(ev["a"].category, "block");
        assert_eq!(names("b"), vec![EntryKind::File("z".into())]);
        assert_eq!(names("c"), vec![EntryKind::File("w".into())]);
    }

    #[test]
    #[ignore = "precisa dos assets da 1.8 instalados (Prism Launcher ou MINE_RS_ASSETS)"]
    fn real_assets() {
        let home = std::env::var("HOME").unwrap_or_default();
        let mut jar = Jar::open(std::path::Path::new(&format!("{home}/.local/share/PrismLauncher/libraries/com/mojang/minecraft/1.8.9/minecraft-1.8.9-client.jar"))).expect("jar");
        let a = SoundAssets::load(&mut jar).expect("assets");
        assert!(a.events.len() > 200, "{} eventos", a.events.len());
        for ev in ["random.click", "dig.stone", "step.grass", "mob.villager.idle", "note.harp", "gui.button.press"] {
            assert!(a.events.contains_key(ev), "falta {ev}");
        }
        assert_eq!(a.events["step.grass"].category, "neutral");
        let mut rng = rand::thread_rng();
        // Todo arquivo de todo evento existe no índice e no disco.
        let mut files = 0;
        for ev in a.events.values() {
            for e in &ev.entries {
                if let EntryKind::File(f) = &e.kind {
                    assert!(jar.has(&format!("assets/{}", file_key(f))), "{f} fora do índice");
                    files += 1;
                }
            }
        }
        assert!(files > 500, "{files} arquivos");
        // Um passo de grama decodifica.
        let pick = a.pick("step.grass", &mut rng).unwrap();
        let pcm = decode(jar.bytes(&format!("assets/{}", pick.key)).unwrap()).unwrap();
        assert!(pcm.rate > 0 && pcm.channels > 0 && !pcm.data.is_empty());
        println!("{} eventos, {} arquivos, passo: {} amostras a {} Hz", a.events.len(), files, pcm.data.len(), pcm.rate);
    }
}
