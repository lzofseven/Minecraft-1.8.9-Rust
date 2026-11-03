//! Pacotes das telas de container que o `net.rs` não trata: S31 Window Property, S36 Sign Editor Open,
//! S0A Use Bed, S3F plugin message (`MC|TrList`, `MC|BOpen`) e os pacotes que o cliente manda de volta
//! (C11 Enchant Item, C12 Update Sign, C10 Creative Inventory Action, C17 plugin messages).

use mc_protocol::nbt::{Slot, WriteSlot};
use mc_protocol::{ProtocolError, Reader, WriteExt};

use crate::containers::Trade;

#[derive(Debug)]
pub enum ContainerEvent {
    /// S31: `Container.updateProgressBar`.
    Property { window: u8, id: i16, value: i16 },
    /// S36: abre o editor da placa em `pos`.
    SignEditor { pos: (i32, i32, i32) },
    /// S0A: o jogador `entity` deita na cama em `pos`.
    UseBed { entity: i32, pos: (i32, i32, i32) },
    /// `MC|TrList`.
    Trades { window: i32, trades: Vec<Trade> },
    /// `MC|BOpen`: abre o livro escrito da mão.
    BookOpen,
}

pub fn handles(id: i32) -> bool {
    matches!(id, 0x31 | 0x36 | 0x0A | 0x3F)
}

type R<T> = Result<T, ProtocolError>;

/// `MerchantRecipeList.readFromBuf`.
pub fn read_trades(r: &mut Reader) -> R<Vec<Trade>> {
    let n = r.u8()? as usize;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let buy1 = r.slot()?;
        let sell = r.slot()?;
        let buy2 = if r.bool()? { r.slot()? } else { None };
        let compensate = r.bool()?;
        let uses = r.i32()?;
        let max_uses = r.i32()?;
        if let (Some(buy1), Some(sell)) = (buy1, sell) {
            out.push(Trade { buy1, buy2, sell, disabled: compensate || uses >= max_uses, uses, max_uses });
        }
    }
    Ok(out)
}

pub fn parse(id: i32, body: &[u8]) -> R<Option<ContainerEvent>> {
    let mut r = Reader::new(body);
    Ok(match id {
        0x31 => Some(ContainerEvent::Property { window: r.u8()?, id: r.u16()? as i16, value: r.u16()? as i16 }),
        0x36 => Some(ContainerEvent::SignEditor { pos: r.position()? }),
        0x0A => Some(ContainerEvent::UseBed { entity: r.varint()?, pos: r.position()? }),
        0x3F => match r.string()?.as_str() {
            "MC|TrList" => {
                let window = r.i32()?;
                Some(ContainerEvent::Trades { window, trades: read_trades(&mut r)? })
            }
            "MC|BOpen" => Some(ContainerEvent::BookOpen),
            _ => None,
        },
        _ => None,
    })
}

// ------------------------------------------------------------ serverbound

/// C17 Plugin Message (na 1.8 os dados não têm prefixo de tamanho).
pub fn custom_payload(channel: &str, data: &[u8]) -> Vec<u8> {
    let mut p = Vec::new();
    p.put_varint(0x17);
    p.put_string(channel);
    p.extend_from_slice(data);
    p
}

/// C11 Enchant Item.
pub fn enchant_item(window: u8, button: u8) -> Vec<u8> {
    let mut p = Vec::new();
    p.put_varint(0x11);
    p.put_u8(window);
    p.put_u8(button);
    p
}

/// C10 Creative Inventory Action (`slot` -1 = soltar o item no mundo).
pub fn creative_slot(slot: i16, item: Option<&Slot>) -> Vec<u8> {
    let mut p = Vec::new();
    p.put_varint(0x10);
    p.put_u16(slot as u16);
    p.put_slot(item);
    p
}

/// C12 Update Sign: as quatro linhas viram componentes de texto em JSON.
pub fn update_sign(pos: (i32, i32, i32), lines: &[String; 4]) -> Vec<u8> {
    let mut p = Vec::new();
    p.put_varint(0x12);
    let (x, y, z) = pos;
    p.put_i64(((x as i64 & 0x3FF_FFFF) << 38) | ((y as i64 & 0xFFF) << 26) | (z as i64 & 0x3FF_FFFF));
    for l in lines {
        p.put_string(&serde_json::Value::String(l.clone()).to_string());
    }
    p
}

