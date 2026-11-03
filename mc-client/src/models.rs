//! Modelos de entidade da 1.8: caixas com UV no esquema do `ModelBox`, partes com
//! ponto de giro e rotação Z·Y·X (como o `ModelRenderer`), e as poses do
//! `ModelBiped`/`ModelPlayer`, `ModelZombie` e `ModelVillager`.
//!
//! Coordenadas de modelo em pixels (1/16 de bloco), Y para baixo e frente em -Z,
//! como no jogo; quem chama passa a matriz da entidade já com o `scale(-1,-1,1)`.

use glam::{Mat4, Vec3};
use mc_world::mesh::{Geometry, Vertex};

/// Luz cheia (céu, bloco, AO).
pub const FULL_LIGHT: [u8; 4] = [255, 255, 255, 0];

/// Caixa de um modelo: canto, tamanho em pixels, origem da UV, inflação e espelho.
#[derive(Clone, Copy, Debug)]
pub struct Cuboid {
    pub from: Vec3,
    pub size: [u32; 3],
    pub uv: (u32, u32),
    pub inflate: f32,
    pub mirror: bool,
}

/// Uma face: 4 cantos, UV por canto (pixels da textura) e normal para fora.
pub type Face = ([Vec3; 4], [[f32; 2]; 4], Vec3);

impl Cuboid {
    /// As 6 faces na ordem do `ModelBox` (+X, -X, -Y, +Y, -Z, +Z antes do espelho).
    pub fn faces(&self) -> [Face; 6] {
        let [w, h, d] = self.size;
        let (u, v) = self.uv;
        let (mut x1, y1, z1) = (self.from.x - self.inflate, self.from.y - self.inflate, self.from.z - self.inflate);
        let (mut x2, y2, z2) = (
            self.from.x + w as f32 + self.inflate,
            self.from.y + h as f32 + self.inflate,
            self.from.z + d as f32 + self.inflate,
        );
        if self.mirror {
            std::mem::swap(&mut x1, &mut x2);
        }
        let p = Vec3::new;
        let (a, b, c, e) = (p(x1, y1, z1), p(x2, y1, z1), p(x2, y2, z1), p(x1, y2, z1));
        let (f, g, k, l) = (p(x1, y1, z2), p(x2, y1, z2), p(x2, y2, z2), p(x1, y2, z2));
        // Retângulo (u1, v1, u2, v2): o canto 0 recebe (u2, v1), 1 (u1, v1), 2 (u1, v2), 3 (u2, v2).
        let uv = |u1: u32, v1: u32, u2: u32, v2: u32| {
            let (u1, v1, u2, v2) = (u1 as f32, v1 as f32, u2 as f32, v2 as f32);
            [[u2, v1], [u1, v1], [u1, v2], [u2, v2]]
        };
        let sx = if self.mirror { -1.0 } else { 1.0 };
        [
            ([g, b, c, k], uv(u + d + w, v + d, u + d + w + d, v + d + h), Vec3::X * sx),
            ([a, f, l, e], uv(u, v + d, u + d, v + d + h), -Vec3::X * sx),
            ([g, f, a, b], uv(u + d, v, u + d + w, v + d), -Vec3::Y),
            ([c, e, l, k], uv(u + d + w, v + d, u + d + w + w, v), Vec3::Y),
            ([b, a, e, c], uv(u + d, v + d, u + d + w, v + d + h), -Vec3::Z),
            ([f, g, k, l], uv(u + d + w + d, v + d, u + d + w + d + w, v + d + h), Vec3::Z),
        ]
    }
}

/// Parte de um modelo (`ModelRenderer`).
#[derive(Clone, Debug)]
pub struct Part {
    pub cubes: Vec<Cuboid>,
    pub pivot: Vec3,
    /// Radianos, aplicados na ordem Z, Y, X.
    pub rot: Vec3,
    pub show: bool,
    pub children: Vec<Part>,
}

impl Part {
    pub fn new(pivot: [f32; 3]) -> Self {
        Self { cubes: Vec::new(), pivot: Vec3::from(pivot), rot: Vec3::ZERO, show: true, children: Vec::new() }
    }

    pub fn cube(mut self, uv: (u32, u32), from: [f32; 3], size: [u32; 3], inflate: f32, mirror: bool) -> Self {
        self.cubes.push(Cuboid { from: Vec3::from(from), size, uv, inflate, mirror });
        self
    }

