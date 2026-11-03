//! Modelo dos containers da 1.8.9 (`net.minecraft.inventory.Container*`, `Slot*`): layout, validade e limite
//! de cada slot, `mergeItemStack`, `transferStackInSlot` e `slotClick` (modos 0 a 6) como no original.
//! Os números do layout e as tabelas de item vêm de `gen_containers.rs` e `gen_container_items.rs`.

use mc_protocol::nbt::Slot;

use crate::gen_container_items::{FUEL, ITEM_INFO, SMELTABLE};
use crate::gen_containers as g;
use crate::gen_containers::Kind;

// ---------------------------------------------------------------- itens

fn info(id: i16) -> Option<&'static (u16, u8, u16, bool, u8, bool)> {
    ITEM_INFO.binary_search_by_key(&(id as u16), |t| t.0).ok().map(|i| &ITEM_INFO[i])
}

/// `ItemStack.getMaxStackSize`.
pub fn max_stack(s: &Slot) -> u8 {
    info(s.id).map_or(64, |i| i.1)
}

pub fn has_subtypes(id: i16) -> bool {
    info(id).is_some_and(|i| i.3)
}

/// `ItemStack.getMetadata` (só os itens com subtipos usam o dano).
/// `ItemStack.getMetadata` (Forge: `Item.getMetadata(stack)` = o dano, com ou sem subtipos).
pub fn metadata(s: &Slot) -> i16 {
    s.damage
}

/// `ItemStack.isStackable`.
pub fn is_stackable(s: &Slot) -> bool {
    let damageable = info(s.id).is_some_and(|i| i.2 > 0);
    max_stack(s) > 1 && (!damageable || s.damage <= 0)
}

/// `Block.getBlockFromName`: nome de registro (domínio "minecraft" se faltar) ou id numérico.
pub fn block_from_name(name: &str) -> Option<u16> {
    let n = name.trim().to_lowercase();
    let full = if n.contains(':') { n.clone() } else { format!("minecraft:{n}") };
    crate::gen_block_names::BLOCK_NAMES.iter().find(|(_, b)| *b == full).map(|(id, _)| *id).or_else(|| n.parse::<u16>().ok().filter(|id| crate::gen_block_names::BLOCK_NAMES.iter().any(|(i, _)| i == id)))
}

/// `ItemStack.canDestroy` / `canPlaceOn`: a lista `CanDestroy` ou `CanPlaceOn` do NBT tem o bloco.
pub fn tag_allows(s: &Slot, key: &str, block: u16) -> bool {
    use mc_protocol::nbt::Nbt;
    match s.nbt.as_ref().and_then(|n| n.get(key)) {
        Some(Nbt::List(list)) => list.iter().any(|e| matches!(e, Nbt::String(name) if block_from_name(name) == Some(block))),
        _ => false,
    }
}

/// `Item.getMaxDamage` (0 = sem durabilidade).
pub fn max_damage(id: i16) -> i32 {
    info(id).map_or(0, |i| i.2 as i32)
}

/// `ItemStack.isItemStackDamageable`: tem durabilidade e não é `Unbreakable`.
pub fn is_damageable(s: &Slot) -> bool {
    use mc_protocol::nbt::Nbt;
    let unbreakable = matches!(s.nbt.as_ref().and_then(|n| n.get("Unbreakable")), Some(Nbt::Byte(b)) if *b != 0);
    info(s.id).is_some_and(|i| i.2 > 0) && !unbreakable
}

/// `ItemStack.areItemStackTagsEqual`.
pub fn same_tags(a: &Slot, b: &Slot) -> bool {
    match (a.nbt.is_some(), b.nbt.is_some()) {
        (false, false) => true,
        (true, true) => a.raw_nbt == b.raw_nbt,
        _ => false,
    }
}

/// `ItemStack.isItemEqual` (item e dano).
pub fn item_equal(a: &Slot, b: &Slot) -> bool {
    a.id == b.id && a.damage == b.damage
}

/// `TileEntityFurnace.getItemBurnTime`.
pub fn burn_time(s: &Slot) -> u16 {
    FUEL.iter().find(|(id, d, _)| *id as i16 == s.id && (*d < 0 || *d == s.damage)).map_or(0, |t| t.2)
}

pub fn is_fuel(s: &Slot) -> bool {
    burn_time(s) > 0
}

/// `FurnaceRecipes.getSmeltingResult(stack) != null`.
pub fn is_smeltable(s: &Slot) -> bool {
    SMELTABLE.iter().any(|(id, d)| *id as i16 == s.id && (*d < 0 || *d == s.damage))
}

pub fn is_bucket(s: &Slot) -> bool {
    s.id == 325
}

/// Pedaço de `n` itens do mesmo tipo (`ItemStack.splitStack` devolve isto e tira de `s`).
pub fn split(s: &mut Slot, n: u8) -> Slot {
    let mut out = s.clone();
    out.count = n;
    s.count -= n;
    out
}

