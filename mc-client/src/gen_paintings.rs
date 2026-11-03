// GERADO por scripts/gen_paintings.py a partir de EntityPainting.EnumArt do Java 1.8.9. Não editar.

/// `EntityPainting.EnumArt`: título, tamanho em pixels e origem na `paintings_kristoffer_zetterstrand.png`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Art {
    pub title: &'static str,
    pub size_x: u32,
    pub size_y: u32,
    pub offset_x: u32,
    pub offset_y: u32,
}

/// `EnumArt.field_180001_A`: tamanho máximo do título no S10.
pub const TITLE_MAX: usize = 13;

pub const ARTS: [Art; 26] = [
    Art { title: "Kebab", size_x: 16, size_y: 16, offset_x: 0, offset_y: 0 },
    Art { title: "Aztec", size_x: 16, size_y: 16, offset_x: 16, offset_y: 0 },
    Art { title: "Alban", size_x: 16, size_y: 16, offset_x: 32, offset_y: 0 },
    Art { title: "Aztec2", size_x: 16, size_y: 16, offset_x: 48, offset_y: 0 },
    Art { title: "Bomb", size_x: 16, size_y: 16, offset_x: 64, offset_y: 0 },
    Art { title: "Plant", size_x: 16, size_y: 16, offset_x: 80, offset_y: 0 },
    Art { title: "Wasteland", size_x: 16, size_y: 16, offset_x: 96, offset_y: 0 },
    Art { title: "Pool", size_x: 32, size_y: 16, offset_x: 0, offset_y: 32 },
    Art { title: "Courbet", size_x: 32, size_y: 16, offset_x: 32, offset_y: 32 },
    Art { title: "Sea", size_x: 32, size_y: 16, offset_x: 64, offset_y: 32 },
    Art { title: "Sunset", size_x: 32, size_y: 16, offset_x: 96, offset_y: 32 },
    Art { title: "Creebet", size_x: 32, size_y: 16, offset_x: 128, offset_y: 32 },
    Art { title: "Wanderer", size_x: 16, size_y: 32, offset_x: 0, offset_y: 64 },
    Art { title: "Graham", size_x: 16, size_y: 32, offset_x: 16, offset_y: 64 },
    Art { title: "Match", size_x: 32, size_y: 32, offset_x: 0, offset_y: 128 },
    Art { title: "Bust", size_x: 32, size_y: 32, offset_x: 32, offset_y: 128 },
    Art { title: "Stage", size_x: 32, size_y: 32, offset_x: 64, offset_y: 128 },
    Art { title: "Void", size_x: 32, size_y: 32, offset_x: 96, offset_y: 128 },
    Art { title: "SkullAndRoses", size_x: 32, size_y: 32, offset_x: 128, offset_y: 128 },
    Art { title: "Wither", size_x: 32, size_y: 32, offset_x: 160, offset_y: 128 },
    Art { title: "Fighters", size_x: 64, size_y: 32, offset_x: 0, offset_y: 96 },
    Art { title: "Pointer", size_x: 64, size_y: 64, offset_x: 0, offset_y: 192 },
    Art { title: "Pigscene", size_x: 64, size_y: 64, offset_x: 64, offset_y: 192 },
    Art { title: "BurningSkull", size_x: 64, size_y: 64, offset_x: 128, offset_y: 192 },
    Art { title: "Skeleton", size_x: 64, size_y: 48, offset_x: 192, offset_y: 64 },
    Art { title: "DonkeyKong", size_x: 64, size_y: 48, offset_x: 192, offset_y: 112 },
];
