//! Pacotes de entidades do protocolo 47: spawn, movimento, metadados (DataWatcher),
//! equipamento, animação e lista de jogadores.
//!
//! Posições chegam em ponto fixo (x32) e ângulos em byte (256 = volta inteira);
//! a conversão fica para o estado (`entities.rs`), como no cliente original.

use std::fmt;

use mc_protocol::{ProtocolError, Reader, Result};

/// UUID de 128 bits (mais significativo primeiro, como no fio).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, PartialOrd, Ord)]
pub struct Uuid(pub u128);

impl Uuid {
    pub fn from_halves(msb: u64, lsb: u64) -> Self {
        Self(((msb as u128) << 64) | lsb as u128)
    }

    /// `UUID.hashCode()` do Java (decide Steve ou Alex na skin padrão).
    pub fn java_hash(self) -> i32 {
        let hilo = (self.0 >> 64) as u64 ^ self.0 as u64;
        ((hilo >> 32) as i32) ^ (hilo as i32)
    }
}

impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let h = format!("{:032x}", self.0);
        write!(f, "{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
    }
}

/// Pilha de itens como vem no fio. A NBT fica crua.
#[derive(Clone, Debug, PartialEq)]
pub struct Slot {
    pub id: i16,
    pub count: i8,
    pub damage: i16,
    pub nbt: Option<Vec<u8>>,
}

/// Valor de metadado (tipos 0 a 7 do DataWatcher).
#[derive(Clone, Debug, PartialEq)]
pub enum MetaValue {
    Byte(i8),
    Short(i16),
    Int(i32),
    Float(f32),
    Str(String),
    Slot(Option<Slot>),
    Pos(i32, i32, i32),
    Rot(f32, f32, f32),
}

pub type Metadata = Vec<(u8, MetaValue)>;

#[derive(Clone, Debug, PartialEq)]
pub struct ProfileProperty {
    pub name: String,
    pub value: String,
    pub signature: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlayerListAction {
    Add {
        uuid: Uuid,
        name: String,
        properties: Vec<ProfileProperty>,
        gamemode: i32,
        ping: i32,
        /// Componente de chat em JSON.
        display_name: Option<String>,
    },
    GameMode { uuid: Uuid, gamemode: i32 },
    Latency { uuid: Uuid, ping: i32 },
    DisplayName { uuid: Uuid, display_name: Option<String> },
    Remove { uuid: Uuid },
}

#[derive(Clone, Debug, PartialEq)]
pub enum EntityPacket {
    /// Id do próprio jogador (do Join Game, 0x01).
    JoinGame { entity_id: i32 },
    /// 0x0C
    SpawnPlayer { id: i32, uuid: Uuid, x: i32, y: i32, z: i32, yaw: i8, pitch: i8, item: i16, meta: Metadata },
    /// 0x0E. `data` é o campo extra (dono da flecha, bloco que cai...); a velocidade só vem com `data > 0`.
    SpawnObject { id: i32, kind: u8, x: i32, y: i32, z: i32, pitch: i8, yaw: i8, data: i32, vel: Option<(i16, i16, i16)> },
    /// 0x0F
    SpawnMob {
        id: i32,
        kind: u8,
        x: i32,
        y: i32,
        z: i32,
        yaw: i8,
        pitch: i8,
        head_yaw: i8,
        vel: (i16, i16, i16),
        meta: Metadata,
    },
    /// 0x10 (`S10PacketSpawnPainting`): título, posição do bloco pendurado e direção horizontal (0 sul, 1 oeste, 2 norte, 3 leste).
    SpawnPainting { id: i32, title: String, pos: (i32, i32, i32), facing: u8 },
    /// 0x11 (`S11PacketSpawnExperienceOrb`): posição em ponto fixo (x32) e valor de XP.
    SpawnXpOrb { id: i32, x: i32, y: i32, z: i32, value: i16 },
    /// 0x2C (`S2CPacketSpawnGlobalEntity`): só o tipo 1 (raio) existe.
    SpawnGlobal { id: i32, kind: u8, x: i32, y: i32, z: i32 },
    /// 0x13
    Destroy(Vec<i32>),
    /// 0x14 a 0x17: deslocamento relativo (x32) e, se veio, a rotação nova.
    Move { id: i32, dx: i8, dy: i8, dz: i8, rot: Option<(i8, i8)>, on_ground: bool },
    /// 0x18
    Teleport { id: i32, x: i32, y: i32, z: i32, yaw: i8, pitch: i8, on_ground: bool },
    /// 0x19
    HeadLook { id: i32, yaw: i8 },
    /// 0x1C
    Metadata { id: i32, meta: Metadata },
    /// 0x0B: 0 balança o braço, 1 dano, 2 sai da cama, 4/5 partículas de crítico.
    Animation { id: i32, kind: u8 },
    /// 0x12, em 1/8000 de bloco por tick.
    Velocity { id: i32, vx: i16, vy: i16, vz: i16 },
    /// 0x04: 0 = mão, 1 a 4 = botas, calça, peito, capacete.
    Equipment { id: i32, slot: i16, item: Option<Slot> },
    /// 0x1A: 2 = dano, 3 = morte.
    Status { id: i32, status: i8 },
    /// 0x38
    PlayerList(Vec<PlayerListAction>),
    /// 0x1B: `vehicle` -1 = desmontou; `leash` = coleira em vez de montaria.
    Attach { entity: i32, vehicle: i32, leash: bool },
}

/// Pacotes (estado Play) que este módulo entende.
pub fn handles(id: i32) -> bool {
    matches!(id, 0x04 | 0x0B | 0x0C | 0x0E | 0x0F | 0x10 | 0x11 | 0x12 | 0x13 | 0x14..=0x19 | 0x1A | 0x1B | 0x1C | 0x2C | 0x38)
}

pub(crate) trait ReadExt {
    fn i8(&mut self) -> Result<i8>;
    fn i16(&mut self) -> Result<i16>;
    fn uuid(&mut self) -> Result<Uuid>;
}

impl ReadExt for Reader<'_> {
    fn i8(&mut self) -> Result<i8> {
        Ok(self.u8()? as i8)
    }
    fn i16(&mut self) -> Result<i16> {
        Ok(self.u16()? as i16)
    }
    fn uuid(&mut self) -> Result<Uuid> {
        Ok(Uuid::from_halves(self.i64()? as u64, self.i64()? as u64))
    }
}

