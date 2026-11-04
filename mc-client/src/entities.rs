//! Entidades do cliente: estado vindo da rede, interpolação de 3 ticks
//! (`setPositionAndRotation2`), corpo x cabeça, balanço de braços e pernas,
//! soco, lista de jogadores e montagem da geometria de cada quadro (modelos e nomes).

use std::borrow::Cow;
use std::collections::HashMap;

use glam::{DVec3, Mat4, Vec3, Vec4};
use mc_render::ui::{UiBatch, UiVertex};
use mc_render::{Camera, DrawItem, Jar, Renderer, Tex};
use mc_world::chunk::Section;
use mc_world::mesh::{Geometry, Vertex};
use mc_world::World;

use crate::entity_net::{EntityPacket, MetaValue, PlayerListAction, ProfileProperty, Slot, Uuid};
use crate::models::{living_matrix, Anim, Biped, Sink, Villager, FULL_LIGHT};
use crate::physics::Player;
use crate::skins::{default_skin, skin_from_properties, SkinRef, Textures};

/// Entrada da lista de jogadores (tab).
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerInfo {
    pub name: String,
    /// Nome de exibição como componente de chat (JSON), se o servidor mandou.
    pub display_name: Option<String>,
    pub ping: i32,
    pub gamemode: i32,
    pub skin: Option<SkinRef>,
    pub properties: Vec<ProfileProperty>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Player { uuid: Uuid, name: String, skin: Option<SkinRef> },
    /// Id de mob do `EntityList` (50 creeper, 54 zumbi, 120 aldeão...).
    Mob(u8),
    /// Tipo de objeto do 0x0E (2 item, 78 armor stand...) e o campo extra.
    Object { kind: u8, data: i32 },
}

pub const ARMOR_STAND: u8 = 78;
pub const ITEM: u8 = 2;

/// Graus de um ângulo em byte.
pub fn angle(b: i8) -> f32 {
    b as f32 * 360.0 / 256.0
}

/// `MathHelper.wrapAngleTo180_float`.
pub fn wrap_deg(a: f32) -> f32 {
    let mut a = a % 360.0;
    if a >= 180.0 {
        a -= 360.0;
    }
    if a < -180.0 {
        a += 360.0;
    }
    a
}

/// `RendererLivingEntity.interpolateRotation`.
pub fn lerp_rot(prev: f32, cur: f32, t: f32) -> f32 {
    let mut d = cur - prev;
    while d < -180.0 {
        d += 360.0;
    }
    while d >= 180.0 {
        d -= 360.0;
    }
    prev + t * d
}

/// Duração do soco em ticks (`getArmSwingAnimationEnd` sem poções).
const SWING_END: i32 = 6;

/// Quem segura a coleira (`getLeashedToEntity`) como o `RenderLiving` lê: `prev`/`pos` para interpolar, os
/// ângulos, a altura dos olhos, se é um nó pendurado (`EntityHanging`) e a caixa para o frustum.
#[derive(Clone, Copy, Debug)]
pub struct Anchor {
    pub prev: DVec3,
    pub pos: DVec3,
    pub prev_yaw: f32,
    pub yaw: f32,
    pub prev_pitch: f32,
    pub pitch: f32,
    pub eye: f64,
    pub hanging: bool,
    /// `getEntityBoundingBox` (a posição atual, não a interpolada).
    pub bb: (DVec3, DVec3),
}

impl Anchor {
    /// Uma entidade da lista como segurador: a caixa dela; o nó tem a sua (`EntityLeashKnot`).
    pub fn of(e: &Entity) -> Self {
        let (w, h) = e.size();
        let p = e.pos;
        let bb = if matches!(e.kind, Kind::Object { kind: crate::naovivas::LEASH_KNOT, .. }) {
            // EntityLeashKnot: 0,375 de lado, de y - 0,125 a y + 0,375.
            (DVec3::new(p.x - 0.1875, p.y - 0.125, p.z - 0.1875), DVec3::new(p.x + 0.1875, p.y + 0.375, p.z + 0.1875))
        } else {
            let hw = w as f64 / 2.0;
            (DVec3::new(p.x - hw, p.y, p.z - hw), DVec3::new(p.x + hw, p.y + h as f64, p.z + hw))
        };
        // EntityPlayer.getEyeHeight: 1,62, menos 0,08 agachado; os outros, `Entity.getEyeHeight`.
        let eye = match e.kind {
            Kind::Player { .. } => {
                let f: f32 = if e.sneaking() { 1.62 - 0.08 } else { 1.62 };
                f as f64
            }
            _ => e.eye_height() as f64,
        };
        let hanging = matches!(e.kind, Kind::Object { kind: crate::naovivas::LEASH_KNOT | crate::naovivas::ITEM_FRAME | crate::naovivas::PAINTING, .. });
        Self { prev: e.prev, pos: p, prev_yaw: e.prev_yaw, yaw: e.yaw, prev_pitch: e.prev_pitch, pitch: e.pitch, eye, hanging, bb }
    }

    /// O próprio jogador, fora da lista de entidades: caixa de 0,6 x 1,8.
    pub fn of_player(p: &Player) -> Self {
        let hw = 0.3;
        Self {
            prev: p.prev,
            pos: p.pos,
            prev_yaw: p.prev_yaw,
            yaw: p.yaw,
            prev_pitch: p.prev_pitch,
            pitch: p.pitch,
            eye: p.eye_height(),
            hanging: false,
            bb: (DVec3::new(p.pos.x - hw, p.pos.y, p.pos.z - hw), DVec3::new(p.pos.x + hw, p.pos.y + 1.8, p.pos.z + hw)),
        }
    }
}

/// `EntityLeashKnot(world, BlockPos)`: o nó fica no centro do bloco (+0,5). O pacote manda o bloco x32.
fn place_knot(e: &mut Entity) {
    let b = e.server.map(|v| v.div_euclid(32));
    e.pos = DVec3::new(b[0] as f64 + 0.5, b[1] as f64 + 0.5, b[2] as f64 + 0.5);
    e.prev = e.pos;
    e.obj.hanging = b;
}

#[derive(Clone, Debug)]
pub struct Entity {
    pub id: i32,
    pub kind: Kind,
    pub pos: DVec3,
    pub prev: DVec3,
    /// Posição do servidor em ponto fixo (x32).
    pub server: [i32; 3],
    target: DVec3,
    target_yaw: f32,
    target_pitch: f32,
    /// Ticks restantes até chegar no alvo (3 a cada pacote).
    pub increments: u32,
    pub yaw: f32,
    pub prev_yaw: f32,
    pub pitch: f32,
    pub prev_pitch: f32,
    pub head_yaw: f32,
    pub prev_head_yaw: f32,
    pub body_yaw: f32,
    pub prev_body_yaw: f32,
    pub limb_swing: f32,
    pub limb_amount: f32,
    pub prev_limb_amount: f32,
    pub swing_progress: f32,
    pub prev_swing_progress: f32,
    swing_ticks: i32,
    swinging: bool,
    /// `EntityBodyHelper` dos mobs.
    body_counter: i32,
    body_prev_head: f32,
    pub on_ground: bool,
    /// `inWater` (calculado por `Entities::tick_in`).
    pub in_water: bool,
    pub vel: DVec3,
    pub age: u32,
    pub hurt_time: i32,
    pub death_time: i32,
    pub dead: bool,
    pub meta: HashMap<u8, MetaValue>,
    /// Mão, botas, calça, peito, capacete.
    pub equipment: [Option<Slot>; 5],
    /// `itemInUse` de outro jogador: ação (se o item tem), duração máxima e contagem (`getItemInUseCount`).
    pub item_use: Option<(Option<crate::physics::UseAction>, i32, i32)>,
    /// `EntityOtherPlayerMP.isItemInUse` e o item da mão trocado pelo 0x04 neste tick.
    pub using_item: bool,
    pub held_changed: bool,
    /// Tem um anzol na água (`fishEntity`, pelo campo `data` do 0x0E).
    pub fishing: bool,
    /// `riddenByEntity` (id de quem monta, pelo 0x1B).
    pub ridden_by: Option<i32>,
    /// `leashedToEntity` (id de quem segura, pelo 0x1B com leash = 1); `None` = sem coleira.
    pub leash_to: Option<i32>,
    /// Guardião: `field_175479_bo` (ticks do ataque) e o alvo do metadado 17 que ele acompanha.
    pub attack_time: i32,
    pub attack_target: i32,
    /// Estado por tick que o modelo lê (inchaço do creeper, asas da galinha).
    pub mob_state: crate::mobs::MobState,
    /// Estado das entidades não-vivas (item no chão, flecha...).
    pub obj: crate::naovivas::ObjState,
    /// Jogadores: `chasingPos` e `cameraYaw` (o balanço da capa).
    pub chasing: DVec3,
    pub prev_chasing: DVec3,
    pub camera_yaw: f32,
    pub prev_camera_yaw: f32,
}

impl Entity {
    pub fn new(id: i32, kind: Kind, server: [i32; 3], yaw: f32, pitch: f32) -> Self {
        let pos = DVec3::new(server[0] as f64, server[1] as f64, server[2] as f64) / 32.0;
        Self {
            id,
            kind,
            pos,
            prev: pos,
            server,
            target: pos,
            target_yaw: yaw,
            target_pitch: pitch,
            increments: 0,
            yaw,
            prev_yaw: yaw,
            pitch,
            prev_pitch: pitch,
            head_yaw: yaw,
            prev_head_yaw: yaw,
            body_yaw: yaw,
            prev_body_yaw: yaw,
            limb_swing: 0.0,
            limb_amount: 0.0,
            prev_limb_amount: 0.0,
            swing_progress: 0.0,
            prev_swing_progress: 0.0,
            swing_ticks: 0,
            swinging: false,
            body_counter: 0,
            body_prev_head: yaw,
            on_ground: false,
            in_water: false,
            vel: DVec3::ZERO,
            age: 0,
            hurt_time: 0,
            death_time: 0,
            dead: false,
            meta: HashMap::new(),
            equipment: Default::default(),
            item_use: None,
            using_item: false,
            held_changed: false,
            fishing: false,
            ridden_by: None,
            leash_to: None,
            attack_time: 0,
            attack_target: 0,
            mob_state: Default::default(),
            obj: crate::naovivas::ObjState { hover_start: crate::naovivas::hover_for(id), ..Default::default() },
            chasing: pos,
            prev_chasing: pos,
            camera_yaw: 0.0,
            prev_camera_yaw: 0.0,
        }
    }

    /// Entidades vivas (`EntityLivingBase`) interpolam; objetos vão direto para a posição nova.
    pub fn is_living(&self) -> bool {
        match self.kind {
            Kind::Player { .. } | Kind::Mob(_) => true,
            Kind::Object { kind, .. } => kind == ARMOR_STAND,
        }
    }

    /// Barco e carrinho também interpolam a posição (`boatPosRotationIncrements`), como os vivos.
    pub fn interpolates(&self) -> bool {
        self.is_living() || matches!(self.kind, Kind::Object { kind: 1 | 10, .. })
    }

    /// `canBeCollidedWith` com o `EntitySelectors.NOT_SPECTATING` do `getMouseOver`: o que a mira pode pegar.
    /// Itens, orbes, flechas e os outros projéteis (menos a bola de fogo grande) passam direto.
    pub fn can_be_collided(&self, players: &HashMap<Uuid, PlayerInfo>) -> bool {
        use crate::naovivas::*;
        match &self.kind {
            // EntityLivingBase: !isDead; EntityPlayer espectador fica de fora.
            Kind::Player { uuid, .. } => !self.dead && players.get(uuid).is_none_or(|p| p.gamemode != 3),
            // EntityDragon: só as partes colidem.
            Kind::Mob(63) => false,
            Kind::Mob(_) => !self.dead,
            Kind::Object { kind, .. } => match *kind {
                ARMOR_STAND => !self.dead && self.meta_byte(10) & 0x10 == 0,
                BOAT | MINECART | TNT | FALLING_BLOCK => !self.dead,
                CRYSTAL | LARGE_FIREBALL | ITEM_FRAME | LEASH_KNOT | PAINTING => true,
                _ => false,
            },
        }
    }

    pub fn meta_byte(&self, i: u8) -> i8 {
        match self.meta.get(&i) {
            Some(MetaValue::Byte(b)) => *b,
            _ => 0,
        }
    }

    pub fn meta_int(&self, i: u8) -> i32 {
        match self.meta.get(&i) {
            Some(MetaValue::Int(v)) => *v,
            _ => 0,
        }
    }

    pub fn meta_str(&self, i: u8) -> &str {
        match self.meta.get(&i) {
            Some(MetaValue::Str(s)) => s,
            _ => "",
        }
    }

    pub fn sneaking(&self) -> bool {
        self.meta_byte(0) & 0x02 != 0
    }

    pub fn invisible(&self) -> bool {
        self.meta_byte(0) & 0x20 != 0
    }

    /// Filhote (zumbi: metadado 12 = 1; aldeão: idade < 0).
    pub fn child(&self) -> bool {
        match self.kind {
            Kind::Mob(54 | 57) => self.meta_byte(12) == 1,
            // EntityAgeable.getGrowingAge < 0 (o byte 12 vale -1 para filhote).
            Kind::Mob(90 | 91 | 92 | 93 | 95 | 96 | 98 | 100 | 101 | 120) => self.meta_byte(12) < 0,
            Kind::Object { kind: ARMOR_STAND, .. } => self.meta_byte(10) & 0x01 != 0,
            _ => false,
        }
    }

    /// Largura e altura da caixa (`setSize` de cada classe).
    pub fn size(&self) -> (f32, f32) {
        let child = if self.child() { 0.5 } else { 1.0 };
        match self.kind {
            Kind::Player { .. } => (0.6, 1.8),
            Kind::Mob(id) => match id {
                51 if self.meta_byte(13) == 1 => (0.72, 2.535),
                51 | 66 => (0.6, 1.95),
                54 | 57 => (0.6 * child, 1.95 * child),
                52 => (1.4, 0.9),
                53 => (3.6, 11.7),
                55 | 62 => {
                    let s = 0.51000005 * self.meta_byte(16).max(1) as f32;
                    (s, s)
                }
                56 => (4.0, 4.0),
                58 => (0.6, 2.9),
                59 => (0.7, 0.5),
                60 | 67 => (0.4, 0.3),
                63 => (16.0, 8.0),
                64 => (0.9, 3.5),
                65 => (0.5, 0.9),
                // EntityGuardian.setElder: 1.9975 quando o bit 4 do byte 16 está ligado.
                68 if self.meta_byte(16) & 4 != 0 => (1.9975, 1.9975),
                68 => (0.85, 0.85),
                90 => (0.9, 0.9),
                91 | 92 | 96 => (0.9, 1.3),
                93 => (0.4, 0.7),
                94 => (0.95, 0.95),
                95 => (0.6, 0.8),
                97 => (0.7, 1.9),
                98 | 101 => (0.6, 0.7),
                99 => (1.4, 2.9),
                100 => (1.4, 1.6),
                120 => (0.6 * child, 1.8 * child),
                _ => (0.6, 1.8),
            },
            Kind::Object { kind, .. } => match kind {
                ARMOR_STAND if self.meta_byte(10) & 0x10 != 0 => (0.0, 0.0),
                ARMOR_STAND => (0.5, 1.975),
                1 => (1.5, 0.6),
                10 => (0.98, 0.7),
                50 | 70 => (0.98, 0.98),
                51 => (2.0, 2.0),
                60 | crate::naovivas::XP_ORB => (0.5, 0.5),
                64 | 66 => (0.3125, 0.3125),
                crate::naovivas::PAINTING | crate::naovivas::LIGHTNING => (0.5, 0.5),
                63 => (1.0, 1.0),
                71 => (0.5, 0.5),
                // EntityHanging.setSize(0,5; 0,5).
                crate::naovivas::LEASH_KNOT => (0.5, 0.5),
                _ => (0.25, 0.25),
            },
        }
    }

