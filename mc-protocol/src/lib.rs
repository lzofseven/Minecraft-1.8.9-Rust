//! Tipos primitivos e pacotes do protocolo 47 (Minecraft 1.8.x).

use thiserror::Error;

pub mod nbt;

pub const PROTOCOL_VERSION: i32 = 47;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("fim inesperado dos dados")]
    Eof,
    #[error("VarInt grande demais")]
    VarIntTooLong,
    #[error("string inválida: {0}")]
    BadString(String),
    #[error("tamanho inválido: {0}")]
    BadLength(i32),
}

pub type Result<T> = std::result::Result<T, ProtocolError>;

/// Leitor sobre um slice de bytes.
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn remaining(&self) -> &'a [u8] {
        &self.data[self.pos..]
    }

    pub fn u8(&mut self) -> Result<u8> {
        let b = *self.data.get(self.pos).ok_or(ProtocolError::Eof)?;
        self.pos += 1;
        Ok(b)
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or(ProtocolError::Eof)?;
        let s = self.data.get(self.pos..end).ok_or(ProtocolError::Eof)?;
        self.pos = end;
        Ok(s)
    }

    pub fn bool(&mut self) -> Result<bool> {
        Ok(self.u8()? != 0)
    }

    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.bytes(2)?.try_into().unwrap()))
    }

    pub fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.bytes(4)?.try_into().unwrap()))
    }

    pub fn i64(&mut self) -> Result<i64> {
        Ok(i64::from_be_bytes(self.bytes(8)?.try_into().unwrap()))
    }

    pub fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_be_bytes(self.bytes(4)?.try_into().unwrap()))
    }

    pub fn f64(&mut self) -> Result<f64> {
        Ok(f64::from_be_bytes(self.bytes(8)?.try_into().unwrap()))
    }

    /// Position da 1.8: x 26 bits, y 12 bits, z 26 bits.
    pub fn position(&mut self) -> Result<(i32, i32, i32)> {
        let v = self.i64()?;
        Ok(((v >> 38) as i32, ((v >> 26) & 0xFFF) as i32, (v << 38 >> 38) as i32))
    }

    pub fn varint(&mut self) -> Result<i32> {
        let mut value: u32 = 0;
        for i in 0..5 {
            let b = self.u8()?;
            value |= ((b & 0x7F) as u32) << (7 * i);
            if b & 0x80 == 0 {
                return Ok(value as i32);
            }
        }
        Err(ProtocolError::VarIntTooLong)
    }

    pub fn string(&mut self) -> Result<String> {
        let len = self.varint()?;
        if len < 0 {
            return Err(ProtocolError::BadLength(len));
        }
        let b = self.bytes(len as usize)?;
        String::from_utf8(b.to_vec()).map_err(|e| ProtocolError::BadString(e.to_string()))
    }
}

/// Escritores de primitivos para um `Vec<u8>`.
pub trait WriteExt {
    fn put_u8(&mut self, v: u8);
    fn put_i64(&mut self, v: i64);
    fn put_u16(&mut self, v: u16);
    fn put_f32(&mut self, v: f32);
    fn put_f64(&mut self, v: f64);
    fn put_bool(&mut self, v: bool);
    fn put_varint(&mut self, v: i32);
    fn put_string(&mut self, s: &str);
}

impl WriteExt for Vec<u8> {
    fn put_u8(&mut self, v: u8) {
        self.push(v);
    }
    fn put_i64(&mut self, v: i64) {
        self.extend_from_slice(&v.to_be_bytes());
    }
    fn put_u16(&mut self, v: u16) {
        self.extend_from_slice(&v.to_be_bytes());
    }
    fn put_f32(&mut self, v: f32) {
        self.extend_from_slice(&v.to_be_bytes());
    }
    fn put_f64(&mut self, v: f64) {
        self.extend_from_slice(&v.to_be_bytes());
    }
    fn put_bool(&mut self, v: bool) {
        self.push(v as u8);
    }
    fn put_varint(&mut self, v: i32) {
        let mut v = v as u32;
        loop {
            if v & !0x7F == 0 {
                self.push(v as u8);
                return;
            }
            self.push((v & 0x7F) as u8 | 0x80);
            v >>= 7;
        }
    }
    fn put_string(&mut self, s: &str) {
        self.put_varint(s.len() as i32);
        self.extend_from_slice(s.as_bytes());
    }
}

