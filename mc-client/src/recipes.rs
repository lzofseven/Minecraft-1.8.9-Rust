//! `CraftingManager.findMatchingRecipe` no cliente: o resultado da grade de criação aparece na hora (o
//! `onCraftMatrixChanged` do `ContainerPlayer`/`ContainerWorkbench`). Receitas geradas do forgeBin.

use mc_protocol::nbt::Slot;

use crate::gen_recipes::RECIPES;

/// Uma receita: cada ingrediente é a lista de (item, dano) aceitos (32767 = qualquer dano); vazia = célula vazia.
pub enum R {
    Shaped { w: usize, h: usize, mirrored: bool, items: &'static [&'static [(i16, i16)]], out: (i16, u8, i16) },
    Shapeless { items: &'static [&'static [(i16, i16)]], out: (i16, u8, i16) },
    /// `RecipesArmorDyes`, `RecipeFireworks`, `RecipeRepairItem`...: pela classe.
    Special(&'static str),
}

fn accepts(alt: &[(i16, i16)], s: &Slot) -> bool {
    alt.iter().any(|&(id, m)| id == s.id && (m == 32767 || m == s.damage))
}

/// Uma receita comum casa com a grade.
fn matches_regular(r: &R, grid: &[Option<Slot>], width: usize) -> bool {
    let height = grid.len() / width.max(1);
    // InventoryCrafting.getStackInRowAndColumn: fora da grade é vazio.
    let at = |x: i32, y: i32| (x >= 0 && y >= 0 && (x as usize) < width && (y as usize) < height).then(|| grid[x as usize + y as usize * width].as_ref()).flatten();
    match r {
        R::Shaped { w, h, mirrored, items, .. } => {
            let (w, h) = (*w as i32, *h as i32);
            // ShapedRecipes.checkMatch (o espelhado primeiro).
            let check = |i: i32, j: i32, mirror: bool| {
                (0..3).all(|x| {
                    (0..3).all(|y| {
                        let (k, l) = (x - i, y - j);
                        let expected = (k >= 0 && l >= 0 && k < w && l < h).then(|| items[if mirror { w - k - 1 + l * w } else { k + l * w } as usize]).filter(|a| !a.is_empty());
                        match (expected, at(x, y)) {
                            (None, None) => true,
                            (Some(a), Some(s)) => accepts(a, s),
                            _ => false,
                        }
                    })
                })
            };
            (0..=(3 - w)).any(|i| (0..=(3 - h)).any(|j| (*mirrored && check(i, j, true)) || check(i, j, false)))
        }
        R::Shapeless { items, .. } => {
            // ShapelessRecipes.matches: cada item da grade gasta um ingrediente; no fim não sobra nenhum.
            let mut need: Vec<&[(i16, i16)]> = items.to_vec();
            let ok = grid.iter().flatten().all(|s| match need.iter().position(|a| accepts(a, s)) {
                Some(p) => {
                    need.remove(p);
                    true
                }
                None => false,
            });
            ok && need.is_empty()
        }
        R::Special(_) => false,
    }
}

/// `findMatchingRecipe` numa grade `width` de largura, na ordem da lista (as especiais também).
pub fn find(grid: &[Option<Slot>], width: usize) -> Option<Slot> {
    if grid.iter().all(Option::is_none) {
        return None;
    }
    for r in RECIPES {
        match r {
            R::Special(name) => {
                if let Some(s) = special(name, grid) {
                    return Some(s);
                }
            }
            R::Shaped { out, .. } | R::Shapeless { out, .. } => {
                if matches_regular(r, grid, width) {
                    return Some(Slot { id: out.0, count: out.1, damage: out.2, nbt: None, raw_nbt: Vec::new() });
                }
            }
        }
    }
    None
}

/// `Item.getContainerItem`: o que volta à grade (balde de água, lava e leite -> balde).
pub fn container_item(id: i16) -> Option<i16> {
    crate::gen_recipes::CONTAINER_ITEMS.iter().find(|(i, _)| *i == id).map(|(_, c)| *c)
}

// ------------------------------------------------------------------------------------- receitas especiais

use std::collections::HashMap;

use mc_protocol::nbt::Nbt;

use crate::gen_special_recipes::{BANNER_PATTERNS, DYE_COLORS, DYE_RGB, LEATHER, REPAIRABLE};

const DYE: i16 = 351;
const BANNER: i16 = 425;

fn compound(s: &Slot) -> Option<&HashMap<String, Nbt>> {
    match &s.nbt {
        Some(Nbt::Compound(m)) => Some(m),
        _ => None,
    }
}

