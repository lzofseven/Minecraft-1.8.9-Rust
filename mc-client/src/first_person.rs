//! Primeira pessoa (`ItemRenderer.renderItemInFirstPerson`): braço com a skin,
//! item na mão (bloco 3D ou item com espessura do `ItemModelGenerator`), troca de
//! item (`updateEquippedItem`), soco, comer/beber, defender com a espada e arco.
//! As transformações de `firstperson` vêm dos `models/item/*.json` do jar.

use std::collections::HashMap;

use glam::{Mat4, Vec3};
use image::RgbaImage;
use mc_protocol::nbt::Slot;
use mc_render::{DrawItem, Jar, Renderer, Tex};
use mc_world::blocks::{BlockTable, Layer};
use mc_world::items::{layer_color, model_name};
use mc_world::model::{bake, ModelLib, Rot};
use mc_world::mesh::{atlas_uv, Geometry, Vertex};

use crate::camera::{rot, tr};
use crate::models::{diffuse_with, Anim, Biped, Sink};
use crate::mth;
use crate::physics::{Player, UseAction};

/// Luzes do `RenderHelper.enableStandardItemLighting` (antes do giro).
const LIGHT0: Vec3 = Vec3::new(0.2, 1.0, -0.7);
const LIGHT1: Vec3 = Vec3::new(-0.2, 1.0, 0.7);

/// `ItemTransformVec3f` de `display.firstperson`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Display {
    pub rotation: Vec3,
    pub translation: Vec3,
    pub scale: Vec3,
}

impl Display {
    /// `ItemCameraTransforms.applyTransform`.
    pub fn matrix(&self) -> Mat4 {
        let t = self.translation;
        tr(t.x, t.y, t.z)
            * rot(self.rotation.y, 0.0, 1.0, 0.0)
            * rot(self.rotation.x, 1.0, 0.0, 0.0)
            * rot(self.rotation.z, 0.0, 0.0, 1.0)
            * Mat4::from_scale(self.scale)
    }
}

/// Quadrilátero de modelo em pixels (0..16), UV final na textura e normal.
#[derive(Clone, Debug)]
pub struct MQuad {
    pub pos: [Vec3; 4],
    pub uv: [[f32; 2]; 4],
    pub normal: Vec3,
    /// Cor RGB multiplicada (0xFFFFFF = sem tinta).
    pub tint: u32,
}

pub struct ItemModel {
    pub tex: Tex,
    pub quads: Vec<MQuad>,
    pub gui3d: bool,
    pub translucent: bool,
    /// `display.firstperson` e `display.thirdperson`.
    pub display: Option<Display>,
    pub display_tp: Option<Display>,
    /// `display.ground` (item no chão) e `display.fixed` (moldura); ausentes = sem transformação.
    pub display_ground: Option<Display>,
    pub display_fixed: Option<Display>,
    /// `display.head` (`TransformType.HEAD`: bloco na cabeça, `LayerCustomHead`).
    pub display_head: Option<Display>,
    /// `builtin/entity`: desenhado por `TileEntityItemStackRenderer` (baús, cabeça, estandarte), sem quads.
    pub entity: bool,
}

impl ItemModel {
    /// `ItemRenderer.renderItem` a partir de `m`: dobra o 3D, aplica o display e o -0.5 do RenderItem.
    pub fn item_matrix(&self, mut m: Mat4, third_person: bool) -> Mat4 {
        // Um 2x para todo item: `ItemRenderer.renderItem` dobra os `isGui3d` (blocos) e
        // `RenderItem.preTransform` dobra os outros (itens planos).
        m *= Mat4::from_scale(Vec3::splat(2.0));
        if let Some(d) = if third_person { self.display_tp } else { self.display } {
            m *= d.matrix();
        }
        m * Mat4::from_scale(Vec3::splat(0.5)) * tr(-0.5, -0.5, -0.5)
    }

    /// Geometria do item `builtin/entity` (`renderByItem`) com a textura de `tex`.
    pub fn emit_entity(&self, item: &crate::tile_entities::EntityItem, m: Mat4, third_person: bool, lights: &[Vec3; 2], light: [u8; 4], geo: &mut Geometry) {
        let mut sink = Sink { geo, tex: (64.0, 64.0), tint: [1.0; 3], alpha: 255, light, lights: *lights };
        crate::tile_entities::emit_item(item, &mut sink, &builtin_matrix(self.item_matrix(m, third_person)));
    }

    pub fn emit(&self, m: Mat4, third_person: bool, lights: &[Vec3; 2], light: [u8; 4], geo: &mut Geometry) {
        let m = self.item_matrix(m, third_person);
        self.emit_raw(m, lights, light, geo);
    }