/// Estados da conexão.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Handshaking,
    Status,
    Login,
    Play,
}

/// Pacotes do cliente para o servidor (serverbound) usados nesta fase.
pub mod serverbound {
    use super::*;

    /// Handshake (0x00). `next_state`: 1 = status, 2 = login.
    pub fn handshake(host: &str, port: u16, next_state: i32) -> Vec<u8> {
        let mut p = Vec::new();
        p.put_varint(0x00);
        p.put_varint(PROTOCOL_VERSION);
        p.put_string(host);
        p.put_u16(port);
        p.put_varint(next_state);
        p
    }

    pub fn status_request() -> Vec<u8> {
        let mut p = Vec::new();
        p.put_varint(0x00);
        p
    }

    pub fn status_ping(payload: i64) -> Vec<u8> {
        let mut p = Vec::new();
        p.put_varint(0x01);
        p.put_i64(payload);
        p
    }

    /// Login Start (0x00).
    pub fn login_start(name: &str) -> Vec<u8> {
        let mut p = Vec::new();
        p.put_varint(0x00);
        p.put_string(name);
        p
    }

    /// Keep Alive (0x00 em Play).
    pub fn keep_alive(id: i32) -> Vec<u8> {
        let mut p = Vec::new();
        p.put_varint(0x00);
        p.put_varint(id);
        p
    }

    /// Chat Message (0x01 em Play).
    pub fn chat(msg: &str) -> Vec<u8> {
        let mut p = Vec::new();
        p.put_varint(0x01);
        p.put_string(msg);
        p
    }

    /// Tab-Complete (0x14): texto até o cursor e, se o jogador mira num bloco, a posição dele.
    pub fn tab_complete(text: &str, looking_at: Option<(i32, i32, i32)>) -> Vec<u8> {
        let mut p = Vec::new();
        p.put_varint(0x14);
        p.put_string(text);
        p.put_bool(looking_at.is_some());
        if let Some((x, y, z)) = looking_at {
            p.put_i64(((x as i64 & 0x3FFFFFF) << 38) | ((y as i64 & 0xFFF) << 26) | (z as i64 & 0x3FFFFFF));
        }
        p
    }

    /// Player (0x03): só o estado no chão.
    pub fn player(on_ground: bool) -> Vec<u8> {
        let mut p = Vec::new();
        p.put_varint(0x03);
        p.put_bool(on_ground);
        p
    }

    /// Player Position (0x04). `y` é a altura dos pés.
    pub fn player_pos(x: f64, y: f64, z: f64, on_ground: bool) -> Vec<u8> {
        let mut p = Vec::new();
        p.put_varint(0x04);
        p.put_f64(x);
        p.put_f64(y);
        p.put_f64(z);
        p.put_bool(on_ground);
        p
    }

    /// Player Look (0x05).
    pub fn player_look(yaw: f32, pitch: f32, on_ground: bool) -> Vec<u8> {
        let mut p = Vec::new();
        p.put_varint(0x05);
        p.put_f32(yaw);
        p.put_f32(pitch);
        p.put_bool(on_ground);
        p
    }

    /// Player Position And Look (0x06).
    pub fn player_pos_look(x: f64, y: f64, z: f64, yaw: f32, pitch: f32, on_ground: bool) -> Vec<u8> {
        let mut p = Vec::new();
        p.put_varint(0x06);
        p.put_f64(x);
        p.put_f64(y);
        p.put_f64(z);
        p.put_f32(yaw);
        p.put_f32(pitch);
        p.put_bool(on_ground);
        p
    }

    /// Confirm Transaction (0x0F): resposta ao 0x32 recusado (anticheats medem o lag com isso).
    pub fn confirm_transaction(window: u8, action: u16) -> Vec<u8> {
        let mut p = Vec::new();
        p.put_varint(0x0F);
        p.put_u8(window);
        p.put_u16(action);
        p.put_bool(true);
        p
    }

    /// Close Window (0x0D).
    pub fn close_window(window: u8) -> Vec<u8> {
        let mut p = Vec::new();
        p.put_varint(0x0D);
        p.put_u8(window);
        p
    }

