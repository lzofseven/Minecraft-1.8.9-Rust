# Port do Minecraft 1.8.9 para Rust (mine-rs)

Port no estilo do Horizon: o código Java decompilado da cópia do próprio jogador
fica só em `.local/` (fora do git) e serve de referência para reproduzir o
comportamento em Rust. Nada da Mojang é publicado; as texturas são lidas do jar
local em tempo de execução.

## Regra principal: nada recriado à mão

Tudo sai do original. Dados vêm do jar (modelos JSON de bloco e item, texturas, `.mcmeta`,
`sounds.json`, `pt_BR.lang`, fonte). O que na 1.8 só existe como código (modelos de mob,
colisões, dureza, cores, física, layouts de tela) é portado com os números exatos do Java
decompilado ou gerado por script em `scripts/` a partir de `.local/src`. Nada de aproximação,
tabela inventada ou texto de interface fixo.

Ainda feitos à mão e a substituir: formas e colisões em `mc-world/src/blocks.rs`
(→ modelos JSON e `setBlockBounds`), textos fixos da GUI (→ `pt_BR.lang`). Itens (modelos, cores,
dureza/ferramentas) e traduções do chat já saem do original: `scripts/gen_items/gen.sh` gera
`mc-world/src/gen_items.rs` e `gen_tools.rs` rodando as classes do forgeBin; ícones vêm de
`models/item/*.json` do jar.

## Como gerar a referência local

1. `forgeBin-1.8.9-11.15.1.2318-1.8.9.jar` (nomes MCP stable_22) vem do cache do
   ForgeGradle (`~/.gradle/caches/minecraft/...`).
2. Decompilar com o Fernflower do mesmo cache para `.local/decompiled/` e extrair
   em `.local/src/` (2.171 arquivos `.java`).
3. Indexar `.local/src` no codebase-memory como `mc189-src`.

## Escopo atual: cliente multiplayer

Legenda: ✅ portado e testado · 🟡 parcial · ⬜ falta

| Sistema | Referência (net.minecraft.…) | mine-rs | Status |
|---|---|---|---|
| Protocolo 47, VarInt, framing, compressão | network.PacketBuffer, NettyCompressionEncoder | mc-protocol, mc-net | ✅ |
| Login online, criptografia AES/CFB8 | client.network.NetHandlerLoginClient, util.CryptManager | mc-client/auth.rs | ✅ |
| Keep-alive, transações (anti-lag) | client.network.NetHandlerPlayClient | mc-client/net.rs | ✅ |
| Chunks 1.8 (0x21, 0x26), mudanças de bloco | world.chunk.Chunk, S21/S26/S22/S23 | mc-world/chunk.rs | ✅ |
| Formas de bloco e texturas | block.*, client.renderer.BlockModelShapes | mc-world/blocks.rs, model.rs, gen_shapes.rs | ✅ (modelos JSON do jar; colisões geradas) |
| Malha, luz suave, AO | client.renderer.BlockModelRenderer | mc-world/mesh.rs | ✅ |
| Céu, neblina, dia/noite | world.WorldProvider, client.renderer.EntityRenderer | renderer.rs, game.rs | ✅ (domo, sol/lua, estrelas, nuvens, neblina, lightmap, chuva/neve) |
| Física do jogador | entity.Entity.moveEntity, EntityLivingBase.moveEntityWithHeading | mc-client/physics.rs | ✅ |
| Envio de movimento | client.entity.EntityPlayerSP.onUpdateWalkingPlayer | game.rs | ✅ |
| HUD (hotbar, vida, fome, xp) | client.gui.GuiIngame | mc-client/hud.rs | ✅ (ícones de item pendentes) |
| Chat na tela e digitação | client.gui.GuiNewChat, GuiChat, GuiConfirmOpenLink | mc-client/chat.rs, chat_gui.rs, hud.rs | ✅ (componentes, cliques, dicas, Tab, histórico) |
| Placar lateral, times, tab, títulos | scoreboard.*, client.gui.GuiPlayerTabOverlay | mc-client/hud.rs, tab_list.rs | ✅ (tab com cabeças, objetivo do espaço 0, corações) |
| Tela F3 e lagômetro | client.gui.GuiOverlayDebug, util.FrameTimer | mc-client/debug_overlay.rs, gen_debug.rs | ✅ (sem gráfico do profiler) |
| Aviso de conquista | client.gui.achievement.GuiAchievement, NetHandlerPlayClient.handleStatistics | mc-client/achievement_toast.rs, gen_achievements.rs | ✅ |
| Fonte (ascii + unicode) | client.gui.FontRenderer | mc-render/ui.rs | ✅ |
| Jogadores, skins, nomes | client.renderer.entity.RenderPlayer, model.ModelPlayer | mc-client/entities.rs, models.rs, skins.rs | ✅ |
| Outras entidades | client.renderer.entity.* | mc-client/mobs/, naovivas.rs, fx.rs | ✅ (todos os mobs e objetos; pendências em HANDOFF-mobs.md) |
| Mirar, quebrar, colocar, usar item, NPCs | client.multiplayer.PlayerControllerMP, EntityRenderer.getMouseOver | mc-client/interact.rs, game.rs | ✅ |
| Inventário, containers, criativo, placa, livro, cama, morte | client.gui.inventory.*, inventory.Container, GuiGameOver | mc-client/inventory.rs, containers.rs, gui_creative.rs, gui_special.rs, menu.rs | ✅ |
| Itens (ícones), NBT e nomes pt_BR | client.renderer.RenderItem, nbt.*, Item.registerItems | mc-render/items.rs, mc-world/items.rs, mc-client/lang.rs | 🟡 (baú, cabeça e estandarte por `TileEntityItemStackRenderer` ✅; bússola/relógio animados) |
| Sons | client.audio.SoundHandler, SoundManager | mc-client/sound.rs | 🟡 (sem categorias de volume e música) |
| Partículas (EffectRenderer, S2A/S27/S28/S0B, randomDisplayTick) | client.particle.*, RenderGlobal.playAuxSFX/spawnEntityFX, Explosion.doExplosionB | mc-client/particles.rs | 🟡 (ver pendências em HANDOFF) |
| Menu de servidores, pausa, desconexão, opções | client.gui.GuiMultiplayer, GuiIngameMenu, GuiDisconnected, GuiOptions | mc-client/menu.rs | ✅ |

