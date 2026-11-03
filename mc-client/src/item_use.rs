//! GERADO por scripts/gen_item_use.py a partir do código decompilado da 1.8.9. Não editar.

use crate::physics::UseAction;

/// (id, ação, duração em ticks, `alwaysEdible`).
pub const ITEM_USE: &[(i16, UseAction, i32, bool)] = &[
    (260, UseAction::Eat, 32, false), // apple (ItemFood)
    (261, UseAction::Bow, 72000, false), // bow (ItemBow)
    (267, UseAction::Block, 72000, false), // iron_sword (ItemSword)
    (268, UseAction::Block, 72000, false), // wooden_sword (ItemSword)
    (272, UseAction::Block, 72000, false), // stone_sword (ItemSword)
    (276, UseAction::Block, 72000, false), // diamond_sword (ItemSword)
    (282, UseAction::Eat, 32, false), // mushroom_stew (ItemSoup)
    (283, UseAction::Block, 72000, false), // golden_sword (ItemSword)
    (297, UseAction::Eat, 32, false), // bread (ItemFood)
    (319, UseAction::Eat, 32, false), // porkchop (ItemFood)
    (320, UseAction::Eat, 32, false), // cooked_porkchop (ItemFood)
    (322, UseAction::Eat, 32, true), // golden_apple (ItemAppleGold)
    (335, UseAction::Drink, 32, false), // milk_bucket (ItemBucketMilk)
    (349, UseAction::Eat, 32, false), // fish (ItemFishFood)
    (350, UseAction::Eat, 32, false), // cooked_fish (ItemFishFood)
    (357, UseAction::Eat, 32, false), // cookie (ItemFood)
    (360, UseAction::Eat, 32, false), // melon (ItemFood)
    (363, UseAction::Eat, 32, false), // beef (ItemFood)
    (364, UseAction::Eat, 32, false), // cooked_beef (ItemFood)
    (365, UseAction::Eat, 32, false), // chicken (ItemFood)
    (366, UseAction::Eat, 32, false), // cooked_chicken (ItemFood)
    (367, UseAction::Eat, 32, false), // rotten_flesh (ItemFood)
    (373, UseAction::Drink, 32, false), // potion (ItemPotion)
    (375, UseAction::Eat, 32, false), // spider_eye (ItemFood)
    (391, UseAction::Eat, 32, false), // carrot (ItemSeedFood)
    (392, UseAction::Eat, 32, false), // potato (ItemSeedFood)
    (393, UseAction::Eat, 32, false), // baked_potato (ItemFood)
    (394, UseAction::Eat, 32, false), // poisonous_potato (ItemFood)
    (396, UseAction::Eat, 32, false), // golden_carrot (ItemFood)
    (400, UseAction::Eat, 32, false), // pumpkin_pie (ItemFood)
    (411, UseAction::Eat, 32, false), // rabbit (ItemFood)
    (412, UseAction::Eat, 32, false), // cooked_rabbit (ItemFood)
    (413, UseAction::Eat, 32, false), // rabbit_stew (ItemSoup)
    (423, UseAction::Eat, 32, false), // mutton (ItemFood)
    (424, UseAction::Eat, 32, false), // cooked_mutton (ItemFood)
];

/// (id, nome do material em models/armor, armorType: 0 capacete, 1 peito, 2 calça, 3 botas).
pub const ARMOR: &[(i16, &str, u8)] = &[
    (298, "leather", 0),
    (299, "leather", 1),
    (300, "leather", 2),
    (301, "leather", 3),
    (302, "chainmail", 0),
    (303, "chainmail", 1),
    (304, "chainmail", 2),
    (305, "chainmail", 3),
    (306, "iron", 0),
    (307, "iron", 1),
    (308, "iron", 2),
    (309, "iron", 3),
    (310, "diamond", 0),
    (311, "diamond", 1),
    (312, "diamond", 2),
    (313, "diamond", 3),
    (314, "gold", 0),
    (315, "gold", 1),
    (316, "gold", 2),
    (317, "gold", 3),
];

/// `ItemArmor.getColor` do couro sem tinta.
pub const LEATHER_DEFAULT_COLOR: u32 = 10511680;
