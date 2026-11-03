// Gera mc-client/src/gen_pick_block.rs rodando o código original (forgeBin 1.8.9): o item do
// Block.getPickBlock (getItem + getDamageValue) de cada estado, usado pelo clique do meio.
import java.io.*;
import java.lang.reflect.Field;
import net.minecraft.block.Block;
import net.minecraft.block.state.IBlockState;
import net.minecraft.item.Item;
import net.minecraft.item.ItemBlock;
import net.minecraft.util.BlockPos;
import net.minecraft.world.World;

public class GenPickBlock {
    /** Mundo de um bloco só: getBlockState devolve o estado da vez (criado sem construtor). */
    public static class OneBlock extends World {
        static IBlockState state;
        protected OneBlock() { super(null, null, null, null, false); }
        @Override public IBlockState getBlockState(BlockPos pos) { return state; }
        @Override protected net.minecraft.world.chunk.IChunkProvider createChunkProvider() { return null; }
        @Override protected int getRenderDistanceChunks() { return 0; }
    }

    public static void main(String[] args) throws Exception {
        Field reg = net.minecraft.init.Bootstrap.class.getDeclaredField("alreadyRegistered");
        reg.setAccessible(true);
        Block.registerBlocks();
        reg.setBoolean(null, true);
        Item.registerItems();
        Field uf = sun.misc.Unsafe.class.getDeclaredField("theUnsafe");
        uf.setAccessible(true);
        sun.misc.Unsafe unsafe = (sun.misc.Unsafe) uf.get(null);
        World world = null;
        StringBuilder rows = new StringBuilder();
        int n = 0;
        for (int id = 0; id < 4096; id++) {
            Block b = Block.getBlockById(id);
            if (b == null || (id != 0 && Block.getIdFromBlock(b) != id)) continue;
            for (int meta = 0; meta < 16; meta++) {
                IBlockState st;
                try { st = b.getStateFromMeta(meta); } catch (Throwable t) { continue; }
                if (b.getMetaFromState(st) != meta) continue;
                OneBlock.state = st;
                if (world == null) world = (World) unsafe.allocateInstance(OneBlock.class);
                Item item;
                int dmg;
                try {
                    item = b.getItem(world, BlockPos.ORIGIN);
                    if (item == null) continue;
                    Block from = item instanceof ItemBlock && !b.isFlowerPot() ? Block.getBlockFromItem(item) : b;
                    try { dmg = from.getDamageValue(world, BlockPos.ORIGIN); } catch (Throwable t) { dmg = 0; }
                } catch (Throwable t) { continue; }
                rows.append("    (").append(id << 4 | meta).append(", ").append(Item.getIdFromItem(item)).append(", ").append(dmg).append("),\n");
                n++;
            }
        }
        String o = "// Gerado por scripts/gen_pick_block/gen.sh a partir do forgeBin 1.8.9. Não editar à mão.\n\n"
            + "/// `Block.getPickBlock` por estado (id << 4 | meta): (estado, item, dano). " + n + " estados.\n"
            + "pub const PICK_BLOCK: &[(u16, i16, i16)] = &[\n" + rows + "];\n";
        try (Writer w = new OutputStreamWriter(new FileOutputStream(args[0]), "UTF-8")) { w.write(o); }
    }
}
