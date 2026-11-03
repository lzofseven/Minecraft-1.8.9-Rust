//! NBT sem compressão (formato dos slots na 1.8) e o slot de item.

use std::collections::HashMap;

use crate::{ProtocolError, Reader, Result, WriteExt};

#[derive(Clone, Debug, PartialEq)]
pub enum Nbt {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<u8>),
    String(String),
    List(Vec<Nbt>),
    Compound(HashMap<String, Nbt>),
    IntArray(Vec<i32>),
}

impl Nbt {
    pub fn get(&self, key: &str) -> Option<&Nbt> {
        match self {
            Nbt::Compound(m) => m.get(key),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Nbt::String(s) => Some(s),
            _ => None,
        }
    }
}

fn nbt_string(r: &mut Reader) -> Result<String> {
    let len = r.u16()? as usize;
    let b = r.bytes(len)?;
    // NBT usa UTF-8 modificado; para textos comuns é igual ao UTF-8.
    Ok(String::from_utf8_lossy(b).into_owned())
}

fn payload(r: &mut Reader, tag: u8, depth: u32) -> Result<Nbt> {
    if depth > 512 {
        return Err(ProtocolError::BadLength(depth as i32));
    }
    Ok(match tag {
        1 => Nbt::Byte(r.u8()? as i8),
        2 => Nbt::Short(r.u16()? as i16),
        3 => Nbt::Int(r.i32()?),
        4 => Nbt::Long(r.i64()?),
        5 => Nbt::Float(r.f32()?),
        6 => Nbt::Double(r.f64()?),
        7 => {
            let n = r.i32()?;
            if n < 0 {
                return Err(ProtocolError::BadLength(n));
            }
            Nbt::ByteArray(r.bytes(n as usize)?.to_vec())
        }
        8 => Nbt::String(nbt_string(r)?),
        9 => {
            let inner = r.u8()?;
            let n = r.i32()?;
            if n < 0 {
                return Err(ProtocolError::BadLength(n));
            }
            let mut v = Vec::with_capacity(n.min(1024) as usize);
            for _ in 0..n {
                v.push(payload(r, inner, depth + 1)?);
            }
            Nbt::List(v)
        }
        10 => {
            let mut m = HashMap::new();
            loop {
                let t = r.u8()?;
                if t == 0 {
                    break;
                }
                let name = nbt_string(r)?;
                m.insert(name, payload(r, t, depth + 1)?);
            }
            Nbt::Compound(m)
        }
        11 => {
            let n = r.i32()?;
            if n < 0 {
                return Err(ProtocolError::BadLength(n));
            }
            Nbt::IntArray((0..n).map(|_| r.i32()).collect::<Result<_>>()?)
        }
        t => return Err(ProtocolError::BadLength(t as i32)),
    })
}

/// Tag raiz com nome (o nome é descartado). `None` se for TAG_End (sem NBT).
pub fn read_root(r: &mut Reader) -> Result<Option<Nbt>> {
    let t = r.u8()?;
    if t == 0 {
        return Ok(None);
    }
    let _name = nbt_string(r)?;
    Ok(Some(payload(r, t, 0)?))
}

/// `DataOutputStream.writeUTF` (UTF-8 modificado do NBT): NUL vira C0 80 e fora do plano básico
/// vai em pares substitutos de 3 bytes cada.
fn put_nbt_string(out: &mut Vec<u8>, s: &str) {
    let mut b = Vec::with_capacity(s.len());
    for u in s.encode_utf16() {
        match u {
            0x0001..=0x007F => b.push(u as u8),
            0x0000 | 0x0080..=0x07FF => {
                b.push(0xC0 | (u >> 6) as u8);
                b.push(0x80 | (u & 0x3F) as u8);
            }
            _ => {
                b.push(0xE0 | (u >> 12) as u8);
                b.push(0x80 | ((u >> 6) & 0x3F) as u8);
                b.push(0x80 | (u & 0x3F) as u8);
            }
        }
    }
    out.extend_from_slice(&(b.len() as u16).to_be_bytes());
    out.extend_from_slice(&b);
}

impl Nbt {
    /// Número da tag (`NBTBase.getId`).
    fn tag_id(&self) -> u8 {
        match self {
            Nbt::Byte(_) => 1,
            Nbt::Short(_) => 2,
            Nbt::Int(_) => 3,
            Nbt::Long(_) => 4,
            Nbt::Float(_) => 5,
            Nbt::Double(_) => 6,
            Nbt::ByteArray(_) => 7,
            Nbt::String(_) => 8,
            Nbt::List(_) => 9,
            Nbt::Compound(_) => 10,
            Nbt::IntArray(_) => 11,
        }
    }