    pub fn eye_height(&self) -> f32 {
        match self.kind {
            Kind::Player { .. } => 1.62,
            // EntityTNTPrimed e EntityArrow: getEyeHeight() = 0.
            Kind::Object { kind: 50 | 60, .. } => 0.0,
            // EntityLeashKnot.getEyeHeight: -0,0625.
            Kind::Object { kind: crate::naovivas::LEASH_KNOT, .. } => -0.0625,
            _ => self.size().1 * 0.85,
        }
    }

    /// `setPositionAndRotation2(..., 3, ...)`.
    fn set_target(&mut self, pos: DVec3, yaw: f32, pitch: f32) {
        if self.interpolates() {
            self.target = pos;
            self.target_yaw = yaw;
            self.target_pitch = pitch;
            self.increments = 3;
        } else {
            self.pos = pos;
            self.yaw = yaw;
            self.pitch = pitch;
        }
    }

    fn server_pos(&self) -> DVec3 {
        DVec3::new(self.server[0] as f64, self.server[1] as f64, self.server[2] as f64) / 32.0
    }

    /// `swingItem`: reinicia o soco se o anterior já passou da metade.
    pub fn swing_arm(&mut self) {
        if !self.swinging || self.swing_ticks >= SWING_END / 2 || self.swing_ticks < 0 {
            self.swing_ticks = -1;
            self.swinging = true;
        }
    }

    /// `getSwingProgress(partial)`.
    pub fn swing_at(&self, t: f32) -> f32 {
        let mut d = self.swing_progress - self.prev_swing_progress;
        if d < 0.0 {
            d += 1.0;
        }
        self.prev_swing_progress + d * t
    }

    /// Um tick do cliente (`onUpdate` de `EntityOtherPlayerMP` / `EntityLivingBase`).
    pub fn tick(&mut self) {
        self.age += 1;
        self.prev = self.pos;
        self.prev_yaw = self.yaw;
        self.prev_pitch = self.pitch;
        self.prev_body_yaw = self.body_yaw;
        self.prev_head_yaw = self.head_yaw;
        self.prev_swing_progress = self.swing_progress;
        if self.hurt_time > 0 {
            self.hurt_time -= 1;
        }
        if self.obj.fixed_age == Some(-1) {
            // Gancho de teste (`MINE_RS_TEST_FX=dano`): mantém a tinta de dano.
            self.hurt_time = 5;
        }
        if let Kind::Mob(id) = self.kind {
            let tick = crate::mobs::TickIn { id, eid: self.id, meta: &self.meta, on_ground: self.on_ground, in_water: self.in_water, body_yaw: self.body_yaw, yaw: self.yaw, y: self.pos.y, vel: self.vel };
            self.mob_state.tick(&tick);
        }
        if !self.is_living() {
            if self.increments > 0 {
                let n = self.increments as f64;
                self.pos += (self.target - self.pos) / n;
                self.yaw += wrap_deg(self.target_yaw - self.yaw) / n as f32;
                self.pitch += (self.target_pitch - self.pitch) / n as f32;
                self.increments -= 1;
            }
            crate::naovivas::tick(self);
            return;
        }
        let health = match self.meta.get(&6) {
            Some(MetaValue::Float(h)) => Some(*h),
            _ => None,
        };
        if self.dead || health.is_some_and(|h| h <= 0.0) {
            self.death_time += 1;
        }

        // onLivingUpdate: anda 1/n do caminho até o alvo.
        if self.increments > 0 {
            let n = self.increments as f64;
            self.pos += (self.target - self.pos) / n;
            self.yaw += wrap_deg(self.target_yaw - self.yaw) / n as f32;
            self.pitch += (self.target_pitch - self.pitch) / n as f32;
            self.increments -= 1;
        }

        // updateArmSwingProgress
        if self.swinging {
            self.swing_ticks += 1;
            if self.swing_ticks >= SWING_END {
                self.swing_ticks = 0;
                self.swinging = false;
            }
        } else {
            self.swing_ticks = 0;
        }
        self.swing_progress = self.swing_ticks as f32 / SWING_END as f32;

        let (dx, dz) = (self.pos.x - self.prev.x, self.pos.z - self.prev.z);
        let dist2 = dx * dx + dz * dz;
        match self.kind {
            Kind::Player { .. } => {
                let mut target = self.body_yaw;
                if dist2 as f32 > 0.0025000002 {
                    target = (dz.atan2(dx) as f32).to_degrees() - 90.0;
                }
                if self.swing_progress > 0.0 {
                    target = self.yaw;
                }
                self.update_body(target);
            }
            Kind::Object { .. } => {
                // Armor stand: corpo e cabeça seguem o yaw.
                self.body_yaw = self.yaw;
                self.head_yaw = self.yaw;
            }
            Kind::Mob(id) => {
                if id == 94 && self.in_water {
                    // EntitySquid.onLivingUpdate: o corpo gira para a direção do movimento.
                    let target = -(self.vel.x.atan2(self.vel.z) as f32) * 180.0 / std::f32::consts::PI;
                    self.body_yaw += (target - self.body_yaw) * 0.1;
                    self.yaw = self.body_yaw;
                }
                self.body_helper(dist2)
            }
        }

        // Balanço dos membros pela distância andada.
        self.prev_limb_amount = self.limb_amount;
        let f = ((dist2.sqrt() * 4.0) as f32).min(1.0);
        self.limb_amount += (f - self.limb_amount) * 0.4;
        self.limb_swing += self.limb_amount;

        // Mantém prev a menos de 180° do atual para a interpolação não dar a volta.
        for (cur, prev) in [
            (self.yaw, &mut self.prev_yaw),
            (self.body_yaw, &mut self.prev_body_yaw),
            (self.pitch, &mut self.prev_pitch),
            (self.head_yaw, &mut self.prev_head_yaw),
        ] {
            while cur - *prev < -180.0 {
                *prev -= 360.0;
            }
            while cur - *prev >= 180.0 {
                *prev += 360.0;
            }
        }
    }

    /// `EntityLivingBase.updateDistance`: o corpo segue o movimento, até 75° da cabeça.
    fn update_body(&mut self, target: f32) {
        self.body_yaw += wrap_deg(target - self.body_yaw) * 0.3;
        let d = wrap_deg(self.yaw - self.body_yaw).clamp(-75.0, 75.0);
        self.body_yaw = self.yaw - d;
        if d * d > 2500.0 {
            self.body_yaw += d * 0.2;
        }
    }

    /// `EntityBodyHelper.updateRenderAngles` (mobs).
    fn body_helper(&mut self, dist2: f64) {
        let bound = |a: f32, b: f32, max: f32| a - wrap_deg(a - b).clamp(-max, max);
        if dist2 > 2.500000277905201E-7 {
            self.body_yaw = self.yaw;
            self.head_yaw = bound(self.body_yaw, self.head_yaw, 75.0);
            self.body_prev_head = self.head_yaw;
            self.body_counter = 0;
        } else {
            let mut max = 75.0;
            if (self.head_yaw - self.body_prev_head).abs() > 15.0 {
                self.body_counter = 0;
                self.body_prev_head = self.head_yaw;
            } else {
                self.body_counter += 1;
                if self.body_counter > 10 {
                    max = (1.0 - (self.body_counter - 10) as f32 / 10.0).max(0.0) * 75.0;
                }
            }
            self.body_yaw = bound(self.head_yaw, self.body_yaw, max);
        }
    }

    /// Nome sobre a cabeça e alcance, pelas regras de cada renderizador.
    pub fn label(&self) -> Option<(&str, f32)> {
        let custom = self.meta_str(2);
        let always = self.meta_byte(3) != 0;
        match &self.kind {
            Kind::Player { name, .. } => {
                (!self.invisible() && !name.is_empty()).then_some((name.as_str(), if self.sneaking() { 32.0 } else { 64.0 }))
            }
            Kind::Object { kind: ARMOR_STAND, .. } => (always && !custom.is_empty()).then_some((custom, 64.0)),
            Kind::Mob(_) => (always && !custom.is_empty() && !self.invisible()).then_some((custom, 64.0)),
            Kind::Object { .. } => (always && !custom.is_empty()).then_some((custom, 64.0)),
        }
    }
}

/// `GuiInventory.drawEntityOnScreen`: a matriz da tela até o `renderEntityWithPosYaw`, os ângulos que ele
/// troca (corpo, cabeça e pitch) e as luzes do `enableStandardItemLighting` no espaço da tela.
pub struct GuiView {
    pub base: Mat4,
    pub body: f32,
    pub head: f32,
    pub pitch: f32,
    pub lights: [Vec3; 2],
}

impl GuiView {
    /// `drawEntityOnScreen(x, y, scale, mouseX, mouseY, ...)`.
    pub fn new(x: f32, y: f32, scale: f32, mx: f32, my: f32) -> Self {
        let screen = Mat4::from_translation(Vec3::new(x, y, 50.0)) * Mat4::from_scale(Vec3::new(-scale, scale, scale)) * Mat4::from_rotation_z(std::f32::consts::PI);
        // As luzes são fixadas com mais um giro de 135° em Y, desfeito logo depois.
        let at_light = screen * Mat4::from_rotation_y(135f32.to_radians());
        let lights = crate::models::WORLD_LIGHTS.map(|l| at_light.transform_vector3(l.normalize()).normalize());
        let base = screen * Mat4::from_rotation_x((-(my / 40.0).atan() * 20.0).to_radians());
        Self { base, body: (mx / 40.0).atan() * 20.0, head: (mx / 40.0).atan() * 40.0, pitch: -(my / 40.0).atan() * 20.0, lights }
    }
}

/// Entidades do mundo, lista de jogadores e texturas.
/// O que os nametags de jogador leem do placar: times (prefixo, sufixo, visibilidade), o time do próprio jogador
/// e o objetivo abaixo do nome (nome de exibição e pontos por jogador).
#[derive(Default, Clone)]
pub struct NameBoard {
    pub player_team: HashMap<String, String>,
    pub teams: HashMap<String, (String, String, u8)>,
    pub own_team: Option<String>,
    pub below: Option<(String, HashMap<String, i32>)>,
}

/// A partir de quantas entidades o tick roda em paralelo.
const PARALLEL_TICK: usize = 256;
/// Modelo, sombra e fogo das entidades puras (TNT, bloco caindo), depois do laço principal. Dividir isto entre os
/// núcleos foi medido e saiu mais lento (juntar as partes custa mais que montar), então fica em série.
#[allow(clippy::too_many_arguments)]
fn draw_pure_batch(list: &[(&Entity, Vec3, DVec3, f32, [u8; 4])], opaque: &mut HashMap<Tex, Geometry>, glow: &mut HashMap<Tex, Geometry>, partial: f32, time: i64, white: u32, world: &World, table: &mc_world::blocks::BlockTable, shadow_tex: Option<u32>, view_yaw: f32) {
    let one = |(e, p, pos, d2, light): &(&Entity, Vec3, DVec3, f32, [u8; 4]), out: &mut HashMap<Tex, Geometry>, gl: &mut HashMap<Tex, Geometry>| {
        let cx = crate::naovivas::Pure { t: partial, light: *light, time, white, world, table };
        crate::naovivas::draw_pure(e, *p, &cx, out, gl);
        if let Some(tex) = shadow_tex {
            let (size, op) = crate::fx::shadow_of(e);
            let strength = crate::fx::shadow_strength(*d2 as f64, op);
            if size > 0.0 && strength > 0.0 {
                crate::fx::shadow(out.entry(Tex::Custom(tex | crate::mobs::TRANSLUCENT)).or_default(), world, table, *pos, size, strength, time, *light);
            }
        }
        if e.meta_byte(0) & 1 != 0 {
            let (w, h) = e.size();
            crate::fx::fire(out.entry(Tex::Blocks).or_default(), table, *p, w, h, view_yaw);
        }
    };
    for item in list {
        one(item, opaque, glow);
    }
}

#[derive(Default)]
pub struct Entities {
    /// Placar visto pelos nametags (o game atualiza quando o placar muda).
    pub board: NameBoard,
    pub list: HashMap<i32, Entity>,
    /// Lista de jogadores (0x38), para o tab.
    pub players: HashMap<Uuid, PlayerInfo>,
    /// Id do próprio jogador (Join Game).
    pub own_id: Option<i32>,
    textures: Textures,
    /// Geometria com mistura aditiva e rótulos sem profundidade do último `draw` (vão no `Frame`).
    pub additive: Vec<DrawItem>,
    pub overlay: Vec<DrawItem>,
    /// `getTotalWorldTime` (idade do mundo pelo 0x03, +1 por tick).
    pub world_age: i64,
    /// Tempo do mundo (brilho do TNT, carrinho, sombras) e a opção `entityShadows`.
    pub world_time: i64,
    pub entity_shadows: bool,
    /// O jogador local é espectador (vê invisíveis a 15% de alfa).
    pub viewer_spectator: bool,
    /// `Minecraft.isGuiEnabled` (F1 esconde os nomes de jogadores e mobs).
    pub gui_enabled: bool,
    /// Entidades desenhadas no último `draw` (o `E:` do F3).
    pub rendered: usize,
    cache: crate::naovivas::Cache,
    /// Quadros dos nomes por texto (o layout do texto não muda enquanto o nome não muda).
    names: HashMap<String, NameQuads>,
    /// A `renderViewEntity` em primeira pessoa (S43): não é desenhada (`renderEntities`).
    pub hidden_view: Option<i32>,
    /// Visão de frente (`thirdPersonView == 2`): os nomes giram 180° a mais.
    pub front_view: bool,
}

