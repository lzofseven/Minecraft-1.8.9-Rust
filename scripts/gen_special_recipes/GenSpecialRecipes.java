// Gera mc-client/src/gen_special_recipes.rs rodando o código original (forgeBin 1.8.9): os dados que as receitas
// especiais do CraftingManager leem (cores dos corantes, padrões de estandarte, itens reparáveis e de couro).
import java.io.*;
import java.lang.reflect.Field;
import net.minecraft.block.Block;
import net.minecraft.entity.passive.EntitySheep;
import net.minecraft.item.*;
import net.minecraft.tileentity.TileEntityBanner;

public class GenSpecialRecipes {
    public static void main(String[] args) throws Exception {
        Field reg = net.minecraft.init.Bootstrap.class.getDeclaredField("alreadyRegistered");
        reg.setAccessible(true);
        Block.registerBlocks();
        reg.setBoolean(null, true);
        Item.registerItems();
        StringBuilder o = new StringBuilder("// Gerado por scripts/gen_special_recipes/gen.sh a partir do forgeBin 1.8.9. Não editar à mão.\n\n");
        o.append("/// `EntitySheep.getDyeRgb(EnumDyeColor.byDyeDamage(d))` em bits de f32, por dano do corante.\npub const DYE_RGB: [[u32; 3]; 16] = [\n");
        for (int d = 0; d < 16; d++) {
            float[] f = EntitySheep.getDyeRgb(net.minecraft.item.EnumDyeColor.byDyeDamage(d));
            o.append("    [").append(Float.floatToIntBits(f[0])).append(", ").append(Float.floatToIntBits(f[1])).append(", ").append(Float.floatToIntBits(f[2])).append("],\n");
        }
        o.append("];\n\n/// `ItemDye.dyeColors`.\npub const DYE_COLORS: [i32; 16] = [");
        for (int c : ItemDye.dyeColors) o.append(c).append(", ");
        o.append("];\n\n/// `TileEntityBanner.EnumBannerPattern.values()`: (id, receita válida, item da receita (id, dano) ou -1, camadas).\n");
        o.append("pub const BANNER_PATTERNS: &[(&str, bool, (i16, i16), [&str; 3])] = &[\n");
        for (TileEntityBanner.EnumBannerPattern p : TileEntityBanner.EnumBannerPattern.values()) {
            ItemStack s = p.hasCraftingStack() ? p.getCraftingStack() : null;
            String[] l = p.getCraftingLayers();
            o.append("    (\"").append(p.getPatternID()).append("\", ").append(p.hasValidCrafting()).append(", (")
             .append(s == null ? -1 : Item.getIdFromItem(s.getItem())).append(", ").append(s == null ? 0 : s.getMetadata()).append("), [");
            for (int i = 0; i < 3; i++) o.append("\"").append(l != null && i < l.length && l[i] != null ? l[i] : "").append("\", ");
            o.append("]),\n");
        }
        o.append("];\n\n/// `Item.isRepairable()`.\npub const REPAIRABLE: &[i16] = &[");
        StringBuilder leather = new StringBuilder();
        for (int id = 0; id < 32000; id++) {
            Item it = Item.getItemById(id);
            if (it == null) continue;
            if (it.isRepairable()) o.append(id).append(", ");
            if (it instanceof ItemArmor && ((ItemArmor) it).getArmorMaterial() == ItemArmor.ArmorMaterial.LEATHER) leather.append(id).append(", ");
        }
        o.append("];\n\n/// `ItemArmor` de couro (`ArmorMaterial.LEATHER`).\npub const LEATHER: &[i16] = &[").append(leather).append("];\n");
        try (Writer w = new OutputStreamWriter(new FileOutputStream(args[0]), "UTF-8")) { w.write(o.toString()); }
    }
}