    /// Quads com `m` já completa (`RenderItem.renderItem`: escala 0.5 e translação -0.5 inclusas por quem chama).
    pub fn emit_raw(&self, m: Mat4, lights: &[Vec3; 2], light: [u8; 4], geo: &mut Geometry) {
        for q in &self.quads {
            let n = m.transform_vector3(q.normal).normalize_or_zero();
            let k = diffuse_with(n, lights);
            let ch = |sh: u32| (((q.tint >> sh) & 0xFF) as f32 * k) as u8;
            let color = [ch(16), ch(8), ch(0), 255];
            let vert = |i: usize| Vertex { pos: m.transform_point3(q.pos[i] / 16.0).to_array(), uv: q.uv[i], color, light };
            geo.quad([vert(0), vert(1), vert(2), vert(3)]);
        }
    }
}

/// Modelos de item já montados (por id e dano), usados na mão e na terceira pessoa.
#[derive(Default)]
pub struct ItemModels {
    models: HashMap<(i16, i16), Option<std::rc::Rc<ItemModel>>>,
    /// Modelos trocados (`entity_override`) por item e nome.
    named: HashMap<(i16, i16, &'static str), Option<std::rc::Rc<ItemModel>>>,
    lib: ModelLib,
    /// Quadro atual de `TextureCompass`/`TextureClock` (345, 347): a geometria sai do primeiro quadro
    /// (`ItemModelGenerator`) e a textura amostrada é o sprite animado do atlas.
    live: HashMap<i16, RgbaImage>,
}

impl ItemModels {
    pub fn get(&mut self, id: i16, damage: i16, jar: &mut Jar, table: &BlockTable, r: &mut Renderer) -> Option<std::rc::Rc<ItemModel>> {
        let key = (id, damage);
        if let Some(m) = self.models.get(&key) {
            return m.clone();
        }
        let m = build_model(id, damage, jar, &mut self.lib, table, r).map(std::rc::Rc::new);
        if let (Some(m), Some(img)) = (&m, self.live.get(&id)) {
            write_live(m, img, r);
        }
        self.models.insert(key, m.clone());
        m
    }

    /// `get`, ou o modelo trocado `name` quando há um (`entity_override`).
    pub fn get_for(&mut self, id: i16, damage: i16, name: Option<&'static str>, jar: &mut Jar, table: &BlockTable, r: &mut Renderer) -> Option<std::rc::Rc<ItemModel>> {
        let Some(name) = name else { return self.get(id, damage, jar, table, r) };
        if let Some(m) = self.named.get(&(id, damage, name)) {
            return m.clone();
        }
        let m = build_model_named(id, damage, name, jar, &mut self.lib, table, r).map(std::rc::Rc::new);
        self.named.insert((id, damage, name), m.clone());
        m
    }

