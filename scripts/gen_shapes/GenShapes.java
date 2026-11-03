// Gera mc-world/src/gen_shapes.rs executando o código original da 1.8.9 (jar forgeBin com
// nomes MCP): para cada estado de bloco (e cada vizinhança relevante) consulta
// addCollisionBoxesToList, getSelectedBoundingBox e as propriedades usadas na malha.
//
// Uso: scripts/gen_shapes/gen.sh  (monta o classpath, compila e roda).

import java.io.PrintWriter;
import java.lang.reflect.Field;
import java.util.*;

import net.minecraft.block.*;
import net.minecraft.block.material.Material;
import net.minecraft.block.state.IBlockState;
import net.minecraft.entity.Entity;
import net.minecraft.init.Blocks;
import net.minecraft.util.AxisAlignedBB;
import net.minecraft.util.BlockPos;
import net.minecraft.util.EnumFacing;
import net.minecraft.world.World;
import net.minecraft.world.chunk.IChunkProvider;
import sun.misc.Unsafe;

public class GenShapes {
    /// Mundo falso: só guarda estados num mapa (criado sem construtor).
    public static abstract class FakeWorld extends World {
        public Map<BlockPos, IBlockState> map;

        protected FakeWorld() {
            super(null, null, null, null, false);
        }

        @Override
        public IBlockState getBlockState(BlockPos pos) {
            IBlockState s = map.get(pos);
            return s == null ? Blocks.air.getDefaultState() : s;
        }

        @Override
        public boolean isAirBlock(BlockPos pos) {
            return getBlockState(pos).getBlock().getMaterial() == Material.air;
        }

        @Override
        public net.minecraft.tileentity.TileEntity getTileEntity(BlockPos pos) {
            return null;
        }
    }

    public static class Impl extends FakeWorld {
        @Override
        protected IChunkProvider createChunkProvider() {
            return null;
        }

        @Override
        protected int getRenderDistanceChunks() {
            return 0;
        }
    }

    // Na origem: as caixas saem sem erro de subtração.
    static final BlockPos POS = new BlockPos(0, 0, 0);
    // Ordem dos bits de conexão: norte, leste, sul, oeste.
    static final EnumFacing[] NESW = {EnumFacing.NORTH, EnumFacing.EAST, EnumFacing.SOUTH, EnumFacing.WEST};
    // getHorizontal: sul, oeste, norte, leste.
    static final EnumFacing[] HORIZ = {EnumFacing.SOUTH, EnumFacing.WEST, EnumFacing.NORTH, EnumFacing.EAST};

    static boolean isStairs(int id) {
        return Block.getBlockById(id) instanceof BlockStairs;
    }

    /// Quantas vizinhanças distintas o bloco tem para a forma (igual a states::shape_count).
    static int count(int id) {
        Block b = Block.getBlockById(id);
        if (b instanceof BlockFence || b instanceof BlockPane || b instanceof BlockWall) return 16;
        if (b instanceof BlockStairs) return 5;
        if (b instanceof BlockDoor) return 8;
        if (b instanceof BlockChest) return 5;
        return 1;
    }

    static int stairMeta(EnumFacing f, boolean top) {
        // getStateFromMeta: facing = getFront(5 - (meta & 3)).
        int m = 5 - f.getIndex();
        return m | (top ? 4 : 0);
    }

