//! Um jogador: em vez do servidor integrado do jogo (que não foi portado), roda o servidor oficial 1.8.9 da
//! Mojang em segundo plano, só em `127.0.0.1`, e o cliente entra nele como num servidor qualquer.
//!
//! O `server.jar` é baixado uma vez dos servidores da Mojang (SHA-1 conferido) para `<dados>/server`. Cada
//! mundo é uma pasta em `<dados>/singleplayer` (ver `worlds`). O servidor precisa do Java 8: `MINE_RS_JAVA`, o já baixado, o
//! `jre-legacy` do Prism Launcher ou do launcher oficial; sem nenhum, baixa o `jre-legacy` da Mojang (o mesmo
//! do launcher oficial, SHA-1 de cada arquivo conferido) para `<dados>/runtime`. Ao sair, o cliente manda
//! `stop` (o servidor salva o mundo) e espera ele terminar.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::net::BoxErr;

/// O que a preparação do mundo avisa ao jogo.
pub enum Event {
    /// Linha de progresso para a tela (download, "Preparing spawn area: 40%").
    Status(String),
    /// Servidor pronto: entrar em `127.0.0.1:port`.
    Ready(LocalServer),
    Failed(String),
}

/// O servidor rodando. Ao ser descartado, salva o mundo e para.
pub struct LocalServer {
    child: Child,
    stdin: Option<ChildStdin>,
    pub port: u16,
    /// Pasta do mundo (`worlds`).
    pub folder: String,
}

impl LocalServer {
    pub fn addr(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }

    /// Comando no console do servidor.
    pub fn command(&mut self, cmd: &str) {
        if let Some(s) = &mut self.stdin {
            let _ = writeln!(s, "{cmd}");
            let _ = s.flush();
        }
    }

    /// `stop` (salva o mundo) e espera até 60 s; depois disso, mata o processo.
    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        if self.stdin.is_none() {
            return;
        }
        self.command("stop");
        self.stdin = None;
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_secs(60) {
            if let Ok(Some(_)) = self.child.try_wait() {
                println!("[um jogador] mundo salvo, servidor parado");
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        eprintln!("[um jogador] o servidor não parou em 60 s: encerrando à força");
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for LocalServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Para o servidor em outra thread (o jogo continua respondendo enquanto o mundo é salvo).
pub fn stop_async(srv: LocalServer) -> JoinHandle<()> {
    std::thread::spawn(move || srv.stop())
}

/// Pasta do mundo e do `server.properties`.
pub fn world_dir(folder: &str) -> PathBuf {
    crate::worlds::root().join(folder)
}

/// Qual mundo abrir: um da lista ou um novo do `GuiCreateWorld`.
pub struct Launch {
    pub folder: String,
    pub new: Option<crate::worlds::NewWorld>,
}

/// Prepara e sobe o servidor numa thread. `previous` é a parada de um servidor anterior, que precisa terminar
/// antes (o mundo é o mesmo e fica travado pelo `session.lock`).
pub fn start(name: String, view_distance: u8, previous: Option<JoinHandle<()>>, world: Launch) -> mpsc::Receiver<Event> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        if let Some(h) = previous {
            let _ = tx.send(Event::Status("Salvando o mundo anterior...".into()));
            let _ = h.join();
        }
        let ev = match launch(&name, view_distance, &world, &tx) {
            Ok(srv) => Event::Ready(srv),
            Err(e) => Event::Failed(e.to_string()),
        };
        let _ = tx.send(ev);
    });
    rx
}

