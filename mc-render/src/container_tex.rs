//! Texturas das telas de container, guardadas na metade de baixo do atlas da GUI (y >= 2048).
//! Cada textura de 256x256 do jar ocupa uma vaga de 256 px; o resto são recortes pequenos.

use image::RgbaImage;

use crate::assets::Jar;

/// Início da área extra do atlas (a de cima é de `ui.rs`).
pub const EXTRA_Y: u32 = 2048;

macro_rules! slot {
    ($name:ident, $col:expr, $row:expr) => {
        pub const $name: (u32, u32) = ($col * 256, EXTRA_Y + $row * 256);
    };
}

slot!(FURNACE, 0, 0);
slot!(CRAFTING_TABLE, 1, 0);
slot!(DISPENSER, 2, 0);
slot!(HOPPER, 3, 0);
slot!(ENCHANTING_TABLE, 4, 0);
slot!(BREWING_STAND, 5, 0);
slot!(ANVIL, 6, 0);
slot!(BEACON, 7, 0);
slot!(VILLAGER, 0, 1);
slot!(HORSE, 1, 1);
slot!(TAB_ITEMS, 2, 1);
slot!(TAB_INVENTORY, 3, 1);
slot!(TAB_ITEM_SEARCH, 4, 1);
slot!(TABS, 5, 1);
slot!(BOOK, 6, 1);
slot!(ACHIEVEMENT_BG, 2, 2);
/// `font/ascii_sga.png` (128x128): fonte "standard galactic" das runas do encantamento.
pub const SGA_FONT: (u32, u32) = (7 * 256, EXTRA_Y + 256);
/// `items/empty_armor_slot_{helmet,chestplate,leggings,boots}.png` (16x16), um ao lado do outro.
pub const EMPTY_ARMOR: (u32, u32) = (0, EXTRA_Y + 512);
/// `entity/sign.png` (64x32), para a placa de `GuiEditSign`.
pub const SIGN: (u32, u32) = (256, EXTRA_Y + 512);
/// Imagens desenhadas por software a cada quadro (livro do encantamento, placa, cavalo, jogador):
/// duas faixas de 1024x256.
pub const LIVE: [(u32, u32); 2] = [(0, EXTRA_Y + 768), (1024, EXTRA_Y + 768)];
pub const LIVE_W: u32 = 1024;
pub const LIVE_H: u32 = 256;

/// Textura do jar (`gui/container/...`) e onde ela fica no atlas.
/// Caminho, posição no atlas e tamanho lógico (a largura da textura padrão).
const TEXTURES: &[(&str, (u32, u32), u32)] = &[
    ("gui/container/furnace", FURNACE, 256),
    ("gui/container/crafting_table", CRAFTING_TABLE, 256),
    ("gui/container/dispenser", DISPENSER, 256),
    ("gui/container/hopper", HOPPER, 256),
    ("gui/container/enchanting_table", ENCHANTING_TABLE, 256),
    ("gui/container/brewing_stand", BREWING_STAND, 256),
    ("gui/container/anvil", ANVIL, 256),
    ("gui/container/beacon", BEACON, 256),
    ("gui/container/villager", VILLAGER, 256),
    ("gui/container/horse", HORSE, 256),
    ("gui/container/creative_inventory/tab_items", TAB_ITEMS, 256),
    ("gui/container/creative_inventory/tab_inventory", TAB_INVENTORY, 256),
    ("gui/container/creative_inventory/tab_item_search", TAB_ITEM_SEARCH, 256),
    ("gui/container/creative_inventory/tabs", TABS, 256),
    ("gui/book", BOOK, 256),
    ("font/ascii_sga", SGA_FONT, 128),
    ("gui/achievement/achievement_background", ACHIEVEMENT_BG, 256),
];

/// Grava as texturas das telas de container no atlas (`put` copia a imagem na origem dada).
/// `put(imagem, x, y, largura lógica)`.
pub fn put_all(jar: &mut Jar, put: &mut dyn FnMut(&RgbaImage, u32, u32, u32)) {
    for (path, at, logical) in TEXTURES {
        if let Some(img) = jar.texture(path) {
            put(&img, at.0, at.1, *logical);
        }
    }
    if let Some(img) = jar.texture("entity/sign") {
        put(&img, SIGN.0, SIGN.1, 64);
    }
    for (i, part) in ["helmet", "chestplate", "leggings", "boots"].iter().enumerate() {
        if let Some(img) = jar.texture(&format!("items/empty_armor_slot_{part}")) {
            put(&img, EMPTY_ARMOR.0 + i as u32 * 16, EMPTY_ARMOR.1, 16);
        }
    }
}
