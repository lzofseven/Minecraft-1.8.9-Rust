//! Estandartes: dados do `TileEntityBanner`, textura em camadas (`LayeredColorMaskTexture`)
//! e o `ModelBanner` do `TileEntityBannerRenderer`. Padrões e cores vêm de `gen_banner.rs`
//! (gerado do Java por `scripts/gen_banner.py`).

use std::collections::HashMap;
use std::f32::consts::PI;

use glam::{Mat4, Vec3};
use image::{Rgba, RgbaImage};
use mc_protocol::nbt::Nbt;
use mc_render::{Jar, Renderer};

use crate::gen_banner::{DYE_COLORS, PATTERNS};
use crate::models::{Part, Sink};

/// Base e camadas (`Patterns`: id curto do padrão + cor em dano de tinta).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BannerData {
    /// Só desenha depois de receber o NBT (0x35, tipo 6): antes o jogo não tem textura.
    pub loaded: bool,
    pub base: i32,
    pub layers: Vec<(&'static str, i32)>,
}

impl BannerData {
    /// `TileEntityBanner.readFromNBT` + `initializeBannerData` (padrões desconhecidos são ignorados).
    pub fn from_nbt(n: &Nbt) -> Self {
        let int = |t: Option<&Nbt>| match t {
            Some(Nbt::Int(v)) => *v,
            Some(Nbt::Byte(v)) => *v as i32,
            Some(Nbt::Short(v)) => *v as i32,
            _ => 0,
        };
        let mut layers = Vec::new();
        if let Some(Nbt::List(list)) = n.get("Patterns") {
            for e in list {
                let id = e.get("Pattern").and_then(Nbt::as_str).unwrap_or_default();
                if let Some((name, _)) = PATTERNS.iter().find(|(_, short)| *short == id) {
                    layers.push((*name, int(e.get("Color"))));
                }
            }
        }
        Self { loaded: true, base: int(n.get("Base")), layers }
    }

    /// `getPatternResourceLocation`: "b" + base + (id + cor)...; vazio se nunca recebeu dados.
    pub fn key(&self) -> String {
        if !self.loaded {
            return String::new();
        }
        let mut s = format!("b{}", self.base);
        for (name, color) in &self.layers {
            let short = PATTERNS.iter().find(|(n, _)| n == name).map_or("", |p| p.1);
            s.push_str(&format!("{short}{color}"));
        }
        s
    }
}

/// `EnumDyeColor.byDyeDamage(i).getMapColor().colorValue` (índices fora de 0..15 usam `& 15`).
pub fn dye_rgb(damage: i32) -> u32 {
    DYE_COLORS[(damage & 15) as usize]
}

/// `MathHelper.func_180188_d`: multiplica canal a canal.
fn multiply(pixel: [u8; 3], color: u32) -> [u8; 3] {
    let c = [(color >> 16) as u8, (color >> 8) as u8, color as u8];
    std::array::from_fn(|i| (pixel[i] as f32 * c[i] as f32 / 255.0) as u8)
}

/// `drawImage` SrcOver (cores não pré-multiplicadas) de um pixel de camada sobre o acumulado.
fn over(dst: Rgba<u8>, src: Rgba<u8>) -> Rgba<u8> {
    let (sa, da) = (src.0[3] as f32 / 255.0, dst.0[3] as f32 / 255.0);
    let ra = sa + da * (1.0 - sa);
    if ra <= 0.0 {
        return Rgba([0; 4]);
    }
    let ch = |i: usize| ((src.0[i] as f32 * sa + dst.0[i] as f32 * da * (1.0 - sa)) / ra + 0.5).min(255.0) as u8;
    Rgba([ch(0), ch(1), ch(2), (ra * 255.0 + 0.5) as u8])
}

/// `LayeredColorMaskTexture.loadTexture`: a textura base e, por camada, a máscara
/// (alfa = canal vermelho) pintada com a cor da camada sobre os pixels da base.
pub fn compose(base: &RgbaImage, masks: &[(RgbaImage, i32)]) -> RgbaImage {
    let mut out = base.clone();
    for (mask, color) in masks.iter().take(17) {
        if mask.dimensions() != base.dimensions() {
            continue;
        }
        let rgb = dye_rgb(*color);
        for (x, y, m) in mask.enumerate_pixels() {
            if m.0[3] == 0 {
                continue;
            }
            let b = base.get_pixel(x, y).0;
            let [r, g, bl] = multiply([b[0], b[1], b[2]], rgb);
            let layer = Rgba([r, g, bl, m.0[0]]);
            let cur = *out.get_pixel(x, y);
            out.put_pixel(x, y, over(cur, layer));
        }
    }
    out
}

