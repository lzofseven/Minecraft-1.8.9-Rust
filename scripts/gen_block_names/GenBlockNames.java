// Gera mc-client/src/gen_block_names.rs rodando o código original (forgeBin 1.8.9): o nome de registro de cada
// bloco (Block.blockRegistry), usado pelo CanDestroy e CanPlaceOn (Block.getBlockFromName).
import java.io.*;
import java.lang.reflect.Field;
import net.minecraft.block.Block;
import net.minecraft.util.ResourceLocation;

public class GenBlockNames {
    public static void main(String[] args) throws Exception {
        Field reg = net.minecraft.init.Bootstrap.class.getDeclaredField("alreadyRegistered");
        reg.setAccessible(true);
        Block.registerBlocks();
        reg.setBoolean(null, true);
        StringBuilder rows = new StringBuilder();
        int n = 0;
        for (int id = 0; id < 4096; id++) {
            Block b = Block.getBlockById(id);
            if (b == null || Block.getIdFromBlock(b) != id) continue;
            ResourceLocation name = (ResourceLocation) Block.blockRegistry.getNameForObject(b);
            if (name == null) continue;
            rows.append("    (").append(id).append(", \"").append(name.toString()).append("\"),\n");
            n++;
        }
        String o = "// Gerado por scripts/gen_block_names/gen.sh a partir do forgeBin 1.8.9. Não editar à mão.\n\n"
            + "/// `Block.blockRegistry`: (id, nome de registro). " + n + " blocos.\n"
            + "pub const BLOCK_NAMES: &[(u16, &str)] = &[\n" + rows + "];\n";
        try (Writer w = new OutputStreamWriter(new FileOutputStream(args[0]), "UTF-8")) { w.write(o); }
    }
}
