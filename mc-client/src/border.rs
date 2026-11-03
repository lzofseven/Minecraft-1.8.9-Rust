//! `WorldBorder` do cliente (S44): centro, diâmetro com a transição pelo relógio, tamanho do mundo e avisos;
//! a parede de `RenderGlobal.renderWorldBorder`.

use std::time::Instant;

use mc_world::mesh::{Geometry, Vertex};

/// Ação do S44 (`S44PacketWorldBorder.Action`).
#[derive(Clone, Debug, PartialEq)]
pub enum BorderPacket {
    SetSize(f64),
    LerpSize { from: f64, to: f64, ms: i64 },
    SetCenter(f64, f64),
    WarningTime(i32),
    WarningBlocks(i32),
    Initialize { x: f64, z: f64, from: f64, to: f64, ms: i64, size: i32, warning_distance: i32, warning_time: i32 },
}

#[derive(Clone, Debug)]
pub struct WorldBorder {
    center: (f64, f64),
    start_diameter: f64,
    end_diameter: f64,
    start: Instant,
    end: Instant,
    world_size: i32,
    pub warning_time: i32,
    pub warning_distance: i32,
}

impl Default for WorldBorder {
    fn default() -> Self {
        let now = Instant::now();
        Self { center: (0.0, 0.0), start_diameter: 6.0e7, end_diameter: 6.0e7, start: now, end: now, world_size: 29999984, warning_time: 15, warning_distance: 5 }
    }
}

impl WorldBorder {
    /// `S44PacketWorldBorder.func_179788_a`.
    pub fn apply(&mut self, p: &BorderPacket) {
        match *p {
            BorderPacket::SetSize(d) => self.set_transition(d),
            BorderPacket::LerpSize { from, to, ms } => self.lerp(from, to, ms),
            BorderPacket::SetCenter(x, z) => self.center = (x, z),
            BorderPacket::WarningTime(t) => self.warning_time = t,
            BorderPacket::WarningBlocks(b) => self.warning_distance = b,
            BorderPacket::Initialize { x, z, from, to, ms, size, warning_distance, warning_time } => {
                self.center = (x, z);
                if ms > 0 {
                    self.lerp(from, to, ms);
                } else {
                    self.set_transition(to);
                }
                self.world_size = size;
                self.warning_distance = warning_distance;
                self.warning_time = warning_time;
            }
        }
    }

    fn set_transition(&mut self, d: f64) {
        self.start_diameter = d;
        self.end_diameter = d;
        self.start = Instant::now();
        self.end = self.start;
    }

    fn lerp(&mut self, from: f64, to: f64, ms: i64) {
        self.start_diameter = from;
        self.end_diameter = to;
        self.start = Instant::now();
        self.end = self.start + std::time::Duration::from_millis(ms.max(0) as u64);
    }

    /// Já chegou ao fim da transição (o `getDiameter` do Java troca para parado nessa hora).
    fn stationary(&self) -> bool {
        self.end_diameter == self.start_diameter || Instant::now() >= self.end
    }

    /// `getStatus().getID()`: a cor da parede.
    pub fn status_color(&self) -> u32 {
        if self.stationary() {
            0x20A0FF
        } else if self.end_diameter < self.start_diameter {
            0xFF3030
        } else {
            0x40FF80
        }
    }

    /// `getDiameter`: interpolado pelo tempo da transição (em float, como o Java).
    pub fn diameter(&self) -> f64 {
        if self.stationary() {
            return self.end_diameter;
        }
        let total = self.end.duration_since(self.start).as_millis() as f32;
        let d0 = (self.start.elapsed().as_millis() as f32 / total) as f64;
        self.start_diameter + (self.end_diameter - self.start_diameter) * d0
    }

    /// (minX, minZ, maxX, maxZ), presos ao tamanho do mundo.
    pub fn bounds(&self) -> [f64; 4] {
        let r = self.diameter() / 2.0;
        let s = self.world_size as f64;
        [(self.center.0 - r).max(-s), (self.center.1 - r).max(-s), (self.center.0 + r).min(s), (self.center.1 + r).min(s)]
    }

    /// `contains(BlockPos)`.
    pub fn contains(&self, x: i32, z: i32) -> bool {
        contains(&self.bounds(), x, z)
    }

    /// `getResizeSpeed` (blocos por ms) e `getTargetSize`.
    pub fn resize_speed(&self) -> f64 {
        if self.stationary() {
            return 0.0;
        }
        (self.start_diameter - self.end_diameter).abs() / self.end.duration_since(self.start).as_millis().max(1) as f64
    }