/// `MC|TrSel`: índice da troca escolhida.
pub fn trade_select(index: i32) -> Vec<u8> {
    custom_payload("MC|TrSel", &index.to_be_bytes())
}

/// `MC|Beacon`: efeitos primário e secundário.
pub fn beacon(primary: i32, secondary: i32) -> Vec<u8> {
    let mut d = Vec::new();
    d.extend_from_slice(&primary.to_be_bytes());
    d.extend_from_slice(&secondary.to_be_bytes());
    custom_payload("MC|Beacon", &d)
}

/// `MC|ItemName`: texto do campo de nome da bigorna (string de PacketBuffer).
pub fn item_name(name: &str) -> Vec<u8> {
    let mut d = Vec::new();
    d.put_string(name);
    custom_payload("MC|ItemName", &d)
}

/// `MC|BEdit` / `MC|BSign`: o livro inteiro como slot.
pub fn book(channel: &str, item: &Slot) -> Vec<u8> {
    let mut d = Vec::new();
    d.put_slot(Some(item));
    custom_payload(channel, &d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put_item(out: &mut Vec<u8>, id: u16, count: u8, dmg: u16) {
        out.put_u16(id);
        out.put_u8(count);
        out.put_u16(dmg);
        out.put_u8(0);
    }

    #[test]
    fn parses_window_property() {
        let mut b = Vec::new();
        b.put_u8(7);
        b.put_u16(2);
        b.put_u16(150);
        match parse(0x31, &b).unwrap() {
            Some(ContainerEvent::Property { window: 7, id: 2, value: 150 }) => {}
            e => panic!("{e:?}"),
        }
    }

    #[test]
    fn parses_sign_editor_position() {
        let mut b = Vec::new();
        let (x, y, z) = (-12i64, 70i64, 31i64);
        b.put_i64(((x & 0x3FF_FFFF) << 38) | ((y & 0xFFF) << 26) | (z & 0x3FF_FFFF));
        match parse(0x36, &b).unwrap() {
            Some(ContainerEvent::SignEditor { pos: (-12, 70, 31) }) => {}
            e => panic!("{e:?}"),
        }
    }

    #[test]
    fn parses_trade_list() {
        let mut b = Vec::new();
        b.put_string("MC|TrList");
        b.extend_from_slice(&5i32.to_be_bytes());
        b.put_u8(2);
        // troca 1: 24 esmeraldas? (388) -> pão (297), sem segundo item
        put_item(&mut b, 388, 1, 0);
        put_item(&mut b, 297, 4, 0);
        b.put_bool(false);
        b.put_bool(false);
        b.extend_from_slice(&0i32.to_be_bytes());
        b.extend_from_slice(&7i32.to_be_bytes());
        // troca 2: com segundo item e esgotada
        put_item(&mut b, 265, 1, 0);
        put_item(&mut b, 264, 1, 0);
        b.put_bool(true);
        put_item(&mut b, 388, 2, 0);
        b.put_bool(false);
        b.extend_from_slice(&7i32.to_be_bytes());
        b.extend_from_slice(&7i32.to_be_bytes());
        let Some(ContainerEvent::Trades { window, trades }) = parse(0x3F, &b).unwrap() else { panic!() };
        assert_eq!(window, 5);
        assert_eq!(trades.len(), 2);
        assert_eq!((trades[0].buy1.id, trades[0].sell.id, trades[0].sell.count), (388, 297, 4));
        assert!(trades[0].buy2.is_none() && !trades[0].disabled);
        assert_eq!(trades[1].buy2.as_ref().map(|s| (s.id, s.count)), Some((388, 2)));
        assert!(trades[1].disabled);
    }

    #[test]
    fn book_open_has_no_payload() {
        let mut b = Vec::new();
        b.put_string("MC|BOpen");
        assert!(matches!(parse(0x3F, &b).unwrap(), Some(ContainerEvent::BookOpen)));
    }

    #[test]
    fn custom_payload_has_no_length_prefix() {
        let p = trade_select(3);
        assert_eq!(p[0], 0x17);
        assert_eq!(&p[p.len() - 4..], &[0, 0, 0, 3]);
    }
}
