//! Ícones de item 16x16 gerados sob demanda e gravados no atlas da GUI.

use std::collections::HashMap;

use image::RgbaImage;
use mc_world::items::{layer_color, model_meta, model_name};
use mc_world::model::{bake, ModelLib, Rot};

use crate::assets::Jar;
use crate::ui::{item_origin, ITEM_PX, ITEM_SLOTS};

/// Primeiro quadro, ampliado sem suavizar para ITEM_PX.
fn first_frame(img: RgbaImage) -> RgbaImage {
    let w = img.width().max(1);
    RgbaImage::from_fn(ITEM_PX, ITEM_PX, |x, y| *img.get_pixel(x * w / ITEM_PX, (y * w / ITEM_PX).min(img.height() - 1)))
}

/// Variação do ícone além de id/dano: cor do couro (0xRRGGBB) e bit 1 << 24 = encantado.
pub const ENCHANTED: u32 = 1 << 24;

fn tinted(mut img: RgbaImage, rgb: u32) -> RgbaImage {
    for p in img.pixels_mut() {
        p.0[0] = (p.0[0] as u32 * ((rgb >> 16) & 0xFF) / 255) as u8;
        p.0[1] = (p.0[1] as u32 * ((rgb >> 8) & 0xFF) / 255) as u8;
        p.0[2] = (p.0[2] as u32 * (rgb & 0xFF) / 255) as u8;
    }
    img
}

/// `top` por cima de `base` onde for opaco.
fn layered(mut base: RgbaImage, top: &RgbaImage) -> RgbaImage {
    for (b, t) in base.pixels_mut().zip(top.pixels()) {
        if t.0[3] > 0 {
            *b = *t;
        }
    }
    base
}

/// Primeiro quadro quadrado (as texturas animadas são tiras verticais).
fn frame0(img: &RgbaImage) -> RgbaImage {
    let w = img.width().max(1);
    image::imageops::crop_imm(img, 0, 0, w, w.min(img.height())).to_image()
}

const WHITE: u32 = 0xFFFFFF;

/// Normais de EnumFacing na ordem down, up, north, south, west, east.
const NORMALS: [[f32; 3]; 6] = [[0., -1., 0.], [0., 1., 0.], [0., 0., -1.], [0., 0., 1.], [-1., 0., 0.], [1., 0., 0.]];

type M3 = [[f32; 3]; 3];

fn mul(a: &M3, b: &M3) -> M3 {
    let mut r = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            r[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    r
}

/// `GlStateManager.rotate(graus, eixo)` (matriz de rotação do OpenGL).
fn rot_axis(deg: f32, axis: usize) -> M3 {
    let (s, c) = deg.to_radians().sin_cos();
    match axis {
        0 => [[1., 0., 0.], [0., c, -s], [0., s, c]],
        1 => [[c, 0., s], [0., 1., 0.], [-s, 0., c]],
        _ => [[c, -s, 0.], [s, c, 0.], [0., 0., 1.]],
    }
}

fn apply(m: &M3, v: [f32; 3]) -> [f32; 3] {
    [0, 1, 2].map(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
}

/// Transformação da GUI de um item 3D: `setupGuiTransform` (escala 40 * 0.5, Rx(210), Ry(-135)),
/// `display.gui` do modelo, escala 0.5 de `renderItem` e centralização -0.5.
struct GuiXform {
    local: M3,
    base: M3,
    sc: [f32; 3],
    tr: [f32; 3],
}

impl GuiXform {
    fn new(gui: Option<[[f32; 3]; 3]>) -> Self {
        // Parte linear (rotação e escala) e deslocamento, aplicados da direita para a esquerda.
        let (rot, tr, sc) = match gui {
            Some([r, t, s]) => (r, t, s),
            None => ([0.0; 3], [0.0; 3], [1.0; 3]),
        };
        let local = mul(&mul(&rot_axis(rot[1], 1), &rot_axis(rot[0], 0)), &rot_axis(rot[2], 2)); // Ry·Rx·Rz
        let base = mul(&rot_axis(210.0, 0), &rot_axis(-135.0, 1)); // Rx(210)·Ry(-135)
        Self { local, base, sc, tr }
    }

    /// Ponto do modelo (unidades de bloco) -> pixels centrados na tela e profundidade.
    fn to_eye(&self, p: [f32; 3]) -> [f32; 3] {
        let mut v = [p[0] - 0.5, p[1] - 0.5, p[2] - 0.5];
        v = v.map(|c| c * 0.5);
        v = [v[0] * self.sc[0], v[1] * self.sc[1], v[2] * self.sc[2]];
        v = apply(&self.local, v);
        v = [v[0] + self.tr[0], v[1] + self.tr[1], v[2] + self.tr[2]];
        v = apply(&self.base, v);
        let k = 40.0 * 0.5;
        [v[0] * k, v[1] * k, -v[2] * k] // scale(1, 1, -1)
    }

    fn normal_m(&self) -> M3 {
        mul(&self.base, &self.local)
    }

    /// Luzes: direções de RenderHelper (LIGHT0/1_POS) giradas por Ry(-30)·Rx(165) e normalizadas (espaço do olho).
    fn eye_lights() -> Vec<[f32; 3]> {
        [[0.2f32, 1.0, -0.7], [-0.2, 1.0, 0.7]]
            .iter()
            .map(|l| {
                let n = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
                apply(&mul(&rot_axis(-30.0, 1), &rot_axis(165.0, 0)), [l[0] / n, l[1] / n, l[2] / n])
            })
            .collect()
    }
}

/// Direções das duas luzes da GUI vistas do espaço do modelo (inverso da rotação das normais e do
/// espelho em z), para quem calcula o sombreado por face antes de rasterizar (tile entities).
pub fn gui_model_lights(gui: Option<[[f32; 3]; 3]>) -> [[f32; 3]; 2] {
    let n = GuiXform::new(gui).normal_m();
    let mut t = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            t[i][j] = n[j][i];
        }
    }
    let lights = GuiXform::eye_lights();
    [0, 1].map(|i| apply(&t, [lights[i][0], lights[i][1], -lights[i][2]]))
}