/// Luz do mundo num ponto, no formato do vértice. Sem coluna: luz cheia; sem seção: céu aberto.
pub fn light_at(world: &World, p: DVec3) -> [u8; 4] {
    let (x, y, z) = (p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
    let Some(col) = world.columns.get(&(x >> 4, z >> 4)) else { return FULL_LIGHT };
    let open = if world.has_sky { [255, 0, 255, 0] } else { FULL_LIGHT };
    if !(0..256).contains(&y) {
        return open;
    }
    match &col.sections[(y >> 4) as usize] {
        None => open,
        Some(s) => {
            let l = s.light(Section::index(x & 15, y & 15, z & 15));
            [(l >> 4) * 17, (l & 15) * 17, 255, 0]
        }
    }
}

impl Entities {
    /// `updateRiderPosition`: ponto do assento (`posY + getMountedYOffset()`) da montaria.
    pub fn mount_point(&self, id: i32) -> Option<DVec3> {
        let e = self.list.get(&id)?;
        let (_, h) = e.size();
        let off = match e.kind {
            Kind::Object { kind: 1, .. } => -0.3,
            Kind::Object { kind: 10, .. } => 0.0,
            Kind::Mob(52 | 59) => (h * 0.5) as f64,
            _ => h as f64 * 0.75,
        };
        Some(e.pos + DVec3::new(0.0, off, 0.0))
    }

    /// O próprio jogador visto em terceira pessoa (`RenderPlayer` com os dados do `EntityPlayerSP`) ou,
    /// com `gui`, dentro de uma tela (`GuiInventory.drawEntityOnScreen`).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_local(
        &mut self,
        p: &crate::physics::Player,
        partial: f32,
        gui: Option<&GuiView>,
        skin: (u32, bool),
        parts: u8,
        held: Option<Worn>,
        armor: [Option<Worn>; 4],
        light: [u8; 4],
        tick: u32,
        cape: Option<u32>,
        items: &mut crate::first_person::ItemModels,
        table: &mc_world::blocks::BlockTable,
        r: &mut Renderer,
        jar: &mut Jar,
    ) -> Vec<DrawItem> {
        // renderEntityWithPosYaw(..., 1.0): na tela o tempo parcial é 1 e os ângulos são os trocados.
        let t = if gui.is_some() { 1.0 } else { partial };
        let body = gui.map_or_else(|| lerp_rot(p.prev_render_yaw_offset, p.render_yaw_offset, t), |g| g.body);
        let head = gui.map_or_else(|| lerp_rot(p.prev_yaw, p.yaw, t), |g| g.head);
        let pitch = gui.map_or_else(|| p.prev_pitch + (p.pitch - p.prev_pitch) * t, |g| g.pitch);
        let using = p.item_use.filter(|u| u.count > 0);
        let anim = Anim {
            limb_swing: p.limb_swing - p.limb_swing_amount * (1.0 - t),
            limb_amount: (p.prev_limb_swing_amount + (p.limb_swing_amount - p.prev_limb_swing_amount) * t).min(1.0),
            age: tick as f32 + t,
            head_yaw: head - body,
            head_pitch: pitch,
            swing: p.swing(t),
            sneak: p.sneaking,
            // RenderPlayer.setModelVisibilities: 1 segurando, 3 defendendo; arco puxado arma os braços.
            held_right: match (held.is_some(), using.map(|u| u.action)) {
                (false, _) => 0,
                (true, Some(crate::physics::UseAction::Block)) => 3,
                (true, _) => 1,
            },
            aimed_bow: using.is_some_and(|u| u.action == crate::physics::UseAction::Bow),
            ..Default::default()
        };
        let pos = p.prev.lerp(p.pos, t as f64).as_vec3();
        let mut m = Biped::player(skin.1);
        m.set_skin_parts(parts);
        m.pose(&anim);
        // RenderPlayer.doRender: o 0,125 de quem agacha não vale para o EntityPlayerSP.
        let mat = match gui {
            Some(g) => g.base * living_matrix(Vec3::ZERO, body, 0.0, 0.9375),
            None => living_matrix(pos, body, 0.0, 0.9375),
        };
        let lights = gui.map_or(crate::models::WORLD_LIGHTS, |g| g.lights);
        let mut out: HashMap<Tex, Geometry> = HashMap::new();
        let mut sink = Sink { geo: out.entry(Tex::Custom(skin.0)).or_default(), tex: (64.0, 64.0), tint: [1.0; 3], alpha: 255, light, lights };
        m.emit(&mut sink, &mat, &anim);
        biped_layers(&m, skin.1, &anim, &mat, held, armor, light, [1.0; 3], lights, false, None, items, table, &mut self.textures, r, jar, &mut out);
        if let Some(tex) = cape {
            let td = t as f64;
            let c = Cape {
                chase: p.prev_chasing.lerp(p.chasing, td) - p.prev.lerp(p.pos, td),
                body: gui.map_or(p.prev_render_yaw_offset + (p.render_yaw_offset - p.prev_render_yaw_offset) * t, |g| g.body),
                camera_yaw: p.prev_camera_yaw + (p.camera_yaw - p.prev_camera_yaw) * t,
                walked: p.prev_distance_walked + (p.distance_walked - p.prev_distance_walked) * t,
                sneak: p.sneaking,
            };
            emit_cape(out.entry(Tex::Custom(tex)).or_default(), &mat, &c, light, [1.0; 3], lights);
        }
        out.into_iter().map(|(tex, geo)| DrawItem { tex, geo, translucent: false }).collect()
    }

    /// Outra entidade numa tela (`drawEntityOnScreen` com o cavalo do `GuiScreenHorseInventory`): o modelo
    /// na origem com os ângulos trocados e o tempo parcial 1, sombreado com as luzes da tela trazidas para
    /// o espaço do modelo (a transposta da parte linear) e então levado para a tela.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_in_gui(
        &mut self,
        id: i32,
        view: &GuiView,
        cam: &Camera,
        world: &World,
        r: &mut Renderer,
        jar: &mut Jar,
        items: &mut crate::first_person::ItemModels,
        table: &mc_world::blocks::BlockTable,
    ) -> Vec<DrawItem> {
        let Some(mut e) = self.list.get(&id).cloned() else { return Vec::new() };
        (e.body_yaw, e.prev_body_yaw) = (view.body, view.body);
        (e.head_yaw, e.prev_head_yaw, e.yaw) = (view.head, view.head, view.head);
        (e.pitch, e.prev_pitch) = (view.pitch, view.pitch);
        let linear = glam::Mat3::from_mat4(view.base).transpose();
        let lights = view.lights.map(|l| (linear * l).normalize());
        let white = self.textures.white(r);
        let (mut glow, mut out) = (HashMap::new(), HashMap::new());
        let Self { players, textures, cache, world_time, .. } = self;
        let mut fx = crate::naovivas::Fx { cam, world, glow: &mut glow, time: *world_time, cache, body: true };
        draw_model(&e, &mut fx, Vec3::ZERO, 1.0, FULL_LIGHT, 1.0, textures, r, jar, players, white, items, table, lights, &mut out);
        out.into_iter()
            .map(|(tex, mut geo)| {
                for v in &mut geo.vertices {
                    v.pos = view.base.transform_point3(Vec3::from(v.pos)).to_array();
                }
                match tex {
                    Tex::Custom(id) if id & crate::mobs::TRANSLUCENT != 0 => DrawItem { tex: Tex::Custom(id & !crate::mobs::TRANSLUCENT), geo, translucent: true },
                    tex => DrawItem { tex, geo, translucent: false },
                }
            })
            .collect()
    }

    /// Skin do próprio jogador: a da lista de jogadores (pelo nome) ou, sem propriedades,
    /// a do sessionserver pelo UUID da sessão; enquanto não chega, a padrão.
    /// Capa do próprio jogador (`getLocationCape` pela entrada do tab, ou o perfil pelo sessionserver).
    pub fn local_cape(&mut self, name: &str, uuid: Option<&str>) -> Option<u32> {
        let listed = self.players.values().find(|p| p.name == name)?;
        let url = match listed.skin.as_ref() {
            Some(s) => s.cape.clone(),
            None => uuid.and_then(|u| self.textures.profile(u)).and_then(|s| s.cape),
        }?;
        self.textures.cape(&url)
    }

    pub fn local_skin(&mut self, r: &mut Renderer, jar: &mut Jar, name: &str, uuid: Option<&str>) -> Option<(u32, bool)> {
        self.textures.poll(r);
        let listed = self.players.iter().find(|(_, p)| p.name == name).map(|(u, p)| (*u, p.skin.clone()));
        let own_uuid = listed.as_ref().map(|l| l.0).or_else(|| uuid.and_then(|u| u128::from_str_radix(u, 16).ok()).map(Uuid));
        let skin = match listed.and_then(|l| l.1) {
            Some(s) => Some(s),
            None => uuid.and_then(|u| self.textures.profile(u)),
        };
        if let Some(s) = skin {
            if let Some(id) = self.textures.skin(&s.url) {
                return Some((id, s.slim));
            }
        }
        let (path, slim) = default_skin(own_uuid.unwrap_or(Uuid(0)));
        self.textures.jar(r, jar, path).map(|id| (id, slim))
    }
}

impl Entities {
    /// Cache de texturas (jar e skins) compartilhado com os tile entities.
    /// Esquece os nomes guardados: o layout deles depende da fonte, que muda com os pacotes de recursos.
    pub fn clear_names(&mut self) {
        self.names.clear();
    }

    pub fn textures_mut(&mut self) -> &mut Textures {
        &mut self.textures
    }

    /// Os mapas do 0x34 e as texturas juntos (mapa na mão).
    pub fn maps_textures(&mut self) -> (&mut crate::maps::Maps, &mut Textures) {
        (&mut self.cache.maps, &mut self.textures)
    }

    /// Esquece as entidades (mundo novo); a lista de jogadores continua.
    pub fn clear(&mut self) {
        self.list.clear();
    }

    pub fn tick(&mut self) {
        self.list.values_mut().for_each(Entity::tick);
        self.list.retain(|_, e| !e.obj.expired);
    }

    /// 0x34: muda o `MapData` (a textura é refeita no próximo desenho).
    pub fn apply_map(&mut self, p: crate::maps::MapPacket) {
        self.cache.maps.entry(p.id).or_default().apply(&p);
    }

    /// `EntityWither.onLivingUpdate`: cada cabeça lateral com alvo (metadado 18 e 19) gira para os olhos dele, a
    /// partir do próprio ponto (`func_82214_u/v/w`), até 10° de giro e 40° de inclinação por tick (`func_82204_b`).
    /// `own` é o jogador local (pés e altura dos olhos), que não está na lista.
    pub fn tick_wither_heads(&mut self, own: (DVec3, f64)) {
        let step = |a: f32, b: f32, max: f32| a + crate::mth::wrap_degrees(b - a).clamp(-max, max);
        let ids: Vec<i32> = self.list.iter().filter(|(_, e)| e.kind == Kind::Mob(64)).map(|(id, _)| *id).collect();
        for id in ids {
            for j in 0..2 {
                let Some(w) = self.list.get(&id) else { continue };
                let target = w.meta_int(18 + j as u8);
                if target <= 0 {
                    continue;
                }
                let eye = if Some(target) == self.own_id {
                    Some(own.0 + DVec3::new(0.0, own.1, 0.0))
                } else {
                    self.list.get(&target).map(|t| t.pos + DVec3::new(0.0, t.eye_height() as f64, 0.0))
                };
                let Some(eye) = eye else { continue };
                let a = (w.body_yaw + (180 * j) as f32) / 180.0 * std::f32::consts::PI;
                let head = DVec3::new(w.pos.x + crate::mth::cos(a) as f64 * 1.3, w.pos.y + 2.2, w.pos.z + crate::mth::sin(a) as f64 * 1.3);
                let d = eye - head;
                let flat = (d.x * d.x + d.z * d.z).sqrt();
                let yaw = (d.z.atan2(d.x) * 180.0 / std::f64::consts::PI) as f32 - 90.0;
                let pitch = -(d.y.atan2(flat) * 180.0 / std::f64::consts::PI) as f32;
                let s = &mut self.list.get_mut(&id).unwrap().mob_state;
                s.wither_pitch[j] = step(s.wither_pitch[j], pitch, 40.0);
                s.wither_yaw[j] = step(s.wither_yaw[j], yaw, 10.0);
            }
        }
    }

    /// `EntityGuardian`: o alvo novo no metadado 17 zera o tempo de ataque (`func_145781_i`), que sobe até
    /// `func_175464_ck` (80, ou 60 no ancião) enquanto há alvo.
    pub fn tick_guardians(&mut self) {
        for e in self.list.values_mut() {
            if e.kind != Kind::Mob(68) {
                continue;
            }
            let target = e.meta_int(17);
            if target != e.attack_target {
                e.attack_target = target;
                e.attack_time = 0;
            }
            let dur = if e.meta_int(16) & 4 != 0 { 60 } else { 80 };
            if target != 0 && e.attack_time < dur {
                e.attack_time += 1;
            }
        }
    }

    /// `EntityPlayer.onUpdate` (contagem do `itemInUse`) e `EntityOtherPlayerMP.onLivingUpdate` (o bit 4 do
    /// metadado 0 começa e termina o uso) dos outros jogadores, e quem tem anzol na água.
    pub fn tick_players(&mut self) {
        let anglers: Vec<i32> = self.list.values().filter_map(|e| match e.kind {
            Kind::Object { kind: crate::naovivas::FISH_HOOK, data } => Some(data),
            _ => None,
        }).collect();
        for e in self.list.values_mut() {
            if !matches!(e.kind, Kind::Player { .. }) {
                continue;
            }
            e.fishing = anglers.contains(&e.id);
            // EntityOtherPlayerMP.onLivingUpdate: cameraYaw pela velocidade, só no chão e vivo.
            e.prev_camera_yaw = e.camera_yaw;
            let dead = e.dead || matches!(e.meta.get(&6), Some(MetaValue::Float(h)) if *h <= 0.0);
            let mut f = (e.vel.x * e.vel.x + e.vel.z * e.vel.z).sqrt() as f32;
            if f > 0.1 {
                f = 0.1;
            }
            if !e.on_ground || dead {
                f = 0.0;
            }
            e.camera_yaw += (f - e.camera_yaw) * 0.4;
            let pos = e.pos;
            crate::physics::chase(&mut e.prev_chasing, &mut e.chasing, pos);
            if e.item_use.is_some() {
                if e.held_changed {
                    e.item_use = None;
                } else if let Some(u) = &mut e.item_use {
                    u.2 -= 1;
                }
            }
            e.held_changed = false;
            let eating = e.meta_byte(0) & 0x10 != 0;
            match (&e.equipment[0], e.using_item, eating) {
                (Some(held), false, true) => {
                    let found = crate::item_use::ITEM_USE.iter().find(|u| u.0 == held.id);
                    let (action, dur) = found.map_or((None, 0), |u| (Some(u.1), u.2));
                    e.item_use = Some((action, dur, dur));
                    e.using_item = true;
                }
                (_, true, false) => {
                    e.item_use = None;
                    e.using_item = false;
                }
                _ => {}
            }
        }
    }

    /// `tick` com o mundo: marca `inWater` dos mobs (`Entity.handleWaterMovement`) antes do tick.
    pub fn tick_in(&mut self, w: &World) {
        // EntityPickupFX.onUpdate: 3 ticks e some.
        for e in self.list.values_mut() {
            if let Some((_, age)) = &mut e.obj.pickup {
                *age += 1;
                if *age >= 3 {
                    e.obj.expired = true;
                }
            }
        }
        self.list.retain(|_, e| !(e.obj.pickup.is_some() && e.obj.expired));
        let one = |e: &mut Entity| {
            if e.obj.pickup.is_some() {
                return;
            }
            if matches!(e.kind, Kind::Mob(_)) {
                let (width, height) = e.size();
                let (hw, p) = (width as f64 / 2.0, e.pos);
                let bb = crate::physics::Bb { min: DVec3::new(p.x - hw, p.y, p.z - hw), max: DVec3::new(p.x + hw, p.y + height as f64, p.z + hw) };
                e.in_water = crate::physics::box_in_water(w, &bb);
            }
            e.tick();
        };
        // Cada entidade só mexe em si e lê o mundo: com muitas (TNT em massa, animações de vitória) o tick é
        // dividido entre os núcleos.
        if self.list.len() >= PARALLEL_TICK {
            use rayon::prelude::*;
            self.list.par_iter_mut().for_each(|(_, e)| one(e));
        } else {
            self.list.values_mut().for_each(one);
        }
        // EntityLightningBolt.setDead: o raio que acabou os clarões sai do mundo (antes ficava parado no céu).
        self.list.retain(|_, e| !e.obj.expired);
    }

    fn spawn(&mut self, e: Entity) {
        self.list.insert(e.id, e);
    }

