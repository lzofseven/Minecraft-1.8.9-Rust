//! Estado de bloco da 1.8: `id + meta` (+ vizinhos) -> arquivo de blockstate e variante.
//!
//! Reproduz `getStateFromMeta`, `getActualState` e os mapeadores de
//! `BlockModelShapes.registerAllBlocks`: o nome do arquivo vem do registro (ou de uma
//! propriedade com sufixo) e a variante é `chave=valor` em ordem alfabética, sem as
//! propriedades ignoradas; sem propriedades vira `normal`.
//!
//! O que depende dos vizinhos fica num número pequeno (`actual_bits`) para a tabela
//! de variantes ser montada uma vez só e a malha não precisar montar strings.

/// Faces na ordem do `EnumFacing`.
pub const FACINGS: [&str; 6] = ["down", "up", "north", "south", "west", "east"];
pub const DIRS: [[i32; 3]; 6] = [[0, -1, 0], [0, 1, 0], [0, 0, -1], [0, 0, 1], [-1, 0, 0], [1, 0, 0]];
const D: usize = 0;
const U: usize = 1;
const N: usize = 2;
const S: usize = 3;
const W: usize = 4;
const E: usize = 5;

/// `EnumFacing.getHorizontal`: 0 sul, 1 oeste, 2 norte, 3 leste.
const HORIZ: [usize; 4] = [S, W, N, E];

fn horizontal(i: u16) -> usize {
    HORIZ[(i & 3) as usize]
}

fn front(i: u16) -> usize {
    (i as usize) % 6
}

/// Gira no sentido horário visto de cima (norte -> leste).
pub fn rot_y(f: usize) -> usize {
    match f {
        N => E,
        E => S,
        S => W,
        W => N,
        x => x,
    }
}

pub fn rot_y_ccw(f: usize) -> usize {
    match f {
        N => W,
        W => S,
        S => E,
        E => N,
        x => x,
    }
}

pub const COLORS: [&str; 16] = [
    "white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "silver", "cyan", "purple",
    "blue", "brown", "green", "red", "black",
];
const WOODS: [&str; 6] = ["oak", "spruce", "birch", "jungle", "acacia", "dark_oak"];

pub fn is_stairs(id: u16) -> bool {
    matches!(id, 53 | 67 | 108 | 109 | 114 | 128 | 134 | 135 | 136 | 156 | 163 | 164 | 180)
}

/// Lajes de meia altura (`BlockSlab.isSlab`).
pub fn is_half_slab(id: u16) -> bool {
    matches!(id, 44 | 126 | 182)
}

pub fn is_slab(id: u16) -> bool {
    matches!(id, 43 | 44 | 125 | 126 | 181 | 182)
}

pub fn is_fence(id: u16) -> bool {
    matches!(id, 85 | 113 | 188..=192)
}

pub fn is_gate(id: u16) -> bool {
    matches!(id, 107 | 183..=187)
}

pub fn is_pane(id: u16) -> bool {
    matches!(id, 101 | 102 | 160)
}

pub fn is_door(id: u16) -> bool {
    matches!(id, 64 | 71 | 193..=197)
}

/// Blocos desenhados por código próprio no jogo (sem modelo JSON).
pub fn is_builtin(id: u16) -> bool {
    matches!(id, 0 | 8 | 9 | 10 | 11 | 36 | 54 | 63 | 68 | 119 | 130 | 144 | 146 | 166 | 176 | 177)
}

/// Propriedades físicas do bloco usadas pelas regras de estado.
pub trait Props {
    /// `isOpaqueCube`.
    fn opaque(&self, st: u16) -> bool;
    /// `isBlockNormalCube` (material sólido e cubo inteiro).
    fn normal_cube(&self, st: u16) -> bool;
    /// `material.isOpaque() && isFullCube()`.
    fn opaque_full(&self, st: u16) -> bool;
    /// `isFullBlock`.
    fn full_block(&self, st: u16) -> bool;
}

/// Vizinhança usada pela forma de colisão/seleção (mesma codificação do gerador):
/// conexões N/L/S/O de cercas, painéis e muros, forma da escada, a outra metade
/// da porta e o baú vizinho (0 nenhum, 1 N, 2 S, 3 O, 4 L).
pub fn shape_bits(p: &dyn Props, st: u16, nb: Nb) -> u32 {
    let id = st >> 4;
    if is_fence(id) || is_pane(id) || id == 139 {
        actual_bits(p, st, [0; 3], nb) & 15
    } else if is_stairs(id) || is_door(id) {
        actual_bits(p, st, [0; 3], nb)
    } else if id == 54 || id == 146 {
        [N, S, W, E].iter().position(|&f| at(nb, f) >> 4 == id).map_or(0, |i| i as u32 + 1)
    } else {
        0
    }
}