/// Pixel a pixel como `GL_QUADS` com teste de profundidade: acumula cor e cobertura.
struct Canvas {
    color: Vec<[f32; 4]>,
    depth: Vec<f32>,
}

impl Canvas {
    fn new() -> Self {
        Self { color: vec![[0.0f32; 4]; (ITEM_PX * ITEM_PX) as usize], depth: vec![f32::NEG_INFINITY; (ITEM_PX * ITEM_PX) as usize] }
    }

    /// Triângulo em pixels (x, y, profundidade) com UV normalizado (0..1) e cor multiplicada.
    fn tri(&mut self, p: [[f32; 3]; 3], uv: [[f32; 2]; 3], tex: &RgbaImage, rgb: [f32; 3]) {
        let area = (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1]);
        if area.abs() < 1e-6 {
            return;
        }
        let (minx, maxx) = (p.iter().map(|v| v[0]).fold(f32::MAX, f32::min), p.iter().map(|v| v[0]).fold(f32::MIN, f32::max));
        let (miny, maxy) = (p.iter().map(|v| v[1]).fold(f32::MAX, f32::min), p.iter().map(|v| v[1]).fold(f32::MIN, f32::max));
        for py in (miny.floor().max(0.0) as u32)..=(maxy.ceil().min(ITEM_PX as f32 - 1.0) as u32) {
            for px in (minx.floor().max(0.0) as u32)..=(maxx.ceil().min(ITEM_PX as f32 - 1.0) as u32) {
                let (x, y) = (px as f32 + 0.5, py as f32 + 0.5);
                let w1 = ((x - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (y - p[0][1])) / area;
                let w2 = ((p[1][0] - p[0][0]) * (y - p[0][1]) - (x - p[0][0]) * (p[1][1] - p[0][1])) / area;
                let w0 = 1.0 - w1 - w2;
                if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                    continue;
                }
                let z = w0 * p[0][2] + w1 * p[1][2] + w2 * p[2][2];
                let idx = (py * ITEM_PX + px) as usize;
                if z < self.depth[idx] {
                    continue;
                }
                let u = |k: usize| w0 * uv[0][k] + w1 * uv[1][k] + w2 * uv[2][k];
                let tx = ((u(0) * tex.width() as f32) as i32).clamp(0, tex.width() as i32 - 1) as u32;
                let ty = ((u(1) * tex.height() as f32) as i32).clamp(0, tex.height() as i32 - 1) as u32;
                let s = tex.get_pixel(tx, ty).0;
                let a = s[3] as f32 / 255.0;
                if a < 0.1 {
                    continue; // alphaFunc(GL_GREATER, 0.1)
                }
                let src = [s[0] as f32 / 255.0 * rgb[0], s[1] as f32 / 255.0 * rgb[1], s[2] as f32 / 255.0 * rgb[2]];
                let dst = &mut self.color[idx];
                let da = dst[3];
                for k in 0..3 {
                    dst[k] = src[k] * a + dst[k] * (1.0 - a);
                }
                dst[3] = a + da * (1.0 - a);
                self.depth[idx] = z;
            }
        }
    }

    fn finish(self) -> RgbaImage {
        let color = self.color;
        RgbaImage::from_fn(ITEM_PX, ITEM_PX, |x, y| {
            let c = color[(y * ITEM_PX + x) as usize];
            // Cobertura acumulada: cor já está misturada com o fundo transparente; reverte a pré-multiplicação.
            let a = c[3];
            if a <= 0.0 {
                return image::Rgba([0, 0, 0, 0]);
            }
            let f = |v: f32| ((v / a).clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            image::Rgba([f(c[0]), f(c[1]), f(c[2]), (a.clamp(0.0, 1.0) * 255.0 + 0.5) as u8])
        })
    }
}