## Próximos passos (em ordem)

1. Desempenho: reaproveitar buffers por quadro, vértice compacto (28 → 16 bytes), culling por oclusão de seções, medir com MINE_RS_STATS.
2. Paridade visual: modelos de bloco lidos dos JSON da 1.8 (`assets/minecraft/models/block`) no lugar das formas à mão; animação de água/lava/fogo; nuvens e estrelas; brilho animado.
3. Entidades: armadura e item na mão, nomes atravessando paredes, mais modelos de mob.
4. Menu de servidores (servers.dat) e tela de desconexão; menu de pausa.
5. Conferir cada sistema contra o Java (comparação de prints lado a lado no Xvfb).

Testes visuais rodam no display virtual (`Xvfb :99`), nunca no display do usuário.

## Frentes em andamento (30/10/2026)

| Branch / worktree | Escopo | Estado |
|---|---|---|
| `port/modelos` | Modelos de bloco JSON, meta→variante, bake, AO por face, colisões geradas | ✅ no master |
| `port/jogador` | EntityPlayerSP completo (voo/shift, sprint, agachar), câmera/F5/bobbing, mão e item em 1ª pessoa, armadura e item na 3ª pessoa, cache de skins | ✅ no master |
| `port/opcoes` | Menus, opções, options.txt, servers.dat | ✅ no master |
| `port/itens` | Ícones e item na mão por `models/item`, cores de camada, dureza/ferramentas, chat pelo lang | ✅ no master |
| `port/particulas`, `port/animacoes`, `port/ceu` | Partículas; texturas animadas, fluidos, bioma; céu, nuvens, neblina, chuva/neve | ✅ no master (bússola/relógio na mão pendente) |
| `port/tileentities` | Baús, placas, cabeças, estandartes (tile entities) e itens `builtin/entity` (ícone, mão, 3ª pessoa) | ✅ no master |
| `port/animacoes` | Texturas animadas (.mcmeta, bússola, relógio), BlockFluidRenderer, cores de bioma | ✅ pronto para merge (ver abaixo) |

Bugs relatados pelo dono (31/10): skins demoram, shift não para o voo, mão não aparece, Esc com só 2 opções.
Regras: testes visuais só no Xvfb :99; uma conexão com a conta por vez; limite de uso 60% semanal.

Depois do merge: animações de textura (.mcmeta), nuvens/estrelas/sol e lua com textura, partículas, nomes através de paredes, efeitos de poção na tela, otimização (buffers reaproveitados, vértice compacto, culling por oclusão), e comparação lado a lado com o Java no Xvfb.

## Frente `port/animacoes` (30/10/2026)

- **Biomas** (`mc-world/src/biome.rs`, `gen_biomes.rs`): tabela de `BiomeGenBase` e 732 vetores de cor gerados executando o
  código original (`scripts/gen_biomes/gen.sh`); ruído simplex/Perlin e `java.util.Random` portados; `ColorizerGrass`/`Foliage`
  com `colormap/*.png` do jar; média 3x3 do `BiomeColorHelper`. Os biomas das colunas (0x21/0x26) agora são guardados
  (`Column.biomes`) e copiados para `Padded.biomes`. Grama, folha (carvalho/selva/acácia/carvalho-escuro), samambaia, cana,
  planta dupla e vinha usam a cor do bioma; abeto e bétula seguem fixos como no jogo.
- **Fluidos** (`mc-world/src/fluid.rs`, `math.rs`): `BlockFluidRenderer.renderFluid` (altura por canto, fluxo com `atan2` e
  `sin/cos` de `MathHelper`, faces laterais com `water_flow`/`lava_flow`, luz de `getMixedBrightnessForBlock`, tinta de água
  por bioma). `Material.isSolid` entra pelo gerador de formas (`F_MAT_SOLID`).
- **Animações** (`mc-render/src/anim.rs`): `AnimationMetadataSection` + `updateAnimation` (frametime, frames com tempo próprio,
  interpolate), regravando só o tile e os mipmaps no atlas (`Renderer::write_atlas_tile`) a cada tick do jogo. Bússola e
  relógio (`TextureCompass`/`TextureClock`) regravam o ícone no atlas da GUI. Spawn Position (0x05) agora é lido.
- Gancho de teste: `MINE_RS_DEMO=1` abre um mundo local (planície/pântano, piscina, fluxo, lava, folhas) sem servidor.
- Pendências: tiles do atlas continuam 16 px (water_flow/lava_flow 32 px são reduzidos por amostragem); ícones de bússola e
  relógio só no atlas da GUI (a mão e os modelos de item da frente do jogador precisam ler `ItemIcons.live`); mipmap das
  animações é média 2x2 simples (o Java usa `generateMipmapData` com mistura de alfa); cor de bioma de blocos fora do mapa
  carregado usa planície.