    /// Gancho do mundo de teste (`MINE_RS_TEST_MOBS=id[:meta16][:idx=valor]...,id...`): põe mobs em
    /// fileiras de 6 à frente do jogador (z = 4, 7, ...), virados para ele, sem rede. `idx=valor`
    /// grava um metadado: com `.` é float, com `i` no fim é int, com `s` na frente é texto, senão byte.
    pub fn spawn_test_mobs(&mut self, spec: &str) {
        for (n, item) in spec.split(',').filter(|s| !s.is_empty()).enumerate() {
            let mut parts = item.split(':');
            let Ok(id) = parts.next().unwrap_or("").trim().parse::<u8>() else { continue };
            let (col, row) = ((n % 6) as f64, (n / 6) as f64);
            let knob = |k: &str, d: f64| std::env::var(k).ok().and_then(|v| v.parse::<f64>().ok()).unwrap_or(d);
            let (dx, dz, z0) = (knob("MINE_RS_TEST_MOBS_DX", 2.2), knob("MINE_RS_TEST_MOBS_DZ", 3.0), knob("MINE_RS_TEST_MOBS_Z", 4.0));
            let (x, z) = (0.5 + (col - 2.5) * dx, z0 + row * dz);
            let y = knob("MINE_RS_TEST_MOBS_Y", 64.0);
            let fixed = [(x * 32.0) as i32, (y * 32.0) as i32, (z * 32.0) as i32];
            // Gancho: 250 = armor stand (objeto 78), 251 = cristal do End (objeto 51).
            let kind = match id {
                250 => Kind::Object { kind: ARMOR_STAND, data: 0 },
                251 => Kind::Object { kind: 51, data: 0 },
                // 249 = outro jogador ("Inimigo<n>", skin padrão).
                249 => Kind::Player { uuid: Uuid(1000 + n as u128), name: format!("Inimigo{n}"), skin: None },
                _ => Kind::Mob(id),
            };
            let mut e = Entity::new(-100 - n as i32, kind, fixed, 180.0, 0.0);
            for p in parts {
                if let Some((k, v)) = p.split_once('=') {
                    let Ok(k) = k.trim().parse::<u8>() else { continue };
                    let v = v.trim();
                    let val = if let Some(t) = v.strip_prefix('s') {
                        MetaValue::Str(t.to_string())
                    } else if v.contains('.') {
                        MetaValue::Float(v.parse().unwrap_or(0.0))
                    } else if let Some(i) = v.strip_suffix('i') {
                        MetaValue::Int(i.parse().unwrap_or(0))
                    } else {
                        MetaValue::Byte(v.parse().unwrap_or(0))
                    };
                    e.meta.insert(k, val);
                } else if let Ok(m) = p.trim().parse::<i8>() {
                    e.meta.insert(16, MetaValue::Byte(m));
                }
            }
            // Gancho: `250=1` num golem de ferro liga a rosa (o servidor mandaria o status 11).
            if id == 99 && e.meta_byte(250) != 0 {
                e.mob_state.golem_rose = 400;
            }
            e.equipment = crate::test_world::equipment().map(|s| s.map(|s| Slot { id: s.id, count: s.count as i8, damage: s.damage, nbt: (!s.raw_nbt.is_empty()).then_some(s.raw_nbt) }));
            // MINE_RS_TEST_FX=fogo,dano,nome,invisivel,agachado: efeitos de renderizador em todos os mobs de teste.
            for fx in crate::test_world::env("FX").unwrap_or_default().split(',') {
                match fx.trim() {
                    "fogo" => e.meta.insert(0, MetaValue::Byte(e.meta_byte(0) | 1)),
                    "invisivel" => e.meta.insert(0, MetaValue::Byte(e.meta_byte(0) | 0x20)),
                    // Alguém montado (o próprio jogador de teste, id -2): rédeas do cavalo selado.
                    "montado" => {
                        e.ridden_by = Some(-2);
                        None
                    }
                    "dano" => {
                        e.obj.fixed_age = Some(-1);
                        None
                    }
                    "nome" => {
                        e.meta.insert(3, MetaValue::Byte(1));
                        e.meta.insert(2, MetaValue::Str(format!("Mob {n}")))
                    }
                    _ => None,
                };
            }
            self.spawn(e);
        }
    }

    /// Gancho de teste `MINE_RS_TEST_LACO=bx,by,bz`: um nó no bloco dado (a cerca vem de `MINE_RS_TEST_BLOCOS`),
    /// uma vaca presa ao nó e um porco preso ao próprio jogador (id -2).
    pub fn spawn_test_leash(&mut self, spec: &str) {
        let v: Vec<i32> = spec.split(',').filter_map(|s| s.trim().parse().ok()).collect();
        let [bx, by, bz] = v[..] else { return };
        let mut knot = Entity::new(-200, Kind::Object { kind: crate::naovivas::LEASH_KNOT, data: 0 }, [bx * 32, by * 32, bz * 32], 0.0, 0.0);
        place_knot(&mut knot);
        self.spawn(knot);
        let at = |x: f64, z: f64| [(x * 32.0) as i32, by * 32, (z * 32.0) as i32];
        let mut cow = Entity::new(-201, Kind::Mob(92), at(bx as f64 + 2.5, bz as f64 + 0.5), 180.0, 0.0);
        cow.leash_to = Some(-200);
        self.spawn(cow);
        let mut pig = Entity::new(-202, Kind::Mob(90), at(-2.5, 2.5), 90.0, 0.0);
        pig.leash_to = Some(-2);
        self.spawn(pig);
    }

    /// Gancho de teste `MINE_RS_TEST_OBJ` (ver `naovivas::test_entities`).
    pub fn spawn_test_objects(&mut self, spec: &str) {
        for e in crate::naovivas::test_entities(spec) {
            self.spawn(e);
        }
    }

    /// `onUpdate` das entidades não-vivas que olham o mundo (flecha cravada).
    pub fn tick_objects(&mut self, world: &World, table: &mc_world::blocks::BlockTable) {
        let one = |e: &mut Entity| {
            if e.obj.pickup.is_none() {
                crate::naovivas::tick_world(e, world, table);
            }
        };
        if self.list.len() >= PARALLEL_TICK {
            use rayon::prelude::*;
            self.list.par_iter_mut().for_each(|(_, e)| one(e));
        } else {
            self.list.values_mut().for_each(one);
        }
    }

    fn apply_meta(e: &mut Entity, meta: Vec<(u8, MetaValue)>) {
        e.meta.extend(meta);
    }

    /// Aplica um pacote como o `NetHandlerPlayClient`.
    pub fn handle(&mut self, p: EntityPacket, player: &mut Player) {
        match p {
            EntityPacket::JoinGame { entity_id } => {
                self.own_id = Some(entity_id);
                self.list.clear();
            }
            EntityPacket::SpawnPlayer { id, uuid, x, y, z, yaw, pitch, item, meta } => {
                let info = self.players.get(&uuid);
                let name = info.map(|i| i.name.clone()).unwrap_or_default();
                let skin = info.and_then(|i| i.skin.clone());
                let mut e = Entity::new(id, Kind::Player { uuid, name, skin }, [x, y, z], angle(yaw), angle(pitch));
                if item != 0 {
                    e.equipment[0] = Some(Slot { id: item, count: 1, damage: 0, nbt: None });
                }
                Self::apply_meta(&mut e, meta);
                self.spawn(e);
            }
            EntityPacket::SpawnObject { id, kind, x, y, z, pitch, yaw, data, vel } => {
                let mut e = Entity::new(id, Kind::Object { kind, data }, [x, y, z], angle(yaw), angle(pitch));
                if kind == crate::naovivas::LEASH_KNOT {
                    place_knot(&mut e);
                }
                if kind == crate::naovivas::ITEM_FRAME {
                    // O servidor manda o bloco pendurado x32 e a direção em `data`.
                    e.obj.hanging = [x.div_euclid(32), y.div_euclid(32), z.div_euclid(32)];
                    e.obj.facing = (data & 3) as u8;
                }
                if let Some((vx, vy, vz)) = vel {
                    e.vel = DVec3::new(vx as f64, vy as f64, vz as f64) / 8000.0;
                }
                // handleSpawnObject: a bola de fogo usa a velocidade do pacote como aceleração (normalizada para
                // 0.1, `EntityFireball(world, x, y, z, ax, ay, az)`) e começa parada.
                if matches!(kind, crate::naovivas::LARGE_FIREBALL | crate::naovivas::SMALL_FIREBALL | crate::naovivas::WITHER_SKULL) {
                    let a = e.vel;
                    let len = a.length();
                    e.obj.accel = if len > 0.0 { a / len * 0.1 } else { DVec3::ZERO };
                    e.vel = DVec3::ZERO;
                }
                self.spawn(e);
            }
            EntityPacket::SpawnMob { id, kind, x, y, z, yaw, pitch, head_yaw, vel, meta } => {
                // EntityList.createEntityByID: o 30 é o EntityArmorStand (hologramas chegam assim pelo S0F).
                let kind = if kind == 30 { Kind::Object { kind: ARMOR_STAND, data: 0 } } else { Kind::Mob(kind) };
                let mut e = Entity::new(id, kind, [x, y, z], angle(yaw), angle(pitch));
                e.body_yaw = angle(head_yaw);
                e.head_yaw = e.body_yaw;
                e.prev_body_yaw = e.body_yaw;
                e.prev_head_yaw = e.body_yaw;
                e.body_prev_head = e.body_yaw;
                e.vel = DVec3::new(vel.0 as f64, vel.1 as f64, vel.2 as f64) / 8000.0;
                Self::apply_meta(&mut e, meta);
                self.spawn(e);
            }
            // Entity.mountEntity: o veículo guarda quem monta; desmontar limpa o anterior.
            EntityPacket::Attach { entity, vehicle, leash: false } => {
                for e in self.list.values_mut() {
                    if e.ridden_by == Some(entity) {
                        e.ridden_by = None;
                    }
                }
                if let Some(v) = self.list.get_mut(&vehicle) {
                    v.ridden_by = Some(entity);
                }
                if Some(entity) == self.own_id {
                    player.riding = (vehicle != -1).then_some(vehicle);
                }
            }
            // NetHandlerPlayClient.handleEntityAttach com leash = 1: `setLeashedToEntity` com o vizinho (o próprio
            // jogador também conta, `getEntityByID`), ou `clearLeashed` se o id é -1 ou não existe. Só `EntityLiving`
            // (os mobs) recebe a coleira.
            EntityPacket::Attach { entity, vehicle, leash: true } => {
                let holder = (Some(vehicle) == self.own_id || self.list.contains_key(&vehicle)).then_some(vehicle);
                if let Some(e) = self.list.get_mut(&entity).filter(|e| matches!(e.kind, Kind::Mob(_))) {
                    e.leash_to = holder;
                }
            }
            EntityPacket::Destroy(ids) => {
                if ids.iter().any(|id| Some(*id) == player.riding) {
                    player.riding = None;
                }
                // O item coletado já saiu do mundo no S0D: a animação de coleta segue até o fim.
                for id in ids {
                    if self.list.get(&id).is_some_and(|e| e.obj.pickup.is_none()) {
                        self.list.remove(&id);
                    }
                }
            }
            EntityPacket::Move { id, dx, dy, dz, rot, on_ground } => {
                if let Some(e) = self.list.get_mut(&id) {
                    e.server[0] += dx as i32;
                    e.server[1] += dy as i32;
                    e.server[2] += dz as i32;
                    let (yaw, pitch) = rot.map_or((e.yaw, e.pitch), |(y, p)| (angle(y), angle(p)));
                    let target = e.server_pos();
                    e.set_target(target, yaw, pitch);
                    e.on_ground = on_ground;
                }
            }
            EntityPacket::Teleport { id, x, y, z, yaw, pitch, on_ground } => {
                if let Some(e) = self.list.get_mut(&id) {
                    e.server = [x, y, z];
                    let t = e.server_pos();
                    // Diferença menor que a precisão do pacote: só gira.
                    let near = (e.pos.x - t.x).abs() < 0.03125 && (e.pos.y - t.y).abs() < 0.015625 && (e.pos.z - t.z).abs() < 0.03125;
                    let target = if near { e.pos } else { t };
                    e.set_target(target, angle(yaw), angle(pitch));
                    e.on_ground = on_ground;
                }
            }
            EntityPacket::HeadLook { id, yaw } => {
                if let Some(e) = self.list.get_mut(&id) {
                    e.head_yaw = angle(yaw);
                }
            }
            EntityPacket::Metadata { id, meta } => {
                if let Some(e) = self.list.get_mut(&id) {
                    Self::apply_meta(e, meta);
                }
            }
            EntityPacket::Animation { id, kind } => {
                if let Some(e) = self.list.get_mut(&id) {
                    match kind {
                        0 => e.swing_arm(),
                        1 => e.hurt_time = 10,
                        _ => {}
                    }
                }
            }
            EntityPacket::Velocity { id, vx, vy, vz } => {
                let v = DVec3::new(vx as f64, vy as f64, vz as f64) / 8000.0;
                if Some(id) == self.own_id {
                    // Knockback no próprio jogador (`setVelocity`).
                    player.vel = v;
                } else if let Some(e) = self.list.get_mut(&id) {
                    e.vel = v;
                }
            }
            EntityPacket::Equipment { id, slot, item } => {
                if let Some(e) = self.list.get_mut(&id) {
                    if let Some(s) = e.equipment.get_mut(slot as usize) {
                        *s = item;
                    }
                    // O 0x04 põe outro ItemStack na mão: o `itemInUse` deixa de ser o item atual.
                    if slot == 0 {
                        e.held_changed = true;
                    }
                }
            }
            EntityPacket::Status { id, status } if Some(id) == self.own_id => match status {
                // Dano no próprio jogador (câmera de dano) e fim do uso do item (comer, beber).
                2 => player.hurt_status(),
                9 => player.item_use = None,
                22 => player.reduced_debug = true,
                23 => player.reduced_debug = false,
                _ => {}
            },
            EntityPacket::Status { id, status } => {
                if let Some(e) = self.list.get_mut(&id) {
                    if let Kind::Mob(id) = e.kind {
                        e.mob_state.status(id, status);
                    }
                    match status {
                        2 => {
                            e.limb_amount = 1.5;
                            e.hurt_time = 10;
                        }
                        3 => e.dead = true,
                        _ => {}
                    }
                }
            }
            EntityPacket::PlayerList(actions) => {
                if std::env::var_os("MINE_RS_STATS_ENT").is_some() {
                    let names: Vec<String> = actions.iter().map(|a| match a { PlayerListAction::Add { name, .. } => format!("+{name}"), PlayerListAction::Remove { uuid } => format!("-{}", self.players.get(uuid).map_or("?", |p| p.name.as_str())), _ => String::new() }).filter(|s| !s.is_empty()).collect();
                    if !names.is_empty() { println!("[stats] s38 {}", names.join(" ")); }
                }
                actions.into_iter().for_each(|a| self.player_list(a))
            }
            EntityPacket::SpawnXpOrb { id, x, y, z, value } => {
                self.spawn(Entity::new(id, Kind::Object { kind: crate::naovivas::XP_ORB, data: value as i32 }, [x, y, z], 0.0, 0.0));
            }
            EntityPacket::SpawnPainting { id, title, pos, facing } => {
                // `new EntityPainting(world, pos, facing, title)`: a arte pelo título e a caixa pela arte.
                let idx = crate::naovivas::art_index(&title);
                let art = crate::gen_paintings::ARTS[idx];
                let hanging = [pos.0, pos.1, pos.2];
                let p = crate::naovivas::hanging_pos(hanging, facing, art.size_x as i32, art.size_y as i32);
                let fixed = [(p[0] * 32.0) as i32, (p[1] * 32.0) as i32, (p[2] * 32.0) as i32];
                let mut e = Entity::new(id, Kind::Object { kind: crate::naovivas::PAINTING, data: idx as i32 | (facing as i32 & 3) << 8 }, fixed, (facing & 3) as f32 * 90.0, 0.0);
                e.pos = DVec3::new(p[0], p[1], p[2]);
                e.prev = e.pos;
                e.obj.hanging = hanging;
                e.obj.facing = facing & 3;
                self.spawn(e);
            }
            EntityPacket::SpawnGlobal { id, kind: 1, x, y, z } => {
                self.spawn(Entity::new(id, Kind::Object { kind: crate::naovivas::LIGHTNING, data: 0 }, [x, y, z], 0.0, 0.0));
            }
            EntityPacket::SpawnGlobal { .. } => {}
        }
    }

    fn player_list(&mut self, a: PlayerListAction) {
        match a {
            PlayerListAction::Add { uuid, name, properties, gamemode, ping, display_name } => {
                let skin = skin_from_properties(&properties);
                self.players.insert(uuid, PlayerInfo { name, display_name, ping, gamemode, skin, properties });
            }
            PlayerListAction::GameMode { uuid, gamemode } => {
                if let Some(p) = self.players.get_mut(&uuid) {
                    p.gamemode = gamemode;
                }
            }
            PlayerListAction::Latency { uuid, ping } => {
                if let Some(p) = self.players.get_mut(&uuid) {
                    p.ping = ping;
                }
            }
            PlayerListAction::DisplayName { uuid, display_name } => {
                if let Some(p) = self.players.get_mut(&uuid) {
                    p.display_name = display_name;
                }
            }
            PlayerListAction::Remove { uuid } => {
                self.players.remove(&uuid);
            }
        }
    }