/// Imagem do desenho: `entity/banner_base` com a base e cada camada (`LayeredColorMaskTexture`).
pub fn compose_banner(jar: &mut Jar, data: &BannerData) -> Option<RgbaImage> {
    let base = jar.texture("entity/banner_base")?;
    let mut masks = Vec::new();
    // A primeira camada é sempre o padrão "base" com a cor de base.
    let layers = std::iter::once(("base", data.base)).chain(data.layers.iter().copied());
    for (name, color) in layers {
        if let Some(img) = jar.texture(&format!("entity/banner/{name}")) {
            masks.push((img, color));
        }
    }
    Some(compose(&base, &masks))
}

/// Cache de texturas por desenho (`DESIGNS`, no máximo 256).
#[derive(Default)]
pub struct BannerTextures {
    designs: HashMap<String, u32>,
}

impl BannerTextures {
    /// Ids das texturas na GPU (para soltar numa recarga de recursos).
    pub fn ids(&self) -> Vec<u32> {
        self.designs.values().copied().collect()
    }

    pub fn texture(&mut self, r: &mut Renderer, jar: &mut Jar, data: &BannerData) -> Option<u32> {
        let key = data.key();
        if key.is_empty() {
            return None;
        }
        if let Some(t) = self.designs.get(&key) {
            return Some(*t);
        }
        if self.designs.len() >= 256 {
            return None;
        }
        let img = compose_banner(jar, data)?;
        let id = r.create_texture(img.width(), img.height(), img.as_raw());
        self.designs.insert(key, id);
        Some(id)
    }
}

/// `ModelBanner` (textura 64x64): placa, poste e travessa.
pub struct BannerModel {
    pub slate: Part,
    pub stand: Part,
    pub top: Part,
}

impl BannerModel {
    pub fn new() -> Self {
        Self {
            // `renderBanner` põe `bannerSlate.rotationPointY = -32`.
            slate: Part::new([0.0, -32.0, 0.0]).cube((0, 0), [-10.0, 0.0, -2.0], [20, 40, 1], 0.0, false),
            stand: Part::new([0.0; 3]).cube((44, 0), [-1.0, -30.0, -1.0], [2, 42, 2], 0.0, false),
            top: Part::new([0.0; 3]).cube((0, 42), [-10.0, -32.0, -1.0], [20, 2, 2], 0.0, false),
        }
    }

    pub fn emit(&mut self, sink: &mut Sink, m: &Mat4, sway: f32) {
        self.slate.rot.x = sway;
        sink.tex = (64.0, 64.0);
        sink.part(&self.slate, m);
        sink.part(&self.stand, m);
        sink.part(&self.top, m);
    }
}

impl Default for BannerModel {
    fn default() -> Self {
        Self::new()
    }
}

const SCALE: f32 = 0.6666667;

/// Matriz do `TileEntityBannerRenderer` com a escala do modelo: em pé gira `-meta * 22,5°`;
/// na parede fica um quarto de bloco abaixo e recuada.
pub fn banner_matrix(pos: (i32, i32, i32), wall: bool, meta: u16) -> Mat4 {
    let (x, y, z) = (pos.0 as f32, pos.1 as f32, pos.2 as f32);
    let base = if !wall {
        Mat4::from_translation(Vec3::new(x + 0.5, y + 0.75 * SCALE, z + 0.5)) * Mat4::from_rotation_y((-(meta as f32 * 360.0) / 16.0).to_radians())
    } else {
        let rot = crate::tile_entities::wall_rotation(meta);
        Mat4::from_translation(Vec3::new(x + 0.5, y - 0.25 * SCALE, z + 0.5))
            * Mat4::from_rotation_y((-rot).to_radians())
            * Mat4::from_translation(Vec3::new(0.0, -0.3125, -0.4375))
    };
    base * Mat4::from_scale(Vec3::new(SCALE, -SCALE, -SCALE))
}