/// Quantas combinações de `actual_bits` o bloco tem (1 = não depende dos vizinhos).
pub fn dyn_count(id: u16) -> u32 {
    match id {
        2 | 3 | 110 | 106 | 131 | 93 | 94 | 107 | 183..=187 => 2,
        _ if is_stairs(id) => 5,
        _ if is_fence(id) || is_pane(id) => 16,
        139 => 32,
        _ if is_door(id) => 8,
        55 => 256,
        132 => 16,
        104 | 105 => 5,
        175 => 8,
        51 => 256,
        _ => 1,
    }
}

type Nb<'a> = &'a dyn Fn(i32, i32, i32) -> u16;

fn at(nb: Nb, f: usize) -> u16 {
    let d = DIRS[f];
    nb(d[0], d[1], d[2])
}

/// `World.doesBlockHaveSolidTopSurface`.
fn solid_top(p: &dyn Props, st: u16) -> bool {
    let (id, m) = (st >> 4, st & 15);
    p.opaque_full(st)
        || (is_stairs(id) && m & 4 != 0)
        || (is_half_slab(id) && m & 8 != 0)
        || is_slab(id) && !is_half_slab(id)
        || id == 154
        || (id == 78 && m & 7 == 6)
}

fn flammable(id: u16) -> bool {
    matches!(
        id,
        5 | 125 | 126 | 107 | 183..=187 | 85 | 188..=192 | 53 | 134 | 135 | 136 | 17 | 162 | 18 | 161 | 47 | 46 | 31
            | 175 | 37 | 38 | 32 | 35 | 106 | 173 | 170 | 171
    )
}

fn stair_facing(m: u16) -> usize {
    front(5 - (m & 3))
}

/// Forma da escada (`BlockStairs.getActualState`): 0 reta, 1 inner_left,
/// 2 inner_right, 3 outer_left, 4 outer_right.
fn stair_shape(st: u16, nb: Nb) -> u32 {
    let m = st & 15;
    let facing = stair_facing(m);
    let top = m & 4 != 0;
    let same = |f: usize| {
        let o = at(nb, f);
        is_stairs(o >> 4) && (o & 4) == (m & 4) && stair_facing(o & 15) == facing
    };
    // 1 = "direita" (o vizinho aponta no sentido horário), 2 = "esquerda"; invertido no topo.
    let side = |o: u16| -> Option<usize> {
        if is_stairs(o >> 4) && (o & 4) == (m & 4) {
            let of = stair_facing(o & 15);
            if of == rot_y(facing) || of == rot_y_ccw(facing) {
                return Some(of);
            }
        }
        None
    };
    let kind = |of: usize| -> u32 {
        let cw = of == rot_y(facing);
        match (cw, top) {
            (true, false) | (false, true) => 1,
            _ => 2,
        }
    };
    // Atrás (na direção do facing): canto externo.
    if let Some(of) = side(at(nb, facing)) {
        if !same(of ^ 1) {
            return if kind(of) == 1 { 4 } else { 3 };
        }
    }
    // Na frente: canto interno.
    if let Some(of) = side(at(nb, facing ^ 1)) {
        if !same(of) {
            return if kind(of) == 1 { 2 } else { 1 };
        }
    }
    0
}

fn fence_connects(p: &dyn Props, id: u16, n: u16) -> bool {
    let nid = n >> 4;
    if nid == 166 {
        return false;
    }
    if (is_fence(nid) && (nid == 113) == (id == 113)) || is_gate(nid) {
        return true;
    }
    p.opaque_full(n) && !matches!(nid, 86 | 91 | 103)
}

fn wall_connects(p: &dyn Props, n: u16) -> bool {
    let nid = n >> 4;
    if nid == 166 {
        return false;
    }
    if nid == 139 || is_gate(nid) {
        return true;
    }
    p.opaque_full(n) && !matches!(nid, 86 | 91 | 103)
}

fn pane_connects(p: &dyn Props, id: u16, n: u16) -> bool {
    let nid = n >> 4;
    p.full_block(n) || nid == id || nid == 20 || nid == 95 || is_pane(nid)
}

fn provides_power(id: u16) -> bool {
    matches!(id, 69 | 70 | 72 | 147 | 148 | 75 | 76 | 77 | 143 | 28 | 131 | 146 | 151 | 178 | 152 | 149 | 150 | 93 | 94)
}

/// `BlockRedstoneWire.canConnectTo`.
fn wire_connects(st: u16, side: Option<usize>) -> bool {
    let id = st >> 4;
    match id {
        55 => true,
        93 | 94 => match side {
            Some(s) => {
                let f = horizontal(st & 3);
                f == s || f ^ 1 == s
            }
            None => false,
        },
        _ => provides_power(id) && side.is_some(),
    }
}

