//! O jogador local da 1.8.9, na ordem do `EntityPlayerSP.onUpdate`:
//! `Entity.onEntityUpdate` (água), `EntityPlayerSP.onLivingUpdate` (corrida, voo,
//! empurrão para fora de blocos), `EntityLivingBase.onLivingUpdate` (pulo e
//! `moveEntityWithHeading`), `Entity.moveEntity` (colisão, degrau, beirada) e
//! `onUpdateWalkingPlayer` (pacotes). Os números e a ordem das contas são os do
//! código original, inclusive as conversões entre `float` e `double`.

use glam::DVec3;
use mc_world::blocks::BlockTable;
use mc_world::World;

use crate::block_physics::{BLOCKS, MATERIALS, M_LAVA, M_WATER};
use crate::mth;

/// `AxisAlignedBB`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bb {
    pub min: DVec3,
    pub max: DVec3,
}

impl Bb {
    pub fn offset(self, d: DVec3) -> Bb {
        Bb { min: self.min + d, max: self.max + d }
    }

    /// `addCoord`: estende na direção do movimento.
    fn add_coord(self, d: DVec3) -> Bb {
        Bb { min: self.min + d.min(DVec3::ZERO), max: self.max + d.max(DVec3::ZERO) }
    }

    /// `expand`: cresce (ou encolhe, com valor negativo) dos dois lados.
    fn expand(self, d: DVec3) -> Bb {
        Bb { min: self.min - d, max: self.max + d }
    }

    fn intersects(&self, o: &Bb) -> bool {
        o.max.x > self.min.x && o.min.x < self.max.x && o.max.y > self.min.y && o.min.y < self.max.y && o.max.z > self.min.z && o.min.z < self.max.z
    }

    /// `calculateXOffset` (self = caixa do bloco, `o` = entidade).
    fn clip_x(&self, o: &Bb, mut d: f64) -> f64 {
        if o.max.y > self.min.y && o.min.y < self.max.y && o.max.z > self.min.z && o.min.z < self.max.z {
            if d > 0.0 && o.max.x <= self.min.x {
                let k = self.min.x - o.max.x;
                if k < d {
                    d = k;
                }
            } else if d < 0.0 && o.min.x >= self.max.x {
                let k = self.max.x - o.min.x;
                if k > d {
                    d = k;
                }
            }
        }
        d
    }

    fn clip_y(&self, o: &Bb, mut d: f64) -> f64 {
        if o.max.x > self.min.x && o.min.x < self.max.x && o.max.z > self.min.z && o.min.z < self.max.z {
            if d > 0.0 && o.max.y <= self.min.y {
                let k = self.min.y - o.max.y;
                if k < d {
                    d = k;
                }
            } else if d < 0.0 && o.min.y >= self.max.y {
                let k = self.max.y - o.min.y;
                if k > d {
                    d = k;
                }
            }
        }
        d
    }

    fn clip_z(&self, o: &Bb, mut d: f64) -> f64 {
        if o.max.x > self.min.x && o.min.x < self.max.x && o.max.y > self.min.y && o.min.y < self.max.y {
            if d > 0.0 && o.max.z <= self.min.z {
                let k = self.min.z - o.max.z;
                if k < d {
                    d = k;
                }
            } else if d < 0.0 && o.min.z >= self.max.z {
                let k = self.max.z - o.min.z;
                if k > d {
                    d = k;
                }
            }
        }
        d
    }
}

/// Teclas de movimento num tick (`GameSettings.keyBind*.isKeyDown()`).
#[derive(Default, Clone, Copy, Debug)]
pub struct Keys {
    pub forward: bool,
    pub back: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub sneak: bool,
    pub sprint: bool,
}

/// `MovementInputFromOptions`.
#[derive(Default, Clone, Copy, Debug)]
pub struct MoveInput {
    pub strafe: f32,
    pub forward: f32,
    pub jump: bool,
    pub sneak: bool,
}

impl MoveInput {
    /// `updatePlayerMoveState`.
    pub fn update(&mut self, k: &Keys) {
        self.strafe = 0.0;
        self.forward = 0.0;
        if k.forward {
            self.forward += 1.0;
        }
        if k.back {
            self.forward -= 1.0;
        }
        if k.left {
            self.strafe += 1.0;
        }
        if k.right {
            self.strafe -= 1.0;
        }
        self.jump = k.jump;
        self.sneak = k.sneak;
        if self.sneak {
            self.strafe = (self.strafe as f64 * 0.3) as f32;
            self.forward = (self.forward as f64 * 0.3) as f32;
        }
    }
}

/// Ação do `EnumAction` de um item em uso.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UseAction {
    Eat,
    Drink,
    Block,
    Bow,
}

/// Item em uso (`itemInUse` / `itemInUseCount`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemUse {
    pub action: UseAction,
    pub max_duration: i32,
    pub count: i32,
}

/// UUID do `sprintingSpeedBoostModifier`.
pub const SPRINT_UUID: u128 = 0x662A6B8D_DA3E_4C1C_8813_96EA6097278D;

/// `generic.movementSpeed` do jogador (`ModifiableAttributeInstance`).
#[derive(Clone, Debug)]
pub struct SpeedAttribute {
    pub base: f64,
    /// (uuid, valor, operação).
    pub modifiers: Vec<(u128, f64, u8)>,
}

impl Default for SpeedAttribute {
    fn default() -> Self {
        Self { base: 0.10000000149011612, modifiers: Vec::new() }
    }
}

impl SpeedAttribute {
    /// `computeValue` + `clampValue` (0..1024).
    pub fn value(&self) -> f64 {
        let mut a = self.base;
        for m in self.modifiers.iter().filter(|m| m.2 == 0) {
            a += m.1;
        }
        let mut b = a;
        for m in self.modifiers.iter().filter(|m| m.2 == 1) {
            b += a * m.1;
        }
        for m in self.modifiers.iter().filter(|m| m.2 == 2) {
            b *= 1.0 + m.1;
        }
        b.clamp(0.0, 1024.0)
    }

    fn set_sprint(&mut self, on: bool) {
        self.modifiers.retain(|m| m.0 != SPRINT_UUID);
        if on {
            self.modifiers.push((SPRINT_UUID, 0.30000001192092896, 2));
        }
    }
}

/// Pacote de movimento escolhido por `onUpdateWalkingPlayer`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MovePacket {
    PosLook,
    Pos,
    Look,
    Ground,
}

/// O que o tick quer mandar ao servidor, na ordem do jogo.
#[derive(Default, Debug, PartialEq)]
pub struct TickOut {
    /// `sendPlayerAbilities` (voo ligado/desligado).
    pub abilities: bool,
    /// Ações do C0B: 0/1 agachar, 3/4 correr.
    pub actions: Vec<u8>,
    pub movement: Option<MovePacket>,
    /// Montado: C0C (lado, frente, pulo, agachar) depois do C05.
    pub steer: Option<(f32, f32, bool, bool)>,
    /// `onItemUseFinish` no cliente: a comida/poção na mão acabou de ser usada.
    pub item_finished: bool,
}