    /// Monta os vizinhos que produzem `bits` (mesma codificação de states.rs).
    static void neighbors(World w, Map<BlockPos, IBlockState> map, Block b, int meta, IBlockState st, int bits) {
        if (b instanceof BlockFence || b instanceof BlockPane || b instanceof BlockWall) {
            for (int i = 0; i < 4; i++) if ((bits & (1 << i)) != 0) map.put(POS.offset(NESW[i]), st);
        } else if (b instanceof BlockStairs) {
            if (bits == 0) return;
            EnumFacing f = (EnumFacing) st.getValue(BlockStairs.FACING);
            boolean top = st.getValue(BlockStairs.HALF) == BlockStairs.EnumHalf.TOP;
            // 1 inner_left, 2 inner_right, 3 outer_left, 4 outer_right; "direita" = vizinho no sentido horário (invertido no topo).
            boolean right = bits == 2 || bits == 4;
            boolean cw = right != top;
            EnumFacing of = cw ? f.rotateY() : f.rotateYCCW();
            BlockPos at = bits >= 3 ? POS.offset(f) : POS.offset(f.getOpposite());
            map.put(at, b.getStateFromMeta(stairMeta(of, top)));
        } else if (b instanceof BlockDoor) {
            if ((meta & 8) == 0) {
                map.put(POS.up(), b.getStateFromMeta(8 | (bits & 1)));
            } else {
                EnumFacing f = HORIZ[bits & 3];
                // Metade de baixo: facing = getHorizontal(meta & 3).rotateYCCW().
                int lm = -1;
                for (int k = 0; k < 4; k++) if (HORIZ[k].rotateYCCW() == f) lm = k;
                if ((bits & 4) != 0) lm |= 4;
                map.put(POS.down(), b.getStateFromMeta(lm));
            }
        } else if (b instanceof BlockChest) {
            EnumFacing[] order = {null, EnumFacing.NORTH, EnumFacing.SOUTH, EnumFacing.WEST, EnumFacing.EAST};
            if (bits > 0) map.put(POS.offset(order[bits]), st);
        }
    }

    static String d(double v) {
        String s = Double.toString(v);
        return s;
    }

