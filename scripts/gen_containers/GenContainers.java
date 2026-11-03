// Gera mc-client/src/gen_container_items.rs rodando o código original (forgeBin 1.8.9):
// Item.getItemStackLimit/getMaxDamage/getHasSubtypes/isValidArmor/isPotionIngredient,
// TileEntityFurnace.getItemBurnTime e FurnaceRecipes.getSmeltingResult.
import java.io.*;
import java.lang.reflect.Field;
import java.util.*;
import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;
import net.minecraft.item.crafting.FurnaceRecipes;
import net.minecraft.tileentity.TileEntityFurnace;
import net.minecraft.tileentity.TileEntityBeacon;
import net.minecraft.potion.Potion;
import net.minecraft.enchantment.Enchantment;

public class GenContainers {
    public static void main(String[] args) throws Exception {
        Field reg = net.minecraft.init.Bootstrap.class.getDeclaredField("alreadyRegistered");
        reg.setAccessible(true);
        net.minecraft.block.Block.registerBlocks();
        reg.setBoolean(null, true);
        Item.registerItems();
        StringBuilder info = new StringBuilder();
        StringBuilder fuel = new StringBuilder();
        StringBuilder smelt = new StringBuilder();
        TreeMap<Integer, Item> items = new TreeMap<>();
        for (Object o : Item.itemRegistry) {
            Item it = (Item) o;
            items.put(Item.getIdFromItem(it), it);
        }
        for (Map.Entry<Integer, Item> e : items.entrySet()) {
            int id = e.getKey();
            Item it = e.getValue();
            ItemStack st = new ItemStack(it, 1, 0);
            int armor = 0;
            for (int k = 0; k < 4; k++) {
                try { if (it.isValidArmor(st, k, null)) armor |= 1 << k; } catch (Throwable t) { }
            }
            info.append(String.format("    (%d, %d, %d, %b, %d, %b),\n", id, it.getItemStackLimit(st), it.getMaxDamage(),
                it.getHasSubtypes(), armor, it.isPotionIngredient(st)));
            int[] burn = new int[16];
            int[] smelts = new int[16];
            boolean allBurn = true, allSmelt = true;
            for (int d = 0; d < 16; d++) {
                ItemStack s = new ItemStack(it, 1, d);
                try { burn[d] = TileEntityFurnace.getItemBurnTime(s); } catch (Throwable t) { burn[d] = 0; }
                smelts[d] = FurnaceRecipes.instance().getSmeltingResult(s) != null ? 1 : 0;
                if (burn[d] != burn[0]) allBurn = false;
                if (smelts[d] != smelts[0]) allSmelt = false;
            }
            if (allBurn) {
                if (burn[0] > 0) fuel.append(String.format("    (%d, -1, %d),\n", id, burn[0]));
            } else {
                for (int d = 0; d < 16; d++) if (burn[d] > 0) fuel.append(String.format("    (%d, %d, %d),\n", id, d, burn[d]));
            }
            if (allSmelt) {
                if (smelts[0] > 0) smelt.append(String.format("    (%d, -1),\n", id));
            } else {
                for (int d = 0; d < 16; d++) if (smelts[d] > 0) smelt.append(String.format("    (%d, %d),\n", id, d));
            }
        }
        StringBuilder beacon = new StringBuilder();
        for (Potion[] row : TileEntityBeacon.effectsList) {
            beacon.append("    &[");
            for (Potion p : row) beacon.append(String.format("(%d, %d, \"%s\"), ", p.id, p.getStatusIconIndex(), p.getName()));
            beacon.append("],\n");
        }
        StringBuilder ench = new StringBuilder();
        for (int id = 0; id < 256; id++) {
            Enchantment e = Enchantment.getEnchantmentById(id);
            if (e != null) ench.append(String.format("    (%d, \"%s\"),\n", e.effectId, e.getName()));
        }
        try (PrintWriter w = new PrintWriter(args[0], "UTF-8")) {
            w.println("//! Gerado por scripts/gen_containers/gen.sh executando Item, TileEntityFurnace e FurnaceRecipes do jogo original. Não editar.");
            w.println();
            w.println("/// (id, `getItemStackLimit`, `getMaxDamage`, `getHasSubtypes`, máscara de `isValidArmor` por tipo 0..3, `isPotionIngredient`).");
            w.println("pub const ITEM_INFO: &[(u16, u8, u16, bool, u8, bool)] = &[");
            w.print(info);
            w.println("];");
            w.println();
            w.println("/// (id, dano ou -1 para qualquer, ticks de `getItemBurnTime`).");
            w.println("pub const FUEL: &[(u16, i16, u16)] = &[");
            w.print(fuel);
            w.println("];");
            w.println();
            w.println("/// Entradas com `FurnaceRecipes.getSmeltingResult` != null: (id, dano ou -1 para qualquer).");
            w.println("pub const SMELTABLE: &[(u16, i16)] = &[");
            w.print(smelt);
            w.println("];");
            w.println();
            w.println("/// `TileEntityBeacon.effectsList`: por nível, (id do efeito, `getStatusIconIndex`, `Potion.getName`).");
            w.println("pub const BEACON_EFFECTS: [&[(u8, u8, &str)]; 4] = [");
            w.print(beacon);
            w.println("];");
            w.println();
            w.println("/// `Enchantment.enchantmentsList`: (id, `getName`).");
            w.println("pub const ENCHANTS: &[(u8, &str)] = &[");
            w.print(ench);
            w.println("];");
        }
    }
}
