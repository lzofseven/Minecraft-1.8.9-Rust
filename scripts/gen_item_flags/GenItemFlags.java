// Gera mc-client/src/gen_item_flags.rs rodando o código original (forgeBin 1.8.9): os itens com
// Item.isFull3D() e com shouldRotateAroundWhenRendering(), usados nas camadas de item na mão.
import java.io.*;
import java.lang.reflect.Field;
import net.minecraft.block.Block;
import net.minecraft.item.Item;

public class GenItemFlags {
    public static void main(String[] args) throws Exception {
        Field reg = net.minecraft.init.Bootstrap.class.getDeclaredField("alreadyRegistered");
        reg.setAccessible(true);
        Block.registerBlocks();
        reg.setBoolean(null, true);
        Item.registerItems();
        StringBuilder full = new StringBuilder(), rot = new StringBuilder();
        for (int id = 0; id < 32000; id++) {
            Item it = Item.getItemById(id);
            if (it == null) continue;
            if (it.isFull3D()) full.append(id).append(", ");
            if (it.shouldRotateAroundWhenRendering()) rot.append(id).append(", ");
        }
        String o = "// Gerado por scripts/gen_item_flags/gen.sh a partir do forgeBin 1.8.9. Não editar à mão.\n\n"
            + "/// Ids com `Item.isFull3D()`.\npub const FULL_3D: &[i16] = &[" + full + "];\n\n"
            + "/// Ids com `Item.shouldRotateAroundWhenRendering()`.\npub const ROTATE_AROUND: &[i16] = &[" + rot + "];\n";
        try (Writer w = new OutputStreamWriter(new FileOutputStream(args[0]), "UTF-8")) {
            w.write(o);
        }
    }
}