/// Ícone 3D de um modelo de bloco, como `RenderItem.renderItemIntoGUI` (ver `GuiXform`).
fn render_model3d(
    jar: &mut Jar,
    quads: &[mc_world::model::BakedQuad],
    names: &[String],
    gui: Option<[[f32; 3]; 3]>,
    tint: &dyn Fn(i32) -> u32,
) -> RgbaImage {
    let texs: Vec<Option<RgbaImage>> = names.iter().map(|n| jar.texture(n).map(|i| frame0(&i))).collect();
    let xf = GuiXform::new(gui);
    let lights = GuiXform::eye_lights();
    let normal_m = xf.normal_m();
    let px_per_unit = ITEM_PX as f32 / 16.0;
    let center = 8.0;
    let mut canvas = Canvas::new();
    for q in quads {
        let eye = q.pos.map(|p| xf.to_eye(p));
        let n = apply(&normal_m, NORMALS[q.face as usize % 6]);
        let n = [n[0], n[1], -n[2]];
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-6);
        let n = n.map(|c| c / len);
        let mut lum = 0.4;
        for l in &lights {
            lum += (n[0] * l[0] + n[1] * l[1] + n[2] * l[2]).max(0.0) * 0.6;
        }
        let lum = lum.min(1.0);
        let t = if q.tint >= 0 { tint(q.tint) } else { WHITE };
        let rgb = [(t >> 16 & 0xFF) as f32 / 255.0, (t >> 8 & 0xFF) as f32 / 255.0, (t & 0xFF) as f32 / 255.0].map(|c| c * lum);
        let Some(Some(tex)) = texs.get(q.tex as usize) else { continue };
        for tri in [[0usize, 1, 2], [0, 2, 3]] {
            let p = tri.map(|i| [(eye[i][0] + center) * px_per_unit, (eye[i][1] + center) * px_per_unit, eye[i][2]]);
            canvas.tri(p, tri.map(|i| [q.uv[i][0] / 16.0, q.uv[i][1] / 16.0]), tex, rgb);
        }
    }
    canvas.finish()
}

/// Ícone de um item `builtin/entity` (`TileEntityItemStackRenderer`): a geometria do tile entity em
/// unidades de bloco, já sombreada nos vértices com `gui_model_lights`, vista pela transformação da GUI.
pub fn render_entity_icon(geo: &mc_world::mesh::Geometry, tex: &RgbaImage, gui: Option<[[f32; 3]; 3]>) -> RgbaImage {
    let xf = GuiXform::new(gui);
    let px_per_unit = ITEM_PX as f32 / 16.0;
    let center = 8.0;
    let mut canvas = Canvas::new();
    for t in geo.indices.chunks_exact(3) {
        let v = [t[0], t[1], t[2]].map(|i| &geo.vertices[i as usize]);
        let p = v.map(|v| {
            let e = xf.to_eye(v.pos);
            [(e[0] + center) * px_per_unit, (e[1] + center) * px_per_unit, e[2]]
        });
        let c = v[0].color;
        let rgb = [c[0], c[1], c[2]].map(|c| c as f32 / 255.0);
        canvas.tri(p, v.map(|v| v.uv), tex, rgb);
    }
    canvas.finish()
}