/// Largura e altura do jogador como `float` (`setSize(0.6F, 1.8F)`).
pub const HALF_WIDTH: f64 = (0.6f32 / 2.0) as f64;
pub const HEIGHT: f64 = 1.8f32 as f64;
const STEP_HEIGHT: f64 = 0.6f32 as f64;

pub struct Player {
    /// Caixa de colisão: é o estado de verdade, a posição sai dela (`resetPositionToBB`).
    pub bb: Bb,
    /// Posição dos pés.
    pub pos: DVec3,
    /// `prevPos` (começo do tick).
    pub prev: DVec3,
    pub vel: DVec3,
    pub yaw: f32,
    pub pitch: f32,
    pub prev_yaw: f32,
    pub prev_pitch: f32,
    pub on_ground: bool,
    pub collided_h: bool,
    pub collided_v: bool,
    pub in_water: bool,
    in_web: bool,
    pub fall_distance: f32,
    pub input: MoveInput,
    pub sneaking: bool,
    pub sprinting: bool,
    pub flying: bool,
    pub allow_flying: bool,
    pub creative: bool,
    pub invulnerable: bool,
    pub spectator: bool,
    pub fly_speed: f32,
    pub walk_speed: f32,
    pub speed: SpeedAttribute,
    jump_movement_factor: f32,
    jump_ticks: i32,
    pub fly_toggle_timer: i32,
    pub sprint_toggle_timer: i32,
    sprinting_ticks_left: i32,
    pub food_level: i32,
    pub health: f32,
    has_valid_health: bool,
    pub hurt_time: i32,
    pub max_hurt_time: i32,
    pub attacked_at_yaw: f32,
    pub item_use: Option<ItemUse>,
    /// Amplificadores de Salto, Pressa e Fadiga; Cegueira.
    /// Amplificador com sinal (o S1D manda um byte; negativo diminui o pulo).
    pub jump_boost: Option<i8>,
    pub haste: Option<i8>,
    /// `EnchantmentHelper.getDepthStriderModifier`: maior nível de Passos Profundos no equipamento.
    pub depth_strider: i32,
    /// Borda do mundo (minX, minZ, maxX, maxZ), `isOutsideBorder` e a borda que vale nesta colisão (só de dentro).
    pub world_border: Option<[f64; 4]>,
    pub outside_border: bool,
    border_cells: Option<[f64; 4]>,
    pub mining_fatigue: Option<i8>,
    pub blindness: bool,
    // Câmera e animação.
    pub camera_yaw: f32,
    pub prev_camera_yaw: f32,
    /// `chasingPos` do `EntityPlayer`: segue a posição com atraso (o balanço da capa).
    pub chasing: DVec3,
    pub prev_chasing: DVec3,
    pub camera_pitch: f32,
    pub prev_camera_pitch: f32,
    pub distance_walked: f32,
    pub prev_distance_walked: f32,
    pub limb_swing: f32,
    pub limb_swing_amount: f32,
    pub prev_limb_swing_amount: f32,
    pub render_arm_yaw: f32,
    pub render_arm_pitch: f32,
    pub prev_render_arm_yaw: f32,
    pub prev_render_arm_pitch: f32,
    pub render_yaw_offset: f32,
    pub prev_render_yaw_offset: f32,
    pub swing_progress: f32,
    pub prev_swing_progress: f32,
    swing_progress_int: i32,
    pub swinging: bool,
    pub ticks_existed: i32,
    /// Montaria (S1B) e o ponto do assento dela neste tick.
    pub riding: Option<i32>,
    pub mount: Option<DVec3>,
    move_strafing: f32,
    move_forward: f32,
    // Estado já informado ao servidor (`onUpdateWalkingPlayer`).
    server_sprint: bool,
    server_sneak: bool,
    last_pos: DVec3,
    last_rot: (f32, f32),
    position_update_ticks: i32,
    /// `hasReducedDebug` (status 22/23 do 0x1A).
    pub reduced_debug: bool,
}

impl Default for Player {
    fn default() -> Self {
        let mut p = Self {
            reduced_debug: false,
            bb: Bb { min: DVec3::ZERO, max: DVec3::ZERO },
            pos: DVec3::ZERO,
            prev: DVec3::ZERO,
            vel: DVec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            prev_yaw: 0.0,
            prev_pitch: 0.0,
            on_ground: false,
            collided_h: false,
            collided_v: false,
            in_water: false,
            in_web: false,
            fall_distance: 0.0,
            input: MoveInput::default(),
            sneaking: false,
            sprinting: false,
            flying: false,
            allow_flying: false,
            creative: false,
            invulnerable: false,
            spectator: false,
            fly_speed: 0.05,
            walk_speed: 0.1,
            speed: SpeedAttribute::default(),
            jump_movement_factor: 0.02,
            jump_ticks: 0,
            fly_toggle_timer: 0,
            sprint_toggle_timer: 0,
            sprinting_ticks_left: 0,
            food_level: 20,
            health: 20.0,
            has_valid_health: false,
            hurt_time: 0,
            max_hurt_time: 0,
            attacked_at_yaw: 0.0,
            item_use: None,
            jump_boost: None,
            haste: None,
            depth_strider: 0,
            world_border: None,
            outside_border: false,
            border_cells: None,
            mining_fatigue: None,
            blindness: false,
            camera_yaw: 0.0,
            prev_camera_yaw: 0.0,
            chasing: DVec3::ZERO,
            prev_chasing: DVec3::ZERO,
            camera_pitch: 0.0,
            prev_camera_pitch: 0.0,
            distance_walked: 0.0,
            prev_distance_walked: 0.0,
            limb_swing: 0.0,
            limb_swing_amount: 0.0,
            prev_limb_swing_amount: 0.0,
            render_arm_yaw: 0.0,
            render_arm_pitch: 0.0,
            prev_render_arm_yaw: 0.0,
            prev_render_arm_pitch: 0.0,
            render_yaw_offset: 0.0,
            prev_render_yaw_offset: 0.0,
            swing_progress: 0.0,
            prev_swing_progress: 0.0,
            swing_progress_int: 0,
            swinging: false,
            ticks_existed: 0,
            riding: None,
            mount: None,
            move_strafing: 0.0,
            move_forward: 0.0,
            server_sprint: false,
            server_sneak: false,
            last_pos: DVec3::ZERO,
            last_rot: (0.0, 0.0),
            position_update_ticks: 0,
        };
        p.set_position(DVec3::ZERO);
        p
    }
}

fn state_at(w: &World, x: i32, y: i32, z: i32) -> u16 {
    w.block(x, y, z)
}

fn id_at(w: &World, x: i32, y: i32, z: i32) -> usize {
    (state_at(w, x, y, z) >> 4) as usize
}

fn material(id: usize) -> u8 {
    BLOCKS[id].0
}

/// `Entity.moveEntity` de quem não sobe degrau nem agacha (item, TNT, bloco caindo): a caixa nova e o
/// deslocamento que sobrou em cada eixo depois das colisões.
pub fn move_box(w: &World, t: &BlockTable, bb: Bb, d: DVec3) -> (Bb, DVec3) {
    let list = colliding_boxes(w, t, &bb.add_coord(d));
    let (mut x, mut y, mut z) = (d.x, d.y, d.z);
    let mut bb = bb;
    for b in &list {
        y = b.clip_y(&bb, y);
    }
    bb = bb.offset(DVec3::new(0.0, y, 0.0));
    for b in &list {
        x = b.clip_x(&bb, x);
    }
    bb = bb.offset(DVec3::new(x, 0.0, 0.0));
    for b in &list {
        z = b.clip_z(&bb, z);
    }
    bb = bb.offset(DVec3::new(0.0, 0.0, z));
    (bb, DVec3::new(x, y, z))
}

