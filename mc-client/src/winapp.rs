//! Windows: o jogo abre sem janela de console (`windows_subsystem`). Aberto por um terminal, escreve nele;
//! aberto pelo atalho, a saída vai para `%LOCALAPPDATA%\mine-rs\mine-rs.log`, um console só aparece no
//! primeiro uso, com o progresso dos downloads, e os erros fatais viram uma caixa de mensagem. Fora do
//! Windows, tudo aqui é nada.
//!
//! `FreeConsole` zera os handles padrão. Por isso, ao fechar o console do download, os handles do
//! `mine-rs.log` (abertos no começo e guardados) voltam com `SetStdHandle`. O stdout do Rust relê
//! esse handle a cada gravação, e o resto da sessão continua no log.

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    const STD_ERROR_HANDLE: u32 = -12i32 as u32;
    const MB_ICONERROR: u32 = 0x10;
    const CP_UTF8: u32 = 65001;
    const GENERIC_WRITE: u32 = 0x4000_0000;
    const FILE_SHARE_READ: u32 = 1;
    const FILE_SHARE_WRITE: u32 = 2;
    const OPEN_EXISTING: u32 = 3;
    const DUPLICATE_SAME_ACCESS: u32 = 2;
    const TITLE: &str = "Minecraft 1.8.9 Rust";

    #[link(name = "kernel32")]
    extern "system" {
        fn AttachConsole(pid: u32) -> i32;
        fn AllocConsole() -> i32;
        fn FreeConsole() -> i32;
        fn GetConsoleWindow() -> *mut c_void;
        fn SetConsoleTitleW(title: *const u16) -> i32;
        fn SetConsoleOutputCP(cp: u32) -> i32;
        fn SetStdHandle(which: u32, handle: *mut c_void) -> i32;
        fn CreateFileW(name: *const u16, access: u32, share: u32, sec: *mut c_void, disp: u32, flags: u32, template: *mut c_void) -> *mut c_void;
        fn GetCurrentProcess() -> *mut c_void;
        fn DuplicateHandle(src_proc: *mut c_void, src: *mut c_void, dst_proc: *mut c_void, dst: *mut *mut c_void, access: u32, inherit: i32, opts: u32) -> i32;
    }
    #[link(name = "user32")]
    extern "system" {
        fn MessageBoxW(wnd: *mut c_void, text: *const u16, caption: *const u16, kind: u32) -> i32;
    }

    /// Console criado por nós (e não o terminal que abriu o jogo).
    static OWN: AtomicBool = AtomicBool::new(false);
    /// Handles do `mine-rs.log`, estáveis pela vida do processo. `FreeConsole` não os fecha.
    static LOG_OUT: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
    static LOG_ERR: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain([0]).collect()
    }

    fn invalid(h: *mut c_void) -> bool {
        h.is_null() || h == (-1isize) as *mut c_void
    }

    fn duplicate(h: *mut c_void) -> *mut c_void {
        let mut out = std::ptr::null_mut();
        let ok = unsafe { DuplicateHandle(GetCurrentProcess(), h, GetCurrentProcess(), &mut out, 0, 0, DUPLICATE_SAME_ACCESS) };
        if ok == 0 { std::ptr::null_mut() } else { out }
    }

    /// Aponta stdout e stderr para o console atual (`CONOUT$`).
    fn bind_conout() {
        let name = wide("CONOUT$");
        let h = unsafe { CreateFileW(name.as_ptr(), GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE, std::ptr::null_mut(), OPEN_EXISTING, 0, std::ptr::null_mut()) };
        if invalid(h) {
            return;
        }
        let h2 = duplicate(h);
        unsafe {
            SetStdHandle(STD_OUTPUT_HANDLE, h);
            if !invalid(h2) {
                SetStdHandle(STD_ERROR_HANDLE, h2);
            }
        }
    }

    /// Abre (ou cria) o log e guarda os handles. `novo` trunca; senão acrescenta.
    fn open_log(novo: bool) -> Option<(*mut c_void, *mut c_void)> {
        use std::os::windows::io::IntoRawHandle;
        let dir = crate::setup::data_dir();
        let _ = std::fs::create_dir_all(&dir);
        let f = std::fs::OpenOptions::new().create(true).write(true).append(!novo).truncate(novo).open(dir.join("mine-rs.log")).ok()?;
        let f2 = f.try_clone().ok()?;
        Some((f.into_raw_handle(), f2.into_raw_handle()))
    }

    fn use_handles(out: *mut c_void, err: *mut c_void) {
        unsafe {
            if !invalid(out) {
                SetStdHandle(STD_OUTPUT_HANDLE, out);
            }
            if !invalid(err) {
                SetStdHandle(STD_ERROR_HANDLE, err);
            }
        }
    }

    /// Devolve stdout/stderr ao log aberto no `init`. Se ele não existir, abre em modo append.
    fn restore_log() {
        let out = LOG_OUT.load(Ordering::Relaxed);
        let err = LOG_ERR.load(Ordering::Relaxed);
        if !invalid(out) && !invalid(err) {
            use_handles(out, err);
            return;
        }
        if let Some((out, err)) = open_log(false) {
            LOG_OUT.store(out, Ordering::Relaxed);
            LOG_ERR.store(err, Ordering::Relaxed);
            use_handles(out, err);
        }
    }

    pub fn init() {
        if unsafe { AttachConsole(ATTACH_PARENT_PROCESS) } == 0 {
            if let Some((out, err)) = open_log(true) {
                LOG_OUT.store(out, Ordering::Relaxed);
                LOG_ERR.store(err, Ordering::Relaxed);
                use_handles(out, err);
            }
        } else {
            // Subsystem GUI não herda o console: liga `CONOUT$` para o `println!` chegar no terminal.
            unsafe { SetConsoleOutputCP(CP_UTF8) };
            bind_conout();
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
                SetConsoleOutputCP(CP_UTF8);
                SetConsoleTitleW(wide(&format!("{TITLE} - baixando arquivos do jogo (só na primeira vez)")).as_ptr());
                // `AllocConsole` pode deixar os handles que já apontavam para o log.
                // `CONOUT$` garante que o progresso apareça nesta janela.
                bind_conout();
            }
        }
    }

    pub fn progress_done() {
        if OWN.swap(false, Ordering::Relaxed) {
            unsafe {
                FreeConsole();
            }
            restore_log();
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
/// Downloads prontos: fecha o console aberto por `progress_console` e devolve a saída ao log.
pub use imp::progress_done;
/// Erro que fecha o jogo, numa caixa de mensagem.
pub use imp::fatal;