    fn write_payload(&self, out: &mut Vec<u8>) {
        match self {
            Nbt::Byte(v) => out.push(*v as u8),
            Nbt::Short(v) => out.extend_from_slice(&v.to_be_bytes()),
            Nbt::Int(v) => out.extend_from_slice(&v.to_be_bytes()),
            Nbt::Long(v) => out.extend_from_slice(&v.to_be_bytes()),
            Nbt::Float(v) => out.extend_from_slice(&v.to_be_bytes()),
            Nbt::Double(v) => out.extend_from_slice(&v.to_be_bytes()),
            Nbt::ByteArray(v) => {
                out.extend_from_slice(&(v.len() as i32).to_be_bytes());
                out.extend_from_slice(v);
            }
            Nbt::String(s) => put_nbt_string(out, s),
            Nbt::List(v) => {
                // NBTTagList.write: o tipo é o do primeiro elemento (0 se vazia).
                out.push(v.first().map_or(0, Nbt::tag_id));
                out.extend_from_slice(&(v.len() as i32).to_be_bytes());
                for e in v {
                    e.write_payload(out);
                }
            }
            Nbt::Compound(m) => {
                // A ordem das chaves não importa para o NBT; ordenar deixa a saída estável.
                let mut keys: Vec<&String> = m.keys().collect();
                keys.sort();
                for k in keys {
                    let v = &m[k];
                    out.push(v.tag_id());
                    put_nbt_string(out, k);
                    v.write_payload(out);
                }
                out.push(0);
            }
            Nbt::IntArray(v) => {
                out.extend_from_slice(&(v.len() as i32).to_be_bytes());
                for e in v {
                    out.extend_from_slice(&e.to_be_bytes());
                }
            }
        }
    }

    /// Tag raiz sem nome, como vai no slot (`CompressedStreamTools.write` sem compressão).
    pub fn write_root(&self) -> Vec<u8> {
        let mut out = vec![self.tag_id()];
        put_nbt_string(&mut out, "");
        self.write_payload(&mut out);
        out
    }
}

/// Item num slot (`ItemStack` como vai na rede).
#[derive(Clone, Debug, PartialEq)]
pub struct Slot {
    pub id: i16,
    pub count: u8,
    pub damage: i16,
    pub nbt: Option<Nbt>,
    /// Bytes do NBT como vieram, para devolver igual ao servidor.
    pub raw_nbt: Vec<u8>,
}

impl Slot {
    /// Nome customizado (`tag.display.Name`).
    pub fn display_name(&self) -> Option<&str> {
        self.nbt.as_ref()?.get("display")?.get("Name")?.as_str()
    }

    /// Lore (`tag.display.Lore`).
    pub fn lore(&self) -> Vec<String> {
        match self.nbt.as_ref().and_then(|n| n.get("display")).and_then(|d| d.get("Lore")) {
            Some(Nbt::List(v)) => v.iter().filter_map(|l| l.as_str().map(str::to_string)).collect(),
            _ => Vec::new(),
        }
    }

    /// `EnchantmentHelper.getEnchantmentLevel`: nível do encantamento `id` na lista "ench" (0 sem ele).
    pub fn enchantment_level(&self, id: i32) -> i32 {
        let Some(Nbt::List(list)) = self.nbt.as_ref().and_then(|n| n.get("ench")) else { return 0 };
        let num = |n: Option<&Nbt>| match n {
            Some(Nbt::Short(v)) => Some(*v as i32),
            Some(Nbt::Int(v)) => Some(*v),
            Some(Nbt::Byte(v)) => Some(*v as i32),
            _ => None,
        };
        list.iter().find(|e| num(e.get("id")) == Some(id)).and_then(|e| num(e.get("lvl"))).unwrap_or(0)
    }

    /// Item encantado (brilho).
    pub fn enchanted(&self) -> bool {
        matches!(self.nbt.as_ref().and_then(|n| n.get("ench")), Some(Nbt::List(v)) if !v.is_empty())
    }
}