    /// Copia ângulos e ponto de giro (`ModelBase.copyModelAngles`).
    pub fn copy_pose(&mut self, from: &Part) {
        self.rot = from.rot;
        self.pivot = from.pivot;
    }

    pub fn matrix(&self, parent: &Mat4) -> Mat4 {
        *parent
            * Mat4::from_translation(self.pivot / 16.0)
            * Mat4::from_rotation_z(self.rot.z)
            * Mat4::from_rotation_y(self.rot.y)
            * Mat4::from_rotation_x(self.rot.x)
    }

    #[cfg(test)]
    pub fn quad_count(&self) -> usize {
        if !self.show {
            return 0;
        }
        self.cubes.len() * 6 + self.children.iter().map(Part::quad_count).sum::<usize>()
    }
}

/// Destino dos quadriláteros: textura de `tex` pixels, tinta sRGB e luz da entidade.
pub struct Sink<'a> {
    pub geo: &'a mut Geometry,
    pub tex: (f32, f32),
    pub tint: [f32; 3],
    pub alpha: u8,
    pub light: [u8; 4],
    /// Direções das duas luzes (`RenderHelper`), já no espaço dos vértices.
    pub lights: [Vec3; 2],
}

/// As luzes do `RenderHelper.enableStandardLighting` fixas no mundo.
pub const WORLD_LIGHTS: [Vec3; 2] = [Vec3::new(0.2, 1.0, -0.7), Vec3::new(-0.2, 1.0, 0.7)];

/// Luz direcional das entidades (`RenderHelper`): duas luzes fixas no mundo, difusa 0.6 e ambiente 0.4.
pub fn diffuse(n: Vec3) -> f32 {
    diffuse_with(n, &WORLD_LIGHTS)
}

/// Difusa 0.6 por luz + ambiente 0.4 (`GL_LIGHTING` com `colorMaterial`), limitada a 1.
pub fn diffuse_with(n: Vec3, lights: &[Vec3; 2]) -> f32 {
    diffuse_norm(n, &lights.map(Vec3::normalize))
}

/// `diffuse_with` com as direções das luzes já normalizadas.
fn diffuse_norm(n: Vec3, l: &[Vec3; 2]) -> f32 {
    (0.4 + 0.6 * (n.dot(l[0]).max(0.0) + n.dot(l[1]).max(0.0))).min(1.0)
}

impl Sink<'_> {
    pub fn part(&mut self, part: &Part, parent: &Mat4) {
        if !part.show {
            return;
        }
        let m = part.matrix(parent);
        // As luzes não mudam entre as faces: normalizadas uma vez por parte, não a cada face.
        let lights = self.lights.map(Vec3::normalize);
        for c in &part.cubes {
            self.cube(c, &m, &lights);
        }
        for child in &part.children {
            self.part(child, &m);
        }
    }

    /// As 6 faces de uma caixa na matriz `m` da parte; `lights` são as luzes já normalizadas.
    pub(crate) fn cube(&mut self, c: &Cuboid, m: &Mat4, lights: &[Vec3; 2]) {
        for (corners, uv, normal) in c.faces() {
            let n = m.transform_vector3(normal).normalize_or_zero();
            let k = diffuse_norm(n, lights);
            let color = [
                (self.tint[0] * k * 255.0) as u8,
                (self.tint[1] * k * 255.0) as u8,
                (self.tint[2] * k * 255.0) as u8,
                self.alpha,
            ];
            let vert = |i: usize| Vertex {
                pos: m.transform_point3(corners[i] / 16.0).to_array(),
                uv: [uv[i][0] / self.tex.0, uv[i][1] / self.tex.1],
                color,
                light: self.light,
            };
            self.geo.quad([vert(0), vert(1), vert(2), vert(3)]);
        }
    }

    /// Caixa simples em coordenadas de bloco (mobs sem modelo, itens), textura inteira em cada face.
    pub fn block_box(&mut self, m: &Mat4, min: Vec3, max: Vec3) {
        let c = Cuboid { from: min * 16.0, size: [1, 1, 1], uv: (0, 0), inflate: 0.0, mirror: false };
        let size = (max - min) * 16.0;
        for (corners, _, normal) in c.faces() {
            let n = m.transform_vector3(normal).normalize_or_zero();
            let k = diffuse(n);
            let color = [(self.tint[0] * k * 255.0) as u8, (self.tint[1] * k * 255.0) as u8, (self.tint[2] * k * 255.0) as u8, self.alpha];
            let uv = [[1.0, 0.0], [0.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
            let vert = |i: usize| {
                // Estica a caixa unitária até o tamanho pedido.
                let local = min * 16.0 + (corners[i] - min * 16.0) * size;
                Vertex { pos: m.transform_point3(local / 16.0).to_array(), uv: uv[i], color, light: self.light }
            };
            self.geo.quad([vert(0), vert(1), vert(2), vert(3)]);
        }
    }
}

/// Parâmetros de animação de um quadro (os argumentos de `setRotationAngles`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Anim {
    pub limb_swing: f32,
    pub limb_amount: f32,
    /// Ticks de vida + fração do tick.
    pub age: f32,
    /// Graus, relativo ao corpo.
    pub head_yaw: f32,
    /// Graus.
    pub head_pitch: f32,
    /// 0..1 (`getSwingProgress`).
    pub swing: f32,
    pub sneak: bool,
    pub riding: bool,
    /// 0 = mão vazia, 1 = segurando, 3 = defendendo.
    pub held_right: i32,
    pub aimed_bow: bool,
    pub child: bool,
}