// ---------------------------------------------------------------- slots

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Plain,
    /// Armadura do jogador (`ContainerPlayer`): tipo 0 capacete .. 3 botas.
    Armor(u8),
    Crafting,
    FurnaceFuel,
    FurnaceOutput,
    BrewPotion,
    BrewIngredient,
    Beacon,
    MerchantResult,
    EnchantItem,
    Lapis,
    AnvilOutput,
    Saddle,
    HorseArmor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WinKind {
    Player,
    Chest { rows: usize },
    Workbench,
    Furnace,
    Dispenser,
    Hopper,
    Enchant,
    Brewing,
    Anvil,
    Beacon,
    Merchant,
    Horse { chested: bool },
    Creative,
}

#[derive(Clone, Copy, Debug)]
pub struct SlotDef {
    pub x: i32,
    pub y: i32,
    pub role: Role,
    /// O slot pertence ao `InventoryPlayer` (Slot.inventory == inventoryplayer).
    pub player_inv: bool,
}

fn convert(defs: &[g::Def], kind: WinKind) -> Vec<SlotDef> {
    let n = defs.len();
    defs.iter()
        .enumerate()
        .map(|(i, (k, x, y, anon))| {
            let role = match (kind, *k, *anon) {
                (_, Kind::Crafting, _) => Role::Crafting,
                (_, Kind::FurnaceFuel, _) => Role::FurnaceFuel,
                (_, Kind::FurnaceOutput, _) => Role::FurnaceOutput,
                (_, Kind::BrewPotion, _) => Role::BrewPotion,
                (_, Kind::BrewIngredient, _) => Role::BrewIngredient,
                (_, Kind::Beacon, _) => Role::Beacon,
                (_, Kind::MerchantResult, _) => Role::MerchantResult,
                // Classes anônimas: ContainerPlayer (armadura), Enchantment, Repair, HorseInventory.
                (WinKind::Player, Kind::Plain, true) => Role::Armor(i as u8 - 5),
                (WinKind::Enchant, Kind::Plain, true) => {
                    if i == 0 { Role::EnchantItem } else { Role::Lapis }
                }
                (WinKind::Anvil, Kind::Plain, true) => Role::AnvilOutput,
                (WinKind::Horse { .. }, Kind::Plain, true) => {
                    if i == 0 { Role::Saddle } else { Role::HorseArmor }
                }
                _ => Role::Plain,
            };
            let player_inv = match kind {
                WinKind::Player => i >= 5,
                _ => i + 36 >= n,
            };
            SlotDef { x: *x, y: *y, role, player_inv }
        })
        .collect()
}

/// `GuiContainerCreative.ContainerCreative`: 5x9 da grade (`field_147060_v`) e a hotbar do jogador.
fn creative_defs() -> Vec<SlotDef> {
    let mut v = Vec::with_capacity(54);
    for i in 0..5 {
        for j in 0..9 {
            v.push(SlotDef { x: 9 + j * 18, y: 18 + i * 18, role: Role::Plain, player_inv: false });
        }
    }
    for k in 0..9 {
        v.push(SlotDef { x: 9 + k * 18, y: 112, role: Role::Plain, player_inv: true });
    }
    v
}

/// Layout dos slots (ordem das janelas do protocolo) de cada tipo de janela.
/// `Enchant`/`Anvil` têm só os slots do `Container`.
pub fn defs_for(kind: WinKind) -> Vec<SlotDef> {
    if kind == WinKind::Creative {
        return creative_defs();
    }
    let d: &[g::Def] = match kind {
        WinKind::Player => g::PLAYER,
        WinKind::Chest { rows } => g::CHEST_ROWS[rows.clamp(1, 6) - 1],
        WinKind::Workbench => g::WORKBENCH,
        WinKind::Furnace => g::FURNACE,
        WinKind::Dispenser => g::DISPENSER,
        WinKind::Hopper => g::HOPPER,
        WinKind::Enchant => g::ENCHANTMENT,
        WinKind::Brewing => g::BREWING_STAND,
        WinKind::Anvil => g::REPAIR,
        WinKind::Beacon => g::BEACON,
        WinKind::Merchant => g::MERCHANT,
        WinKind::Horse { chested: false } => g::HORSE_FALSE,
        WinKind::Horse { chested: true } => g::HORSE_TRUE,
        WinKind::Creative => &[],
    };
    convert(d, kind)
}

/// Quantidade de slots da janela (sem o inventário do jogador) segundo o protocolo.
pub fn container_size(kind: WinKind) -> usize {
    match kind {
        WinKind::Player => 9,
        _ => defs_for(kind).len().saturating_sub(36),
    }
}

// ---------------------------------------------------------------- troca com aldeão

/// `MerchantRecipe` (MC|TrList).
#[derive(Clone, Debug, PartialEq)]
pub struct Trade {
    pub buy1: Slot,
    pub buy2: Option<Slot>,
    pub sell: Slot,
    pub disabled: bool,
    pub uses: i32,
    pub max_uses: i32,
}

/// `MerchantRecipeList.func_181078_a`: mesmo item e, se o pedido tem NBT, o NBT da oferta confere.
fn trade_item_matches(a: &Slot, b: &Slot) -> bool {
    item_equal(a, b) && (b.nbt.is_none() || a.raw_nbt == b.raw_nbt)
}

