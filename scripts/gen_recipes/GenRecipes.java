// Gera mc-client/src/gen_recipes.rs rodando o código original (forgeBin 1.8.9): as receitas do CraftingManager na
// ordem da lista (as com forma e as sem forma; as do OreDictionary do Forge também, se houver) e os itens de container.
import java.io.*;
import java.lang.reflect.Field;
import java.util.*;
import net.minecraft.block.Block;
import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;
import net.minecraft.item.crafting.*;

public class GenRecipes {
    static Object get(Object o, String... names) throws Exception {
        for (Class<?> c = o.getClass(); c != null; c = c.getSuperclass()) {
            for (String n : names) {
                try { Field f = c.getDeclaredField(n); f.setAccessible(true); return f.get(o); } catch (NoSuchFieldException e) { }
            }
        }
        throw new NoSuchFieldException(names[0]);
    }
    static String alt(Object in) {
        if (in == null) return "&[]";
        StringBuilder b = new StringBuilder("&[");
        List<ItemStack> list = new ArrayList<ItemStack>();
        if (in instanceof ItemStack) list.add((ItemStack) in);
        else for (Object o : (Collection<?>) in) list.add((ItemStack) o);
        for (ItemStack s : list) b.append("(").append(Item.getIdFromItem(s.getItem())).append(", ").append(s.getMetadata()).append("), ");
        return b.append("]").toString();
    }
    static String out(ItemStack s) { return "(" + Item.getIdFromItem(s.getItem()) + ", " + s.stackSize + ", " + s.getMetadata() + ")"; }

    public static void main(String[] args) throws Exception {
        Field reg = net.minecraft.init.Bootstrap.class.getDeclaredField("alreadyRegistered");
        reg.setAccessible(true);
        Block.registerBlocks();
        reg.setBoolean(null, true);
        Item.registerItems();
        StringBuilder rows = new StringBuilder();
        int n = 0, special = 0;
        for (IRecipe r : CraftingManager.getInstance().getRecipeList()) {
            String cls = r.getClass().getSimpleName();
            if (r instanceof ShapedRecipes) {
                int w = (Integer) get(r, "recipeWidth", "field_77576_b"), h = (Integer) get(r, "recipeHeight", "field_77577_c");
                ItemStack[] items = (ItemStack[]) get(r, "recipeItems", "field_77574_d");
                StringBuilder it = new StringBuilder();
                for (ItemStack s : items) it.append(alt(s)).append(", ");
                rows.append("    R::Shaped { w: ").append(w).append(", h: ").append(h).append(", mirrored: true, items: &[").append(it).append("], out: ").append(out(r.getRecipeOutput())).append(" },\n");
            } else if (r instanceof ShapelessRecipes) {
                List<?> items = (List<?>) get(r, "recipeItems", "field_77579_b");
                StringBuilder it = new StringBuilder();
                for (Object s : items) it.append(alt(s)).append(", ");
                rows.append("    R::Shapeless { items: &[").append(it).append("], out: ").append(out(r.getRecipeOutput())).append(" },\n");
            } else if (cls.equals("ShapedOreRecipe")) {
                Object[] input = (Object[]) get(r, "input");
                int w = (Integer) get(r, "width"), h = (Integer) get(r, "height");
                boolean m = (Boolean) get(r, "mirrored");
                StringBuilder it = new StringBuilder();
                for (Object s : input) it.append(alt(s)).append(", ");
                rows.append("    R::Shaped { w: ").append(w).append(", h: ").append(h).append(", mirrored: ").append(m).append(", items: &[").append(it).append("], out: ").append(out(r.getRecipeOutput())).append(" },\n");
            } else if (cls.equals("ShapelessOreRecipe")) {
                List<?> input = (List<?>) get(r, "input");
                StringBuilder it = new StringBuilder();
                for (Object s : input) it.append(alt(s)).append(", ");
                rows.append("    R::Shapeless { items: &[").append(it).append("], out: ").append(out(r.getRecipeOutput())).append(" },\n");
            } else {
                rows.append("    R::Special(\"").append(cls).append("\"),\n");
                special++;
            }
            n++;
        }
        StringBuilder cont = new StringBuilder();
        for (int id = 0; id < 32000; id++) {
            Item it = Item.getItemById(id);
            if (it != null && it.getContainerItem() != null) cont.append("    (").append(id).append(", ").append(Item.getIdFromItem(it.getContainerItem())).append("),\n");
        }
        String o = "// Gerado por scripts/gen_recipes/gen.sh a partir do forgeBin 1.8.9. Não editar à mão.\n\n"
            + "use crate::recipes::R;\n\n"
            + "/// `CraftingManager.getRecipeList()`, na ordem: " + n + " receitas (" + special + " especiais sem port).\n"
            + "pub const RECIPES: &[R] = &[\n" + rows + "];\n\n"
            + "/// `Item.getContainerItem`: (item, o que volta à grade).\n"
            + "pub const CONTAINER_ITEMS: &[(i16, i16)] = &[\n" + cont + "];\n";
        try (Writer w = new OutputStreamWriter(new FileOutputStream(args[0]), "UTF-8")) { w.write(o); }
    }
}