    public static void main(String[] args) throws Exception {
        Block.registerBlocks();
        // `Blocks` exige o Bootstrap; só os blocos interessam aqui.
        Field reg = net.minecraft.init.Bootstrap.class.getDeclaredField("alreadyRegistered");
        reg.setAccessible(true);
        reg.setBoolean(null, true);
        Field uf = Unsafe.class.getDeclaredField("theUnsafe");
        uf.setAccessible(true);
        Unsafe unsafe = (Unsafe) uf.get(null);
        Impl w = (Impl) unsafe.allocateInstance(Impl.class);
        w.map = new HashMap<>();

        List<double[]> boxes = new ArrayList<>();
        Map<String, Integer> boxIdx = new HashMap<>();
        List<int[]> shapes = new ArrayList<>();
        Map<String, Integer> shapeIdx = new HashMap<>();
        shapes.add(new int[0]);
        shapeIdx.put("", 0);

        int[] base = new int[4096];
        Arrays.fill(base, -1);
        List<int[]> entries = new ArrayList<>();
        int[] flags = new int[256];
        int[] light = new int[256];
        int[] valid = new int[256];
        List<String> problems = new ArrayList<>();

        // Estado de cada id<<4|meta como o jogo monta (BLOCK_STATE_IDS: o último estado válido com aquele meta vence).
        IBlockState[] byId = new IBlockState[4096];
        for (int id = 0; id < 256; id++) {
            Block b = Block.getBlockById(id);
            if (b == null || (id != 0 && b == Blocks.air)) continue;
            for (IBlockState s : b.getBlockState().getValidStates()) {
                int m = b.getMetaFromState(s);
                byId[id << 4 | m] = s;
            }
        }

        for (int id = 0; id < 256; id++) {
            Block b = Block.getBlockById(id);
            if (b == null || (id != 0 && b == Blocks.air)) continue;
            int f = 0;
            if (b.isOpaqueCube()) f |= 1;
            if (b.isFullCube()) f |= 2;
            if (b.isBlockNormalCube()) f |= 4;
            if (b.isTranslucent()) f |= 8;
            if (b.getUseNeighborBrightness()) f |= 16;
            if (b.getMaterial().isOpaque() && b.isFullCube()) f |= 32;
            if (b.isFullBlock()) f |= 64;
            if (b.getMaterial().isLiquid()) f |= 128;
            if (b.getMaterial().blocksMovement()) f |= 256;
            if (b.getMaterial().isSolid()) f |= 2048;
            // O raio da mira usa canCollideCheck(estado, false): ar e líquidos ficam de fora.
            if (b.canCollideCheck(b.getDefaultState(), false)) f |= 512;
            flags[id] = f;
            light[id] = b.getLightValue();
            int n = count(id);
            for (int m = 0; m < 16; m++) {
                IBlockState st = byId[id << 4 | m];
                if (st == null) continue;
                valid[id] |= 1 << m;
                base[id << 4 | m] = entries.size();
                for (int bits = 0; bits < n; bits++) {
                    w.map.clear();
                    w.map.put(POS, st);
                    neighbors(w, w.map, b, m, st, bits);
                    // Confere que a vizinhança dá a forma esperada (valida o port de getActualState).
                    if (b instanceof BlockStairs) {
                        int shape = ((BlockStairs.EnumShape) b.getActualState(st, w, POS).getValue(BlockStairs.SHAPE)).ordinal();
                        // EnumShape: STRAIGHT, INNER_LEFT, INNER_RIGHT, OUTER_LEFT, OUTER_RIGHT.
                        if (shape != bits) problems.add("escada " + id + ":" + m + " bits " + bits + " deu " + shape);
                    }
                    List<AxisAlignedBB> list = new ArrayList<>();
                    AxisAlignedBB mask = new AxisAlignedBB(POS.add(-3, -3, -3), POS.add(4, 4, 4));
                    b.setBlockBoundsBasedOnState(w, POS);
                    b.addCollisionBoxesToList(w, POS, st, mask, list, (Entity) null);
                    int[] shape = new int[list.size()];
                    StringBuilder key = new StringBuilder();
                    for (int i = 0; i < list.size(); i++) {
                        shape[i] = box(list.get(i), boxes, boxIdx);
                        key.append(shape[i]).append(',');
                    }
                    Integer si = shapeIdx.get(key.toString());
                    if (si == null) {
                        si = shapes.size();
                        shapes.add(shape);
                        shapeIdx.put(key.toString(), si);
                    }
                    b.setBlockBoundsBasedOnState(w, POS);
                    AxisAlignedBB sel = b.getSelectedBoundingBox(w, POS);
                    int sb = sel == null ? 65535 : box(sel, boxes, boxIdx);
                    entries.add(new int[] {si, sb});
                }
            }
        }

        // Variante de cada estado (vizinhos = ar) pelos mapeadores do próprio jogo.
        if (args.length > 1) {
            net.minecraft.client.renderer.BlockModelShapes bms = new net.minecraft.client.renderer.BlockModelShapes(null);
            Map<IBlockState, net.minecraft.client.resources.model.ModelResourceLocation> locs =
                bms.getBlockStateMapper().putAllStateModelLocations();
            PrintWriter vo = new PrintWriter(args[1], "UTF-8");
            vo.println("# Gerado por scripts/gen_shapes: id meta arquivo#variante (vizinhos = ar).");
            for (int id = 0; id < 256; id++) {
                for (int m = 0; m < 16; m++) {
                    IBlockState st = byId[id << 4 | m];
                    if (st == null) continue;
                    w.map.clear();
                    w.map.put(POS, st);
                    IBlockState act = st.getBlock().getActualState(st, w, POS);
                    net.minecraft.client.resources.model.ModelResourceLocation loc = locs.get(act);
                    String v = loc == null ? "-" : loc.getResourcePath() + "#" + loc.getVariant();
                    vo.println(id + " " + m + " " + v);
                }
            }
            vo.close();
        }

        PrintWriter out = new PrintWriter(args[0], "UTF-8");
        out.println("//! Gerado por `scripts/gen_shapes` executando o código da 1.8.9; não editar à mão.");
        out.println("//! Caixas em blocos, relativas ao canto do bloco.");
        out.println();
        out.println("pub const F_OPAQUE: u16 = 1;");
        out.println("pub const F_FULL_CUBE: u16 = 2;");
        out.println("pub const F_NORMAL_CUBE: u16 = 4;");
        out.println("pub const F_TRANSLUCENT: u16 = 8;");
        out.println("pub const F_NEIGHBOR_LIGHT: u16 = 16;");
        out.println("pub const F_OPAQUE_FULL: u16 = 32;");
        out.println("pub const F_FULL_BLOCK: u16 = 64;");
        out.println("pub const F_LIQUID: u16 = 128;");
        out.println("pub const F_SOLID: u16 = 256;");
        out.println("pub const F_COLLIDABLE: u16 = 512;");
        out.println("pub const F_MAT_SOLID: u16 = 2048;");
        out.println();
        out.println("/// Propriedades por id (isOpaqueCube, isFullCube, ...).");
        out.print("pub static FLAGS: [u16; 256] = [");
        for (int i = 0; i < 256; i++) out.print((i % 16 == 0 ? "\n    " : " ") + flags[i] + ",");
        out.println("\n];");
        out.println("/// `getLightValue` por id.");
        out.print("pub static LIGHT: [u8; 256] = [");
        for (int i = 0; i < 256; i++) out.print((i % 16 == 0 ? "\n    " : " ") + light[i] + ",");
        out.println("\n];");
        out.println("/// Metas válidos por id (bit m); os outros viram ar no jogo.");
        out.print("pub static VALID: [u16; 256] = [");
        for (int i = 0; i < 256; i++) out.print((i % 16 == 0 ? "\n    " : " ") + valid[i] + ",");
        out.println("\n];");
        out.println("pub static BOXES: [[f64; 6]; " + boxes.size() + "] = [");
        for (double[] bx : boxes) {
            out.print("    [");
            for (int i = 0; i < 6; i++) out.print(d(bx[i]) + (i < 5 ? ", " : ""));
            out.println("],");
        }
        out.println("];");
        int total = 0;
        for (int[] s : shapes) total += s.length;
        out.println("/// Formas de colisão: (início, quantidade) em `SHAPE_BOXES`.");
        out.println("pub static SHAPES: [(u16, u8); " + shapes.size() + "] = [");
        int start = 0;
        StringBuilder sbx = new StringBuilder();
        for (int[] s : shapes) {
            out.println("    (" + start + ", " + s.length + "),");
            for (int v : s) sbx.append(v).append(", ");
            start += s.length;
        }
        out.println("];");
        out.println("pub static SHAPE_BOXES: [u16; " + total + "] = [" + sbx + "];");
        out.println("/// Por estado `id << 4 | meta`: início em `ENTRIES` (u32::MAX = estado inválido).");
        out.print("pub static BASE: [u32; 4096] = [");
        for (int i = 0; i < 4096; i++) out.print((i % 16 == 0 ? "\n    " : " ") + (base[i] < 0 ? "u32::MAX" : Integer.toString(base[i])) + ",");
        out.println("\n];");
        out.println("/// (forma de colisão, caixa de seleção ou u16::MAX) por estado e vizinhança.");
        out.println("pub static ENTRIES: [(u16, u16); " + entries.size() + "] = [");
        StringBuilder line = new StringBuilder("   ");
        for (int[] e : entries) {
            line.append(" (").append(e[0]).append(", ").append(e[1]).append("),");
            if (line.length() > 100) {
                out.println(line);
                line = new StringBuilder("   ");
            }
        }
        if (line.length() > 3) out.println(line);
        out.println("];");
        out.close();
        System.err.println(boxes.size() + " caixas, " + shapes.size() + " formas, " + entries.size() + " entradas");
        for (String p : problems) System.err.println("AVISO: " + p);
    }

    static int box(AxisAlignedBB a, List<double[]> boxes, Map<String, Integer> idx) {
        double[] v = {a.minX - POS.getX(), a.minY - POS.getY(), a.minZ - POS.getZ(), a.maxX - POS.getX(), a.maxY - POS.getY(), a.maxZ - POS.getZ()};
        String k = Arrays.toString(v);
        Integer i = idx.get(k);
        if (i == null) {
            i = boxes.size();
            boxes.add(v);
            idx.put(k, i);
        }
        return i;
    }
}
