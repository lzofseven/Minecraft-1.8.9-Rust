//! Interface 2D: atlas da GUI, fonte do jogo e lote de quadros por frame.
//!
//! Coordenadas em "pixels de GUI" (como o jogo: multiplicadas pela escala da GUI).

use std::collections::{HashMap, HashSet};

use bytemuck::{Pod, Zeroable};
use image::RgbaImage;

use crate::assets::Jar;
use crate::container_tex::SGA_FONT;

pub const UI_SIZE: u32 = 2048;
/// Altura do atlas: a metade de baixo (y >= 2048) guarda as telas de container (`container_tex.rs`).
pub const UI_H: u32 = 4096;
pub const WIDGETS: (u32, u32) = (0, 0);
pub const ICONS: (u32, u32) = (256, 0);
pub const CHEST: (u32, u32) = (512, 0);
pub const INVENTORY: (u32, u32) = (768, 0);
pub const BACKGROUND: (u32, u32) = (1024, 0);
/// `gui/server_selection.png` (setas da lista de servidores).
pub const SERVER_SELECTION: (u32, u32) = (1280, 0);
/// `gui/title/minecraft.png` (logotipo do menu principal).
pub const TITLE: (u32, u32) = (1536, 0);
/// `gui/resource_packs.png` (setas da tela de pacotes de recursos).
pub const RESOURCE_PACKS: (u32, u32) = (1792, 256);
/// `misc/unknown_server.png`, guardado em 64x64.
pub const UNKNOWN_SERVER: (u32, u32) = (1792, 0);
/// `gui/title/background/panorama_0.png`, guardado em 256x256.
pub const PANORAMA: (u32, u32) = (512, 256);
/// `misc/pumpkinblur.png` (256x256), tela inteira com a abóbora na cabeça.
pub const PUMPKIN_BLUR: (u32, u32) = (768, 256);
/// `misc/vignette.png` convertida: RGB preto e alfa = brilho do original (o jogo usa
/// `ZERO, ONE_MINUS_SRC_COLOR`; com a mistura normal o resultado em RGB é o mesmo para cor cinza).
pub const VIGNETTE: (u32, u32) = (1024, 256);
/// `misc/underwater.png` (16x16), ladrilhada 4x sobre a tela.
pub const UNDERWATER: (u32, u32) = (1280, 256);
/// Primeiro quadro de `blocks/portal` (16x16) e de `blocks/fire_layer_1`.
pub const PORTAL: (u32, u32) = (1536, 256);
pub const FIRE: (u32, u32) = (1552, 256);
const FONT: (u32, u32) = (0, 256);
const WHITE: (u32, u32) = (130, 258);
/// Ícones de item 32x32 a partir daqui, 64 por linha.
pub const ITEMS: (u32, u32) = (0, 512);
pub const ITEM_PX: u32 = 32;
pub const ITEM_COLS: u32 = 64;
pub const ITEM_SLOTS: u32 = 64 * 16;
/// Páginas unicode 256x256 a partir daqui, 8 por linha (32 vagas).
const PAGES: (u32, u32) = (0, 1024);

/// Ordem dos caracteres em `font/ascii.png` (16x16 células).
const ASCII: &str = "ÀÁÂÈÊËÍÓÔÕÚßãõğİıŒœŞşŴŵžȇ\0\0\0\0\0\0\0 !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~\0ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜø£Ø×ƒáíóúñÑªº¿®¬½¼¡«»░▒▓│┤╡╢╖╕╣║╗╝╜╛┐└┴┬├─┼╞╟╚╔╩╦╠═╬╧╨╤╥╙╘╒╓╫╪┘┌█▄▌▐▀αβΓπΣσμτΦΘΩδ∞∅∈∩≡±≥≤⌠⌡÷≈°∙·√ⁿ²■\0";

const COLORS: [u32; 16] = [
    0x000000, 0x0000AA, 0x00AA00, 0x00AAAA, 0xAA0000, 0xAA00AA, 0xFFAA00, 0xAAAAAA, 0x555555, 0x5555FF, 0x55FF55,
    0x55FFFF, 0xFF5555, 0xFF55FF, 0xFFFF55, 0xFFFFFF,
];

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct UiVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: [u8; 4],
}

