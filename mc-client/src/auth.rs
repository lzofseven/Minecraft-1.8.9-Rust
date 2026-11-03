//! Login online: sessão Microsoft/Mojang + resposta de criptografia.

use num_bigint::BigInt;
use rand::RngCore;
use rsa::{pkcs8::DecodePublicKey, Pkcs1v15Encrypt, RsaPublicKey};
use sha1::{Digest, Sha1};

use mc_protocol::{Reader, WriteExt};

use crate::net::BoxErr;

#[derive(Clone)]
pub struct Session {
    pub name: String,
    /// UUID sem hífens.
    pub uuid: String,
    pub access_token: String,
}

impl Session {
    /// Lê uma sessão salva por um launcher (`auth.json` com `name`, `uuid` e `access_token`).
    pub fn load(path: &str) -> Result<Self, BoxErr> {
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        let field = |k: &str| {
            v[k].as_str().map(str::to_string).ok_or_else(|| format!("campo '{k}' ausente em {path}"))
        };
        Ok(Self { name: field("name")?, uuid: field("uuid")?, access_token: field("access_token")? })
    }
}

/// Hash do servidor no formato do Minecraft (SHA-1 como inteiro com sinal, em hex).
pub fn server_hash(server_id: &str, secret: &[u8], public_key_der: &[u8]) -> String {
    let mut h = Sha1::new();
    h.update(server_id.as_bytes());
    h.update(secret);
    h.update(public_key_der);
    BigInt::from_signed_bytes_be(&h.finalize()).to_str_radix(16)
}

pub struct EncryptionReply {
    pub packet: Vec<u8>,
    pub secret: [u8; 16],
    pub hash: String,
}

/// Processa o Encryption Request (0x01) e monta o Encryption Response.
pub fn answer_encryption_request(body: &[u8]) -> Result<EncryptionReply, BoxErr> {
    let mut r = Reader::new(body);
    let server_id = r.string()?;
    let key_len = r.varint()? as usize;
    let key_der = r.bytes(key_len)?.to_vec();
    let tok_len = r.varint()? as usize;
    let token = r.bytes(tok_len)?.to_vec();

    let key = RsaPublicKey::from_public_key_der(&key_der)?;
    let mut secret = [0u8; 16];
    let mut rng = rand::thread_rng();
    rng.fill_bytes(&mut secret);

    let enc_secret = key.encrypt(&mut rng, Pkcs1v15Encrypt, &secret)?;
    let enc_token = key.encrypt(&mut rng, Pkcs1v15Encrypt, &token)?;

    let mut packet = Vec::new();
    packet.put_varint(0x01);
    packet.put_varint(enc_secret.len() as i32);
    packet.extend_from_slice(&enc_secret);
    packet.put_varint(enc_token.len() as i32);
    packet.extend_from_slice(&enc_token);

    Ok(EncryptionReply { hash: server_hash(&server_id, &secret, &key_der), packet, secret })
}

/// Registra o join na sessão da Mojang (bloqueante).
pub fn session_join(s: &Session, hash: &str) -> Result<(), BoxErr> {
    let res = ureq::post("https://sessionserver.mojang.com/session/minecraft/join").send_json(
        serde_json::json!({
            "accessToken": s.access_token,
            "selectedProfile": s.uuid,
            "serverId": hash,
        }),
    );
    match res {
        Ok(_) => Ok(()),
        Err(ureq::Error::Status(code, _)) => Err(format!(
            "sessão Mojang recusou o join (HTTP {code}); entre de novo na conta (menu principal → Conta)"
        )
        .into()),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Vetores conhecidos da wiki.vg (hash de login do Minecraft).
    #[test]
    fn known_hashes() {
        let h = |s: &str| {
            let mut x = Sha1::new();
            x.update(s.as_bytes());
            BigInt::from_signed_bytes_be(&x.finalize()).to_str_radix(16)
        };
        assert_eq!(h("Notch"), "4ed1f46bbe04bc756bcb17c0c7ce3e4632f06a48");
        assert_eq!(h("jeb_"), "-7c9d5b0044c130109a5d7b5fb5c317c02b4e28c1");
        assert_eq!(h("simon"), "88e16a1019277b15d58faf0541e11910eb756f6");
    }
}
