//! Pacote de recursos do servidor (S48): `ResourcePackRepository.downloadResourcePack` e
//! `HttpUtil.downloadResourcePack` da 1.8.9. Baixa para `~/.config/mine-rs/server-resource-packs/`,
//! com cache por hash SHA-1 e limite de 50 MiB.
//!
//! Diferenças de segurança em relação ao Java: só `http`/`https` (o `URL` do Java aceita `file:`),
//! e o pedido leva só `X-Minecraft-Version` (sem nome de usuário e UUID).

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha1::{Digest, Sha1};

/// Limite do Java: 52428800 bytes.
pub const MAX_SIZE: u64 = 52_428_800;
/// Quantos arquivos antigos ficam na pasta (`deleteOldServerResourcesPacks`).
pub const KEEP: usize = 10;

/// `C19PacketResourcePackStatus.Action` (ordem do enum = valor no pacote).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    SuccessfullyLoaded = 0,
    Declined = 1,
    FailedDownload = 2,
    Accepted = 3,
}

/// Pacote C19 com o hash truncado em 40 caracteres (como o construtor do Java).
pub fn status_packet(hash: &str, status: Status) -> Vec<u8> {
    let h: String = hash.chars().take(40).collect();
    mc_protocol::serverbound::resource_pack_status(&h, status as i32)
}

pub fn dir() -> PathBuf {
    crate::settings::config_dir().join("server-resource-packs")
}

pub fn sha1_hex(bytes: &[u8]) -> String {
    Sha1::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Nome do arquivo em cache: o próprio hash (40 hex minúsculos) ou "legacy".
pub fn cache_name(hash: &str) -> String {
    if hash.len() == 40 && hash.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        hash.to_string()
    } else {
        "legacy".to_string()
    }
}

/// Mantém os `KEEP` arquivos mais recentes da pasta e apaga o resto.
pub fn prune_old(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .map(|e| (e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH), e.path()))
        .collect();
    files.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, p) in files.into_iter().skip(KEEP) {
        let _ = std::fs::remove_file(p);
    }
}

/// O arquivo em cache vale? Só com hash de 40 caracteres e SHA-1 igual; um arquivo com hash errado é apagado.
pub fn cache_hit(file: &Path, hash: &str) -> bool {
    if !file.exists() || hash.len() != 40 {
        return false;
    }
    match std::fs::read(file) {
        Ok(b) if sha1_hex(&b) == hash => true,
        _ => {
            let _ = std::fs::remove_file(file);
            false
        }
    }
}

/// `HttpUtil.downloadResourcePack`. `progress(mensagem, porcentagem)`.
pub fn download(url: &str, dest: &Path, max: u64, progress: &mut dyn FnMut(&str, i32)) -> Result<(), String> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(format!("endereço não suportado: {url}"));
    }
    progress("Making Request...", 0);
    let agent = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(30)).timeout_read(Duration::from_secs(60)).build();
    let resp = agent.get(url).set("X-Minecraft-Version", "1.8.9").call().map_err(|e| e.to_string())?;
    let length: i64 = resp.header("Content-Length").and_then(|v| v.parse().ok()).unwrap_or(-1);
    progress(&format!("Downloading file ({:.2} MB)...", length.max(0) as f32 / 1000.0 / 1000.0), 0);
    if dest.exists() {
        let have = std::fs::metadata(dest).map(|m| m.len() as i64).unwrap_or(-1);
        if have == length {
            return Ok(());
        }
        let _ = std::fs::remove_file(dest);
    } else if let Some(parent) = dest.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if max > 0 && length > max as i64 {
        return Err(format!("Filesize is bigger than maximum allowed (file is {length}, limit is {max})"));
    }
    let mut out = std::fs::File::create(dest).map_err(|e| e.to_string())?;
    let mut reader = resp.into_reader();
    let (mut buf, mut got) = ([0u8; 4096], 0u64);
    let result = loop {
        let n = match reader.read(&mut buf) {
            Ok(0) => break Ok(()),
            Ok(n) => n,
            Err(e) => break Err(e.to_string()),
        };
        got += n as u64;
        if max > 0 && got > max {
            break Err(format!("Filesize was bigger than maximum allowed (got >= {got}, limit was {max})"));
        }
        if let Err(e) = out.write_all(&buf[..n]) {
            break Err(e.to_string());
        }
        if length > 0 {
            progress("", (got as f32 / length as f32 * 100.0) as i32);
        }
    };
    drop(out);
    if result.is_err() {
        let _ = std::fs::remove_file(dest);
    }
    result
}

