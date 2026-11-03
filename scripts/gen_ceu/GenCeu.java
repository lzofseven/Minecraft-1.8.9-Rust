// Gera os dados do céu a partir do código original da 1.8.9 (forgeBin com nomes MCP):
//  - mc-render/src/sky_golden.txt: valores de referência (World/WorldProvider/RenderGlobal
//    executados de verdade) contra os quais os testes em Rust comparam.
//
// Uso: scripts/gen_ceu/gen.sh

import java.io.PrintWriter;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.ByteBuffer;
import java.util.*;

import net.minecraft.client.Minecraft;
import net.minecraft.client.renderer.RenderGlobal;
import net.minecraft.client.renderer.WorldRenderer;
import net.minecraft.client.settings.GameSettings;
import net.minecraft.entity.Entity;
import net.minecraft.util.BlockPos;
import net.minecraft.util.MathHelper;
import net.minecraft.util.Vec3;
import net.minecraft.world.World;
import net.minecraft.world.WorldProvider;
import net.minecraft.world.WorldProviderEnd;
import net.minecraft.world.WorldProviderHell;
import net.minecraft.world.WorldProviderSurface;
import net.minecraft.world.biome.BiomeGenBase;
import net.minecraft.world.chunk.IChunkProvider;
import net.minecraft.world.storage.WorldInfo;
import sun.misc.Unsafe;

public class GenCeu {
    static Unsafe U;
    static int[] ids;

    /// Bioma em função da coluna (determinístico; o teste em Rust usa a mesma regra).
    static BiomeGenBase biomeAt(int x, int z) {
        int k = Math.floorMod((x >> 3) * 31 + (z >> 3) * 17, ids.length);
        return BiomeGenBase.getBiome(ids[k]);
    }

    public static abstract class FakeWorld extends World {
        protected FakeWorld() {
            super(null, null, null, null, false);
        }

        @Override
        public BiomeGenBase getBiomeGenForCoords(BlockPos pos) {
            return biomeAt(pos.getX(), pos.getZ());
        }
    }

    public static class Impl extends FakeWorld {
        @Override
        protected IChunkProvider createChunkProvider() {
            return null;
        }

        @Override
        protected int getRenderDistanceChunks() {
            return 10;
        }
    }

    static Field field(Class<?> c, String name) throws Exception {
        for (Class<?> k = c; k != null; k = k.getSuperclass()) {
            try {
                Field f = k.getDeclaredField(name);
                f.setAccessible(true);
                return f;
            } catch (NoSuchFieldException e) {
            }
        }
        throw new NoSuchFieldException(name);
    }

    static void putObj(Object o, String name, Object v) throws Exception {
        Field f = field(o.getClass(), name);
        U.putObject(o, U.objectFieldOffset(f), v);
    }

    static void putFloat(Object o, String name, float v) throws Exception {
        U.putFloat(o, U.objectFieldOffset(field(o.getClass(), name)), v);
    }

    static void putInt(Object o, String name, int v) throws Exception {
        U.putInt(o, U.objectFieldOffset(field(o.getClass(), name)), v);
    }

    static void putLong(Object o, String name, long v) throws Exception {
        U.putLong(o, U.objectFieldOffset(field(o.getClass(), name)), v);
    }

    static String bits(float f) {
        return Integer.toHexString(Float.floatToIntBits(f));
    }

    static String bits(double d) {
        return Integer.toHexString(Float.floatToIntBits((float) d));
    }

    static String fl(float f) {
        return Float.toString(f);
    }

    static World world(WorldProvider p, long time, float rain, float thunder, int lightning) throws Exception {
        Impl w = (Impl) U.allocateInstance(Impl.class);
        WorldInfo info = (WorldInfo) U.allocateInstance(WorldInfo.class);
        putLong(info, "worldTime", time);
        putObj(w, "worldInfo", info);
        putObj(w, "provider", p);
        putObj(p, "worldObj", w);
        w.prevRainingStrength = rain;
        w.rainingStrength = rain;
        w.prevThunderingStrength = thunder;
        w.thunderingStrength = thunder;
        putInt(w, "lastLightningBolt", lightning);
        putLong(w, "cloudColour", 16777215L);
        return w;
    }