    /// Click Window (0x0E). `clicked` é o item que estava no slot antes do clique.
    pub fn click_window(window: u8, slot: i16, button: u8, action: u16, mode: u8, clicked: Option<&nbt::Slot>) -> Vec<u8> {
        use nbt::WriteSlot;
        let mut p = Vec::new();
        p.put_varint(0x0E);
        p.put_u8(window);
        p.put_u16(slot as u16);
        p.put_u8(button);
        p.put_u16(action);
        p.put_u8(mode);
        p.put_slot(clicked);
        p
    }

    /// Animation (0x0A): balançar o braço.
    pub fn swing() -> Vec<u8> {
        let mut p = Vec::new();
        p.put_varint(0x0A);
        p
    }

    /// Player Block Placement (0x08). Sem bloco (usar item no ar): posição -1 e face 255.
    pub fn block_placement(pos: Option<(i32, i32, i32)>, face: u8, held: Option<&nbt::Slot>, cursor: [u8; 3]) -> Vec<u8> {
        use nbt::WriteSlot;
        let (x, y, z) = pos.unwrap_or((-1, -1, -1));
        let mut p = Vec::new();
        p.put_varint(0x08);
        p.put_i64(((x as i64 & 0x3FFFFFF) << 38) | ((y as i64 & 0xFFF) << 26) | (z as i64 & 0x3FFFFFF));
        p.put_u8(if pos.is_some() { face } else { 255 });
        p.put_slot(held);
        p.extend_from_slice(&cursor);
        p
    }

    /// Player Digging (0x07): 0 começar, 1 cancelar, 2 terminar, 3 soltar pilha, 4 soltar item.
    pub fn digging(status: u8, pos: (i32, i32, i32), face: u8) -> Vec<u8> {
        let (x, y, z) = pos;
        let mut p = Vec::new();
        p.put_varint(0x07);
        p.put_u8(status);
        p.put_i64(((x as i64 & 0x3FFFFFF) << 38) | ((y as i64 & 0xFFF) << 26) | (z as i64 & 0x3FFFFFF));
        p.put_u8(face);
        p
    }

    /// Use Entity (0x02): 0 interagir, 1 atacar, 2 interagir em (x, y, z).
    pub fn use_entity(target: i32, kind: i32, at: Option<[f32; 3]>) -> Vec<u8> {
        let mut p = Vec::new();
        p.put_varint(0x02);
        p.put_varint(target);
        p.put_varint(kind);
        if let (2, Some([x, y, z])) = (kind, at) {
            p.put_f32(x);
            p.put_f32(y);
            p.put_f32(z);
        }
        p
    }

    /// Held Item Change (0x09): slot da hotbar 0..8.
    pub fn held_item_change(slot: u16) -> Vec<u8> {
        let mut p = Vec::new();
        p.put_varint(0x09);
        p.put_u16(slot);
        p
    }

    /// Entity Action (0x0B): 0/1 agachar, 3/4 correr, 6 abrir inventário (montaria).
    pub fn entity_action(entity_id: i32, action: i32, jump_boost: i32) -> Vec<u8> {
        let mut p = Vec::new();
        p.put_varint(0x0B);
        p.put_varint(entity_id);
        p.put_varint(action);
        p.put_varint(jump_boost);
        p
    }

    /// Steer Vehicle (0x0C): lado, frente e bits 1 = pular, 2 = desmontar (agachar).
    pub fn steer_vehicle(strafe: f32, forward: f32, jump: bool, sneak: bool) -> Vec<u8> {
        let mut p = Vec::new();
        p.put_varint(0x0C);
        p.put_f32(strafe);
        p.put_f32(forward);
        p.put_u8(jump as u8 | (sneak as u8) << 1);
        p
    }

    /// Player Abilities (0x13).
    pub fn abilities(flags: u8, fly_speed: f32, walk_speed: f32) -> Vec<u8> {
        let mut p = Vec::new();
        p.put_varint(0x13);
        p.put_u8(flags);
        p.put_f32(fly_speed);
        p.put_f32(walk_speed);
        p
    }

