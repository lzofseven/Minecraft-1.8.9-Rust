//! Câmera do `EntityRenderer`: FOV com o modificador de corrida/voo
//! (`updateFovModifierHand`), dano (`hurtCameraEffect`), balanço ao andar
//! (`setupViewBobbing`), primeira/terceira pessoa (`orientCamera`) e a projeção da mão.

use glam::{DVec3, Mat4, Vec3};
use mc_world::blocks::BlockTable;
use mc_world::World;

use crate::block_physics::{BLOCKS, MATERIALS, M_WATER};
use crate::interact;
use crate::mth;
use crate::physics::Player;

/// `GL rotate(ângulo em graus, eixo)`.
pub fn rot(deg: f32, x: f32, y: f32, z: f32) -> Mat4 {
    Mat4::from_axis_angle(Vec3::new(x, y, z).normalize(), deg.to_radians())
}

pub fn tr(x: f32, y: f32, z: f32) -> Mat4 {
    Mat4::from_translation(Vec3::new(x, y, z))
}

pub struct CameraState {
    /// OptiFine `ofDynamicFov`.
    pub dynamic_fov: bool,
    /// Altura dos olhos da entidade da câmera quando não é o próprio jogador (S43).
    pub eye_override: Option<f32>,
    pub fov_hand: f32,
    pub fov_hand_prev: f32,
    /// `gameSettings.thirdPersonView`: 0 primeira pessoa, 1 de costas, 2 de frente.
    pub third_person: u8,
    /// `thirdPersonDistance` (fixo em 4 na 1.8).
    pub distance: f32,
    /// `gameSettings.fovSetting` e `viewBobbing` (padrões do options.txt).
    pub fov_setting: f32,
    pub view_bobbing: bool,
    /// `rendererUpdateCount` (sobe um por tick).
    pub ticks: u32,
    /// `prevTimeInPortal`/`timeInPortal` do jogador e náusea ativa (distorção do portal e da náusea).
    pub portal: (f32, f32),
    pub confusion: bool,
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            dynamic_fov: true,
            eye_override: None,
            fov_hand: 1.0,
            fov_hand_prev: 1.0,
            third_person: 0,
            distance: 4.0,
            fov_setting: 70.0,
            view_bobbing: true,
            ticks: 0,
            portal: (0.0, 0.0),
            confusion: false,
        }
    }
}

/// Resultado de `setupCameraTransform` para um quadro.
pub struct View {
    pub view: Mat4,
    /// Posição real da câmera (para neblina, ordenação e culling).
    pub eye: DVec3,
    pub fov: f32,
    pub hand_fov: f32,
    /// `hurtCameraEffect` + `setupViewBobbing` (a base da mão).
    pub hand_base: Mat4,
}

impl CameraState {
    /// `updateFovModifierHand` (no começo do tick, antes do jogador andar).
    pub fn tick(&mut self, p: &Player) {
        self.ticks = self.ticks.wrapping_add(1);
        let f = if self.dynamic_fov { p.fov_modifier() } else { 1.0 };
        self.fov_hand_prev = self.fov_hand;
        self.fov_hand += (f - self.fov_hand) * 0.5;
        self.fov_hand = self.fov_hand.clamp(0.1, 1.5);
    }

    /// F5: primeira -> costas -> frente.
    pub fn cycle(&mut self) {
        self.third_person = (self.third_person + 1) % 3;
    }

    /// `hurtCameraEffect`.
    fn hurt(p: &Player, partial: f32) -> Mat4 {
        let mut f = p.hurt_time as f32 - partial;
        if f < 0.0 || p.max_hurt_time == 0 {
            return Mat4::IDENTITY;
        }
        f /= p.max_hurt_time as f32;
        f = mth::sin(f * f * f * f * mth::PI);
        let y = p.attacked_at_yaw;
        rot(-y, 0.0, 1.0, 0.0) * rot(-f * 14.0, 0.0, 0.0, 1.0) * rot(y, 0.0, 1.0, 0.0)
    }

    /// `setupViewBobbing`.
    fn bobbing(p: &Player, partial: f32) -> Mat4 {
        let f = p.distance_walked - p.prev_distance_walked;
        let f1 = -(p.distance_walked + f * partial);
        let f2 = p.prev_camera_yaw + (p.camera_yaw - p.prev_camera_yaw) * partial;
        let f3 = p.prev_camera_pitch + (p.camera_pitch - p.prev_camera_pitch) * partial;
        tr(mth::sin(f1 * mth::PI) * f2 * 0.5, -(mth::cos(f1 * mth::PI) * f2).abs(), 0.0)
            * rot(mth::sin(f1 * mth::PI) * f2 * 3.0, 0.0, 0.0, 1.0)
            * rot((mth::cos(f1 * mth::PI - 0.2) * f2).abs() * 5.0, 1.0, 0.0, 0.0)
            * rot(f3, 1.0, 0.0, 0.0)
    }