/// Item plano (`builtin/generated`): camadas `layer0..` com a cor de cada camada, uma por cima da outra.
fn render_flat(jar: &mut Jar, m: &mc_world::model::ResolvedModel, tint: &dyn Fn(i32) -> u32) -> Option<RgbaImage> {
    let mut out: Option<RgbaImage> = None;
    for n in 0..8 {
        let name = m.texture(&format!("layer{n}"));
        if name == "missingno" {
            break;
        }
        let Some(img) = jar.texture(&name) else { continue };
        let layer = tinted(first_frame(img), tint(n));
        out = Some(match out {
            None => layer,
            Some(base) => layered(base, &layer),
        });
    }
    out
}

/// Brilho de `RenderItem.renderEffect`: duas passadas de `enchanted_item_glint.png` (SRC_COLOR, ONE) com a
/// matriz de textura escala(8)·translada·gira, cor -8372020, só onde o ícone foi desenhado.
fn glint_frame(base: &RgbaImage, glint: &RgbaImage, now_ms: u64) -> RgbaImage {
    // Coordenada de atlas: a posição da sprite no atlas do jogo não é reproduzida; usa o canto do atlas.
    const ATLAS: f32 = 512.0;
    let col = [0x80 as f32 / 255.0, 0x40 as f32 / 255.0, 0xCC as f32 / 255.0];
    let passes = [
        (now_ms % 3000) as f32 / 3000.0 / 8.0 * 1.0,
        -((now_ms % 4873) as f32 / 4873.0 / 8.0),
    ];
    let angles = [-50.0f32, 10.0];
    let mut out = base.clone();
    for (px, py, p) in out.enumerate_pixels_mut() {
        if p.0[3] == 0 {
            continue;
        }
        let (u, v) = ((px as f32 + 0.5) / 2.0 / ATLAS, (py as f32 + 0.5) / 2.0 / ATLAS);
        let mut add = [0.0f32; 3];
        for i in 0..2 {
            let (s, c) = angles[i].to_radians().sin_cos();
            let (ru, rv) = (u * c - v * s, u * s + v * c);
            let (tu, tv) = ((ru + passes[i]) * 8.0, rv * 8.0);
            let gx = (tu.rem_euclid(1.0) * glint.width() as f32) as u32 % glint.width();
            let gy = (tv.rem_euclid(1.0) * glint.height() as f32) as u32 % glint.height();
            let g = glint.get_pixel(gx, gy).0;
            for k in 0..3 {
                let src = g[k] as f32 / 255.0 * col[k];
                add[k] += src * src;
            }
        }
        for k in 0..3 {
            p.0[k] = ((p.0[k] as f32 / 255.0 + add[k]).min(1.0) * 255.0 + 0.5) as u8;
        }
    }
    out
}

/// Ícone do item: `ItemModelMesher.getItemModel` -> `models/item/<nome>.json` -> plano ou 3D.
fn render_icon(jar: &mut Jar, lib: &mut ModelLib, id: i16, damage: i16, leather: Option<u32>) -> Option<RgbaImage> {
    let name = model_name(id, damage)?;
    let mut bytes = |p: &str| jar.bytes(p);
    let model = lib.resolve(&format!("item/{name}"), &mut bytes)?;
    let tint = |layer: i32| layer_color(id, damage, layer.max(0) as u32, leather);
    match model.builtin.as_deref() {
        // Modelos desenhados por código (baús, cabeças, bandeiras): ainda não portados.
        Some("builtin/entity") => None,
        // O relógio e a bússola trocam de quadro por código; o ícone usa a textura base (camada 0).
        Some("builtin/generated") | Some("builtin/compass") | Some("builtin/clock") => render_flat(jar, &model, &tint),
        _ => {
            let mut names: Vec<String> = Vec::new();
            let baked = bake(&model, Rot::new(0, 0), false, &mut |t: &str| {
                let i = names.iter().position(|n| n == t).unwrap_or_else(|| {
                    names.push(t.to_string());
                    names.len() - 1
                });
                i as u16
            });
            Some(render_model3d(jar, &baked.quads, &names, model.gui, &tint))
        }
    }
}

