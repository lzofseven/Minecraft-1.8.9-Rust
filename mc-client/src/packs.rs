//! Pacotes de recursos escolhidos: do `options.txt` para a pilha (`ResourcePackRepository`).

use std::path::PathBuf;

use mc_render::resources::{self, Entry};

use crate::settings::Settings;

/// Pacotes ativos, do menos para o mais prioritário. Como o construtor do `ResourcePackRepository`:
/// um pacote achado mas incompatível (e não aceito) sai da lista do `options.txt`; nomes que não
/// existem mais ficam lá.
pub fn selected(settings: &mut Settings) -> Vec<Entry> {
    let all = resources::scan(&resources::resourcepacks_dir(), None);
    let chosen = resources::resolve_selected(&all, &settings.resource_packs, &settings.incompatible_resource_packs);
    settings.resource_packs.retain(|n| !all.iter().any(|e| &e.name == n) || chosen.iter().any(|e| &e.name == n));
    chosen
}

/// Caminhos da pilha acima do padrão: pacotes do jogador e, no topo, o do servidor.
pub fn stack_paths(chosen: &[Entry], server: Option<&PathBuf>) -> Vec<PathBuf> {
    chosen.iter().map(|e| e.path.clone()).chain(server.cloned()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_pack_goes_on_top() {
        let e = |n: &str| Entry { name: n.into(), path: PathBuf::from(n), is_dir: false, modified: 0, meta: None, icon: None };
        let server = PathBuf::from("/srv/abc");
        let p = stack_paths(&[e("a"), e("b")], Some(&server));
        assert_eq!(p, vec![PathBuf::from("a"), PathBuf::from("b"), server]);
        assert_eq!(stack_paths(&[], None), Vec::<PathBuf>::new());
    }
}
