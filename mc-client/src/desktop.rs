//! Integração com o desktop: área de transferência (`GuiScreen.getClipboardString`/`setClipboardString`)
//! e abrir link (`openWebLink`). Sem dependência
//! nova: usa o programa do sistema que existir (`wl-clipboard`, `xclip` ou `xsel`) no ambiente
//! gráfico do próprio processo (`DISPLAY`/`WAYLAND_DISPLAY`).

use std::io::Write;
use std::process::{Command, Stdio};

/// Comandos de leitura, na ordem de preferência.
const READERS: &[&[&str]] = &[&["wl-paste", "-n"], &["xclip", "-o", "-selection", "clipboard"], &["xsel", "-b", "-o"]];
/// Comandos de escrita (o texto vai pela entrada padrão).
const WRITERS: &[&[&str]] = &[&["wl-copy"], &["xclip", "-selection", "clipboard"], &["xsel", "-b", "-i"]];

/// `getClipboardString`: vazio se não houver conteúdo ou programa.
pub fn get() -> String {
    for cmd in READERS {
        if let Ok(out) = Command::new(cmd[0]).args(&cmd[1..]).stdin(Stdio::null()).stderr(Stdio::null()).output() {
            if out.status.success() {
                return String::from_utf8_lossy(&out.stdout).into_owned();
            }
        }
    }
    String::new()
}

/// `setClipboardString`: ignora texto vazio, como o Java. Devolve se algum programa aceitou.
pub fn set(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    for cmd in WRITERS {
        let child = Command::new(cmd[0]).args(&cmd[1..]).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
        if let Ok(mut child) = child {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            if child.wait().is_ok_and(|s| s.success()) {
                return true;
            }
        }
    }
    false
}

/// `openWebLink`: abre a URL no navegador padrão (`xdg-open`).
pub fn open_url(url: &str) -> bool {
    Command::new("xdg-open").arg(url).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().is_ok()
}