/// Balanço da placa: `(-0.0125 + 0.01 cos(t π 0.02)) π`, com `t = 7x + 9y + 13z + tempo`.
pub fn sway(pos: (i32, i32, i32), time: f32) -> f32 {
    let t = (pos.0.wrapping_mul(7).wrapping_add(pos.1.wrapping_mul(9)).wrapping_add(pos.2.wrapping_mul(13))) as f32 + time;
    (-0.0125 + 0.01 * (t * PI * 0.02).cos()) * PI
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn pattern(id: &str, color: i32) -> Nbt {
        Nbt::Compound(HashMap::from([("Pattern".to_string(), Nbt::String(id.into())), ("Color".to_string(), Nbt::Int(color))]))
    }

    #[test]
    fn banner_nbt_and_key() {
        let nbt = Nbt::Compound(HashMap::from([
            ("Base".to_string(), Nbt::Int(1)),
            ("Patterns".to_string(), Nbt::List(vec![pattern("cr", 15), pattern("xx", 3), pattern("bo", 0)])),
        ]));
        let b = BannerData::from_nbt(&nbt);
        // Padrão desconhecido é ignorado (`getPatternByID` devolve null).
        assert_eq!(b.layers, vec![("cross", 15), ("border", 0)]);
        assert_eq!(b.key(), "b1cr15bo0");
        assert_eq!(BannerData::default().key(), "");
        assert_eq!(PATTERNS.len(), 39);
        assert_eq!(PATTERNS[0], ("base", "b"));
        // Dano de tinta 15 = branco (neve), 0 = preto.
        assert_eq!((dye_rgb(15), dye_rgb(0)), (0xFFFFFF, 0x191919));
    }

    #[test]
    fn layers_are_masked_and_tinted() {
        let base = RgbaImage::from_pixel(2, 1, Rgba([200, 100, 50, 255]));
        // Máscara: pixel 0 com alfa na vermelha 255, pixel 1 sem cobertura.
        let mut mask = RgbaImage::new(2, 1);
        mask.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        let out = compose(&base, &[(mask.clone(), 14)]);
        // Vermelho (0x992D33... = 10040115): canais da base multiplicados.
        let rgb = dye_rgb(14);
        let want = [(200.0 * ((rgb >> 16) & 255) as f32 / 255.0) as u8, (100.0 * ((rgb >> 8) & 255) as f32 / 255.0) as u8, (50.0 * (rgb & 255) as f32 / 255.0) as u8];
        assert_eq!(out.get_pixel(0, 0).0, [want[0], want[1], want[2], 255]);
        assert_eq!(out.get_pixel(1, 0).0, [200, 100, 50, 255]);
        // Alfa da camada vem do canal vermelho da máscara: 0 não muda nada, metade mistura.
        let mut half = mask;
        half.put_pixel(0, 0, Rgba([128, 0, 0, 255]));
        let out = compose(&base, &[(half, 14)]);
        let px = out.get_pixel(0, 0).0;
        assert_eq!(px[3], 255);
        assert!(px[0] < 200 && px[0] > want[0] && px[1] < 100, "{px:?}");
        // Máscara de outro tamanho é ignorada.
        assert_eq!(compose(&base, &[(RgbaImage::new(3, 3), 1)]), base);
    }

    #[test]
    fn banner_placement_and_sway() {
        let near = |a: Vec3, b: Vec3| (a - b).length() < 1e-5;
        // Em pé: 0,75 * 2/3 acima do chão, centrado; gira -meta * 22,5°.
        let m = banner_matrix((2, 10, 3), false, 0);
        assert!(near(m.transform_point3(Vec3::ZERO), Vec3::new(2.5, 10.5, 3.5)));
        let m = banner_matrix((0, 0, 0), false, 4);
        assert!(near(m.transform_vector3(-Vec3::Z * 3.0).normalize(), -Vec3::X));
        // Na parede: desce 0,25 * 2/3 + 0,3125 e recua 0,4375 do centro.
        let m = banner_matrix((0, 0, 0), true, 3);
        assert!(near(m.transform_point3(Vec3::ZERO), Vec3::new(0.5, -0.25 * SCALE - 0.3125, 0.5 - 0.4375)));
        // Balanço: com t = 0 vale (-0,0125 + 0,01) * π; meio ciclo depois inverte o cosseno.
        assert!((sway((0, 0, 0), 0.0) - (-0.0025 * PI)).abs() < 1e-6);
        assert!((sway((0, 0, 0), 50.0) - (-0.0225 * PI)).abs() < 1e-5);
        assert!((sway((1, 1, 1), 0.0) - sway((0, 0, 0), 29.0)).abs() < 1e-6);
        // Modelo: placa 20x40x1 presa em y = -32.
        let mut model = BannerModel::new();
        assert_eq!(model.slate.pivot.y, -32.0);
        model.stand.show = false;
        assert_eq!(model.stand.quad_count(), 0);
        assert_eq!(model.slate.cubes[0].size, [20, 40, 1]);
    }
}
