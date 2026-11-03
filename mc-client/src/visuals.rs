//! Opções gráficas do OptiFine (`optionsof.txt` na pasta de configuração do mine-rs): o que o jogador desliga
//! para ganhar FPS ou limpar a tela. Sem o arquivo, vale o jogo vanilla.

use std::collections::HashMap;

/// `Default` = o jogo vanilla, tudo ligado.
#[derive(Debug, Clone, Copy)]
pub struct Visuals {
    /// Névoa de distância (`ofFogType:3` desliga).
    pub fog: bool,
    /// Domo do céu, estrelas, sol e lua (`ofSky`, `ofStars`, `ofSunMoon`).
    pub sky: bool,
    pub stars: bool,
    pub sun_moon: bool,
    /// `ofClouds:3` desliga as nuvens.
    pub clouds: bool,
    /// `ofRain:3` não desenha a chuva (nem as gotas e o som).
    pub rain: bool,
    /// Chuva e trovão no mundo (`ofWeather:false` deixa em 0: o céu não escurece).
    pub weather: bool,
    /// `ofDynamicFov:false`: o FOV não muda com corrida, velocidade e arco.
    pub dynamic_fov: bool,
    /// Animação das texturas de água, lava e fogo (`ofAnimatedWater/Lava:2`, `ofAnimatedFire:false`).
    pub anim_water: bool,
    pub anim_lava: bool,
    pub anim_fire: bool,
    /// `ofVignette` (Config.isVignetteEnabled): `None` segue os gráficos detalhados, 1 desliga, 2 liga.
    pub vignette: Option<bool>,
}

impl Default for Visuals {
    fn default() -> Self {
        Self {
            fog: true,
            sky: true,
            stars: true,
            sun_moon: true,
            clouds: true,
            rain: true,
            weather: true,
            dynamic_fov: true,
            anim_water: true,
            anim_lava: true,
            anim_fire: true,
            vignette: None,
        }
    }
}

impl Visuals {
    /// Do optionsof.txt da pasta de configuração.
    pub fn load() -> Self {
        let of = std::fs::read_to_string(crate::settings::config_dir().join("optionsof.txt")).unwrap_or_default();
        Self::from(&of)
    }

    pub fn from(optionsof: &str) -> Self {
        let of: HashMap<&str, &str> = optionsof.lines().filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim(), v.trim())).collect();
        let is = |k: &str, v: &str| of.get(k) == Some(&v);
        Self {
            fog: !is("ofFogType", "3"),
            sky: !is("ofSky", "false"),
            stars: !is("ofStars", "false"),
            sun_moon: !is("ofSunMoon", "false"),
            clouds: !is("ofClouds", "3"),
            rain: !is("ofRain", "3"),
            weather: !is("ofWeather", "false"),
            dynamic_fov: !is("ofDynamicFov", "false"),
            anim_water: !is("ofAnimatedWater", "2"),
            anim_lava: !is("ofAnimatedLava", "2"),
            anim_fire: !is("ofAnimatedFire", "false"),
            vignette: match of.get("ofVignette").copied() {
                Some("1") => Some(false),
                Some("2") => Some(true),
                _ => None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn opcoes_do_optifine() {
        let v = super::Visuals::from("ofFogType:3\nofClouds:3\nofVignette:1\nofAnimatedWater:2\n");
        assert!(!v.fog && !v.clouds && v.sky && !v.anim_water && v.anim_lava);
        assert_eq!(v.vignette, Some(false));
        assert!(super::Visuals::from("").fog);
    }
}