fn sub<'a>(m: &'a HashMap<String, Nbt>, k: &str) -> Option<&'a HashMap<String, Nbt>> {
    match m.get(k) {
        Some(Nbt::Compound(c)) => Some(c),
        _ => None,
    }
}

/// `getSubCompound(key, true)`.
fn sub_mut<'a>(m: &'a mut HashMap<String, Nbt>, k: &str) -> &'a mut HashMap<String, Nbt> {
    if !matches!(m.get(k), Some(Nbt::Compound(_))) {
        m.insert(k.to_string(), Nbt::Compound(HashMap::new()));
    }
    match m.get_mut(k) {
        Some(Nbt::Compound(c)) => c,
        _ => unreachable!(),
    }
}

fn int(m: Option<&HashMap<String, Nbt>>, k: &str) -> Option<i32> {
    match m?.get(k) {
        Some(Nbt::Int(v)) => Some(*v),
        Some(Nbt::Short(v)) => Some(*v as i32),
        Some(Nbt::Byte(v)) => Some(*v as i32),
        _ => None,
    }
}

/// O item com este NBT (`setTagCompound`), com os bytes que vão de volta ao servidor.
fn with_tag(mut s: Slot, tag: HashMap<String, Nbt>) -> Slot {
    let n = Nbt::Compound(tag);
    s.raw_nbt = n.write_root();
    s.nbt = Some(n);
    s
}

fn one(s: &Slot) -> Slot {
    let mut c = s.clone();
    c.count = 1;
    c
}

/// `TileEntityBanner.getPatterns`: padrões no `BlockEntityTag`.
fn banner_patterns(s: &Slot) -> usize {
    match compound(s).and_then(|m| sub(m, "BlockEntityTag")).and_then(|b| b.get("Patterns")) {
        Some(Nbt::List(l)) => l.len(),
        _ => 0,
    }
}

/// `TileEntityBanner.getBaseColor`: `Base` do `BlockEntityTag`, senão o dano.
fn banner_base(s: &Slot) -> i32 {
    int(compound(s).and_then(|m| sub(m, "BlockEntityTag")), "Base").unwrap_or(s.damage as i32)
}

