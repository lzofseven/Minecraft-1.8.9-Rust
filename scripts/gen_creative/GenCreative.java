// Gera mc-client/src/gen_creative.rs rodando o código original (forgeBin 1.8.9): as abas de
// CreativeTabs (posição, ícone, fundo, busca) e a lista de itens de cada uma
// (displayAllReleventItems), mais a lista da aba de busca como GuiContainerCreative.updateCreativeSearch
// a monta (getSubItems de todo o registro + livros encantados de Enchantment.enchantmentsBookList).
import java.io.*;
import java.lang.reflect.Field;
import java.util.*;
import net.minecraft.creativetab.CreativeTabs;
import net.minecraft.enchantment.Enchantment;
import net.minecraft.init.Items;
import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;
import net.minecraft.nbt.CompressedStreamTools;

public class GenCreative {
    static String stack(ItemStack s) throws IOException {
        String nbt = "&[]";
        if (s.getTagCompound() != null) {
            ByteArrayOutputStream b = new ByteArrayOutputStream();
            CompressedStreamTools.write(s.getTagCompound(), new DataOutputStream(b));
            StringBuilder h = new StringBuilder("&[");
            for (byte x : b.toByteArray()) h.append(x & 0xFF).append(',');
            nbt = h.append(']').toString();
        }
        return String.format("(%d, %d, %s)", Item.getIdFromItem(s.getItem()), s.getMetadata(), nbt);
    }

    static void list(StringBuilder out, List<ItemStack> l) throws IOException {
        out.append("&[\n");
        for (ItemStack s : l) out.append("        ").append(stack(s)).append(",\n");
        out.append("    ]");
    }

    public static void main(String[] args) throws Exception {
        Field reg = net.minecraft.init.Bootstrap.class.getDeclaredField("alreadyRegistered");
        reg.setAccessible(true);
        net.minecraft.block.Block.registerBlocks();
        reg.setBoolean(null, true);
        Item.registerItems();
        StringBuilder o = new StringBuilder();
        o.append("// Gerado por scripts/gen_creative/gen.sh a partir do forgeBin 1.8.9. Não edite.\n\n");
        o.append("/// (id, dano, NBT como vai na rede: raiz composta sem nome, vazio = sem NBT)\n");
        o.append("pub type Stack = (i16, i16, &'static [u8]);\n\n");
        o.append("pub struct Tab {\n    pub index: usize,\n    /// `getTranslatedTabLabel` (chave do .lang).\n    pub label: &'static str,\n");
        o.append("    pub icon: (i16, i16),\n    /// `tab_<nome>` em gui/container/creative_inventory.\n    pub background: &'static str,\n");
        o.append("    pub search: bool,\n    pub search_width: i32,\n    pub hide_player_inventory: bool,\n    pub draw_title: bool,\n");
        o.append("    pub column: i32,\n    pub first_row: bool,\n    pub page: i32,\n}\n\n");
        o.append("pub const TABS: [Tab; " + CreativeTabs.creativeTabArray.length + "] = [\n");
        for (CreativeTabs t : CreativeTabs.creativeTabArray) {
            ItemStack ic = t.getIconItemStack();
            o.append(String.format("    Tab { index: %d, label: \"%s\", icon: (%d, %d), background: \"%s\", search: %b, search_width: %d, hide_player_inventory: %b, draw_title: %b, column: %d, first_row: %b, page: %d },\n",
                t.getTabIndex(), t.getTranslatedTabLabel(), Item.getIdFromItem(ic.getItem()), ic.getMetadata(), t.getBackgroundImageName(),
                t.hasSearchBar(), t.getSearchbarWidth(), t.shouldHidePlayerInventory(), t.drawInForegroundOfTab(), t.getTabColumn(), t.isTabInFirstRow(), t.getTabPage()));
        }
        o.append("];\n\n");
        o.append("/// `displayAllReleventItems` de cada aba.\n");
        o.append("pub const ITEMS: [&[Stack]; " + CreativeTabs.creativeTabArray.length + "] = [\n");
        for (CreativeTabs t : CreativeTabs.creativeTabArray) {
            List<ItemStack> l = new ArrayList<>();
            t.displayAllReleventItems(l);
            o.append("    ");
            list(o, l);
            o.append(",\n");
        }
        o.append("];\n\n");
        List<ItemStack> all = new ArrayList<>();
        for (Item item : Item.itemRegistry) {
            if (item != null && item.getCreativeTab() != null) item.getSubItems(item, (CreativeTabs) null, all);
        }
        for (Enchantment e : Enchantment.enchantmentsBookList) {
            if (e != null && e.type != null) Items.enchanted_book.getAll(e, all);
        }
        o.append("/// Lista da aba de busca antes do filtro (`updateCreativeSearch` + `updateFilteredItems`).\n");
        o.append("pub const SEARCH: &[Stack] = ");
        list(o, all);
        o.append(";\n");
        try (Writer w = new OutputStreamWriter(new FileOutputStream(args[0]), "UTF-8")) {
            w.write(o.toString());
        }
        System.out.println("abas: " + CreativeTabs.creativeTabArray.length + ", busca: " + all.size());
    }
}