fn launch(name: &str, view_distance: u8, world: &Launch, tx: &mpsc::Sender<Event>) -> Result<LocalServer, BoxErr> {
    let status = |s: &str| {
        let _ = tx.send(Event::Status(s.to_string()));
    };
    let java = match find_java() {
        Some(j) => j,
        None => install_java(&status).map_err(|e| format!("não deu para baixar o Java 8: {e}"))?,
    };
    status("Baixando o servidor da Mojang...");
    let jar = crate::setup::server_jar()?;
    let dir = world_dir(&world.folder);
    std::fs::create_dir_all(&dir)?;
    // mundo.json: o novo nasce com as escolhas da tela; o da lista só ganha a data de agora.
    let mut meta = match &world.new {
        Some(n) => crate::worlds::WorldEntry {
            folder: world.folder.clone(),
            name: n.name.clone(),
            mode: n.mode,
            cheats: n.cheats,
            last_played: 0,
            world_type: n.world_type,
            generator: n.generator.clone(),
            structures: n.structures,
        },
        None => crate::worlds::WorldEntry::load(&world.folder).ok_or("mundo sem mundo.json")?,
    };
    meta.last_played = crate::worlds::now_ms();
    meta.save()?;
    let port = free_port()?;
    let extra = world.new.as_ref().map(|n| n.properties()).unwrap_or_default();
    write_config(&dir, port, view_distance, &extra)?;

    status("Iniciando o servidor...");
    println!("[um jogador] {} -jar {} (porta {port}, mundo em {})", java.display(), jar.display(), dir.display());
    let mut child = Command::new(&java)
        .args(["-Xms512M", "-Xmx2G", "-jar"])
        .arg(&jar)
        .arg("nogui")
        .current_dir(&dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("não deu para rodar {}: {e}", java.display()))?;
    let stdin = child.stdin.take();
    let stdout = child.stdout.take().ok_or("servidor sem saída")?;
    let mut srv = LocalServer { child, stdin, port, folder: world.folder.clone() };

    // Lê o console até o "Done (...)!"; depois disso, só repassa para o log do jogo.
    let (line_tx, line_rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            println!("[servidor] {line}");
            let _ = line_tx.send(line);
        }
    });
    let mut last = Vec::new();
    loop {
        match line_rx.recv_timeout(Duration::from_millis(200)) {
            Ok(line) => {
                let msg = message(&line).to_string();
                if msg.starts_with("Done (") {
                    break;
                }
                if msg.contains("FAILED TO BIND") {
                    return Err(format!("o servidor não conseguiu abrir a porta {port}").into());
                }
                status(&msg);
                last.push(msg);
                if last.len() > 3 {
                    last.remove(0);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                srv.stdin = None;
                let code = srv.child.wait().ok().and_then(|s| s.code());
                return Err(format!("o servidor fechou (código {code:?}): {}", last.join(" / ")).into());
            }
        }
    }
    // "Permitir comandos" do mundo: o jogador é operador (ou deixa de ser).
    srv.command(&format!("{} {name}", if meta.cheats { "op" } else { "deop" }));
    Ok(srv)
}

/// Só a mensagem de uma linha do console (`[12:00:00] [Server thread/INFO]: mensagem`).
fn message(line: &str) -> &str {
    line.split_once("]: ").map_or(line, |(_, m)| m).trim()
}

/// Uma porta livre em 127.0.0.1.
fn free_port() -> std::io::Result<u16> {
    Ok(std::net::TcpListener::bind("127.0.0.1:0")?.local_addr()?.port())
}

/// `eula.txt` e o `server.properties`: mantém o que o jogador mudou, mas força porta, endereço local e
/// modo offline (o servidor só aceita conexões desta máquina).
/// `extra` (as escolhas de um mundo novo) também é forçado.
fn write_config(dir: &Path, port: u16, view_distance: u8, extra: &[(&str, String)]) -> std::io::Result<()> {
    std::fs::write(
        dir.join("eula.txt"),
        "# Aceito ao usar o modo Um jogador do mine-rs (https://account.mojang.com/documents/minecraft_eula)\neula=true\n",
    )?;
    let path = dir.join("server.properties");
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    let mut forced = vec![
        ("server-ip", "127.0.0.1".to_string()),
        ("server-port", port.to_string()),
        ("online-mode", "false".to_string()),
        ("max-players", "1".to_string()),
        ("view-distance", view_distance.clamp(3, 15).to_string()),
    ];
    forced.extend(extra.iter().cloned());
    let defaults = [("level-name", "world"), ("motd", "Um jogador"), ("spawn-protection", "0"), ("snooper-enabled", "false"), ("enable-query", "false"), ("enable-rcon", "false")];
    let mut out = String::new();
    let mut seen = Vec::new();
    for line in old.lines() {
        let key = line.split_once('=').map(|(k, _)| k.trim());
        match key.and_then(|k| forced.iter().find(|(f, _)| *f == k)) {
            Some((k, v)) => {
                out.push_str(&format!("{k}={v}\n"));
                seen.push(k.to_string());
            }
            None => {
                out.push_str(line);
                out.push('\n');
                if let Some(k) = key {
                    seen.push(k.to_string());
                }
            }
        }
    }
    for (k, v) in forced.iter().map(|(k, v)| (*k, v.as_str())).chain(defaults) {
        if !seen.iter().any(|s| s == k) {
            out.push_str(&format!("{k}={v}\n"));
        }
    }
    std::fs::write(path, out)
}

