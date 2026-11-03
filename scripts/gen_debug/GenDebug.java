// Gera mc-client/src/gen_debug.rs rodando o código original (forgeBin 1.8.9): para cada id e meta,
// o nome no Block.blockRegistry e as propriedades de getStateFromMeta na ordem de getProperties()
// com o toString() de cada valor, como o GuiOverlayDebug.getDebugInfoRight lista.
import java.io.*;
import java.lang.reflect.Field;
import java.util.*;
import net.minecraft.block.Block;
import net.minecraft.block.properties.IProperty;
import net.minecraft.block.state.IBlockState;

public class GenDebug {
    static String q(String s) {
        return "\"" + s.replace("\\", "\\\\").replace("\"", "\\\"") + "\"";
    }

    public static void main(String[] args) throws Exception {
        Field reg = net.minecraft.init.Bootstrap.class.getDeclaredField("alreadyRegistered");
        reg.setAccessible(true);
        Block.registerBlocks();
        reg.setBoolean(null, true);
        StringBuilder o = new StringBuilder();
        o.append("// Gerado por scripts/gen_debug/gen.sh a partir do forgeBin 1.8.9. Não editar à mão.\n\n");
        o.append("/// `Block.blockRegistry.getNameForObject` por id.\n");
        o.append("pub static NAMES: [&str; 256] = [\n");
        for (int id = 0; id < 256; id++) {
            Block b = Block.getBlockById(id);
            o.append("    ").append(q(String.valueOf(Block.blockRegistry.getNameForObject(b)))).append(",\n");
        }
        o.append("];\n\n");
        o.append("/// Propriedades (nome, `toString` do valor, se é booleano) de `getStateFromMeta`, por `id << 4 | meta`.\n");
        o.append("pub static PROPS: [&[(&str, &str, bool)]; 4096] = [\n");
        for (int id = 0; id < 256; id++) {
            Block b = Block.getBlockById(id);
            for (int m = 0; m < 16; m++) {
                IBlockState st;
                try {
                    st = b.getStateFromMeta(m);
                } catch (Throwable t) {
                    st = b.getDefaultState();
                }
                o.append("    &[");
                for (Map.Entry<IProperty, Comparable> e : st.getProperties().entrySet()) {
                    Comparable v = e.getValue();
                    o.append("(").append(q(e.getKey().getName())).append(", ").append(q(v.toString())).append(", ").append(v instanceof Boolean).append("), ");
                }
                o.append("],\n");
            }
        }
        o.append("];\n");
        try (Writer w = new OutputStreamWriter(new FileOutputStream(args[0]), "UTF-8")) {
            w.write(o.toString());
        }
    }
}