pub struct Font {
    ascii: HashMap<char, u8>,
    widths: [u8; 256],
    /// Larguras de `font/ascii_sga.png` (fonte das runas do encantamento).
    sga_widths: [u8; 256],
    glyph_sizes: Vec<u8>,
    pages: HashMap<u8, u32>,
    /// Páginas unicode que apareceram no texto e ainda não estão no atlas.
    pub wanted: HashSet<u8>,
}

impl Font {
    /// Fonte vazia (testes de layout sem o jar).
    pub fn empty() -> Self {
        Self { ascii: HashMap::new(), widths: [0; 256], sga_widths: [0; 256], glyph_sizes: Vec::new(), pages: HashMap::new(), wanted: HashSet::new() }
    }

    /// Fonte de testes: toda célula de `ascii.png` com a mesma largura (o espaço fica com 4), sem o jar.
    pub fn fixed_width(width: u8) -> Self {
        let mut f = Self::empty();
        f.ascii = ASCII.chars().enumerate().filter(|(_, c)| *c != '\0').map(|(i, c)| (c, i as u8)).collect();
        f.widths = [width; 256];
        f.widths[32] = 4;
        f
    }

    /// Vaga no atlas para uma página recém-carregada.
    pub fn page_slot(&self) -> Option<u32> {
        let n = self.pages.len() as u32;
        (n < 32).then_some(n)
    }

    pub fn page_loaded(&mut self, page: u8, slot: u32) {
        self.pages.insert(page, slot);
        self.wanted.remove(&page);
    }

    /// Posição (x, y) no atlas onde uma página deve ser gravada.
    pub fn page_origin(slot: u32) -> (u32, u32) {
        (PAGES.0 + (slot % 8) * 256, PAGES.1 + (slot / 8) * 256)
    }
}

/// Fator máximo de ampliação do atlas da GUI: a altura `UI_H` (4096) x 2 = 8192 px, o limite de
/// textura 2D padrão do wgpu. Pacotes de GUI 64x entram reduzidos a 2x.
pub const MAX_UI_SCALE: u32 = 2;

/// Fator de escala (potência de 2) de uma textura de `w` px cuja largura lógica é `logical`.
fn scale_of(w: u32, logical: u32) -> u32 {
    (w.max(1).div_ceil(logical.max(1))).next_power_of_two().min(MAX_UI_SCALE)
}