/// O Java 8: `MINE_RS_JAVA`, o baixado pelo mine-rs ou o `jre-legacy` do Prism ou do launcher oficial.
fn find_java() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("MINE_RS_JAVA").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(p));
    }
    if let Some(p) = std::fs::read_to_string(runtime_dir().join(READY)).ok().map(|rel| runtime_dir().join(rel.trim())) {
        if p.is_file() {
            return Some(p);
        }
    }
    let exe = if cfg!(windows) { "java.exe" } else { "java" };
    let mut cands = Vec::new();
    if let Some(d) = mc_render::resources::prism_dir() {
        cands.push(d.join("java/jre-legacy/bin").join(exe));
    }
    if let Some(d) = mc_render::resources::minecraft_dir() {
        // runtime/jre-legacy/<plataforma>/jre-legacy/bin/java
        if let Ok(rd) = std::fs::read_dir(d.join("runtime/jre-legacy")) {
            for e in rd.flatten() {
                cands.push(e.path().join("jre-legacy/bin").join(exe));
            }
        }
    }
    cands.into_iter().find(|p| p.is_file())
}

/// Lista dos Javas do launcher oficial, por plataforma e componente.
const JAVA_RUNTIMES: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";
/// Marca de instalação completa: o caminho do `java` relativo a `runtime_dir`.
const READY: &str = ".pronto";

fn runtime_dir() -> PathBuf {
    crate::setup::data_dir().join("runtime/jre-legacy")
}

/// A chave da plataforma no `all.json`.
fn platform() -> Option<&'static str> {
    Some(match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "linux",
        ("linux", "x86") => "linux-i386",
        ("windows", "x86_64") => "windows-x64",
        ("windows", "x86") => "windows-x86",
        ("windows", "aarch64") => "windows-arm64",
        ("macos", "x86_64") => "mac-os",
        ("macos", "aarch64") => "mac-os-arm64",
        _ => return None,
    })
}