/// O bloco tem caixa de colisão (`rayTraceBlocks` com `ignoreBlockWithoutBoundingBox`).
pub fn has_collision(w: &World, t: &BlockTable, x: i32, y: i32, z: i32) -> bool {
    let cell = Bb { min: DVec3::new(x as f64, y as f64, z as f64), max: DVec3::new(x as f64 + 1.0, y as f64 + 1.0, z as f64 + 1.0) };
    colliding_boxes(w, t, &cell.expand(DVec3::splat(-0.001))).iter().any(|b| b.intersects(&cell))
}

/// `Block.slipperiness` do bloco em (x, y, z).
pub fn slipperiness(w: &World, x: i32, y: i32, z: i32) -> f32 {
    BLOCKS[id_at(w, x, y, z)].1
}

/// Id do bloco em (x, y, z).
pub fn block_id(w: &World, x: i32, y: i32, z: i32) -> usize {
    id_at(w, x, y, z)
}

/// Caixas de colisão no formato do `World.getCollidingBoundingBoxes` (sem entidades).
fn colliding_boxes(w: &World, t: &BlockTable, area: &Bb) -> Vec<Bb> {
    colliding_boxes_b(w, t, area, None)
}

/// Com a borda: para quem está dentro dela, o que fica fora vira pedra (`getCollidingBoundingBoxes`).
fn colliding_boxes_b(w: &World, t: &BlockTable, area: &Bb, border: Option<&[f64; 4]>) -> Vec<Bb> {
    let mut out = Vec::new();
    let mut tmp: Vec<[f64; 6]> = Vec::new();
    let (x0, x1) = (mth::floor(area.min.x), mth::floor(area.max.x + 1.0));
    let (y0, y1) = (mth::floor(area.min.y), mth::floor(area.max.y + 1.0));
    let (z0, z1) = (mth::floor(area.min.z), mth::floor(area.max.z + 1.0));
    for x in x0..x1 {
        for z in z0..z1 {
            if !w.is_loaded(x, z) {
                continue;
            }
            let outside = border.is_some_and(|b| !crate::border::contains(b, x, z));
            for y in y0 - 1..y1 {
                if outside {
                    let o = DVec3::new(x as f64, y as f64, z as f64);
                    let bb = Bb { min: o, max: o + DVec3::ONE };
                    if area.intersects(&bb) {
                        out.push(bb);
                    }
                    continue;
                }
                let st = state_at(w, x, y, z);
                if st == 0 {
                    continue;
                }
                tmp.clear();
                t.collision(st, &|dx, dy, dz| w.block(x + dx, y + dy, z + dz), &mut tmp);
                let o = DVec3::new(x as f64, y as f64, z as f64);
                for b in &tmp {
                    let bb = Bb { min: o + DVec3::new(b[0], b[1], b[2]), max: o + DVec3::new(b[3], b[4], b[5]) };
                    if area.intersects(&bb) {
                        out.push(bb);
                    }
                }
            }
        }
    }
    out
}

/// `BlockLiquid.getLiquidHeightPercent`.
fn liquid_height_percent(mut level: i32) -> f32 {
    if level >= 8 {
        level = 0;
    }
    (level + 1) as f32 / 9.0
}

/// `BlockLiquid.getEffectiveFlowDecay` para o material `mat`.
fn flow_decay(w: &World, mat: u8, x: i32, y: i32, z: i32) -> i32 {
    let st = state_at(w, x, y, z);
    if material((st >> 4) as usize) != mat {
        return -1;
    }
    let level = (st & 15) as i32;
    if level >= 8 {
        0
    } else {
        level
    }
}

/// `Vec3.normalize`: abaixo de 1e-4 vira zero.
fn normalize(v: DVec3) -> DVec3 {
    let l = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
    if l < 1.0e-4 {
        DVec3::ZERO
    } else {
        DVec3::new(v.x / l, v.y / l, v.z / l)
    }
}

/// `EnumFacing.Plane.HORIZONTAL`: norte, leste, sul, oeste.
const HORIZONTAL: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

/// `BlockLiquid.getFlowVector`.
fn flow_vector(w: &World, mat: u8, x: i32, y: i32, z: i32) -> DVec3 {
    let mut v = DVec3::ZERO;
    let here = flow_decay(w, mat, x, y, z);
    for (dx, dz) in HORIZONTAL {
        let (nx, nz) = (x + dx, z + dz);
        let mut d = flow_decay(w, mat, nx, y, nz);
        if d < 0 {
            if !MATERIALS[material(id_at(w, nx, y, nz)) as usize].blocks_movement {
                d = flow_decay(w, mat, nx, y - 1, nz);
                if d >= 0 {
                    let k = d - (here - 8);
                    v += DVec3::new((dx * k) as f64, 0.0, (dz * k) as f64);
                }
            }
        } else {
            let k = d - here;
            v += DVec3::new((dx * k) as f64, 0.0, (dz * k) as f64);
        }
    }
    if state_at(w, x, y, z) & 15 >= 8 {
        // `BlockLiquid.isBlockSolid`: o mesmo material não conta; nas laterais, gelo também não.
        let solid = |bx: i32, by: i32, bz: i32| {
            let m = material(id_at(w, bx, by, bz));
            m != mat && MATERIALS[m as usize].name != "ice" && MATERIALS[m as usize].solid
        };
        for (dx, dz) in HORIZONTAL {
            if solid(x + dx, y, z + dz) || solid(x + dx, y + 1, z + dz) {
                v = normalize(v) + DVec3::new(0.0, -6.0, 0.0);
                break;
            }
        }
    }
    normalize(v)
}