    /// Geometria do quadro: modelos (por textura) e nomes (translúcidos, no atlas da GUI).
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        cam: &Camera,
        partial: f32,
        world: &World,
        r: &mut Renderer,
        jar: &mut Jar,
        ui: &mut UiBatch,
        max_dist: f32,
        items: &mut crate::first_person::ItemModels,
        table: &mc_world::blocks::BlockTable,
        // O próprio jogador, que não está na lista: é o segurador com id `own_id`.
        own: Option<Anchor>,
    ) -> Vec<DrawItem> {
        let Self { list, players, textures, cache, world_time, viewer_spectator, entity_shadows, rendered, gui_enabled, names, own_id, board, hidden_view, front_view, .. } = self;
        let time = *world_time;
        let mut glow: HashMap<Tex, Geometry> = HashMap::new();
        textures.poll(r);
        let white = textures.white(r);
        let mut opaque: HashMap<Tex, Geometry> = HashMap::new();
        // (distância², texto, base, agachado).
        let mut labels: Vec<(f32, Cow<str>, Vec3, bool)> = Vec::new();
        let t = partial as f64;
        *rendered = 0;
        let planes = frustum_planes(cam, r.size());
        // TNT e blocos caindo vão para `pure` e são desenhados no fim, sem passar pelo `draw_model` dos vivos.
        let shadow_tex = if *entity_shadows { textures.jar(r, jar, "misc/shadow") } else { None };
        let mut pure: Vec<(&Entity, Vec3, DVec3, f32, [u8; 4])> = Vec::new();
        for e in list.values() {
            if Some(e.id) == *hidden_view {
                continue;
            }
            // EntityPickupFX.renderParticle: do item até quem coletou (+0.5), com f² de (idade + parcial) / 3.
            let pos = match e.obj.pickup {
                Some((by, age)) => {
                    let target = match list.get(&by).filter(|_| Some(by) != *own_id) {
                        Some(c) => c.prev.lerp(c.pos, t),
                        None => own.as_ref().map_or(e.pos, |a| a.prev.lerp(a.pos, t)),
                    } + DVec3::new(0.0, 0.5, 0.0);
                    let f = ((age as f32 + partial) / 3.0).powi(2) as f64;
                    e.pos + (target - e.pos) * f
                }
                None => e.prev.lerp(e.pos, t),
            };
            let p = pos.as_vec3();
            let d2 = (p - cam.eye).length_squared();
            if d2 > max_dist * max_dist {
                continue;
            }
            // RenderGlobal.renderEntities: `shouldRender` ou o veículo do jogador; fora disso a entidade não é
            // desenhada (nem o nome nem a sombra).
            let carries_player = own_id.is_some() && e.ridden_by == *own_id;
            // Quem segura a coleira (`getLeashedToEntity`), se ainda está na lista ou é o próprio jogador.
            let holder = e.leash_to.and_then(|id| holder_of(list, *own_id, own, id));
            if !carries_player && !planes.as_ref().map_or(true, |pl| should_render(pl, e, list, holder.as_ref())) {
                continue;
            }
            *rendered += 1;
            let burning = e.meta_byte(0) & 1 != 0;
            // RenderManager: entidade em chamas usa a luz máxima (15728880).
            let light = if burning { FULL_LIGHT } else { light_at(world, pos + DVec3::new(0.0, e.eye_height() as f64, 0.0)) };
            if let Kind::Object { kind, .. } = e.kind {
                if crate::naovivas::is_pure(kind) && !e.invisible() && holder.is_none() && e.label().is_none() && crate::fx::tint_code(e, partial).is_none() {
                    pure.push((e, p, pos, d2, light));
                    continue;
                }
            }
            let living = matches!(e.kind, Kind::Player { .. } | Kind::Mob(_)) || matches!(e.kind, Kind::Object { kind: ARMOR_STAND, .. });
            // A tinta só precisa saber o que este desenho acrescentou: o instantâneo só é tirado quando há tinta.
            let tint = crate::fx::tint_code(e, partial);
            let before = tint.map(|_| crate::fx::snapshot(&opaque));
            // Invisível: sumido; só o espectador vê, com alfa 0,15 (`renderModel` com `isInvisibleToPlayer` falso).
            let ghost = e.invisible() && living && *viewer_spectator;
            // RendererLivingEntity.doRender: invisível pula só o renderModel; as camadas (cabeça no capacete,
            // armadura, item na mão) continuam. É assim que os lobbies fazem as cabeças 3D girando (suportes de
            // armadura invisíveis com uma cabeça) e os itens flutuando.
            let layers_only = e.invisible() && !ghost && matches!(e.kind, Kind::Player { .. } | Kind::Object { kind: ARMOR_STAND, .. });
            if !e.invisible() || ghost || layers_only {
                let mut scratch: HashMap<Tex, Geometry> = HashMap::new();
                let target = if ghost { &mut scratch } else { &mut opaque };
                // `Entity.getBrightness`: luz no ponto dos olhos com o escurecimento do dia (light = [céu, bloco]).
                let brightness = crate::hud_stats::brightness_at((light[1] / 17) as i32, (light[0] / 17) as i32, time);
                draw_model(e, &mut crate::naovivas::Fx { cam, world, glow: &mut glow, time, cache, body: !layers_only }, p, partial, light, brightness, textures, r, jar, players, white, items, table, crate::models::WORLD_LIGHTS, target);
                if ghost {
                    for (tex, mut geo) in scratch {
                        geo.vertices.iter_mut().for_each(|v| v.color[3] = 38);
                        let tex = match tex {
                            Tex::Custom(id) => Tex::Custom(id | crate::mobs::TRANSLUCENT),
                            o => o,
                        };
                        let dst = opaque.entry(tex).or_default();
                        let base = dst.vertices.len() as u32;
                        dst.vertices.extend(geo.vertices);
                        dst.indices.extend(geo.indices.iter().map(|i| i + base));
                    }
                }
            }
            if let (Some((code, all)), Some(before)) = (tint, before) {
                crate::fx::apply_tint(&mut opaque, &before, code, all);
            }
            // RenderLiving.doRender: a corda sai depois do modelo e fora da tinta de dano; sai também com o bicho
            // invisível (o modelo é que some).
            if let Some(h) = holder {
                crate::naovivas::leash_rope(opaque.entry(Tex::Custom(white)).or_default(), e, &h, partial, [0.5, 0.5]);
            }
            if !e.invisible() {
                let (w, h) = e.size();
                if *entity_shadows {
                    let (size, op) = crate::fx::shadow_of(e);
                    let strength = crate::fx::shadow_strength(d2 as f64, op);
                    if size > 0.0 && strength > 0.0 {
                        if let Some(tex) = textures.jar(r, jar, "misc/shadow") {
                            let geo = opaque.entry(Tex::Custom(tex | crate::mobs::TRANSLUCENT)).or_default();
                            crate::fx::shadow(geo, world, table, pos, size, strength, time, light);
                        }
                    }
                }
                if burning {
                    crate::fx::fire(opaque.entry(Tex::Blocks).or_default(), table, p, w, h, cam.yaw);
                }
            }
            // RendererLivingEntity.canRenderName pede isGuiEnabled; o suporte de armadura tem o seu.
            let living_name = matches!(e.kind, Kind::Player { .. } | Kind::Mob(_));
            // Jogadores: RendererLivingEntity.canRenderName com o time (ALWAYS mostra até invisível) e o texto
            // getDisplayName (prefixo + nome + sufixo); RenderPlayer põe o placar abaixo do nome a menos de 10 blocos.
            let player_label = match &e.kind {
                Kind::Player { name, .. } if !name.is_empty() => {
                    let team = board.player_team.get(name).and_then(|t| board.teams.get(t).map(|tt| (t, tt)));
                    let show = match team {
                        Some((t, (_, _, vis))) => match vis {
                            1 => false,
                            2 => board.own_team.as_ref().is_none_or(|o| o == t),
                            3 => board.own_team.as_ref().is_none_or(|o| o != t),
                            _ => true,
                        },
                        None => *gui_enabled && !e.invisible() && e.ridden_by.is_none(),
                    };
                    let text = match team {
                        Some((_, (pre, suf, _))) => format!("{pre}{name}{suf}"),
                        None => name.clone(),
                    };
                    Some(show.then(|| (Cow::Owned(text), if e.sneaking() { 32.0 } else { 64.0 })))
                }
                _ => None,
            };
            let label = match player_label {
                Some(l) => l,
                None => e.label().filter(|_| *gui_enabled || !living_name).map(|(t, r)| (Cow::Borrowed(t), r)),
            };
            if let Some((text, range)) = label {
                if d2 < range * range {
                    let (_, h) = e.size();
                    let mut y = p.y + h + 0.5;
                    if e.child() {
                        y -= h / 2.0;
                    }
                    if matches!(e.kind, Kind::Player { .. }) && e.sneaking() {
                        y -= 0.125 + 0.25;
                    }
                    if let (Kind::Player { name, .. }, Some((disp, scores)), false) = (&e.kind, &board.below, e.sneaking()) {
                        if d2 < 100.0 {
                            let score = scores.get(name).copied().unwrap_or(0);
                            labels.push((d2, Cow::Owned(format!("{score} {disp}")), Vec3::new(p.x, y, p.z), false));
                            y += 9.0 * 1.15 * 0.026_666_668;
                        }
                    }
                    labels.push((d2, text, Vec3::new(p.x, y, p.z), e.sneaking()));
                }
            }
        }
        draw_pure_batch(&pure, &mut opaque, &mut glow, partial, time, white, world, table, shadow_tex, cam.yaw);
        self.additive = glow.into_iter().map(|(tex, geo)| DrawItem { tex, geo, translucent: true }).collect();
        self.overlay.clear();
        let mut items: Vec<DrawItem> = opaque
            .into_iter()
            .map(|(tex, geo)| match tex {
                Tex::Custom(id) if id & crate::mobs::TRANSLUCENT != 0 => DrawItem { tex: Tex::Custom(id & !crate::mobs::TRANSLUCENT), geo, translucent: true },
                // O renderer lê o bit aditivo no id e usa o pipeline `blendFunc(1, 1)`.
                Tex::Custom(id) if id & crate::mobs::ADDITIVE != 0 => DrawItem { tex, geo, translucent: true },
                _ => DrawItem { tex, geo, translucent: false },
            })
            .collect();
        if !labels.is_empty() {
            labels.sort_by(|a, b| b.0.total_cmp(&a.0));
            // renderLivingLabel: fundo e texto fraco sem profundidade (`overlay`), depois o texto branco com profundidade.
            // Agachado (RendererLivingEntity.renderName): só fundo e texto fraco, com teste de profundidade.
            let (mut near, mut through) = (Geometry::default(), Geometry::default());
            for (_, text, at, sneak) in &labels {
                let q = name_quads(names, ui, text.as_ref());
                // RenderManager.cacheActiveRenderInfo: na visão de frente (F5 duas vezes) playerViewY += 180.
                let vy = cam.yaw + if *front_view { 180.0 } else { 0.0 };
                if *sneak {
                    name_tag(&mut near, &q.bg, *at, vy, cam.pitch, 1.0);
                    name_tag(&mut near, &q.dim, *at, vy, cam.pitch, 1.0);
                } else {
                    name_tag(&mut through, &q.bg, *at, vy, cam.pitch, 1.0);
                    name_tag(&mut through, &q.dim, *at, vy, cam.pitch, 1.0);
                    name_tag(&mut near, &q.bright, *at, vy, cam.pitch, 1.0);
                }
            }
            if !through.is_empty() {
                self.overlay.push(DrawItem { tex: Tex::Ui, geo: through, translucent: true });
            }
            items.push(DrawItem { tex: Tex::Ui, geo: near, translucent: true });
        }
        items
    }
}

/// O segurador pelo id: o próprio jogador (fora da lista) ou uma entidade da lista (`getEntityByID`).
fn holder_of(list: &HashMap<i32, Entity>, own_id: Option<i32>, own: Option<Anchor>, id: i32) -> Option<Anchor> {
    if Some(id) == own_id {
        own
    } else {
        list.get(&id).map(Anchor::of)
    }
}

/// Planos do frustum da câmera, como `Renderer::render` (visão vezes projeção); `None` sem janela válida.
fn frustum_planes(cam: &Camera, (w, h): (u32, u32)) -> Option<[Vec4; 6]> {
    if h == 0 {
        return None;
    }
    let proj = Mat4::perspective_rh(cam.fov_deg.to_radians(), w as f32 / h as f32, 0.05, cam.view_distance * std::f32::consts::SQRT_2);
    let view = cam.view.unwrap_or_else(|| Mat4::look_to_rh(cam.eye, cam.forward(), Vec3::Y));
    let vp = proj * view;
    let r = [vp.row(0), vp.row(1), vp.row(2), vp.row(3)];
    let planes = [r[3] + r[0], r[3] - r[0], r[3] + r[1], r[3] - r[1], r[2], r[3] - r[2]];
    planes.iter().all(|p| p.is_finite()).then_some(planes)
}

/// `Render.shouldRender` e as variantes: `ignoreFrustumCheck` (dragão e anzol) sempre desenha; o guardião
/// também desenha quando o raio até o alvo cruza o frustum (`RenderGuardian.shouldRender`).
fn should_render(planes: &[Vec4; 6], e: &Entity, list: &HashMap<i32, Entity>, holder: Option<&Anchor>) -> bool {
    if matches!(e.kind, Kind::Mob(63) | Kind::Object { kind: crate::naovivas::FISH_HOOK, .. }) {
        return true;
    }
    if in_frustum(planes, e) {
        return true;
    }
    // RenderLiving.shouldRender: fora do frustum, um bicho preso à coleira ainda desenha quando a caixa de quem
    // segura aparece.
    if holder.is_some_and(|h| box_in(planes, h.bb.0.as_vec3(), h.bb.1.as_vec3())) {
        return true;
    }
    if e.kind != Kind::Mob(68) || e.attack_target == 0 {
        return false;
    }
    let Some(t) = list.get(&e.attack_target) else { return false };
    let a = e.pos.as_vec3() + Vec3::new(0.0, e.eye_height(), 0.0);
    let b = t.pos.as_vec3() + Vec3::new(0.0, t.size().1 * 0.5, 0.0);
    a.is_finite() && b.is_finite() && box_in(planes, a.min(b), a.max(b))
}

/// A caixa da entidade (`getEntityBoundingBox`, posição atual, não a interpolada) cruza o frustum. Ela vai
/// alargada 0,5 em X e Z, uma folga a mais que o jogo: a posição aqui é a do servidor, à frente da desenhada.
/// Caixa sem tamanho vira ±2 em volta da posição, como no `Render.shouldRender`; sem números válidos, desenha.
fn in_frustum(planes: &[Vec4; 6], e: &Entity) -> bool {
    let (w, h) = e.size();
    let c = e.pos.as_vec3();
    if !c.is_finite() || !w.is_finite() || !h.is_finite() {
        return true;
    }
    if w == 0.0 && h == 0.0 {
        return box_in(planes, c - Vec3::splat(2.0), c + Vec3::splat(2.0));
    }
    let hw = w / 2.0 + 0.5;
    box_in(planes, Vec3::new(c.x - hw, c.y, c.z - hw), Vec3::new(c.x + hw, c.y + h, c.z + hw))
}

/// `ClippingHelper.isBoxInFrustum`: algum canto da caixa fica do lado de dentro de cada plano.
fn box_in(planes: &[Vec4; 6], min: Vec3, max: Vec3) -> bool {
    planes.iter().all(|p| {
        let v = Vec3::new(
            if p.x >= 0.0 { max.x } else { min.x },
            if p.y >= 0.0 { max.y } else { min.y },
            if p.z >= 0.0 { max.z } else { min.z },
        );
        p.truncate().dot(v) + p.w >= 0.0
    })
}

/// Cor das caixas dos mobs ainda sem modelo próprio.
fn mob_color(id: u8) -> [f32; 3] {
    match id {
        50 => [0.35, 0.75, 0.3],
        52 | 59 => [0.25, 0.2, 0.2],
        55 => [0.45, 0.8, 0.4],
        56 => [0.95, 0.95, 0.95],
        58 => [0.1, 0.05, 0.12],
        60 | 67 => [0.55, 0.55, 0.6],
        61 | 62 => [0.95, 0.6, 0.15],
        63 | 64 => [0.15, 0.15, 0.18],
        65 => [0.3, 0.25, 0.2],
        66 => [0.35, 0.25, 0.4],
        68 => [0.4, 0.6, 0.55],
        90 => [0.95, 0.65, 0.65],
        91 => [0.9, 0.9, 0.88],
        92 | 96 => [0.4, 0.28, 0.2],
        93 => [0.97, 0.97, 0.97],
        94 => [0.2, 0.3, 0.45],
        95 => [0.85, 0.85, 0.82],
        97 => [0.95, 0.97, 1.0],
        98 => [0.9, 0.75, 0.35],
        99 => [0.75, 0.72, 0.68],
        100 => [0.55, 0.4, 0.25],
        101 => [0.65, 0.55, 0.45],
        _ => [0.7, 0.7, 0.7],
    }
}

