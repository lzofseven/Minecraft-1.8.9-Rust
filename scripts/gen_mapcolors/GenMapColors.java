// Gera mc-client/src/gen_mapcolors.rs rodando o código original (forgeBin 1.8.9): a cor de cada
// MapColor.mapColorArray com os quatro tons de getMapColor, como o MapItemRenderer pinta o mapa.
import java.io.*;
import net.minecraft.block.material.MapColor;

public class GenMapColors {
    public static void main(String[] args) throws Exception {
        StringBuilder o = new StringBuilder();
        o.append("// Gerado por scripts/gen_mapcolors/gen.sh a partir do forgeBin 1.8.9. Não editar à mão.\n\n");
        o.append("/// `MapColor.mapColorArray[i / 4].getMapColor(i & 3)` (ARGB) por byte do mapa; 0 sem cor.\n");
        o.append("pub const MAP_COLORS: [u32; 256] = [\n");
        for (int i = 0; i < 256; i++) {
            MapColor c = MapColor.mapColorArray[i / 4];
            int v = c == null ? 0 : c.getMapColor(i & 3);
            o.append(String.format("    0x%08X,\n", v));
        }
        o.append("];\n");
        try (Writer w = new OutputStreamWriter(new FileOutputStream(args[0]), "UTF-8")) {
            w.write(o.toString());
        }
    }
}