/// Cache dos ícones já gravados no atlas da GUI.
#[derive(Default)]
pub struct ItemIcons {
    slots: HashMap<(i16, i16, u32), Option<u32>>,
    next: u32,
    lib: ModelLib,
    /// Ícones encantados: vaga e imagem sem brilho, recompostos com o tempo.
    enchanted: Vec<(u32, RgbaImage)>,
    glint: Option<RgbaImage>,
    last_bucket: u64,
    /// Quadro atual de ícones animados pelo mundo (bússola e relógio), por id de item.
    pub live: HashMap<i16, RgbaImage>,
}

impl ItemIcons {
    /// Vagas já gravadas no atlas para qualquer dano/variante do item `id`.
    pub fn slots_of(&self, id: i16) -> Vec<u32> {
        self.slots.iter().filter(|(k, _)| k.0 == id).filter_map(|(_, s)| *s).collect()
    }

    /// Vaga do ícone no atlas. Se ainda não existe, devolve também a imagem a gravar em `item_origin(vaga)`.
    pub fn get(&mut self, id: i16, damage: i16, variant: u32, jar: &mut Jar) -> (Option<u32>, Option<RgbaImage>) {
        let damage = normalize_damage(id, damage);
        if let Some(s) = self.slots.get(&(id, damage, variant)) {
            return (*s, None);
        }
        if self.next >= ITEM_SLOTS {
            return (None, None);
        }
        let leather = (variant & 0xFFFFFF != 0).then_some(variant & 0xFFFFFF);
        let img = render_icon(jar, &mut self.lib, id, damage, leather);
        // Bússola e relógio: o quadro atual de `TextureCompass`/`TextureClock` (`live`) no lugar do quadro base.
        let img = match self.live.get(&id) {
            Some(l) if img.is_some() => Some(l.clone()),
            _ => img,
        };
        let slot = img.as_ref().map(|_| {
            self.next += 1;
            self.next - 1
        });
        self.slots.insert((id, damage, variant), slot);
        if let (Some(s), Some(base), true) = (slot, &img, variant & ENCHANTED != 0) {
            self.enchanted.push((s, base.clone()));
            if self.glint.is_none() {
                self.glint = jar.texture("misc/enchanted_item_glint");
            }
            let shown = self.glint.as_ref().map(|g| glint_frame(base, g, self.last_bucket * 50)).unwrap_or_else(|| base.clone());
            return (slot, Some(shown));
        }
        (slot, img)
    }

    /// `display.gui` de um item `builtin/entity` (baú, cabeça, estandarte): `Some(gui)` se o modelo
    /// do item é desenhado por código (`TileEntityItemStackRenderer`), `None` se não.
    pub fn entity_display(&mut self, id: i16, damage: i16, jar: &mut Jar) -> Option<Option<[[f32; 3]; 3]>> {
        let name = model_name(id, normalize_damage(id, damage))?;
        let model = self.lib.resolve(&format!("item/{name}"), &mut |p: &str| jar.bytes(p))?;
        (model.builtin.as_deref() == Some("builtin/entity")).then_some(model.gui)
    }

    /// Grava (ou regrava) a imagem de um ícone feito fora daqui; devolve a vaga no atlas.
    pub fn put(&mut self, id: i16, damage: i16, variant: u32) -> Option<u32> {
        let key = (id, normalize_damage(id, damage), variant);
        if let Some(Some(s)) = self.slots.get(&key) {
            return Some(*s);
        }
        if self.next >= ITEM_SLOTS {
            return None;
        }
        self.next += 1;
        self.slots.insert(key, Some(self.next - 1));
        Some(self.next - 1)
    }

    /// Quadros novos do brilho (a cada 50 ms): `(vaga, imagem)` a regravar no atlas.
    pub fn animate(&mut self, now_ms: u64) -> Vec<(u32, RgbaImage)> {
        let bucket = now_ms / 50;
        if bucket == self.last_bucket || self.enchanted.is_empty() {
            self.last_bucket = bucket;
            return Vec::new();
        }
        self.last_bucket = bucket;
        let Some(g) = &self.glint else { return Vec::new() };
        self.enchanted.iter().map(|(s, base)| (*s, glint_frame(base, g, now_ms))).collect()
    }
}