    /// Bússola e relógio mudaram de quadro: regrava a textura dos modelos já montados.
    pub fn set_live(&mut self, id: i16, img: RgbaImage, r: &mut Renderer) {
        for ((mid, _), m) in &self.models {
            if *mid == id {
                if let Some(m) = m {
                    write_live(m, &img, r);
                }
            }
        }
        self.live.insert(id, img);
    }
}

/// Modelo gerado de uma camada (`builtin/compass`/`builtin/clock`): troca a imagem pelo quadro vivo.
fn write_live(m: &ItemModel, img: &RgbaImage, r: &mut Renderer) {
    let Tex::Custom(tid) = m.tex else { return };
    let Some(&(w, h)) = r.custom_size(tid).as_ref() else { return };
    if w != h {
        return;
    }
    let frame = if img.width() == w { img.clone() } else { image::imageops::resize(img, w, w, image::imageops::FilterType::Nearest) };
    r.update_texture(tid, frame.as_raw());
}

/// Cantos de uma face (`EnumFaceDirection`) e a normal, na ordem 0 baixo, 1 cima, 2 norte, 3 sul, 4 oeste, 5 leste.
fn face_corners(face: usize, a: Vec3, b: Vec3) -> ([Vec3; 4], Vec3) {
    let v = Vec3::new;
    match face {
        0 => ([v(a.x, a.y, b.z), v(a.x, a.y, a.z), v(b.x, a.y, a.z), v(b.x, a.y, b.z)], -Vec3::Y),
        1 => ([v(a.x, b.y, a.z), v(a.x, b.y, b.z), v(b.x, b.y, b.z), v(b.x, b.y, a.z)], Vec3::Y),
        2 => ([v(b.x, b.y, a.z), v(b.x, a.y, a.z), v(a.x, a.y, a.z), v(a.x, b.y, a.z)], -Vec3::Z),
        3 => ([v(a.x, b.y, b.z), v(a.x, a.y, b.z), v(b.x, a.y, b.z), v(b.x, b.y, b.z)], Vec3::Z),
        4 => ([v(a.x, b.y, a.z), v(a.x, a.y, a.z), v(a.x, a.y, b.z), v(a.x, b.y, b.z)], -Vec3::X),
        _ => ([v(b.x, b.y, b.z), v(b.x, a.y, b.z), v(b.x, a.y, a.z), v(b.x, b.y, a.z)], Vec3::X),
    }
}

/// Vértice i recebe (u1,v1), (u1,v2), (u2,v2), (u2,v1) (`BlockFaceUV`, rotação 0).
fn corner_uv(uv: [f32; 4]) -> [[f32; 2]; 4] {
    [[uv[0], uv[1]], [uv[0], uv[3]], [uv[2], uv[3]], [uv[2], uv[1]]]
}

/// `ItemModelGenerator`: frente e verso da camada e uma tira por borda de pixel opaco.
pub fn generated_quads(img: &RgbaImage) -> Vec<MQuad> {
    let (w, h) = (img.width() as i32, img.width() as i32);
    let transparent = |x: i32, y: i32| x < 0 || y < 0 || x >= w || y >= h || img.get_pixel(x as u32, y as u32).0[3] == 0;
    let mut out = Vec::new();
    let full = |face: usize, uv: [f32; 4]| {
        let (a, b) = (Vec3::new(0.0, 0.0, 7.5), Vec3::new(16.0, 16.0, 8.5));
        let (pos, normal) = face_corners(face, a, b);
        MQuad { pos, uv: corner_uv(uv).map(|[u, v]| [u / 16.0, v / 16.0]), normal, tint: 0xFFFFFF }
    };
    out.push(full(3, [0.0, 0.0, 16.0, 16.0]));
    out.push(full(2, [16.0, 0.0, 0.0, 16.0]));
    // Spans: (lado, min, max, linha/coluna). Lado: 0 UP, 1 DOWN, 2 LEFT, 3 RIGHT.
    let mut spans: Vec<(usize, i32, i32, i32)> = Vec::new();
    let dirs = [(0, -1), (0, 1), (-1, 0), (1, 0)];
    for y in 0..h {
        for x in 0..w {
            let solid = !transparent(x, y);
            for (side, (dx, dy)) in dirs.iter().enumerate() {
                if solid && transparent(x + dx, y + dy) {
                    let vertical = side < 2;
                    let (key, along) = if vertical { (y, x) } else { (x, y) };
                    match spans.iter_mut().find(|s| s.0 == side && s.3 == key) {
                        Some(s) => {
                            if along < s.1 {
                                s.1 = along;
                            } else if along > s.2 {
                                s.2 = along;
                            }
                        }
                        None => spans.push((side, along, along, key)),
                    }
                }
            }
        }
    }
    let (fw, fh) = (w as f32, h as f32);
    for (side, lo, hi, key) in spans {
        let (b, c, d) = (lo as f32, hi as f32, key as f32);
        // (x0, y0, x1, y1) da caixa e (u0, v0, u1, v1) da UV, antes das escalas.
        let (mut x0, mut y0, mut x1, mut y1, u0, v0, u1, v1, su, sv) = match side {
            0 => (b, d, c + 1.0, d, b, d, c + 1.0, d, 16.0 / fw, 16.0 / (fh - 1.0)),
            1 => (b, d + 1.0, c + 1.0, d + 1.0, b, d, c + 1.0, d, 16.0 / fw, 16.0 / (fh - 1.0)),
            2 => (d, b, d, c + 1.0, d, c + 1.0, d, b, 16.0 / (fw - 1.0), 16.0 / fh),
            _ => (d + 1.0, b, d + 1.0, c + 1.0, d, c + 1.0, d, b, 16.0 / (fw - 1.0), 16.0 / fh),
        };
        let (kx, ky) = (16.0 / fw, 16.0 / fh);
        x0 *= kx;
        x1 *= kx;
        y0 = 16.0 - y0 * ky;
        y1 = 16.0 - y1 * ky;
        let uv = [u0 * su, v0 * sv, u1 * su, v1 * sv];
        // Lado -> face e caixa da BlockPart (de/até).
        let (face, a, bb) = match side {
            0 => (1, Vec3::new(x0, y0, 7.5), Vec3::new(x1, y0, 8.5)),
            1 => (0, Vec3::new(x0, y1, 7.5), Vec3::new(x1, y1, 8.5)),
            2 => (5, Vec3::new(x0, y0, 7.5), Vec3::new(x0, y1, 8.5)),
            _ => (4, Vec3::new(x1, y0, 7.5), Vec3::new(x1, y1, 8.5)),
        };
        // A BlockPart guarda de/até como vieram (o Y já invertido): os índices do
        // `EnumFaceDirection` pegam "de" e "até" assim mesmo, sem ordenar.
        let (pos, normal) = face_corners(face, a, bb);
        out.push(MQuad { pos, uv: corner_uv(uv).map(|[u, v]| [u / 16.0, v / 16.0]), normal, tint: 0xFFFFFF });
    }
    out
}

/// Primeiro quadro (texturas animadas são tiras verticais).
fn first_frame(img: RgbaImage) -> RgbaImage {
    let w = img.width();
    if img.height() > w {
        image::imageops::crop_imm(&img, 0, 0, w, w).to_image()
    } else {
        img
    }
}

pub struct FirstPerson {
    /// `itemToRender`, `equippedItemSlot`, `equippedProgress`.
    pub item: Option<Slot>,
    slot: i32,
    pub equipped: f32,
    pub prev_equipped: f32,
}

impl Default for FirstPerson {
    fn default() -> Self {
        Self { item: None, slot: -1, equipped: 0.0, prev_equipped: 0.0 }
    }
}

/// `ItemStack.getIsItemStackEqual`: quantidade, item, dano e NBT.
fn same_stack(a: &Slot, b: &Slot) -> bool {
    a.id == b.id && a.count == b.count && a.damage == b.damage && a.raw_nbt == b.raw_nbt
}

impl FirstPerson {
    /// `ItemRenderer.updateEquippedItem`.
    pub fn update_equipped(&mut self, held: Option<&Slot>, selected: u8) {
        self.prev_equipped = self.equipped;
        let flag = match (&self.item, held) {
            (Some(a), Some(b)) => !same_stack(a, b),
            (None, None) => false,
            _ => true,
        };
        let f = 0.4f32;
        let target = if flag { 0.0 } else { 1.0 };
        self.equipped += (target - self.equipped).clamp(-f, f);
        if self.equipped < 0.1 {
            self.item = held.cloned();
            self.slot = selected as i32;
        }
    }

