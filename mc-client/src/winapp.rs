//! Windows: o jogo abre sem janela de console (`windows_subsystem`). Aberto por um terminal, escreve nele;
//! aberto pelo atalho, a saída vai para `%LOCALAPPDATA%\mine-rs\mine-rs.log`, um console só aparece no
//! primeiro uso, com o progresso dos downloads, e os erros fatais viram uma caixa de mensagem. Fora do
//! Windows, tudo aqui é nada.

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};

    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    const STD_ERROR_HANDLE: u32 = -12i32 as u32;
    const MB_ICONERROR: u32 = 0x10;
    const TITLE: &str = "Minecraft 1.8.9 Rust";

    #[link(name = "kernel32")]
    extern "system" {
        fn AttachConsole(pid: u32) -> i32;
        fn AllocConsole() -> i32;
        fn FreeConsole() -> i32;
        fn GetConsoleWindow() -> *mut c_void;
        fn SetConsoleTitleW(title: *const u16) -> i32;
        fn SetStdHandle(which: u32, handle: *mut c_void) -> i32;
    }
    #[link(name = "user32")]
    extern "system" {
        fn MessageBoxW(wnd: *mut c_void, text: *const u16, caption: *const u16, kind: u32) -> i32;
    }

    /// Console criado por nós (e não o do terminal que abriu o jogo).
    static OWN: AtomicBool = AtomicBool::new(false);

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain([0]).collect()
    }

    /// Sem console: saída padrão e de erro para o `mine-rs.log` da pasta de dados (`novo`: começa do zero).
    fn log_to_file(novo: bool) {
        use std::os::windows::io::IntoRawHandle;
        let dir = crate::setup::data_dir();
        let _ = std::fs::create_dir_all(&dir);
        let Ok(f) = std::fs::OpenOptions::new().create(true).write(true).append(!novo).truncate(novo).open(dir.join("mine-rs.log")) else { return };
        let Ok(f2) = f.try_clone() else { return };
        unsafe {
            SetStdHandle(STD_OUTPUT_HANDLE, f.into_raw_handle());
            SetStdHandle(STD_ERROR_HANDLE, f2.into_raw_handle());
        }
    }

    pub fn init() {
        if unsafe { AttachConsole(ATTACH_PARENT_PROCESS) } == 0 {
            log_to_file(true);
        }
        let default = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            default(info);
            fatal(&format!("O jogo fechou por um erro interno:\n\n{info}"));
        }));
    }

    pub fn progress_console() {
        unsafe {
            if GetConsoleWindow().is_null() && AllocConsole() != 0 {
                OWN.store(true, Ordering::Relaxed);
                SetConsoleTitleW(wide(&format!("{TITLE} - baixando arquivos do jogo (só na primeira vez)")).as_ptr());
            }
        }
    }

    pub fn progress_done() {
        if OWN.swap(false, Ordering::Relaxed) {
            unsafe {
                FreeConsole();
            }
            log_to_file(false);
        }
    }

    pub fn fatal(msg: &str) {
        unsafe {
            MessageBoxW(std::ptr::null_mut(), wide(msg).as_ptr(), wide(TITLE).as_ptr(), MB_ICONERROR);
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn init() {}
    pub fn progress_console() {}
    pub fn progress_done() {}
    pub fn fatal(_msg: &str) {}
}

/// No começo do `main`: liga ao console do terminal, se houver, e mostra os pânicos numa caixa.
pub use imp::init;
/// Antes de um download do primeiro uso: abre um console para o progresso, se não houver.
pub use imp::progress_console;
/// Downloads prontos: fecha o console aberto por `progress_console`.
pub use imp::progress_done;
/// Erro que fecha o jogo, numa caixa de mensagem.
pub use imp::fatal;