/// `ModelBiped` com as camadas externas do `ModelPlayer` quando `wear` existe.
#[derive(Clone, Debug)]
pub struct Biped {
    pub head: Part,
    pub hat: Part,
    pub body: Part,
    pub right_arm: Part,
    pub left_arm: Part,
    pub right_leg: Part,
    pub left_leg: Part,
    /// Jaqueta, manga esquerda, manga direita, calça esquerda, calça direita.
    pub wear: Option<Box<[Part; 5]>>,
    pub tex: (f32, f32),
}

/// Bits do metadado 10 do jogador (`EnumPlayerModelParts`).
pub mod skin_parts {
    pub const JACKET: u8 = 0x02;
    pub const LEFT_SLEEVE: u8 = 0x04;
    pub const RIGHT_SLEEVE: u8 = 0x08;
    pub const LEFT_PANTS: u8 = 0x10;
    pub const RIGHT_PANTS: u8 = 0x20;
    pub const HAT: u8 = 0x40;
}

impl Biped {
    /// `ModelBiped(0, 0, w, h)`.
    pub fn new(tex: (u32, u32)) -> Self {
        Self::sized(0.0, tex)
    }

    /// `ModelBiped(modelSize, 0, w, h)`: todas as caixas infladas por `s` (o chapéu por `s + 0.5`).
    pub fn sized(s: f32, tex: (u32, u32)) -> Self {
        Self {
            head: Part::new([0.0, 0.0, 0.0]).cube((0, 0), [-4.0, -8.0, -4.0], [8, 8, 8], s, false),
            hat: Part::new([0.0, 0.0, 0.0]).cube((32, 0), [-4.0, -8.0, -4.0], [8, 8, 8], s + 0.5, false),
            body: Part::new([0.0, 0.0, 0.0]).cube((16, 16), [-4.0, 0.0, -2.0], [8, 12, 4], s, false),
            right_arm: Part::new([-5.0, 2.0, 0.0]).cube((40, 16), [-3.0, -2.0, -2.0], [4, 12, 4], s, false),
            left_arm: Part::new([5.0, 2.0, 0.0]).cube((40, 16), [-1.0, -2.0, -2.0], [4, 12, 4], s, true),
            right_leg: Part::new([-1.9, 12.0, 0.0]).cube((0, 16), [-2.0, 0.0, -2.0], [4, 12, 4], s, false),
            left_leg: Part::new([1.9, 12.0, 0.0]).cube((0, 16), [-2.0, 0.0, -2.0], [4, 12, 4], s, true),
            wear: None,
            tex: (tex.0 as f32, tex.1 as f32),
        }
    }

    /// `LayerBipedArmor`: `ModelBiped(0.5)` na calça, `ModelBiped(1.0)` nas outras peças, só as partes da peça.
    pub fn armor(armor_type: u8) -> Self {
        let mut m = Self::sized(if armor_type == 2 { 0.5 } else { 1.0 }, (64, 32));
        for p in [&mut m.head, &mut m.hat, &mut m.body, &mut m.right_arm, &mut m.left_arm, &mut m.right_leg, &mut m.left_leg] {
            p.show = false;
        }
        match armor_type {
            0 => {
                m.head.show = true;
                m.hat.show = true;
            }
            1 => {
                m.body.show = true;
                m.right_arm.show = true;
                m.left_arm.show = true;
            }
            2 => {
                m.body.show = true;
                m.right_leg.show = true;
                m.left_leg.show = true;
            }
            _ => {
                m.right_leg.show = true;
                m.left_leg.show = true;
            }
        }
        m
    }