/// 0 nada, 1 lado, 2 sobe.
fn wire_attach(p: &dyn Props, nb: Nb, d: usize) -> u32 {
    let [dx, _, dz] = DIRS[d];
    let b = nb(dx, 0, dz);
    let bnorm = p.normal_cube(b);
    if !wire_connects(b, Some(d)) && (bnorm || !wire_connects(nb(dx, -1, dz), None)) {
        let above = nb(0, 1, 0);
        if !p.normal_cube(above) && bnorm && wire_connects(nb(dx, 1, dz), None) {
            2
        } else {
            0
        }
    } else {
        1
    }
}

/// Parte do estado que vem dos vizinhos (`getActualState`), em `0..dyn_count(id)`.
pub fn actual_bits(p: &dyn Props, st: u16, pos: [i32; 3], nb: Nb) -> u32 {
    let (id, m) = (st >> 4, st & 15);
    let bit = |b: bool, i: u32| (b as u32) << i;
    match id {
        2 | 110 | 3 => {
            if id == 3 && m != 2 {
                return 0;
            }
            let up = at(nb, U) >> 4;
            bit(up == 78 || up == 80, 0)
        }
        _ if is_stairs(id) => stair_shape(st, nb),
        _ if is_fence(id) => {
            [N, E, S, W].iter().enumerate().map(|(i, &f)| bit(fence_connects(p, id, at(nb, f)), i as u32)).sum()
        }
        _ if is_pane(id) => {
            [N, E, S, W].iter().enumerate().map(|(i, &f)| bit(pane_connects(p, id, at(nb, f)), i as u32)).sum()
        }
        139 => {
            let sides: u32 = [N, E, S, W].iter().enumerate().map(|(i, &f)| bit(wall_connects(p, at(nb, f)), i as u32)).sum();
            sides | bit(at(nb, U) != 0, 4)
        }
        _ if is_gate(id) => {
            let f = horizontal(m);
            let (a, b) = if f == N || f == S { (W, E) } else { (N, S) };
            bit(at(nb, a) >> 4 == 139 || at(nb, b) >> 4 == 139, 0)
        }
        _ if is_door(id) => {
            if m & 8 == 0 {
                let up = at(nb, U);
                if up >> 4 == id {
                    (up & 1) as u32
                } else {
                    // Sem a outra metade fica o estado do BLOCK_STATE_IDS (o último válido com
                    // esse meta): dobradiça à direita.
                    1
                }
            } else {
                let down = at(nb, D);
                if down >> 4 == id {
                    // Facing horizontal (índice) e aberta.
                    let f = rot_y_ccw(horizontal(down & 3));
                    let hi = HORIZ.iter().position(|&h| h == f).unwrap_or(2) as u32;
                    hi | bit(down & 4 != 0, 2)
                } else {
                    // Idem: virada para leste e fechada.
                    3
                }
            }
        }
        55 => [W, E, N, S].iter().enumerate().map(|(i, &d)| wire_attach(p, nb, d) << (i * 2)).sum(),
        132 => {
            let suspended = m & 2 != 0;
            [N, E, S, W]
                .iter()
                .enumerate()
                .map(|(i, &d)| {
                    let n = at(nb, d);
                    let c = match n >> 4 {
                        131 => horizontal(n & 3) == d ^ 1,
                        132 => (n & 2 != 0) == suspended,
                        _ => false,
                    };
                    bit(c, i as u32)
                })
                .sum()
        }
        131 => bit(!solid_top(p, at(nb, D)), 0),
        93 | 94 => {
            let f = horizontal(m & 3);
            let locked = [rot_y(f), rot_y_ccw(f)].iter().any(|&side| {
                let n = at(nb, side);
                let nid = n >> 4;
                let powered = nid == 94 || (matches!(nid, 149 | 150) && n & 8 != 0);
                powered && horizontal(n & 3) == side
            });
            bit(locked, 0)
        }
        106 => bit(p.normal_cube(at(nb, U)), 0),
        104 | 105 => {
            let crop = if id == 104 { 86 } else { 103 };
            [N, E, S, W].iter().position(|&f| at(nb, f) >> 4 == crop).map_or(0, |i| i as u32 + 1)
        }
        175 => {
            if m & 8 != 0 {
                let down = at(nb, D);
                if down >> 4 == 175 {
                    ((down & 7) as u32).min(5)
                } else {
                    // Sem a metade de baixo o estado do jogo é a peônia (último tipo).
                    5
                }
            } else {
                0
            }
        }
        51 => {
            let below = at(nb, D);
            if solid_top(p, below) || flammable(below >> 4) {
                return 0;
            }
            let [x, y, z] = pos;
            let alt = (x + y + z) & 1 == 1;
            let flip = (x / 2 + y / 2 + z / 2) & 1 == 1;
            let upper = if flammable(at(nb, U) >> 4) {
                if alt {
                    1
                } else {
                    2
                }
            } else {
                0
            };
            bit(alt, 0)
                | bit(flip, 1)
                | [N, E, S, W].iter().enumerate().map(|(i, &f)| bit(flammable(at(nb, f) >> 4), i as u32 + 2)).sum::<u32>()
                | upper << 6
        }
        _ => 0,
    }
}