/// Lê o corpo de um pacote já identificado por `handles`.
pub fn parse(id: i32, body: &[u8]) -> Result<EntityPacket> {
    let mut r = Reader::new(body);
    let r = &mut r;
    Ok(match id {
        0x04 => EntityPacket::Equipment { id: r.varint()?, slot: r.i16()?, item: read_slot(r)? },
        0x0B => EntityPacket::Animation { id: r.varint()?, kind: r.u8()? },
        0x0C => EntityPacket::SpawnPlayer {
            id: r.varint()?,
            uuid: r.uuid()?,
            x: r.i32()?,
            y: r.i32()?,
            z: r.i32()?,
            yaw: r.i8()?,
            pitch: r.i8()?,
            item: r.i16()?,
            meta: read_metadata(r)?,
        },
        0x0E => {
            let (id, kind) = (r.varint()?, r.u8()?);
            let (x, y, z) = (r.i32()?, r.i32()?, r.i32()?);
            let (pitch, yaw, data) = (r.i8()?, r.i8()?, r.i32()?);
            let vel = if data > 0 { Some((r.i16()?, r.i16()?, r.i16()?)) } else { None };
            EntityPacket::SpawnObject { id, kind, x, y, z, pitch, yaw, data, vel }
        }
        0x0F => EntityPacket::SpawnMob {
            id: r.varint()?,
            kind: r.u8()?,
            x: r.i32()?,
            y: r.i32()?,
            z: r.i32()?,
            yaw: r.i8()?,
            pitch: r.i8()?,
            head_yaw: r.i8()?,
            vel: (r.i16()?, r.i16()?, r.i16()?),
            meta: read_metadata(r)?,
        },
        0x10 => EntityPacket::SpawnPainting { id: r.varint()?, title: r.string()?, pos: r.position()?, facing: r.u8()? },
        0x11 => EntityPacket::SpawnXpOrb { id: r.varint()?, x: r.i32()?, y: r.i32()?, z: r.i32()?, value: r.i16()? },
        0x2C => EntityPacket::SpawnGlobal { id: r.varint()?, kind: r.u8()?, x: r.i32()?, y: r.i32()?, z: r.i32()? },
        0x12 => EntityPacket::Velocity { id: r.varint()?, vx: r.i16()?, vy: r.i16()?, vz: r.i16()? },
        0x13 => {
            let n = r.varint()?;
            if !(0..=65536).contains(&n) {
                return Err(ProtocolError::BadLength(n));
            }
            EntityPacket::Destroy((0..n).map(|_| r.varint()).collect::<Result<_>>()?)
        }
        0x14 => EntityPacket::Move { id: r.varint()?, dx: 0, dy: 0, dz: 0, rot: None, on_ground: false },
        0x15 => EntityPacket::Move { id: r.varint()?, dx: r.i8()?, dy: r.i8()?, dz: r.i8()?, rot: None, on_ground: r.bool()? },
        0x16 => EntityPacket::Move { id: r.varint()?, dx: 0, dy: 0, dz: 0, rot: Some((r.i8()?, r.i8()?)), on_ground: r.bool()? },
        0x17 => EntityPacket::Move {
            id: r.varint()?,
            dx: r.i8()?,
            dy: r.i8()?,
            dz: r.i8()?,
            rot: Some((r.i8()?, r.i8()?)),
            on_ground: r.bool()?,
        },
        0x18 => EntityPacket::Teleport {
            id: r.varint()?,
            x: r.i32()?,
            y: r.i32()?,
            z: r.i32()?,
            yaw: r.i8()?,
            pitch: r.i8()?,
            on_ground: r.bool()?,
        },
        0x19 => EntityPacket::HeadLook { id: r.varint()?, yaw: r.i8()? },
        0x1A => EntityPacket::Status { id: r.i32()?, status: r.i8()? },
        0x1B => EntityPacket::Attach { entity: r.i32()?, vehicle: r.i32()?, leash: r.bool()? },
        0x1C => EntityPacket::Metadata { id: r.varint()?, meta: read_metadata(r)? },
        0x38 => EntityPacket::PlayerList(read_player_list(r)?),
        _ => return Err(ProtocolError::BadLength(id)),
    })
}