#[allow(clippy::too_many_arguments)]
/// Peça que um biped veste ou segura: id, dano, cor do couro (`display.color`) e o arco/bloqueio.
#[derive(Clone, Copy, Debug)]
pub struct Worn {
    pub id: i16,
    pub damage: i16,
    /// Modelo trocado (`first_person::entity_override`), só no item da mão.
    pub model: Option<&'static str>,
    pub color: Option<u32>,
    /// Variante de ícone (`tile_entities::entity_variant`) se o item é `builtin/entity` (baú, cabeça, estandarte).
    pub entity: Option<u32>,
}

impl Worn {
    pub(crate) fn from_slot(s: &Slot) -> Worn {
        let tag = s.nbt.as_ref().and_then(|raw| {
            let mut r = mc_protocol::Reader::new(raw);
            mc_protocol::nbt::read_root(&mut r).ok()?
        });
        let color = tag.as_ref().and_then(|n| match n.get("display")?.get("color")? {
            mc_protocol::nbt::Nbt::Int(c) => Some(*c as u32 & 0xFFFFFF),
            _ => None,
        });
        let entity = crate::tile_entities::entity_item_of(s.id, s.damage, tag.as_ref()).map(crate::tile_entities::entity_variant);
        Worn { id: s.id, damage: s.damage, model: None, color, entity }
    }
}

/// `LayerCustomHead`: o capacete que não é armadura — bloco (`ItemBlock`, `TransformType.HEAD`) ou cabeça
/// (`Items.skull`, `renderSkull` virado 180°) — preso no `postRender` da cabeça do modelo.
#[allow(clippy::too_many_arguments)]
fn custom_head(
    head: &crate::models::Part,
    mat: &Mat4,
    w: Worn,
    anim: &Anim,
    villager: bool,
    light: [u8; 4],
    lights: [Vec3; 2],
    items: &mut crate::first_person::ItemModels,
    table: &mc_world::blocks::BlockTable,
    textures: &mut Textures,
    r: &mut Renderer,
    jar: &mut Jar,
    out: &mut HashMap<Tex, Geometry>,
) {
    let mut m = *mat;
    if anim.sneak {
        m *= Mat4::from_translation(Vec3::new(0.0, 0.2, 0.0));
    }
    if !villager && anim.child {
        m *= Mat4::from_scale(Vec3::splat(1.4 / 2.0)) * Mat4::from_translation(Vec3::new(0.0, 1.0, 0.0));
    }
    m = head.matrix(&m);
    if villager {
        m *= Mat4::from_translation(Vec3::new(0.0, 0.1875, 0.0));
    }
    if (0..256).contains(&w.id) {
        let Some(model) = items.get(w.id, w.damage, jar, table, r) else { return };
        m *= Mat4::from_translation(Vec3::new(0.0, -0.25, 0.0)) * Mat4::from_rotation_y(std::f32::consts::PI) * Mat4::from_scale(Vec3::new(0.625, -0.625, -0.625));
        // ItemRenderer.renderItem(..., HEAD): 2x nos 3D, display.head e o 0,5 / -0,5 do RenderItem.renderItem.
        if model.gui3d {
            m *= Mat4::from_scale(Vec3::splat(2.0));
        }
        if let Some(d) = model.display_head {
            m *= d.matrix();
        }
        let m = m * Mat4::from_scale(Vec3::splat(0.5)) * Mat4::from_translation(Vec3::splat(-0.5));
        model.emit_raw(m, &lights, light, out.entry(model.tex).or_default());
    } else if w.id == 397 {
        let Some(item) = w.entity.and_then(crate::tile_entities::entity_from_variant) else { return };
        let crate::tile_entities::EntityItem::Skull { skull_type, .. } = &item else { return };
        let Some(tex) = crate::tile_entities::item_texture(&item, textures, r, jar) else { return };
        m *= Mat4::from_scale(Vec3::new(1.1875, -1.1875, -1.1875));
        if villager {
            m *= Mat4::from_translation(Vec3::new(0.0, 0.0625, 0.0));
        }
        // renderSkull(-0,5, 0, -0,5, UP, 180, ...): translate(x + 0,5, y, z + 0,5) e scale(-1, -1, 1).
        m *= Mat4::from_scale(Vec3::new(-1.0, -1.0, 1.0));
        let mut sink = Sink { geo: out.entry(Tex::Custom(tex)).or_default(), tex: (64.0, 64.0), tint: [1.0; 3], alpha: 255, light, lights };
        crate::tile_entities::SkullModel::new(matches!(skull_type, 2 | 3)).emit(&mut sink, &m, 180.0);
    }
}

/// `LayerBipedArmor` (capacete, peito, calça, botas), `LayerCustomHead` e `LayerHeldItem` sobre o modelo já posado.
/// `armor[i]`: 0 botas .. 3 capacete, como `getCurrentArmor`.
#[allow(clippy::too_many_arguments)]
pub fn biped_layers(
    model: &Biped,
    slim: bool,
    anim: &Anim,
    mat: &Mat4,
    held: Option<Worn>,
    armor: [Option<Worn>; 4],
    light: [u8; 4],
    tint: [f32; 3],
    lights: [Vec3; 2],
    villager: bool,
    repose: Option<&dyn Fn(&mut Biped)>,
    items: &mut crate::first_person::ItemModels,
    table: &mc_world::blocks::BlockTable,
    textures: &mut Textures,
    r: &mut Renderer,
    jar: &mut Jar,
    out: &mut HashMap<Tex, Geometry>,
) {
    use crate::item_use::{ARMOR, LEATHER_DEFAULT_COLOR};
    // doRenderLayer: capacete (4), peito (3), calça (2), botas (1).
    for slot in (1..=4).rev() {
        let Some(w) = armor[slot - 1] else { continue };
        let Some(&(_, material, kind)) = ARMOR.iter().find(|a| a.0 == w.id) else { continue };
        let layer = if kind == 2 { 2 } else { 1 };
        let mut m = Biped::armor(kind);
        m.pose(anim);
        // ModelArmorStandArmor: no suporte de armadura os ângulos vêm do metadado.
        if let Some(f) = repose {
            f(&mut m);
        }
        let mut draw = |path: String, color: u32| {
            let Some(tex) = textures.jar(r, jar, &path) else { return };
            let c = |sh: u32| ((color >> sh) & 0xFF) as f32 / 255.0;
            let mut sink = Sink { geo: out.entry(Tex::Custom(tex)).or_default(), tex: (64.0, 32.0), tint: [tint[0] * c(16), tint[1] * c(8), tint[2] * c(0)], alpha: 255, light, lights };
            m.emit(&mut sink, mat, anim);
        };
        if material == "leather" {
            // ItemArmor.getColor: a cor tinge a base; o overlay vai sem tinta.
            draw(format!("models/armor/{material}_layer_{layer}"), w.color.unwrap_or(LEATHER_DEFAULT_COLOR));
            draw(format!("models/armor/{material}_layer_{layer}_overlay"), 0xFFFFFF);
        } else {
            draw(format!("models/armor/{material}_layer_{layer}"), 0xFFFFFF);
        }
    }
    if let Some(w) = armor[3] {
        custom_head(&model.head, mat, w, anim, villager, light, lights, items, table, textures, r, jar, out);
    }
    let Some(w) = held else { return };
    let Some(item) = items.get_for(w.id, w.damage, w.model, jar, table, r) else { return };
    // postRenderArm: no braço fino o ponto de giro vai 1 px para fora.
    let mut arm = model.right_arm.clone();
    if slim {
        arm.pivot.x += 1.0;
    }
    let mut m = arm.matrix(mat) * Mat4::from_translation(Vec3::new(-0.0625, 0.4375, 0.0625));
    if w.id < 256 && crate::block_physics::RENDER_TYPE_2.contains(&(w.id as u16)) {
        m = m * Mat4::from_translation(Vec3::new(0.0, 0.1875, -0.3125)) * crate::camera::rot(20.0, 1.0, 0.0, 0.0) * crate::camera::rot(45.0, 0.0, 1.0, 0.0) * Mat4::from_scale(Vec3::new(-0.375, -0.375, 0.375));
    }
    if anim.sneak {
        m *= Mat4::from_translation(Vec3::new(0.0, 0.203125, 0.0));
    }
    if item.entity {
        // `renderByItem`: o tile entity do item, com a textura do NBT (estandarte, dono da cabeça).
        let Some(ent) = w.entity.and_then(crate::tile_entities::entity_from_variant) else { return };
        let Some(tex) = crate::tile_entities::item_texture(&ent, textures, r, jar) else { return };
        item.emit_entity(&ent, m, true, &lights, light, out.entry(Tex::Custom(tex)).or_default());
        return;
    }
    item.emit(m, true, &lights, light, out.entry(item.tex).or_default());
}

#[allow(clippy::too_many_arguments)]
/// O que o `LayerCape` lê do jogador, já no tempo parcial.
pub struct Cape {
    /// `chasingPos - pos` interpolados.
    pub chase: DVec3,
    /// `renderYawOffset`.
    pub body: f32,
    pub camera_yaw: f32,
    /// `distanceWalkedModified`.
    pub walked: f32,
    pub sneak: bool,
}

/// `LayerCape.doRenderLayer` + `ModelPlayer.renderCape`: a capa de 10x16x1 (textura de 64x32) presa às costas,
/// inclinada pelo atraso do `chasingPos`, pelo balanço da caminhada e pelo agachar.
pub fn emit_cape(geo: &mut Geometry, mat: &Mat4, c: &Cape, light: [u8; 4], tint: [f32; 3], lights: [Vec3; 2]) {
    let (dx, dy, dz) = (c.chase.x, c.chase.y, c.chase.z);
    let a = c.body * std::f32::consts::PI / 180.0;
    let (s, co) = (a.sin() as f64, -(a.cos() as f64));
    let mut f = (dy as f32 * 10.0).clamp(-6.0, 32.0);
    let f1 = ((dx * s + dz * co) as f32 * 100.0).max(0.0);
    let f2 = (dx * co - dz * s) as f32 * 100.0;
    f += (c.walked * 6.0).sin() * 32.0 * c.camera_yaw;
    if c.sneak {
        f += 25.0;
    }
    let m = *mat
        * Mat4::from_translation(Vec3::new(0.0, 0.0, 0.125))
        * Mat4::from_rotation_x((6.0 + f1 / 2.0 + f).to_radians())
        * Mat4::from_rotation_z((f2 / 2.0).to_radians())
        * Mat4::from_rotation_y((-f2 / 2.0).to_radians())
        * Mat4::from_rotation_y(std::f32::consts::PI);
    // ModelPlayer.setRotationAngles: o ponto de giro desce 2 com o agachar.
    let part = crate::models::Part::new([0.0, if c.sneak { 2.0 } else { 0.0 }, 0.0]).cube((0, 0), [-5.0, 0.0, -1.0], [10, 16, 1], 0.0, false);
    let mut sink = Sink { geo, tex: (64.0, 32.0), tint, alpha: 255, light, lights };
    sink.part(&part, &m);
}

