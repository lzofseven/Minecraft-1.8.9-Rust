//! GERADO por scripts/gen_block_physics.py a partir do código decompilado da 1.8.9.
//! Não editar à mão.

#![allow(dead_code)]

/// Flags de `Material`: líquido, sólido, bloqueia movimento, opaco.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Material {
    pub name: &'static str,
    pub liquid: bool,
    pub solid: bool,
    pub blocks_movement: bool,
    pub opaque: bool,
}

pub const MATERIALS: [Material; 35] = [
    Material { name: "air", liquid: false, solid: false, blocks_movement: false, opaque: false },
    Material { name: "grass", liquid: false, solid: true, blocks_movement: true, opaque: true },
    Material { name: "ground", liquid: false, solid: true, blocks_movement: true, opaque: true },
    Material { name: "wood", liquid: false, solid: true, blocks_movement: true, opaque: true },
    Material { name: "rock", liquid: false, solid: true, blocks_movement: true, opaque: true },
    Material { name: "iron", liquid: false, solid: true, blocks_movement: true, opaque: true },
    Material { name: "anvil", liquid: false, solid: true, blocks_movement: true, opaque: true },
    Material { name: "water", liquid: true, solid: false, blocks_movement: false, opaque: false },
    Material { name: "lava", liquid: true, solid: false, blocks_movement: false, opaque: false },
    Material { name: "leaves", liquid: false, solid: true, blocks_movement: true, opaque: false },
    Material { name: "plants", liquid: false, solid: false, blocks_movement: false, opaque: false },
    Material { name: "vine", liquid: false, solid: false, blocks_movement: false, opaque: false },
    Material { name: "sponge", liquid: false, solid: true, blocks_movement: true, opaque: true },
    Material { name: "cloth", liquid: false, solid: true, blocks_movement: true, opaque: true },
    Material { name: "fire", liquid: false, solid: false, blocks_movement: false, opaque: false },
    Material { name: "sand", liquid: false, solid: true, blocks_movement: true, opaque: true },
    Material { name: "circuits", liquid: false, solid: false, blocks_movement: false, opaque: false },
    Material { name: "carpet", liquid: false, solid: false, blocks_movement: false, opaque: false },
    Material { name: "glass", liquid: false, solid: true, blocks_movement: true, opaque: false },
    Material { name: "redstoneLight", liquid: false, solid: true, blocks_movement: true, opaque: true },
    Material { name: "tnt", liquid: false, solid: true, blocks_movement: true, opaque: false },
    Material { name: "coral", liquid: false, solid: true, blocks_movement: true, opaque: true },
    Material { name: "ice", liquid: false, solid: true, blocks_movement: true, opaque: false },
    Material { name: "packedIce", liquid: false, solid: true, blocks_movement: true, opaque: true },
    Material { name: "snow", liquid: false, solid: false, blocks_movement: false, opaque: false },
    Material { name: "craftedSnow", liquid: false, solid: true, blocks_movement: true, opaque: true },
    Material { name: "cactus", liquid: false, solid: true, blocks_movement: true, opaque: false },
    Material { name: "clay", liquid: false, solid: true, blocks_movement: true, opaque: true },
    Material { name: "gourd", liquid: false, solid: true, blocks_movement: true, opaque: true },
    Material { name: "dragonEgg", liquid: false, solid: true, blocks_movement: true, opaque: true },
    Material { name: "portal", liquid: false, solid: false, blocks_movement: false, opaque: false },
    Material { name: "cake", liquid: false, solid: true, blocks_movement: true, opaque: true },
    Material { name: "web", liquid: false, solid: true, blocks_movement: false, opaque: false },
    Material { name: "piston", liquid: false, solid: true, blocks_movement: true, opaque: true },
    Material { name: "barrier", liquid: false, solid: true, blocks_movement: true, opaque: true },
];