/// Lista do DataWatcher: byte `tipo << 5 | índice`, valor, ..., terminada em 127.
pub fn read_metadata(r: &mut Reader) -> Result<Metadata> {
    let mut out = Vec::new();
    loop {
        let b = r.u8()?;
        if b == 127 {
            return Ok(out);
        }
        let index = b & 0x1F;
        let value = match b >> 5 {
            0 => MetaValue::Byte(r.i8()?),
            1 => MetaValue::Short(r.i16()?),
            2 => MetaValue::Int(r.i32()?),
            3 => MetaValue::Float(r.f32()?),
            4 => MetaValue::Str(r.string()?),
            5 => MetaValue::Slot(read_slot(r)?),
            6 => MetaValue::Pos(r.i32()?, r.i32()?, r.i32()?),
            _ => MetaValue::Rot(r.f32()?, r.f32()?, r.f32()?),
        };
        out.push((index, value));
    }
}

/// ItemStack: id (-1 = vazio), quantidade, dano e NBT (0 = sem NBT).
pub fn read_slot(r: &mut Reader) -> Result<Option<Slot>> {
    let id = r.i16()?;
    if id < 0 {
        return Ok(None);
    }
    let (count, damage) = (r.i8()?, r.i16()?);
    let start = r.remaining();
    let nbt = if r.u8()? == 0 {
        None
    } else {
        // Já leu o tipo (deve ser compound); falta o nome e o conteúdo.
        let tag = start[0];
        let name_len = r.u16()? as usize;
        r.bytes(name_len)?;
        skip_nbt(r, tag, 0)?;
        Some(start[..start.len() - r.remaining().len()].to_vec())
    };
    Ok(Some(Slot { id, count, damage, nbt }))
}