fn draw_model(
    e: &Entity,
    fx: &mut crate::naovivas::Fx,
    pos: Vec3,
    t: f32,
    light: [u8; 4],
    brightness: f32,
    textures: &mut Textures,
    r: &mut Renderer,
    jar: &mut Jar,
    players: &HashMap<Uuid, PlayerInfo>,
    white: u32,
    items: &mut crate::first_person::ItemModels,
    table: &mc_world::blocks::BlockTable,
    lights: [Vec3; 2],
    out: &mut HashMap<Tex, Geometry>,
) {
    let hurt = e.hurt_time > 0 || e.death_time > 0;
    // A tinta vermelha do dano é a mistura do shader (`fx::apply_tint`), não um multiplicador.
    let _ = hurt;
    let tint = [1.0; 3];
    let body = lerp_rot(e.prev_body_yaw, e.body_yaw, t);
    let head = lerp_rot(e.prev_head_yaw, e.head_yaw, t);
    let child = e.child();
    let mut anim = Anim {
        limb_swing: e.limb_swing - e.limb_amount * (1.0 - t),
        limb_amount: (e.prev_limb_amount + (e.limb_amount - e.prev_limb_amount) * t).min(1.0),
        age: e.age as f32 + t,
        head_yaw: head - body,
        head_pitch: e.prev_pitch + (e.pitch - e.prev_pitch) * t,
        swing: e.swing_at(t),
        sneak: e.sneaking(),
        held_right: e.equipment[0].is_some() as i32,
        child,
        ..Default::default()
    };
    if child {
        anim.limb_swing *= 3.0;
    }
    let death = if e.death_time > 0 { ((e.death_time as f32 + t - 1.0) / 20.0 * 1.6).max(0.0).sqrt().min(1.0) * 90.0 } else { 0.0 };
    fn sink_for(out: &mut HashMap<Tex, Geometry>, tex: u32, tint: [f32; 3], light: [u8; 4], lights: [Vec3; 2]) -> Sink<'_> {
        Sink { geo: out.entry(Tex::Custom(tex)).or_default(), tex: (64.0, 64.0), tint, alpha: 255, light, lights }
    }

    match &e.kind {
        Kind::Player { uuid, skin, .. } => {
            let skin = skin.as_ref().or_else(|| players.get(uuid).and_then(|p| p.skin.as_ref()));
            let downloaded = skin.and_then(|s| textures.skin(&s.url).map(|id| (id, s.slim)));
            let (tex, slim) = match downloaded {
                Some(x) => x,
                None => {
                    let (path, slim) = default_skin(*uuid);
                    let Some(id) = textures.jar(r, jar, path) else { return };
                    (id, slim)
                }
            };
            // RenderPlayer.setModelVisibilities: defendendo (3) e arco puxado pelo `getItemInUseCount`.
            let using = e.item_use.filter(|u| u.2 > 0 && e.equipment[0].is_some());
            if let Some((Some(action), _, _)) = using {
                match action {
                    crate::physics::UseAction::Block => anim.held_right = 3,
                    crate::physics::UseAction::Bow => anim.aimed_bow = true,
                    _ => {}
                }
            }
            let mut m = Biped::player(slim);
            m.set_skin_parts(e.meta_byte(10) as u8);
            m.pose(&anim);
            if players.get(uuid).is_some_and(|p| p.gamemode == 3) {
                m.head_only();
            }
            let y = if anim.sneak { pos.y - 0.125 } else { pos.y };
            let mat = living_matrix(Vec3::new(pos.x, y, pos.z), body, death, 0.9375);
            if fx.body {
                m.emit(&mut sink_for(out, tex, tint, light, lights), &mat, &anim);
            }
            if !players.get(uuid).is_some_and(|p| p.gamemode == 3) {
                let worn = |i: usize| e.equipment[i].as_ref().map(Worn::from_slot);
                let held = worn(0).map(|w| Worn { model: crate::first_person::entity_override(w.id, e.fishing, e.item_use.map(|u| u.1 - u.2)), ..w });
                biped_layers(&m, slim, &anim, &mat, held, [worn(1), worn(2), worn(3), worn(4)], light, tint, lights, false, None, items, table, textures, r, jar, out);
            }
            // LayerCape: com a entrada no tab, visível, com a parte CAPE ligada e a capa baixada.
            let cape = skin.and_then(|s| s.cape.as_ref()).filter(|_| players.contains_key(uuid) && !e.invisible() && e.meta_byte(10) & 1 != 0);
            if let Some(tex) = cape.and_then(|u| textures.cape(u)) {
                let td = t as f64;
                let c = Cape {
                    chase: e.prev_chasing.lerp(e.chasing, td) - e.prev.lerp(e.pos, td),
                    body,
                    camera_yaw: e.prev_camera_yaw + (e.camera_yaw - e.prev_camera_yaw) * t,
                    walked: 0.0,
                    sneak: anim.sneak,
                };
                emit_cape(out.entry(Tex::Custom(tex)).or_default(), &mat, &c, light, tint, lights);
            }
        }
        Kind::Mob(id @ (51 | 54 | 57)) => {
            let wither = *id == 51 && e.meta_byte(13) == 1;
            let villager = *id == 54 && e.meta_byte(13) == 1;
            let path = match id {
                51 if wither => "entity/skeleton/wither_skeleton",
                51 => "entity/skeleton/skeleton",
                54 if villager => "entity/zombie/zombie_villager",
                54 => "entity/zombie/zombie",
                _ => "entity/zombie_pigman",
            };
            let Some(tex) = textures.jar(r, jar, path) else { return };
            let mut m = match id {
                51 => Biped::skeleton(),
                _ if villager => Biped::zombie_villager(),
                _ => Biped::zombie(),
            };
            anim.held_right = 0;
            // ModelSkeleton liga o arco armado para o tipo 1.
            anim.aimed_bow = wither;
            m.pose(&anim);
            if *id != 51 {
                m.zombie_arms(&anim);
            }
            let mat = living_matrix(pos, body, death, if wither { 1.2 } else { 1.0 });
            m.emit(&mut sink_for(out, tex, tint, light, lights), &mat, &anim);
            let worn = |i: usize| e.equipment[i].as_ref().map(Worn::from_slot);
            biped_layers(&m, false, &anim, &mat, worn(0), [worn(1), worn(2), worn(3), worn(4)], light, tint, lights, villager, None, items, table, textures, r, jar, out);
        }
        Kind::Mob(120) => {
            let path = match e.meta_int(16) {
                0 => "entity/villager/farmer",
                1 => "entity/villager/librarian",
                2 => "entity/villager/priest",
                3 => "entity/villager/smith",
                4 => "entity/villager/butcher",
                _ => "entity/villager/villager",
            };
            let Some(tex) = textures.jar(r, jar, path) else { return };
            let mut m = Villager::default();
            m.pose(&anim);
            let scale = if child { 0.9375 * 0.5 } else { 0.9375 };
            m.emit(&mut sink_for(out, tex, tint, light, lights), &living_matrix(pos, body, death, scale));
        }
        Kind::Mob(id) => {
            let mut cx = crate::mobs::Ctx { textures, r, jar, out, light, tint, items, table, brightness, lights, body: fx.body };
            if crate::mobs::draw(e, *id, pos, t, &anim, body, death, &mut cx) {
                return;
            }
            let (w, h) = e.size();
            let c = mob_color(*id);
            let mut s = sink_for(out, white, tint, light, lights);
            s.tint = [c[0] * tint[0], c[1] * tint[1], c[2] * tint[2]];
            let mat = Mat4::from_translation(pos) * Mat4::from_rotation_y((-body).to_radians()) * Mat4::from_rotation_z(-death.to_radians());
            s.block_box(&mat, Vec3::new(-w / 2.0, 0.0, -w / 2.0), Vec3::new(w / 2.0, h, w / 2.0));
        }
        Kind::Object { kind, .. } => {
            if matches!(*kind, 78 | 51) {
                let mut cx = crate::mobs::Ctx { textures, r, jar, out, light, tint, items, table, brightness, lights, body: fx.body };
                if crate::mobs::draw_object(e, *kind, pos, t, body, &mut cx) {
                    return;
                }
            }
            let mut cx = crate::naovivas::Cx { cam: fx.cam, world: fx.world, glow: &mut *fx.glow, time: fx.time, cache: &mut *fx.cache, t, light, textures, r, jar, items, table, out };
            if crate::naovivas::draw(e, pos, &mut cx) {
                return;
            }
            let (w, h) = e.size();
            if w <= 0.0 || h <= 0.0 {
                return;
            }
            let mut s = sink_for(out, white, tint, light, lights);
            let age = e.age as f32 + t;
            let mat = if *kind == ITEM {
                // RenderEntityItem: sobe e desce e gira devagar.
                let hover = (e.id as u32).wrapping_mul(2654435761) as f32 / u32::MAX as f32 * std::f32::consts::TAU;
                let bob = (age / 10.0 + hover).sin() * 0.1 + 0.1;
                let item = match &e.meta.get(&10) {
                    Some(MetaValue::Slot(Some(s))) => s.id as u32,
                    _ => 0,
                };
                let hue = item.wrapping_mul(2654435761);
                s.tint = [0.55 + (hue & 0xFF) as f32 / 600.0, 0.55 + (hue >> 8 & 0xFF) as f32 / 600.0, 0.55 + (hue >> 16 & 0xFF) as f32 / 600.0];
                Mat4::from_translation(pos + Vec3::new(0.0, bob, 0.0)) * Mat4::from_rotation_y(age / 20.0 + hover)
            } else {
                s.tint = if *kind == ARMOR_STAND { [0.62, 0.47, 0.3] } else { [0.6, 0.6, 0.6] };
                Mat4::from_translation(pos) * Mat4::from_rotation_y((-lerp_rot(e.prev_yaw, e.yaw, t)).to_radians())
            };
            s.block_box(&mat, Vec3::new(-w / 2.0, 0.0, -w / 2.0), Vec3::new(w / 2.0, h, w / 2.0));
        }
    }
}

/// `Render.renderLivingLabel`: texto virado para a câmera, fundo preto a 25%, escala 0.016666668 * 1.6.
/// Usa a fonte do atlas da GUI: escreve no `UiBatch`, copia os quadros para o mundo e desfaz.
/// Parte do rótulo: fundo + texto fraco (0x20FFFFFF) ou só o texto branco.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NameLayer {
    Bg,
    Dim,
    Bright,
}

/// Quadros de um rótulo no espaço do texto, antes da câmera: camada fraca (fundo e texto) e camada clara.
#[derive(Clone, Default)]
pub struct NameQuads {
    bg: Vec<[UiVertex; 4]>,
    dim: Vec<[UiVertex; 4]>,
    bright: Vec<[UiVertex; 4]>,
}

/// Quadros de um nome, guardados por texto. O texto com `§k` sorteia caracteres a cada desenho (o
/// `fontRandom` avança), então esse não é guardado.
fn name_quads<'a>(names: &'a mut HashMap<String, NameQuads>, ui: &mut UiBatch, text: &str) -> Cow<'a, NameQuads> {
    let build = |ui: &mut UiBatch| NameQuads {
        bg: name_layer(ui, text, NameLayer::Bg),
        dim: name_layer(ui, text, NameLayer::Dim),
        bright: name_layer(ui, text, NameLayer::Bright),
    };
    if text.contains("§k") || text.contains("§K") {
        return Cow::Owned(build(ui));
    }
    if names.len() > 4096 {
        names.clear();
    }
    if !names.contains_key(text) {
        let q = build(ui);
        // Glifo de página unicode ainda fora do atlas: o quadro dele não saiu; guardar perderia a letra.
        if !ui.font.wanted.is_empty() {
            return Cow::Owned(q);
        }
        names.insert(text.to_string(), q);
    }
    Cow::Borrowed(&names[text])
}

/// Uma camada do rótulo pelo layout do `UiBatch`; o batch volta ao tamanho que tinha.
fn name_layer(ui: &mut UiBatch, text: &str, layer: NameLayer) -> Vec<[UiVertex; 4]> {
    let (v0, i0) = (ui.vertices.len(), ui.indices.len());
    let half = (ui.text_width(text) as i32 / 2) as f32;
    let dy = if text == "deadmau5" { -10.0 } else { 0.0 };
    match layer {
        NameLayer::Bg => ui.rect(-half - 1.0, -1.0 + dy, 2.0 * half + 2.0, 9.0, 0x40000000),
        NameLayer::Dim => {
            ui.text(-half, dy, text, 0x20FFFFFF, false);
        }
        NameLayer::Bright => {
            ui.text(-half, dy, text, 0xFFFFFFFF, false);
        }
    }
    let quads = ui.vertices[v0..].chunks_exact(4).map(|q| [q[0], q[1], q[2], q[3]]).collect();
    ui.vertices.truncate(v0);
    ui.indices.truncate(i0);
    quads
}