    /// Client Settings (0x15). Na 1.8 o modo de chat ainda é um byte.
    pub fn client_settings(locale: &str, view_distance: u8) -> Vec<u8> {
        let mut p = Vec::new();
        p.put_varint(0x15);
        p.put_string(locale);
        p.put_u8(view_distance);
        p.put_u8(0);
        p.put_bool(true);
        p.put_u8(0x7F);
        p
    }

    /// Client Settings (0x15) completo: idioma, distância, chat (0 tudo, 1 comandos, 2 nada),
    /// cores no chat e partes da skin ligadas.
    pub fn client_settings_full(locale: &str, view_distance: u8, chat_mode: u8, colors: bool, skin_parts: u8) -> Vec<u8> {
        let mut p = Vec::new();
        p.put_varint(0x15);
        p.put_string(locale);
        p.put_u8(view_distance);
        p.put_u8(chat_mode);
        p.put_bool(colors);
        p.put_u8(skin_parts);
        p
    }

    /// `C16PacketClientStatus.EnumState.PERFORM_RESPAWN`.
    pub const STATUS_PERFORM_RESPAWN: i32 = 0;
    /// `C16PacketClientStatus.EnumState.OPEN_INVENTORY_ACHIEVEMENT`: a conquista "Taking Inventory".
    pub const STATUS_OPEN_INVENTORY_ACHIEVEMENT: i32 = 2;

    /// Client Status (0x16). Ações: `STATUS_PERFORM_RESPAWN`, `STATUS_OPEN_INVENTORY_ACHIEVEMENT`.
    pub fn client_status(action: i32) -> Vec<u8> {
        let mut p = Vec::new();
        p.put_varint(0x16);
        p.put_varint(action);
        p
    }

    /// Plugin Message MC|Brand (0x17). Na 1.8 os dados não têm prefixo de tamanho.
    pub fn brand(name: &str) -> Vec<u8> {
        let mut p = Vec::new();
        p.put_varint(0x17);
        p.put_string("MC|Brand");
        p.put_string(name);
        p
    }

    /// Resource Pack Status (0x19). 1 = recusado.
    pub fn resource_pack_status(hash: &str, result: i32) -> Vec<u8> {
        let mut p = Vec::new();
        p.put_varint(0x19);
        p.put_string(hash);
        p.put_varint(result);
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_roundtrip() {
        for v in [0, 1, 127, 128, 255, 25565, 2097151, i32::MAX, -1, i32::MIN] {
            let mut b = Vec::new();
            b.put_varint(v);
            assert_eq!(Reader::new(&b).varint().unwrap(), v);
        }
    }

    #[test]
    fn varint_known_bytes() {
        let mut b = Vec::new();
        b.put_varint(300);
        assert_eq!(b, [0xAC, 0x02]);
        let mut b = Vec::new();
        b.put_varint(-1);
        assert_eq!(b, [0xFF, 0xFF, 0xFF, 0xFF, 0x0F]);
    }

    #[test]
    fn varint_too_long() {
        assert!(matches!(
            Reader::new(&[0x80; 6]).varint(),
            Err(ProtocolError::VarIntTooLong)
        ));
    }

    #[test]
    fn string_roundtrip() {
        let mut b = Vec::new();
        b.put_string("olá");
        assert_eq!(Reader::new(&b).string().unwrap(), "olá");
    }

    #[test]
    fn position_decode() {
        let (x, y, z) = (-1234567i32, 200i32, 7654321i32);
        let v = ((x as i64 & 0x3FFFFFF) << 38) | ((y as i64 & 0xFFF) << 26) | (z as i64 & 0x3FFFFFF);
        assert_eq!(Reader::new(&v.to_be_bytes()).position().unwrap(), (x, y, z));
    }

    #[test]
    fn handshake_layout() {
        let p = serverbound::handshake("localhost", 25565, 2);
        let mut r = Reader::new(&p);
        assert_eq!(r.varint().unwrap(), 0);
        assert_eq!(r.varint().unwrap(), 47);
        assert_eq!(r.string().unwrap(), "localhost");
        assert_eq!(r.bytes(2).unwrap(), 25565u16.to_be_bytes());
        assert_eq!(r.varint().unwrap(), 2);
        assert!(r.remaining().is_empty());
    }
}
