//! Conexão TCP com framing, compressão e criptografia do protocolo 47.

use std::io::{Read, Write};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

use aes::Aes128;
use cfb8::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};
use mc_protocol::{ProtocolError, Reader, WriteExt};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpStream, ToSocketAddrs};

const MAX_PACKET: i32 = 8 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum NetError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("protocolo: {0}")]
    Protocol(#[from] ProtocolError),
    #[error("pacote com tamanho inválido: {0}")]
    BadSize(i32),
}

pub type Result<T> = std::result::Result<T, NetError>;

/// Pacote recebido: id + corpo (sem o id).
#[derive(Debug)]
pub struct Packet {
    pub id: i32,
    pub body: Vec<u8>,
}

/// Limiar de compressão compartilhado entre leitor e escritor (-1 = desligada).
type Threshold = Arc<AtomicI32>;

fn threshold(t: &Threshold) -> Option<i32> {
    let v = t.load(Ordering::Relaxed);
    (v >= 0).then_some(v)
}

pub struct ConnReader {
    stream: OwnedReadHalf,
    dec: Option<cfb8::Decryptor<Aes128>>,
    threshold: Threshold,
}

pub struct ConnWriter {
    stream: OwnedWriteHalf,
    enc: Option<cfb8::Encryptor<Aes128>>,
    threshold: Threshold,
}

pub struct Connection {
    reader: ConnReader,
    writer: ConnWriter,
}

impl Connection {
    pub async fn connect(addr: impl ToSocketAddrs) -> Result<Self> {
        let stream = TcpStream::connect(addr).await?;
        stream.set_nodelay(true)?;
        let (r, w) = stream.into_split();
        let threshold: Threshold = Arc::new(AtomicI32::new(-1));
        Ok(Self {
            reader: ConnReader { stream: r, dec: None, threshold: threshold.clone() },
            writer: ConnWriter { stream: w, enc: None, threshold },
        })
    }

    pub fn set_compression(&mut self, t: i32) {
        self.reader.set_compression(t);
    }

    /// Liga AES-128/CFB8 com o segredo compartilhado (key = iv = segredo).
    pub fn enable_encryption(&mut self, secret: &[u8; 16]) {
        self.reader.dec = Some(cfb8::Decryptor::<Aes128>::new(secret.into(), secret.into()));
        self.writer.enc = Some(cfb8::Encryptor::<Aes128>::new(secret.into(), secret.into()));
    }

    pub fn encrypted(&self) -> bool {
        self.reader.dec.is_some()
    }

    /// Envia um pacote já montado (`varint id` + corpo).
    pub async fn send(&mut self, packet: &[u8]) -> Result<()> {
        self.writer.send(packet).await
    }

    pub async fn recv(&mut self) -> Result<Packet> {
        self.reader.recv().await
    }

    /// Separa leitura e escrita para rodarem em tarefas diferentes.
    pub fn into_split(self) -> (ConnReader, ConnWriter) {
        (self.reader, self.writer)
    }
}

impl ConnReader {
    /// Vale para as duas metades (o limiar é compartilhado).
    pub fn set_compression(&self, t: i32) {
        self.threshold.store(if t >= 0 { t } else { -1 }, Ordering::Relaxed);
    }

    pub async fn recv(&mut self) -> Result<Packet> {
        let len = self.read_varint().await?;
        if !(0..=MAX_PACKET).contains(&len) {
            return Err(NetError::BadSize(len));
        }
        let mut buf = vec![0u8; len as usize];
        self.stream.read_exact(&mut buf).await?;
        if let Some(d) = &mut self.dec {
            for b in buf.iter_mut() {
                d.decrypt_block_mut(std::slice::from_mut(b).into());
            }
        }
        decode_frame(&buf, threshold(&self.threshold).is_some())
    }

    async fn read_varint(&mut self) -> Result<i32> {
        let mut value: u32 = 0;
        for i in 0..5 {
            let mut b = self.stream.read_u8().await?;
            if let Some(d) = &mut self.dec {
                d.decrypt_block_mut(std::slice::from_mut(&mut b).into());
            }
            value |= ((b & 0x7F) as u32) << (7 * i);
            if b & 0x80 == 0 {
                return Ok(value as i32);
            }
        }
        Err(ProtocolError::VarIntTooLong.into())
    }
}

impl ConnWriter {
    pub async fn send(&mut self, packet: &[u8]) -> Result<()> {
        let mut frame = encode_frame(packet, threshold(&self.threshold))?;
        if let Some(e) = &mut self.enc {
            for b in frame.iter_mut() {
                e.encrypt_block_mut(std::slice::from_mut(b).into());
            }
        }
        self.stream.write_all(&frame).await?;
        Ok(())
    }
}

/// Monta o frame: `length` + (com compressão: `data_length` + dados).
pub fn encode_frame(packet: &[u8], threshold: Option<i32>) -> Result<Vec<u8>> {
    let mut inner = Vec::new();
    match threshold {
        None => inner.extend_from_slice(packet),
        Some(t) if (packet.len() as i32) < t => {
            inner.put_varint(0);
            inner.extend_from_slice(packet);
        }
        Some(_) => {
            inner.put_varint(packet.len() as i32);
            let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
            enc.write_all(packet)?;
            inner.extend_from_slice(&enc.finish()?);
        }
    }
    let mut frame = Vec::with_capacity(inner.len() + 5);
    frame.put_varint(inner.len() as i32);
    frame.extend_from_slice(&inner);
    Ok(frame)
}

/// Decodifica o conteúdo de um frame (sem o prefixo `length`).
pub fn decode_frame(buf: &[u8], compressed: bool) -> Result<Packet> {
    let data: Vec<u8> = if compressed {
        let mut r = Reader::new(buf);
        let data_len = r.varint()?;
        if data_len == 0 {
            r.remaining().to_vec()
        } else {
            if !(0..=MAX_PACKET).contains(&data_len) {
                return Err(NetError::BadSize(data_len));
            }
            let mut out = Vec::with_capacity(data_len as usize);
            ZlibDecoder::new(r.remaining()).read_to_end(&mut out)?;
            out
        }
    } else {
        buf.to_vec()
    };
    let mut r = Reader::new(&data);
    let id = r.varint()?;
    Ok(Packet { id, body: r.remaining().to_vec() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strip_len(frame: &[u8]) -> Vec<u8> {
        let mut r = Reader::new(frame);
        let n = r.varint().unwrap() as usize;
        assert_eq!(r.remaining().len(), n);
        r.remaining().to_vec()
    }

    #[test]
    fn frame_plain() {
        let mut pkt = Vec::new();
        pkt.put_varint(0x21);
        pkt.extend_from_slice(b"abc");
        let f = encode_frame(&pkt, None).unwrap();
        let p = decode_frame(&strip_len(&f), false).unwrap();
        assert_eq!((p.id, p.body.as_slice()), (0x21, &b"abc"[..]));
    }

    #[test]
    fn frame_compressed_small_and_large() {
        for size in [3usize, 5000] {
            let mut pkt = Vec::new();
            pkt.put_varint(0x26);
            pkt.extend(std::iter::repeat(7u8).take(size));
            let f = encode_frame(&pkt, Some(256)).unwrap();
            let p = decode_frame(&strip_len(&f), true).unwrap();
            assert_eq!(p.id, 0x26);
            assert_eq!(p.body.len(), size);
        }
    }
}