/// Uma especial pela classe: o resultado (`None` = não casa) e o que sobra na grade.
fn special(name: &str, grid: &[Option<Slot>]) -> Option<Slot> {
    let items: Vec<&Slot> = grid.iter().flatten().collect();
    match name {
        // RecipesArmorDyes: uma armadura de couro e corantes; a cor é a média com o brilho do mais claro.
        "RecipesArmorDyes" => {
            let mut armor: Option<Slot> = None;
            let (mut i, mut j, mut acc) = (0i32, 0i32, [0i32; 3]);
            for s in &items {
                if LEATHER.contains(&s.id) {
                    if armor.is_some() {
                        return None;
                    }
                    let a = one(s);
                    if let Some(l) = int(compound(s).and_then(|m| sub(m, "display")), "color") {
                        let (f, f1, f2) = (((l >> 16) & 255) as f32 / 255.0, ((l >> 8) & 255) as f32 / 255.0, (l & 255) as f32 / 255.0);
                        i = (i as f32 + f.max(f1.max(f2)) * 255.0) as i32;
                        acc[0] = (acc[0] as f32 + f * 255.0) as i32;
                        acc[1] = (acc[1] as f32 + f1 * 255.0) as i32;
                        acc[2] = (acc[2] as f32 + f2 * 255.0) as i32;
                        j += 1;
                    }
                    armor = Some(a);
                } else if s.id == DYE {
                    let d = if (0..16).contains(&s.damage) { s.damage as usize } else { 0 };
                    let rgb = DYE_RGB[d].map(f32::from_bits);
                    let (l1, i2, j2) = ((rgb[0] * 255.0) as i32, (rgb[1] * 255.0) as i32, (rgb[2] * 255.0) as i32);
                    i += l1.max(i2.max(j2));
                    acc[0] += l1;
                    acc[1] += i2;
                    acc[2] += j2;
                    j += 1;
                } else {
                    return None;
                }
            }
            let armor = armor?;
            if !items.iter().any(|s| s.id == DYE) {
                return None;
            }
            let (mut i1, mut j1, mut k1) = (acc[0] / j, acc[1] / j, acc[2] / j);
            let f3 = i as f32 / j as f32;
            let f4 = i1.max(j1.max(k1)) as f32;
            i1 = (i1 as f32 * f3 / f4) as i32;
            j1 = (j1 as f32 * f3 / f4) as i32;
            k1 = (k1 as f32 * f3 / f4) as i32;
            let color = (((i1 << 8) + j1) << 8) + k1;
            let mut tag = compound(&armor).cloned().unwrap_or_default();
            sub_mut(&mut tag, "display").insert("color".into(), Nbt::Int(color));
            Some(with_tag(armor, tag))
        }
        // RecipeRepairItem: dois do mesmo item reparável, um de cada; soma as durabilidades com 5% a mais.
        "RecipeRepairItem" => {
            if items.len() != 2 {
                return None;
            }
            let (a, b) = (items[0], items[1]);
            if a.id != b.id || a.count != 1 || b.count != 1 || !REPAIRABLE.contains(&a.id) {
                return None;
            }
            let max = crate::containers::max_damage(a.id);
            let l = (max - a.damage as i32) + (max - b.damage as i32) + max * 5 / 100;
            Some(Slot { id: a.id, count: 1, damage: (max - l).max(0) as i16, nbt: None, raw_nbt: Vec::new() })
        }
        // RecipeBookCloning: um livro escrito (geração < 2) e livros para escrever viram cópias (geração + 1).
        "RecipeBookCloning" => {
            let written: Vec<&&Slot> = items.iter().filter(|s| s.id == 387).collect();
            if written.len() != 1 || items.iter().any(|s| s.id != 387 && s.id != 386) {
                return None;
            }
            let n = items.iter().filter(|s| s.id == 386).count();
            let w = written[0];
            let mut tag = compound(w)?.clone();
            let generation = int(Some(&tag), "generation").unwrap_or(0);
            if n < 1 || generation >= 2 {
                return None;
            }
            tag.insert("generation".into(), Nbt::Int(generation + 1));
            Some(with_tag(Slot { id: 387, count: n as u8, damage: 0, nbt: None, raw_nbt: Vec::new() }, tag))
        }
        // RecipesMapCloning: um mapa preenchido e mapas vazios viram cópias (mesmo dano e nome).
        "RecipesMapCloning" => {
            let filled: Vec<&&Slot> = items.iter().filter(|s| s.id == 358).collect();
            if filled.len() != 1 || items.iter().any(|s| s.id != 358 && s.id != 395) {
                return None;
            }
            let n = items.iter().filter(|s| s.id == 395).count();
            if n < 1 {
                return None;
            }
            let f = filled[0];
            let out = Slot { id: 358, count: (n + 1) as u8, damage: f.damage, nbt: None, raw_nbt: Vec::new() };
            match compound(f).and_then(|m| sub(m, "display")).and_then(|d| d.get("Name")) {
                Some(Nbt::String(name)) => {
                    let mut tag = HashMap::new();
                    sub_mut(&mut tag, "display").insert("Name".into(), Nbt::String(name.clone()));
                    Some(with_tag(out, tag))
                }
                _ => Some(out),
            }
        }
        "RecipeFireworks" => fireworks(&items),
        // RecipeAddPattern: um estandarte (menos de 6 padrões) e o desenho do padrão com um corante.
        "RecipeAddPattern" => {
            let banners: Vec<&&Slot> = items.iter().filter(|s| s.id == BANNER).collect();
            if banners.len() != 1 || banner_patterns(banners[0]) >= 6 {
                return None;
            }
            let pattern = banner_pattern(grid)?;
            let mut out = one(banners[0]);
            let k = items.iter().find(|s| s.id == DYE).map_or(0, |s| s.damage as i32);
            let mut tag = compound(&out).cloned().unwrap_or_default();
            let bet = sub_mut(&mut tag, "BlockEntityTag");
            if !matches!(bet.get("Patterns"), Some(Nbt::List(_))) {
                bet.insert("Patterns".into(), Nbt::List(Vec::new()));
            }
            if let Some(Nbt::List(l)) = bet.get_mut("Patterns") {
                let mut p = HashMap::new();
                p.insert("Pattern".into(), Nbt::String(pattern.to_string()));
                p.insert("Color".into(), Nbt::Int(k));
                l.push(Nbt::Compound(p));
            }
            out = with_tag(out, tag);
            Some(out)
        }
        // RecipeDuplicatePattern: um estandarte com padrões e um liso da mesma cor de base.
        "RecipeDuplicatePattern" => {
            let (mut with, mut plain): (Option<&Slot>, Option<&Slot>) = (None, None);
            for s in &items {
                if s.id != BANNER || (with.is_some() && plain.is_some()) {
                    return None;
                }
                let base = banner_base(s);
                let has = banner_patterns(s) > 0;
                if let Some(w) = with {
                    if has || base != banner_base(w) {
                        return None;
                    }
                    plain = Some(s);
                } else if let Some(p) = plain {
                    if !has || base != banner_base(p) {
                        return None;
                    }
                    with = Some(s);
                } else if has {
                    with = Some(s);
                } else {
                    plain = Some(s);
                }
            }
            plain?;
            Some(one(with?))
        }
        _ => None,
    }
}