/// Pula o conteúdo de uma tag NBT do tipo `tag`.
fn skip_nbt(r: &mut Reader, tag: u8, depth: u32) -> Result<()> {
    if depth > 512 {
        return Err(ProtocolError::BadLength(depth as i32));
    }
    let len = |r: &mut Reader| -> Result<usize> {
        let n = r.i32()?;
        usize::try_from(n).map_err(|_| ProtocolError::BadLength(n))
    };
    match tag {
        0 => {}
        1 => drop(r.bytes(1)?),
        2 => drop(r.bytes(2)?),
        3 | 5 => drop(r.bytes(4)?),
        4 | 6 => drop(r.bytes(8)?),
        7 => {
            let n = len(r)?;
            r.bytes(n)?;
        }
        8 => {
            let n = r.u16()? as usize;
            r.bytes(n)?;
        }
        9 => {
            let inner = r.u8()?;
            for _ in 0..len(r)? {
                skip_nbt(r, inner, depth + 1)?;
            }
        }
        10 => loop {
            let inner = r.u8()?;
            if inner == 0 {
                break;
            }
            let n = r.u16()? as usize;
            r.bytes(n)?;
            skip_nbt(r, inner, depth + 1)?;
        },
        11 => {
            let n = len(r)?;
            r.bytes(n.checked_mul(4).ok_or(ProtocolError::Eof)?)?;
        }
        t => return Err(ProtocolError::BadLength(t as i32)),
    }
    Ok(())
}