/// Imagem RGBA do atlas da GUI e a fonte.
///
/// O atlas é uma grade lógica de `UI_SIZE` px (as constantes acima). Pacotes HD têm texturas
/// maiores que o lugar lógico: o atlas físico passa a `UI_SIZE * k` x `UI_H * k` px (k = 1 ou 2) e cada
/// textura entra ampliada para `k` px por px lógico. As coordenadas de UV são normalizadas, então
/// o resto da interface não muda.
pub fn build_ui_atlas(jar: &mut Jar) -> (Vec<u8>, Font) {
    // (textura, origem lógica, largura lógica)
    let fixed: [(&str, (u32, u32), u32); 8] = [
        ("gui/widgets", WIDGETS, 256),
        ("gui/icons", ICONS, 256),
        ("gui/container/generic_54", CHEST, 256),
        ("gui/container/inventory", INVENTORY, 256),
        ("gui/options_background", BACKGROUND, 16),
        ("gui/server_selection", SERVER_SELECTION, 256),
        ("gui/title/minecraft", TITLE, 256),
        ("gui/resource_packs", RESOURCE_PACKS, 256),
    ];
    let resized: [(&str, (u32, u32), u32); 2] = [("misc/unknown_server", UNKNOWN_SERVER, 64), ("gui/title/background/panorama_0", PANORAMA, 256)];
    let mut images: Vec<(RgbaImage, (u32, u32), u32)> = Vec::new();
    for (path, at, logical) in fixed {
        if let Some(img) = jar.texture(path) {
            images.push((img, at, logical));
        }
    }
    let ascii = jar.texture("font/ascii");
    if let Some(img) = &ascii {
        images.push((img.clone(), FONT, 128));
    }
    let k = images.iter().map(|(img, _, l)| scale_of(img.width(), *l)).max().unwrap_or(1);
    let (w, h) = (UI_SIZE * k, UI_H * k);
    let mut rgba = vec![0u8; (w as usize) * (h as usize) * 4];
    // `lx, ly`: origem lógica; a imagem já está na escala `k`.
    let mut put = |img: &RgbaImage, lx: u32, ly: u32| {
        let (ox, oy) = (lx * k, ly * k);
        for (x, y, p) in img.enumerate_pixels() {
            if x + ox < w && y + oy < h {
                let o = (((oy + y) * w + ox + x) * 4) as usize;
                rgba[o..o + 4].copy_from_slice(&p.0);
            }
        }
    };
    let to_scale = |img: &RgbaImage, logical: u32| -> RgbaImage {
        let want = logical * k;
        if img.width() == want {
            return img.clone();
        }
        let h = (img.height() as u64 * want as u64 / img.width().max(1) as u64).max(1) as u32;
        let filter = if img.width() > want { image::imageops::FilterType::Triangle } else { image::imageops::FilterType::Nearest };
        image::imageops::resize(img, want, h, filter)
    };
    for (img, at, logical) in &images {
        put(&to_scale(img, *logical), at.0, at.1);
    }
    for (path, at, size) in resized {
        if let Some(img) = jar.texture(path) {
            put(&image::imageops::resize(&img, size * k, size * k, image::imageops::FilterType::Triangle), at.0, at.1);
        }
    }
    // Texturas dos overlays do HUD (`GuiIngame`): abóbora, vinheta, água e o primeiro quadro de portal/fogo.
    let fit = |img: &RgbaImage, side: u32| image::imageops::resize(img, side * k, side * k, image::imageops::FilterType::Triangle);
    if let Some(img) = jar.texture("misc/pumpkinblur") {
        put(&fit(&img, 256), PUMPKIN_BLUR.0, PUMPKIN_BLUR.1);
    }
    if let Some(img) = jar.texture("misc/vignette") {
        let mut v = fit(&img, 256);
        for p in v.pixels_mut() {
            *p = image::Rgba([0, 0, 0, p.0[0]]);
        }
        put(&v, VIGNETTE.0, VIGNETTE.1);
    }
    if let Some(img) = jar.texture("misc/underwater") {
        put(&fit(&img, 16), UNDERWATER.0, UNDERWATER.1);
    }
    for (path, at) in [("blocks/portal", PORTAL), ("blocks/fire_layer_1", FIRE)] {
        if let Some(img) = jar.texture(path) {
            let side = img.width().min(img.height());
            put(&fit(&image::imageops::crop_imm(&img, 0, 0, side, side).to_image(), 16), at.0, at.1);
        }
    }
    crate::container_tex::put_all(jar, &mut |img: &RgbaImage, x: u32, y: u32, logical: u32| put(&to_scale(img, logical), x, y));
    // `FontRenderer.readFontTexture`: largura de cada célula 8x8 pela última coluna com pixel.
    let cell_widths = |img: &RgbaImage| {
        let mut widths = [0u8; 256];
        let cell = img.width() / 16;
        for (i, w) in widths.iter_mut().enumerate() {
            let (cx, cy) = ((i as u32 % 16) * cell, (i as u32 / 16) * cell);
            let mut last = 0;
            for x in 0..cell {
                if (0..cell).any(|y| img.get_pixel(cx + x, cy + y).0[3] > 0) {
                    last = x + 1;
                }
            }
            // FontRenderer.readFontTexture: (int)(0.5 + colunas * 8 / célula) + 1.
            *w = (0.5 + last as f64 * (8.0 / cell.max(1) as f32) as f64) as u8 + 1;
        }
        widths[32] = 4;
        widths
    };
    let widths = ascii.as_ref().map_or([0u8; 256], &cell_widths);
    let sga_widths = jar.texture("font/ascii_sga").map_or([0u8; 256], |img| cell_widths(&img));
    put(&RgbaImage::from_pixel(4 * k, 4 * k, image::Rgba([255; 4])), WHITE.0 - 1, WHITE.1 - 1);
    let ascii = ASCII.chars().enumerate().filter(|(_, c)| *c != '\0').map(|(i, c)| (c, i as u8)).collect();
    let glyph_sizes = jar.bytes("assets/minecraft/font/glyph_sizes.bin").unwrap_or_default();
    (rgba, Font { ascii, widths, sga_widths, glyph_sizes, pages: HashMap::new(), wanted: HashSet::new() })
}