    /// `ModelPlayer(0, slim)`: textura 64x64, braço e perna esquerdos próprios e camadas externas.
    pub fn player(slim: bool) -> Self {
        let mut m = Self::new((64, 64));
        let aw = if slim { 3 } else { 4 };
        let ay = if slim { 2.5 } else { 2.0 };
        m.left_arm = Part::new([5.0, ay, 0.0]).cube((32, 48), [-1.0, -2.0, -2.0], [aw, 12, 4], 0.0, false);
        let left_sleeve = Part::new([5.0, ay, 0.0]).cube((48, 48), [-1.0, -2.0, -2.0], [aw, 12, 4], 0.25, false);
        let right_x = if slim { -2.0 } else { -3.0 };
        if slim {
            m.right_arm = Part::new([-5.0, 2.5, 0.0]).cube((40, 16), [-2.0, -2.0, -2.0], [3, 12, 4], 0.0, false);
        }
        let right_sleeve = Part::new([-5.0, ay, 10.0]).cube((40, 32), [right_x, -2.0, -2.0], [aw, 12, 4], 0.25, false);
        m.left_leg = Part::new([1.9, 12.0, 0.0]).cube((16, 48), [-2.0, 0.0, -2.0], [4, 12, 4], 0.0, false);
        let left_pants = Part::new([1.9, 12.0, 0.0]).cube((0, 48), [-2.0, 0.0, -2.0], [4, 12, 4], 0.25, false);
        let right_pants = Part::new([-1.9, 12.0, 0.0]).cube((0, 32), [-2.0, 0.0, -2.0], [4, 12, 4], 0.25, false);
        let jacket = Part::new([0.0, 0.0, 0.0]).cube((16, 32), [-4.0, 0.0, -2.0], [8, 12, 4], 0.25, false);
        m.wear = Some(Box::new([jacket, left_sleeve, right_sleeve, left_pants, right_pants]));
        m
    }

    /// `ModelZombie()`: o biped com textura 64x64 (zumbi e porco-zumbi).
    pub fn zombie() -> Self {
        Self::new((64, 64))
    }

    /// `ModelZombieVillager()`: zumbi com a cabeça comprida e o nariz do aldeão.
    pub fn zombie_villager() -> Self {
        let mut m = Self::new((64, 64));
        m.head = Part::new([0.0, 0.0, 0.0])
            .cube((0, 32), [-4.0, -10.0, -4.0], [8, 10, 8], 0.0, false)
            .cube((24, 32), [-1.0, -3.0, -6.0], [2, 4, 2], 0.0, false);
        m
    }

    /// `ModelSkeleton()`: braços e pernas de 2x12x2, textura 64x32.
    pub fn skeleton() -> Self {
        let mut m = Self::new((64, 32));
        m.right_arm = Part::new([-5.0, 2.0, 0.0]).cube((40, 16), [-1.0, -2.0, -1.0], [2, 12, 2], 0.0, false);
        m.left_arm = Part::new([5.0, 2.0, 0.0]).cube((40, 16), [-1.0, -2.0, -1.0], [2, 12, 2], 0.0, true);
        m.right_leg = Part::new([-2.0, 12.0, 0.0]).cube((0, 16), [-1.0, 0.0, -1.0], [2, 12, 2], 0.0, false);
        m.left_leg = Part::new([2.0, 12.0, 0.0]).cube((0, 16), [-1.0, 0.0, -1.0], [2, 12, 2], 0.0, true);
        m
    }

    /// Liga as camadas externas pelos bits do metadado 10.
    pub fn set_skin_parts(&mut self, bits: u8) {
        use skin_parts::*;
        self.hat.show = bits & HAT != 0;
        if let Some(w) = &mut self.wear {
            for (part, bit) in w.iter_mut().zip([JACKET, LEFT_SLEEVE, RIGHT_SLEEVE, LEFT_PANTS, RIGHT_PANTS]) {
                part.show = bits & bit != 0;
            }
        }
    }