    /// Geometria da mão/item no espaço da câmera (`renderItemInFirstPerson`).
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_arguments)]
    pub fn build(&mut self, p: &Player, partial: f32, base: Mat4, light: [u8; 4], skin: Option<(u32, bool, u8)>, fishing: bool, items: &mut ItemModels, jar: &mut Jar, table: &BlockTable, r: &mut Renderer, (maps, textures): (&mut crate::maps::Maps, &mut crate::skins::Textures)) -> Vec<DrawItem> {
        let f = 1.0 - (self.prev_equipped + (self.equipped - self.prev_equipped) * partial);
        let f1 = p.swing(partial);
        let f2 = p.prev_pitch + (p.pitch - p.prev_pitch) * partial;
        let f3 = p.prev_yaw + (p.yaw - p.prev_yaw) * partial;
        // rotateArroundXAndY: as luzes ficam presas à visão do jogador.
        let lm = base * rot(f2, 1.0, 0.0, 0.0) * rot(f3, 0.0, 1.0, 0.0);
        let lights = [lm.transform_vector3(LIGHT0).normalize(), lm.transform_vector3(LIGHT1).normalize()];
        // rotateWithPlayerRotations: o braço atrasa um pouco ao virar.
        let arm_pitch = p.prev_render_arm_pitch + (p.render_arm_pitch - p.prev_render_arm_pitch) * partial;
        let arm_yaw = p.prev_render_arm_yaw + (p.render_arm_yaw - p.prev_render_arm_yaw) * partial;
        let mut m = base * rot((p.pitch - arm_pitch) * 0.1, 1.0, 0.0, 0.0) * rot((p.yaw - arm_yaw) * 0.1, 0.0, 1.0, 0.0);
        let mut out = Vec::new();
        if let Some(item) = self.item.clone().filter(|i| i.id == 358) {
            // renderItemMap(jogador, pitch, equipado, balanço).
            let s = f1.sqrt();
            m *= tr(-0.4 * mth::sin(s * mth::PI), 0.2 * mth::sin(s * mth::PI * 2.0), -0.2 * mth::sin(f1 * mth::PI));
            let a = map_angle(f2);
            m *= tr(0.0, 0.04, -0.72) * tr(0.0, f * -1.2, 0.0) * tr(0.0, a * -0.5, 0.0) * rot(90.0, 0.0, 1.0, 0.0) * rot(a * -85.0, 0.0, 0.0, 1.0);
            // renderPlayerArms: os dois braços com a pose de quem segura um item (heldItemRight = 1).
            if let Some((tex, slim, parts)) = skin {
                let mut model = Biped::player(slim);
                model.set_skin_parts(parts);
                model.pose(&Anim { held_right: 1, ..Default::default() });
                let mut geo = Geometry::default();
                let mut sink = Sink { geo: &mut geo, tex: (64.0, 64.0), tint: [1.0; 3], alpha: 255, light, lights };
                let right = m * rot(54.0, 0.0, 1.0, 0.0) * rot(64.0, 1.0, 0.0, 0.0) * rot(-62.0, 0.0, 0.0, 1.0) * tr(0.25, -0.85, 0.75);
                sink.part(&model.right_arm, &right);
                let left = m * rot(92.0, 0.0, 1.0, 0.0) * rot(45.0, 1.0, 0.0, 0.0) * rot(41.0, 0.0, 0.0, 1.0) * tr(-0.3, -1.1, 0.45);
                sink.part(&model.left_arm, &left);
                if let Some(w) = &model.wear {
                    sink.part(&w[2], &right);
                    sink.part(&w[1], &left);
                }
                out.push(DrawItem { tex: Tex::Custom(tex), geo, translucent: false });
            }
            let f4 = mth::sin(f1 * f1 * mth::PI);
            let f5 = mth::sin(f1.sqrt() * mth::PI);
            m *= rot(f4 * -20.0, 0.0, 1.0, 0.0) * rot(f5 * -20.0, 0.0, 0.0, 1.0) * rot(f5 * -80.0, 1.0, 0.0, 0.0) * Mat4::from_scale(Vec3::splat(0.38));
            m *= rot(90.0, 0.0, 1.0, 0.0) * rot(180.0, 0.0, 0.0, 1.0) * tr(-1.0, -1.0, 0.0) * Mat4::from_scale(Vec3::splat(0.015625));
            // glNormal3f(0, 0, -1) com a luz de itens ligada: fundo e mapa sombreados pela mesma normal.
            let shade = crate::models::diffuse_with(m.transform_vector3(Vec3::new(0.0, 0.0, -1.0)).normalize_or_zero(), &lights);
            let c = (shade * 255.0) as u8;
            if let Some(bg) = textures.jar(r, jar, "map/map_background") {
                let v = |x: f32, y: f32, u: f32, w: f32| Vertex { pos: m.transform_point3(Vec3::new(x, y, 0.0)).to_array(), uv: [u, w], color: [c, c, c, 255], light };
                let mut geo = Geometry::default();
                geo.quad([v(-7.0, 135.0, 0.0, 1.0), v(135.0, 135.0, 1.0, 1.0), v(135.0, -7.0, 1.0, 0.0), v(-7.0, -7.0, 0.0, 0.0)]);
                out.push(DrawItem { tex: Tex::Custom(bg), geo, translucent: false });
            }
            let mut layers = HashMap::new();
            crate::maps::emit(maps, textures, r, jar, item.damage as i32, &m, false, light, shade, &mut layers);
            for (tex, geo) in layers {
                match tex {
                    Tex::Custom(id) => out.push(DrawItem { tex: Tex::Custom(id & !crate::mobs::TRANSLUCENT), geo, translucent: true }),
                    tex => out.push(DrawItem { tex, geo, translucent: true }),
                }
            }
            return out;
        }
        if let Some(item) = self.item.clone() {
            let use_ticks = p.item_use.map(|u| u.max_duration - u.count);
            let Some(model) = items.get_for(item.id, item.damage, entity_override(item.id, fishing, use_ticks), jar, table, r) else { return out };
            let using = p.item_use.filter(|u| u.count > 0);
            match using {
                Some(u) => match u.action {
                    UseAction::Eat | UseAction::Drink => {
                        m *= drinking(u.count, partial, u.max_duration) * transform_first_person(f, 0.0);
                    }
                    UseAction::Block => m *= transform_first_person(f, 0.0) * block_transform(),
                    UseAction::Bow => m *= transform_first_person(f, 0.0) * bow_transform(u.count, partial, u.max_duration),
                },
                None => m *= item_used(f1) * transform_first_person(f, f1),
            }
            let mut geo = Geometry::default();
            if model.entity {
                let Some(ent) = crate::tile_entities::entity_item(&item) else { return out };
                let Some(tex) = crate::tile_entities::item_texture(&ent, textures, r, jar) else { return out };
                model.emit_entity(&ent, m, false, &lights, light, &mut geo);
                out.push(DrawItem { tex: Tex::Custom(tex), geo, translucent: false });
                return out;
            }
            model.emit(m, false, &lights, light, &mut geo);
            out.push(DrawItem { tex: model.tex, geo, translucent: model.translucent });
        } else if let Some((tex, slim, parts)) = skin {
            m *= arm_transform(f, f1);
            // RenderPlayer.renderRightArm: pose parada, sem soco e sem agachar.
            let mut model = Biped::player(slim);
            model.set_skin_parts(parts);
            model.pose(&Anim::default());
            let mut geo = Geometry::default();
            let mut sink = Sink { geo: &mut geo, tex: (64.0, 64.0), tint: [1.0; 3], alpha: 255, light, lights };
            sink.part(&model.right_arm, &m);
            if let Some(w) = &model.wear {
                sink.part(&w[2], &m);
            }
            out.push(DrawItem { tex: Tex::Custom(tex), geo, translucent: false });
        }
        out
    }
}