/// Lê uma página unicode do jar (256x256, glifos de 16 px).
pub fn load_unicode_page(jar: &mut Jar, page: u8) -> Option<RgbaImage> {
    jar.texture(&format!("font/unicode_page_{page:02x}"))
}

/// Pedido de modelo 3D no meio da GUI: o jogo monta a geometria e o renderer a desenha entre os
/// índices `0..at` e `at..` (`GuiModels`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Gui3d {
    /// `GuiInventory.drawEntityOnScreen(x, y, scale, mx, my, entidade)`; `entity` -1 é o próprio jogador.
    Entity { at: u32, x: i32, y: i32, scale: i32, mx: f32, my: f32, entity: i32 },
    /// `GuiEnchantment`: o livro, com o canto da tela em (`l`, `t`).
    Book { at: u32, l: i32, t: i32, partial: f32 },
}

pub struct UiBatch {
    pub vertices: Vec<UiVertex>,
    pub indices: Vec<u32>,
    pub font: Font,
    /// Transformação afim (a, b, c, d, tx, ty) aplicada às posições: x' = a*x + c*y + tx,
    /// y' = b*x + d*y + ty (como o pushMatrix/rotate/scale do GlStateManager).
    pub transform: Option<[f32; 6]>,
    /// Estado do sorteio do texto ofuscado (`§k`).
    rng: u64,
    pub gui3d: Option<Gui3d>,
    /// Índices do quadro da vinheta (`renderVignette`), desenhado com a mistura ZERO, ONE_MINUS_SRC_COLOR.
    pub vignette: Option<std::ops::Range<u32>>,
}

fn uv(x: f32, y: f32) -> [f32; 2] {
    [x / UI_SIZE as f32, y / UI_H as f32]
}

fn rgba(argb: u32) -> [u8; 4] {
    let a = (argb >> 24) as u8;
    [(argb >> 16) as u8, (argb >> 8) as u8, argb as u8, if a == 0 { 255 } else { a }]
}

impl UiBatch {
    pub fn new(font: Font) -> Self {
        Self { vertices: Vec::new(), indices: Vec::new(), font, transform: None, rng: 0x9E37_79B9_7F4A_7C15, gui3d: None, vignette: None }
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.gui3d = None;
        self.vignette = None;
    }

    /// `GuiIngame.renderVignette`: o fundo vezes (1 - luminância da textura x cor); a cor é (brilho, brilho,
    /// brilho) ou (0, f, f) perto da borda do mundo.
    pub fn vignette(&mut self, w: f32, h: f32, rgb: [f32; 3]) {
        let c = rgb.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
        let start = self.indices.len() as u32;
        self.quad([0.0, 0.0, w, h], [VIGNETTE.0 as f32, VIGNETTE.1 as f32, VIGNETTE.0 as f32 + 256.0, VIGNETTE.1 as f32 + 256.0], [[c[0], c[1], c[2], 255]; 2]);
        self.vignette = Some(start..self.indices.len() as u32);
    }

    /// Recorte do atlas (`sx, sy, sw, sh` em pixels do atlas) desenhado em `x, y, w, h`.
    /// Cor em 0xAARRGGBB (alfa 0 = opaco).
    #[allow(clippy::too_many_arguments)]
    pub fn sprite(&mut self, x: f32, y: f32, w: f32, h: f32, sx: f32, sy: f32, sw: f32, sh: f32, argb: u32) {
        let c = rgba(argb);
        self.quad([x, y, x + w, y + h], [sx, sy, sx + sw, sy + sh], [c, c]);
    }