    static WorldProvider provider(int dim) throws Exception {
        WorldProvider p;
        if (dim == 0) p = (WorldProvider) U.allocateInstance(WorldProviderSurface.class);
        else if (dim == -1) p = (WorldProvider) U.allocateInstance(WorldProviderHell.class);
        else p = (WorldProvider) U.allocateInstance(WorldProviderEnd.class);
        putInt(p, "dimensionId", dim);
        // a tabela de brilho é preenchida pelo generateLightBrightnessTable da própria classe
        putObj(p, "lightBrightnessTable", new float[16]);
        Method m = WorldProvider.class.getDeclaredMethod("generateLightBrightnessTable");
        m.setAccessible(true);
        m.invoke(p);
        putObj(p, "colorsSunriseSunset", new float[4]);
        return p;
    }

    static void resetSkyCache() throws Exception {
        Class<?> c = Class.forName("net.minecraftforge.client.ForgeHooksClient");
        Field f = field(c, "skyInit");
        f.setBoolean(null, false);
    }

    /// Cópia literal (aritmética) do EntityRenderer.updateLightmap sem poções/chefe.
    static float[] lightmap(float sun, float flicker, float gamma, boolean lightning, int dim, float[] table) {
        float f1 = sun * 0.95F + 0.05F;
        float[] out = new float[256 * 3];
        for (int i = 0; i < 256; ++i) {
            float f2 = table[i / 16] * f1;
            float f3 = table[i % 16] * (flicker * 0.1F + 1.5F);
            if (lightning) {
                f2 = table[i / 16];
            }
            float f4 = f2 * (sun * 0.65F + 0.35F);
            float f5 = f2 * (sun * 0.65F + 0.35F);
            float f6 = f3 * ((f3 * 0.6F + 0.4F) * 0.6F + 0.4F);
            float f7 = f3 * (f3 * f3 * 0.6F + 0.4F);
            float f8 = f4 + f3;
            float f9 = f5 + f6;
            float f10 = f2 + f7;
            f8 = f8 * 0.96F + 0.03F;
            f9 = f9 * 0.96F + 0.03F;
            f10 = f10 * 0.96F + 0.03F;
            if (dim == 1) {
                f8 = 0.22F + f3 * 0.75F;
                f9 = 0.28F + f6 * 0.75F;
                f10 = 0.25F + f7 * 0.75F;
            }
            if (f8 > 1.0F) f8 = 1.0F;
            if (f9 > 1.0F) f9 = 1.0F;
            if (f10 > 1.0F) f10 = 1.0F;
            float f17 = 1.0F - f8;
            float f13 = 1.0F - f9;
            float f14 = 1.0F - f10;
            f17 = 1.0F - f17 * f17 * f17 * f17;
            f13 = 1.0F - f13 * f13 * f13 * f13;
            f14 = 1.0F - f14 * f14 * f14 * f14;
            f8 = f8 * (1.0F - gamma) + f17 * gamma;
            f9 = f9 * (1.0F - gamma) + f13 * gamma;
            f10 = f10 * (1.0F - gamma) + f14 * gamma;
            f8 = f8 * 0.96F + 0.03F;
            f9 = f9 * 0.96F + 0.03F;
            f10 = f10 * 0.96F + 0.03F;
            if (f8 > 1.0F) f8 = 1.0F;
            if (f9 > 1.0F) f9 = 1.0F;
            if (f10 > 1.0F) f10 = 1.0F;
            if (f8 < 0.0F) f8 = 0.0F;
            if (f9 < 0.0F) f9 = 0.0F;
            if (f10 < 0.0F) f10 = 0.0F;
            out[i * 3] = (int) (f8 * 255.0F);
            out[i * 3 + 1] = (int) (f9 * 255.0F);
            out[i * 3 + 2] = (int) (f10 * 255.0F);
        }
        return out;
    }

