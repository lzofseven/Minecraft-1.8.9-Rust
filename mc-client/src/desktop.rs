//! Integração com o desktop: área de transferência (`GuiScreen.getClipboardString`/`setClipboardString`)
//! e abrir link (`openWebLink`). Sem dependência
//! nova: usa o programa do sistema que existir (`wl-clipboard`, `xclip` ou `xsel`) no ambiente
//! gráfico do próprio processo (`DISPLAY`/`WAYLAND_DISPLAY`). No Windows, a API do sistema.

#[cfg(not(windows))]
use std::io::Write;
#[cfg(not(windows))]
use std::process::{Command, Stdio};

#[cfg(windows)]
use win::{get as get_cmd, open as open_cmd, set as set_cmd};

/// `getClipboardString`: vazio se não houver conteúdo ou programa.
pub fn get() -> String {
    get_cmd()
}

/// `setClipboardString`: ignora texto vazio, como o Java. Devolve se algum programa aceitou.
pub fn set(text: &str) -> bool {
    !text.is_empty() && set_cmd(text)
}

/// `openWebLink`: abre a URL no navegador padrão (`xdg-open`; no Windows, `ShellExecute`).
pub fn open_url(url: &str) -> bool {
    open_cmd(url)
}

/// Comandos de leitura, na ordem de preferência.
#[cfg(not(windows))]
const READERS: &[&[&str]] = &[&["wl-paste", "-n"], &["xclip", "-o", "-selection", "clipboard"], &["xsel", "-b", "-o"]];
/// Comandos de escrita (o texto vai pela entrada padrão).
#[cfg(not(windows))]
const WRITERS: &[&[&str]] = &[&["wl-copy"], &["xclip", "-selection", "clipboard"], &["xsel", "-b", "-i"]];

#[cfg(not(windows))]
fn get_cmd() -> String {
    for cmd in READERS {
        if let Ok(out) = Command::new(cmd[0]).args(&cmd[1..]).stdin(Stdio::null()).stderr(Stdio::null()).output() {
            if out.status.success() {
                return String::from_utf8_lossy(&out.stdout).into_owned();
            }
        }
    }
    String::new()
}

#[cfg(not(windows))]
fn set_cmd(text: &str) -> bool {
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

#[cfg(not(windows))]
fn open_cmd(url: &str) -> bool {
    Command::new("xdg-open").arg(url).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().is_ok()
}

/// Área de transferência (`CF_UNICODETEXT`) e `ShellExecute` do Windows.
#[cfg(windows)]
mod win {
    use std::ffi::c_void;
    use std::ptr::null_mut;

    const CF_UNICODETEXT: u32 = 13;
    const GMEM_MOVEABLE: u32 = 2;
    const SW_SHOWNORMAL: i32 = 1;

    #[link(name = "user32")]
    extern "system" {
        fn OpenClipboard(owner: *mut c_void) -> i32;
        fn CloseClipboard() -> i32;
        fn EmptyClipboard() -> i32;
        fn GetClipboardData(format: u32) -> *mut c_void;
        fn SetClipboardData(format: u32, mem: *mut c_void) -> *mut c_void;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalAlloc(flags: u32, bytes: usize) -> *mut c_void;
        fn GlobalLock(mem: *mut c_void) -> *mut c_void;
        fn GlobalUnlock(mem: *mut c_void) -> i32;
        fn GlobalFree(mem: *mut c_void) -> *mut c_void;
    }
    #[link(name = "shell32")]
    extern "system" {
        fn ShellExecuteW(wnd: *mut c_void, op: *const u16, file: *const u16, params: *const u16, dir: *const u16, show: i32) -> isize;
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain([0]).collect()
    }

    pub fn get() -> String {
        let mut out = String::new();
        unsafe {
            if OpenClipboard(null_mut()) == 0 {
                return out;
            }
            let h = GetClipboardData(CF_UNICODETEXT);
            let p = if h.is_null() { std::ptr::null() } else { GlobalLock(h) as *const u16 };
            if !p.is_null() {
                let mut n = 0;
                while *p.add(n) != 0 {
                    n += 1;
                }
                out = String::from_utf16_lossy(std::slice::from_raw_parts(p, n));
                GlobalUnlock(h);
            }
            CloseClipboard();
        }
        out
    }

    pub fn set(text: &str) -> bool {
        let w = wide(text);
        unsafe {
            if OpenClipboard(null_mut()) == 0 {
                return false;
            }
            EmptyClipboard();
            let h = GlobalAlloc(GMEM_MOVEABLE, w.len() * 2);
            let mut ok = false;
            if !h.is_null() {
                let p = GlobalLock(h) as *mut u16;
                if !p.is_null() {
                    std::ptr::copy_nonoverlapping(w.as_ptr(), p, w.len());
                    GlobalUnlock(h);
                    ok = !SetClipboardData(CF_UNICODETEXT, h).is_null();
                }
                // Aceito, o sistema passa a ser o dono da memória.
                if !ok {
                    GlobalFree(h);
                }
            }
            CloseClipboard();
            ok
        }
    }

    pub fn open(target: &str) -> bool {
        unsafe { ShellExecuteW(null_mut(), wide("open").as_ptr(), wide(target).as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL) > 32 }
    }
}