/// `RenderItem.renderItem` com `isBuiltInRenderer`: depois do 0,5 e do -0,5 (já em `m`) vem o giro de 180° em
/// Y antes do `renderByItem`; como `m` já traz o -0,5, ele é desfeito, girado e refeito.
pub fn builtin_matrix(m: Mat4) -> Mat4 {
    m * tr(0.5, 0.5, 0.5) * rot(180.0, 0.0, 1.0, 0.0) * tr(-0.5, -0.5, -0.5)
}

/// `ItemRenderer.getMapAngleFromPitch`.
fn map_angle(pitch: f32) -> f32 {
    let f = (1.0 - pitch / 45.0 + 0.1).clamp(0.0, 1.0);
    -mth::cos(f * mth::PI) * 0.5 + 0.5
}

/// `transformFirstPersonItem`.
pub fn transform_first_person(equip: f32, swing: f32) -> Mat4 {
    let f = mth::sin(swing * swing * mth::PI);
    let f1 = mth::sin(swing.sqrt() * mth::PI);
    tr(0.56, -0.52, -0.71999997)
        * tr(0.0, equip * -0.6, 0.0)
        * rot(45.0, 0.0, 1.0, 0.0)
        * rot(f * -20.0, 0.0, 1.0, 0.0)
        * rot(f1 * -20.0, 0.0, 0.0, 1.0)
        * rot(f1 * -80.0, 1.0, 0.0, 0.0)
        * Mat4::from_scale(Vec3::splat(0.4))
}

