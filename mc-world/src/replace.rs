//! Troca um arquivo temporário pelo destino.
//!
//! No Unix, `rename` substitui o destino no mesmo volume. No Windows, `std::fs::rename`
//! falha se o destino já existe; `MoveFileExW` com `MOVEFILE_REPLACE_EXISTING` faz a troca
//! (os dois caminhos ficam no mesmo diretório, então é o mesmo volume).

use std::path::Path;

pub fn replace_file(tmp: &Path, dest: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        windows_replace(tmp, dest)
    }
    #[cfg(not(windows))]
    {
        std::fs::rename(tmp, dest)
    }
}

#[cfg(windows)]
fn windows_replace(tmp: &Path, dest: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    const MOVEFILE_REPLACE_EXISTING: u32 = 1;
    const MOVEFILE_WRITE_THROUGH: u32 = 8;

    #[link(name = "kernel32")]
    extern "system" {
        fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
    }

    fn wide(p: &Path) -> Vec<u16> {
        p.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
    }

    let from = wide(tmp);
    let to = wide(dest);
    let ok = unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH) };
    if ok == 0 { Err(std::io::Error::last_os_error()) } else { Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitui_arquivo_que_ja_existe() {
        let dir = std::env::temp_dir().join(format!("mine-rs-replace-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("conta.json");
        let primeiro = dir.join("a.tmp");
        std::fs::write(&primeiro, b"velho").unwrap();
        replace_file(&primeiro, &dest).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"velho");
        let segundo = dir.join("b.tmp");
        std::fs::write(&segundo, b"novo").unwrap();
        replace_file(&segundo, &dest).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"novo");
        assert!(!segundo.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