    /// `GuiIngame.renderVignette`: quanto a vinheta avermelha perto da borda (0 longe dela).
    pub fn warning(&self, x: f64, z: f64) -> f32 {
        let d = self.closest_distance(x, z) as f32;
        let l5 = (self.resize_speed() * self.warning_time as f64 * 1000.0).min((self.end_diameter - self.diameter()).abs());
        let l7 = (self.warning_distance as f64).max(l5);
        if (d as f64) < l7 { 1.0 - (d as f64 / l7) as f32 } else { 0.0 }
    }

    /// `getClosestDistance`.
    pub fn closest_distance(&self, x: f64, z: f64) -> f64 {
        let b = self.bounds();
        (x - b[0]).min(b[2] - x).min(z - b[1]).min(b[3] - z)
    }
}

pub fn contains(b: &[f64; 4], x: i32, z: i32) -> bool {
    (x + 1) as f64 > b[0] && (x as f64) < b[2] && (z + 1) as f64 > b[1] && (z as f64) < b[3]
}

/// `RenderGlobal.renderWorldBorder`: faixas de 1 bloco de 0 a 256 nos lados a menos da distância de
/// desenho, com a cor do estado e alfa (1 - distância / alcance)⁴; a textura anda com o relógio.
pub fn wall(border: &WorldBorder, eye: (f64, f64), view: f64, time_ms: u64) -> Option<Geometry> {
    let b = border.bounds();
    let (px, pz) = eye;
    if !(px >= b[2] - view || px <= b[0] + view || pz >= b[3] - view || pz <= b[1] + view) {
        return None;
    }
    let d1 = (1.0 - border.closest_distance(px, pz) / view).powi(4);
    let c = border.status_color();
    let color = [(c >> 16) as u8, (c >> 8) as u8, c as u8, (d1.clamp(0.0, 1.0) * 255.0) as u8];
    let f3 = (time_ms % 3000) as f32 / 3000.0;
    let light = crate::models::FULL_LIGHT;
    let v = |x: f64, y: f64, z: f64, u: f32, w: f32| Vertex { pos: [x as f32, y as f32, z as f32], uv: [u, w], color, light };
    let mut geo = Geometry::default();
    let mut strip = |fixed: f64, from: f64, to: f64, along_z: bool| {
        let (mut d, mut f) = (from, 0.0f32);
        while d < to {
            let len = (to - d).min(1.0);
            let k = len as f32 * 0.5;
            let p = |a: f64, y: f64| if along_z { (fixed, y, a) } else { (a, y, fixed) };
            let (a0, a1) = (p(d, 256.0), p(d + len, 256.0));
            let (a2, a3) = (p(d + len, 0.0), p(d, 0.0));
            geo.quad([v(a0.0, a0.1, a0.2, f3 + f, f3), v(a1.0, a1.1, a1.2, f3 + k + f, f3), v(a2.0, a2.1, a2.2, f3 + k + f, f3 + 128.0), v(a3.0, a3.1, a3.2, f3 + f, f3 + 128.0)]);
            d += 1.0;
            f += 0.5;
        }
    };
    let (z0, z1) = ((pz - view).floor().max(b[1]), (pz + view).ceil().min(b[3]));
    if px > b[2] - view {
        strip(b[2], z0, z1, true);
    }
    if px < b[0] + view {
        strip(b[0], z0, z1, true);
    }
    let (x0, x1) = ((px - view).floor().max(b[0]), (px + view).ceil().min(b[2]));
    if pz > b[3] - view {
        strip(b[3], x0, x1, false);
    }
    if pz < b[1] + view {
        strip(b[1], x0, x1, false);
    }
    (!geo.is_empty()).then_some(geo)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn borda_de_100_blocos_em_volta_do_centro() {
        let mut b = WorldBorder::default();
        b.apply(&BorderPacket::Initialize { x: 0.5, z: 0.5, from: 100.0, to: 100.0, ms: 0, size: 29999984, warning_distance: 5, warning_time: 15 });
        assert_eq!(b.bounds(), [-49.5, -49.5, 50.5, 50.5]);
        assert!(b.contains(50, 0) && !b.contains(51, 0) && b.contains(-50, 0) && !b.contains(-51, 0));
        assert_eq!(b.closest_distance(40.5, 0.0), 10.0);
        assert_eq!(b.status_color(), 0x20A0FF);
        assert!(wall(&b, (0.0, 0.0), 128.0, 0).is_some() && wall(&b, (0.0, 0.0), 32.0, 0).is_none());
        b.apply(&BorderPacket::LerpSize { from: 100.0, to: 10.0, ms: 60_000 });
        assert_eq!(b.status_color(), 0xFF3030);
        assert!(b.diameter() <= 100.0 && b.diameter() > 99.0);
    }
}