/// `doItemUsedTransformations`.
fn item_used(swing: f32) -> Mat4 {
    let s = swing.sqrt();
    tr(-0.4 * mth::sin(s * mth::PI), 0.2 * mth::sin(s * mth::PI * 2.0), -0.2 * mth::sin(swing * mth::PI))
}

/// `doBlockTransformations` (espada defendendo).
fn block_transform() -> Mat4 {
    tr(-0.5, 0.2, 0.0) * rot(30.0, 0.0, 1.0, 0.0) * rot(-80.0, 1.0, 0.0, 0.0) * rot(60.0, 0.0, 1.0, 0.0)
}

/// `performDrinking` (comer e beber).
fn drinking(count: i32, partial: f32, max: i32) -> Mat4 {
    let f = count as f32 - partial + 1.0;
    let f1 = f / max as f32;
    let mut f2 = (mth::cos(f / 4.0 * mth::PI) * 0.1).abs();
    if f1 >= 0.8 {
        f2 = 0.0;
    }
    let f3 = 1.0 - (f1 as f64).powf(27.0) as f32;
    tr(0.0, f2, 0.0) * tr(f3 * 0.6, f3 * -0.5, f3 * 0.0) * rot(f3 * 90.0, 0.0, 1.0, 0.0) * rot(f3 * 10.0, 1.0, 0.0, 0.0) * rot(f3 * 30.0, 0.0, 0.0, 1.0)
}

/// `doBowTransformations`.
fn bow_transform(count: i32, partial: f32, max: i32) -> Mat4 {
    let mut m = rot(-18.0, 0.0, 0.0, 1.0) * rot(-12.0, 0.0, 1.0, 0.0) * rot(-8.0, 1.0, 0.0, 0.0) * tr(-0.9, 0.2, 0.0);
    let f = max as f32 - (count as f32 - partial + 1.0);
    let mut f1 = f / 20.0;
    f1 = (f1 * f1 + f1 * 2.0) / 3.0;
    if f1 > 1.0 {
        f1 = 1.0;
    }
    if f1 > 0.1 {
        let f2 = mth::sin((f - 0.1) * 1.3);
        let f4 = f2 * (f1 - 0.1);
        m *= tr(f4 * 0.0, f4 * 0.01, f4 * 0.0);
    }
    m * tr(f1 * 0.0, f1 * 0.0, f1 * 0.1) * Mat4::from_scale(Vec3::new(1.0, 1.0, 1.0 + f1 * 0.2))
}

/// `renderPlayerArm` até o `renderRightArm`.
pub fn arm_transform(equip: f32, swing: f32) -> Mat4 {
    let s = swing.sqrt();
    let f = -0.3 * mth::sin(s * mth::PI);
    let f1 = 0.4 * mth::sin(s * mth::PI * 2.0);
    let f2 = -0.4 * mth::sin(swing * mth::PI);
    let f3 = mth::sin(swing * swing * mth::PI);
    let f4 = mth::sin(s * mth::PI);
    tr(f, f1, f2)
        * tr(0.64000005, -0.6, -0.71999997)
        * tr(0.0, equip * -0.6, 0.0)
        * rot(45.0, 0.0, 1.0, 0.0)
        * rot(f4 * 70.0, 0.0, 1.0, 0.0)
        * rot(f3 * -20.0, 0.0, 0.0, 1.0)
        * tr(-1.0, 3.6, 3.5)
        * rot(120.0, 0.0, 0.0, 1.0)
        * rot(200.0, 1.0, 0.0, 0.0)
        * rot(-135.0, 0.0, 1.0, 0.0)
        * tr(5.6, 0.0, 0.0)
}