fn b(v: bool) -> String {
    if v { "true" } else { "false" }.to_string()
}

fn s(v: &str) -> String {
    v.to_string()
}

/// Monta "a=1,b=2" em ordem alfabética, ou "normal".
fn variant(mut props: Vec<(&str, String)>) -> String {
    if props.is_empty() {
        return "normal".to_string();
    }
    props.sort_by(|a, b| a.0.cmp(b.0));
    props.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(",")
}

const RAILS: [&str; 10] = [
    "north_south", "east_west", "ascending_east", "ascending_west", "ascending_north", "ascending_south",
    "south_east", "south_west", "north_west", "north_east",
];

fn facing_name(f: usize) -> String {
    FACINGS[f].to_string()
}

fn nesw(bits: u32) -> Vec<(&'static str, String)> {
    vec![("north", b(bits & 1 != 0)), ("east", b(bits & 2 != 0)), ("south", b(bits & 4 != 0)), ("west", b(bits & 8 != 0))]
}

/// Arquivo de blockstate (sem `.json`) e variante; `None` para blocos sem modelo.
pub fn location(id: u16, m: u16, bits: u32) -> Option<(String, String)> {
    if is_builtin(id) || id > 197 {
        return None;
    }
    let reg = |name: &str, props: Vec<(&str, String)>| Some((name.to_string(), variant(props)));
    let normal = |name: String| Some((name, "normal".to_string()));
    match id {
        1 => normal(
            ["stone", "granite", "smooth_granite", "diorite", "smooth_diorite", "andesite", "smooth_andesite"]
                .get(m as usize)
                .unwrap_or(&"stone")
                .to_string(),
        ),
        2 | 110 => reg(if id == 2 { "grass" } else { "mycelium" }, vec![("snowy", b(bits & 1 != 0))]),
        3 => match m {
            1 => normal(s("coarse_dirt")),
            2 => reg("podzol", vec![("snowy", b(bits & 1 != 0))]),
            _ => normal(s("dirt")),
        },
        4 => normal(s("cobblestone")),
        5 => normal(format!("{}_planks", WOODS[(m as usize).min(5)])),
        6 => Some((format!("{}_sapling", WOODS[((m & 7) as usize).min(5)]), variant(vec![("stage", (m >> 3).to_string())]))),
        7 => normal(s("bedrock")),
        12 => normal(s(if m == 1 { "red_sand" } else { "sand" })),
        13 => normal(s("gravel")),
        14 => normal(s("gold_ore")),
        15 => normal(s("iron_ore")),
        16 => normal(s("coal_ore")),
        17 | 162 => {
            let w = if id == 17 { WOODS[(m & 3) as usize] } else { WOODS[4 + (m & 1) as usize] };
            let axis = ["y", "x", "z", "none"][(m >> 2) as usize];
            Some((format!("{w}_log"), variant(vec![("axis", s(axis))])))
        }
        18 | 161 => {
            let w = if id == 18 { WOODS[(m & 3) as usize] } else { WOODS[4 + (m & 1) as usize] };
            normal(format!("{w}_leaves"))
        }
        19 => reg("sponge", vec![("wet", b(m & 1 != 0))]),
        20 => normal(s("glass")),
        21 => normal(s("lapis_ore")),
        22 => normal(s("lapis_block")),
        23 | 158 => reg(if id == 23 { "dispenser" } else { "dropper" }, vec![("facing", facing_name(front(m & 7)))]),
        24 | 179 => {
            let n = if id == 24 {
                ["sandstone", "chiseled_sandstone", "smooth_sandstone"]
            } else {
                ["red_sandstone", "chiseled_red_sandstone", "smooth_red_sandstone"]
            };
            normal(s(n[(m as usize).min(2)]))
        }
        25 => normal(s("noteblock")),
        26 => reg(
            "bed",
            vec![("facing", facing_name(horizontal(m))), ("part", s(if m & 8 != 0 { "head" } else { "foot" }))],
        ),
        27 | 28 | 157 => {
            let name = match id {
                27 => "golden_rail",
                28 => "detector_rail",
                _ => "activator_rail",
            };
            reg(name, vec![("powered", b(m & 8 != 0)), ("shape", s(RAILS[((m & 7) as usize).min(5)]))])
        }
        66 => reg("rail", vec![("shape", s(RAILS[(m as usize).min(9)]))]),
        29 | 33 => {
            let name = if id == 29 { "sticky_piston" } else { "piston" };
            reg(name, vec![("extended", b(m & 8 != 0)), ("facing", facing_name(front(m & 7)))])
        }
        34 => reg(
            "piston_head",
            vec![
                ("facing", facing_name(front(m & 7))),
                ("short", b(false)),
                ("type", s(if m & 8 != 0 { "sticky" } else { "normal" })),
            ],
        ),
        30 => normal(s("web")),
        31 => normal(s(["dead_bush", "tall_grass", "fern"][(m as usize).min(2)])),
        32 => normal(s("dead_bush")),
        35 => normal(format!("{}_wool", COLORS[m as usize])),
        37 => normal(s("dandelion")),
        38 => normal(s([
            "poppy", "blue_orchid", "allium", "houstonia", "red_tulip", "orange_tulip", "white_tulip", "pink_tulip",
            "oxeye_daisy",
        ][(m as usize).min(8)])),
        39 => normal(s("brown_mushroom")),
        40 => normal(s("red_mushroom")),
        41 => normal(s("gold_block")),
        42 => normal(s("iron_block")),
        43 | 44 => {
            let v = ["stone", "sandstone", "wood_old", "cobblestone", "brick", "stone_brick", "nether_brick", "quartz"][(m & 7) as usize];
            if id == 43 {
                Some((format!("{v}_double_slab"), s(if m & 8 != 0 { "all" } else { "normal" })))
            } else {
                Some((format!("{v}_slab"), variant(vec![("half", s(if m & 8 != 0 { "top" } else { "bottom" }))])))
            }
        }
        181 => Some((s("red_sandstone_double_slab"), s(if m & 8 != 0 { "all" } else { "normal" }))),
        182 => Some((s("red_sandstone_slab"), variant(vec![("half", s(if m & 8 != 0 { "top" } else { "bottom" }))]))),
        125 => normal(format!("{}_double_slab", WOODS[((m & 7) as usize).min(5)])),
        126 => Some((
            format!("{}_slab", WOODS[((m & 7) as usize).min(5)]),
            variant(vec![("half", s(if m & 8 != 0 { "top" } else { "bottom" }))]),
        )),
        45 => normal(s("brick_block")),
        46 => normal(s("tnt")),
        47 => normal(s("bookshelf")),
        48 => normal(s("mossy_cobblestone")),
        49 => normal(s("obsidian")),
        50 | 75 | 76 => {
            let f = match m {
                1 => E,
                2 => W,
                3 => S,
                4 => N,
                _ => U,
            };
            let name = match id {
                50 => "torch",
                75 => "unlit_redstone_torch",
                _ => "redstone_torch",
            };
            reg(name, vec![("facing", facing_name(f))])
        }
        51 => {
            let mut p = vec![("alt", b(bits & 1 != 0)), ("flip", b(bits & 2 != 0)), ("upper", (bits >> 6).to_string())];
            p.extend(nesw(bits >> 2));
            reg("fire", p)
        }
        52 => normal(s("mob_spawner")),
        _ if is_stairs(id) => {
            let name = match id {
                53 => "oak_stairs",
                67 => "stone_stairs",
                108 => "brick_stairs",
                109 => "stone_brick_stairs",
                114 => "nether_brick_stairs",
                128 => "sandstone_stairs",
                134 => "spruce_stairs",
                135 => "birch_stairs",
                136 => "jungle_stairs",
                156 => "quartz_stairs",
                163 => "acacia_stairs",
                164 => "dark_oak_stairs",
                _ => "red_sandstone_stairs",
            };
            let shape = ["straight", "inner_left", "inner_right", "outer_left", "outer_right"][bits as usize % 5];
            reg(
                name,
                vec![
                    ("facing", facing_name(stair_facing(m))),
                    ("half", s(if m & 4 != 0 { "top" } else { "bottom" })),
                    ("shape", s(shape)),
                ],
            )
        }
        55 => {
            let a = |i: u32| s(["none", "side", "up", "none"][((bits >> (i * 2)) & 3) as usize]);
            reg("redstone_wire", vec![("west", a(0)), ("east", a(1)), ("north", a(2)), ("south", a(3))])
        }
        56 => normal(s("diamond_ore")),
        57 => normal(s("diamond_block")),
        58 => normal(s("crafting_table")),
        59 => reg("wheat", vec![("age", (m & 7).to_string())]),
        60 => reg("farmland", vec![("moisture", (m & 7).to_string())]),
        61 | 62 => {
            let mut f = front(m);
            if f < 2 {
                f = N;
            }
            reg(if id == 61 { "furnace" } else { "lit_furnace" }, vec![("facing", facing_name(f))])
        }
        65 => {
            let mut f = front(m);
            if f < 2 {
                f = N;
            }
            reg("ladder", vec![("facing", facing_name(f))])
        }
        _ if is_door(id) => {
            let name = match id {
                64 => "wooden_door",
                71 => "iron_door",
                193 => "spruce_door",
                194 => "birch_door",
                195 => "jungle_door",
                196 => "acacia_door",
                _ => "dark_oak_door",
            };
            let (facing, open, hinge, upper) = if m & 8 == 0 {
                (rot_y_ccw(horizontal(m & 3)), m & 4 != 0, bits & 1 != 0, false)
            } else {
                (HORIZ[(bits & 3) as usize], bits & 4 != 0, m & 1 != 0, true)
            };
            reg(
                name,
                vec![
                    ("facing", facing_name(facing)),
                    ("half", s(if upper { "upper" } else { "lower" })),
                    ("hinge", s(if hinge { "right" } else { "left" })),
                    ("open", b(open)),
                ],
            )
        }
        69 => {
            let f = ["down_x", "east", "west", "south", "north", "up_z", "up_x", "down_z"][(m & 7) as usize];
            reg("lever", vec![("facing", s(f)), ("powered", b(m & 8 != 0))])
        }
        70 | 72 => reg(
            if id == 70 { "stone_pressure_plate" } else { "wooden_pressure_plate" },
            vec![("powered", b(m == 1))],
        ),
        147 | 148 => reg(
            if id == 147 { "light_weighted_pressure_plate" } else { "heavy_weighted_pressure_plate" },
            vec![("power", m.to_string())],
        ),
        73 => normal(s("redstone_ore")),
        74 => normal(s("lit_redstone_ore")),
        77 | 143 => {
            let f = match m & 7 {
                0 => D,
                1 => E,
                2 => W,
                3 => S,
                4 => N,
                _ => U,
            };
            reg(
                if id == 77 { "stone_button" } else { "wooden_button" },
                vec![("facing", facing_name(f)), ("powered", b(m & 8 != 0))],
            )
        }
        78 => reg("snow_layer", vec![("layers", ((m & 7) + 1).to_string())]),
        79 => normal(s("ice")),
        80 => normal(s("snow")),
        81 => normal(s("cactus")),
        82 => normal(s("clay")),
        83 => normal(s("reeds")),
        84 => normal(s("jukebox")),
        _ if is_fence(id) => {
            let name = match id {
                85 => "fence",
                113 => "nether_brick_fence",
                188 => "spruce_fence",
                189 => "birch_fence",
                190 => "jungle_fence",
                191 => "dark_oak_fence",
                _ => "acacia_fence",
            };
            reg(name, nesw(bits))
        }
        86 | 91 => reg(if id == 86 { "pumpkin" } else { "lit_pumpkin" }, vec![("facing", facing_name(horizontal(m)))]),
        87 => normal(s("netherrack")),
        88 => normal(s("soul_sand")),
        89 => normal(s("glowstone")),
        90 => reg("portal", vec![("axis", s(if m & 3 == 2 { "z" } else { "x" }))]),
        92 => reg("cake", vec![("bites", (m & 7).min(6).to_string())]),
        93 | 94 => reg(
            if id == 93 { "unpowered_repeater" } else { "powered_repeater" },
            vec![
                ("delay", ((m >> 2) + 1).to_string()),
                ("facing", facing_name(horizontal(m))),
                ("locked", b(bits & 1 != 0)),
            ],
        ),
        95 => normal(format!("{}_stained_glass", COLORS[m as usize])),
        96 | 167 => {
            let f = [N, S, W, E][(m & 3) as usize];
            reg(
                if id == 96 { "trapdoor" } else { "iron_trapdoor" },
                vec![
                    ("facing", facing_name(f)),
                    ("half", s(if m & 8 != 0 { "top" } else { "bottom" })),
                    ("open", b(m & 4 != 0)),
                ],
            )
        }
        97 => normal(format!(
            "{}_monster_egg",
            ["stone", "cobblestone", "stone_brick", "mossy_brick", "cracked_brick", "chiseled_brick"][(m as usize).min(5)]
        )),
        98 => normal(s(["stonebrick", "mossy_stonebrick", "cracked_stonebrick", "chiseled_stonebrick"][(m & 3) as usize])),
        99 | 100 => {
            let v = match m {
                0 => "all_inside",
                1 => "north_west",
                2 => "north",
                3 => "north_east",
                4 => "west",
                5 => "center",
                6 => "east",
                7 => "south_west",
                8 => "south",
                9 => "south_east",
                10 => "stem",
                14 => "all_outside",
                15 => "all_stem",
                _ => "all_inside",
            };
            reg(if id == 99 { "brown_mushroom_block" } else { "red_mushroom_block" }, vec![("variant", s(v))])
        }
        101 | 102 => reg(if id == 101 { "iron_bars" } else { "glass_pane" }, nesw(bits)),
        160 => Some((format!("{}_stained_glass_pane", COLORS[m as usize]), variant(nesw(bits)))),
        103 => normal(s("melon_block")),
        104 | 105 => {
            let name = if id == 104 { "pumpkin_stem" } else { "melon_stem" };
            if bits == 0 {
                reg(name, vec![("age", (m & 7).to_string()), ("facing", s("up"))])
            } else {
                reg(name, vec![("facing", facing_name([N, E, S, W][(bits - 1) as usize]))])
            }
        }
        106 => reg(
            "vine",
            vec![
                ("south", b(m & 1 != 0)),
                ("west", b(m & 2 != 0)),
                ("north", b(m & 4 != 0)),
                ("east", b(m & 8 != 0)),
                ("up", b(bits & 1 != 0)),
            ],
        ),
        _ if is_gate(id) => {
            let name = match id {
                107 => "fence_gate",
                183 => "spruce_fence_gate",
                184 => "birch_fence_gate",
                185 => "jungle_fence_gate",
                186 => "dark_oak_fence_gate",
                _ => "acacia_fence_gate",
            };
            reg(
                name,
                vec![("facing", facing_name(horizontal(m))), ("in_wall", b(bits & 1 != 0)), ("open", b(m & 4 != 0))],
            )
        }
        111 => normal(s("waterlily")),
        112 => normal(s("nether_brick")),
        115 => reg("nether_wart", vec![("age", (m & 3).to_string())]),
        116 => normal(s("enchanting_table")),
        117 => reg(
            "brewing_stand",
            vec![("has_bottle_0", b(m & 1 != 0)), ("has_bottle_1", b(m & 2 != 0)), ("has_bottle_2", b(m & 4 != 0))],
        ),
        118 => reg("cauldron", vec![("level", (m & 3).to_string())]),
        120 => reg("end_portal_frame", vec![("eye", b(m & 4 != 0)), ("facing", facing_name(horizontal(m)))]),
        121 => normal(s("end_stone")),
        122 => normal(s("dragon_egg")),
        123 => normal(s("redstone_lamp")),
        124 => normal(s("lit_redstone_lamp")),
        127 => reg("cocoa", vec![("age", ((m >> 2) & 3).min(2).to_string()), ("facing", facing_name(horizontal(m)))]),
        129 => normal(s("emerald_ore")),
        131 => reg(
            "tripwire_hook",
            vec![
                ("attached", b(m & 4 != 0)),
                ("facing", facing_name(horizontal(m))),
                ("powered", b(m & 8 != 0)),
                ("suspended", b(bits & 1 != 0)),
            ],
        ),
        132 => {
            let mut p = vec![("attached", b(m & 4 != 0)), ("suspended", b(m & 2 != 0))];
            p.extend(nesw(bits));
            reg("tripwire", p)
        }
        133 => normal(s("emerald_block")),
        137 => normal(s("command_block")),
        138 => normal(s("beacon")),
        139 => {
            let mut p = nesw(bits);
            p.push(("up", b(bits & 16 != 0)));
            Some((s(if m & 1 != 0 { "mossy_cobblestone_wall" } else { "cobblestone_wall" }), variant(p)))
        }
        140 => reg("flower_pot", vec![("contents", s("empty"))]),
        141 | 142 => reg(if id == 141 { "carrots" } else { "potatoes" }, vec![("age", (m & 7).to_string())]),
        145 => reg("anvil", vec![("damage", ((m >> 2) & 3).min(2).to_string()), ("facing", facing_name(horizontal(m)))]),
        149 | 150 => reg(
            if id == 149 { "unpowered_comparator" } else { "powered_comparator" },
            vec![
                ("facing", facing_name(horizontal(m))),
                ("mode", s(if m & 4 != 0 { "subtract" } else { "compare" })),
                ("powered", b(m & 8 != 0)),
            ],
        ),
        151 | 178 => reg(
            if id == 151 { "daylight_detector" } else { "daylight_detector_inverted" },
            vec![("power", m.to_string())],
        ),
        152 => normal(s("redstone_block")),
        153 => normal(s("quartz_ore")),
        154 => reg("hopper", vec![("facing", facing_name(front(m & 7)))]),
        155 => match m {
            1 => normal(s("chiseled_quartz_block")),
            2 => Some((s("quartz_column"), s("axis=y"))),
            3 => Some((s("quartz_column"), s("axis=x"))),
            4 => Some((s("quartz_column"), s("axis=z"))),
            _ => normal(s("quartz_block")),
        },
        159 => normal(format!("{}_stained_hardened_clay", COLORS[m as usize])),
        165 => normal(s("slime")),
        168 => normal(s(["prismarine", "prismarine_bricks", "dark_prismarine"][(m as usize).min(2)])),
        169 => normal(s("sea_lantern")),
        170 => {
            let axis = match m & 12 {
                4 => "x",
                8 => "z",
                _ => "y",
            };
            reg("hay_block", vec![("axis", s(axis))])
        }
        171 => normal(format!("{}_carpet", COLORS[m as usize])),
        172 => normal(s("hardened_clay")),
        173 => normal(s("coal_block")),
        174 => normal(s("packed_ice")),
        175 => {
            let kind = if m & 8 != 0 { bits as usize } else { ((m & 7) as usize).min(5) };
            let name = ["sunflower", "syringa", "double_grass", "double_fern", "double_rose", "paeonia"][kind.min(5)];
            reg(name, vec![("half", s(if m & 8 != 0 { "upper" } else { "lower" }))])
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Simple;
    impl Props for Simple {
        fn opaque(&self, st: u16) -> bool {
            st >> 4 == 1
        }
        fn normal_cube(&self, st: u16) -> bool {
            st >> 4 == 1
        }
        fn opaque_full(&self, st: u16) -> bool {
            st >> 4 == 1
        }
        fn full_block(&self, st: u16) -> bool {
            st >> 4 == 1
        }
    }

    fn loc(id: u16, m: u16, bits: u32) -> (String, String) {
        location(id, m, bits).unwrap()
    }

    #[test]
    fn meta_to_variant() {
        assert_eq!(loc(1, 3, 0), ("diorite".into(), "normal".into()));
        // Escada de carvalho, meta 0 = leste, metade de baixo.
        assert_eq!(loc(53, 0, 0).1, "facing=east,half=bottom,shape=straight");
        assert_eq!(loc(53, 7, 0).1, "facing=north,half=top,shape=straight");
        // Laje de pedra em cima; laje dupla lisa.
        assert_eq!(loc(44, 8, 0), ("stone_slab".into(), "half=top".into()));
        assert_eq!(loc(43, 8, 0), ("stone_double_slab".into(), "all".into()));
        assert_eq!(loc(126, 5, 0), ("dark_oak_slab".into(), "half=bottom".into()));
        // Tronco de bétula deitado no eixo X.
        assert_eq!(loc(17, 6, 0), ("birch_log".into(), "axis=x".into()));
        assert_eq!(loc(162, 13, 0), ("dark_oak_log".into(), "axis=none".into()));
        assert_eq!(loc(155, 3, 0), ("quartz_column".into(), "axis=x".into()));
        assert_eq!(loc(35, 14, 0).0, "red_wool");
    }

    #[test]
    fn fence_with_neighbors() {
        // Pedra ao norte e cerca a leste.
        let nb = |dx: i32, _dy: i32, dz: i32| match (dx, dz) {
            (0, -1) => 1 << 4,
            (1, 0) => 85 << 4,
            _ => 0,
        };
        let bits = actual_bits(&Simple, 85 << 4, [0; 3], &nb);
        assert_eq!(loc(85, 0, bits).1, "east=true,north=true,south=false,west=false");
        // Cerca do Nether não liga na de madeira.
        let bits = actual_bits(&Simple, 113 << 4, [0; 3], &nb);
        assert_eq!(loc(113, 0, bits).1, "east=false,north=true,south=false,west=false");
    }

    #[test]
    fn stairs_corners() {
        // Escada para leste (meta 0) com escada para norte (meta 3) atrás: canto externo.
        let nb = |dx: i32, _dy: i32, dz: i32| if (dx, dz) == (1, 0) { 53 << 4 | 3 } else { 0 };
        let bits = actual_bits(&Simple, 53 << 4, [0; 3], &nb);
        assert_eq!(loc(53, 0, bits).1, "facing=east,half=bottom,shape=outer_left");
        // A mesma escada na frente: canto interno.
        let nb = |dx: i32, _dy: i32, dz: i32| if (dx, dz) == (-1, 0) { 53 << 4 | 3 } else { 0 };
        let bits = actual_bits(&Simple, 53 << 4, [0; 3], &nb);
        assert_eq!(loc(53, 0, bits).1, "facing=east,half=bottom,shape=inner_left");
    }

    #[test]
    fn door_halves_merge() {
        // Porta de baixo virada (meta 1 -> facing sul), aberta; a de cima com dobradiça à direita.
        let lower = 64 << 4 | 1 | 4;
        let upper = 64 << 4 | 8 | 1;
        let nb_l = move |_: i32, dy: i32, _: i32| if dy == 1 { upper } else { 0 };
        let nb_u = move |_: i32, dy: i32, _: i32| if dy == -1 { lower } else { 0 };
        let bl = actual_bits(&Simple, lower, [0; 3], &nb_l);
        let bu = actual_bits(&Simple, upper, [0; 3], &nb_u);
        assert_eq!(loc(64, lower & 15, bl).1, "facing=south,half=lower,hinge=right,open=true");
        assert_eq!(loc(64, upper & 15, bu).1, "facing=south,half=upper,hinge=right,open=true");
    }
}