    /// `ModelBiped.setRotationAngles` (+ cópia para as camadas do `ModelPlayer`), com o `MathHelper` do jogo.
    pub fn pose(&mut self, a: &Anim) {
        use crate::mth::{cos, sin, PI};
        let (ls, la, age) = (a.limb_swing, a.limb_amount, a.age);
        self.head.rot.y = a.head_yaw / 57.295776;
        self.head.rot.x = a.head_pitch / 57.295776;
        self.right_arm.rot.x = cos(ls * 0.6662 + PI) * 2.0 * la * 0.5;
        self.left_arm.rot.x = cos(ls * 0.6662) * 2.0 * la * 0.5;
        self.right_arm.rot.z = 0.0;
        self.left_arm.rot.z = 0.0;
        self.right_leg.rot.x = cos(ls * 0.6662) * 1.4 * la;
        self.left_leg.rot.x = cos(ls * 0.6662 + PI) * 1.4 * la;
        self.right_leg.rot.y = 0.0;
        self.left_leg.rot.y = 0.0;
        if a.riding {
            self.right_arm.rot.x += -0.62831855;
            self.left_arm.rot.x += -0.62831855;
            self.right_leg.rot.x = -1.2566371;
            self.left_leg.rot.x = -1.2566371;
            self.right_leg.rot.y = 0.31415927;
            self.left_leg.rot.y = -0.31415927;
        }
        self.right_arm.rot.y = 0.0;
        self.right_arm.rot.z = 0.0;
        if matches!(a.held_right, 1 | 3) {
            self.right_arm.rot.x = self.right_arm.rot.x * 0.5 - 0.31415927 * a.held_right as f32;
            if a.held_right == 3 {
                self.right_arm.rot.y = -0.5235988;
            }
        }
        self.left_arm.rot.y = 0.0;

        // Soco: o corpo gira e o braço direito desce em arco.
        let s = a.swing;
        self.body.rot.y = sin(s.sqrt() * PI * 2.0) * 0.2;
        let by = self.body.rot.y;
        self.right_arm.pivot.z = sin(by) * 5.0;
        self.right_arm.pivot.x = -cos(by) * 5.0;
        self.left_arm.pivot.z = -sin(by) * 5.0;
        self.left_arm.pivot.x = cos(by) * 5.0;
        self.right_arm.rot.y += by;
        self.left_arm.rot.y += by;
        self.left_arm.rot.x += by;
        let mut f = 1.0 - s;
        f *= f;
        f *= f;
        f = 1.0 - f;
        let g = sin(f * PI);
        let h = sin(s * PI) * -(self.head.rot.x - 0.7) * 0.75;
        self.right_arm.rot.x = (self.right_arm.rot.x as f64 - (g as f64 * 1.2 + h as f64)) as f32;
        self.right_arm.rot.y += by * 2.0;
        self.right_arm.rot.z += sin(s * PI) * -0.4;

        if a.sneak {
            self.body.rot.x = 0.5;
            self.right_arm.rot.x += 0.4;
            self.left_arm.rot.x += 0.4;
            self.right_leg.pivot.z = 4.0;
            self.left_leg.pivot.z = 4.0;
            self.right_leg.pivot.y = 9.0;
            self.left_leg.pivot.y = 9.0;
            self.head.pivot.y = 1.0;
        } else {
            self.body.rot.x = 0.0;
            self.right_leg.pivot.z = 0.1;
            self.left_leg.pivot.z = 0.1;
            self.right_leg.pivot.y = 12.0;
            self.left_leg.pivot.y = 12.0;
            self.head.pivot.y = 0.0;
        }

        // Respiração dos braços.
        self.idle_arms(age);
        if a.aimed_bow {
            self.right_arm.rot.z = 0.0;
            self.left_arm.rot.z = 0.0;
            self.right_arm.rot.y = -0.1 + self.head.rot.y;
            self.left_arm.rot.y = 0.1 + self.head.rot.y + 0.4;
            self.right_arm.rot.x = -1.5707964 + self.head.rot.x;
            self.left_arm.rot.x = -1.5707964 + self.head.rot.x;
            self.idle_arms(age);
        }
        self.hat.copy_pose(&self.head);
        if let Some(w) = &mut self.wear {
            w[0].copy_pose(&self.body);
            w[1].copy_pose(&self.left_arm);
            w[2].copy_pose(&self.right_arm);
            w[3].copy_pose(&self.left_leg);
            w[4].copy_pose(&self.right_leg);
        }
    }

    fn idle_arms(&mut self, age: f32) {
        use crate::mth::{cos, sin};
        self.right_arm.rot.z += cos(age * 0.09) * 0.05 + 0.05;
        self.left_arm.rot.z -= cos(age * 0.09) * 0.05 + 0.05;
        self.right_arm.rot.x += sin(age * 0.067) * 0.05;
        self.left_arm.rot.x -= sin(age * 0.067) * 0.05;
    }