impl Reader<'_> {
    /// Slot da 1.8: id (-1 = vazio), quantidade, dano e NBT.
    pub fn slot(&mut self) -> Result<Option<Slot>> {
        let id = self.u16()? as i16;
        if id < 0 {
            return Ok(None);
        }
        let count = self.u8()?;
        let damage = self.u16()? as i16;
        let start = self.remaining();
        let nbt = read_root(self)?;
        let used = start.len() - self.remaining().len();
        Ok(Some(Slot { id, count, damage, nbt, raw_nbt: start[..used].to_vec() }))
    }
}

pub trait WriteSlot {
    fn put_slot(&mut self, s: Option<&Slot>);
}

impl WriteSlot for Vec<u8> {
    fn put_slot(&mut self, s: Option<&Slot>) {
        match s {
            None => self.put_u16(0xFFFF),
            Some(s) => {
                self.put_u16(s.id as u16);
                self.put_u8(s.count);
                self.put_u16(s.damage as u16);
                if s.raw_nbt.is_empty() {
                    self.put_u8(0);
                } else {
                    self.extend_from_slice(&s.raw_nbt);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(out: &mut Vec<u8>, v: &str) {
        out.put_u16(v.len() as u16);
        out.extend_from_slice(v.as_bytes());
    }

    #[test]
    fn slot_with_display_name_and_lore_roundtrip() {
        // Bússola com nome e lore.
        let mut b = Vec::new();
        b.put_u16(345);
        b.put_u8(1);
        b.put_u16(0);
        b.put_u8(10);
        s(&mut b, "");
        b.put_u8(10);
        s(&mut b, "display");
        b.put_u8(8);
        s(&mut b, "Name");
        s(&mut b, "§aSelecionar jogo");
        b.put_u8(9);
        s(&mut b, "Lore");
        b.put_u8(8);
        b.extend_from_slice(&2i32.to_be_bytes());
        s(&mut b, "linha 1");
        s(&mut b, "linha 2");
        b.put_u8(0);
        b.put_u8(0);
        b.push(0xAB);

        let mut r = Reader::new(&b);
        let slot = r.slot().unwrap().unwrap();
        assert_eq!((slot.id, slot.count), (345, 1));
        assert_eq!(slot.display_name(), Some("§aSelecionar jogo"));
        assert_eq!(slot.lore(), vec!["linha 1", "linha 2"]);
        assert_eq!(r.remaining(), &[0xAB]);

        let mut out = Vec::new();
        out.put_slot(Some(&slot));
        assert_eq!(out, b[..b.len() - 1]);
    }

    #[test]
    fn write_root_roundtrips_book_tag() {
        let mut m = HashMap::new();
        m.insert("pages".to_string(), Nbt::List(vec![Nbt::String("oi\n\u{e9}\u{1F600}".into()), Nbt::String(String::new())]));
        m.insert("title".to_string(), Nbt::String("Livro".into()));
        m.insert("generation".to_string(), Nbt::Int(2));
        m.insert("ench".to_string(), Nbt::List(Vec::new()));
        m.insert("pos".to_string(), Nbt::IntArray(vec![1, -2]));
        let tag = Nbt::Compound(m);
        let bytes = tag.write_root();
        let mut r = Reader::new(&bytes);
        let back = read_root(&mut r).unwrap().unwrap();
        assert!(r.remaining().is_empty());
        // O emoji vira par substituto em 2 x 3 bytes; a leitura (UTF-8 comum) não o reconstrói,
        // então só o resto precisa voltar igual.
        let Nbt::Compound(b) = &back else { panic!() };
        assert_eq!(b["title"], Nbt::String("Livro".into()));
        assert_eq!(b["generation"], Nbt::Int(2));
        assert_eq!(b["ench"], Nbt::List(Vec::new()));
        assert_eq!(b["pos"], Nbt::IntArray(vec![1, -2]));
        let Nbt::List(p) = &b["pages"] else { panic!() };
        assert_eq!(p.len(), 2);
        assert_eq!(p[1], Nbt::String(String::new()));
        // Mesma entrada, mesma saída (chaves ordenadas).
        assert_eq!(tag.write_root(), bytes);
    }

    #[test]
    fn modified_utf8_strings() {
        let mut out = Vec::new();
        put_nbt_string(&mut out, "a\0\u{e9}\u{20ac}\u{1F600}");
        assert_eq!(out, [0, 14, b'a', 0xC0, 0x80, 0xC3, 0xA9, 0xE2, 0x82, 0xAC, 0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80]);
    }

    #[test]
    fn empty_slot() {
        let mut r = Reader::new(&[0xFF, 0xFF]);
        assert!(r.slot().unwrap().is_none());
    }
}
