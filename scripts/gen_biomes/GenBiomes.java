// Gera mc-world/src/gen_biomes.rs (tabela de BiomeGenBase) e gen_biome_vectors.txt (cores de
// grama/folhagem calculadas pelo código original) executando as classes da 1.8.9.
//
// Uso: scripts/gen_biomes/gen.sh

import java.io.PrintWriter;
import java.lang.reflect.Field;
import java.util.*;
import java.util.zip.ZipFile;

import javax.imageio.ImageIO;
import net.minecraft.util.BlockPos;
import net.minecraft.world.ColorizerFoliage;
import net.minecraft.world.ColorizerGrass;
import net.minecraft.world.biome.*;

public class GenBiomes {
    static int[] readPng(ZipFile z, String name) throws Exception {
        java.awt.image.BufferedImage img = ImageIO.read(z.getInputStream(z.getEntry(name)));
        return img.getRGB(0, 0, img.getWidth(), img.getHeight(), null, 0, img.getWidth());
    }

    static Object field(Class<?> c, Object o, String n) throws Exception {
        Field f = c.getDeclaredField(n);
        f.setAccessible(true);
        return f.get(o);
    }

    public static void main(String[] a) throws Exception {
        net.minecraft.block.Block.registerBlocks();
        Field reg = net.minecraft.init.Bootstrap.class.getDeclaredField("alreadyRegistered");
        reg.setAccessible(true);
        reg.setBoolean(null, true);
        ZipFile z = new ZipFile(a[2]);
        ColorizerGrass.setGrassBiomeColorizer(readPng(z, "assets/minecraft/textures/colormap/grass.png"));
        ColorizerFoliage.setFoliageBiomeColorizer(readPng(z, "assets/minecraft/textures/colormap/foliage.png"));
        BiomeGenBase[] all = BiomeGenBase.getBiomeGenArray();
        PrintWriter rs = new PrintWriter(a[0]);
        rs.println("//! Gerado por scripts/gen_biomes/gen.sh executando BiomeGenBase da 1.8.9. Não edite.");
        rs.println();
        rs.println("/// (id, nome, temperatura, chuva, cor da água, tipo de cor, delegado, pode_chover, neve, céu_zero)");
        rs.println("/// tipo: 0 padrão, 1 pântano, 2 mesa, 3 floresta escura. delegado: bioma-base de `BiomeGenMutated` (-1 = nenhum).");
        rs.println("/// pode_chover = canRain(), neve = getEnableSnow(), céu_zero = getSkyColorByTemp sobrescrito (BiomeGenEnd: devolve 0).");
        rs.println("pub const BIOMES: &[(u8, &str, f32, f32, u32, u8, i16, bool, bool, bool)] = &[");
        PrintWriter vec = new PrintWriter(a[1]);
        vec.println("# id x y z grama folhagem (RGB do código original, com BiomeGenBase.getGrassColorAtPos/getFoliageColorAtPos)");
        Random r = new Random(42);
        for (BiomeGenBase b : all) {
            if (b == null) continue;
            int kind = 0;
            int delegate = -1;
            BiomeGenBase k = b;
            if (b instanceof BiomeGenMutated) {
                k = (BiomeGenBase) field(BiomeGenMutated.class, b, "baseBiome");
                delegate = k.biomeID;
            }
            if (k instanceof BiomeGenSwamp) kind = 1;
            else if (k instanceof BiomeGenMesa) kind = 2;
            else if (k instanceof BiomeGenForest && ((Integer) field(BiomeGenForest.class, k, "field_150632_aF")) == 3) kind = 3;
            rs.println("    (" + b.biomeID + ", \"" + b.biomeName + "\", " + Float.toString(b.temperature) + "_f32, " + Float.toString(b.rainfall) + "_f32, " + b.waterColorMultiplier + ", " + kind + ", " + delegate + ", " + b.canRain() + ", " + b.getEnableSnow() + ", " + (b.getClass().getMethod("getSkyColorByTemp", float.class).getDeclaringClass() != BiomeGenBase.class) + "),");
            for (int i = 0; i < 12; i++) {
                int x = r.nextInt(20000) - 10000, z2 = r.nextInt(20000) - 10000;
                int y = i < 6 ? 40 + r.nextInt(25) : 65 + r.nextInt(180);
                BlockPos p = new BlockPos(x, y, z2);
                vec.println(b.biomeID + " " + x + " " + y + " " + z2 + " " + (b.getGrassColorAtPos(p) & 0xFFFFFF) + " " + (b.getFoliageColorAtPos(p) & 0xFFFFFF));
            }
        }
        rs.println("];");
        rs.close();
        vec.close();
    }
}