/// `MerchantRecipeList.canRecipeBeUsed`: devolve o índice da troca aplicável.
pub fn can_recipe_be_used(trades: &[Trade], a: &Slot, b: Option<&Slot>, index: usize) -> Option<usize> {
    if index > 0 && index < trades.len() {
        let t = &trades[index];
        let bad = !trade_item_matches(a, &t.buy1)
            || ((b.is_some() || t.buy2.is_some())
                && (t.buy2.is_none() || !b.zip(t.buy2.as_ref()).is_some_and(|(b, s)| trade_item_matches(b, s))))
            || a.count < t.buy1.count
            || t.buy2.as_ref().is_some_and(|s2| b.is_none_or(|b| b.count < s2.count));
        return if bad { None } else { Some(index) };
    }
    trades.iter().position(|t| {
        trade_item_matches(a, &t.buy1)
            && a.count >= t.buy1.count
            && match (&t.buy2, b) {
                (None, None) => true,
                (Some(s2), Some(b)) => trade_item_matches(b, s2) && b.count >= s2.count,
                _ => false,
            }
    })
}

// ---------------------------------------------------------------- container

/// Quem clica: modo criativo (clone, arrastar de 64) e nível de xp (bigorna).
#[derive(Clone, Copy, Default)]
pub struct Env {
    pub creative: bool,
    pub level: i32,
}

#[derive(Default, Clone)]
pub struct Drag {
    pub event: i32,
    pub mode: i32,
    pub slots: Vec<usize>,
}

/// Estado do `Container` aberto, menos os itens (que ficam em `Vec<Option<Slot>>` à parte para o
/// inventário do jogador continuar acessível como `inv.player`).
#[derive(Clone)]
pub struct Container {
    pub kind: WinKind,
    pub defs: Vec<SlotDef>,
    pub drag: Drag,
    /// `Container.updateProgressBar` (S31): fornalha, poções, encantamento, bigorna e farol.
    pub props: [i32; 16],
    pub trades: Vec<Trade>,
    pub trade_index: usize,
    /// Cavalo que aceita armadura (`EntityHorse.canWearArmor`).
    pub horse_armor: bool,
    /// Contador de transações (`Container.getNextTransactionID`).
    pub tx: u16,
}

impl Container {
    pub fn new(kind: WinKind) -> Self {
        let mut props = [0; 16];
        if kind == WinKind::Enchant {
            props[4..7].fill(-1);
        }
        Self { kind, defs: defs_for(kind), drag: Drag::default(), props, trades: Vec::new(), trade_index: 0, horse_armor: false, tx: 0 }
    }

    pub fn len(&self) -> usize {
        self.defs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }

    fn role(&self, i: usize) -> Role {
        self.defs[i].role
    }

    /// Janela do jogador: slots da grade de criação (índices) para `SlotCrafting.onPickupFromSlot`.
    fn grid(&self) -> std::ops::Range<usize> {
        match self.kind {
            WinKind::Player => 1..5,
            WinKind::Workbench => 1..10,
            _ => 0..0,
        }
    }

    // --- Slot.*

    /// `Slot.isItemValid`.
    pub fn valid(&self, slots: &[Option<Slot>], i: usize, s: &Slot) -> bool {
        match self.role(i) {
            Role::Plain | Role::EnchantItem => true,
            Role::Armor(k) => info(s.id).is_some_and(|t| t.4 & (1 << k) != 0),
            Role::Crafting | Role::FurnaceOutput | Role::AnvilOutput | Role::MerchantResult => false,
            Role::FurnaceFuel => is_fuel(s) || is_bucket(s),
            Role::Lapis => s.id == 351 && s.damage == 4,
            Role::BrewPotion => s.id == 373 || s.id == 374,
            Role::BrewIngredient => info(s.id).is_some_and(|t| t.5),
            Role::Beacon => matches!(s.id, 388 | 264 | 266 | 265),
            Role::Saddle => s.id == 329 && slots[i].is_none(),
            Role::HorseArmor => self.horse_armor && matches!(s.id, 417..=419),
        }
    }

    /// `Slot.getItemStackLimit`.
    pub fn limit(&self, i: usize, s: &Slot) -> u8 {
        match self.role(i) {
            Role::Armor(_) | Role::EnchantItem | Role::BrewPotion | Role::Beacon => 1,
            Role::FurnaceFuel if is_bucket(s) => 1,
            _ => 64,
        }
    }

    /// `Slot.canTakeStack`.
    pub fn can_take(&self, slots: &[Option<Slot>], i: usize, env: Env) -> bool {
        match self.role(i) {
            Role::AnvilOutput => {
                let cost = self.props[0];
                (env.creative || env.level >= cost) && cost > 0 && slots[i].is_some()
            }
            _ => true,
        }
    }

    /// `Container.canMergeSlot` (ContainerPlayer/Workbench/Repair; o criativo só na hotbar, y > 90).
    fn can_merge(&self, i: usize) -> bool {
        if self.kind == WinKind::Creative {
            return self.defs[i].y > 90;
        }
        !matches!(self.role(i), Role::Crafting)
    }

    /// `Container.canDragIntoSlot` (ContainerCreative: só slots do jogador).
    fn can_drag_into(&self, i: usize) -> bool {
        if self.kind == WinKind::Creative {
            let d = self.defs[i];
            return d.player_inv || (d.y > 90 && d.x <= 162);
        }
        true
    }

    /// `Slot.decrStackSize` + `putStack(null)` quando esvazia: tira até `n` itens do slot.
    fn decr(&self, slots: &mut [Option<Slot>], i: usize, n: u8) -> Option<Slot> {
        let s = slots[i].as_mut()?;
        if s.count <= n {
            slots[i].take()
        } else {
            Some(split(s, n))
        }
    }

