//! Tabela de blocos da 1.8: `id << 4 | meta` -> forma, texturas e colisão.
//!
//! Os nomes de textura são os arquivos de `assets/minecraft/textures/blocks/` do jar.

use std::collections::HashMap;

use crate::gen_shapes as gen;
use crate::model::{load_blockstate, Loader, ModelLib, Weighted};
use crate::states::{self, Props};

/// Caixa em 1/16 de bloco (colisão pode passar de 16, como as cercas).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Aabb {
    pub min: [u8; 3],
    pub max: [u8; 3],
}

impl Aabb {
    pub const FULL: Aabb = Aabb::new(0, 0, 0, 16, 16, 16);

    pub const fn new(x0: u8, y0: u8, z0: u8, x1: u8, y1: u8, z1: u8) -> Self {
        Self { min: [x0, y0, z0], max: [x1, y1, z1] }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    /// SOLID do `Block.getBlockLayer`: desenhado sem teste de alfa. Bloco que o Java põe em CUTOUT ou
    /// CUTOUT_MIPPED tem de ser `Cutout`, senão os pixels transparentes da textura aparecem.
    Opaque,
    /// CUTOUT_MIPPED: recorte por alfa com mipmap (grama, folhas bonitas, painéis, funil).
    CutoutMipped,
    /// CUTOUT: recorte por alfa sem mipmap (`setBlurMipmap(false, false)`): vidro, plantas, tochas...
    Cutout,
    /// Mistura de alfa (água, gelo, vidro colorido).
    Translucent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Connect {
    Fence,
    Pane,
    Wall,
}

#[derive(Clone, Debug)]
pub enum Model {
    Air,
    /// Sólido e invisível (barrier).
    Invisible,
    Boxes(Vec<Aabb>),
    /// Planta em X.
    Cross,
    Liquid { lava: bool },
    /// Cercas, painéis e muros: a forma depende dos vizinhos.
    Connected(Connect),
    /// Porta: `tex` tem a metade de baixo e `upper` a de cima.
    Door { upper: u16 },
    /// Planta de dois blocos (175): o tipo fica na metade de baixo.
    DoublePlant,
    /// Ainda sem modelo: não é desenhado e não colide.
    Unsupported,
}

pub const DOWN: usize = 0;
pub const UP: usize = 1;
pub const NORTH: usize = 2;
pub const SOUTH: usize = 3;
pub const WEST: usize = 4;
pub const EAST: usize = 5;

#[derive(Clone, Debug)]
pub struct BlockInfo {
    pub model: Model,
    /// Texturas por face: baixo, cima, norte, sul, oeste, leste.
    pub tex: [u16; 6],
    pub layer: Layer,
    /// Cor RGB multiplicada nas faces (0xFFFFFF = sem tinta).
    pub tint: u32,
    pub tint_top_only: bool,
    /// Cubo opaco inteiro: esconde as faces vizinhas.
    pub full_opaque: bool,
    /// Esconde faces encostadas no mesmo bloco (vidro, água, gelo).
    pub cull_same: bool,
}

impl BlockInfo {
    fn air() -> Self {
        Self {
            model: Model::Air,
            tex: [0; 6],
            layer: Layer::Opaque,
            tint: 0xFFFFFF,
            tint_top_only: false,
            full_opaque: false,
            cull_same: false,
        }
    }

    fn layer(mut self, l: Layer) -> Self {
        self.layer = l;
        self
    }

    fn tint(mut self, c: u32) -> Self {
        self.tint = c;
        self
    }

    fn tint_top(mut self, c: u32) -> Self {
        self.tint = c;
        self.tint_top_only = true;
        self
    }

    fn cull_same(mut self) -> Self {
        self.cull_same = true;
        self
    }
}

const GRASS: u32 = 0x91BD59;
const FOLIAGE: u32 = 0x77AB2F;
const SPRUCE: u32 = 0x619961;
const BIRCH: u32 = 0x80A755;
const LILY: u32 = 0x208030;

const COLORS: [&str; 16] = [
    "white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "silver", "cyan", "purple",
    "blue", "brown", "green", "red", "black",
];
const WOODS: [&str; 6] = ["oak", "spruce", "birch", "jungle", "acacia", "big_oak"];

#[derive(Clone)]
pub struct BlockTable {
    infos: Vec<BlockInfo>,
    /// Texturas usadas, na ordem dos índices do atlas.
    pub textures: Vec<String>,
    names: HashMap<String, u16>,
    /// Planta dupla por tipo: [baixo, cima].
    pub double_plant: [[u16; 2]; 6],
    /// Rachaduras de quebra (destroy_stage_0..9).
    pub destroy: [u16; 10],
    unknown: BlockInfo,
    /// Bits por estado: ver `F_*`.
    flags: Vec<u8>,
    /// Luz emitida por id (`getLightValue`).
    emission: [u8; 256],
    /// Modelos do jar, quando carregados.
    pub models: Option<BlockModels>,
    /// `colormap/grass.png` e `foliage.png` do jar (cores por bioma).
    pub colormaps: Option<crate::biome::Colormaps>,
    /// Texturas do `BlockFluidRenderer`: [água, lava] x [parada, fluindo].
    pub fluid_tex: [[u16; 2]; 2],
}

const F_OPAQUE: u8 = 1;
const F_FULL: u8 = 2;
const F_TRANSLUCENT: u8 = 4;
const F_NEIGHBOR_LIGHT: u8 = 8;
const F_OPAQUE_FULL: u8 = 16;
const F_NORMAL: u8 = 32;
const F_FULL_BLOCK: u8 = 64;

const NO_MODEL: u32 = u32::MAX;

/// Variantes assadas e a tabela estado (+ bits dos vizinhos) -> variante.
#[derive(Clone)]
pub struct BlockModels {
    pub variants: Vec<Weighted>,
    base: Vec<u32>,
    slots: Vec<u32>,
    /// Estados cuja variante não existe no jar ("arquivo#variante").
    pub missing: Vec<String>,
    pub files: usize,
}

impl BlockModels {
    /// Variante do estado com os bits de `states::actual_bits`.
    #[inline]
    pub fn get(&self, state: u16, bits: u32) -> Option<&Weighted> {
        let b = *self.base.get(state as usize)?;
        if b == NO_MODEL {
            return None;
        }
        let v = *self.slots.get((b + bits) as usize)?;
        self.variants.get(v as usize)
    }

    #[inline]
    pub fn has(&self, state: u16) -> bool {
        self.base.get(state as usize).is_some_and(|b| *b != NO_MODEL)
    }
}

impl Props for BlockTable {
    fn opaque(&self, st: u16) -> bool {
        self.opaque_cube(st)
    }
    fn normal_cube(&self, st: u16) -> bool {
        self.normal_cube(st)
    }
    fn opaque_full(&self, st: u16) -> bool {
        self.flag(st, F_OPAQUE_FULL)
    }
    fn full_block(&self, st: u16) -> bool {
        self.full_block(st)
    }
}

impl Default for BlockTable {
    fn default() -> Self {
        Self::new()
    }
}

impl BlockTable {
    pub fn new() -> Self {
        let mut unknown = BlockInfo::air();
        unknown.model = Model::Unsupported;
        let mut t = Self {
            infos: vec![unknown.clone(); 4096],
            textures: Vec::new(),
            names: HashMap::new(),
            double_plant: [[0; 2]; 6],
            destroy: [0; 10],
            unknown,
            flags: vec![0; 4096],
            emission: [0; 256],
            models: None,
            colormaps: None,
            fluid_tex: [[0; 2]; 2],
        };
        t.infos[0] = BlockInfo::air();
        t.register_all();
        for i in 0..10 {
            t.destroy[i] = t.tex(&format!("destroy_stage_{i}"));
        }
        t.fluid_tex = [[t.tex("water_still"), t.tex("water_flow")], [t.tex("lava_still"), t.tex("lava_flow")]];
        // Propriedades geradas do código do jogo (scripts/gen_shapes).
        for (st, info) in t.infos.iter_mut().enumerate() {
            let id = st >> 4;
            let g = gen::FLAGS[id];
            let mut f = 0;
            // Folhas: o cliente desenha com gráficos "Fancy", onde isOpaqueCube = false.
            if g & gen::F_OPAQUE != 0 && !matches!(id, 18 | 161) {
                f |= F_OPAQUE;
            }
            if g & gen::F_FULL_CUBE != 0 {
                f |= F_FULL;
            }
            if g & gen::F_NORMAL_CUBE != 0 {
                f |= F_NORMAL;
            }
            if g & gen::F_TRANSLUCENT != 0 {
                f |= F_TRANSLUCENT;
            }
            if g & gen::F_NEIGHBOR_LIGHT != 0 {
                f |= F_NEIGHBOR_LIGHT;
            }
            if g & gen::F_OPAQUE_FULL != 0 {
                f |= F_OPAQUE_FULL;
            }
            if g & gen::F_FULL_BLOCK != 0 {
                f |= F_FULL_BLOCK;
            }
            info.full_opaque = f & F_OPAQUE != 0;
            t.flags[st] = f;
            t.emission[id] = gen::LIGHT[id];
        }
        t
    }

    /// Tabela com os modelos de bloco do jar (`load` lê `assets/minecraft/...`).
    pub fn with_models(load: &mut Loader) -> Self {
        let mut t = Self::new();
        t.load_models(load);
        t.colormaps = crate::biome::Colormaps::load(load);
        t
    }

    fn load_models(&mut self, load: &mut Loader) {
        let mut lib = ModelLib::default();
        let mut files: HashMap<String, Option<HashMap<String, u32>>> = HashMap::new();
        let mut variants: Vec<Weighted> = Vec::new();
        let mut base = vec![NO_MODEL; 4096];
        let mut slots = Vec::new();
        let mut missing = Vec::new();
        for id in 0..198u16 {
            for m in 0..16u16 {
                if states::location(id, m, 0).is_none() {
                    continue;
                }
                let st = (id << 4 | m) as usize;
                base[st] = slots.len() as u32;
                for bits in 0..states::dyn_count(id) {
                    let mut slot = NO_MODEL;
                    if let Some((file, var)) = states::location(id, m, bits) {
                        let entry = files.entry(file.clone()).or_insert_with(|| {
                            let mut tex = |name: &str| {
                                let n = name.strip_prefix("blocks/").map(str::to_string).unwrap_or_else(|| format!("~{name}"));
                                self.tex(&n)
                            };
                            load_blockstate(&file, &mut lib, load, &mut tex).map(|vars| {
                                vars.into_iter()
                                    .map(|(k, w)| {
                                        variants.push(w);
                                        (k, variants.len() as u32 - 1)
                                    })
                                    .collect()
                            })
                        });
                        match entry.as_ref().and_then(|m| m.get(&var)) {
                            Some(v) => slot = *v,
                            None if bits == 0 => missing.push(format!("{file}#{var}")),
                            None => {}
                        }
                    }
                    slots.push(slot);
                }
            }
        }
        // ModelBakery.loadBlocks: o blockstate `item_frame` (normal e map) do RenderItemFrame também entra no
        // atlas de blocos.
        let mut tex = |name: &str| {
            let n = name.strip_prefix("blocks/").map(str::to_string).unwrap_or_else(|| format!("~{name}"));
            self.tex(&n)
        };
        let _ = load_blockstate("item_frame", &mut lib, load, &mut tex);
        let files = files.values().filter(|f| f.is_some()).count();
        self.models = Some(BlockModels { variants, base, slots, missing, files });
    }

    #[inline]
    fn flag(&self, st: u16, f: u8) -> bool {
        self.flags.get(st as usize).is_some_and(|v| v & f != 0)
    }

    /// `isOpaqueCube`: esconde as faces encostadas.
    #[inline]
    pub fn opaque_cube(&self, st: u16) -> bool {
        self.flag(st, F_OPAQUE)
    }

    /// `Block.getMaterial().isSolid()` (gerado do código original).
    #[inline]
    pub fn material_solid(&self, st: u16) -> bool {
        gen::FLAGS[(st >> 4) as usize & 255] & gen::F_MAT_SOLID != 0
    }

    /// `Block.getMaterial().blocksMovement()`.
    #[inline]
    pub fn material_blocks_movement(&self, st: u16) -> bool {
        gen::FLAGS[(st >> 4) as usize & 255] & gen::F_SOLID != 0
    }

    /// `isFullCube`.
    #[inline]
    pub fn full_cube(&self, st: u16) -> bool {
        self.flag(st, F_FULL)
    }

    /// `isBlockNormalCube`: escurece os cantos na oclusão de ambiente.
    #[inline]
    pub fn normal_cube(&self, st: u16) -> bool {
        self.flag(st, F_NORMAL)
    }

    /// `isFullBlock` (opaco na criação do bloco; usado pelos painéis).
    #[inline]
    pub fn full_block(&self, st: u16) -> bool {
        self.flag(st, F_FULL_BLOCK)
    }

    /// O meta existe no jogo (os outros viram ar).
    #[inline]
    pub fn valid(&self, st: u16) -> bool {
        gen::VALID[(st >> 4) as usize & 255] & (1 << (st & 15)) != 0
    }

    fn shape_entry(&self, st: u16, nb: &dyn Fn(i32, i32, i32) -> u16) -> Option<(u16, u16)> {
        let base = *gen::BASE.get(st as usize)?;
        if base == u32::MAX {
            return None;
        }
        let bits = states::shape_bits(self, st, nb);
        gen::ENTRIES.get((base + bits) as usize).copied()
    }

    /// Caixas de colisão (`addCollisionBoxesToList`), em blocos relativas ao canto.
    pub fn collision(&self, st: u16, nb: &dyn Fn(i32, i32, i32) -> u16, out: &mut Vec<[f64; 6]>) {
        if let Some((shape, _)) = self.shape_entry(st, nb) {
            let (start, len) = gen::SHAPES[shape as usize];
            for &b in &gen::SHAPE_BOXES[start as usize..start as usize + len as usize] {
                out.push(gen::BOXES[b as usize]);
            }
        }
    }

    /// Caixa de seleção (`getSelectedBoundingBox`) se o raio pode parar no bloco (`canCollideCheck`).
    pub fn selection(&self, st: u16, nb: &dyn Fn(i32, i32, i32) -> u16) -> Option<[f64; 6]> {
        let g = gen::FLAGS[(st >> 4) as usize & 255];
        if g & gen::F_COLLIDABLE == 0 || g & gen::F_LIQUID != 0 {
            return None;
        }
        let (_, sel) = self.shape_entry(st, nb)?;
        (sel != u16::MAX).then(|| gen::BOXES[sel as usize])
    }

    /// `isTranslucent` (material que não bloqueia luz).
    #[inline]
    pub fn translucent(&self, st: u16) -> bool {
        self.flag(st, F_TRANSLUCENT)
    }

    /// `getUseNeighborBrightness`.
    #[inline]
    pub fn neighbor_light(&self, st: u16) -> bool {
        self.flag(st, F_NEIGHBOR_LIGHT)
    }

    /// Luz emitida pelo bloco (0..15).
    #[inline]
    pub fn emission(&self, st: u16) -> u8 {
        self.emission[(st >> 4) as usize & 255]
    }

    /// Cor de `colorMultiplier` para quads com `tintindex`.
    pub fn tint_color(&self, st: u16) -> u32 {
        let (id, m) = (st >> 4, st & 15);
        match id {
            2 | 31 | 83 | 175 => GRASS,
            18 => [FOLIAGE, SPRUCE, BIRCH, FOLIAGE][(m & 3) as usize],
            161 | 106 => FOLIAGE,
            111 => LILY,
            104 | 105 => {
                let age = (m & 7) as u32;
                (age * 32) << 16 | (255 - age * 8) << 8 | age * 4
            }
            55 => {
                let f = m as f32 / 15.0;
                let r = if m == 0 { 0.3 } else { f * 0.6 + 0.4 };
                let g = (f * f * 0.7 - 0.5).max(0.0);
                let b = (f * f * 0.6 - 0.7).max(0.0);
                let c = |v: f32| ((v * 255.0) as u32).min(255);
                c(r) << 16 | c(g) << 8 | c(b)
            }
            _ => 0xFFFFFF,
        }
    }

    #[inline]
    pub fn get(&self, state: u16) -> &BlockInfo {
        self.infos.get(state as usize).unwrap_or(&self.unknown)
    }

    /// Caixas desenhadas pelo caminho sem modelo do jar (blocos de entidade e testes).
    /// A colisão de verdade vem de `collision`/`selection`, geradas do jogo.
    /// `nb(dx, dy, dz)` devolve o estado do vizinho.
    pub fn boxes(&self, state: u16, nb: &dyn Fn(i32, i32, i32) -> u16, out: &mut Vec<Aabb>) {
        let info = self.get(state);
        match &info.model {
            Model::Boxes(b) => out.extend_from_slice(b),
            Model::Connected(kind) => {
                let conn = [(0, -1), (0, 1), (-1, 0), (1, 0)].map(|(dx, dz)| self.connects(*kind, nb(dx, 0, dz)));
                connected_boxes(*kind, conn, out);
            }
            Model::Door { .. } => {
                let meta = state & 15;
                let (lower, upper) = if meta & 8 != 0 { (nb(0, -1, 0) & 15, meta) } else { (meta, nb(0, 1, 0) & 15) };
                out.push(door_box(lower, upper));
            }
            _ => {}
        }
    }

    fn connects(&self, kind: Connect, n: u16) -> bool {
        let id = n >> 4;
        let ni = self.get(n);
        if ni.full_opaque {
            return true;
        }
        match (&ni.model, kind) {
            (Model::Connected(k), _) if *k == kind => true,
            (_, Connect::Pane) => id == 20 || id == 95,
            _ => id == 107 || (183..=187).contains(&id),
        }
    }

    fn tex(&mut self, name: &str) -> u16 {
        if let Some(&i) = self.names.get(name) {
            return i;
        }
        let i = self.textures.len() as u16;
        self.textures.push(name.to_string());
        self.names.insert(name.to_string(), i);
        i
    }

    fn info(&mut self, model: Model, faces: [&str; 6], layer: Layer) -> BlockInfo {
        let tex = faces.map(|n| self.tex(n));
        BlockInfo { model, tex, layer, ..BlockInfo::air() }
    }

    /// `BlockLeaves.setGraphicsLevel`, chamado por `RenderGlobal.loadRenderers`: nos gráficos rápidos a folha
    /// é cubo opaco (`isOpaqueCube`, esconde os vizinhos e fecha a visibilidade) na camada SOLID; nos
    /// bonitos, recorte (CUTOUT_MIPPED) que não esconde nada. A tabela nasce nos bonitos.
    pub fn set_fancy_leaves(&mut self, fancy: bool) {
        for id in [18usize, 161] {
            for info in &mut self.infos[id << 4..(id + 1) << 4] {
                info.layer = if fancy { Layer::CutoutMipped } else { Layer::Opaque };
                info.full_opaque = !fancy;
            }
        }
    }

    fn set(&mut self, id: u16, meta: usize, info: BlockInfo) {
        self.infos[((id as usize) << 4) | (meta & 15)] = info;
    }

    fn set_all(&mut self, id: u16, info: BlockInfo) {
        for m in 0..16 {
            self.set(id, m, info.clone());
        }
    }

    fn cube(&mut self, name: &str) -> BlockInfo {
        self.info(Model::Boxes(vec![Aabb::FULL]), [name; 6], Layer::Opaque)
    }

    /// Topo, base e lados.
    fn tbs(&mut self, top: &str, bottom: &str, side: &str) -> BlockInfo {
        self.info(Model::Boxes(vec![Aabb::FULL]), [bottom, top, side, side, side, side], Layer::Opaque)
    }

    fn shape(&mut self, boxes: Vec<Aabb>, faces: [&str; 6], layer: Layer) -> BlockInfo {
        self.info(Model::Boxes(boxes), faces, layer)
    }

    fn cross(&mut self, name: &str) -> BlockInfo {
        self.info(Model::Cross, [name; 6], Layer::Cutout)
    }

    /// Bloco com frente virada para `front` (índice de face).
    fn facing(&mut self, top: &str, bottom: &str, side: &str, front: &str, face: usize) -> BlockInfo {
        let mut f = [bottom, top, side, side, side, side];
        f[face] = front;
        self.info(Model::Boxes(vec![Aabb::FULL]), f, Layer::Opaque)
    }

    /// Tronco/feno: `axis` 0 = Y, 1 = X, 2 = Z, 3 = só casca.
    fn pillar(&mut self, top: &str, side: &str, axis: usize) -> BlockInfo {
        let f = match axis {
            0 => [top, top, side, side, side, side],
            1 => [side, side, side, side, top, top],
            2 => [side, side, top, top, side, side],
            _ => [side; 6],
        };
        self.info(Model::Boxes(vec![Aabb::FULL]), f, Layer::Opaque)
    }

    fn slab(&mut self, id: u16, double_id: u16, variants: &[[&str; 3]]) {
        for m in 0..16 {
            let Some([top, bottom, side]) = variants.get(m & 7) else { continue };
            let (y0, y1) = if m & 8 != 0 { (8, 16) } else { (0, 8) };
            let i = self.shape(vec![Aabb::new(0, y0, 0, 16, y1, 16)], [bottom, top, side, side, side, side], Layer::Opaque);
            self.set(id, m, i);
            let d = self.tbs(top, bottom, side);
            self.set(double_id, m, d);
        }
    }

    fn stairs(&mut self, id: u16, top: &str, bottom: &str, side: &str) {
        for m in 0..16 {
            let upside = m & 4 != 0;
            let (by0, by1, sy0, sy1) = if upside { (8, 16, 0, 8) } else { (0, 8, 8, 16) };
            let step = match m & 3 {
                0 => Aabb::new(8, sy0, 0, 16, sy1, 16),
                1 => Aabb::new(0, sy0, 0, 8, sy1, 16),
                2 => Aabb::new(0, sy0, 8, 16, sy1, 16),
                _ => Aabb::new(0, sy0, 0, 16, sy1, 8),
            };
            let i = self.shape(
                vec![Aabb::new(0, by0, 0, 16, by1, 16), step],
                [bottom, top, side, side, side, side],
                Layer::Opaque,
            );
            self.set(id, m, i);
        }
    }

    fn register_all(&mut self) {
        use Layer::*;

        // Cubos simples.
        for (id, n) in [
            (4, "cobblestone"), (7, "bedrock"), (13, "gravel"), (14, "gold_ore"), (15, "iron_ore"),
            (16, "coal_ore"), (21, "lapis_ore"), (22, "lapis_block"), (25, "noteblock"), (41, "gold_block"),
            (42, "iron_block"), (45, "brick"), (48, "cobblestone_mossy"), (49, "obsidian"), (56, "diamond_ore"),
            (57, "diamond_block"), (73, "redstone_ore"), (74, "redstone_ore"), (80, "snow"), (82, "clay"),
            (87, "netherrack"), (88, "soul_sand"), (89, "glowstone"), (112, "nether_brick"), (121, "end_stone"),
            (123, "redstone_lamp_off"), (124, "redstone_lamp_on"), (129, "emerald_ore"), (133, "emerald_block"),
            (137, "command_block"), (152, "redstone_block"), (153, "quartz_ore"), (169, "sea_lantern"),
            (172, "hardened_clay"), (173, "coal_block"), (174, "ice_packed"),
        ] {
            let i = self.cube(n);
            self.set_all(id, i);
        }

        let stones = [
            "stone", "stone_granite", "stone_granite_smooth", "stone_diorite", "stone_diorite_smooth",
            "stone_andesite", "stone_andesite_smooth",
        ];
        for (m, n) in stones.iter().enumerate() {
            let i = self.cube(n);
            self.set(1, m, i);
        }

        // BlockGrass.getBlockLayer: CUTOUT_MIPPED (a camada grass_side_overlay tem pixels transparentes).
        let i = self.tbs("grass_top", "dirt", "grass_side").tint_top(GRASS).layer(CutoutMipped);
        self.set_all(2, i);
        let i = self.cube("dirt");
        self.set_all(3, i);
        let i = self.cube("coarse_dirt");
        self.set(3, 1, i);
        let i = self.tbs("dirt_podzol_top", "dirt", "dirt_podzol_side");
        self.set(3, 2, i);
        let i = self.tbs("mycelium_top", "dirt", "mycelium_side");
        self.set_all(110, i);

        for (m, w) in WOODS.iter().enumerate() {
            let i = self.cube(&format!("planks_{w}"));
            self.set(5, m, i);
        }

        let saplings = ["sapling_oak", "sapling_spruce", "sapling_birch", "sapling_jungle", "sapling_acacia", "sapling_roofed_oak"];
        for m in 0..16 {
            let i = self.cross(saplings[(m & 7).min(5)]);
            self.set(6, m, i);
        }

        let i = self.info(Model::Liquid { lava: false }, ["water_still"; 6], Translucent);
        self.set_all(8, i.clone());
        self.set_all(9, i);
        let i = self.info(Model::Liquid { lava: true }, ["lava_still"; 6], Opaque);
        self.set_all(10, i.clone());
        self.set_all(11, i);

        let i = self.cube("sand");
        self.set_all(12, i);
        let i = self.cube("red_sand");
        self.set(12, 1, i);

        for m in 0..16 {
            let w = WOODS[m & 3];
            let i = self.pillar(&format!("log_{w}_top"), &format!("log_{w}"), m >> 2);
            self.set(17, m, i);
            let w = WOODS[4 + (m & 1)];
            let i = self.pillar(&format!("log_{w}_top"), &format!("log_{w}"), m >> 2);
            self.set(162, m, i);

            let (w, tint) = [("oak", FOLIAGE), ("spruce", SPRUCE), ("birch", BIRCH), ("jungle", FOLIAGE)][m & 3];
            let i = self.cube(&format!("leaves_{w}")).layer(CutoutMipped).tint(tint);
            self.set(18, m, i);
            let w = ["acacia", "big_oak"][m & 1];
            let i = self.cube(&format!("leaves_{w}")).layer(CutoutMipped).tint(FOLIAGE);
            self.set(161, m, i);

            let i = self.pillar("hay_block_top", "hay_block_side", (m >> 2) & 3);
            self.set(170, m, i);
        }

        let i = self.cube("sponge");
        self.set_all(19, i);
        let i = self.cube("sponge_wet");
        self.set(19, 1, i);
        let i = self.cube("glass").layer(Cutout).cull_same();
        self.set_all(20, i);
        let i = self.cube("glass").layer(Cutout);
        self.set_all(138, i);

        // Dispensador, liberador, fornalhas: meta 2..5 = norte, sul, oeste, leste.
        for m in 0..16 {
            let face = if (2..=5).contains(&m) { m } else { NORTH };
            let i = self.facing("furnace_top", "furnace_top", "furnace_side", "furnace_front_off", face);
            self.set(61, m, i);
            let i = self.facing("furnace_top", "furnace_top", "furnace_side", "furnace_front_on", face);
            self.set(62, m, i);
            let (front, face2) = match m & 7 {
                0 => ("dispenser_front_vertical", DOWN),
                1 => ("dispenser_front_vertical", UP),
                f @ 2..=5 => ("dispenser_front_horizontal", f),
                _ => ("dispenser_front_horizontal", NORTH),
            };
            let i = self.facing("furnace_top", "furnace_top", "furnace_side", front, face2);
            self.set(23, m, i);
            let front = front.replace("dispenser", "dropper");
            let i = self.facing("furnace_top", "furnace_top", "furnace_side", &front, face2);
            self.set(158, m, i);

            // Abóbora: meta & 3 = sul, oeste, norte, leste.
            let face = [SOUTH, WEST, NORTH, EAST][m & 3];
            let i = self.facing("pumpkin_top", "pumpkin_top", "pumpkin_side", "pumpkin_face_off", face);
            self.set(86, m, i);
            let i = self.facing("pumpkin_top", "pumpkin_top", "pumpkin_side", "pumpkin_face_on", face);
            self.set(91, m, i);

            // Pistões: meta & 7 = baixo, cima, norte, sul, oeste, leste.
            let face = [DOWN, UP, NORTH, SOUTH, WEST, EAST, UP, UP][m & 7];
            let back = [UP, DOWN, SOUTH, NORTH, EAST, WEST, DOWN, DOWN][m & 7];
            for (id, top) in [(33, "piston_top_normal"), (29, "piston_top_sticky")] {
                let mut i = self.facing("piston_side", "piston_side", "piston_side", top, face);
                i.tex[back] = self.tex("piston_bottom");
                self.set(id, m, i);
            }
        }

        for (id, prefix) in [(24, "sandstone"), (179, "red_sandstone")] {
            let i = self.tbs(&format!("{prefix}_top"), &format!("{prefix}_bottom"), &format!("{prefix}_normal"));
            self.set_all(id, i);
            let i = self.tbs(&format!("{prefix}_top"), &format!("{prefix}_top"), &format!("{prefix}_carved"));
            self.set(id, 1, i);
            let i = self.tbs(&format!("{prefix}_top"), &format!("{prefix}_top"), &format!("{prefix}_smooth"));
            self.set(id, 2, i);
        }

        for m in 0..16 {
            let head = m & 8 != 0;
            let (top, side) = if head { ("bed_head_top", "bed_head_side") } else { ("bed_feet_top", "bed_feet_side") };
            let i = self.shape(vec![Aabb::new(0, 0, 0, 16, 9, 16)], ["planks_oak", top, side, side, side, side], Cutout); // BlockBed.getBlockLayer
            self.set(26, m, i);
        }

        // Trilhos: placa fina sem colisão.
        let rail = Aabb::new(0, 0, 0, 16, 1, 16);
        for m in 0..16 {
            let powered = m & 8 != 0;
            for (id, name) in [
                (27, if powered { "rail_golden_powered" } else { "rail_golden" }),
                (28, if powered { "rail_detector_powered" } else { "rail_detector" }),
                (157, if powered { "rail_activator_powered" } else { "rail_activator" }),
                (66, if m >= 6 { "rail_normal_turned" } else { "rail_normal" }),
            ] {
                let i = self.shape(vec![rail], [name; 6], Cutout);
                self.set(id, m, i);
            }
        }

        let i = self.cross("web");
        self.set_all(30, i);
        let i = self.cross("deadbush");
        self.set_all(31, i.clone());
        self.set_all(32, i);
        let i = self.cross("tallgrass").tint(GRASS);
        self.set(31, 1, i);
        let i = self.cross("fern").tint(GRASS);
        self.set(31, 2, i);

        for (m, c) in COLORS.iter().enumerate() {
            let i = self.cube(&format!("wool_colored_{c}"));
            self.set(35, m, i);
            let i = self.cube(&format!("hardened_clay_stained_{c}"));
            self.set(159, m, i);
            let i = self.cube(&format!("glass_{c}")).layer(Translucent).cull_same();
            self.set(95, m, i);
            let n = format!("wool_colored_{c}");
            let i = self.shape(vec![Aabb::new(0, 0, 0, 16, 1, 16)], [&*n; 6], Opaque);
            self.set(171, m, i);
            let n = format!("glass_{c}");
            let i = self.info(Model::Connected(Connect::Pane), [&*n; 6], Translucent);
            self.set(160, m, i);
        }

        let i = self.cross("flower_dandelion");
        self.set_all(37, i);
        let flowers = [
            "flower_rose", "flower_blue_orchid", "flower_allium", "flower_houstonia", "flower_tulip_red",
            "flower_tulip_orange", "flower_tulip_white", "flower_tulip_pink", "flower_oxeye_daisy",
        ];
        for m in 0..16 {
            let i = self.cross(flowers[m.min(8)]);
            self.set(38, m, i);
        }
        let i = self.cross("mushroom_brown");
        self.set_all(39, i);
        let i = self.cross("mushroom_red");
        self.set_all(40, i);

        self.slab(
            44,
            43,
            &[
                ["stone_slab_top", "stone_slab_top", "stone_slab_side"],
                ["sandstone_top", "sandstone_bottom", "sandstone_normal"],
                ["planks_oak", "planks_oak", "planks_oak"],
                ["cobblestone", "cobblestone", "cobblestone"],
                ["brick", "brick", "brick"],
                ["stonebrick", "stonebrick", "stonebrick"],
                ["nether_brick", "nether_brick", "nether_brick"],
                ["quartz_block_top", "quartz_block_bottom", "quartz_block_side"],
            ],
        );
        let planks: Vec<String> = WOODS.iter().map(|w| format!("planks_{w}")).collect();
        let wood_slabs: Vec<[&str; 3]> = planks.iter().map(|p| [p.as_str(); 3]).collect();
        self.slab(126, 125, &wood_slabs);
        self.slab(182, 181, &[["red_sandstone_top", "red_sandstone_bottom", "red_sandstone_normal"]]);

        let i = self.tbs("tnt_top", "tnt_bottom", "tnt_side");
        self.set_all(46, i);
        let i = self.tbs("planks_oak", "planks_oak", "bookshelf");
        self.set_all(47, i);
        let i = self.cube("mob_spawner").layer(Cutout);
        self.set_all(52, i);

        let torch = vec![Aabb::new(7, 0, 7, 9, 10, 9)];
        for (id, n) in [(50, "torch_on"), (75, "redstone_torch_off"), (76, "redstone_torch_on")] {
            let i = self.shape(torch.clone(), [n; 6], Cutout);
            self.set_all(id, i);
        }

        self.stairs(53, "planks_oak", "planks_oak", "planks_oak");
        self.stairs(67, "cobblestone", "cobblestone", "cobblestone");
        self.stairs(108, "brick", "brick", "brick");
        self.stairs(109, "stonebrick", "stonebrick", "stonebrick");
        self.stairs(114, "nether_brick", "nether_brick", "nether_brick");
        self.stairs(128, "sandstone_top", "sandstone_bottom", "sandstone_normal");
        self.stairs(134, "planks_spruce", "planks_spruce", "planks_spruce");
        self.stairs(135, "planks_birch", "planks_birch", "planks_birch");
        self.stairs(136, "planks_jungle", "planks_jungle", "planks_jungle");
        self.stairs(156, "quartz_block_top", "quartz_block_bottom", "quartz_block_side");
        self.stairs(163, "planks_acacia", "planks_acacia", "planks_acacia");
        self.stairs(164, "planks_big_oak", "planks_big_oak", "planks_big_oak");
        self.stairs(180, "red_sandstone_top", "red_sandstone_bottom", "red_sandstone_normal");

        // Baús, placas, cabeças e estandartes não têm modelo no jogo: o desenho vem dos
        // renderers de tile entity (mc-client/src/tile_entities.rs).

        for m in 0..16 {
            // Botões: meta & 7 = 0 teto, 1 leste, 2 oeste, 3 sul, 4 norte, 5 chão.
            let b = match m & 7 {
                0 => Aabb::new(5, 14, 6, 11, 16, 10),
                1 => Aabb::new(0, 6, 5, 2, 10, 11),
                2 => Aabb::new(14, 6, 5, 16, 10, 11),
                3 => Aabb::new(5, 6, 0, 11, 10, 2),
                4 => Aabb::new(5, 6, 14, 11, 10, 16),
                _ => Aabb::new(5, 0, 6, 11, 2, 10),
            };
            for (id, n) in [(77, "stone"), (143, "planks_oak")] {
                let i = self.shape(vec![b], [n; 6], Opaque);
                self.set(id, m, i);
            }
            let lever = match m & 7 {
                1 => vec![Aabb::new(0, 4, 5, 3, 12, 11)],
                2 => vec![Aabb::new(13, 4, 5, 16, 12, 11)],
                3 => vec![Aabb::new(5, 4, 0, 11, 12, 3)],
                4 => vec![Aabb::new(5, 4, 13, 11, 12, 16)],
                0 | 7 => vec![Aabb::new(4, 13, 5, 12, 16, 11)],
                _ => vec![Aabb::new(4, 0, 5, 12, 3, 11), Aabb::new(7, 3, 7, 9, 10, 9)],
            };
            let i = self.shape(lever, ["cobblestone"; 6], Opaque);
            self.set(69, m, i);

            // Trepadeira: bits 1 sul, 2 oeste, 4 norte, 8 leste; 0 = só no teto.
            let mut v = Vec::new();
            if m & 1 != 0 { v.push(Aabb::new(0, 0, 15, 16, 16, 16)); }
            if m & 2 != 0 { v.push(Aabb::new(0, 0, 0, 1, 16, 16)); }
            if m & 4 != 0 { v.push(Aabb::new(0, 0, 0, 16, 16, 1)); }
            if m & 8 != 0 { v.push(Aabb::new(15, 0, 0, 16, 16, 16)); }
            if v.is_empty() { v.push(Aabb::new(0, 15, 0, 16, 16, 16)); }
            let i = self.shape(v, ["vine"; 6], Cutout).tint(FOLIAGE);
            self.set(106, m, i);

            let b = if m & 3 == 2 { Aabb::new(6, 0, 0, 10, 16, 16) } else { Aabb::new(0, 0, 6, 16, 16, 10) };
            let i = self.shape(vec![b], ["portal"; 6], Translucent);
            self.set(90, m, i);

            let anvil = if m & 1 == 0 {
                vec![Aabb::new(2, 0, 2, 14, 4, 14), Aabb::new(4, 4, 3, 12, 5, 13), Aabb::new(6, 5, 4, 10, 10, 12), Aabb::new(3, 10, 0, 13, 16, 16)]
            } else {
                vec![Aabb::new(2, 0, 2, 14, 4, 14), Aabb::new(3, 4, 4, 13, 5, 12), Aabb::new(4, 5, 6, 12, 10, 10), Aabb::new(0, 10, 3, 16, 16, 13)]
            };
            let i = self.shape(anvil, ["anvil_base", "anvil_top_damaged_0", "anvil_base", "anvil_base", "anvil_base", "anvil_base"], Opaque);
            self.set(145, m, i);
        }

        let i = self.cross("fire_layer_0");
        self.set_all(51, i);
        let i = self.shape(
            vec![
                Aabb::new(0, 0, 0, 16, 16, 2), Aabb::new(0, 0, 14, 16, 16, 16), Aabb::new(0, 0, 2, 2, 16, 14),
                Aabb::new(14, 0, 2, 16, 16, 14), Aabb::new(2, 3, 2, 14, 4, 14),
            ],
            ["cauldron_bottom", "cauldron_top", "cauldron_side", "cauldron_side", "cauldron_side", "cauldron_side"],
            Opaque,
        );
        self.set_all(118, i);
        let i = self.shape(
            vec![Aabb::new(0, 10, 0, 16, 16, 16), Aabb::new(4, 4, 4, 12, 10, 12), Aabb::new(6, 0, 6, 10, 4, 10)],
            ["hopper_outside", "hopper_top", "hopper_outside", "hopper_outside", "hopper_outside", "hopper_outside"],
            CutoutMipped, // BlockHopper
        );
        self.set_all(154, i);
        let i = self.shape(vec![Aabb::new(1, 0, 1, 15, 2, 15), Aabb::new(7, 2, 7, 9, 14, 9)], ["brewing_stand_base"; 6], Cutout);
        self.set_all(117, i);
        let i = self.shape(vec![Aabb::new(1, 0, 1, 15, 16, 15)], ["dragon_egg"; 6], Opaque);
        self.set_all(122, i);
        let flat = Aabb::new(0, 0, 0, 16, 1, 16);
        let i = self.shape(vec![flat], ["redstone_dust_cross"; 6], Cutout).tint(0xC01010);
        self.set_all(55, i);
        for (id, n) in [(93, "repeater_off"), (94, "repeater_on"), (149, "comparator_off"), (150, "comparator_on")] {
            let i = self.shape(vec![Aabb::new(0, 0, 0, 16, 2, 16)], [n, n, "stone_slab_top", "stone_slab_top", "stone_slab_top", "stone_slab_top"], Cutout); // BlockRedstoneDiode
            self.set_all(id, i);
        }
        let i = self.cross("pumpkin_stem_disconnected").tint(0x80C040);
        self.set_all(104, i);
        let i = self.cross("melon_stem_disconnected").tint(0x80C040);
        self.set_all(105, i);

        let i = self.info(
            Model::Boxes(vec![Aabb::FULL]),
            ["planks_oak", "crafting_table_top", "crafting_table_front", "crafting_table_side", "crafting_table_front", "crafting_table_side"],
            Opaque,
        );
        self.set_all(58, i);

        for m in 0..16 {
            let i = self.cross(&format!("wheat_stage_{}", m.min(7)));
            self.set(59, m, i);
            let stage = [0, 0, 1, 1, 2, 2, 2, 3][m & 7];
            let i = self.cross(&format!("carrots_stage_{stage}"));
            self.set(141, m, i);
            let i = self.cross(&format!("potatoes_stage_{stage}"));
            self.set(142, m, i);
            let i = self.cross(&format!("nether_wart_stage_{}", [0, 1, 1, 2][m & 3]));
            self.set(115, m, i);

            let top = if m > 0 { "farmland_wet" } else { "farmland_dry" };
            let i = self.shape(vec![Aabb::new(0, 0, 0, 16, 15, 16)], ["dirt", top, "dirt", "dirt", "dirt", "dirt"], Opaque);
            self.set(60, m, i);

            let h = ((m & 7) as u8 + 1) * 2;
            let i = self.shape(vec![Aabb::new(0, 0, 0, 16, h, 16)], ["snow"; 6], Opaque);
            self.set(78, m, i);
        }

        for (id, w) in [(64, "wood"), (71, "iron"), (193, "spruce"), (194, "birch"), (195, "jungle"), (196, "acacia"), (197, "dark_oak")] {
            let lower = format!("door_{w}_lower");
            let upper = self.tex(&format!("door_{w}_upper"));
            let i = self.info(Model::Door { upper }, [&*lower; 6], Cutout);
            self.set_all(id, i);
        }

        // Escada de mão: meta 2..5 = virada para norte, sul, oeste, leste.
        for m in 0..16 {
            let render = match m {
                3 => Aabb::new(0, 0, 0, 16, 16, 1),
                4 => Aabb::new(15, 0, 0, 16, 16, 16),
                5 => Aabb::new(0, 0, 0, 1, 16, 16),
                _ => Aabb::new(0, 0, 15, 16, 16, 16),
            };
            let i = self.shape(vec![render], ["ladder"; 6], Cutout);
            self.set(65, m, i);
        }

        let plate = vec![Aabb::new(1, 0, 1, 15, 1, 15)];
        for (id, n) in [(70, "stone"), (72, "planks_oak"), (147, "gold_block"), (148, "iron_block")] {
            let i = self.shape(plate.clone(), [n; 6], Opaque);
            self.set_all(id, i);
        }

        let i = self.cube("ice").layer(Translucent).cull_same();
        self.set_all(79, i);
        let i = self.shape(
            vec![Aabb::new(1, 0, 1, 15, 16, 15)],
            ["cactus_bottom", "cactus_top", "cactus_side", "cactus_side", "cactus_side", "cactus_side"],
            Cutout,
        );
        self.set_all(81, i);
        let i = self.cross("reeds").tint(GRASS);
        self.set_all(83, i);
        let i = self.tbs("jukebox_top", "jukebox_side", "jukebox_side");
        self.set_all(84, i);

        for (id, n) in [
            (85, "planks_oak"), (188, "planks_spruce"), (189, "planks_birch"), (190, "planks_jungle"),
            (191, "planks_big_oak"), (192, "planks_acacia"), (113, "nether_brick"),
        ] {
            let i = self.info(Model::Connected(Connect::Fence), [n; 6], Opaque);
            self.set_all(id, i);
        }
        let i = self.info(Model::Connected(Connect::Pane), ["glass"; 6], CutoutMipped); // BlockPane
        self.set_all(102, i);
        let i = self.info(Model::Connected(Connect::Pane), ["iron_bars"; 6], CutoutMipped);
        self.set_all(101, i);
        let i = self.info(Model::Connected(Connect::Wall), ["cobblestone"; 6], Opaque);
        self.set_all(139, i);
        let i = self.info(Model::Connected(Connect::Wall), ["cobblestone_mossy"; 6], Opaque);
        self.set(139, 1, i);

        // Portões: meta & 3 = sul, oeste, norte, leste; bit 2 = aberto.
        for (id, n) in [
            (107, "planks_oak"), (183, "planks_spruce"), (184, "planks_birch"), (185, "planks_jungle"),
            (186, "planks_big_oak"), (187, "planks_acacia"),
        ] {
            for m in 0..16 {
                let along_x = m & 1 == 0;
                let open = m & 4 != 0;
                let rot = |b: Aabb| {
                    if along_x {
                        b
                    } else {
                        Aabb::new(b.min[2], b.min[1], b.min[0], b.max[2], b.max[1], b.max[0])
                    }
                };
                let mut boxes = vec![rot(Aabb::new(0, 5, 7, 2, 16, 9)), rot(Aabb::new(14, 5, 7, 16, 16, 9))];
                if !open {
                    boxes.push(rot(Aabb::new(2, 6, 7, 14, 9, 9)));
                    boxes.push(rot(Aabb::new(2, 12, 7, 14, 15, 9)));
                }
                let i = self.shape(boxes, [n; 6], Opaque);
                self.set(id, m, i);
            }
        }

        let i = self.tbs("melon_top", "melon_top", "melon_side");
        self.set_all(103, i);

        // Alçapões: bits 0-1 = direção, bit 2 = aberto, bit 3 = metade de cima.
        for (id, n) in [(96, "trapdoor"), (167, "iron_trapdoor")] {
            for m in 0..16 {
                let b = if m & 4 != 0 {
                    match m & 3 {
                        0 => Aabb::new(0, 0, 13, 16, 16, 16),
                        1 => Aabb::new(0, 0, 0, 16, 16, 3),
                        2 => Aabb::new(13, 0, 0, 16, 16, 16),
                        _ => Aabb::new(0, 0, 0, 3, 16, 16),
                    }
                } else if m & 8 != 0 {
                    Aabb::new(0, 13, 0, 16, 16, 16)
                } else {
                    Aabb::new(0, 0, 0, 16, 3, 16)
                };
                let i = self.shape(vec![b], [n; 6], Cutout);
                self.set(id, m, i);
            }
        }

        let eggs = ["stone", "cobblestone", "stonebrick", "stonebrick_mossy", "stonebrick_cracked", "stonebrick_carved"];
        for (m, n) in eggs.iter().enumerate() {
            let i = self.cube(n);
            self.set(97, m, i);
        }
        let bricks = ["stonebrick", "stonebrick_mossy", "stonebrick_cracked", "stonebrick_carved"];
        for m in 0..16 {
            let i = self.cube(bricks[m & 3]);
            self.set(98, m, i);
        }

        for (id, skin) in [(99, "mushroom_block_skin_brown"), (100, "mushroom_block_skin_red")] {
            for m in 0..16 {
                let i = match m {
                    0 => self.cube("mushroom_block_inside"),
                    10 => self.tbs("mushroom_block_inside", "mushroom_block_inside", "mushroom_block_skin_stem"),
                    14 => self.cube(skin),
                    15 => self.cube("mushroom_block_skin_stem"),
                    _ => self.tbs(skin, "mushroom_block_inside", skin),
                };
                self.set(id, m, i);
            }
        }

        let i = self.shape(vec![Aabb::new(0, 0, 0, 16, 1, 16)], ["waterlily"; 6], Cutout).tint(LILY);
        self.set_all(111, i);

        let i = self.shape(
            vec![Aabb::new(0, 0, 0, 16, 12, 16)],
            ["enchanting_table_bottom", "enchanting_table_top", "enchanting_table_side", "enchanting_table_side", "enchanting_table_side", "enchanting_table_side"],
            Opaque,
        );
        self.set_all(116, i);
        let i = self.shape(
            vec![Aabb::new(0, 0, 0, 16, 13, 16)],
            ["end_stone", "endframe_top", "endframe_side", "endframe_side", "endframe_side", "endframe_side"],
            Opaque,
        );
        self.set_all(120, i);

        let i = self.shape(vec![Aabb::new(5, 0, 5, 11, 6, 11)], ["flower_pot"; 6], Cutout);
        self.set_all(140, i);
        let i = self.shape(
            vec![Aabb::new(1, 0, 1, 15, 8, 15)],
            ["cake_bottom", "cake_top", "cake_side", "cake_side", "cake_side", "cake_side"],
            Cutout, // BlockCake.getBlockLayer
        );
        self.set_all(92, i);

        for (id, top) in [(151, "daylight_detector_top"), (178, "daylight_detector_inverted_top")] {
            let s = "daylight_detector_side";
            let i = self.shape(vec![Aabb::new(0, 0, 0, 16, 6, 16)], [s, top, s, s, s, s], Opaque);
            self.set_all(id, i);
        }

        let i = self.tbs("quartz_block_top", "quartz_block_bottom", "quartz_block_side");
        self.set_all(155, i);
        let i = self.tbs("quartz_block_chiseled_top", "quartz_block_chiseled_top", "quartz_block_chiseled");
        self.set(155, 1, i);
        for (m, axis) in [(2, 0), (3, 1), (4, 2)] {
            let i = self.pillar("quartz_block_lines_top", "quartz_block_lines", axis);
            self.set(155, m, i);
        }

        for (m, n) in ["prismarine_rough", "prismarine_bricks", "prismarine_dark"].iter().enumerate() {
            let i = self.cube(n);
            self.set(168, m, i);
        }
        let i = self.cube("slime").layer(Translucent);
        self.set_all(165, i);
        let mut barrier = BlockInfo::air();
        barrier.model = Model::Invisible;
        self.set_all(166, barrier);

        let kinds = ["sunflower", "syringa", "grass", "fern", "rose", "paeonia"];
        for (k, n) in kinds.iter().enumerate() {
            self.double_plant[k] = [self.tex(&format!("double_plant_{n}_bottom")), self.tex(&format!("double_plant_{n}_top"))];
        }
        let i = self.info(Model::DoublePlant, ["double_plant_grass_bottom"; 6], Cutout);
        self.set_all(175, i);
        // Linha de armadilha: só modelo, na camada translúcida como no jogo.
        for m in 0..16 {
            self.infos[132 << 4 | m].layer = Translucent;
        }
    }
}

/// Lados na ordem norte, sul, oeste, leste.
fn connected_boxes(kind: Connect, conn: [bool; 4], out: &mut Vec<Aabb>) {
    match kind {
        Connect::Fence => {
            out.push(Aabb::new(6, 0, 6, 10, 16, 10));
            for (i, c) in conn.iter().enumerate() {
                if !c {
                    continue;
                }
                let (x0, z0, x1, z1) = [(7, 0, 9, 6), (7, 10, 9, 16), (0, 7, 6, 9), (10, 7, 16, 9)][i];
                out.push(Aabb::new(x0, 12, z0, x1, 15, z1));
                out.push(Aabb::new(x0, 6, z0, x1, 9, z1));
            }
        }
        Connect::Wall => {
            out.push(Aabb::new(4, 0, 4, 12, 16, 12));
            let h = 13;
            for (i, c) in conn.iter().enumerate() {
                if *c {
                    let (x0, z0, x1, z1) = [(5, 0, 11, 4), (5, 12, 11, 16), (0, 5, 4, 11), (12, 5, 16, 11)][i];
                    out.push(Aabb::new(x0, 0, z0, x1, h, z1));
                }
            }
        }
        Connect::Pane => {
            let any = conn.iter().any(|c| *c);
            out.push(Aabb::new(7, 0, 7, 9, 16, 9));
            for (i, c) in conn.iter().enumerate() {
                if *c || !any {
                    let (x0, z0, x1, z1) = [(7, 0, 9, 7), (7, 9, 9, 16), (0, 7, 7, 9), (9, 7, 16, 9)][i];
                    out.push(Aabb::new(x0, 0, z0, x1, 16, z1));
                }
            }
        }
    }
}

/// Porta 1.8: a metade de baixo guarda direção e se está aberta; a de cima, a dobradiça.
fn door_box(lower: u16, upper: u16) -> Aabb {
    const SIDES: [Aabb; 4] = [
        Aabb::new(0, 0, 0, 3, 16, 16),
        Aabb::new(0, 0, 0, 16, 16, 3),
        Aabb::new(13, 0, 0, 16, 16, 16),
        Aabb::new(0, 0, 13, 16, 16, 16),
    ];
    let facing = (lower & 3) as usize;
    let i = if lower & 4 == 0 {
        facing
    } else if upper & 1 != 0 {
        (facing + 3) % 4
    } else {
        (facing + 1) % 4
    };
    SIDES[i]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_lookups() {
        let t = BlockTable::new();
        assert!(matches!(t.get(0).model, Model::Air));
        assert!(t.get(1 << 4).full_opaque);
        assert!(!t.get(18 << 4).full_opaque, "folhas não escondem vizinhos");
        let mut rapido = BlockTable::new();
        rapido.set_fancy_leaves(false);
        assert!(rapido.get(18 << 4).full_opaque && rapido.get(161 << 4 | 1).full_opaque, "folhas rápidas são cubo opaco");
        assert_eq!(rapido.get(18 << 4).layer, Layer::Opaque);
        rapido.set_fancy_leaves(true);
        assert!(!rapido.get(18 << 4).full_opaque);
        assert_eq!(rapido.get(18 << 4).layer, Layer::CutoutMipped);
        assert_eq!(t.get(2 << 4).layer, Layer::CutoutMipped, "grama: a camada da lateral precisa do recorte");
        assert_eq!(t.get(20 << 4).layer, Layer::Cutout, "vidro: CUTOUT, sem mipmap");
        assert!(!t.get(44 << 4).full_opaque, "laje não é cubo inteiro");
        assert!(matches!(t.get(31 << 4 | 1).model, Model::Cross));
        assert!(t.textures.len() < 1024, "cabe no atlas 32x32");
    }

    #[test]
    fn fence_connects_to_stone() {
        let t = BlockTable::new();
        let mut out = Vec::new();
        // Pedra ao norte, ar nos outros lados.
        let nb = |_dx: i32, _dy: i32, dz: i32| if dz == -1 { 1 << 4 } else { 0 };
        t.boxes(85 << 4, &nb, &mut out);
        assert_eq!(out.len(), 3, "poste + 2 travessas para o norte");
        // Colisão do jogo: poste e braço de 1,5 bloco (BlockFence.addCollisionBoxesToList).
        let mut c = Vec::new();
        t.collision(85 << 4, &nb, &mut c);
        assert_eq!(c, vec![[0.375, 0.0, 0.0, 0.625, 1.5, 0.625]]);
        c.clear();
        t.collision(85 << 4, &|_, _, _| 0, &mut c);
        assert_eq!(c, vec![[0.375, 0.0, 0.375, 0.625, 1.5, 0.625]]);
    }

    #[test]
    fn variants_match_the_game_mappers() {
        // Tabela gerada pelo BlockModelShapes/getActualState do jogo (vizinhos = ar).
        let t = BlockTable::new();
        let mut wrong = Vec::new();
        let mut checked = 0;
        for line in include_str!("gen_variants.txt").lines().filter(|l| !l.starts_with('#')) {
            let mut it = line.split(' ');
            let (id, m, want): (u16, u16, &str) = (it.next().unwrap().parse().unwrap(), it.next().unwrap().parse().unwrap(), it.next().unwrap());
            let st = id << 4 | m;
            let bits = if states::dyn_count(id) > 1 { states::actual_bits(&t, st, [0; 3], &|_, _, _| 0) } else { 0 };
            let got = states::location(id, m, bits).map_or("-".to_string(), |(f, v)| format!("{f}#{v}"));
            if got != want {
                wrong.push(format!("{id}:{m} {got} != {want}"));
            }
            checked += 1;
        }
        assert!(checked > 1300);
        assert!(wrong.is_empty(), "{} diferentes:\n{}", wrong.len(), wrong.join("\n"));
    }

    #[test]
    fn generated_shapes_match_the_game() {
        let t = BlockTable::new();
        let air = |_: i32, _: i32, _: i32| 0u16;
        let coll = |st: u16, nb: &dyn Fn(i32, i32, i32) -> u16| {
            let mut v = Vec::new();
            t.collision(st, nb, &mut v);
            v
        };
        assert_eq!(coll(1 << 4, &air), vec![[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]]);
        assert_eq!(coll(44 << 4, &air), vec![[0.0, 0.0, 0.0, 1.0, 0.5, 1.0]], "laje de baixo");
        assert_eq!(coll(44 << 4 | 8, &air), vec![[0.0, 0.5, 0.0, 1.0, 1.0, 1.0]], "laje de cima");
        // Escada reta para leste: base + degrau.
        assert_eq!(coll(53 << 4, &air), vec![[0.0, 0.0, 0.0, 1.0, 0.5, 1.0], [0.5, 0.5, 0.0, 1.0, 1.0, 1.0]]);
        // Plantas não colidem, mas dá para mirar com a caixa menor.
        assert!(coll(31 << 4 | 1, &air).is_empty());
        let sel = t.selection(31 << 4 | 1, &air).unwrap();
        assert!((sel[0] - 0.1).abs() < 1e-6 && (sel[4] - 0.8).abs() < 1e-6, "{sel:?}");
        // Ar e água não são miráveis; meta inválido não existe.
        assert!(t.selection(0, &air).is_none());
        assert!(t.selection(9 << 4, &air).is_none());
        assert!(!t.valid(1 << 4 | 9));
        // Alma de areia afunda 1/8; neve com 3 camadas = 2/8 de colisão.
        assert_eq!(coll(88 << 4, &air), vec![[0.0, 0.0, 0.0, 1.0, 0.875, 1.0]]);
        assert_eq!(coll(78 << 4 | 2, &air), vec![[0.0, 0.0, 0.0, 1.0, 0.25, 1.0]]);
        // Porta de baixo fechada virada para leste (meta 0) com a de cima: placa de 3/16.
        let nb = |_: i32, dy: i32, _: i32| if dy == 1 { 64 << 4 | 8 } else { 0 };
        assert_eq!(coll(64 << 4, &nb), vec![[0.0, 0.0, 0.0, 0.1875, 1.0, 1.0]]);
    }
}

#[cfg(test)]
pub(crate) mod jar_tests {
    use super::*;
    use std::io::Read;

    /// Jar do jogo instalado pelo Prism (os testes que dependem dele são pulados sem ele).
    pub fn jar_loader() -> Option<impl FnMut(&str) -> Option<Vec<u8>>> {
        let home = std::env::var("HOME").ok()?;
        let path = format!("{home}/.local/share/PrismLauncher/libraries/com/mojang/minecraft/1.8.9/minecraft-1.8.9-client.jar");
        let mut zip = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
        Some(move |p: &str| {
            let mut f = zip.by_name(p).ok()?;
            let mut v = Vec::new();
            f.read_to_end(&mut v).ok()?;
            Some(v)
        })
    }

    pub fn table() -> Option<BlockTable> {
        let mut load = jar_loader()?;
        Some(BlockTable::with_models(&mut load))
    }

    #[test]
    fn every_state_has_a_variant() {
        let Some(t) = table() else { return };
        let m = t.models.as_ref().unwrap();
        println!("{} arquivos, {} variantes, {} texturas", m.files, m.variants.len(), t.textures.len());
        // Funil virado para cima não existe no jogo (lá também cai no modelo de "faltando").
        let missing: Vec<_> = m.missing.iter().filter(|s| !s.starts_with("hopper#facing=up")).collect();
        assert!(missing.is_empty(), "variantes faltando: {missing:?}");
        assert!(t.textures.len() <= 1024, "{} texturas", t.textures.len());
        assert!(t.textures.iter().all(|n| !n.starts_with('~') && n != "missingno"), "{:?}", t.textures.iter().filter(|n| n.starts_with('~') || *n == "missingno").collect::<Vec<_>>());
    }
}