pub const M_AIR: u8 = 0;
pub const M_GRASS: u8 = 1;
pub const M_GROUND: u8 = 2;
pub const M_WOOD: u8 = 3;
pub const M_ROCK: u8 = 4;
pub const M_IRON: u8 = 5;
pub const M_ANVIL: u8 = 6;
pub const M_WATER: u8 = 7;
pub const M_LAVA: u8 = 8;
pub const M_LEAVES: u8 = 9;
pub const M_PLANTS: u8 = 10;
pub const M_VINE: u8 = 11;
pub const M_SPONGE: u8 = 12;
pub const M_CLOTH: u8 = 13;
pub const M_FIRE: u8 = 14;
pub const M_SAND: u8 = 15;
pub const M_CIRCUITS: u8 = 16;
pub const M_CARPET: u8 = 17;
pub const M_GLASS: u8 = 18;
pub const M_REDSTONELIGHT: u8 = 19;
pub const M_TNT: u8 = 20;
pub const M_CORAL: u8 = 21;
pub const M_ICE: u8 = 22;
pub const M_PACKEDICE: u8 = 23;
pub const M_SNOW: u8 = 24;
pub const M_CRAFTEDSNOW: u8 = 25;
pub const M_CACTUS: u8 = 26;
pub const M_CLAY: u8 = 27;
pub const M_GOURD: u8 = 28;
pub const M_DRAGONEGG: u8 = 29;
pub const M_PORTAL: u8 = 30;
pub const M_CAKE: u8 = 31;
pub const M_WEB: u8 = 32;
pub const M_PISTON: u8 = 33;
pub const M_BARRIER: u8 = 34;