    /// `ModelZombie.setRotationAngles`: braços esticados para a frente.
    pub fn zombie_arms(&mut self, a: &Anim) {
        use crate::mth::{sin, PI};
        let f = sin(a.swing * PI);
        let g = sin((1.0 - (1.0 - a.swing) * (1.0 - a.swing)) * PI);
        self.right_arm.rot.z = 0.0;
        self.left_arm.rot.z = 0.0;
        self.right_arm.rot.y = -(0.1 - f * 0.6);
        self.left_arm.rot.y = 0.1 - f * 0.6;
        self.right_arm.rot.x = -1.5707964;
        self.left_arm.rot.x = -1.5707964;
        self.right_arm.rot.x -= f * 1.2 - g * 0.4;
        self.left_arm.rot.x -= f * 1.2 - g * 0.4;
        self.idle_arms(a.age);
    }

    /// Mostra só a cabeça e o chapéu (espectador).
    pub fn head_only(&mut self) {
        for p in [&mut self.body, &mut self.right_arm, &mut self.left_arm, &mut self.right_leg, &mut self.left_leg] {
            p.show = false;
        }
        if let Some(w) = &mut self.wear {
            w.iter_mut().for_each(|p| p.show = false);
        }
    }

    #[cfg(test)]
    pub fn quad_count(&self) -> usize {
        let base = [&self.head, &self.hat, &self.body, &self.right_arm, &self.left_arm, &self.right_leg, &self.left_leg];
        base.iter().map(|p| p.quad_count()).sum::<usize>() + self.wear.as_ref().map_or(0, |w| w.iter().map(Part::quad_count).sum())
    }

    /// `ModelBiped.render` (+ `ModelPlayer.render`): filhote com cabeça maior, agachado desce 0.2.
    pub fn emit(&self, sink: &mut Sink, m: &Mat4, a: &Anim) {
        sink.tex = self.tex;
        let body_parts = [&self.body, &self.right_arm, &self.left_arm, &self.right_leg, &self.left_leg];
        if a.child {
            let head = *m * Mat4::from_scale(Vec3::splat(0.75)) * Mat4::from_translation(Vec3::new(0.0, 1.0, 0.0));
            sink.part(&self.head, &head);
            let body = *m * Mat4::from_scale(Vec3::splat(0.5)) * Mat4::from_translation(Vec3::new(0.0, 1.5, 0.0));
            body_parts.iter().for_each(|p| sink.part(p, &body));
            sink.part(&self.hat, &body);
            if let Some(w) = &self.wear {
                w.iter().for_each(|p| sink.part(p, &body));
            }
            return;
        }
        let m = if a.sneak { *m * Mat4::from_translation(Vec3::new(0.0, 0.2, 0.0)) } else { *m };
        sink.part(&self.head, &m);
        body_parts.iter().for_each(|p| sink.part(p, &m));
        sink.part(&self.hat, &m);
        if let Some(w) = &self.wear {
            // Ordem do jogo: calças, mangas, jaqueta.
            for i in [3, 4, 1, 2, 0] {
                sink.part(&w[i], &m);
            }
        }
    }
}

/// `ModelVillager(0)`: cabeça com nariz, túnica, braços cruzados e pernas; textura 64x64.
#[derive(Clone, Debug)]
pub struct Villager {
    pub head: Part,
    pub body: Part,
    pub arms: Part,
    pub right_leg: Part,
    pub left_leg: Part,
}

impl Default for Villager {
    fn default() -> Self {
        let nose = Part::new([0.0, -2.0, 0.0]).cube((24, 0), [-1.0, -1.0, -6.0], [2, 4, 2], 0.0, false);
        let mut head = Part::new([0.0, 0.0, 0.0]).cube((0, 0), [-4.0, -10.0, -4.0], [8, 10, 8], 0.0, false);
        head.children.push(nose);
        Self {
            head,
            body: Part::new([0.0, 0.0, 0.0])
                .cube((16, 20), [-4.0, 0.0, -3.0], [8, 12, 6], 0.0, false)
                .cube((0, 38), [-4.0, 0.0, -3.0], [8, 18, 6], 0.5, false),
            arms: Part::new([0.0, 2.0, 0.0])
                .cube((44, 22), [-8.0, -2.0, -2.0], [4, 8, 4], 0.0, false)
                .cube((44, 22), [4.0, -2.0, -2.0], [4, 8, 4], 0.0, false)
                .cube((40, 38), [-4.0, 2.0, -2.0], [8, 4, 4], 0.0, false),
            right_leg: Part::new([-2.0, 12.0, 0.0]).cube((0, 22), [-2.0, 0.0, -2.0], [4, 12, 4], 0.0, false),
            left_leg: Part::new([2.0, 12.0, 0.0]).cube((0, 22), [-2.0, 0.0, -2.0], [4, 12, 4], 0.0, true),
        }
    }
}