fn sha1_hex(b: &[u8]) -> String {
    use sha1::{Digest, Sha1};
    Sha1::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

/// Caminho do manifesto sem `..` nem raiz (os arquivos ficam dentro de `runtime_dir`).
fn safe_rel(p: &str) -> Option<PathBuf> {
    let path = Path::new(p);
    path.components().all(|c| matches!(c, std::path::Component::Normal(_))).then(|| path.to_path_buf())
}

/// Baixa o `jre-legacy` da Mojang (como o launcher oficial: a versão LZMA quando há, SHA-1 conferido antes
/// e depois de descomprimir). Arquivos já certos ficam, então uma queda no meio continua de onde parou.
fn install_java(status: &(dyn Fn(&str) + Sync)) -> Result<PathBuf, BoxErr> {
    use rayon::prelude::*;
    use std::io::Read;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let plat = platform().ok_or("plataforma sem Java da Mojang")?;
    status("Procurando o Java 8 da Mojang...");
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(120)).build();
    let all: serde_json::Value = agent.get(JAVA_RUNTIMES).call()?.into_json()?;
    let entry = &all[plat]["jre-legacy"][0]["manifest"];
    let url = entry["url"].as_str().ok_or("a Mojang não tem o jre-legacy para esta plataforma")?;
    let want = entry["sha1"].as_str().unwrap_or_default();
    let mut raw = Vec::new();
    agent.get(url).call()?.into_reader().take(16 << 20).read_to_end(&mut raw)?;
    if sha1_hex(&raw) != want {
        return Err("SHA-1 diferente no manifesto do Java".into());
    }
    let manifest: serde_json::Value = serde_json::from_slice(&raw)?;
    let files = manifest["files"].as_object().ok_or("manifesto do Java sem arquivos")?;

    let root = runtime_dir();
    let _ = std::fs::remove_file(root.join(READY));
    let mut plain = Vec::new();
    let mut links = Vec::new();
    for (name, f) in files {
        let rel = safe_rel(name).ok_or_else(|| format!("caminho inválido no manifesto: {name}"))?;
        match f["type"].as_str() {
            Some("directory") => std::fs::create_dir_all(root.join(&rel))?,
            Some("file") => plain.push((rel, f)),
            Some("link") => links.push((rel, f["target"].as_str().unwrap_or_default().to_string())),
            _ => {}
        }
    }
    let java_rel = plain
        .iter()
        .map(|(rel, _)| rel)
        .find(|rel| rel.ends_with(if cfg!(windows) { "bin/java.exe" } else { "bin/java" }) && !rel.to_string_lossy().contains("jre/bin"))
        .or_else(|| plain.iter().map(|(rel, _)| rel).find(|rel| rel.ends_with(if cfg!(windows) { "bin/java.exe" } else { "bin/java" })))
        .cloned()
        .ok_or("o manifesto não tem bin/java")?;

    let total = plain.len();
    let done = AtomicUsize::new(0);
    let mb = AtomicUsize::new(0);
    let one = |rel: &PathBuf, f: &serde_json::Value| -> Result<(), String> {
        let dst = root.join(rel);
        let raw_info = &f["downloads"]["raw"];
        let raw_sha = raw_info["sha1"].as_str().ok_or("arquivo sem SHA-1")?;
        let ok = std::fs::read(&dst).map(|b| sha1_hex(&b) == raw_sha).unwrap_or(false);
        if !ok {
            let (info, lzma) = match f["downloads"].get("lzma") {
                Some(l) if l["url"].is_string() => (l, true),
                _ => (raw_info, false),
            };
            let url = info["url"].as_str().ok_or("arquivo sem URL")?;
            let mut got = Vec::new();
            agent.get(url).call().map_err(|e| e.to_string())?.into_reader().take(256 << 20).read_to_end(&mut got).map_err(|e| e.to_string())?;
            if sha1_hex(&got) != info["sha1"].as_str().unwrap_or_default() {
                return Err(format!("SHA-1 diferente em {}", rel.display()));
            }
            mb.fetch_add(got.len(), Ordering::Relaxed);
            let bytes = if lzma {
                let mut out = Vec::new();
                lzma_rs::lzma_decompress(&mut &got[..], &mut out).map_err(|e| format!("{}: {e}", rel.display()))?;
                out
            } else {
                got
            };
            if sha1_hex(&bytes) != raw_sha {
                return Err(format!("SHA-1 diferente em {} (descomprimido)", rel.display()));
            }
            if let Some(d) = dst.parent() {
                std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
            }
            std::fs::write(&dst, &bytes).map_err(|e| e.to_string())?;
        }
        #[cfg(unix)]
        if f["executable"].as_bool() == Some(true) {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dst, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
        }
        let n = done.fetch_add(1, Ordering::Relaxed) + 1;
        if n % 5 == 0 || n == total {
            status(&format!("Baixando o Java 8 da Mojang: {n}/{total} arquivos ({} MB)", mb.load(Ordering::Relaxed) >> 20));
        }
        Ok(())
    };
    let pool = rayon::ThreadPoolBuilder::new().num_threads(8).build()?;
    pool.install(|| plain.par_iter().try_for_each(|(rel, f)| one(rel, f)))?;
    #[cfg(unix)]
    for (rel, target) in links {
        let dst = root.join(&rel);
        let _ = std::fs::remove_file(&dst);
        std::os::unix::fs::symlink(&target, &dst)?;
    }
    #[cfg(not(unix))]
    let _ = links;
    std::fs::write(root.join(READY), java_rel.to_string_lossy().as_bytes())?;
    println!("[um jogador] Java 8 instalado em {}", root.display());
    Ok(root.join(java_rel))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mensagem_do_console() {
        assert_eq!(message("[12:00:01] [Server thread/INFO]: Done (2.1s)! For help, type \"help\""), "Done (2.1s)! For help, type \"help\"");
        assert_eq!(message("sem prefixo"), "sem prefixo");
    }

    /// Ponta a ponta (rede, ~70 MB): `XDG_DATA_HOME=/tmp/x cargo test --release -p mc-client sobe_o_servidor -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn sobe_o_servidor() {
        let status = |s: &str| println!("status: {s}");
        let java = install_java(&status).unwrap();
        assert_eq!(find_java(), Some(java));
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for ev in rx {
                if let Event::Status(s) = ev {
                    println!("status: {s}");
                }
            }
        });
        let new = crate::worlds::NewWorld {
            name: "Plano".into(),
            seed: "teste".into(),
            mode: crate::worlds::Mode::Creative,
            structures: true,
            world_type: 1,
            cheats: true,
            generator: crate::worlds::FLAT_PRESETS[1].2.into(),
        };
        let folder = crate::worlds::folder_for(&new.name);
        let srv = launch("Teste", 4, &Launch { folder: folder.clone(), new: Some(new) }, &tx).unwrap();
        assert!(std::net::TcpStream::connect(srv.addr()).is_ok());
        srv.stop();
        let w = crate::worlds::list().into_iter().find(|w| w.folder == folder).unwrap();
        assert_eq!((w.name.as_str(), w.mode, w.cheats, w.world_type), ("Plano", crate::worlds::Mode::Creative, true, 1));
        // String.hashCode("teste") como semente, igual ao jogo.
        assert_eq!(w.seed(), Some(110251539));
        let props = std::fs::read_to_string(world_dir(&folder).join("server.properties")).unwrap();
        // O servidor regrava o arquivo escapando o `:` (`java.util.Properties`).
        assert!(props.contains("level-type=FLAT"), "{props}");
        assert!(props.contains("230*minecraft\\:stone") || props.contains("230*minecraft:stone"), "{props}");
        // Reabrir: as escolhas ficam.
        let srv = launch("Teste", 4, &Launch { folder: folder.clone(), new: None }, &tx).unwrap();
        srv.stop();
    }

    #[test]
    fn caminho_do_manifesto() {
        assert!(safe_rel("bin/java").is_some());
        assert!(safe_rel("../x").is_none());
        assert!(safe_rel("/etc/passwd").is_none());
        assert!(safe_rel("lib/../../x").is_none());
    }

    #[test]
    fn config_mantem_o_do_jogador_e_forca_o_local() {
        let dir = std::env::temp_dir().join(format!("mine-rs-sp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("server.properties"), "#comentário\ndifficulty=3\nserver-port=25565\nonline-mode=true\ngamemode=1\n").unwrap();
        write_config(&dir, 41234, 12, &[("level-type", "FLAT".to_string())]).unwrap();
        let s = std::fs::read_to_string(dir.join("server.properties")).unwrap();
        for l in ["#comentário", "difficulty=3", "gamemode=1", "level-type=FLAT", "server-port=41234", "online-mode=false", "server-ip=127.0.0.1", "level-name=world", "view-distance=12"] {
            assert!(s.lines().any(|x| x == l), "falta {l} em:\n{s}");
        }
        assert!(!s.contains("25565") && !s.contains("online-mode=true"));
        assert!(std::fs::read_to_string(dir.join("eula.txt")).unwrap().contains("eula=true"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