    /// `Slot.onPickupFromSlot` (efeitos no cliente).
    fn on_pickup(&mut self, slots: &mut [Option<Slot>], i: usize, _picked: &Slot) {
        match self.role(i) {
            Role::Crafting => {
                // SlotCrafting.onPickupFromSlot: o que sobra (getRemainingItems) é calculado antes de gastar; volta
                // ao slot que ficou vazio (com o slot ainda ocupado o Java põe no inventário).
                let width = if self.kind == WinKind::Player { 2 } else { 3 };
                let g0 = self.grid().start;
                let rest = crate::recipes::remaining(&slots[self.grid()], width);
                for g in self.grid() {
                    if let Some(s) = slots[g].as_mut() {
                        s.count -= 1;
                        if s.count == 0 {
                            slots[g] = None;
                        }
                    }
                    if let (None, Some(r)) = (&slots[g], &rest[g - g0]) {
                        slots[g] = Some(r.clone());
                    }
                }
                // InventoryCrafting.decrStackSize -> onCraftMatrixChanged.
                self.craft_update(slots);
            }
            Role::AnvilOutput => {
                // `materialCost` vem do servidor (updateRepairOutput); sem ele, esvazia como se fosse 0.
                slots[0] = None;
                slots[1] = None;
            }
            Role::MerchantResult => {
                let Some(ti) = self.current_trade(slots) else { return };
                let t = self.trades[ti].clone();
                let (mut a, mut b) = (slots[0].take(), slots[1].take());
                let _ = do_trade(&t, &mut a, &mut b) || do_trade(&t, &mut b, &mut a);
                slots[0] = a.filter(|x| x.count > 0);
                slots[1] = b.filter(|x| x.count > 0);
            }
            _ => {}
        }
    }

    // --- InventoryMerchant

    fn current_trade(&self, slots: &[Option<Slot>]) -> Option<usize> {
        let (mut a, mut b) = (slots[0].as_ref(), slots[1].as_ref());
        if a.is_none() {
            a = b;
            b = None;
        }
        let a = a?;
        match can_recipe_be_used(&self.trades, a, b, self.trade_index) {
            Some(i) if !self.trades[i].disabled => Some(i),
            _ if b.is_some() => {
                let i = can_recipe_be_used(&self.trades, b?, Some(a), self.trade_index)?;
                (!self.trades[i].disabled).then_some(i)
            }
            _ => None,
        }
    }

    /// `onCraftMatrixChanged`: o resultado (slot 0) pela receita da grade.
    pub fn craft_update(&self, slots: &mut [Option<Slot>]) {
        let g = self.grid();
        if g.is_empty() || slots.len() < g.end {
            return;
        }
        let width = if self.kind == WinKind::Player { 2 } else { 3 };
        slots[0] = crate::recipes::find(&slots[g], width);
    }

    /// Mudou um slot da grade de criação (`InventoryCrafting.setInventorySlotContents`).
    pub fn grid_changed(&self, slots: &mut [Option<Slot>], idx: usize) {
        if self.grid().contains(&idx) {
            self.craft_update(slots);
        }
    }

    /// `InventoryMerchant.resetRecipeAndSlots`: recalcula o resultado (slot 2) depois de mudar a entrada.
    pub fn after_change(&mut self, slots: &mut [Option<Slot>]) {
        if self.kind == WinKind::Merchant && slots.len() > 2 && !self.trades.is_empty() {
            slots[2] = self.current_trade(slots).map(|i| self.trades[i].sell.clone());
        } else if self.kind == WinKind::Merchant && slots.len() > 2 && slots[0].is_none() && slots[1].is_none() {
            slots[2] = None;
        }
    }

    // --- Container.mergeItemStack

    pub fn merge(&self, slots: &mut [Option<Slot>], stack: &mut Slot, start: usize, end: usize, reverse: bool) -> bool {
        let mut flag = false;
        let mut i = if reverse { end as i32 - 1 } else { start as i32 };
        let in_range = |i: i32| if reverse { i >= start as i32 } else { i < end as i32 };
        if is_stackable(stack) {
            while stack.count > 0 && in_range(i) {
                let idx = i as usize;
                if let Some(s) = slots[idx].as_mut() {
                    if s.id == stack.id && (!has_subtypes(stack.id) || metadata(stack) == metadata(s)) && same_tags(stack, s) {
                        let j = s.count as i32 + stack.count as i32;
                        let max = max_stack(stack) as i32;
                        if j <= max {
                            stack.count = 0;
                            s.count = j as u8;
                            flag = true;
                        } else if (s.count as i32) < max {
                            stack.count -= (max - s.count as i32) as u8;
                            s.count = max as u8;
                            flag = true;
                        }
                    }
                }
                i += if reverse { -1 } else { 1 };
            }
        }
        if stack.count > 0 {
            i = if reverse { end as i32 - 1 } else { start as i32 };
            while in_range(i) {
                let idx = i as usize;
                if slots[idx].is_none() && self.valid(slots, idx, stack) {
                    slots[idx] = Some(stack.clone());
                    stack.count = 0;
                    flag = true;
                    break;
                }
                i += if reverse { -1 } else { 1 };
            }
        }
        flag
    }

