//! Mapas (`MapData` do cliente pelo 0x34 `S34PacketMaps` e o `MapItemRenderer`).

use std::collections::HashMap;

use crate::gen_mapcolors::MAP_COLORS;

/// Um 0x34: id, escala, ícones (tipo, x, z, giro) e o retângulo de cores mudado (colunas, linhas, x, z, bytes).
#[derive(Clone, Debug, PartialEq)]
pub struct MapPacket {
    pub id: i32,
    pub scale: i8,
    pub icons: Vec<[i8; 4]>,
    pub patch: Option<(u8, u8, u8, u8, Vec<u8>)>,
}

/// `MapData` com a textura dele (`MapItemRenderer.Instance`).
pub struct MapData {
    pub colors: Vec<u8>,
    pub icons: Vec<[i8; 4]>,
    pub scale: i8,
    pub tex: Option<u32>,
    pub dirty: bool,
}

impl Default for MapData {
    fn default() -> Self {
        Self { colors: vec![0; 16384], icons: Vec::new(), scale: 0, tex: None, dirty: true }
    }
}

impl MapData {
    /// `S34PacketMaps.setMapdataTo`.
    pub fn apply(&mut self, p: &MapPacket) {
        self.scale = p.scale;
        self.icons = p.icons.clone();
        if let Some((cols, rows, x, z, data)) = &p.patch {
            for j in 0..*cols as usize {
                for k in 0..*rows as usize {
                    let (dst, src) = (*x as usize + j + (*z as usize + k) * 128, j + k * *cols as usize);
                    if let (Some(d), Some(&s)) = (self.colors.get_mut(dst), data.get(src)) {
                        *d = s;
                    }
                }
            }
        }
        self.dirty = true;
    }

    /// `Instance.updateMapTexture`: sem cor fica o xadrez de alfa 16/24; o resto é o `getMapColor`.
    pub fn rgba(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16384 * 4);
        for (i, &c) in self.colors.iter().enumerate() {
            let argb = if c as usize / 4 == 0 { ((((i + i / 128) & 1) * 8 + 16) as u32) << 24 } else { MAP_COLORS[c as usize] };
            out.extend_from_slice(&[(argb >> 16) as u8, (argb >> 8) as u8, argb as u8, (argb >> 24) as u8]);
        }
        out
    }
}

pub type Maps = HashMap<i32, MapData>;

/// `MapItemRenderer.Instance.render`: o mapa de 128x128 em z = -0,01 e os ícones (com `no_overlay`, como na
/// moldura, só os de tipo 1). `shade` é a luz das faces (1 sem `GL_LIGHTING`); a textura sai do `MapData`.
#[allow(clippy::too_many_arguments)]
pub fn emit(
    maps: &mut Maps,
    textures: &mut crate::skins::Textures,
    r: &mut mc_render::Renderer,
    jar: &mut mc_render::Jar,
    id: i32,
    m: &glam::Mat4,
    no_overlay: bool,
    light: [u8; 4],
    shade: f32,
    out: &mut HashMap<mc_render::Tex, mc_world::mesh::Geometry>,
) {
    use glam::{Mat4, Vec3};
    use mc_render::Tex;
    use mc_world::mesh::Vertex;
    use crate::camera::{rot, tr};
    use crate::mobs::TRANSLUCENT;
    let Some(map) = maps.get_mut(&id) else { return };
    let tex = match map.tex {
        Some(t) if !map.dirty => t,
        Some(t) => {
            r.update_texture(t, &map.rgba());
            map.dirty = false;
            t
        }
        None => {
            let t = r.create_texture(128, 128, &map.rgba());
            map.tex = Some(t);
            map.dirty = false;
            t
        }
    };
    let icons = map.icons.clone();
    let c = (shade * 255.0) as u8;
    let v = |m: &Mat4, x: f32, y: f32, z: f32, u: f32, w: f32| Vertex { pos: m.transform_point3(Vec3::new(x, y, z)).to_array(), uv: [u, w], color: [c, c, c, 255], light };
    let geo = out.entry(Tex::Custom(tex | TRANSLUCENT)).or_default();
    geo.quad([v(m, 0.0, 128.0, -0.01, 0.0, 1.0), v(m, 128.0, 128.0, -0.01, 1.0, 1.0), v(m, 128.0, 0.0, -0.01, 1.0, 0.0), v(m, 0.0, 0.0, -0.01, 0.0, 0.0)]);
    let Some(icon_tex) = textures.jar(r, jar, "map/map_icons") else { return };
    let geo = out.entry(Tex::Custom(icon_tex | TRANSLUCENT)).or_default();
    let mut k = 0;
    for [kind, x, z, dir] in icons {
        if no_overlay && kind != 1 {
            continue;
        }
        let im = *m * tr(x as f32 / 2.0 + 64.0, z as f32 / 2.0 + 64.0, -0.02) * rot((dir as i32 * 360) as f32 / 16.0, 0.0, 0.0, 1.0) * Mat4::from_scale(Vec3::new(4.0, 4.0, 3.0)) * tr(-0.125, 0.125, 0.0);
        let (u0, v0, u1, v1) = ((kind % 4) as f32 / 4.0, (kind / 4) as f32 / 4.0, (kind % 4 + 1) as f32 / 4.0, (kind / 4 + 1) as f32 / 4.0);
        let z = k as f32 * -0.001;
        geo.quad([v(&im, -1.0, 1.0, z, u0, v0), v(&im, 1.0, 1.0, z, u1, v0), v(&im, 1.0, -1.0, z, u1, v1), v(&im, -1.0, -1.0, z, u0, v1)]);
        k += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remendo_e_cores() {
        let mut m = MapData::default();
        m.apply(&MapPacket { id: 0, scale: 0, icons: vec![[1, 0, 0, 8]], patch: Some((2, 1, 10, 3, vec![4 * 1 + 2, 0])) });
        assert_eq!(m.colors[10 + 3 * 128], 6);
        let px = m.rgba();
        // Grama (cor 1) no tom 2 (`getMapColor(2)` = cor cheia); sem cor: alfa 16 ou 24, preto.
        let o = (10 + 3 * 128) * 4;
        assert_eq!(&px[o..o + 4], &[0x7F, 0xB2, 0x38, 0xFF]);
        assert_eq!(&px[0..4], &[0, 0, 0, 16]);
        assert_eq!(&px[4..8], &[0, 0, 0, 24]);
    }
}