    public static void main(String[] args) throws Exception {
        net.minecraft.block.Block.registerBlocks();
        Field reg = net.minecraft.init.Bootstrap.class.getDeclaredField("alreadyRegistered");
        reg.setAccessible(true);
        reg.setBoolean(null, true);
        Field uf = Unsafe.class.getDeclaredField("theUnsafe");
        uf.setAccessible(true);
        U = (Unsafe) uf.get(null);

        BiomeGenBase[] all = BiomeGenBase.getBiomeGenArray();
        List<Integer> idl = new ArrayList<>();
        for (int i = 0; i < 256; i++) if (all[i] != null) idl.add(i);
        ids = idl.stream().mapToInt(Integer::intValue).toArray();

        // Minecraft/GameSettings falsos para o ForgeHooksClient.getSkyBlendColour.
        Minecraft mc = (Minecraft) U.allocateInstance(Minecraft.class);
        GameSettings gs = (GameSettings) U.allocateInstance(GameSettings.class);
        gs.fancyGraphics = true;
        gs.renderDistanceChunks = 10;
        putObj(mc, "gameSettings", gs);
        Field tm = field(Minecraft.class, "theMinecraft");
        U.putObject(U.staticFieldBase(tm), U.staticFieldOffset(tm), mc);

        // ---- sky_golden.txt ----
        try (PrintWriter w = new PrintWriter(args[0])) {
            w.println("# Gerado por scripts/gen_ceu/gen.sh (código original da 1.8.9 executado). Floats em bits hex.");
            // Biomas: temperatura com ruído de altura e cor do céu.
            int[][] cols = {{0, 0}, {100, -250}, {-33, 17}, {1234, 4321}, {-2000, 77}};
            int[] ys = {63, 64, 65, 100, 200};
            for (int id : ids) {
                BiomeGenBase b = all[id];
                for (int[] c : cols) {
                    for (int y : ys) {
                        float t = b.getFloatTemperature(new BlockPos(c[0], y, c[1]));
                        w.println("biome " + id + " " + c[0] + " " + y + " " + c[1] + " " + bits(t) + " " + (b.getSkyColorByTemp(t) & 0xFFFFFF));
                    }
                }
            }
            long[] times = {0, 1, 1000, 5999, 6000, 11000, 12000, 12500, 13000, 13800, 14500, 18000, 22000, 23000, 23500, 23999, 24000, 30001, 100000, 123456789};
            float[] pts = {0.0F, 0.37F, 1.0F};
            float[][] weather = {{0, 0}, {0.5F, 0}, {1, 0}, {1, 1}, {0.3F, 0.8F}};
            double[][] eyes = {{0.5, 70.0, 0.5}, {100.3, 80.0, -250.7}, {-33.0, 200.0, 17.5}};
            for (int dim : new int[] {0, -1, 1}) {
                for (long t : times) {
                    for (float pt : pts) {
                        for (float[] wt : weather) {
                            for (int lightning : new int[] {0, 2}) {
                                WorldProvider p = provider(dim);
                                World world = world(p, t, wt[0], wt[1], lightning);
                                float angle = world.getCelestialAngle(pt);
                                float[] sr = p.calcSunriseSunsetColors(angle, pt);
                                Vec3 fog = world.getFogColor(pt);
                                Vec3 cloud = world.getCloudColour(pt);
                                float star = world.getStarBrightness(pt);
                                float sunb = world.getSunBrightness(1.0F);
                                int sub = world.calculateSkylightSubtracted(pt);
                                float fac = world.getSunBrightnessFactor(pt);
                                int moon = world.getMoonPhase();
                                float rs = world.getRainStrength(pt), ts = world.getThunderStrength(pt);
                                StringBuilder sb = new StringBuilder("env " + dim + " " + t + " " + bits(pt) + " " + bits(wt[0]) + " " + bits(wt[1]) + " " + lightning);
                                sb.append(" " + bits(angle) + " " + (sr == null ? "0 0 0 0 0" : "1 " + bits(sr[0]) + " " + bits(sr[1]) + " " + bits(sr[2]) + " " + bits(sr[3])));
                                sb.append(" " + bits(fog.xCoord) + " " + bits(fog.yCoord) + " " + bits(fog.zCoord));
                                sb.append(" " + bits(cloud.xCoord) + " " + bits(cloud.yCoord) + " " + bits(cloud.zCoord));
                                sb.append(" " + bits(star) + " " + bits(sunb) + " " + sub + " " + bits(fac) + " " + moon + " " + bits(rs) + " " + bits(ts));
                                w.println(sb);
                                for (double[] e : eyes) {
                                    Entity en = (Entity) U.allocateInstance(net.minecraft.entity.item.EntityItem.class);
                                    en.posX = e[0];
                                    en.posY = e[1];
                                    en.posZ = e[2];
                                    resetSkyCache();
                                    Vec3 sky = world.getSkyColor(en, pt);
                                    w.println("sky " + dim + " " + t + " " + bits(pt) + " " + bits(wt[0]) + " " + bits(wt[1]) + " " + lightning + " " + e[0] + " " + e[1] + " " + e[2] + " " + bits(sky.xCoord) + " " + bits(sky.yCoord) + " " + bits(sky.zCoord));
                                }
                            }
                        }
                    }
                }
            }

            // Tabela de brilho por dimensão.
            for (int dim : new int[] {0, -1, 1}) {
                WorldProvider p = provider(dim);
                StringBuilder sb = new StringBuilder("table " + dim);
                for (float v : p.getLightBrightnessTable()) sb.append(" " + bits(v));
                w.println(sb);
            }

            // Mapa de luz (lightmap).
            WorldProvider p0 = provider(0);
            for (float sun : new float[] {1.0F, 0.8F, 0.2F}) {
                for (float flicker : new float[] {0.0F, 0.7F, -0.4F}) {
                    for (float gamma : new float[] {0.0F, 0.5F, 1.0F}) {
                        for (int dim : new int[] {0, 1}) {
                            for (boolean lightning : new boolean[] {false, true}) {
                                float[] lm = lightmap(sun, flicker, gamma, lightning, dim, p0.getLightBrightnessTable());
                                long h = 1125899906842597L;
                                for (float v : lm) h = 31 * h + (long) v;
                                StringBuilder sb = new StringBuilder("lightmap " + bits(sun) + " " + bits(flicker) + " " + bits(gamma) + " " + dim + " " + (lightning ? 1 : 0) + " " + h);
                                for (int i : new int[] {0, 15, 16, 17, 100, 240, 255}) {
                                    sb.append(" " + (int) lm[i * 3] + "," + (int) lm[i * 3 + 1] + "," + (int) lm[i * 3 + 2]);
                                }
                                w.println(sb);
                            }
                        }
                    }
                }
            }

            // Estrelas e domo: RenderGlobal executado de verdade (sem GL).
            RenderGlobal rg = (RenderGlobal) U.allocateInstance(RenderGlobal.class);
            WorldRenderer wr = new WorldRenderer(2097152);
            Method stars = RenderGlobal.class.getDeclaredMethod("renderStars", WorldRenderer.class);
            stars.setAccessible(true);
            stars.invoke(rg, wr);
            wr.finishDrawing();
            ByteBuffer bb = wr.getByteBuffer();
            int nv = wr.getVertexCount();
            w.println("stars " + nv);
            double sum = 0;
            for (int i = 0; i < nv; i++) {
                float x = bb.getFloat(i * 12), y = bb.getFloat(i * 12 + 4), z = bb.getFloat(i * 12 + 8);
                if (i < 12) w.println("star " + i + " " + bits(x) + " " + bits(y) + " " + bits(z));
                sum += x * 1.0 + y * 3.0 + z * 7.0;
            }
            w.println("starsum " + Double.toString(sum));
            wr = new WorldRenderer(2097152);
            Method sky = RenderGlobal.class.getDeclaredMethod("renderSky", WorldRenderer.class, float.class, boolean.class);
            sky.setAccessible(true);
            sky.invoke(rg, wr, 16.0F, false);
            wr.finishDrawing();
            w.println("dome " + wr.getVertexCount());
        }
    }
}