    // --- transferStackInSlot de cada container

    /// `Container.transferStackInSlot`: devolve a cópia do que estava no slot (None = nada se moveu).
    pub fn transfer(&mut self, slots: &mut Vec<Option<Slot>>, i: usize) -> Option<Slot> {
        let mut stack = slots.get(i)?.clone()?;
        let original = stack.clone();
        let n = slots.len();
        let ok = match self.kind {
            WinKind::Player => {
                if i == 0 {
                    self.merge(slots, &mut stack, 9, 45, true)
                } else if (1..5).contains(&i) || (5..9).contains(&i) {
                    self.merge(slots, &mut stack, 9, 45, false)
                } else if let Some(k) = armor_type(&stack).filter(|k| slots[5 + *k as usize].is_none()) {
                    let at = 5 + k as usize;
                    self.merge(slots, &mut stack, at, at + 1, false)
                } else if (9..36).contains(&i) {
                    self.merge(slots, &mut stack, 36, 45, false)
                } else if (36..45).contains(&i) {
                    self.merge(slots, &mut stack, 9, 36, false)
                } else {
                    self.merge(slots, &mut stack, 9, 45, false)
                }
            }
            WinKind::Workbench => {
                if i == 0 {
                    self.merge(slots, &mut stack, 10, 46, true)
                } else if (10..37).contains(&i) {
                    self.merge(slots, &mut stack, 37, 46, false)
                } else if (37..46).contains(&i) {
                    self.merge(slots, &mut stack, 10, 37, false)
                } else {
                    self.merge(slots, &mut stack, 10, 46, false)
                }
            }
            WinKind::Furnace => {
                if i == 2 {
                    self.merge(slots, &mut stack, 3, 39, true)
                } else if i != 1 && i != 0 {
                    if is_smeltable(&stack) {
                        self.merge(slots, &mut stack, 0, 1, false)
                    } else if is_fuel(&stack) {
                        self.merge(slots, &mut stack, 1, 2, false)
                    } else if (3..30).contains(&i) {
                        self.merge(slots, &mut stack, 30, 39, false)
                    } else {
                        // `i >= 30 && i < 39 && !merge(...)`: fora dessa faixa o Java segue em frente como se desse certo.
                        !((30..39).contains(&i)) || self.merge(slots, &mut stack, 3, 30, false)
                    }
                } else {
                    self.merge(slots, &mut stack, 3, 39, false)
                }
            }
            WinKind::Dispenser => {
                if i < 9 {
                    self.merge(slots, &mut stack, 9, 45, true)
                } else {
                    self.merge(slots, &mut stack, 0, 9, false)
                }
            }
            WinKind::Hopper => {
                if i < 5 {
                    self.merge(slots, &mut stack, 5, n, true)
                } else {
                    self.merge(slots, &mut stack, 0, 5, false)
                }
            }
            WinKind::Chest { .. } => {
                let rows9 = n - 36;
                if i < rows9 {
                    self.merge(slots, &mut stack, rows9, n, true)
                } else {
                    self.merge(slots, &mut stack, 0, rows9, false)
                }
            }
            WinKind::Enchant => {
                if i == 0 || i == 1 {
                    self.merge(slots, &mut stack, 2, 38, true)
                } else if stack.id == 351 && stack.damage == 4 {
                    self.merge(slots, &mut stack, 1, 2, true)
                } else {
                    if slots[0].is_some() || !self.valid(slots, 0, &stack) {
                        return None;
                    }
                    if stack.nbt.is_some() && stack.count == 1 {
                        slots[0] = Some(stack.clone());
                        stack.count = 0;
                    } else if stack.count >= 1 {
                        slots[0] = Some(Slot { id: stack.id, count: 1, damage: metadata(&stack), nbt: None, raw_nbt: Vec::new() });
                        stack.count -= 1;
                    }
                    true
                }
            }
            WinKind::Brewing => {
                if !(i <= 2) && i != 3 {
                    if slots[3].is_none() && self.valid(slots, 3, &stack) {
                        self.merge(slots, &mut stack, 3, 4, false)
                    } else if stack.id == 373 || stack.id == 374 {
                        self.merge(slots, &mut stack, 0, 3, false)
                    } else if (4..31).contains(&i) {
                        self.merge(slots, &mut stack, 31, 40, false)
                    } else if (31..40).contains(&i) {
                        self.merge(slots, &mut stack, 4, 31, false)
                    } else {
                        self.merge(slots, &mut stack, 4, 40, false)
                    }
                } else {
                    self.merge(slots, &mut stack, 4, 40, true)
                }
            }
            WinKind::Anvil => {
                if i == 2 {
                    self.merge(slots, &mut stack, 3, 39, true)
                } else if i != 0 && i != 1 {
                    // `i >= 3 && i < 39 && !merge(0, 2)` -> falha só nessa faixa.
                    !((3..39).contains(&i)) || self.merge(slots, &mut stack, 0, 2, false)
                } else {
                    self.merge(slots, &mut stack, 3, 39, false)
                }
            }
            WinKind::Beacon => {
                if i == 0 {
                    self.merge(slots, &mut stack, 1, 37, true)
                } else if slots[0].is_none() && self.valid(slots, 0, &stack) && stack.count == 1 {
                    self.merge(slots, &mut stack, 0, 1, false)
                } else if (1..28).contains(&i) {
                    self.merge(slots, &mut stack, 28, 37, false)
                } else if (28..37).contains(&i) {
                    self.merge(slots, &mut stack, 1, 28, false)
                } else {
                    self.merge(slots, &mut stack, 1, 37, false)
                }
            }
            WinKind::Merchant => {
                if i == 2 {
                    self.merge(slots, &mut stack, 3, 39, true)
                } else if i != 0 && i != 1 {
                    if (3..30).contains(&i) {
                        self.merge(slots, &mut stack, 30, 39, false)
                    } else {
                        !((30..39).contains(&i)) || self.merge(slots, &mut stack, 3, 30, false)
                    }
                } else {
                    self.merge(slots, &mut stack, 3, 39, false)
                }
            }
            WinKind::Horse { .. } => {
                let hs = n - 36;
                if i < hs {
                    self.merge(slots, &mut stack, hs, n, true)
                } else if self.valid(slots, 1, &stack) && slots[1].is_none() {
                    self.merge(slots, &mut stack, 1, 2, false)
                } else if self.valid(slots, 0, &stack) {
                    self.merge(slots, &mut stack, 0, 1, false)
                } else {
                    hs > 2 && self.merge(slots, &mut stack, 2, hs, false)
                }
            }
            WinKind::Creative => {
                // ContainerCreative.transferStackInSlot: shift na hotbar apaga o slot e devolve null.
                if i + 9 >= n && i < n {
                    slots[i] = None;
                }
                return None;
            }
        };
        if !ok {
            return None;
        }
        if stack.count == 0 {
            slots[i] = None;
        } else {
            slots[i] = Some(stack.clone());
        }
        // Hopper, baú e cavalo não conferem `stackSize == original` nem chamam onPickupFromSlot.
        if !matches!(self.kind, WinKind::Hopper | WinKind::Chest { .. } | WinKind::Horse { .. }) {
            if stack.count == original.count {
                return None;
            }
            self.on_pickup(slots, i, &stack.clone());
        }
        Some(original)
    }