impl Villager {
    pub fn pose(&mut self, a: &Anim) {
        use crate::mth::{cos, PI};
        self.head.rot.y = a.head_yaw / 57.295776;
        self.head.rot.x = a.head_pitch / 57.295776;
        self.arms.pivot.y = 3.0;
        self.arms.pivot.z = -1.0;
        self.arms.rot.x = -0.75;
        self.right_leg.rot.x = cos(a.limb_swing * 0.6662) * 1.4 * a.limb_amount * 0.5;
        self.left_leg.rot.x = cos(a.limb_swing * 0.6662 + PI) * 1.4 * a.limb_amount * 0.5;
        self.right_leg.rot.y = 0.0;
        self.left_leg.rot.y = 0.0;
    }

    pub fn emit(&self, sink: &mut Sink, m: &Mat4) {
        sink.tex = (64.0, 64.0);
        for p in [&self.head, &self.body, &self.right_leg, &self.left_leg, &self.arms] {
            sink.part(p, m);
        }
    }

    #[cfg(test)]
    pub fn quad_count(&self) -> usize {
        [&self.head, &self.body, &self.arms, &self.right_leg, &self.left_leg].iter().map(|p| p.quad_count()).sum()
    }
}

/// Matriz de `RendererLivingEntity.doRender` até o modelo: posição, giro do corpo,
/// tombo da morte, `scale(-1,-1,1)`, escala do `preRenderCallback` e o -1.5078125.
pub fn living_matrix(pos: Vec3, body_yaw: f32, death_tilt_deg: f32, scale: f32) -> Mat4 {
    Mat4::from_translation(pos)
        * Mat4::from_rotation_y((180.0 - body_yaw).to_radians())
        * Mat4::from_rotation_z(death_tilt_deg.to_radians())
        * Mat4::from_scale(Vec3::new(-scale, -scale, scale))
        * Mat4::from_translation(Vec3::new(0.0, -1.5078125, 0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sink_geo(f: impl FnOnce(&mut Sink)) -> Geometry {
        let mut geo = Geometry::default();
        let mut s = Sink { geo: &mut geo, tex: (64.0, 64.0), tint: [1.0; 3], alpha: 255, light: FULL_LIGHT, lights: WORLD_LIGHTS };
        f(&mut s);
        geo
    }

    #[test]
    fn head_front_face_uv() {
        let head = Biped::player(false).head;
        let faces = head.cubes[0].faces();
        // Frente (-Z): u 8..16, v 8..16 no esquema da skin.
        let (_, uv, n) = faces[4];
        assert_eq!(n, -Vec3::Z);
        assert_eq!(uv, [[16.0, 8.0], [8.0, 8.0], [8.0, 16.0], [16.0, 16.0]]);
        // Topo (-Y no modelo): u 8..16, v 0..8; base: u 16..24 com v invertido.
        assert_eq!(faces[2].1, [[16.0, 0.0], [8.0, 0.0], [8.0, 8.0], [16.0, 8.0]]);
        assert_eq!(faces[3].1, [[24.0, 8.0], [16.0, 8.0], [16.0, 0.0], [24.0, 0.0]]);
        // Lados e nuca.
        assert_eq!(faces[1].1[1], [0.0, 8.0]);
        assert_eq!(faces[0].1[1], [16.0, 8.0]);
        assert_eq!(faces[5].1[1], [24.0, 8.0]);
        // Chapéu: mesma caixa inflada 0.5 a partir de u=32.
        let hat = Biped::player(false).hat;
        assert_eq!(hat.cubes[0].faces()[4].1[1], [40.0, 8.0]);
        assert_eq!(hat.cubes[0].faces()[4].0[1], Vec3::new(-4.5, -8.5, -4.5));
    }

    #[test]
    fn mirror_swaps_x() {
        let c = Cuboid { from: Vec3::ZERO, size: [4, 12, 4], uv: (40, 16), inflate: 0.0, mirror: true };
        let f = c.faces();
        assert!(f[0].0.iter().all(|p| p.x == 0.0));
        assert_eq!(f[0].2, -Vec3::X);
        assert!(f[1].0.iter().all(|p| p.x == 4.0));
    }

    #[test]
    fn quad_counts() {
        // Jogador: 7 partes do biped + 5 camadas, uma caixa cada.
        let mut p = Biped::player(false);
        assert_eq!(p.quad_count(), 12 * 6);
        p.set_skin_parts(0);
        assert_eq!(p.quad_count(), 6 * 6);
        p.set_skin_parts(0x7F);
        assert_eq!(p.quad_count(), 12 * 6);
        let g = sink_geo(|s| p.emit(s, &Mat4::IDENTITY, &Anim::default()));
        assert_eq!(g.vertices.len(), 12 * 6 * 4);
        assert_eq!(g.indices.len(), 12 * 6 * 6);
        assert_eq!(Biped::zombie().quad_count(), 7 * 6);
        assert_eq!(Villager::default().quad_count(), (1 + 1 + 2 + 3 + 1 + 1) * 6);
        p.head_only();
        assert_eq!(p.quad_count(), 2 * 6);
    }

    #[test]
    fn slim_arms_are_three_wide() {
        let p = Biped::player(true);
        assert_eq!(p.left_arm.cubes[0].size, [3, 12, 4]);
        assert_eq!(p.right_arm.cubes[0].size, [3, 12, 4]);
        assert_eq!(p.left_arm.pivot.y, 2.5);
        assert_eq!(p.left_arm.cubes[0].uv, (32, 48));
        let w = p.wear.unwrap();
        assert_eq!(w[1].cubes[0].uv, (48, 48));
        assert_eq!(w[2].cubes[0].from.x, -2.0);
    }

    #[test]
    fn standing_player_fits_1_8_blocks() {
        let p = Biped::player(false);
        let m = living_matrix(Vec3::ZERO, 0.0, 0.0, 0.9375);
        let g = sink_geo(|s| p.emit(s, &m, &Anim::default()));
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for v in &g.vertices {
            lo = lo.min(v.pos[1]);
            hi = hi.max(v.pos[1]);
        }
        // Pés quase no chão (a camada de 0.25 px passa um pouco) e cabeça perto de 1.8 * 0.9375 + chapéu.
        assert!(lo.abs() < 0.03, "pés em {lo}");
        assert!((hi - (1.5078125 + 0.5 + 0.5 / 16.0) * 0.9375).abs() < 0.01, "topo em {hi}");
    }

    #[test]
    fn facing_follows_yaw() {
        // Yaw 0 olha para +Z: o nariz do aldeão (frente, -Z do modelo) fica em +Z no mundo.
        let v = Villager::default();
        let m = living_matrix(Vec3::ZERO, 0.0, 0.0, 1.0);
        let g = sink_geo(|s| s.part(&v.head, &m));
        let max_z = g.vertices.iter().map(|v| v.pos[2]).fold(f32::MIN, f32::max);
        assert!((max_z - 6.0 / 16.0).abs() < 1e-4, "nariz em z={max_z}");
        // Yaw 90 olha para -X.
        let m = living_matrix(Vec3::ZERO, 90.0, 0.0, 1.0);
        let g = sink_geo(|s| s.part(&v.head, &m));
        let min_x = g.vertices.iter().map(|v| v.pos[0]).fold(f32::MAX, f32::min);
        assert!((min_x + 6.0 / 16.0).abs() < 1e-4, "nariz em x={min_x}");
    }

    #[test]
    fn walking_swings_opposite_limbs() {
        let mut p = Biped::player(false);
        let a = Anim { limb_swing: 1.0, limb_amount: 1.0, ..Default::default() };
        p.pose(&a);
        assert!(p.right_leg.rot.x * p.left_leg.rot.x < 0.0);
        // MathHelper.cos (tabela), como o jogo.
        assert_eq!(p.right_leg.rot.x, crate::mth::cos(0.6662) * 1.4);
        assert_eq!(p.wear.as_ref().unwrap()[4].rot, p.right_leg.rot);
        // Soco no meio: o corpo gira e o braço direito sobe.
        let mut q = Biped::player(false);
        q.pose(&Anim { swing: 0.5, ..Default::default() });
        assert!(q.right_arm.rot.x < -0.5);
    }

    #[test]
    fn diffuse_is_bright_on_top() {
        assert!((diffuse(Vec3::Y) - 1.0).abs() < 1e-6);
        assert!((diffuse(-Vec3::Y) - 0.4).abs() < 1e-6);
    }
}