/// `World.isMaterialInBB`.
/// `Entity.handleWaterMovement` de um mob: a caixa (menos 0,4 em Y, menos 0,001 nos lados) toca água.
pub fn box_in_water(w: &World, bb: &Bb) -> bool {
    let bb = bb.expand(DVec3::new(0.0, -0.4000000059604645, 0.0)).expand(DVec3::splat(-0.001));
    let (x0, x1) = (mth::floor(bb.min.x), mth::floor(bb.max.x + 1.0));
    let (y0, y1) = (mth::floor(bb.min.y), mth::floor(bb.max.y + 1.0));
    let (z0, z1) = (mth::floor(bb.min.z), mth::floor(bb.max.z + 1.0));
    for (x, z) in [(x0, z0), (x0, z1), (x1, z0), (x1, z1)] {
        if !w.is_loaded(x, z) {
            return false;
        }
    }
    for x in x0..x1 {
        for y in y0..y1 {
            for z in z0..z1 {
                let st = state_at(w, x, y, z);
                if material((st >> 4) as usize) == M_WATER {
                    let top = ((y + 1) as f32 - liquid_height_percent((st & 15) as i32)) as f64;
                    if y1 as f64 >= top {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn material_in_bb(w: &World, bb: &Bb, mat: u8) -> bool {
    let (x0, x1) = (mth::floor(bb.min.x), mth::floor(bb.max.x + 1.0));
    let (y0, y1) = (mth::floor(bb.min.y), mth::floor(bb.max.y + 1.0));
    let (z0, z1) = (mth::floor(bb.min.z), mth::floor(bb.max.z + 1.0));
    for x in x0..x1 {
        for y in y0..y1 {
            for z in z0..z1 {
                if material(id_at(w, x, y, z)) == mat {
                    return true;
                }
            }
        }
    }
    false
}

/// `World.isAnyLiquid`.
fn any_liquid(w: &World, bb: &Bb) -> bool {
    for x in mth::floor(bb.min.x)..=mth::floor(bb.max.x) {
        for y in mth::floor(bb.min.y)..=mth::floor(bb.max.y) {
            for z in mth::floor(bb.min.z)..=mth::floor(bb.max.z) {
                if MATERIALS[material(id_at(w, x, y, z)) as usize].liquid {
                    return true;
                }
            }
        }
    }
    false
}

/// Ids de `BlockFence`, `BlockWall` e `BlockFenceGate` (o "bloco pisado" desce até eles).
fn fence_like(id: usize) -> bool {
    matches!(id, 85 | 113 | 188..=192 | 139 | 107 | 183..=187)
}

impl Player {
    /// `getEyeHeight`: 1.62F, menos 0.08F agachado.
    pub fn eye_height(&self) -> f64 {
        let mut f = 1.62f32;
        if self.sneaking {
            f -= 0.08;
        }
        f as f64
    }

    /// `setPosition`: refaz a caixa em volta dos pés.
    pub fn set_position(&mut self, p: DVec3) {
        self.pos = p;
        self.bb = Bb {
            min: DVec3::new(p.x - HALF_WIDTH, p.y, p.z - HALF_WIDTH),
            max: DVec3::new(p.x + HALF_WIDTH, p.y + HEIGHT, p.z + HALF_WIDTH),
        };
    }

    /// Teleporte do servidor (S08): posição, giro e "anterior" iguais.
    pub fn teleport(&mut self, p: DVec3, yaw: f32, pitch: f32) {
        self.set_position(p);
        self.prev = p;
        self.yaw = yaw;
        self.pitch = pitch;
        self.prev_yaw = yaw;
        self.prev_pitch = pitch;
    }

    fn reset_position_to_bb(&mut self) {
        self.pos = DVec3::new((self.bb.min.x + self.bb.max.x) / 2.0, self.bb.min.y, (self.bb.min.z + self.bb.max.z) / 2.0);
    }

    fn is_sneaking(&self) -> bool {
        self.input.sneak
    }

    /// `setSprinting` do `EntityPlayerSP` (+ o modificador de atributo do `EntityLivingBase`).
    pub fn set_sprinting(&mut self, on: bool) {
        self.sprinting = on;
        self.speed.set_sprint(on);
        self.sprinting_ticks_left = if on { 600 } else { 0 };
    }

    pub fn is_using_item(&self) -> bool {
        self.item_use.is_some()
    }

    /// `swingItem` (o pacote é mandado por quem chama).
    pub fn swing_item(&mut self) {
        let end = self.arm_swing_end();
        if !self.swinging || self.swing_progress_int >= end / 2 || self.swing_progress_int < 0 {
            self.swing_progress_int = -1;
            self.swinging = true;
        }
    }

    /// `getArmSwingAnimationEnd`.
    fn arm_swing_end(&self) -> i32 {
        if let Some(a) = self.haste {
            6 - (1 + a as i32)
        } else if let Some(a) = self.mining_fatigue {
            6 + (1 + a as i32) * 2
        } else {
            6
        }
    }

    /// `getSwingProgress`.
    pub fn swing(&self, partial: f32) -> f32 {
        let mut f = self.swing_progress - self.prev_swing_progress;
        if f < 0.0 {
            f += 1.0;
        }
        self.prev_swing_progress + f * partial
    }

    pub fn has_valid_health(&self) -> bool {
        self.has_valid_health
    }

    /// `S07`/`S01`: o Java cria um `EntityPlayerSP` novo, com a vida cheia e ainda sem vida válida.
    pub fn reset_health(&mut self) {
        self.health = 20.0;
        self.has_valid_health = false;
        self.hurt_time = 0;
        self.max_hurt_time = 0;
    }

    /// `setPlayerSPHealth`: queda de vida liga a câmera de dano.
    pub fn set_health(&mut self, h: f32) {
        if self.has_valid_health {
            if self.health - h <= 0.0 {
                self.health = h;
            } else {
                // `EntityPlayerSP.damageEntity(generic, f)`: setHealth(getHealth() - f), o jogador local
                // nunca é `invulnerable` (campo do Entity, só vem do NBT do servidor).
                self.health = h;
                self.hurt_time = 10;
                self.max_hurt_time = 10;
            }
        } else {
            self.health = h;
            self.has_valid_health = true;
        }
    }

    /// `handleStatusUpdate(2)`: dano anunciado pelo servidor.
    pub fn hurt_status(&mut self) {
        self.limb_swing_amount = 1.5;
        self.hurt_time = 10;
        self.max_hurt_time = 10;
        self.attacked_at_yaw = 0.0;
    }

    /// Um tick completo (`EntityPlayerSP.onUpdate`). No cliente o `isBlockLoaded` é sempre verdadeiro
    /// (`ChunkProviderClient.chunkExists`): sem a coluna o jogador cai a -0,1 e manda o C03 normalmente.
    pub fn tick(&mut self, w: &World, t: &BlockTable, keys: &Keys) -> TickOut {
        let mut out = TickOut::default();
        if self.riding.is_some() {
            // Entity.updateRidden: zera o movimento antes do onUpdate.
            self.vel = DVec3::ZERO;
        }
        // EntityPlayer.onUpdate
        if self.spectator {
            self.on_ground = false;
        }
        // EntityPlayer.onUpdate (Forge): com a contagem zerada o próprio cliente termina o uso (onItemUseFinish),
        // um tick depois do servidor; senão conta.
        if let Some(u) = &mut self.item_use {
            if u.count <= 0 {
                self.item_use = None;
                out.item_finished = true;
            } else {
                u.count -= 1;
            }
        }
        // EntityLivingBase.onEntityUpdate / Entity.onEntityUpdate
        self.prev_swing_progress = self.swing_progress;
        self.prev_distance_walked = self.distance_walked;
        self.prev = self.pos;
        self.prev_pitch = self.pitch;
        self.prev_yaw = self.yaw;
        self.handle_water_movement(w);
        self.prev_camera_pitch = self.camera_pitch;
        if self.hurt_time > 0 {
            self.hurt_time -= 1;
        }
        self.prev_render_yaw_offset = self.render_yaw_offset;
        self.ticks_existed += 1;

        self.on_living_update_sp(w, t, keys, &mut out);
        self.after_living_update();
        if self.riding.is_some() {
            // EntityPlayerSP.onUpdate montado: C05 e C0C (o servidor desmonta com o agachar do C0C).
            out.movement = Some(MovePacket::Look);
            out.steer = Some((self.move_strafing, self.move_forward, self.input.jump, self.input.sneak));
            if let Some(m) = self.mount {
                // updateRiderPosition: assento + `EntityPlayer.getYOffset()` (-0.35).
                self.set_position(m + DVec3::new(0.0, -0.35, 0.0));
            }
        } else {
            out.movement = Some(self.walking_packets(&mut out.actions));
        }
        out
    }

        /// `Entity.handleWaterMovement` (+ `World.handleMaterialAcceleration`).
    fn handle_water_movement(&mut self, w: &World) {
        let bb = self.bb.expand(DVec3::new(0.0, -0.4000000059604645, 0.0)).expand(DVec3::splat(-0.001));
        let (x0, x1) = (mth::floor(bb.min.x), mth::floor(bb.max.x + 1.0));
        let (y0, y1) = (mth::floor(bb.min.y), mth::floor(bb.max.y + 1.0));
        let (z0, z1) = (mth::floor(bb.min.z), mth::floor(bb.max.z + 1.0));
        for (x, z) in [(x0, z0), (x0, z1), (x1, z0), (x1, z1)] {
            if !w.is_loaded(x, z) {
                self.in_water = false;
                return;
            }
        }
        let mut found = false;
        let mut flow = DVec3::ZERO;
        for x in x0..x1 {
            for y in y0..y1 {
                for z in z0..z1 {
                    let st = state_at(w, x, y, z);
                    if material((st >> 4) as usize) == M_WATER {
                        let top = ((y + 1) as f32 - liquid_height_percent((st & 15) as i32)) as f64;
                        if y1 as f64 >= top {
                            found = true;
                            flow += flow_vector(w, M_WATER, x, y, z);
                        }
                    }
                }
            }
        }
        // `isPushedByWater`: o jogador voando não é empurrado.
        if flow.length() > 0.0 && !self.flying {
            self.vel += normalize(flow) * 0.014;
        }
        if found {
            self.fall_distance = 0.0;
        }
        self.in_water = found;
    }

    /// `isInLava`.
    fn in_lava(&self, w: &World) -> bool {
        let bb = self.bb.expand(DVec3::new(-0.10000000149011612, -0.4000000059604645, -0.10000000149011612));
        material_in_bb(w, &bb, M_LAVA)
    }

    /// `isOnLadder`: escada ou trepadeira no bloco dos pés.
    pub fn on_ladder(&self, w: &World) -> bool {
        let id = id_at(w, mth::floor(self.pos.x), mth::floor(self.bb.min.y), mth::floor(self.pos.z));
        (id == 65 || id == 106) && !self.spectator
    }

    /// `EntityPlayerSP.onLivingUpdate`.
    fn on_living_update_sp(&mut self, w: &World, t: &BlockTable, keys: &Keys, out: &mut TickOut) {
        if self.sprinting_ticks_left > 0 {
            self.sprinting_ticks_left -= 1;
            if self.sprinting_ticks_left == 0 {
                self.set_sprinting(false);
            }
        }
        if self.sprint_toggle_timer > 0 {
            self.sprint_toggle_timer -= 1;
        }
        let was_jump = self.input.jump;
        let was_sneak = self.input.sneak;
        let f = 0.8f32;
        let was_forward = self.input.forward >= f;
        self.input.update(keys);
        self.sneaking = self.input.sneak;
        if self.is_using_item() {
            self.input.strafe *= 0.2;
            self.input.forward *= 0.2;
            self.sprint_toggle_timer = 0;
        }
        let k = 0.6f32 as f64 * 0.35;
        let y = self.bb.min.y + 0.5;
        for (sx, sz) in [(-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0), (1.0, 1.0)] {
            self.push_out_of_blocks(w, self.pos.x + sx * k, y, self.pos.z + sz * k);
        }
        let can_sprint = self.food_level as f32 > 6.0 || self.allow_flying;
        if self.on_ground && !was_sneak && !was_forward && self.input.forward >= f && !self.sprinting && can_sprint && !self.is_using_item() && !self.blindness {
            if self.sprint_toggle_timer <= 0 && !keys.sprint {
                self.sprint_toggle_timer = 7;
            } else {
                self.set_sprinting(true);
            }
        }
        if !self.sprinting && self.input.forward >= f && can_sprint && !self.is_using_item() && !self.blindness && keys.sprint {
            self.set_sprinting(true);
        }
        if self.sprinting && (self.input.forward < f || self.collided_h || !can_sprint) {
            self.set_sprinting(false);
        }
        if self.allow_flying {
            if self.spectator {
                if !self.flying {
                    self.flying = true;
                    out.abilities = true;
                }
            } else if !was_jump && self.input.jump {
                if self.fly_toggle_timer == 0 {
                    self.fly_toggle_timer = 7;
                } else {
                    self.flying = !self.flying;
                    out.abilities = true;
                    self.fly_toggle_timer = 0;
                }
            }
        }
        if self.flying {
            if self.input.sneak {
                self.vel.y -= (self.fly_speed * 3.0) as f64;
            }
            if self.input.jump {
                self.vel.y += (self.fly_speed * 3.0) as f64;
            }
        }
        // super.onLivingUpdate(): EntityPlayer.onLivingUpdate
        if self.fly_toggle_timer > 0 {
            self.fly_toggle_timer -= 1;
        }
        self.prev_camera_yaw = self.camera_yaw;
        self.on_living_update_base(w, t);
        self.jump_movement_factor = 0.02;
        if self.sprinting {
            self.jump_movement_factor = (0.02f32 as f64 + 0.02f32 as f64 * 0.3) as f32;
        }
        let mut speed = (self.vel.x * self.vel.x + self.vel.z * self.vel.z).sqrt() as f32;
        let mut tilt = ((-self.vel.y * 0.20000000298023224).atan() * 15.0) as f32;
        if speed > 0.1 {
            speed = 0.1;
        }
        if !self.on_ground || self.health <= 0.0 {
            speed = 0.0;
        }
        if self.on_ground || self.health <= 0.0 {
            tilt = 0.0;
        }
        self.camera_yaw += (speed - self.camera_yaw) * 0.4;
        self.camera_pitch += (tilt - self.camera_pitch) * 0.8;
        // Fim do EntityPlayerSP.onLivingUpdate: tocar o chão desliga o voo.
        if self.on_ground && self.flying && !self.spectator {
            self.flying = false;
            out.abilities = true;
        }
    }

    /// `EntityPlayerSP.pushOutOfBlocks` (`isOpenBlockSpace` = não é `isNormalCube`, pés e cabeça).
    fn push_out_of_blocks(&mut self, w: &World, x: f64, y: f64, z: f64) {
        if self.spectator {
            return;
        }
        let (bx, by, bz) = (mth::floor(x), mth::floor(y), mth::floor(z));
        let open = |px: i32, pz: i32| (0..2).all(|dy| !BLOCKS[id_at(w, px, by + dy, pz)].2);
        if open(bx, bz) {
            return;
        }
        let (d0, d1) = (x - bx as f64, z - bz as f64);
        let mut i = -1;
        let mut d2 = 9999.0;
        if open(bx - 1, bz) && d0 < d2 {
            d2 = d0;
            i = 0;
        }
        if open(bx + 1, bz) && 1.0 - d0 < d2 {
            d2 = 1.0 - d0;
            i = 1;
        }
        if open(bx, bz - 1) && d1 < d2 {
            d2 = d1;
            i = 4;
        }
        if open(bx, bz + 1) && 1.0 - d1 < d2 {
            i = 5;
        }
        let f = 0.1f32 as f64;
        match i {
            0 => self.vel.x = -f,
            1 => self.vel.x = f,
            4 => self.vel.z = -f,
            5 => self.vel.z = f,
            _ => {}
        }
    }

    /// `EntityLivingBase.onLivingUpdate` (+ `updateEntityActionState` do jogador).
    fn on_living_update_base(&mut self, w: &World, t: &BlockTable) {
        if self.jump_ticks > 0 {
            self.jump_ticks -= 1;
        }
        for v in [&mut self.vel.x, &mut self.vel.y, &mut self.vel.z] {
            if v.abs() < 0.005 {
                *v = 0.0;
            }
        }
        let (mut strafe, mut forward, mut jumping) = (0.0f32, 0.0f32, false);
        if self.health > 0.0 {
            // EntityPlayer.updateEntityActionState
            self.update_arm_swing();
            // EntityPlayerSP.updateEntityActionState
            strafe = self.input.strafe;
            forward = self.input.forward;
            jumping = self.input.jump;
            self.prev_render_arm_yaw = self.render_arm_yaw;
            self.prev_render_arm_pitch = self.render_arm_pitch;
            self.render_arm_pitch = (self.render_arm_pitch as f64 + (self.pitch - self.render_arm_pitch) as f64 * 0.5) as f32;
            self.render_arm_yaw = (self.render_arm_yaw as f64 + (self.yaw - self.render_arm_yaw) as f64 * 0.5) as f32;
        }
        if jumping {
            if self.in_water || self.in_lava(w) {
                self.vel.y += 0.03999999910593033;
            } else if self.on_ground && self.jump_ticks == 0 {
                self.jump();
                self.jump_ticks = 10;
            }
        } else {
            self.jump_ticks = 0;
        }
        strafe *= 0.98;
        forward *= 0.98;
        self.move_strafing = strafe;
        self.move_forward = forward;
        self.move_with_heading_player(w, t, strafe, forward);
    }

    /// `EntityLivingBase.jump`.
    fn jump(&mut self) {
        self.vel.y = 0.42f32 as f64;
        if let Some(a) = self.jump_boost {
            self.vel.y += ((a as i32 + 1) as f32 * 0.1) as f64;
        }
        if self.sprinting {
            let f = self.yaw * 0.017453292;
            self.vel.x -= (mth::sin(f) * 0.2) as f64;
            self.vel.z += (mth::cos(f) * 0.2) as f64;
        }
    }

    /// `EntityLivingBase.updateArmSwingProgress`.
    fn update_arm_swing(&mut self) {
        let end = self.arm_swing_end();
        if self.swinging {
            self.swing_progress_int += 1;
            if self.swing_progress_int >= end {
                self.swing_progress_int = 0;
                self.swinging = false;
            }
        } else {
            self.swing_progress_int = 0;
        }
        self.swing_progress = self.swing_progress_int as f32 / end as f32;
    }

    /// `EntityPlayer.moveEntityWithHeading`: no voo troca o fator do ar e segura o Y.
    fn move_with_heading_player(&mut self, w: &World, t: &BlockTable, strafe: f32, forward: f32) {
        if self.flying {
            let vy = self.vel.y;
            let saved = self.jump_movement_factor;
            self.jump_movement_factor = self.fly_speed * if self.sprinting { 2.0 } else { 1.0 };
            self.move_with_heading(w, t, strafe, forward);
            self.vel.y = vy * 0.6;
            self.jump_movement_factor = saved;
        } else {
            self.move_with_heading(w, t, strafe, forward);
        }
    }

    /// `Entity.moveFlying`.
    fn move_flying(&mut self, mut strafe: f32, mut forward: f32, friction: f32) {
        let mut f = strafe * strafe + forward * forward;
        if f >= 1.0e-4 {
            f = f.sqrt();
            if f < 1.0 {
                f = 1.0;
            }
            f = friction / f;
            strafe *= f;
            forward *= f;
            let s = mth::sin(self.yaw * mth::PI / 180.0);
            let c = mth::cos(self.yaw * mth::PI / 180.0);
            self.vel.x += (strafe * c - forward * s) as f64;
            self.vel.z += (forward * c + strafe * s) as f64;
        }
    }

    fn ground_slipperiness(&self, w: &World) -> f32 {
        let id = id_at(w, mth::floor(self.pos.x), mth::floor(self.bb.min.y) - 1, mth::floor(self.pos.z));
        BLOCKS[id].1 * 0.91
    }

    /// `EntityLivingBase.moveEntityWithHeading`.
    fn move_with_heading(&mut self, w: &World, t: &BlockTable, strafe: f32, forward: f32) {
        if self.in_water && !self.flying {
            let y0 = self.pos.y;
            // Passos Profundos (até 3, metade fora do chão) aproxima o atrito do chão e a velocidade de andar.
            let (mut f1, mut f2) = (0.8f32, 0.02f32);
            let mut f3 = (self.depth_strider as f32).min(3.0);
            if !self.on_ground {
                f3 *= 0.5;
            }
            if f3 > 0.0 {
                f1 += (0.54600006 - f1) * f3 / 3.0;
                f2 += (self.speed.value() as f32 - f2) * f3 / 3.0;
            }
            self.move_flying(strafe, forward, f2);
            self.move_entity(w, t, self.vel);
            self.vel.x *= f1 as f64;
            self.vel.y *= 0.800000011920929;
            self.vel.z *= f1 as f64;
            self.vel.y -= 0.02;
            if self.collided_h && self.offset_in_liquid(w, t, DVec3::new(self.vel.x, self.vel.y + 0.6000000238418579 - self.pos.y + y0, self.vel.z)) {
                self.vel.y = 0.30000001192092896;
            }
        } else if self.in_lava(w) && !self.flying {
            let y0 = self.pos.y;
            self.move_flying(strafe, forward, 0.02);
            self.move_entity(w, t, self.vel);
            self.vel *= 0.5;
            self.vel.y -= 0.02;
            if self.collided_h && self.offset_in_liquid(w, t, DVec3::new(self.vel.x, self.vel.y + 0.6000000238418579 - self.pos.y + y0, self.vel.z)) {
                self.vel.y = 0.30000001192092896;
            }
        } else {
            let mut f4 = 0.91f32;
            if self.on_ground {
                f4 = self.ground_slipperiness(w);
            }
            let f = 0.16277136f32 / (f4 * f4 * f4);
            let f5 = if self.on_ground { self.speed.value() as f32 * f } else { self.jump_movement_factor };
            self.move_flying(strafe, forward, f5);
            f4 = 0.91;
            if self.on_ground {
                f4 = self.ground_slipperiness(w);
            }
            if self.on_ladder(w) {
                let f6 = 0.15f32 as f64;
                self.vel.x = self.vel.x.clamp(-f6, f6);
                self.vel.z = self.vel.z.clamp(-f6, f6);
                self.fall_distance = 0.0;
                if self.vel.y < -0.15 {
                    self.vel.y = -0.15;
                }
                if self.is_sneaking() && self.vel.y < 0.0 {
                    self.vel.y = 0.0;
                }
            }
            self.move_entity(w, t, self.vel);
            if self.collided_h && self.on_ladder(w) {
                self.vel.y = 0.2;
            }
            // `(int)posX`: aqui o jogo trunca em vez de arredondar para baixo.
            if !w.is_loaded(self.pos.x as i32, self.pos.z as i32) {
                self.vel.y = if self.pos.y > 0.0 { -0.1 } else { 0.0 };
            } else {
                self.vel.y -= 0.08;
            }
            self.vel.y *= 0.9800000190734863;
            self.vel.x *= f4 as f64;
            self.vel.z *= f4 as f64;
        }
        self.prev_limb_swing_amount = self.limb_swing_amount;
        let (dx, dz) = (self.pos.x - self.prev.x, self.pos.z - self.prev.z);
        let mut f7 = (dx * dx + dz * dz).sqrt() as f32 * 4.0;
        if f7 > 1.0 {
            f7 = 1.0;
        }
        self.limb_swing_amount += (f7 - self.limb_swing_amount) * 0.4;
        self.limb_swing += self.limb_swing_amount;
    }

    /// `isOffsetPositionInLiquid`.
    fn offset_in_liquid(&self, w: &World, t: &BlockTable, d: DVec3) -> bool {
        let bb = self.bb.offset(d);
        colliding_boxes_b(w, t, &bb, self.border_cells.as_ref()).is_empty() && !any_liquid(w, &bb)
    }

    /// `Entity.moveEntity`.
    pub fn move_entity(&mut self, w: &World, t: &BlockTable, d: DVec3) {
        if self.spectator {
            self.bb = self.bb.offset(d);
            self.reset_position_to_bb();
            return;
        }
        // World.isInsideBorder com a folga de 1 (para dentro se já está fora) e o setOutsideBorder.
        self.border_cells = None;
        if let Some(b) = self.world_border {
            let e = if self.outside_border { 1.0 } else { -1.0 };
            let inside = self.pos.x > b[0] + e && self.pos.x < b[2] - e && self.pos.z > b[1] + e && self.pos.z < b[3] - e;
            if self.outside_border && inside {
                self.outside_border = false;
            } else if !self.outside_border && !inside {
                self.outside_border = true;
            }
            if inside {
                self.border_cells = Some(b);
            }
        }
        let (mut x, mut y, mut z) = (d.x, d.y, d.z);
        if self.in_web {
            self.in_web = false;
            x *= 0.25;
            y *= 0.05000000074505806;
            z *= 0.25;
            self.vel = DVec3::ZERO;
        }
        let (mut d3, d4, mut d5) = (x, y, z);
        let edge = self.on_ground && self.is_sneaking();
        if edge {
            let step = 0.05;
            let bb = self.bb;
            let cells = self.border_cells;
            let free = |dx: f64, dz: f64| colliding_boxes_b(w, t, &bb.offset(DVec3::new(dx, -1.0, dz)), cells.as_ref()).is_empty();
            let shrink = |v: f64| {
                if v < step && v >= -step {
                    0.0
                } else if v > 0.0 {
                    v - step
                } else {
                    v + step
                }
            };
            while x != 0.0 && free(x, 0.0) {
                x = shrink(x);
                d3 = x;
            }
            while z != 0.0 && free(0.0, z) {
                z = shrink(z);
                d5 = z;
            }
            while x != 0.0 && z != 0.0 && free(x, z) {
                x = shrink(x);
                d3 = x;
                z = shrink(z);
                d5 = z;
            }
        }
        let list = colliding_boxes_b(w, t, &self.bb.add_coord(DVec3::new(x, y, z)), self.border_cells.as_ref());
        let start = self.bb;
        for b in &list {
            y = b.clip_y(&self.bb, y);
        }
        self.bb = self.bb.offset(DVec3::new(0.0, y, 0.0));
        let grounded = self.on_ground || (d4 != y && d4 < 0.0);
        for b in &list {
            x = b.clip_x(&self.bb, x);
        }
        self.bb = self.bb.offset(DVec3::new(x, 0.0, 0.0));
        for b in &list {
            z = b.clip_z(&self.bb, z);
        }
        self.bb = self.bb.offset(DVec3::new(0.0, 0.0, z));

        if STEP_HEIGHT > 0.0 && grounded && (d3 != x || d5 != z) {
            let (d11, d7, d8) = (x, y, z);
            let bb3 = self.bb;
            self.bb = start;
            y = STEP_HEIGHT;
            let list = colliding_boxes_b(w, t, &self.bb.add_coord(DVec3::new(d3, y, d5)), self.border_cells.as_ref());
            // Primeira tentativa: sobe medindo com a caixa já estendida na horizontal.
            let mut bb4 = self.bb;
            let bb5 = bb4.add_coord(DVec3::new(d3, 0.0, d5));
            let mut d9 = y;
            for b in &list {
                d9 = b.clip_y(&bb5, d9);
            }
            bb4 = bb4.offset(DVec3::new(0.0, d9, 0.0));
            let mut d15 = d3;
            for b in &list {
                d15 = b.clip_x(&bb4, d15);
            }
            bb4 = bb4.offset(DVec3::new(d15, 0.0, 0.0));
            let mut d16 = d5;
            for b in &list {
                d16 = b.clip_z(&bb4, d16);
            }
            bb4 = bb4.offset(DVec3::new(0.0, 0.0, d16));
            // Segunda tentativa: sobe com a caixa parada.
            let mut bb14 = self.bb;
            let mut d17 = y;
            for b in &list {
                d17 = b.clip_y(&bb14, d17);
            }
            bb14 = bb14.offset(DVec3::new(0.0, d17, 0.0));
            let mut d18 = d3;
            for b in &list {
                d18 = b.clip_x(&bb14, d18);
            }
            bb14 = bb14.offset(DVec3::new(d18, 0.0, 0.0));
            let mut d19 = d5;
            for b in &list {
                d19 = b.clip_z(&bb14, d19);
            }
            bb14 = bb14.offset(DVec3::new(0.0, 0.0, d19));
            if d15 * d15 + d16 * d16 > d18 * d18 + d19 * d19 {
                x = d15;
                z = d16;
                y = -d9;
                self.bb = bb4;
            } else {
                x = d18;
                z = d19;
                y = -d17;
                self.bb = bb14;
            }
            for b in &list {
                y = b.clip_y(&self.bb, y);
            }
            self.bb = self.bb.offset(DVec3::new(0.0, y, 0.0));
            if d11 * d11 + d8 * d8 >= x * x + z * z {
                x = d11;
                y = d7;
                z = d8;
                self.bb = bb3;
            }
        }

        let before = self.pos;
        self.reset_position_to_bb();
        self.collided_h = d3 != x || d5 != z;
        self.collided_v = d4 != y;
        self.on_ground = self.collided_v && d4 < 0.0;
        // Bloco "pisado": 0,2 abaixo dos pés, ou a cerca/muro logo abaixo do ar.
        let (bx, by, bz) = (mth::floor(self.pos.x), mth::floor(self.pos.y - 0.20000000298023224), mth::floor(self.pos.z));
        let mut block = id_at(w, bx, by, bz);
        if material(block) == 0 && fence_like(id_at(w, bx, by - 1, bz)) {
            block = id_at(w, bx, by - 1, bz);
        }
        // updateFallState (só a distância; o dano é do servidor).
        if self.on_ground {
            self.fall_distance = 0.0;
        } else if y < 0.0 {
            self.fall_distance = (self.fall_distance as f64 - y) as f32;
        }
        if d3 != x {
            self.vel.x = 0.0;
        }
        if d5 != z {
            self.vel.z = 0.0;
        }
        if d4 != y {
            // Block.onLanded: o slime devolve a queda (menos agachado); o resto zera.
            if block == 165 && !self.is_sneaking() {
                if self.vel.y < 0.0 {
                    self.vel.y = -self.vel.y;
                }
            } else {
                self.vel.y = 0.0;
            }
        }
        if !edge {
            if self.on_ground && block == 165 && self.vel.y.abs() < 0.1 && !self.is_sneaking() {
                // BlockSlime.onEntityCollidedWithBlock (andando por cima).
                let k = 0.4 + self.vel.y.abs() * 0.2;
                self.vel.x *= k;
                self.vel.z *= k;
            }
            let (ddx, ddz) = (self.pos.x - before.x, self.pos.z - before.z);
            self.distance_walked = (self.distance_walked as f64 + (ddx * ddx + ddz * ddz).sqrt() * 0.6) as f32;
        }
        self.do_block_collisions(w);
    }

    /// `Entity.doBlockCollisions`: teia e areia das almas.
    fn do_block_collisions(&mut self, w: &World) {
        let a = self.bb.min + DVec3::splat(0.001);
        let b = self.bb.max - DVec3::splat(0.001);
        let (x0, y0, z0) = (mth::floor(a.x), mth::floor(a.y), mth::floor(a.z));
        let (x1, y1, z1) = (mth::floor(b.x), mth::floor(b.y), mth::floor(b.z));
        if !(w.is_loaded(x0, z0) && w.is_loaded(x1, z1) && w.is_loaded(x0, z1) && w.is_loaded(x1, z0)) {
            return;
        }
        for x in x0..=x1 {
            for y in y0..=y1 {
                for z in z0..=z1 {
                    match id_at(w, x, y, z) {
                        30 => {
                            self.in_web = true;
                            self.fall_distance = 0.0;
                        }
                        88 => {
                            self.vel.x *= 0.4;
                            self.vel.z *= 0.4;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// O fim do `EntityLivingBase.onUpdate`: giro do corpo (para a 3ª pessoa) e faixas de ângulo.
    fn after_living_update(&mut self) {
        let (d0, d1) = (self.pos.x - self.prev.x, self.pos.z - self.prev.z);
        let f = (d0 * d0 + d1 * d1) as f32;
        let mut f1 = self.render_yaw_offset;
        if f > 0.0025000002 {
            f1 = (d1.atan2(d0) as f32) * 180.0 / mth::PI - 90.0;
        }
        if self.swing_progress > 0.0 {
            f1 = self.yaw;
        }
        // updateDistance
        let g = mth::wrap_degrees(f1 - self.render_yaw_offset);
        self.render_yaw_offset += g * 0.3;
        let mut h = mth::wrap_degrees(self.yaw - self.render_yaw_offset);
        if h < -75.0 {
            h = -75.0;
        }
        if h >= 75.0 {
            h = 75.0;
        }
        self.render_yaw_offset = self.yaw - h;
        if h * h > 2500.0 {
            self.render_yaw_offset += h * 0.2;
        }
        while self.yaw - self.prev_yaw < -180.0 {
            self.prev_yaw -= 360.0;
        }
        while self.yaw - self.prev_yaw >= 180.0 {
            self.prev_yaw += 360.0;
        }
        while self.render_yaw_offset - self.prev_render_yaw_offset < -180.0 {
            self.prev_render_yaw_offset -= 360.0;
        }
        while self.render_yaw_offset - self.prev_render_yaw_offset >= 180.0 {
            self.prev_render_yaw_offset += 360.0;
        }
    }

    /// `onUpdateWalkingPlayer`: C0B de corrida/agachar e o C03 do tick.
    fn walking_packets(&mut self, actions: &mut Vec<u8>) -> MovePacket {
        if self.sprinting != self.server_sprint {
            actions.push(if self.sprinting { 3 } else { 4 });
            self.server_sprint = self.sprinting;
        }
        let sneak = self.is_sneaking();
        if sneak != self.server_sneak {
            actions.push(if sneak { 0 } else { 1 });
            self.server_sneak = sneak;
        }
        let d0 = self.pos.x - self.last_pos.x;
        let d1 = self.bb.min.y - self.last_pos.y;
        let d2 = self.pos.z - self.last_pos.z;
        let d3 = (self.yaw - self.last_rot.0) as f64;
        let d4 = (self.pitch - self.last_rot.1) as f64;
        let moved = d0 * d0 + d1 * d1 + d2 * d2 > 9.0e-4 || self.position_update_ticks >= 20;
        let rotated = d3 != 0.0 || d4 != 0.0;
        let packet = match (moved, rotated) {
            (true, true) => MovePacket::PosLook,
            (true, false) => MovePacket::Pos,
            (false, true) => MovePacket::Look,
            (false, false) => MovePacket::Ground,
        };
        self.position_update_ticks += 1;
        if moved {
            self.last_pos = DVec3::new(self.pos.x, self.bb.min.y, self.pos.z);
            self.position_update_ticks = 0;
        }
        if rotated {
            self.last_rot = (self.yaw, self.pitch);
        }
        packet
    }

    /// `AbstractClientPlayer.getFovModifier`.
    pub fn fov_modifier(&self) -> f32 {
        let mut f = 1.0f32;
        if self.flying {
            f *= 1.1;
        }
        f = (f as f64 * ((self.speed.value() / self.walk_speed as f64 + 1.0) / 2.0)) as f32;
        if self.walk_speed == 0.0 || f.is_nan() || f.is_infinite() {
            f = 1.0;
        }
        if let Some(u) = self.item_use {
            if u.action == UseAction::Bow {
                let mut f1 = (u.max_duration - u.count) as f32 / 20.0;
                if f1 > 1.0 {
                    f1 = 1.0;
                } else {
                    f1 *= f1;
                }
                f *= 1.0 - f1 * 0.15;
            }
        }
        f
    }
}

#[cfg(test)]
mod tests;

/// `EntityPlayer.onUpdate` depois do `super.onUpdate`: o `chasingPos` anda 1/4 até a posição e pula para ela
/// quando fica a mais de 10 blocos em algum eixo.
pub fn chase(prev: &mut DVec3, chasing: &mut DVec3, pos: DVec3) {
    *prev = *chasing;
    let d = pos - *chasing;
    for (i, v) in d.to_array().into_iter().enumerate() {
        if v.abs() > 10.0 {
            prev[i] = pos[i];
            chasing[i] = pos[i];
        }
    }
    chasing.x += d.x * 0.25;
    chasing.z += d.z * 0.25;
    chasing.y += d.y * 0.25;
}