    /// Distorção do portal e da náusea (`setupCameraTransform`, depois do balanço e antes do `orientCamera`).
    pub fn warp(&self, partial: f32) -> Mat4 {
        let f1 = self.portal.0 + (self.portal.1 - self.portal.0) * partial;
        if f1 <= 0.0 {
            return Mat4::IDENTITY;
        }
        let i = if self.confusion { 7.0 } else { 20.0 };
        let mut f2 = 5.0 / (f1 * f1 + 5.0) - f1 * 0.04;
        f2 *= f2;
        let a = (self.ticks as f32 + partial) * i;
        rot(a, 0.0, 1.0, 1.0) * Mat4::from_scale(Vec3::new(1.0 / f2, 1.0, 1.0)) * rot(-a, 0.0, 1.0, 1.0)
    }

    /// `setupCameraTransform` + `orientCamera`.
    pub fn view(&self, p: &Player, w: &World, t: &BlockTable, partial: f32) -> View {
        let feet = p.prev + (p.pos - p.prev) * partial as f64;
        let eye_h = self.eye_override.unwrap_or(p.eye_height() as f32);
        let eye = feet + DVec3::new(0.0, eye_h as f64, 0.0);
        let mut base = Self::hurt(p, partial);
        if self.view_bobbing {
            base *= Self::bobbing(p, partial);
        }
        let (look_yaw, look_pitch) = (p.yaw, p.pitch);
        let (yaw, pitch) = (p.prev_yaw + (p.yaw - p.prev_yaw) * partial, p.prev_pitch + (p.pitch - p.prev_pitch) * partial);
        let mut m = base * self.warp(partial);
        if self.third_person > 0 {
            let mut d3 = self.distance as f64;
            let f1 = look_yaw;
            let mut f2 = look_pitch;
            if self.third_person == 2 {
                f2 += 180.0;
            }
            let k = mth::PI / 180.0;
            let d4 = (-mth::sin(f1 * k) * mth::cos(f2 * k)) as f64 * d3;
            let d5 = (mth::cos(f1 * k) * mth::cos(f2 * k)) as f64 * d3;
            let d6 = (-mth::sin(f2 * k)) as f64 * d3;
            for i in 0..8 {
                let f3 = ((i & 1) * 2 - 1) as f32 * 0.1;
                let f4 = ((i >> 1 & 1) * 2 - 1) as f32 * 0.1;
                let f5 = ((i >> 2 & 1) * 2 - 1) as f32 * 0.1;
                let from = eye + DVec3::new(f3 as f64, f4 as f64, f5 as f64);
                let to = DVec3::new(eye.x - d4 + f3 as f64 + f5 as f64, eye.y - d6 + f4 as f64, eye.z - d5 + f5 as f64);
                let len = (to - from).length();
                if let Some(hit) = interact::raycast(w, t, from, to - from, len) {
                    let d7 = hit.point.distance(eye);
                    if d7 < d3 {
                        d3 = d7;
                    }
                }
            }
            if self.third_person == 2 {
                m *= rot(180.0, 0.0, 1.0, 0.0);
            }
            m *= rot(look_pitch - f2, 1.0, 0.0, 0.0) * rot(look_yaw - f1, 0.0, 1.0, 0.0) * tr(0.0, 0.0, -d3 as f32) * rot(f1 - look_yaw, 0.0, 1.0, 0.0) * rot(f2 - look_pitch, 1.0, 0.0, 0.0);
        } else {
            m *= tr(0.0, 0.0, -0.1);
        }
        m *= rot(pitch, 1.0, 0.0, 0.0) * rot(yaw + 180.0, 0.0, 1.0, 0.0) * tr(0.0, -eye_h, 0.0);
        // O mundo é desenhado em coordenadas absolutas: desloca pelos pés interpolados.
        let view = m * Mat4::from_translation(-feet.as_vec3());
        let cam_pos = view.inverse().transform_point3(Vec3::ZERO).as_dvec3();
        let water = in_water_at(w, cam_pos);
        let mut fov = self.fov_setting * (self.fov_hand_prev + (self.fov_hand - self.fov_hand_prev) * partial);
        let mut hand_fov = 70.0;
        if water {
            fov = fov * 60.0 / 70.0;
            hand_fov = hand_fov * 60.0 / 70.0;
        }
        View { view, eye: cam_pos, fov, hand_fov, hand_base: base }
    }
}