    // --- Container.slotClick

    /// `canAddItemToSlot`.
    fn can_add(&self, slots: &[Option<Slot>], i: Option<usize>, s: &Slot, ignore_size: bool) -> bool {
        let cur = i.and_then(|i| slots[i].as_ref());
        let mut flag = cur.is_none();
        if let Some(c) = cur {
            if item_equal(s, c) && same_tags(c, s) {
                flag |= c.count as i32 + if ignore_size { 0 } else { s.count as i32 } <= max_stack(s) as i32;
            }
        }
        flag
    }

    /// `Container.computeStackSize`.
    fn compute_size(&self, mode: i32, s: &mut Slot, existing: i32, n_slots: usize) {
        let mut c = s.count as i32;
        match mode {
            0 => c = (c as f32 / n_slots as f32).floor() as i32,
            1 => c = 1,
            2 => c = max_stack(s) as i32,
            _ => {}
        }
        s.count = (c + existing).clamp(0, 255) as u8;
    }

    fn reset_drag(&mut self) {
        self.drag.event = 0;
        self.drag.slots.clear();
    }

    /// `Container.slotClick(slot, button, mode)`; devolve o item que o Java manda no C0E.
    pub fn slot_click(&mut self, slots: &mut Vec<Option<Slot>>, cursor: &mut Option<Slot>, env: Env, slot: i32, button: i32, mode: i32) -> Option<Slot> {
        let before: Vec<Option<Slot>> = slots.get(self.grid()).map(<[_]>::to_vec).unwrap_or_default();
        let out = self.slot_click_inner(slots, cursor, env, slot, button, mode);
        if slots.get(self.grid()).is_some_and(|g| g != before.as_slice()) {
            self.craft_update(slots);
        }
        self.after_change(slots);
        out
    }

