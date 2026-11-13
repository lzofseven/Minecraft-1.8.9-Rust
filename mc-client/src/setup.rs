//! Arquivos do jogo: o jar 1.8.9 (texturas, modelos) e os assets (sons, idiomas). Usa os de um launcher
//! instalado (`~/.minecraft`, Prism Launcher) e, sem eles, baixa uma vez dos servidores da Mojang para a pasta
//! de dados do mine-rs, conferindo o SHA-1 de cada arquivo.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;
use sha1::{Digest, Sha1};

use crate::net::BoxErr;

const MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const RESOURCES: &str = "https://resources.download.minecraft.net";

fn env_dir(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).filter(|v| !v.is_empty()).map(PathBuf::from)
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}

/// `~/.local/share/mine-rs` (ou `$XDG_DATA_HOME/mine-rs`); no Windows, `%LOCALAPPDATA%\mine-rs`.
pub fn data_dir() -> PathBuf {
    if cfg!(windows) {
        return env_dir("LOCALAPPDATA").or_else(|| env_dir("APPDATA")).unwrap_or_default().join("mine-rs");
    }
    std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".local/share")).join("mine-rs")
}

/// Os jars da 1.8.9 dos launchers instalados (oficial e Prism), na ordem de preferência.
pub fn launcher_jars() -> Vec<PathBuf> {
    let mut v = Vec::new();
    v.extend(mc_render::resources::minecraft_dir().map(|d| d.join("versions/1.8.9/1.8.9.jar")));
    v.extend(mc_render::resources::prism_dir().map(|d| d.join("libraries/com/mojang/minecraft/1.8.9/minecraft-1.8.9-client.jar")));
    v
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(Duration::from_secs(60)).build()
}

fn get_bytes(url: &str, limit: u64) -> Result<Vec<u8>, BoxErr> {
    let mut buf = Vec::new();
    agent().get(url).call()?.into_reader().take(limit).read_to_end(&mut buf)?;
    Ok(buf)
}

fn sha1_hex(b: &[u8]) -> String {
    Sha1::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

/// Grava só se o SHA-1 bate.
fn download_checked(url: &str, sha1: &str, dst: &Path, limit: u64) -> Result<(), BoxErr> {
    let b = get_bytes(url, limit)?;
    if sha1_hex(&b) != sha1 {
        return Err(format!("SHA-1 diferente em {url}").into());
    }
    if let Some(d) = dst.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = dst.with_extension("tmp");
    std::fs::write(&tmp, &b)?;
    mc_world::replace::replace_file(&tmp, dst)?;
    Ok(())
}

/// O JSON da versão 1.8.9 pelo manifesto da Mojang.
fn version_json() -> Result<Value, BoxErr> {
    let m: Value = agent().get(MANIFEST).call()?.into_json()?;
    let url = m["versions"].as_array().and_then(|v| v.iter().find(|v| v["id"] == "1.8.9")).and_then(|v| v["url"].as_str()).ok_or("1.8.9 fora do manifesto")?;
    Ok(agent().get(url).call()?.into_json()?)
}

/// O jar do jogo: `--jar`, `MINE_RS_JAR`, um launcher instalado ou o baixado.
pub fn jar(explicit: Option<&str>) -> Result<PathBuf, BoxErr> {
    if let Some(p) = explicit.map(PathBuf::from).or_else(|| std::env::var_os("MINE_RS_JAR").map(PathBuf::from)) {
        return Ok(p);
    }
    let mine = data_dir().join("minecraft-1.8.9.jar");
    for p in launcher_jars().into_iter().chain([mine.clone()]) {
        if p.is_file() {
            return Ok(p);
        }
    }
    crate::winapp::progress_console();
    println!("Baixando o Minecraft 1.8.9 da Mojang (uma vez)...");
    let v = version_json()?;
    let c = &v["downloads"]["client"];
    download_checked(c["url"].as_str().ok_or("sem jar no JSON da versão")?, c["sha1"].as_str().unwrap_or_default(), &mine, 64 << 20)?;
    Ok(mine)
}

/// Os assets (índice 1.8): os de um launcher ou, sem nenhum, baixados para a pasta de dados. Define
/// `MINE_RS_ASSETS` para o resto do jogo achar a pasta.
pub fn assets() -> Result<(), BoxErr> {
    if mc_render::resources::find_assets_dir().is_some() {
        return Ok(());
    }
    let dir = data_dir().join("assets");
    let index_path = dir.join("indexes/1.8.json");
    if !index_path.is_file() {
        crate::winapp::progress_console();
        println!("Baixando o índice de sons e idiomas da Mojang (uma vez)...");
        let v = version_json()?;
        let ai = &v["assetIndex"];
        download_checked(ai["url"].as_str().ok_or("sem índice de assets")?, ai["sha1"].as_str().unwrap_or_default(), &index_path, 8 << 20)?;
    }
    let index: Value = serde_json::from_slice(&std::fs::read(&index_path)?)?;
    let objects: Vec<(String, u64)> = index["objects"]
        .as_object()
        .map(|o| o.values().filter_map(|v| Some((v["hash"].as_str()?.to_string(), v["size"].as_u64().unwrap_or(0)))).collect())
        .unwrap_or_default();
    let missing: Vec<(String, u64)> = objects.into_iter().filter(|(h, _)| !dir.join("objects").join(&h[..2]).join(h).is_file()).collect();
    if !missing.is_empty() {
        let total: u64 = missing.iter().map(|m| m.1).sum();
        crate::winapp::progress_console();
        println!("Baixando {} sons e idiomas ({} MB, uma vez)...", missing.len(), total >> 20);
        let count = missing.len();
        let queue = Arc::new(Mutex::new(missing));
        let done = Arc::new(AtomicUsize::new(0));
        let errors = Arc::new(Mutex::new(Vec::new()));
        let workers: Vec<_> = (0..8)
            .map(|_| {
                let (queue, done, errors, dir) = (queue.clone(), done.clone(), errors.clone(), dir.clone());
                std::thread::spawn(move || loop {
                    let Some((h, _)) = queue.lock().ok().and_then(|mut q| q.pop()) else { break };
                    let dst = dir.join("objects").join(&h[..2]).join(&h);
                    if let Err(e) = download_checked(&format!("{RESOURCES}/{}/{h}", &h[..2]), &h, &dst, 64 << 20) {
                        if let Ok(mut v) = errors.lock() {
                            v.push(e.to_string());
                        }
                    }
                    done.fetch_add(1, Ordering::Relaxed);
                })
            })
            .collect();
        // Progresso numa linha só, para quem abriu pelo atalho não achar que travou.
        while workers.iter().any(|w| !w.is_finished()) {
            let d = done.load(Ordering::Relaxed);
            print!("\r  {d} de {count} ({}%)   ", d * 100 / count.max(1));
            let _ = std::io::Write::flush(&mut std::io::stdout());
            std::thread::sleep(Duration::from_millis(500));
        }
        println!("\r  {count} de {count} (100%)   ");
        for w in workers {
            let _ = w.join();
        }
        if let Some(e) = errors.lock().ok().and_then(|v| v.first().cloned()) {
            eprintln!("alguns assets não baixaram ({e}); o jogo segue sem eles");
        }
    }
    std::env::set_var("MINE_RS_ASSETS", &dir);
    Ok(())
}