/// Põe os quadros de um rótulo no `geo`, virados para a câmera (`renderLivingLabel`).
/// `yaw`/`pitch`: `RenderManager.playerViewY` / `playerViewX` (na visão de frente o yaw ganha 180°).
pub fn name_tag(geo: &mut Geometry, quads: &[[UiVertex; 4]], at: Vec3, yaw: f32, pitch: f32, scale: f32) {
    let s = 0.016666668 * 1.6 * scale;
    let m = Mat4::from_translation(at)
        * Mat4::from_rotation_y((-yaw).to_radians())
        * Mat4::from_rotation_x(pitch.to_radians())
        * Mat4::from_scale(Vec3::new(-s, -s, s));
    for q in quads {
        let v = |i: usize| Vertex {
            pos: m.transform_point3(Vec3::new(q[i].pos[0], q[i].pos[1], 0.0)).to_array(),
            uv: q[i].uv,
            color: q[i].color,
            light: FULL_LIGHT,
        };
        geo.quad([v(0), v(1), v(2), v(3)]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player_entity() -> Entity {
        Entity::new(1, Kind::Player { uuid: Uuid(2), name: "Steve".into(), skin: None }, [0, 64 * 32, 0], 0.0, 0.0)
    }

    #[test]
    fn interpolation_reaches_target_in_three_ticks() {
        let mut ents = Entities::default();
        let mut me = Player::default();
        ents.handle(EntityPacket::SpawnMob { id: 5, kind: 54, x: 0, y: 0, z: 0, yaw: 0, pitch: 0, head_yaw: 0, vel: (0, 0, 0), meta: vec![] }, &mut me);
        // Anda 3 blocos em x (96/32) e gira 90°.
        ents.handle(EntityPacket::Teleport { id: 5, x: 96, y: 0, z: 0, yaw: 64, pitch: 0, on_ground: true }, &mut me);
        let xs: Vec<f64> = (0..4)
            .map(|_| {
                ents.tick();
                ents.list[&5].pos.x
            })
            .collect();
        // 1/3 do resto, depois 1/2, depois tudo: 1, 2, 3 e para.
        assert!((xs[0] - 1.0).abs() < 1e-9 && (xs[1] - 2.0).abs() < 1e-9 && (xs[2] - 3.0).abs() < 1e-9 && xs[3] == 3.0, "{xs:?}");
        let e = &ents.list[&5];
        assert!((e.yaw - 90.0).abs() < 1e-4);
        assert_eq!(e.increments, 0);
        assert_eq!(e.prev.x, 3.0);
    }

    #[test]
    fn som_de_dano_igual_ao_java() {
        let p = player_entity();
        assert_eq!(hurt_sound(&p), Some(("game.player.hurt".to_string(), 1.0)));
        let mut slime = Entity::new(2, Kind::Mob(55), [0; 3], 0.0, 0.0);
        slime.meta.insert(16, MetaValue::Byte(4));
        assert_eq!(hurt_sound(&slime), Some(("mob.slime.big".to_string(), 1.6)));
        let mut quiet = player_entity();
        quiet.meta.insert(4, MetaValue::Byte(1));
        assert_eq!(hurt_sound(&quiet), None);
        assert_eq!(hurt_sound(&Entity::new(3, Kind::Mob(94), [0; 3], 0.0, 0.0)), None);
    }

    #[test]
    fn raio_some_no_tick_do_jogo() {
        // O jogo usa o `tick_in`: o raio que terminou os clarões (setDead) tem de sair da lista.
        let mut ents = Entities::default();
        let mut me = Player::default();
        ents.handle(EntityPacket::SpawnGlobal { id: 42, kind: 1, x: 0, y: 64 * 32, z: 0 }, &mut me);
        assert!(ents.list.contains_key(&42));
        let world = World::default();
        for _ in 0..60 {
            ents.tick_in(&world);
        }
        assert!(!ents.list.contains_key(&42));
    }

    #[test]
    fn relative_moves_accumulate_fixed_point() {
        let mut ents = Entities::default();
        let mut me = Player::default();
        ents.handle(EntityPacket::SpawnPlayer { id: 9, uuid: Uuid(3), x: 32, y: 0, z: 0, yaw: 0, pitch: 0, item: 0, meta: vec![] }, &mut me);
        for _ in 0..4 {
            ents.handle(EntityPacket::Move { id: 9, dx: 8, dy: 0, dz: -16, rot: None, on_ground: true }, &mut me);
        }
        let e = &ents.list[&9];
        assert_eq!(e.server, [64, 0, -64]);
        assert_eq!(e.target, DVec3::new(2.0, 0.0, -2.0));
        // Teleporte para menos de 1/32 de distância não mexe na posição.
        ents.handle(EntityPacket::Teleport { id: 9, x: 32, y: 0, z: 0, yaw: -64, pitch: 0, on_ground: true }, &mut me);
        let e = &ents.list[&9];
        assert_eq!(e.target, DVec3::new(1.0, 0.0, 0.0));
        assert_eq!(e.target_yaw, -90.0);
    }

    #[test]
    fn yaw_interpolates_the_short_way() {
        let mut e = player_entity();
        e.yaw = 170.0;
        e.set_target(e.pos, -170.0, 0.0);
        e.tick();
        // 20° pelo lado curto, um terço por tick.
        assert!((e.yaw - (170.0 + 20.0 / 3.0)).abs() < 1e-3, "{}", e.yaw);
        assert!((lerp_rot(170.0, -170.0, 0.5) - 180.0).abs() < 1e-4);
    }

    #[test]
    fn limb_swing_grows_with_distance() {
        let mut e = player_entity();
        for i in 1..=10 {
            e.set_target(DVec3::new(i as f64 * 0.25, 64.0, 0.0), 0.0, 0.0);
            e.increments = 1;
            e.tick();
        }
        // 0.25 bloco/tick * 4 = 1 (o máximo); aproxima 1 a 40% por tick.
        assert!(e.limb_amount > 0.99 && e.limb_amount <= 1.0, "{}", e.limb_amount);
        assert!(e.limb_swing > 5.0);
        // Andando em +X o corpo vira para -90° (yaw do jogo).
        assert!(e.body_yaw < -45.0, "{}", e.body_yaw);
        for _ in 0..30 {
            e.tick();
        }
        assert!(e.limb_amount < 0.01);
    }

    #[test]
    fn body_stays_within_75_degrees_of_yaw() {
        let mut e = player_entity();
        e.yaw = 120.0;
        e.target_yaw = 120.0;
        e.tick();
        let d = wrap_deg(e.yaw - e.body_yaw);
        assert!(d.abs() <= 75.0 + 1e-3, "{d}");
    }

    #[test]
    fn punch_lasts_six_ticks() {
        let mut ents = Entities::default();
        let mut me = Player::default();
        ents.list.insert(1, player_entity());
        ents.handle(EntityPacket::Animation { id: 1, kind: 0 }, &mut me);
        let mut seen = Vec::new();
        for _ in 0..7 {
            ents.tick();
            seen.push(ents.list[&1].swing_progress);
        }
        assert_eq!(seen[0], 0.0);
        assert!((seen[3] - 0.5).abs() < 1e-6);
        assert!((seen[5] - 5.0 / 6.0).abs() < 1e-6);
        assert_eq!(seen[6], 0.0);
        // Interpolação no meio do tick e na virada (volta a 0 conta como +1).
        let e = &ents.list[&1];
        assert!((e.swing_at(0.5) - (5.0 / 6.0 + 1.0 / 12.0)).abs() < 1e-6);
    }

    #[test]
    fn own_velocity_becomes_knockback() {
        let mut ents = Entities::default();
        let mut me = Player::default();
        ents.handle(EntityPacket::JoinGame { entity_id: 77 }, &mut me);
        ents.handle(EntityPacket::Velocity { id: 77, vx: 4000, vy: 3200, vz: -8000 }, &mut me);
        assert_eq!(me.vel, DVec3::new(0.5, 0.4, -1.0));
        ents.handle(EntityPacket::Velocity { id: 78, vx: 8000, vy: 0, vz: 0 }, &mut me);
        assert_eq!(me.vel, DVec3::new(0.5, 0.4, -1.0));
    }

    #[test]
    fn player_list_and_spawn_take_name() {
        let mut ents = Entities::default();
        let mut me = Player::default();
        let u = Uuid(0xABCD);
        ents.handle(
            EntityPacket::PlayerList(vec![PlayerListAction::Add {
                uuid: u,
                name: "Fulano".into(),
                properties: vec![],
                gamemode: 0,
                ping: 50,
                display_name: None,
            }]),
            &mut me,
        );
        ents.handle(EntityPacket::PlayerList(vec![PlayerListAction::Latency { uuid: u, ping: 80 }, PlayerListAction::GameMode { uuid: u, gamemode: 2 }]), &mut me);
        assert_eq!(ents.players[&u].ping, 80);
        assert_eq!(ents.players[&u].gamemode, 2);
        ents.handle(EntityPacket::SpawnPlayer { id: 3, uuid: u, x: 0, y: 0, z: 0, yaw: 0, pitch: 0, item: 0, meta: vec![] }, &mut me);
        assert_eq!(ents.list[&3].label(), Some(("Fulano", 64.0)));
        ents.handle(EntityPacket::Metadata { id: 3, meta: vec![(0, MetaValue::Byte(0x02))] }, &mut me);
        assert_eq!(ents.list[&3].label(), Some(("Fulano", 32.0)));
        ents.handle(EntityPacket::Metadata { id: 3, meta: vec![(0, MetaValue::Byte(0x20))] }, &mut me);
        assert_eq!(ents.list[&3].label(), None);
        ents.handle(EntityPacket::PlayerList(vec![PlayerListAction::Remove { uuid: u }]), &mut me);
        assert!(ents.players.is_empty());
        ents.handle(EntityPacket::Destroy(vec![3]), &mut me);
        assert!(ents.list.is_empty());
    }

    #[test]
    fn item_coletado_voa_tres_ticks_mesmo_com_o_destroy() {
        let mut ents = Entities::default();
        let mut me = Player::default();
        let mut it = Entity::new(9, Kind::Object { kind: crate::naovivas::ITEM, data: 0 }, [0, 64 * 32, 0], 0.0, 0.0);
        it.obj.pickup = Some((1, 0));
        ents.list.insert(9, it);
        let w = World::new(true);
        ents.handle(EntityPacket::Destroy(vec![9]), &mut me);
        ents.tick_in(&w);
        ents.tick_in(&w);
        assert!(ents.list.contains_key(&9));
        ents.tick_in(&w);
        assert!(ents.list.is_empty());
    }

    #[test]
    fn outro_jogador_defende_e_puxa_o_arco() {
        let mut ents = Entities::default();
        let mut p = Entity::new(7, Kind::Player { uuid: Uuid(1), name: "a".into(), skin: None }, [0; 3], 0.0, 0.0);
        p.equipment[0] = Some(Slot { id: 276, count: 1, damage: 0, nbt: None });
        p.meta.insert(0, MetaValue::Byte(0x10));
        ents.list.insert(7, p);
        ents.tick_players();
        let u = ents.list[&7].item_use.unwrap();
        assert_eq!((u.0, u.1, u.2), (Some(crate::physics::UseAction::Block), 72000, 72000));
        ents.tick_players();
        assert_eq!(ents.list[&7].item_use.unwrap().2, 71999);
        // Trocar o item da mão pelo 0x04 encerra o uso até o bit 4 voltar a subir.
        ents.list.get_mut(&7).unwrap().held_changed = true;
        ents.tick_players();
        assert!(ents.list[&7].item_use.is_none());
        ents.list.get_mut(&7).unwrap().meta.insert(0, MetaValue::Byte(0));
        ents.tick_players();
        assert!(!ents.list[&7].using_item);
        // Anzol com data = 7: o jogador está pescando.
        ents.list.insert(8, Entity::new(8, Kind::Object { kind: crate::naovivas::FISH_HOOK, data: 7 }, [0; 3], 0.0, 0.0));
        ents.tick_players();
        assert!(ents.list[&7].fishing);
        assert_eq!(crate::first_person::entity_override(261, false, Some(20)), Some("bow_pulling_2"));
        assert_eq!(crate::first_person::entity_override(261, false, Some(0)), None);
        assert_eq!(crate::first_person::entity_override(346, true, None), Some("fishing_rod_cast"));
    }

    #[test]
    fn hologram_armor_stand_shows_name() {
        let mut ents = Entities::default();
        let mut me = Player::default();
        ents.handle(EntityPacket::SpawnObject { id: 4, kind: ARMOR_STAND, x: 0, y: 0, z: 0, pitch: 0, yaw: 0, data: 0, vel: None }, &mut me);
        ents.handle(
            EntityPacket::Metadata { id: 4, meta: vec![(0, MetaValue::Byte(0x20)), (2, MetaValue::Str("§6Loja".into())), (3, MetaValue::Byte(1))] },
            &mut me,
        );
        let e = &ents.list[&4];
        assert!(e.invisible());
        assert_eq!(e.label(), Some(("§6Loja", 64.0)));
        assert_eq!(e.size(), (0.5, 1.975));
        // Mob invisível com nome não mostra; sem a flag 3 também não.
        let mut z = Entity::new(5, Kind::Mob(54), [0; 3], 0.0, 0.0);
        z.meta.insert(2, MetaValue::Str("Zé".into()));
        assert_eq!(z.label(), None);
        z.meta.insert(3, MetaValue::Byte(1));
        assert_eq!(z.label(), Some(("Zé", 64.0)));
    }

    #[test]
    fn light_lookup() {
        let world = World::new(true);
        assert_eq!(light_at(&world, DVec3::new(0.5, 70.0, 0.5)), FULL_LIGHT);
    }

    #[test]
    fn angles_from_bytes() {
        assert_eq!(angle(64), 90.0);
        assert_eq!(angle(-128), -180.0);
        assert_eq!(wrap_deg(190.0), -170.0);
        assert_eq!(wrap_deg(-540.0), -180.0);
    }

    #[test]
    fn frustum_culls_like_should_render() {
        // Olha para +Z (yaw 0), campo de 70 graus: à frente aparece, atrás e bem de lado não.
        let cam = Camera { eye: Vec3::new(0.0, 64.0, 0.0), yaw: 0.0, pitch: 0.0, fov_deg: 70.0, view_distance: 128.0, view: None };
        let planes = frustum_planes(&cam, (1280, 720)).unwrap();
        let at = |x: f64, z: f64| {
            let mut e = Entity::new(1, Kind::Mob(90), [0, 0, 0], 0.0, 0.0);
            e.pos = DVec3::new(x, 64.0, z);
            e.prev = e.pos;
            in_frustum(&planes, &e)
        };
        assert!(at(0.0, 20.0));
        assert!(!at(0.0, -20.0));
        assert!(!at(60.0, 10.0));
        // Borda horizontal (~51 graus em 16:9): a 10 blocos o canto da caixa alargada (z ate 10,95) chega a x = 13,6.
        // O centro em x = 14 so entra por causa da alargada de 0,5; em x = 15 fica de fora.
        assert!(at(14.0, 10.0));
        assert!(!at(15.0, 10.0));
        assert!(frustum_planes(&cam, (1280, 0)).is_none());
    }

    #[test]
    fn should_render_exceptions() {
        // Atrás da câmera: dragão e anzol desenham (ignoreFrustumCheck); o guardião só se o raio até o alvo aparece.
        let cam = Camera { eye: Vec3::new(0.0, 64.0, 0.0), yaw: 0.0, pitch: 0.0, fov_deg: 70.0, view_distance: 128.0, view: None };
        let planes = frustum_planes(&cam, (1280, 720)).unwrap();
        let put = |id: i32, kind: Kind, x: f64, z: f64| {
            let mut e = Entity::new(id, kind, [0, 0, 0], 0.0, 0.0);
            e.pos = DVec3::new(x, 64.0, z);
            e.prev = e.pos;
            e
        };
        let mut list = HashMap::new();
        list.insert(2, put(2, Kind::Mob(90), 0.0, 20.0));
        list.insert(3, put(3, Kind::Mob(90), 0.0, -30.0));
        let back = |kind: Kind| put(1, kind, 0.0, -20.0);
        assert!(should_render(&planes, &back(Kind::Mob(63)), &list, None));
        assert!(should_render(&planes, &back(Kind::Object { kind: crate::naovivas::FISH_HOOK, data: 0 }), &list, None));
        assert!(!should_render(&planes, &back(Kind::Mob(90)), &list, None));
        let mut g = back(Kind::Mob(68));
        assert!(!should_render(&planes, &g, &list, None));
        g.attack_target = 2;
        assert!(should_render(&planes, &g, &list, None));
        g.attack_target = 3;
        assert!(!should_render(&planes, &g, &list, None));
    }

    #[test]
    fn leash_packet_binds_mob_to_holder() {
        let mut ents = Entities::default();
        let mut me = Player::default();
        // Nó (objeto 77) no bloco (1, 64, 2): centro em +0,5, olhos em -0,0625 e caixa de 0,5.
        ents.handle(EntityPacket::SpawnObject { id: 6, kind: crate::naovivas::LEASH_KNOT, x: 32, y: 64 * 32, z: 2 * 32, pitch: 0, yaw: 0, data: 0, vel: None }, &mut me);
        assert_eq!(ents.list[&6].pos, DVec3::new(1.5, 64.5, 2.5));
        assert!((ents.list[&6].eye_height() + 0.0625).abs() < 1e-6);
        assert_eq!(ents.list[&6].size(), (0.5, 0.5));
        ents.list.insert(5, Entity::new(5, Kind::Mob(92), [0; 3], 0.0, 0.0));
        ents.handle(EntityPacket::Attach { entity: 5, vehicle: 6, leash: true }, &mut me);
        assert_eq!(ents.list[&5].leash_to, Some(6));
        // Vizinho que não existe: `clearLeashed`.
        ents.handle(EntityPacket::Attach { entity: 5, vehicle: 99, leash: true }, &mut me);
        assert_eq!(ents.list[&5].leash_to, None);
        // O próprio jogador (fora da lista) também segura.
        ents.own_id = Some(-2);
        ents.handle(EntityPacket::Attach { entity: 5, vehicle: -2, leash: true }, &mut me);
        assert_eq!(ents.list[&5].leash_to, Some(-2));
        ents.handle(EntityPacket::Attach { entity: 5, vehicle: -1, leash: true }, &mut me);
        assert_eq!(ents.list[&5].leash_to, None);
        // Só EntityLiving (mob) recebe coleira: nó e jogador não.
        ents.handle(EntityPacket::Attach { entity: 6, vehicle: 5, leash: true }, &mut me);
        assert_eq!(ents.list[&6].leash_to, None);
        ents.list.insert(7, Entity::new(7, Kind::Player { uuid: Uuid(3), name: "a".into(), skin: None }, [0; 3], 0.0, 0.0));
        ents.handle(EntityPacket::Attach { entity: 7, vehicle: 6, leash: true }, &mut me);
        assert_eq!(ents.list[&7].leash_to, None);
        // Montaria (leash = false) não mexe na coleira.
        ents.handle(EntityPacket::Attach { entity: 5, vehicle: 6, leash: true }, &mut me);
        ents.handle(EntityPacket::Attach { entity: 5, vehicle: 6, leash: false }, &mut me);
        assert_eq!(ents.list[&5].leash_to, Some(6));
        assert_eq!(ents.list[&5].ridden_by, None);
    }

    #[test]
    fn leashed_mob_is_drawn_when_its_holder_is() {
        let cam = Camera { eye: Vec3::new(0.0, 64.0, 0.0), yaw: 0.0, pitch: 0.0, fov_deg: 70.0, view_distance: 128.0, view: None };
        let planes = frustum_planes(&cam, (1280, 720)).unwrap();
        // Vaca atrás da câmera: só desenha quando o nó preso a ela (à frente) aparece.
        let mut cow = Entity::new(1, Kind::Mob(92), [0; 3], 0.0, 0.0);
        cow.pos = DVec3::new(0.0, 64.0, -20.0);
        cow.prev = cow.pos;
        let knot = |z: f64| {
            let mut k = Entity::new(2, Kind::Object { kind: crate::naovivas::LEASH_KNOT, data: 0 }, [0; 3], 0.0, 0.0);
            k.pos = DVec3::new(0.5, 64.5, z);
            k.prev = k.pos;
            Anchor::of(&k)
        };
        let empty = HashMap::new();
        assert!(!in_frustum(&planes, &cow));
        assert!(should_render(&planes, &cow, &empty, Some(&knot(20.0))));
        assert!(!should_render(&planes, &cow, &empty, Some(&knot(-30.0))));
        assert!(!should_render(&planes, &cow, &empty, None));
        // À frente da câmera, a vaca desenha mesmo com quem segura fora da tela.
        cow.pos = DVec3::new(0.0, 64.0, 20.0);
        assert!(should_render(&planes, &cow, &empty, Some(&knot(-30.0))));
    }

    #[test]
    fn test_hook_leash_spawns_knot_cow_and_pig() {
        let mut ents = Entities::default();
        ents.own_id = Some(-2);
        ents.spawn_test_leash("1,64,2");
        assert_eq!(ents.list[&-200].pos, DVec3::new(1.5, 64.5, 2.5));
        assert_eq!(ents.list[&-201].leash_to, Some(-200));
        assert_eq!(ents.list[&-202].leash_to, Some(-2));
    }
}

/// `getHurtSound` e `getSoundVolume` da entidade (classes do 1.8.9); `None` para quem não tem som de dano
/// (lula, bruxa, objetos) ou está silenciosa (`Entity.isSilent`, metadado 4).
pub fn hurt_sound(e: &Entity) -> Option<(String, f32)> {
    if e.meta_byte(4) != 0 {
        return None;
    }
    let s = |n: &str, v: f32| Some((n.to_string(), v));
    match e.kind {
        Kind::Player { .. } => s("game.player.hurt", 1.0),
        Kind::Object { kind: ARMOR_STAND, .. } => s("game.neutral.hurt", 1.0),
        Kind::Object { .. } => None,
        Kind::Mob(id) => match id {
            50 => s("mob.creeper.say", 1.0),
            51 => s("mob.skeleton.hurt", 1.0),
            52 | 59 => s("mob.spider.say", 1.0),
            53 | 49 => s("game.hostile.hurt", 1.0),
            54 => s("mob.zombie.hurt", 1.0),
            55 | 62 => {
                // EntitySlime: "mob.slime." + big/small pelo tamanho, volume 0.4 x tamanho.
                let size = (e.meta_byte(16) as i32).max(1);
                s(if size > 1 { "mob.slime.big" } else { "mob.slime.small" }, 0.4 * size as f32)
            }
            56 => s("mob.ghast.scream", 10.0),
            57 => s("mob.zombiepig.zpighurt", 1.0),
            58 => s("mob.endermen.hit", 1.0),
            60 | 67 => s("mob.silverfish.hit", 1.0),
            61 => s("mob.blaze.hit", 1.0),
            63 => s("mob.enderdragon.hit", 5.0),
            64 => s("mob.wither.hurt", 1.0),
            65 => s("mob.bat.hurt", 0.1),
            66 | 94 => None,
            68 => {
                let elder = e.meta_int(16) & 4 != 0;
                s(if !e.in_water { "mob.guardian.land.hit" } else if elder { "mob.guardian.elder.hit" } else { "mob.guardian.hit" }, 1.0)
            }
            90 => s("mob.pig.say", 1.0),
            91 => s("mob.sheep.say", 1.0),
            92 | 96 => s("mob.cow.hurt", 0.4),
            93 => s("mob.chicken.hurt", 1.0),
            95 => s("mob.wolf.hurt", 0.4),
            // EntityGolem.getHurtSound devolve "none" (sem som).
            97 => None,
            98 => s("mob.cat.hitt", 0.4),
            99 => s("mob.irongolem.hit", 1.0),
            100 => {
                let t = e.meta_byte(19);
                s(match t { 3 => "mob.horse.zombie.hit", 4 => "mob.horse.skeleton.hit", 1 | 2 => "mob.horse.donkey.hit", _ => "mob.horse.hit" }, 0.8)
            }
            101 => s("mob.rabbit.hurt", 1.0),
            120 => s("mob.villager.hit", 1.0),
            _ => s("game.neutral.hurt", 1.0),
        },
    }
}
