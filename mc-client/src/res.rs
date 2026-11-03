//! Acesso global à pilha de recursos para quem não recebe o `Jar` por parâmetro (idiomas, telas).

use std::sync::Mutex;

use mc_render::Jar;

static JAR: Mutex<Option<Jar>> = Mutex::new(None);

/// Guarda um punho da pilha (feito uma vez, na abertura do jogo).
pub fn install(jar: Jar) {
    if let Ok(mut g) = JAR.lock() {
        *g = Some(jar);
    }
}

/// O punho global. Sem `install` (testes, ferramentas), abre uma vez o jar que o `setup` acharia.
pub fn jar() -> Option<Jar> {
    let mut g = JAR.lock().ok()?;
    if g.is_none() {
        let home = std::path::PathBuf::from(std::env::var_os("HOME")?);
        let paths = [
            home.join(".minecraft/versions/1.8.9/1.8.9.jar"),
            home.join(".local/share/PrismLauncher/libraries/com/mojang/minecraft/1.8.9/minecraft-1.8.9-client.jar"),
            crate::setup::data_dir().join("minecraft-1.8.9.jar"),
        ];
        *g = paths.iter().find_map(|p| Jar::open(p).ok());
    }
    g.clone()
}

/// Todos os arquivos `path` da pilha, do pacote padrão ao topo, como texto UTF-8.
pub fn all_text(path: &str) -> Vec<String> {
    jar().map(|mut j| j.all(path).iter().map(|b| String::from_utf8_lossy(b).into_owned()).collect()).unwrap_or_default()
}