/// `ItemModelMesher.getItemModel` + `RenderItem`: `model_name` -> `models/item/<nome>.json` do jar.
/// Pai de bloco (`block/...`): quads do bake (texturas no atlas de blocos, tinta por `tintindex` com
/// `layer_color`); `builtin/generated` (e bússola/relógio, que usam a camada base): malha do
/// `ItemModelGenerator` com uma textura e uma cor por camada `layer0..N`.
/// `display.firstperson/thirdperson` vêm do próprio JSON, com herança de `parent`.
/// `builtin/entity` (baú, cabeça, estandarte) usa `tile_entities::emit_item`; pendente: cor de couro por NBT.
fn build_model(id: i16, damage: i16, jar: &mut Jar, lib: &mut ModelLib, table: &BlockTable, r: &mut Renderer) -> Option<ItemModel> {
    let name = model_name(id, damage)?;
    build_model_named(id, damage, &name, jar, lib, table, r)
}

/// `RenderItem.renderItemModelForEntity`: modelo trocado pelo estado do jogador — vara com a linha na água
/// (`fishing_rod_cast`) e arco puxado pelos ticks de uso (`bow_pulling_0..2`).
pub fn entity_override(id: i16, fishing: bool, use_ticks: Option<i32>) -> Option<&'static str> {
    match (id, use_ticks) {
        (346, _) if fishing => Some("fishing_rod_cast"),
        (261, Some(i)) if i >= 18 => Some("bow_pulling_2"),
        (261, Some(i)) if i > 13 => Some("bow_pulling_1"),
        (261, Some(i)) if i > 0 => Some("bow_pulling_0"),
        _ => None,
    }
}

fn build_model_named(id: i16, damage: i16, name: &str, jar: &mut Jar, lib: &mut ModelLib, table: &BlockTable, r: &mut Renderer) -> Option<ItemModel> {
    let model = {
        let mut bytes = |p: &str| jar.bytes(p);
        lib.resolve(&format!("item/{name}"), &mut bytes)?
    };
    let view = |v: &str| {
        model.display.get(v).map(|&[rotation, translation, scale]| Display {
            rotation: Vec3::from(rotation),
            translation: Vec3::from(translation),
            scale: Vec3::from(scale),
        })
    };
    let (display, display_tp) = (view("firstperson"), view("thirdperson"));
    let tint = |layer: u32| layer_color(id, damage, layer, None);
    let built = match model.builtin.as_deref() {
        // Baú, cabeça e estandarte: a geometria sai de `tile_entities::emit_item` (precisa do NBT do item).
        Some("builtin/entity") => Some(ItemModel { tex: Tex::Blocks, quads: Vec::new(), gui3d: true, translucent: false, display, display_tp, display_ground: None, display_fixed: None, display_head: None, entity: true }),
        Some("builtin/generated") | Some("builtin/compass") | Some("builtin/clock") => {
            let mut layers: Vec<(RgbaImage, u32)> = Vec::new();
            for n in 0..8u32 {
                let tex = model.texture(&format!("layer{n}"));
                if tex == "missingno" {
                    break;
                }
                if let Some(img) = jar.texture(&tex) {
                    layers.push((first_frame(img), tint(n)));
                }
            }
            generated(layers, (display, display_tp), r)
        }
        _ => {
            let mut names: Vec<String> = Vec::new();
            let baked = bake(&model, Rot::new(0, 0), false, &mut |t: &str| {
                let i = names.iter().position(|n| n == t).unwrap_or_else(|| {
                    names.push(t.to_string());
                    names.len() - 1
                });
                i as u16
            });
            // Texturas do modelo -> posição no atlas de blocos.
            let tiles: Vec<Option<u16>> = names
                .iter()
                .map(|n| n.strip_prefix("blocks/").and_then(|b| table.textures.iter().position(|t| t == b)).map(|i| i as u16))
                .collect();
            let mut quads = Vec::new();
            for q in &baked.quads {
                let Some(tile) = tiles.get(q.tex as usize).copied().flatten() else {
                    eprintln!("item {id}:{damage} ({name}): textura {} fora do atlas de blocos", names[q.tex as usize]);
                    return None;
                };
                let pos = q.pos.map(|p| Vec3::from(p) * 16.0);
                let uv = q.uv.map(|[u, v]| atlas_uv(tile, u, v));
                let normal = FACE_NORMALS[q.face as usize % 6];
                let tint = if q.tint >= 0 { tint(q.tint as u32) } else { 0xFFFFFF };
                quads.push(MQuad { pos, uv, normal, tint });
            }
            // Bloco translúcido (vidro colorido, gelo): mistura de alfa como o `Block.getRenderBlockPass`.
            let translucent = (0..256).contains(&id) && table.get(((id as u16) << 4) | (damage as u16 & 15)).layer == Layer::Translucent;
            Some(ItemModel { tex: Tex::Blocks, quads, gui3d: true, translucent, display, display_tp, display_ground: None, display_fixed: None, display_head: None, entity: false })
        }
    };
    built.map(|mut m| {
        m.display_ground = view("ground");
        m.display_fixed = view("fixed");
        m.display_head = view("head");
        m
    })
}