    fn slot_click_inner(&mut self, slots: &mut Vec<Option<Slot>>, cursor: &mut Option<Slot>, env: Env, slot: i32, button: i32, mode: i32) -> Option<Slot> {
        let mut result: Option<Slot> = None;
        let nslots = slots.len() as i32;
        if mode == 5 {
            let prev = self.drag.event;
            self.drag.event = button & 3;
            if (prev != 1 || self.drag.event != 2) && prev != self.drag.event {
                self.reset_drag();
            } else if cursor.is_none() {
                self.reset_drag();
            } else if self.drag.event == 0 {
                self.drag.mode = (button >> 2) & 3;
                if matches!(self.drag.mode, 0 | 1) || (self.drag.mode == 2 && env.creative) {
                    self.drag.event = 1;
                    self.drag.slots.clear();
                } else {
                    self.reset_drag();
                }
            } else if self.drag.event == 1 {
                if slot >= 0 && slot < nslots {
                    let i = slot as usize;
                    let c = cursor.clone().unwrap();
                    if self.can_add(slots, Some(i), &c, true) && self.valid(slots, i, &c) && c.count as usize > self.drag.slots.len() && self.can_drag_into(i) && !self.drag.slots.contains(&i) {
                        self.drag.slots.push(i);
                    }
                }
            } else if self.drag.event == 2 {
                if !self.drag.slots.is_empty() {
                    let c = cursor.clone().unwrap();
                    let mut left = c.count as i32;
                    let list = self.drag.slots.clone();
                    for &i in &list {
                        if self.can_add(slots, Some(i), &c, true) && self.valid(slots, i, &c) && c.count as usize >= list.len() && self.can_drag_into(i) {
                            let mut st = c.clone();
                            let k = slots[i].as_ref().map_or(0, |s| s.count as i32);
                            self.compute_size(self.drag.mode, &mut st, k, list.len());
                            if st.count > max_stack(&st) {
                                st.count = max_stack(&st);
                            }
                            let lim = self.limit(i, &st);
                            if st.count > lim {
                                st.count = lim;
                            }
                            left -= st.count as i32 - k;
                            slots[i] = Some(st);
                        }
                    }
                    let mut rest = c;
                    rest.count = left.max(0) as u8;
                    *cursor = (left > 0).then_some(rest);
                }
                self.reset_drag();
            } else {
                self.reset_drag();
            }
        } else if self.drag.event != 0 {
            self.reset_drag();
        } else if (mode == 0 || mode == 1) && (button == 0 || button == 1) {
            if slot == -999 {
                if let Some(c) = cursor.as_mut() {
                    if button == 0 {
                        *cursor = None;
                    } else {
                        split(c, 1);
                        if c.count == 0 {
                            *cursor = None;
                        }
                    }
                }
            } else if mode == 1 {
                if slot < 0 {
                    return None;
                }
                let i = slot as usize;
                if i < slots.len() && self.can_take(slots, i, env) {
                    if let Some(moved) = self.transfer(slots, i) {
                        result = Some(moved.clone());
                        // retrySlotClick: repete enquanto o slot ainda tem o mesmo item.
                        let mut guard = 0;
                        while slots[i].as_ref().is_some_and(|s| s.id == moved.id) && guard < 64 {
                            guard += 1;
                            if self.transfer(slots, i).is_none() {
                                break;
                            }
                        }
                    }
                }
            } else {
                if slot < 0 {
                    return None;
                }
                let i = slot as usize;
                if i >= slots.len() {
                    return None;
                }
                let in_slot = slots[i].clone();
                result = in_slot.clone();
                match (in_slot, cursor.clone()) {
                    (None, Some(mut c)) => {
                        if self.valid(slots, i, &c) {
                            let mut k = if button == 0 { c.count } else { 1 };
                            let lim = self.limit(i, &c);
                            if k > lim {
                                k = lim;
                            }
                            if c.count >= k {
                                slots[i] = Some(split(&mut c, k));
                            }
                            *cursor = (c.count > 0).then_some(c);
                        }
                    }
                    (Some(s), None) => {
                        if self.can_take(slots, i, env) {
                            let n = if button == 0 { s.count } else { (s.count + 1) / 2 };
                            let taken = self.decr(slots, i, n);
                            *cursor = taken.clone();
                            if let Some(t) = taken {
                                self.on_pickup(slots, i, &t);
                            }
                        }
                    }
                    (Some(s), Some(mut c)) => {
                        if self.can_take(slots, i, env) {
                            if self.valid(slots, i, &c) {
                                if s.id == c.id && metadata(&s) == metadata(&c) && same_tags(&s, &c) {
                                    let mut n = if button == 0 { c.count as i32 } else { 1 };
                                    let lim = self.limit(i, &c) as i32;
                                    n = n.min(lim - s.count as i32).min(max_stack(&c) as i32 - s.count as i32).max(0);
                                    c.count -= n as u8;
                                    slots[i].as_mut().unwrap().count += n as u8;
                                    *cursor = (c.count > 0).then_some(c);
                                } else if c.count <= self.limit(i, &c) {
                                    slots[i] = Some(c);
                                    *cursor = Some(s);
                                }
                            } else if s.id == c.id && max_stack(&c) > 1 && (!has_subtypes(s.id) || metadata(&s) == metadata(&c)) && same_tags(&s, &c) {
                                let l1 = s.count as i32;
                                if l1 > 0 && l1 + c.count as i32 <= max_stack(&c) as i32 {
                                    let mut c2 = c;
                                    c2.count += l1 as u8;
                                    let taken = self.decr(slots, i, l1 as u8);
                                    *cursor = Some(c2);
                                    if let Some(t) = taken {
                                        self.on_pickup(slots, i, &t);
                                    }
                                }
                            }
                        }
                    }
                    (None, None) => {}
                }
            }
        } else if mode == 2 && (0..9).contains(&button) {
            if slot < 0 || slot as usize >= slots.len() {
                return None;
            }
            let i = slot as usize;
            if self.can_take(slots, i, env) {
                let hb = slots.len() - 9 + button as usize;
                let held = slots[hb].clone();
                let mut flag = held.is_none() || (self.defs[i].player_inv && self.valid(slots, i, held.as_ref().unwrap()));
                let mut empty: i32 = -1;
                if !flag {
                    empty = first_empty(slots, self.player_range());
                    flag |= empty > -1;
                }
                if slots[i].is_some() && flag {
                    let in_slot = slots[i].clone().unwrap();
                    slots[hb] = Some(in_slot.clone());
                    let held_valid_here = held.as_ref().is_some_and(|h| self.valid(slots, i, h));
                    if (!self.defs[i].player_inv || !held_valid_here) && held.is_some() {
                        if empty > -1 {
                            add_to_inventory(slots, self.player_range(), held.unwrap());
                            slots[i] = None;
                            self.on_pickup(slots, i, &in_slot);
                        }
                    } else {
                        slots[i] = held;
                        self.on_pickup(slots, i, &in_slot);
                    }
                } else if slots[i].is_none() {
                    if let Some(h) = held {
                        if self.valid(slots, i, &h) {
                            slots[hb] = None;
                            slots[i] = Some(h);
                        }
                    }
                }
            }
        } else if mode == 3 && env.creative && cursor.is_none() && slot >= 0 {
            if let Some(Some(s)) = slots.get(slot as usize) {
                let mut c = s.clone();
                c.count = max_stack(&c);
                *cursor = Some(c);
            }
        } else if mode == 4 && cursor.is_none() && slot >= 0 {
            let i = slot as usize;
            if i < slots.len() && slots[i].is_some() && self.can_take(slots, i, env) {
                let n = if button == 0 { 1 } else { slots[i].as_ref().unwrap().count };
                if let Some(t) = self.decr(slots, i, n) {
                    self.on_pickup(slots, i, &t);
                }
            }
        } else if mode == 6 && slot >= 0 {
            let i0 = slot as usize;
            if let Some(mut c) = cursor.clone() {
                let slot_ok = slots.get(i0).is_some_and(|s| s.is_some()) && self.can_take(slots, i0, env);
                if !slot_ok {
                    let n = slots.len() as i32;
                    let (start, step) = if button == 0 { (0, 1) } else { (n - 1, -1) };
                    for pass in 0..2 {
                        let mut i = start;
                        while i >= 0 && i < n && c.count < max_stack(&c) {
                            let u = i as usize;
                            if let Some(s) = slots[u].as_ref() {
                                if self.can_add(slots, Some(u), &c, true)
                                    && self.can_take(slots, u, env)
                                    && self.can_merge(u)
                                    && (pass != 0 || s.count != max_stack(s))
                                {
                                    let l = (max_stack(&c) - c.count).min(s.count);
                                    let taken = self.decr(slots, u, l);
                                    c.count += l;
                                    if let Some(t) = taken {
                                        self.on_pickup(slots, u, &t);
                                    }
                                }
                            }
                            i += step;
                        }
                    }
                    *cursor = Some(c);
                }
            }
        }
        result
    }

