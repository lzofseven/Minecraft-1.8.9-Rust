// Gera mc-client/src/gen_achievements.rs rodando o código original (forgeBin 1.8.9): o ícone
// (theItemStack: id e meta) de cada conquista do AchievementList, para o GuiAchievement.
import java.io.*;
import java.lang.reflect.Field;
import net.minecraft.block.Block;
import net.minecraft.item.Item;
import net.minecraft.stats.Achievement;
import net.minecraft.stats.AchievementList;

public class GenAchievements {
    public static void main(String[] args) throws Exception {
        Field reg = net.minecraft.init.Bootstrap.class.getDeclaredField("alreadyRegistered");
        reg.setAccessible(true);
        Block.registerBlocks();
        reg.setBoolean(null, true);
        Item.registerItems();
        AchievementList.init();
        StringBuilder o = new StringBuilder();
        o.append("// Gerado por scripts/gen_achievements/gen.sh a partir do forgeBin 1.8.9. Não editar à mão.\n\n");
        o.append("/// `AchievementList`: id da estatística e ícone (`theItemStack`: id do item, meta).\n");
        o.append("pub const ICONS: &[(&str, i16, i16)] = &[\n");
        for (Achievement a : AchievementList.achievementList) {
            o.append(String.format("    (\"%s\", %d, %d),\n", a.statId, Item.getIdFromItem(a.theItemStack.getItem()), a.theItemStack.getMetadata()));
        }
        o.append("];\n");
        try (Writer w = new OutputStreamWriter(new FileOutputStream(args[0]), "UTF-8")) {
            w.write(o.toString());
        }
    }
}