/// Normais de `EnumFacing` na ordem baixo, cima, norte, sul, oeste, leste.
const FACE_NORMALS: [Vec3; 6] = [
    Vec3::new(0.0, -1.0, 0.0),
    Vec3::new(0.0, 1.0, 0.0),
    Vec3::new(0.0, 0.0, -1.0),
    Vec3::new(0.0, 0.0, 1.0),
    Vec3::new(-1.0, 0.0, 0.0),
    Vec3::new(1.0, 0.0, 0.0),
];

/// `ItemModelGenerator` com várias camadas: as imagens ficam empilhadas numa só textura e cada
/// camada gera a própria malha (frente, verso e bordas) com a sua cor (`tintindex`).
fn generated(layers: Vec<(RgbaImage, u32)>, (display, display_tp): (Option<Display>, Option<Display>), r: &mut Renderer) -> Option<ItemModel> {
    let w = layers.first()?.0.width();
    let n = layers.len() as u32;
    let mut sheet = RgbaImage::new(w, w * n);
    let mut quads = Vec::new();
    for (i, (img, color)) in layers.into_iter().enumerate() {
        let img = if img.width() == w { img } else { image::imageops::resize(&img, w, w, image::imageops::FilterType::Nearest) };
        let img = image::imageops::crop_imm(&img, 0, 0, w, w).to_image();
        for mut q in generated_quads(&img) {
            for uv in &mut q.uv {
                uv[1] = (uv[1] + i as f32) / n as f32;
            }
            q.tint = color;
            quads.push(q);
        }
        image::imageops::replace(&mut sheet, &img, 0, (i as u32 * w) as i64);
    }
    let id = r.create_texture(sheet.width(), sheet.height(), sheet.as_raw());
    Some(ItemModel { tex: Tex::Custom(id), quads, gui3d: false, translucent: false, display, display_tp, display_ground: None, display_fixed: None, display_head: None, entity: false })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn troca_de_item_desce_e_sobe_como_no_java() {
        let mut fp = FirstPerson::default();
        let s = Slot { id: 1, count: 1, damage: 0, nbt: None, raw_nbt: Vec::new() };
        // Começa vazio: progresso 0 -> sobe 0,4 por tick até 1.
        fp.update_equipped(None, 0);
        assert_eq!(fp.equipped, 0.4);
        fp.update_equipped(None, 0);
        fp.update_equipped(None, 0);
        assert_eq!(fp.equipped, 1.0);
        // Pega um bloco: desce até < 0,1, troca, e volta a subir.
        let mut seq = Vec::new();
        for _ in 0..6 {
            fp.update_equipped(Some(&s), 0);
            seq.push(fp.equipped);
        }
        assert!((seq[0] - 0.6).abs() < 1e-6 && (seq[1] - 0.2).abs() < 1e-6, "{seq:?}");
        assert!(fp.item.is_some() && seq[5] == 1.0, "{seq:?}");
    }

    #[test]
    fn item_gerado_tem_frente_verso_e_bordas() {
        // Quadrado 2x2 opaco no meio de 16x16: cada borda vira uma tira (o Span junta a linha toda).
        let mut img = RgbaImage::new(16, 16);
        for y in 7..9 {
            for x in 7..9 {
                img.put_pixel(x, y, image::Rgba([255; 4]));
            }
        }
        let q = generated_quads(&img);
        assert_eq!(q.len(), 2 + 4);
        // Tiras de 1/16 de espessura em z (7,5 a 8,5).
        assert!(q.iter().all(|q| q.pos.iter().all(|p| p.z == 7.5 || p.z == 8.5)));
    }

    #[test]
    fn braco_parado_fica_no_canto_direito() {
        let m = arm_transform(0.0, 0.0);
        // A mão (ponta do braço, y=10 px) aparece à direita e abaixo do centro, à frente.
        let tip = m.transform_point3(Vec3::new(-5.0 - 1.0, 2.0 + 10.0, 0.0) / 16.0);
        assert!(tip.x > 0.0 && tip.y < 0.0 && tip.z < 0.0, "{tip:?}");
    }
}