/// Por id de bloco: (material, `slipperiness`, `isNormalCube`).
pub const BLOCKS: [(u8, f32, bool); 256] = [
    (0, 0.6, false), // 0: BlockAir (air)
    (4, 0.6, true), // 1: BlockStone (rock)
    (1, 0.6, true), // 2: BlockGrass (grass)
    (2, 0.6, true), // 3: BlockDirt (ground)
    (4, 0.6, true), // 4: Block (rock)
    (3, 0.6, true), // 5: BlockPlanks (wood)
    (10, 0.6, false), // 6: BlockSapling (plants)
    (4, 0.6, true), // 7: Block (rock)
    (7, 0.6, false), // 8: BlockDynamicLiquid (water)
    (7, 0.6, false), // 9: BlockStaticLiquid (water)
    (8, 0.6, false), // 10: BlockDynamicLiquid (lava)
    (8, 0.6, false), // 11: BlockStaticLiquid (lava)
    (15, 0.6, true), // 12: BlockSand (sand)
    (15, 0.6, true), // 13: BlockGravel (sand)
    (4, 0.6, true), // 14: BlockOre (rock)
    (4, 0.6, true), // 15: BlockOre (rock)
    (4, 0.6, true), // 16: BlockOre (rock)
    (3, 0.6, true), // 17: BlockOldLog (wood)
    (9, 0.6, false), // 18: BlockOldLeaf (leaves)
    (12, 0.6, true), // 19: BlockSponge (sponge)
    (18, 0.6, false), // 20: BlockGlass (glass)
    (4, 0.6, true), // 21: BlockOre (rock)
    (5, 0.6, true), // 22: Block (iron)
    (4, 0.6, true), // 23: BlockDispenser (rock)
    (4, 0.6, true), // 24: BlockSandStone (rock)
    (3, 0.6, true), // 25: BlockNote (wood)
    (13, 0.6, false), // 26: BlockBed (cloth)
    (16, 0.6, false), // 27: BlockRailPowered (circuits)
    (16, 0.6, false), // 28: BlockRailDetector (circuits)
    (33, 0.6, false), // 29: BlockPistonBase (piston)
    (32, 0.6, false), // 30: BlockWeb (web)
    (11, 0.6, false), // 31: BlockTallGrass (vine)
    (11, 0.6, false), // 32: BlockDeadBush (vine)
    (33, 0.6, false), // 33: BlockPistonBase (piston)
    (33, 0.6, false), // 34: BlockPistonExtension (piston)
    (13, 0.6, true), // 35: BlockColored (cloth)
    (33, 0.6, false), // 36: BlockPistonMoving (piston)
    (10, 0.6, false), // 37: BlockYellowFlower (plants)
    (10, 0.6, false), // 38: BlockRedFlower (plants)
    (10, 0.6, false), // 39: BlockMushroom (plants)
    (10, 0.6, false), // 40: BlockMushroom (plants)
    (5, 0.6, true), // 41: Block (iron)
    (5, 0.6, true), // 42: Block (iron)
    (4, 0.6, true), // 43: BlockDoubleStoneSlab (rock)
    (4, 0.6, false), // 44: BlockHalfStoneSlab (rock)
    (4, 0.6, true), // 45: Block (rock)
    (20, 0.6, false), // 46: BlockTNT (tnt)
    (3, 0.6, true), // 47: BlockBookshelf (wood)
    (4, 0.6, true), // 48: Block (rock)
    (4, 0.6, true), // 49: BlockObsidian (rock)
    (16, 0.6, false), // 50: BlockTorch (circuits)
    (14, 0.6, false), // 51: BlockFire (fire)
    (4, 0.6, true), // 52: BlockMobSpawner (rock)
    (3, 0.6, false), // 53: BlockStairs (wood)
    (3, 0.6, false), // 54: BlockChest (wood)
    (16, 0.6, false), // 55: BlockRedstoneWire (circuits)
    (4, 0.6, true), // 56: BlockOre (rock)
    (5, 0.6, true), // 57: Block (iron)
    (3, 0.6, true), // 58: BlockWorkbench (wood)
    (10, 0.6, false), // 59: BlockCrops (plants)
    (2, 0.6, false), // 60: BlockFarmland (ground)
    (4, 0.6, true), // 61: BlockFurnace (rock)
    (4, 0.6, true), // 62: BlockFurnace (rock)
    (3, 0.6, false), // 63: BlockStandingSign (wood)
    (3, 0.6, false), // 64: BlockDoor (wood)
    (16, 0.6, false), // 65: BlockLadder (circuits)
    (16, 0.6, false), // 66: BlockRail (circuits)
    (4, 0.6, false), // 67: BlockStairs (rock)
    (3, 0.6, false), // 68: BlockWallSign (wood)
    (16, 0.6, false), // 69: BlockLever (circuits)
    (4, 0.6, false), // 70: BlockPressurePlate (rock)
    (5, 0.6, false), // 71: BlockDoor (iron)
    (3, 0.6, false), // 72: BlockPressurePlate (wood)
    (4, 0.6, true), // 73: BlockRedstoneOre (rock)
    (4, 0.6, true), // 74: BlockRedstoneOre (rock)
    (16, 0.6, false), // 75: BlockRedstoneTorch (circuits)
    (16, 0.6, false), // 76: BlockRedstoneTorch (circuits)
    (16, 0.6, false), // 77: BlockButtonStone (circuits)
    (24, 0.6, false), // 78: BlockSnow (snow)
    (22, 0.98, false), // 79: BlockIce (ice)
    (25, 0.6, true), // 80: BlockSnowBlock (craftedSnow)
    (26, 0.6, false), // 81: BlockCactus (cactus)
    (27, 0.6, true), // 82: BlockClay (clay)
    (10, 0.6, false), // 83: BlockReed (plants)
    (3, 0.6, true), // 84: BlockJukebox (wood)
    (3, 0.6, false), // 85: BlockFence (wood)
    (28, 0.6, true), // 86: BlockPumpkin (gourd)
    (4, 0.6, true), // 87: BlockNetherrack (rock)
    (15, 0.6, true), // 88: BlockSoulSand (sand)
    (18, 0.6, false), // 89: BlockGlowstone (glass)
    (30, 0.6, false), // 90: BlockPortal (portal)
    (28, 0.6, true), // 91: BlockPumpkin (gourd)
    (31, 0.6, false), // 92: BlockCake (cake)
    (16, 0.6, false), // 93: BlockRedstoneRepeater (circuits)
    (16, 0.6, false), // 94: BlockRedstoneRepeater (circuits)
    (18, 0.6, false), // 95: BlockStainedGlass (glass)
    (3, 0.6, false), // 96: BlockTrapDoor (wood)
    (27, 0.6, true), // 97: BlockSilverfish (clay)
    (4, 0.6, true), // 98: BlockStoneBrick (rock)
    (3, 0.6, true), // 99: BlockHugeMushroom (wood)
    (3, 0.6, true), // 100: BlockHugeMushroom (wood)
    (5, 0.6, false), // 101: BlockPane (iron)
    (18, 0.6, false), // 102: BlockPane (glass)
    (28, 0.6, true), // 103: BlockMelon (gourd)
    (28, 0.6, false), // 104: BlockStem (gourd)
    (28, 0.6, false), // 105: BlockStem (gourd)
    (11, 0.6, false), // 106: BlockVine (vine)
    (3, 0.6, false), // 107: BlockFenceGate (wood)
    (4, 0.6, false), // 108: BlockStairs (rock)
    (4, 0.6, false), // 109: BlockStairs (rock)
    (1, 0.6, true), // 110: BlockMycelium (grass)
    (10, 0.6, false), // 111: BlockLilyPad (plants)
    (4, 0.6, true), // 112: BlockNetherBrick (rock)
    (4, 0.6, false), // 113: BlockFence (rock)
    (4, 0.6, false), // 114: BlockStairs (rock)
    (10, 0.6, false), // 115: BlockNetherWart (plants)
    (4, 0.6, false), // 116: BlockEnchantmentTable (rock)
    (5, 0.6, false), // 117: BlockBrewingStand (iron)
    (5, 0.6, false), // 118: BlockCauldron (iron)
    (30, 0.6, false), // 119: BlockEndPortal (portal)
    (4, 0.6, true), // 120: BlockEndPortalFrame (rock)
    (4, 0.6, true), // 121: Block (rock)
    (29, 0.6, false), // 122: BlockDragonEgg (dragonEgg)
    (19, 0.6, true), // 123: BlockRedstoneLight (redstoneLight)
    (19, 0.6, true), // 124: BlockRedstoneLight (redstoneLight)
    (3, 0.6, true), // 125: BlockDoubleWoodSlab (wood)
    (3, 0.6, false), // 126: BlockHalfWoodSlab (wood)
    (10, 0.6, false), // 127: BlockCocoa (plants)
    (4, 0.6, false), // 128: BlockStairs (rock)
    (4, 0.6, true), // 129: BlockOre (rock)
    (4, 0.6, false), // 130: BlockEnderChest (rock)
    (16, 0.6, false), // 131: BlockTripWireHook (circuits)
    (16, 0.6, false), // 132: BlockTripWire (circuits)
    (5, 0.6, true), // 133: Block (iron)
    (3, 0.6, false), // 134: BlockStairs (wood)
    (3, 0.6, false), // 135: BlockStairs (wood)
    (3, 0.6, false), // 136: BlockStairs (wood)
    (5, 0.6, true), // 137: BlockCommandBlock (iron)
    (18, 0.6, false), // 138: BlockBeacon (glass)
    (4, 0.6, false), // 139: BlockWall (rock)
    (16, 0.6, false), // 140: BlockFlowerPot (circuits)
    (10, 0.6, false), // 141: BlockCarrot (plants)
    (10, 0.6, false), // 142: BlockPotato (plants)
    (16, 0.6, false), // 143: BlockButtonWood (circuits)
    (16, 0.6, false), // 144: BlockSkull (circuits)
    (6, 0.6, false), // 145: BlockAnvil (anvil)
    (3, 0.6, false), // 146: BlockChest (wood)
    (5, 0.6, false), // 147: BlockPressurePlateWeighted (iron)
    (5, 0.6, false), // 148: BlockPressurePlateWeighted (iron)
    (16, 0.6, false), // 149: BlockRedstoneComparator (circuits)
    (16, 0.6, false), // 150: BlockRedstoneComparator (circuits)
    (3, 0.6, false), // 151: BlockDaylightDetector (wood)
    (5, 0.6, false), // 152: BlockCompressedPowered (iron)
    (4, 0.6, true), // 153: BlockOre (rock)
    (5, 0.6, false), // 154: BlockHopper (iron)
    (4, 0.6, true), // 155: BlockQuartz (rock)
    (4, 0.6, false), // 156: BlockStairs (rock)
    (16, 0.6, false), // 157: BlockRailPowered (circuits)
    (4, 0.6, true), // 158: BlockDropper (rock)
    (4, 0.6, true), // 159: BlockColored (rock)
    (18, 0.6, false), // 160: BlockStainedGlassPane (glass)
    (9, 0.6, false), // 161: BlockNewLeaf (leaves)
    (3, 0.6, true), // 162: BlockNewLog (wood)
    (3, 0.6, false), // 163: BlockStairs (wood)
    (3, 0.6, false), // 164: BlockStairs (wood)
    (27, 0.8, true), // 165: BlockSlime (clay)
    (34, 0.6, true), // 166: BlockBarrier (barrier)
    (5, 0.6, false), // 167: BlockTrapDoor (iron)
    (4, 0.6, true), // 168: BlockPrismarine (rock)
    (18, 0.6, false), // 169: BlockSeaLantern (glass)
    (1, 0.6, true), // 170: BlockHay (grass)
    (17, 0.6, false), // 171: BlockCarpet (carpet)
    (4, 0.6, true), // 172: BlockHardenedClay (rock)
    (4, 0.6, true), // 173: Block (rock)
    (23, 0.98, true), // 174: BlockPackedIce (packedIce)
    (11, 0.6, false), // 175: BlockDoublePlant (vine)
    (3, 0.6, false), // 176: BlockBanner (wood)
    (3, 0.6, false), // 177: BlockBanner (wood)
    (3, 0.6, false), // 178: BlockDaylightDetector (wood)
    (4, 0.6, true), // 179: BlockRedSandstone (rock)
    (4, 0.6, false), // 180: BlockStairs (rock)
    (4, 0.6, true), // 181: BlockDoubleStoneSlabNew (rock)
    (4, 0.6, false), // 182: BlockHalfStoneSlabNew (rock)
    (3, 0.6, false), // 183: BlockFenceGate (wood)
    (3, 0.6, false), // 184: BlockFenceGate (wood)
    (3, 0.6, false), // 185: BlockFenceGate (wood)
    (3, 0.6, false), // 186: BlockFenceGate (wood)
    (3, 0.6, false), // 187: BlockFenceGate (wood)
    (3, 0.6, false), // 188: BlockFence (wood)
    (3, 0.6, false), // 189: BlockFence (wood)
    (3, 0.6, false), // 190: BlockFence (wood)
    (3, 0.6, false), // 191: BlockFence (wood)
    (3, 0.6, false), // 192: BlockFence (wood)
    (3, 0.6, false), // 193: BlockDoor (wood)
    (3, 0.6, false), // 194: BlockDoor (wood)
    (3, 0.6, false), // 195: BlockDoor (wood)
    (3, 0.6, false), // 196: BlockDoor (wood)
    (3, 0.6, false), // 197: BlockDoor (wood)
    (0, 0.6, false), // 198: sem bloco
    (0, 0.6, false), // 199: sem bloco
    (0, 0.6, false), // 200: sem bloco
    (0, 0.6, false), // 201: sem bloco
    (0, 0.6, false), // 202: sem bloco
    (0, 0.6, false), // 203: sem bloco
    (0, 0.6, false), // 204: sem bloco
    (0, 0.6, false), // 205: sem bloco
    (0, 0.6, false), // 206: sem bloco
    (0, 0.6, false), // 207: sem bloco
    (0, 0.6, false), // 208: sem bloco
    (0, 0.6, false), // 209: sem bloco
    (0, 0.6, false), // 210: sem bloco
    (0, 0.6, false), // 211: sem bloco
    (0, 0.6, false), // 212: sem bloco
    (0, 0.6, false), // 213: sem bloco
    (0, 0.6, false), // 214: sem bloco
    (0, 0.6, false), // 215: sem bloco
    (0, 0.6, false), // 216: sem bloco
    (0, 0.6, false), // 217: sem bloco
    (0, 0.6, false), // 218: sem bloco
    (0, 0.6, false), // 219: sem bloco
    (0, 0.6, false), // 220: sem bloco
    (0, 0.6, false), // 221: sem bloco
    (0, 0.6, false), // 222: sem bloco
    (0, 0.6, false), // 223: sem bloco
    (0, 0.6, false), // 224: sem bloco
    (0, 0.6, false), // 225: sem bloco
    (0, 0.6, false), // 226: sem bloco
    (0, 0.6, false), // 227: sem bloco
    (0, 0.6, false), // 228: sem bloco
    (0, 0.6, false), // 229: sem bloco
    (0, 0.6, false), // 230: sem bloco
    (0, 0.6, false), // 231: sem bloco
    (0, 0.6, false), // 232: sem bloco
    (0, 0.6, false), // 233: sem bloco
    (0, 0.6, false), // 234: sem bloco
    (0, 0.6, false), // 235: sem bloco
    (0, 0.6, false), // 236: sem bloco
    (0, 0.6, false), // 237: sem bloco
    (0, 0.6, false), // 238: sem bloco
    (0, 0.6, false), // 239: sem bloco
    (0, 0.6, false), // 240: sem bloco
    (0, 0.6, false), // 241: sem bloco
    (0, 0.6, false), // 242: sem bloco
    (0, 0.6, false), // 243: sem bloco
    (0, 0.6, false), // 244: sem bloco
    (0, 0.6, false), // 245: sem bloco
    (0, 0.6, false), // 246: sem bloco
    (0, 0.6, false), // 247: sem bloco
    (0, 0.6, false), // 248: sem bloco
    (0, 0.6, false), // 249: sem bloco
    (0, 0.6, false), // 250: sem bloco
    (0, 0.6, false), // 251: sem bloco
    (0, 0.6, false), // 252: sem bloco
    (0, 0.6, false), // 253: sem bloco
    (0, 0.6, false), // 254: sem bloco
    (0, 0.6, false), // 255: sem bloco
];

/// Ids com `getRenderType() == 2`.
pub const RENDER_TYPE_2: &[u16] = &[54, 130, 146];
