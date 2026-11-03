//! Leitura do jar 1.8.9 do jogador e montagem do atlas de blocos.

use image::RgbaImage;
use mc_world::mesh::ATLAS_COLS;

pub use crate::resources::Jar;

pub(crate) const TILE: u32 = 16;

/// Tamanho máximo de um tile do atlas (32 colunas x 128 px = 4096 px de lado, 64 MiB).
pub const MAX_TILE: u32 = 128;

/// Atlas RGBA com todos os níveis de mipmap (do tile inteiro até 1 px por textura).
pub struct AtlasImage {
    pub size: u32,
    /// Lado de cada tile em pixels (16 no jogo padrão; 32, 64, 128 com pacotes HD).
    pub tile: u32,
    pub levels: Vec<Vec<u8>>,
}

fn missing_tile(tile: u32) -> RgbaImage {
    let half = (tile / 2).max(1);
    RgbaImage::from_fn(tile, tile, |x, y| {
        if (x / half + y / half) % 2 == 0 {
            image::Rgba([248, 0, 248, 255])
        } else {
            image::Rgba([0, 0, 0, 255])
        }
    })
}

/// Textura virtual "@caminho:x,y,w,h+x,y,w,h": recortes de `textures/<caminho>.png`
/// empilhados na vertical (coordenadas da textura em 64 px de largura), amostrados em `tile` px.
fn virtual_tile(jar: &mut Jar, spec: &str, tile: u32) -> Option<RgbaImage> {
    let (path, rects) = spec.rsplit_once(':')?;
    let img = jar.texture(path)?;
    let k = img.width() as f32 / 64.0;
    let rects: Vec<[u32; 4]> = rects
        .split('+')
        .filter_map(|r| {
            let v: Vec<u32> = r.split(',').filter_map(|n| n.parse().ok()).collect();
            (v.len() == 4).then(|| [v[0], v[1], v[2], v[3]])
        })
        .collect();
    let total_h: u32 = rects.iter().map(|r| r[3]).sum::<u32>().max(1);
    let mut out = RgbaImage::new(tile, tile);
    for py in 0..tile {
        let mut sy = py as f32 * total_h as f32 / tile as f32;
        let mut rect = *rects.first()?;
        for r in &rects {
            if sy < r[3] as f32 {
                rect = *r;
                break;
            }
            sy -= r[3] as f32;
        }
        for px in 0..tile {
            let sx = rect[0] as f32 + px as f32 * rect[2] as f32 / tile as f32;
            let syy = rect[1] as f32 + sy;
            let p = img.get_pixel(((sx * k) as u32).min(img.width() - 1), ((syy * k) as u32).min(img.height() - 1));
            out.put_pixel(px, py, *p);
        }
    }
    Some(out)
}

/// A imagem de um bloco como veio do pacote (sem reduzir): tiras animadas viram só o primeiro
/// quadro (o quadrado de cima), e `virtual_tile` já sai em `tile` px.
fn raw_tile(jar: &mut Jar, name: &str, tile: u32) -> Option<RgbaImage> {
    if let Some(spec) = name.strip_prefix('@') {
        return virtual_tile(jar, spec, tile);
    }
    let img = jar.texture(&format!("blocks/{name}"))?;
    let w = img.width().max(1);
    if img.height() > w {
        return Some(image::imageops::crop_imm(&img, 0, 0, w, w).to_image());
    }
    Some(img)
}

/// Leva a imagem ao tamanho do tile: ampliação por vizinho mais próximo (como o filtro do jogo),
/// redução por média quando o pacote passa do máximo do atlas.
fn fit_tile(img: RgbaImage, tile: u32) -> RgbaImage {
    let w = img.width().max(1);
    if w == tile && img.height() == tile {
        return img;
    }
    if w > tile {
        return image::imageops::resize(&img, tile, tile, image::imageops::FilterType::Triangle);
    }
    RgbaImage::from_fn(tile, tile, |x, y| *img.get_pixel(x * w / tile, (y * img.height().max(1) / tile).min(img.height() - 1)))
}

/// Uma textura de bloco no tamanho do atlas (`tile` px) pelo nome usado na tabela de blocos.
pub fn block_tile(jar: &mut Jar, name: &str, tile: u32) -> Option<RgbaImage> {
    Some(fit_tile(raw_tile(jar, name, tile)?, tile))
}

/// Lado do tile para estes `names`: a maior textura, em potência de 2 entre 16 e `MAX_TILE`.
pub fn tile_size_for(jar: &mut Jar, names: &[String]) -> u32 {
    let widest = names
        .iter()
        .filter_map(|n| match n.strip_prefix('@') {
            // Textura virtual: os recortes são em 64 px de largura; cada px vale largura/64 do pacote.
            Some(spec) => spec.rsplit_once(':').and_then(|(p, _)| jar.texture(p)).map(|i| (i.width() / 4).max(1)),
            None => jar.texture(&format!("blocks/{n}")).map(|i| i.width()),
        })
        .max()
        .unwrap_or(TILE);
    widest.next_power_of_two().clamp(TILE, MAX_TILE)
}