/// Dano que muda o ícone: `ItemModelMesher.getMetadata` zera o dano dos itens que se gastam.
pub fn normalize_damage(id: i16, damage: i16) -> i16 {
    model_meta(id, damage)
}

pub fn icon_origin(slot: u32) -> (u32, u32) {
    item_origin(slot)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Um quadrado em z = 0.5 (unidades de bloco) cai no meio do ícone; a luz volta para o espaço do modelo.
    #[test]
    fn icone_de_geometria_de_entidade() {
        use mc_world::mesh::{Geometry, Vertex};
        let v = |x: f32, y: f32, u: f32, w: f32| Vertex { pos: [x, y, 0.5], uv: [u, w], color: [255, 255, 255, 255], light: [255; 4] };
        let mut geo = Geometry::default();
        geo.quad([v(0.25, 0.25, 0.0, 0.0), v(0.75, 0.25, 1.0, 0.0), v(0.75, 0.75, 1.0, 1.0), v(0.25, 0.75, 0.0, 1.0)]);
        let tex = RgbaImage::from_pixel(2, 2, image::Rgba([200, 100, 50, 255]));
        let img = render_entity_icon(&geo, &tex, None);
        assert_eq!(img.dimensions(), (ITEM_PX, ITEM_PX));
        let opaque = img.pixels().filter(|p| p.0[3] == 255).count();
        assert!(opaque > 0 && opaque < (ITEM_PX * ITEM_PX) as usize, "{opaque}");
        let c = img.pixels().find(|p| p.0[3] == 255).unwrap().0;
        assert_eq!(c, [200, 100, 50, 255]);
        // As luzes de volta ao modelo são unitárias (rotações apenas).
        for l in gui_model_lights(Some([[30.0, 45.0, 0.0], [0.0; 3], [0.625; 3]])) {
            assert!(((l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt() - 1.0).abs() < 1e-4);
        }
    }

    /// Gera uma folha de ícones em $MINE_RS_FOLHA (png) com o jar real; ignorado sem o jar.
    #[test]
    fn folha_de_icones() {
        let Some(out) = std::env::var_os("MINE_RS_FOLHA") else { return };
        let jar_path = dirs_jar();
        let Ok(mut jar) = Jar::open(&jar_path) else { return };
        let mut icons = ItemIcons::default();
        let list: &[(i16, i16, u32)] = &[
            (2, 0, 0), (1, 0, 0), (17, 0, 0), (18, 0, 0), (35, 14, 0), (35, 5, 0), (54, 0, 0), (58, 0, 0), (61, 0, 0), (44, 0, 0), (53, 0, 0), (96, 0, 0),
            (345, 0, 0), (347, 0, 0), (267, 0, 0), (276, 0, 0), (261, 0, 0), (373, 5, 0), (373, 16389, 0), (383, 50, 0), (298, 0, 0), (298, 0, 0x3A5FCD),
            (299, 0, 0), (351, 4, 0), (322, 0, 0), (322, 1, 0), (38, 0, 0), (6, 0, 0), (50, 0, 0), (276, 0, ENCHANTED), (116, 0, 0), (145, 0, 0), (323, 0, 0), (355, 0, 0),
        ];
        let cols = 12u32;
        let rows = (list.len() as u32 + cols - 1) / cols;
        let mut sheet = RgbaImage::from_pixel(cols * 34, rows * 34, image::Rgba([139, 139, 139, 255]));
        for (i, (id, dmg, var)) in list.iter().enumerate() {
            let (slot, img) = icons.get(*id, *dmg, *var, &mut jar);
            eprintln!("{id}:{dmg} -> {:?} {:?}", slot, model_name(*id, *dmg));
            if let Some(img) = img {
                let (ox, oy) = ((i as u32 % cols) * 34 + 1, (i as u32 / cols) * 34 + 1);
                image::imageops::overlay(&mut sheet, &img, ox as i64, oy as i64);
            }
        }
        sheet.save(out).unwrap();
    }

    fn dirs_jar() -> std::path::PathBuf {
        let home = std::env::var("HOME").unwrap_or_default();
        format!("{home}/.local/share/PrismLauncher/libraries/com/mojang/minecraft/1.8.9/minecraft-1.8.9-client.jar").into()
    }
}