/// `RecipesBanners.RecipeAddPattern.func_179533_c`: o primeiro padrão cujo desenho a grade faz.
fn banner_pattern(grid: &[Option<Slot>]) -> Option<&'static str> {
    for (id, valid, stack, layers) in BANNER_PATTERNS {
        if !valid {
            continue;
        }
        let mut ok = true;
        if stack.0 >= 0 {
            let (mut has_stack, mut has_dye) = (false, false);
            for s in grid.iter().flatten().filter(|s| s.id != BANNER) {
                if s.id == DYE {
                    if has_dye {
                        ok = false;
                        break;
                    }
                    has_dye = true;
                } else {
                    if has_stack || s.id != stack.0 || s.damage != stack.1 {
                        ok = false;
                        break;
                    }
                    has_stack = true;
                }
            }
            if !has_stack {
                ok = false;
            }
        } else if grid.len() == 9 {
            let mut color = -1i32;
            for (k, cell) in grid.iter().enumerate() {
                let want = layers[k / 3].as_bytes().get(k % 3).copied().unwrap_or(b' ') != b' ';
                match cell.as_ref().filter(|s| s.id != BANNER) {
                    Some(s) => {
                        if s.id != DYE || (color != -1 && color != s.damage as i32) || !want {
                            ok = false;
                            break;
                        }
                        color = s.damage as i32;
                    }
                    None if want => {
                        ok = false;
                        break;
                    }
                    None => {}
                }
            }
        } else {
            ok = false;
        }
        if ok {
            return Some(id);
        }
    }
    None
}

/// `RecipeFireworks.matches`: o foguete, a estrela ou o desbotado da estrela.
fn fireworks(items: &[&Slot]) -> Option<Slot> {
    let (mut paper, mut gunpowder, mut dyes, mut charges, mut effects, mut shapes) = (0, 0, 0, 0, 0, 0);
    for s in items {
        match s.id {
            289 => gunpowder += 1,
            402 => charges += 1,
            351 => dyes += 1,
            339 => paper += 1,
            348 | 264 => effects += 1,
            385 | 288 | 371 | 397 => shapes += 1,
            _ => return None,
        }
    }
    let others = effects + dyes + shapes;
    if gunpowder > 3 || paper > 1 {
        return None;
    }
    let item = |id: i16| Slot { id, count: 1, damage: 0, nbt: None, raw_nbt: Vec::new() };
    if gunpowder >= 1 && paper == 1 && others == 0 {
        let mut tag = HashMap::new();
        if charges > 0 {
            let explosions: Vec<Nbt> = items.iter().filter(|s| s.id == 402).filter_map(|s| compound(s).and_then(|m| sub(m, "Explosion")).cloned().map(Nbt::Compound)).collect();
            let mut fw = HashMap::new();
            fw.insert("Explosions".into(), Nbt::List(explosions));
            fw.insert("Flight".into(), Nbt::Byte(gunpowder as i8));
            tag.insert("Fireworks".into(), Nbt::Compound(fw));
        }
        Some(with_tag(item(401), tag))
    } else if gunpowder == 1 && paper == 0 && charges == 0 && dyes > 0 && shapes <= 1 {
        let mut ex = HashMap::new();
        let mut colors = Vec::new();
        let mut kind = 0i8;
        for s in items {
            match s.id {
                351 => colors.push(DYE_COLORS[(s.damage & 15) as usize]),
                348 => {
                    ex.insert("Flicker".into(), Nbt::Byte(1));
                }
                264 => {
                    ex.insert("Trail".into(), Nbt::Byte(1));
                }
                385 => kind = 1,
                288 => kind = 4,
                371 => kind = 2,
                397 => kind = 3,
                _ => {}
            }
        }
        ex.insert("Colors".into(), Nbt::IntArray(colors));
        ex.insert("Type".into(), Nbt::Byte(kind));
        let mut tag = HashMap::new();
        tag.insert("Explosion".into(), Nbt::Compound(ex));
        Some(with_tag(item(402), tag))
    } else if gunpowder == 0 && paper == 0 && charges == 1 && dyes > 0 && dyes == others {
        let fade: Vec<i32> = items.iter().filter(|s| s.id == 351).map(|s| DYE_COLORS[(s.damage & 15) as usize]).collect();
        let charge = one(items.iter().find(|s| s.id == 402)?);
        let mut tag = compound(&charge)?.clone();
        // getCompoundTag("Explosion") sem a chave devolve um composto solto: o resultado fica sem o desbotado.
        if let Some(Nbt::Compound(ex)) = tag.get_mut("Explosion") {
            ex.insert("FadeColors".into(), Nbt::IntArray(fade));
        }
        Some(with_tag(charge, tag))
    } else {
        None
    }
}

