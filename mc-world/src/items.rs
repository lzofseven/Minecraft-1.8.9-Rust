//! Itens da 1.8: id + dano -> modelo (`RenderItem.registerItems`/`ItemModelMesher`) e cor por camada
//! (`Item.getColorFromItemStack`). As tabelas vêm de `gen_items.rs`, gerado por `scripts/gen_items/gen.sh`.

use crate::gen_items::{
    EGG_COLORS, ITEM_COLORS, ITEM_DAMAGEABLE, ITEM_MESH, ITEM_MODELS, POTION_COLORS, POTION_MASK,
};

pub const ID_POTION: i16 = 373;
pub const ID_SPAWN_EGG: i16 = 383;
/// `ItemPotion.isSplash`: bit 14 do dano.
const SPLASH_BIT: i16 = 16384;
/// `ItemArmor.getColor` sem NBT (couro cru).
pub const LEATHER_DEFAULT_COLOR: u32 = 10511680;

/// `ItemStack.isItemStackDamageable`: o `ItemModelMesher` usa dano 0 para esses itens.
pub fn damageable(id: i16) -> bool {
    id >= 0 && ITEM_DAMAGEABLE.binary_search(&(id as u16)).is_ok()
}

/// Dano usado como chave de modelo (`ItemModelMesher.getMetadata`).
pub fn model_meta(id: i16, damage: i16) -> i16 {
    if damageable(id) { 0 } else { damage }
}

/// Nome do modelo em `assets/minecraft/models/item/<nome>.json` (`ItemModelMesher.getItemModel`):
/// primeiro o par registrado (id, dano), depois a `ItemMeshDefinition` do item.
pub fn model_name(id: i16, damage: i16) -> Option<&'static str> {
    if id < 0 {
        return None;
    }
    let meta = model_meta(id, damage).max(0) as u16;
    let key = (id as u16, meta);
    if let Ok(i) = ITEM_MODELS.binary_search_by(|(i, m, _)| (*i, *m).cmp(&key)) {
        return Some(ITEM_MODELS[i].2);
    }
    let (_, plain, splash) = ITEM_MESH.iter().find(|(i, _, _)| *i == id as u16)?;
    Some(if damage & SPLASH_BIT != 0 { splash } else { plain })
}

/// `ItemStack.getItem().getColorFromItemStack(stack, camada)` como 0xRRGGBB.
/// `leather` é a cor do NBT (`display.color`) para armaduras de couro (`ItemArmor.getColor`).
pub fn layer_color(id: i16, damage: i16, layer: u32, leather: Option<u32>) -> u32 {
    const WHITE: u32 = 0xFFFFFF;
    match id {
        ID_POTION => {
            if layer > 0 {
                WHITE
            } else {
                POTION_COLORS[(damage as u16 & POTION_MASK) as usize]
            }
        }
        ID_SPAWN_EGG => match EGG_COLORS.iter().find(|(e, _, _)| *e as i16 == damage) {
            // ItemMonsterPlacer: camada 0 = base, demais = pintas; sem ovo registrado, branco.
            Some((_, base, spots)) => {
                if layer == 0 { *base } else { *spots }
            }
            None => WHITE,
        },
        // ItemArmor: só as armaduras de couro (id 298..=301) respondem à camada 0.
        298..=301 => {
            if layer > 0 { WHITE } else { leather.unwrap_or(LEATHER_DEFAULT_COLOR) }
        }
        _ => {
            let key = (id.max(0) as u16, damage.max(0) as u16);
            // O corpo gerado só lista (id, dano) cujo código original devolve cor diferente de branco.
            let hit = ITEM_COLORS.binary_search_by(|(i, m, _)| (*i, *m).cmp(&key)).ok();
            match hit {
                Some(i) => ITEM_COLORS[i].2.get(layer as usize).copied().unwrap_or(WHITE),
                None => WHITE,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modelos_comuns() {
        assert_eq!(model_name(345, 0), Some("compass"));
        assert_eq!(model_name(35, 14), Some("red_wool"));
        assert_eq!(model_name(373, 0), Some("bottle_drinkable"));
        assert_eq!(model_name(373, 16384 | 5), Some("bottle_splash"));
        assert_eq!(model_name(383, 50), Some("spawn_egg"));
        assert_eq!(model_name(298, 0), Some("leather_helmet"));
        assert_eq!(model_name(267, 120), Some("iron_sword"), "dano de uso não muda o modelo");
        assert_eq!(model_name(2, 0), Some("grass"));
        assert_eq!(model_name(261, 0), Some("bow"));
        assert_eq!(model_name(351, 4), Some("dye_blue"));
    }

    #[test]
    fn cores_por_camada() {
        // Valores calculados pelo código original (PotionHelper/EntityList/ItemArmor).
        assert_eq!(layer_color(373, 1, 0, None), 0xCD5CAB);
        assert_eq!(layer_color(373, 1, 1, None), 0xFFFFFF);
        assert_eq!(layer_color(383, 50, 0, None), 0x0DA70B);
        assert_eq!(layer_color(383, 50, 1, None), 0x000000);
        assert_eq!(layer_color(298, 0, 0, None), 10511680);
        assert_eq!(layer_color(298, 0, 0, Some(0x112233)), 0x112233);
        assert_eq!(layer_color(298, 0, 1, Some(0x112233)), 0xFFFFFF);
        assert_eq!(layer_color(267, 0, 0, None), 0xFFFFFF);
    }
}