/// `ResourcePackRepository.downloadResourcePack`: devolve o arquivo pronto (do cache ou baixado).
pub fn prepare(dir: &Path, url: &str, hash: &str, progress: &mut dyn FnMut(&str, i32)) -> Result<PathBuf, String> {
    let file = dir.join(cache_name(hash));
    if cache_hit(&file, hash) {
        return Ok(file);
    }
    let _ = std::fs::create_dir_all(dir);
    prune_old(dir);
    download(url, &file, MAX_SIZE, progress)?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufRead;
    use std::net::TcpListener;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("mine-rs-srvpack-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Servidor HTTP mínimo: responde `body` a cada conexão, até `n` vezes.
    fn serve(body: Vec<u8>, n: usize) -> (String, std::thread::JoinHandle<usize>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/pack.zip", l.local_addr().unwrap());
        let h = std::thread::spawn(move || {
            let mut served = 0;
            for _ in 0..n {
                let Ok((mut s, _)) = l.accept() else { break };
                let mut r = std::io::BufReader::new(s.try_clone().unwrap());
                let mut line = String::new();
                while r.read_line(&mut line).map(|n| n > 2).unwrap_or(false) {
                    line.clear();
                }
                let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = s.write_all(&body);
                served += 1;
            }
            served
        });
        (url, h)
    }

    #[test]
    fn sha1_and_cache_name() {
        assert_eq!(sha1_hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
        let h = sha1_hex(b"abc");
        assert_eq!(cache_name(&h), h);
        assert_eq!(cache_name(&h.to_uppercase()), "legacy");
        assert_eq!(cache_name("curto"), "legacy");
        assert_eq!(cache_name(""), "legacy");
    }

    #[test]
    fn status_packets_use_java_enum_order() {
        // 0x19, string(hash), varint(ordinal).
        let p = status_packet("abc", Status::Accepted);
        assert_eq!(p, vec![0x19, 3, b'a', b'b', b'c', 3]);
        assert_eq!(status_packet("abc", Status::SuccessfullyLoaded).last(), Some(&0));
        assert_eq!(status_packet("abc", Status::Declined).last(), Some(&1));
        assert_eq!(status_packet("abc", Status::FailedDownload).last(), Some(&2));
        // Hash maior que 40 é cortado como no construtor do C19.
        let long = "x".repeat(50);
        assert_eq!(status_packet(&long, Status::Declined).len(), 1 + 1 + 40 + 1);
    }

    #[test]
    fn cache_hit_checks_hash_and_deletes_bad_file() {
        let d = tmp("cache");
        let f = d.join("x");
        std::fs::write(&f, b"abc").unwrap();
        assert!(cache_hit(&f, "a9993e364706816aba3e25717850c26c9cd0d89d"));
        assert!(f.exists());
        // Hash diferente: apaga.
        assert!(!cache_hit(&f, "0000000000000000000000000000000000000000"));
        assert!(!f.exists());
        // Hash que não tem 40 caracteres nunca vale.
        std::fs::write(&f, b"abc").unwrap();
        assert!(!cache_hit(&f, "a9993e"));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn prune_keeps_ten_newest() {
        let d = tmp("prune");
        for i in 0..13 {
            let p = d.join(format!("f{i:02}"));
            std::fs::write(&p, b"x").unwrap();
            let t = std::time::UNIX_EPOCH + Duration::from_secs(1_000_000 + i * 10);
            std::fs::File::options().write(true).open(&p).unwrap().set_modified(t).unwrap();
        }
        prune_old(&d);
        let mut left: Vec<String> = std::fs::read_dir(&d).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        left.sort();
        assert_eq!(left.len(), 10);
        assert_eq!(left[0], "f03");
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn downloads_once_then_uses_cache_by_hash() {
        let d = tmp("dl");
        let body = b"conteudo do pacote".to_vec();
        let hash = sha1_hex(&body);
        let (url, server) = serve(body.clone(), 1);
        let mut last = 0;
        let f = prepare(&d, &url, &hash, &mut |_, p| last = last.max(p)).unwrap();
        assert_eq!(f, d.join(&hash));
        assert_eq!(std::fs::read(&f).unwrap(), body);
        assert_eq!(last, 100);
        // Segunda vez: cache pelo hash, sem nova conexão (a URL nem precisa responder).
        let f2 = prepare(&d, "http://127.0.0.1:1/nada", &hash, &mut |_, _| {}).unwrap();
        assert_eq!(f2, f);
        assert_eq!(server.join().unwrap(), 1);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn rejects_non_http_and_oversized() {
        let d = tmp("bad");
        assert!(prepare(&d, "file:///etc/passwd", "", &mut |_, _| {}).is_err());
        assert!(prepare(&d, "ftp://x/y", "", &mut |_, _| {}).is_err());
        let (url, server) = serve(vec![7u8; 5000], 1);
        let r = download(&url, &d.join("grande"), 1000, &mut |_, _| {});
        assert!(r.unwrap_err().contains("bigger than maximum"));
        assert!(!d.join("grande").exists(), "arquivo parcial não fica");
        let _ = server.join();
        let _ = std::fs::remove_dir_all(d);
    }
}
