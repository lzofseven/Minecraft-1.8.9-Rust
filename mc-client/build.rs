//! No Windows, embute `windows/icone.ico` no executável (ícone no Explorer e na barra de tarefas) com o
//! `windres` do mingw-w64. Nos outros sistemas não faz nada.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let ico = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../windows/icone.ico");
    println!("cargo:rerun-if-changed={}", ico.display());
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let rc = out.join("icone.rc");
    std::fs::write(&rc, format!("1 ICON \"{}\"\n", ico.display().to_string().replace('\\', "/"))).unwrap();
    let obj = out.join("icone.o");
    let windres = std::env::var("WINDRES").unwrap_or_else(|_| "x86_64-w64-mingw32-windres".into());
    match Command::new(&windres).arg(&rc).arg("-O").arg("coff").arg("-o").arg(&obj).status() {
        Ok(s) if s.success() => println!("cargo:rustc-link-arg-bins={}", obj.display()),
        _ => println!("cargo:warning=sem {windres}: o .exe sai sem ícone"),
    }
}