/// `Entity.isInsideOfMaterial(Material.water)` do jogador (olhos dentro da água, com a altura da superfície).
pub fn eyes_in_water(w: &World, p: &Player) -> bool {
    let y = p.pos.y + p.eye_height();
    let (x, by, z) = (mth::floor(p.pos.x), mth::floor(y), mth::floor(p.pos.z));
    let st = w.block(x, by, z);
    let id = (st >> 4) as usize;
    if BLOCKS[id].0 != M_WATER {
        return false;
    }
    let mut level = (st & 15) as i32;
    if level >= 8 {
        level = 0;
    }
    let f = (level + 1) as f32 / 9.0 - 0.111_111_11;
    y < ((by + 1) as f32 - f) as f64
}

/// `ActiveRenderInfo.getBlockAtEntityViewpoint` == água (com a altura da superfície).
fn in_water_at(w: &World, p: DVec3) -> bool {
    let (x, y, z) = (mth::floor(p.x), mth::floor(p.y), mth::floor(p.z));
    let st = w.block(x, y, z);
    let mut id = (st >> 4) as usize;
    if MATERIALS[BLOCKS[id].0 as usize].liquid {
        let mut level = (st & 15) as i32;
        if level >= 8 {
            level = 0;
        }
        let f = (level + 1) as f32 / 9.0 - 0.11111111;
        let f1 = (y + 1) as f32 - f;
        if p.y >= f1 as f64 {
            id = (w.block(x, y + 1, z) >> 4) as usize;
        }
    }
    BLOCKS[id].0 == M_WATER
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primeira_pessoa_olha_para_o_yaw() {
        let mut p = Player::default();
        p.set_position(DVec3::new(0.0, 64.0, 0.0));
        p.prev = p.pos;
        let w = World::new(true);
        let t = BlockTable::new();
        let c = CameraState { view_bobbing: false, ..Default::default() };
        let v = c.view(&p, &w, &t, 0.0);
        // Yaw 0 olha para +Z: um ponto à frente fica em -Z da câmera.
        let ahead = v.view.transform_point3(Vec3::new(0.0, 65.62, 5.0));
        assert!(ahead.z < 0.0 && ahead.x.abs() < 1e-4 && ahead.y.abs() < 1e-3, "{ahead:?}");
        // O olho fica 0,1 atrás do ponto dos olhos (translate(0, 0, -0.1)).
        assert!((v.eye - DVec3::new(0.0, 64.0 + 1.62f32 as f64, -0.1)).length() < 1e-4, "{:?}", v.eye);
        assert_eq!(v.fov, 70.0);
    }

    #[test]
    fn fov_da_corrida_suaviza_por_tick() {
        let mut p = Player::default();
        p.set_sprinting(true);
        let mut c = CameraState::default();
        c.tick(&p);
        // 1 + (1,15 - 1) * 0,5.
        assert!((c.fov_hand - 1.075).abs() < 1e-6 && c.fov_hand_prev == 1.0);
        for _ in 0..30 {
            c.tick(&p);
        }
        assert!((c.fov_hand - 1.15).abs() < 1e-5);
    }

    #[test]
    fn terceira_pessoa_afasta_quatro_blocos() {
        let mut p = Player::default();
        p.set_position(DVec3::new(0.0, 100.0, 0.0));
        p.prev = p.pos;
        let w = World::new(true);
        let t = BlockTable::new();
        let mut c = CameraState { view_bobbing: false, ..Default::default() };
        c.cycle();
        let v = c.view(&p, &w, &t, 0.0);
        let head = DVec3::new(0.0, 100.0 + 1.62f32 as f64, 0.0);
        assert!((v.eye.distance(head) - 4.0).abs() < 1e-3 && v.eye.z < 0.0, "{:?}", v.eye);
        c.cycle();
        let v = c.view(&p, &w, &t, 0.0);
        assert!((v.eye.distance(head) - 4.0).abs() < 1e-3 && v.eye.z > 0.0, "{:?}", v.eye);
    }
}

#[cfg(test)]
mod warp_tests {
    use super::*;

    #[test]
    fn warp_do_portal_e_da_nausea() {
        let mut c = CameraState::default();
        assert_eq!(c.warp(0.5), Mat4::IDENTITY, "sem portal nem náusea não distorce");
        c.portal = (0.5, 0.5);
        c.ticks = 0;
        // Ângulo 0: só a escala 1/f2 no X, com f2 = (5/(0,25+5) - 0,02)².
        let f2 = (5.0f32 / 5.25 - 0.02).powi(2);
        let m = c.warp(0.0);
        assert!((m.x_axis.x - 1.0 / f2).abs() < 1e-5);
        assert!((m.y_axis.y - 1.0).abs() < 1e-5);
        // A náusea gira mais devagar (7 graus por tick em vez de 20).
        c.ticks = 3;
        c.confusion = true;
        assert!((c.warp(0.0) - rot(21.0, 0.0, 1.0, 1.0) * Mat4::from_scale(Vec3::new(1.0 / f2, 1.0, 1.0)) * rot(-21.0, 0.0, 1.0, 1.0)).abs_diff_eq(Mat4::ZERO, 1e-5));
    }
}