fn read_player_list(r: &mut Reader) -> Result<Vec<PlayerListAction>> {
    let action = r.varint()?;
    let n = r.varint()?;
    if !(0..=65536).contains(&n) {
        return Err(ProtocolError::BadLength(n));
    }
    let mut out = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let uuid = r.uuid()?;
        let optional_json = |r: &mut Reader| -> Result<Option<String>> { if r.bool()? { Ok(Some(r.string()?)) } else { Ok(None) } };
        out.push(match action {
            0 => {
                let name = r.string()?;
                let props = r.varint()?;
                if !(0..=1024).contains(&props) {
                    return Err(ProtocolError::BadLength(props));
                }
                let mut properties = Vec::with_capacity(props as usize);
                for _ in 0..props {
                    let (name, value) = (r.string()?, r.string()?);
                    let signature = if r.bool()? { Some(r.string()?) } else { None };
                    properties.push(ProfileProperty { name, value, signature });
                }
                let (gamemode, ping) = (r.varint()?, r.varint()?);
                PlayerListAction::Add { uuid, name, properties, gamemode, ping, display_name: optional_json(r)? }
            }
            1 => PlayerListAction::GameMode { uuid, gamemode: r.varint()? },
            2 => PlayerListAction::Latency { uuid, ping: r.varint()? },
            3 => PlayerListAction::DisplayName { uuid, display_name: optional_json(r)? },
            4 => PlayerListAction::Remove { uuid },
            a => return Err(ProtocolError::BadLength(a)),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_protocol::WriteExt;

    trait Put {
        fn i32(&mut self, v: i32) -> &mut Self;
        fn i16(&mut self, v: i16) -> &mut Self;
        fn b(&mut self, v: u8) -> &mut Self;
        fn vi(&mut self, v: i32) -> &mut Self;
        fn s(&mut self, v: &str) -> &mut Self;
        fn f(&mut self, v: f32) -> &mut Self;
        fn uuid(&mut self, u: Uuid) -> &mut Self;
    }
    impl Put for Vec<u8> {
        fn i32(&mut self, v: i32) -> &mut Self {
            self.extend_from_slice(&v.to_be_bytes());
            self
        }
        fn i16(&mut self, v: i16) -> &mut Self {
            self.extend_from_slice(&v.to_be_bytes());
            self
        }
        fn b(&mut self, v: u8) -> &mut Self {
            self.push(v);
            self
        }
        fn vi(&mut self, v: i32) -> &mut Self {
            self.put_varint(v);
            self
        }
        fn s(&mut self, v: &str) -> &mut Self {
            self.put_string(v);
            self
        }
        fn f(&mut self, v: f32) -> &mut Self {
            self.put_f32(v);
            self
        }
        fn uuid(&mut self, u: Uuid) -> &mut Self {
            self.extend_from_slice(&u.0.to_be_bytes());
            self
        }
    }

    const U: Uuid = Uuid(0x069a79f4_44e9_4726_a5be_fca90e38aaf5);

    /// Metadados com um valor de cada tipo, terminados em 127.
    fn all_meta() -> Vec<u8> {
        let mut b = Vec::new();
        b.b(0x00).b(0x20); // 0: byte (invisível)
        b.b(0x21).i16(300); // 1: short
        b.b(0x82).s("§aOlá"); // 2: string
        b.b(0x03).b(1); // 3: byte
        b.b(0x66).f(20.0); // 6: float
        b.b(0x47).i32(-5); // 7: int
        // 10: slot com NBT {display:{Name:"x"}, list:[1,2]}
        b.b(0xAA).i16(276).b(1).i16(0);
        b.b(10).extend_from_slice(&0u16.to_be_bytes());
        b.b(10).extend_from_slice(&7u16.to_be_bytes());
        b.extend_from_slice(b"display");
        b.b(8).extend_from_slice(&4u16.to_be_bytes());
        b.extend_from_slice(b"Name");
        b.extend_from_slice(&1u16.to_be_bytes());
        b.push(b'x');
        b.b(0); // fim de display
        b.b(9).extend_from_slice(&4u16.to_be_bytes());
        b.extend_from_slice(b"list");
        b.b(3).i32(2).i32(1).i32(2);
        b.b(0); // fim da raiz
        b.b(0xAB).i16(-1); // 11: slot vazio
        b.b(0xCC).i32(1).i32(64).i32(-3); // 12: posição
        b.b(0xED).f(1.0).f(2.0).f(3.0); // 13: rotação
        b.b(127);
        b
    }

    #[test]
    fn metadata_all_types() {
        let m = read_metadata(&mut Reader::new(&all_meta())).unwrap();
        assert_eq!(m.len(), 10);
        assert_eq!(m[0], (0, MetaValue::Byte(0x20)));
        assert_eq!(m[1], (1, MetaValue::Short(300)));
        assert_eq!(m[2], (2, MetaValue::Str("§aOlá".into())));
        assert_eq!(m[3], (3, MetaValue::Byte(1)));
        assert_eq!(m[4], (6, MetaValue::Float(20.0)));
        assert_eq!(m[5], (7, MetaValue::Int(-5)));
        match &m[6] {
            (10, MetaValue::Slot(Some(s))) => {
                assert_eq!((s.id, s.count, s.damage), (276, 1, 0));
                let nbt = s.nbt.as_ref().unwrap();
                assert_eq!(nbt[0], 10);
                assert_eq!(*nbt.last().unwrap(), 0);
            }
            other => panic!("slot errado: {other:?}"),
        }
        assert_eq!(m[7], (11, MetaValue::Slot(None)));
        assert_eq!(m[8], (12, MetaValue::Pos(1, 64, -3)));
        assert_eq!(m[9], (13, MetaValue::Rot(1.0, 2.0, 3.0)));
    }

    #[test]
    fn spawn_player() {
        let mut b = Vec::new();
        b.vi(42).uuid(U).i32(32 * 100 + 16).i32(64 * 32).i32(-32 * 5).b(64).b(0xE0).i16(276);
        b.b(0x0A).b(0x7F).b(127); // metadado 10 = partes da skin
        let p = parse(0x0C, &b).unwrap();
        assert_eq!(
            p,
            EntityPacket::SpawnPlayer {
                id: 42,
                uuid: U,
                x: 3216,
                y: 2048,
                z: -160,
                yaw: 64,
                pitch: -32,
                item: 276,
                meta: vec![(10, MetaValue::Byte(0x7F))]
            }
        );
    }

    #[test]
    fn spawn_object_with_and_without_velocity() {
        let mut b = Vec::new();
        b.vi(7).b(2).i32(10).i32(20).i32(30).b(1).b(2).i32(1).i16(100).i16(-200).i16(300);
        assert_eq!(
            parse(0x0E, &b).unwrap(),
            EntityPacket::SpawnObject { id: 7, kind: 2, x: 10, y: 20, z: 30, pitch: 1, yaw: 2, data: 1, vel: Some((100, -200, 300)) }
        );
        let mut b = Vec::new();
        b.vi(8).b(78).i32(0).i32(0).i32(0).b(0).b(0).i32(0);
        assert_eq!(parse(0x0E, &b).unwrap(), EntityPacket::SpawnObject { id: 8, kind: 78, x: 0, y: 0, z: 0, pitch: 0, yaw: 0, data: 0, vel: None });
    }

    #[test]
    fn spawn_painting_orb_and_lightning() {
        // Position: x 26 bits, y 12 bits, z 26 bits (aqui x = -3, y = 70, z = 12).
        let packed: i64 = (((-3i64) & 0x3FF_FFFF) << 38) | ((70i64 & 0xFFF) << 26) | (12i64 & 0x3FF_FFFF);
        let mut b = Vec::new();
        b.vi(40).s("Kebab");
        b.extend_from_slice(&packed.to_be_bytes());
        b.b(2);
        assert_eq!(parse(0x10, &b).unwrap(), EntityPacket::SpawnPainting { id: 40, title: "Kebab".into(), pos: (-3, 70, 12), facing: 2 });
        let mut b = Vec::new();
        b.vi(41).i32(32).i32(64).i32(-96).i16(37);
        assert_eq!(parse(0x11, &b).unwrap(), EntityPacket::SpawnXpOrb { id: 41, x: 32, y: 64, z: -96, value: 37 });
        let mut b = Vec::new();
        b.vi(42).b(1).i32(1).i32(2).i32(3);
        assert_eq!(parse(0x2C, &b).unwrap(), EntityPacket::SpawnGlobal { id: 42, kind: 1, x: 1, y: 2, z: 3 });
        assert!(handles(0x10) && handles(0x11) && handles(0x2C));
    }

    #[test]
    fn spawn_mob() {
        let mut b = Vec::new();
        b.vi(300).b(120).i32(1).i32(2).i32(3).b(10).b(20).b(30).i16(0).i16(-8000).i16(0);
        b.b(0x50).i32(2).b(127); // 16: profissão
        assert_eq!(
            parse(0x0F, &b).unwrap(),
            EntityPacket::SpawnMob {
                id: 300,
                kind: 120,
                x: 1,
                y: 2,
                z: 3,
                yaw: 10,
                pitch: 20,
                head_yaw: 30,
                vel: (0, -8000, 0),
                meta: vec![(16, MetaValue::Int(2))]
            }
        );
    }

    #[test]
    fn destroy_and_moves() {
        let mut b = Vec::new();
        b.vi(3).vi(1).vi(200).vi(70000);
        assert_eq!(parse(0x13, &b).unwrap(), EntityPacket::Destroy(vec![1, 200, 70000]));

        let mut b = Vec::new();
        b.vi(5);
        assert_eq!(parse(0x14, &b).unwrap(), EntityPacket::Move { id: 5, dx: 0, dy: 0, dz: 0, rot: None, on_ground: false });
        let mut b = Vec::new();
        b.vi(5).b(1).b(0xFF).b(4).b(1);
        assert_eq!(parse(0x15, &b).unwrap(), EntityPacket::Move { id: 5, dx: 1, dy: -1, dz: 4, rot: None, on_ground: true });
        let mut b = Vec::new();
        b.vi(5).b(0x80).b(10).b(0);
        assert_eq!(parse(0x16, &b).unwrap(), EntityPacket::Move { id: 5, dx: 0, dy: 0, dz: 0, rot: Some((-128, 10)), on_ground: false });
        let mut b = Vec::new();
        b.vi(5).b(2).b(3).b(4).b(5).b(6).b(1);
        assert_eq!(parse(0x17, &b).unwrap(), EntityPacket::Move { id: 5, dx: 2, dy: 3, dz: 4, rot: Some((5, 6)), on_ground: true });
    }

    #[test]
    fn teleport_head_velocity_animation_status_equipment() {
        let mut b = Vec::new();
        b.vi(9).i32(-1).i32(2).i32(3).b(64).b(0).b(1);
        assert_eq!(parse(0x18, &b).unwrap(), EntityPacket::Teleport { id: 9, x: -1, y: 2, z: 3, yaw: 64, pitch: 0, on_ground: true });
        let mut b = Vec::new();
        b.vi(9).b(0xC0);
        assert_eq!(parse(0x19, &b).unwrap(), EntityPacket::HeadLook { id: 9, yaw: -64 });
        let mut b = Vec::new();
        b.vi(9).i16(8000).i16(-4000).i16(0);
        assert_eq!(parse(0x12, &b).unwrap(), EntityPacket::Velocity { id: 9, vx: 8000, vy: -4000, vz: 0 });
        let mut b = Vec::new();
        b.vi(9).b(0);
        assert_eq!(parse(0x0B, &b).unwrap(), EntityPacket::Animation { id: 9, kind: 0 });
        let mut b = Vec::new();
        b.i32(9).b(2);
        assert_eq!(parse(0x1A, &b).unwrap(), EntityPacket::Status { id: 9, status: 2 });
        let mut b = Vec::new();
        b.vi(9).i16(4).i16(310).b(1).i16(5).b(0);
        assert_eq!(
            parse(0x04, &b).unwrap(),
            EntityPacket::Equipment { id: 9, slot: 4, item: Some(Slot { id: 310, count: 1, damage: 5, nbt: None }) }
        );
        let mut b = Vec::new();
        b.vi(9).b(0x00).b(0x02).b(127);
        assert_eq!(parse(0x1C, &b).unwrap(), EntityPacket::Metadata { id: 9, meta: vec![(0, MetaValue::Byte(2))] });
    }

    #[test]
    fn player_list_all_actions() {
        let mut b = Vec::new();
        b.vi(0).vi(1).uuid(U).s("Notch").vi(1).s("textures").s("ZXlK").b(1).s("assinatura").vi(1).vi(35).b(1).s("{\"text\":\"§cNotch\"}");
        let add = parse(0x38, &b).unwrap();
        assert_eq!(
            add,
            EntityPacket::PlayerList(vec![PlayerListAction::Add {
                uuid: U,
                name: "Notch".into(),
                properties: vec![ProfileProperty { name: "textures".into(), value: "ZXlK".into(), signature: Some("assinatura".into()) }],
                gamemode: 1,
                ping: 35,
                display_name: Some("{\"text\":\"§cNotch\"}".into()),
            }])
        );
        let mut b = Vec::new();
        b.vi(1).vi(1).uuid(U).vi(3);
        assert_eq!(parse(0x38, &b).unwrap(), EntityPacket::PlayerList(vec![PlayerListAction::GameMode { uuid: U, gamemode: 3 }]));
        let mut b = Vec::new();
        b.vi(2).vi(2).uuid(U).vi(120).uuid(Uuid(1)).vi(-1);
        assert_eq!(
            parse(0x38, &b).unwrap(),
            EntityPacket::PlayerList(vec![PlayerListAction::Latency { uuid: U, ping: 120 }, PlayerListAction::Latency { uuid: Uuid(1), ping: -1 }])
        );
        let mut b = Vec::new();
        b.vi(3).vi(1).uuid(U).b(0);
        assert_eq!(parse(0x38, &b).unwrap(), EntityPacket::PlayerList(vec![PlayerListAction::DisplayName { uuid: U, display_name: None }]));
        let mut b = Vec::new();
        b.vi(4).vi(1).uuid(U);
        assert_eq!(parse(0x38, &b).unwrap(), EntityPacket::PlayerList(vec![PlayerListAction::Remove { uuid: U }]));
    }

    #[test]
    fn truncated_is_error() {
        let mut b = Vec::new();
        b.vi(42).uuid(U).i32(0);
        assert!(parse(0x0C, &b).is_err());
        assert!(read_metadata(&mut Reader::new(&[0x00])).is_err());
    }

    #[test]
    fn uuid_format_and_hash() {
        assert_eq!(U.to_string(), "069a79f4-44e9-4726-a5be-fca90e38aaf5");
        // UUID.hashCode() do Java: (int)(hilo >> 32) ^ (int)hilo, hilo = msb ^ lsb.
        let hilo = 0x069a79f4_44e9_4726u64 ^ 0xa5be_fca90e38aaf5u64;
        assert_eq!(U.java_hash(), ((hilo >> 32) as i32) ^ (hilo as i32));
        assert_eq!(Uuid(1).java_hash(), 1);
    }
}