/// `getRemainingItems` da receita que casou: o item de container; o livro escrito e o estandarte com padrões
/// das receitas de cópia ficam na grade.
pub fn remaining(grid: &[Option<Slot>], width: usize) -> Vec<Option<Slot>> {
    let which = matched_special(grid, width);
    let mut out: Vec<Option<Slot>> = vec![None; grid.len()];
    match which {
        Some("RecipeBookCloning") => {
            if let Some(i) = grid.iter().position(|s| s.as_ref().is_some_and(|s| s.id == 387)) {
                out[i] = grid[i].clone();
            }
        }
        Some("RecipeDuplicatePattern") => {
            for (i, s) in grid.iter().enumerate() {
                if let Some(s) = s {
                    if let Some(c) = container_item(s.id) {
                        out[i] = Some(Slot { id: c, count: 1, damage: 0, nbt: None, raw_nbt: Vec::new() });
                    } else if banner_patterns(s) > 0 {
                        out[i] = Some(one(s));
                    }
                }
            }
        }
        _ => {
            for (i, s) in grid.iter().enumerate() {
                if let Some(c) = s.as_ref().and_then(|s| container_item(s.id)) {
                    out[i] = Some(Slot { id: c, count: 1, damage: 0, nbt: None, raw_nbt: Vec::new() });
                }
            }
        }
    }
    out
}

/// A especial que decide a grade (a primeira da lista que casa antes de uma receita comum), se alguma.
fn matched_special(grid: &[Option<Slot>], width: usize) -> Option<&'static str> {
    for r in RECIPES {
        match r {
            R::Special(name) => {
                if special(name, grid).is_some() {
                    return Some(name);
                }
            }
            _ => {
                if matches_regular(r, grid, width) {
                    return None;
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(id: i16, damage: i16) -> Option<Slot> {
        Some(Slot { id, count: 1, damage, nbt: None, raw_nbt: Vec::new() })
    }

    #[test]
    fn tabuas_mesa_e_espada() {
        // Tronco de bétula (17:2) em qualquer canto da grade 2x2 -> 4 tábuas de bétula (5:2).
        let r = find(&[None, None, None, s(17, 2)], 2).unwrap();
        assert_eq!((r.id, r.count, r.damage), (5, 4, 2));
        // 4 tábuas de qualquer tipo -> mesa de trabalho.
        let r = find(&[s(5, 1), s(5, 0), s(5, 3), s(5, 2)], 2).unwrap();
        assert_eq!(r.id, 58);
        // Espada de pedra na coluna da direita da mesa 3x3.
        let mut g = vec![None; 9];
        g[2] = s(4, 0);
        g[5] = s(4, 0);
        g[8] = s(280, 0);
        assert_eq!(find(&g, 3).unwrap().id, 272);
        assert!(find(&[s(3, 0), None, None, None], 2).is_none());
        // Especiais: capacete de couro + corante vermelho (1); reparo de duas picaretas de ferro; foguete.
        let r = find(&[s(298, 0), s(351, 1), None, None], 2).unwrap();
        let color = match r.nbt.as_ref().and_then(|n| n.get("display")).and_then(|d| d.get("color")) { Some(mc_protocol::nbt::Nbt::Int(c)) => *c, _ => -1 };
        assert_eq!(color, 0x993333);
        let r = find(&[s(257, 200), s(257, 100), None, None], 2).unwrap();
        assert_eq!((r.id, r.damage), (257, (250 - ((250 - 200) + (250 - 100) + 250 * 5 / 100)).max(0) as i16));
        assert_eq!(find(&[s(339, 0), s(289, 0), None, None], 2).unwrap().id, 401);
        assert_eq!(container_item(335), Some(325));
    }
}