/// Monta o atlas com `names` (índice = posição no vetor). Devolve também as que faltaram.
pub fn build_atlas(jar: &mut Jar, names: &[String]) -> Result<(AtlasImage, Vec<String>), String> {
    let cols = ATLAS_COLS;
    if names.len() > (cols * cols) as usize {
        return Err(format!("{} texturas não cabem no atlas", names.len()));
    }
    let tile = tile_size_for(jar, names);
    let size = cols * tile;
    let mut base = vec![0u8; (size as usize) * (size as usize) * 4];
    let mut missing = Vec::new();
    for (i, name) in names.iter().enumerate() {
        let img = block_tile(jar, name, tile).unwrap_or_else(|| {
            missing.push(name.clone());
            missing_tile(tile)
        });
        let (cx, cy) = ((i as u32 % cols) * tile, (i as u32 / cols) * tile);
        for py in 0..tile {
            let o = (((cy + py) * size + cx) * 4) as usize;
            let row = &img.as_raw()[(py * tile * 4) as usize..((py + 1) * tile * 4) as usize];
            base[o..o + (tile * 4) as usize].copy_from_slice(row);
        }
    }

    // Mipmaps por média 2x2: as texturas são alinhadas em potências de 2, então nada vaza entre elas.
    let mut levels = vec![base];
    let mut s = size;
    while s > cols {
        let prev = levels.last().unwrap();
        let n = s / 2;
        let mut next = vec![0u8; (n * n * 4) as usize];
        for y in 0..n {
            for x in 0..n {
                for c in 0..4 {
                    let at = |dx: u32, dy: u32| prev[(((2 * y + dy) * s + 2 * x + dx) * 4 + c) as usize] as u32;
                    next[((y * n + x) * 4 + c) as usize] = ((at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1)) / 4) as u8;
                }
            }
        }
        levels.push(next);
        s = n;
    }
    Ok((AtlasImage { size, tile, levels }, missing))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_tile_scales_nearest_and_averages_down() {
        let img = RgbaImage::from_fn(2, 2, |x, y| image::Rgba([(x * 100) as u8, (y * 100) as u8, 0, 255]));
        let up = fit_tile(img.clone(), 8);
        assert_eq!(up.dimensions(), (8, 8));
        assert_eq!(up.get_pixel(0, 0).0, [0, 0, 0, 255]);
        assert_eq!(up.get_pixel(3, 3).0, [0, 0, 0, 255]);
        assert_eq!(up.get_pixel(4, 0).0, [100, 0, 0, 255]);
        assert_eq!(up.get_pixel(7, 7).0, [100, 100, 0, 255]);
        let down = fit_tile(RgbaImage::from_pixel(8, 8, image::Rgba([10, 20, 30, 255])), 4);
        assert_eq!(down.dimensions(), (4, 4));
        assert_eq!(down.get_pixel(1, 1).0, [10, 20, 30, 255]);
    }

    /// Pacote em pasta com tiles 16 e 64: o atlas passa para 64 px por tile e os pequenos são ampliados.
    #[test]
    fn atlas_grows_with_hd_tiles() {
        let d = std::env::temp_dir().join(format!("mine-rs-atlas-{}", std::process::id()));
        let tex = d.join("assets/minecraft/textures/blocks");
        std::fs::create_dir_all(&tex).unwrap();
        std::fs::write(d.join("pack.mcmeta"), br#"{"pack":{"pack_format":1,"description":"t"}}"#).unwrap();
        let mut png = |name: &str, w: u32, color: [u8; 4]| {
            let img = RgbaImage::from_pixel(w, w, image::Rgba(color));
            img.save(tex.join(format!("{name}.png"))).unwrap();
        };
        png("pequena", 16, [255, 0, 0, 255]);
        png("grande", 64, [0, 255, 0, 255]);
        let mut jar = Jar::from_packs(&[d.clone()]);
        let names: Vec<String> = ["pequena", "grande", "falta"].iter().map(|s| s.to_string()).collect();
        let (atlas, missing) = build_atlas(&mut jar, &names).unwrap();
        assert_eq!((atlas.tile, atlas.size), (64, 64 * ATLAS_COLS));
        assert_eq!(missing, vec!["falta".to_string()]);
        // 64 px -> 1 px: sete níveis (64, 32, 16, 8, 4, 2, 1 por tile).
        assert_eq!(atlas.levels.len(), 7);
        let px = |x: u32, y: u32| {
            let o = ((y * atlas.size + x) * 4) as usize;
            [atlas.levels[0][o], atlas.levels[0][o + 1], atlas.levels[0][o + 2]]
        };
        assert_eq!(px(10, 10), [255, 0, 0]);
        assert_eq!(px(64 + 63, 63), [0, 255, 0]);
        // Último nível: um pixel por tile (a média do tile).
        let last = atlas.levels.last().unwrap();
        assert_eq!(&last[0..3], &[255, 0, 0]);
        assert_eq!(&last[4..7], &[0, 255, 0]);
        // Sem o pacote (pilha vazia) tudo falta e o atlas volta a 16 px.
        jar.set_packs(&[]);
        let (atlas, missing) = build_atlas(&mut jar, &names).unwrap();
        assert_eq!((atlas.tile, missing.len()), (16, 3));
        let _ = std::fs::remove_dir_all(d);
    }
}