    /// Faixa dos 36 slots do jogador (principal + hotbar) da janela.
    pub fn player_range(&self) -> std::ops::Range<usize> {
        match self.kind {
            WinKind::Player => 9..45,
            WinKind::Creative => 45..54,
            _ => self.defs.len().saturating_sub(36)..self.defs.len(),
        }
    }
}


/// `SlotMerchantResult.doTrade`.
fn do_trade(t: &Trade, a: &mut Option<Slot>, b: &mut Option<Slot>) -> bool {
    let Some(sa) = a.as_mut() else { return false };
    if sa.id != t.buy1.id {
        return false;
    }
    match (&t.buy2, b.as_mut()) {
        (Some(s2), Some(sb)) if s2.id == sb.id => {
            sa.count = sa.count.saturating_sub(t.buy1.count);
            sb.count = sb.count.saturating_sub(s2.count);
            true
        }
        (None, None) => {
            sa.count = sa.count.saturating_sub(t.buy1.count);
            true
        }
        _ => false,
    }
}

/// Tipo de armadura de `ItemArmor` (0 capacete .. 3 botas); abóbora e cabeça não contam (não são `ItemArmor`).
pub fn armor_type(s: &Slot) -> Option<u8> {
    (298..=317).contains(&s.id).then(|| ((s.id - 298) % 4) as u8)
}

/// `InventoryPlayer.getFirstEmptyStack` sobre a faixa dada (hotbar primeiro, como o Java: 0..36 do mainInventory).
fn first_empty(slots: &[Option<Slot>], range: std::ops::Range<usize>) -> i32 {
    let base = range.start;
    let len = range.len();
    // mainInventory: 0..9 hotbar, 9..36 principal; na janela, principal vem antes da hotbar.
    let order = (len - 9..len).chain(0..len - 9);
    for k in order {
        if slots[base + k].is_none() {
            return k as i32;
        }
    }
    -1
}

/// `InventoryPlayer.addItemStackToInventory` (versão enxuta: junta nos iguais, depois no primeiro vazio).
fn add_to_inventory(slots: &mut [Option<Slot>], range: std::ops::Range<usize>, mut s: Slot) {
    let base = range.start;
    let len = range.len();
    let order: Vec<usize> = (len - 9..len).chain(0..len - 9).collect();
    if is_stackable(&s) {
        for &k in &order {
            if let Some(t) = slots[base + k].as_mut() {
                if t.id == s.id && (!has_subtypes(s.id) || t.damage == s.damage) && same_tags(t, &s) && t.count < max_stack(t) {
                    let n = (max_stack(t) - t.count).min(s.count);
                    t.count += n;
                    s.count -= n;
                    if s.count == 0 {
                        return;
                    }
                }
            }
        }
    }
    for &k in &order {
        if slots[base + k].is_none() {
            slots[base + k] = Some(s);
            return;
        }
    }
}

#[cfg(test)]
mod tests;