    /// Quadro com cor de cima e de baixo; uv em pixels do atlas.
    fn quad(&mut self, [x0, y0, x1, y1]: [f32; 4], [u0, v0, u1, v1]: [f32; 4], [top, bottom]: [[u8; 4]; 2]) {
        let t = self.transform;
        let p = |x: f32, y: f32| match t {
            Some([a, b, c, d, tx, ty]) => [a * x + c * y + tx, b * x + d * y + ty],
            None => [x, y],
        };
        let b = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&[
            UiVertex { pos: p(x0, y0), uv: uv(u0, v0), color: top },
            UiVertex { pos: p(x1, y0), uv: uv(u1, v0), color: top },
            UiVertex { pos: p(x1, y1), uv: uv(u1, v1), color: bottom },
            UiVertex { pos: p(x0, y1), uv: uv(u0, v1), color: bottom },
        ]);
        self.indices.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }

    /// `Gui.drawGradientRect`: degradê vertical com alfa literal (0 = transparente).
    pub fn gradient(&mut self, x: f32, y: f32, w: f32, h: f32, top: u32, bottom: u32) {
        let c = |argb: u32| [(argb >> 16) as u8, (argb >> 8) as u8, argb as u8, (argb >> 24) as u8];
        let (u, v) = (WHITE.0 as f32, WHITE.1 as f32);
        self.quad([x, y, x + w, y + h], [u, v, u + 1.0, v + 1.0], [c(top), c(bottom)]);
    }

    /// Recorte do atlas com cor de alfa literal (para fundos com transparência).
    #[allow(clippy::too_many_arguments)]
    pub fn sprite_alpha(&mut self, x: f32, y: f32, w: f32, h: f32, sx: f32, sy: f32, sw: f32, sh: f32, argb: u32) {
        let c = [(argb >> 16) as u8, (argb >> 8) as u8, argb as u8, (argb >> 24) as u8];
        self.quad([x, y, x + w, y + h], [sx, sy, sx + sw, sy + sh], [c, c]);
    }

    /// Recorte de uma das texturas da GUI com origem `at` (coordenadas da textura de 256 px).
    #[allow(clippy::too_many_arguments)]
    pub fn gui(&mut self, at: (u32, u32), x: f32, y: f32, u: f32, v: f32, w: f32, h: f32) {
        self.sprite(x, y, w, h, at.0 as f32 + u, at.1 as f32 + v, w, h, 0xFFFFFF);
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, argb: u32) {
        self.sprite(x, y, w, h, WHITE.0 as f32, WHITE.1 as f32, 1.0, 1.0, argb);
    }

    /// Ícone de item na vaga `slot` (16x16 GUI px).
    pub fn item_icon(&mut self, slot: u32, x: f32, y: f32, size: f32) {
        let (sx, sy) = item_origin(slot);
        self.sprite(x, y, size, size, sx as f32, sy as f32, ITEM_PX as f32, ITEM_PX as f32, 0xFFFFFF);
    }

    /// Glifo: (origem no atlas, tamanho da célula em px do atlas, largura de avanço em GUI px, largura desenhada).
    fn glyph(&mut self, c: char) -> Option<(f32, f32, f32, f32, f32)> {
        if let Some(&i) = self.font.ascii.get(&c) {
            let (cx, cy) = (FONT.0 as f32 + (i % 16) as f32 * 8.0, FONT.1 as f32 + (i / 16) as f32 * 8.0);
            let w = self.font.widths[i as usize] as f32;
            return Some((cx, cy, 8.0, w, w - 1.0));
        }
        let code = c as u32;
        if code > 0xFFFF {
            return None;
        }
        let page = (code >> 8) as u8;
        let Some(&slot) = self.font.pages.get(&page) else {
            self.font.wanted.insert(page);
            return None;
        };
        let size = *self.font.glyph_sizes.get(code as usize)?;
        let (start, end) = ((size >> 4) as f32, (size & 15) as f32 + 1.0);
        let (ox, oy) = Font::page_origin(slot);
        let cx = ox as f32 + (code & 15) as f32 * 16.0 + start;
        let cy = oy as f32 + ((code >> 4) & 15) as f32 * 16.0;
        let w = (end - start) / 2.0;
        Some((cx, cy, 16.0, w + 1.0, w))
    }

    /// Largura de `s` na fonte "standard galactic" (`ascii_sga.png`).
    pub fn sga_width(&self, s: &str) -> f32 {
        s.chars().filter_map(|c| self.font.ascii.get(&c)).map(|&i| self.font.sga_widths[i as usize] as f32).sum()
    }

    /// Texto na fonte "standard galactic" (sem sombra, como `FontRenderer.drawString` do `standardGalacticFontRenderer`).
    pub fn text_sga(&mut self, x: f32, y: f32, s: &str, argb: u32) -> f32 {
        let mut x = x;
        for c in s.chars() {
            if let Some(&i) = self.font.ascii.get(&c) {
                let w = self.font.sga_widths[i as usize] as f32;
                let (sx, sy) = (SGA_FONT.0 as f32 + (i % 16) as f32 * 8.0, SGA_FONT.1 as f32 + (i / 16) as f32 * 8.0);
                self.sprite(x, y, w - 1.0, 8.0, sx, sy, w - 1.0, 8.0, argb);
                x += w;
            }
        }
        x
    }

    /// `getCharWidth` da fonte galáctica (`standardGalacticFontRenderer`, `ascii_sga.png`); fora da tabela
    /// ascii as páginas unicode são as mesmas.
    pub fn sga_char_width(&mut self, c: char) -> i32 {
        match c {
            '§' => -1,
            ' ' => 4,
            _ => match self.font.ascii.get(&c) {
                Some(&i) => self.font.sga_widths[i as usize] as i32,
                None => self.char_width(c),
            },
        }
    }

    /// `FontRenderer.getCharWidth`: `-1` para `§`, 4 para o espaço, a largura da célula do
    /// `ascii.png` e, fora dela, a do `glyph_sizes.bin` (divisão inteira, como no Java).
    pub fn char_width(&mut self, c: char) -> i32 {
        if c == '§' {
            return -1;
        }
        if c == ' ' {
            return 4;
        }
        if let Some(&i) = self.font.ascii.get(&c) {
            return self.font.widths[i as usize] as i32;
        }
        let code = c as u32;
        if code > 0xFFFF {
            return 0;
        }
        self.font.wanted.insert((code >> 8) as u8);
        match self.font.glyph_sizes.get(code as usize) {
            Some(&size) if size != 0 => {
                let (j, k) = ((size >> 4) as i32, (size & 15) as i32 + 1);
                (k - j) / 2 + 1
            }
            _ => 0,
        }
    }

    /// `FontRenderer.getStringWidth`: o negrito (`§l`) soma 1 por caractere até o `§r`.
    pub fn string_width(&mut self, s: &str) -> i32 {
        let chars: Vec<char> = s.chars().collect();
        let (mut total, mut bold) = (0, false);
        let mut j = 0;
        while j < chars.len() {
            let mut k = self.char_width(chars[j]);
            if k < 0 && j + 1 < chars.len() {
                j += 1;
                match chars[j] {
                    'l' | 'L' => bold = true,
                    'r' | 'R' => bold = false,
                    _ => {}
                }
                k = 0;
            }
            total += k;
            if bold && k > 0 {
                total += 1;
            }
            j += 1;
        }
        total
    }

    pub fn text_width(&mut self, s: &str) -> f32 {
        self.string_width(s) as f32
    }

    /// Texto com códigos `§`. Devolve a largura.
    pub fn text(&mut self, x: f32, y: f32, s: &str, argb: u32, shadow: bool) -> f32 {
        self.text_scaled(x, y, s, argb, shadow, 1.0)
    }

    /// Texto ampliado `k` vezes a partir de `x, y` (títulos usam 4 e 2).
    pub fn text_scaled(&mut self, x: f32, y: f32, s: &str, argb: u32, shadow: bool, k: f32) -> f32 {
        if shadow {
            let base = argb & 0xFFFFFF;
            let dark = (argb & 0xFF000000) | ((base & 0xFCFCFC) >> 2);
            self.text_pass(x + k, y + k, s, dark, true, k);
        }
        self.text_pass(x, y, s, argb, false, k)
    }

    /// Sorteio do `fontRandom` do texto ofuscado (xorshift; o jogo também sorteia a cada quadro).
    fn next_random(&mut self, bound: usize) -> usize {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        (x % bound as u64) as usize
    }

    /// `FontRenderer.renderStringAtPos`: cores, negrito, itálico, sublinhado, tachado e ofuscado.
    fn text_pass(&mut self, x: f32, y: f32, s: &str, argb: u32, shadow: bool, k: f32) -> f32 {
        const CODES: &str = "0123456789abcdefklmnor";
        let alpha = argb & 0xFF000000;
        let mut color = argb;
        let (mut bold, mut italic, mut underline, mut strike, mut random) = (false, false, false, false, false);
        let mut cx = x;
        let chars: Vec<char> = s.chars().collect();
        let table: Vec<char> = if s.contains("§k") || s.contains("§K") { ASCII.chars().collect() } else { Vec::new() };
        let mut i = 0;
        while i < chars.len() {
            let mut c = chars[i];
            i += 1;
            if c == '§' && i < chars.len() {
                let code = chars[i].to_ascii_lowercase();
                i += 1;
                match CODES.find(code) {
                    Some(n) if n >= 16 => match n {
                        16 => random = true,
                        17 => bold = true,
                        18 => strike = true,
                        19 => underline = true,
                        20 => italic = true,
                        _ => {
                            (random, bold, strike, underline, italic) = (false, false, false, false, false);
                            color = argb;
                        }
                    },
                    n => {
                        // Código desconhecido (índice -1 no Java) cai na cor 15: branco.
                        (random, bold, strike, underline, italic) = (false, false, false, false, false);
                        let rgb = COLORS[n.unwrap_or(15)];
                        color = alpha | if shadow { (rgb & 0xFCFCFC) >> 2 } else { rgb };
                    }
                }
                continue;
            }
            if random && self.font.ascii.contains_key(&c) {
                let w = self.char_width(c);
                loop {
                    let r = table[self.next_random(table.len())];
                    if self.char_width(r) == w {
                        c = r;
                        break;
                    }
                }
            }
            let f1 = if self.font.ascii.contains_key(&c) { 1.0 } else { 0.5 };
            let mut adv = 0.0;
            if c == ' ' {
                adv = 4.0;
            } else if let Some((sx, sy, cell, a, draw_w)) = self.glyph(c) {
                adv = a;
                let src = cell / 8.0;
                let skew = if italic { k } else { 0.0 };
                let (cw, cs) = (draw_w * k, draw_w * src);
                self.skewed(cx, y, cw, 8.0 * k, [sx, sy, cs, cell], skew, color);
                if bold {
                    self.skewed(cx + f1 * k, y, cw, 8.0 * k, [sx, sy, cs, cell], skew, color);
                    adv += 1.0;
                }
            }
            // `doDraw`: tachado e sublinhado cobrem o avanço (inclusive no espaço) e o cursor anda `(int) f`.
            if strike {
                self.rect(cx, y + 4.0 * k - k, adv * k, k, color);
            }
            if underline {
                self.rect(cx - k, y + 9.0 * k - k, (adv + 1.0) * k, k, color);
            }
            cx += adv.floor() * k;
        }
        cx - x
    }

    /// Glifo `w x h` em `x, y` com a borda de cima deslocada `skew` para a direita e a de baixo
    /// para a esquerda (`renderChar(c, italic)`). `uv` = origem, largura e altura no atlas.
    fn skewed(&mut self, x: f32, y: f32, w: f32, h: f32, [sx, sy, sw, sh]: [f32; 4], skew: f32, argb: u32) {
        let c = rgba(argb);
        let t = self.transform;
        let p = |px: f32, py: f32| match t {
            Some([a, b, cc, d, tx, ty]) => [a * px + cc * py + tx, b * px + d * py + ty],
            None => [px, py],
        };
        let b = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&[
            UiVertex { pos: p(x + skew, y), uv: uv(sx, sy), color: c },
            UiVertex { pos: p(x + w + skew, y), uv: uv(sx + sw, sy), color: c },
            UiVertex { pos: p(x + w - skew, y + h), uv: uv(sx + sw, sy + sh), color: c },
            UiVertex { pos: p(x - skew, y + h), uv: uv(sx, sy + sh), color: c },
        ]);
        self.indices.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }
}

/// UV de um texel branco do atlas da GUI (para linhas e fundos em 3D).
pub fn white_uv() -> [f32; 2] {
    uv(WHITE.0 as f32 + 0.5, WHITE.1 as f32 + 0.5)
}

pub fn item_origin(slot: u32) -> (u32, u32) {
    (ITEMS.0 + (slot % ITEM_COLS) * ITEM_PX, ITEMS.1 + (slot / ITEM_COLS) * ITEM_PX)
}

/// Tira os códigos `§` (para o terminal).
pub fn strip_codes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut skip = false;
    for c in s.chars() {
        if skip {
            skip = false;
        } else if c == '§' {
            skip = true;
        } else {
            out.push(c);
        }
    }
    out
}
