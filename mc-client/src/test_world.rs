//! Mundo de teste local (`--test-world`): um chão plano em memória, sem rede, para
//! conferir câmera, mão e modelos no Xvfb. Variáveis de ambiente opcionais:
//! `MINE_RS_TEST_HELD=id[:dano][@banner]` (item na mão), `MINE_RS_TEST_HOTBAR=id,id:dano,...` (próximas casas da barra), `MINE_RS_TEST_VIEW=0|1|2` (F5),
//! `MINE_RS_TEST_KEYS=w,ctrl,shift,space` (teclas presas), `MINE_RS_TEST_LOOK=yaw,pitch`,
//! `MINE_RS_TEST_SWING=1` (soca a cada 10 ticks), `MINE_RS_TEST_USE=1` (usa o item),
//! `MINE_RS_TEST_ARMOR=capacete,peito,calça,botas` (ids; 0 = vazio),
//! `MINE_RS_TEST_RADIUS=n` (tamanho do chão em chunks, padrão 3), `MINE_RS_TEST_BIOME=id` (bioma de todas as colunas).
//! `MINE_RS_TEST_HUD=gm=0,health=7,absorb=6,...` (ver `Hud::apply_test`).
//! `MINE_RS_TEST_CONQUISTA=achievement.id|dica` (0x37 inicial vazio e depois a conquista, ou a dica do inventário).
//! `MINE_RS_TEST_JANELA=tipo[@tamanho[@prop=valor,...]]` (abre a janela do S2D e aplica o S31; ex.
//! `minecraft:enchanting_table@0@0=5,1=12,2=30` para o livro aberto).
//! `MINE_RS_TEST_CAVALO=1` (inventário do primeiro cavalo de `MINE_RS_TEST_MOBS`, que precisa ter o 100).
//! `MINE_RS_TEST_BLOCOS=x,y,z,id[:meta];...` (blocos extras no chão de teste).
//! `MINE_RS_TEST_EQUIP=mão,botas,calça,peito,cabeça` (equipamento dos mobs de `MINE_RS_TEST_MOBS`).
//! `MINE_RS_TEST_MAPA=id` (0x34 de teste para o mapa `id`, ex. com `MINE_RS_TEST_OBJ=moldura:358:id:0`).
//! `MINE_RS_TEST_LACO=bx,by,bz` (nó de coleira no bloco; uma vaca presa a ele e um porco preso ao jogador).
//! `MINE_RS_TEST_FOGO=1` (o próprio jogador em chamas: bit 0 do metadado 0).
//! `MINE_RS_TEST_TAB=n[:hearts|:integer]` (n jogadores na lista do tab, com objetivo no espaço 0).
//! `MINE_RS_TEST_CAVERNAS=1` (pedra abaixo de y = 60, com cavernas e poços até a grama: seções escondidas para o culling).

use mc_protocol::nbt::Slot;
use mc_world::{ChunkData, Section, World};

/// Chão de grama em y = 63 (pés em 64) com pilares de pedra, vidro e um poço d'água.
pub fn build() -> World {
    let mut w = World::new(true);
    // `MINE_RS_TEST_RADIUS=n`: colunas de -n a n (padrão 3); 12 cobre a distância de 10 chunks.
    let biome = env("BIOME").and_then(|v| v.parse::<u8>().ok());
    let n = env("RADIUS").and_then(|v| v.parse::<i32>().ok()).unwrap_or(3).clamp(1, 40);
    let under = env("CAVERNAS").is_some();
    // MINE_RS_TEST_TERRENO=1: morros, água e árvores (medidas do LOD); a área em volta do jogador continua plana.
    if env("TERRENO").is_some() {
        for cx in -n..=n {
            for cz in -n..=n {
                w.apply(hills(cx, cz, biome));
            }
        }
        return w;
    }
    for cx in -n..=n {
        for cz in -n..=n {
            let mut s = Box::new(Section::empty(true));
            for z in 0..16 {
                for x in 0..16 {
                    s.blocks[Section::index(x, 12, z)] = 7 << 4;
                    s.blocks[Section::index(x, 13, z)] = 3 << 4;
                    s.blocks[Section::index(x, 14, z)] = 3 << 4;
                    s.blocks[Section::index(x, 15, z)] = 2 << 4;
                }
            }
            let (mask, sections) = if under { (0b1111, underground(cx, cz, &s)) } else { (1 << 3, vec![(3, s)]) };
            w.apply(ChunkData { x: cx, z: cz, full: true, mask, sections, biomes: biome.map(|b| Box::new([b; 256])) });
        }
    }
    for y in 64..68 {
        w.set_block(3, y, 6, 1 << 4);
        w.set_block(-3, y, 6, 4 << 4);
        w.set_block(0, y, 9, 20 << 4);
    }
    for x in -2..=2 {
        for z in 12..15 {
            w.set_block(x, 63, z, 9 << 4);
        }
    }
    w.set_block(1, 64, 4, 58 << 4);
    w.set_block(-1, 64, 4, 17 << 4);
    // MINE_RS_TEST_BLOCOS=x,y,z,id[:meta];...: blocos extras (tile entities saem do scan das colunas).
    for b in env("BLOCOS").unwrap_or_default().split(';') {
        let v: Vec<&str> = b.split(',').collect();
        if let [x, y, z, s] = v[..] {
            let (id, meta) = s.split_once(':').unwrap_or((s, "0"));
            if let (Ok(x), Ok(y), Ok(z), Ok(id), Ok(meta)) = (x.parse(), y.parse(), z.parse(), id.parse::<u16>(), meta.parse::<u16>()) {
                w.set_block(x, y, z, id << 4 | meta);
            }
        }
    }
    w
}

/// Altura do terreno de `MINE_RS_TEST_TERRENO` (senos somados; plano em y = 64 perto da origem).
fn hill_height(x: i32, z: i32) -> i32 {
    let (fx, fz) = (x as f32, z as f32);
    let h = 14.0 * (fx / 41.0).sin() * (fz / 37.0).cos() + 9.0 * ((fx + fz) / 23.0).sin() + 5.0 * (fx / 11.0 - fz / 13.0).cos() + 18.0 * (fx / 150.0).sin() * (fz / 130.0).sin();
    let near = ((fx * fx + fz * fz).sqrt() / 48.0).clamp(0.0, 1.0);
    (64.0 + h * near).round() as i32
}

fn tree_at(x: i32, z: i32) -> bool {
    let k = (x.wrapping_mul(73856093) ^ z.wrapping_mul(19349663)) as u32;
    k % 53 == 0 && x.rem_euclid(5) == 0 && z.rem_euclid(5) == 0 && x * x + z * z > 400
}

fn hills(cx: i32, cz: i32, biome: Option<u8>) -> ChunkData {
    let mut secs: Vec<Option<Box<Section>>> = (0..8).map(|_| None).collect();
    let mut put = |x: i32, y: i32, z: i32, st: u16| {
        if !(32..128).contains(&y) {
            return;
        }
        let s = secs[(y >> 4) as usize].get_or_insert_with(|| Box::new(Section::empty(true)));
        s.blocks[Section::index(x, y & 15, z)] = st;
    };
    for z in 0..16 {
        for x in 0..16 {
            let (wx, wz) = (cx * 16 + x, cz * 16 + z);
            let h = hill_height(wx, wz);
            for y in 32..h {
                let st = if y == h - 1 { if h <= 62 { 12 << 4 } else { 2 << 4 } } else if y >= h - 4 { 3 << 4 } else { 1 << 4 };
                put(x, y, z, st);
            }
            for y in h..62 {
                put(x, y, z, 9 << 4);
            }
            if h > 63 && (wx + wz * 3).rem_euclid(7) == 0 {
                put(x, h, z, 31 << 4 | 1);
            }
        }
    }
    // Árvores: tronco de 5 e copa 5x5 (só as que cabem na coluna).
    for z in 2..14 {
        for x in 2..14 {
            let (wx, wz) = (cx * 16 + x, cz * 16 + z);
            let h = hill_height(wx, wz);
            if h <= 63 || !tree_at(wx, wz) {
                continue;
            }
            for dy in 3..7 {
                let r = if dy >= 5 { 1 } else { 2 };
                for dz in -r..=r {
                    for dx in -r..=r {
                        put(x + dx, h + dy, z + dz, 18 << 4);
                    }
                }
            }
            for dy in 0..5 {
                put(x, h + dy, z, 17 << 4);
            }
        }
    }
    let mut mask = 0u16;
    let mut sections = Vec::new();
    for (i, s) in secs.into_iter().enumerate() {
        if let Some(s) = s {
            mask |= 1 << i;
            sections.push((i, s));
        }
    }
    ChunkData { x: cx, z: cz, full: true, mask, sections, biomes: biome.map(|b| Box::new([b; 256])) }
}

/// Colunas de `MINE_RS_TEST_CAVERNAS`: pedra de y = 0 a 59; a seção 3 (y 48 a 63) mantém a grama e a terra de `top`.
fn underground(cx: i32, cz: i32, top: &Section) -> Vec<(usize, Box<Section>)> {
    (0..4usize)
        .map(|sy| {
            let mut s = Box::new(Section::empty(true));
            for y in 0..16 {
                for z in 0..16 {
                    for x in 0..16 {
                        let (i, wy) = (Section::index(x, y, z), sy as i32 * 16 + y);
                        s.blocks[i] = if cave(cx * 16 + x, wy, cz * 16 + z) {
                            0
                        } else if sy == 3 && y >= 12 {
                            top.blocks[i]
                        } else if wy < 60 {
                            1 << 4
                        } else {
                            0
                        };
                    }
                }
            }
            (sy, s)
        })
        .collect()
}

/// Ar das cavernas: uma esfera de raio 4 por chunk (centro entre y 20 e 31) e, em metade dos chunks, um poço até a grama.
fn cave(x: i32, y: i32, z: i32) -> bool {
    let (cx, cz) = (x.div_euclid(16), z.div_euclid(16));
    let (mx, mz) = (cx * 16 + 8, cz * 16 + 8);
    let my = 20 + (cx * 5 + cz * 3).rem_euclid(12);
    let (dx, dy, dz) = (x - mx, y - my, z - mz);
    dx * dx + dy * dy + dz * dz <= 16 || (x == mx && z == mz && y > my && (cx + cz).rem_euclid(2) == 0)
}

pub fn env(name: &str) -> Option<String> {
    std::env::var(format!("MINE_RS_TEST_{name}")).ok().filter(|v| !v.is_empty())
}

/// `id[:dano][@banner]`: o `@banner` põe a `BlockEntityTag` de um estandarte com padrões (`Base` e `Patterns`).
/// `MINE_RS_TEST_EQUIP=mão,botas,calça,peito,cabeça` (`id[:dano]`, `-` vazio): equipamento dos mobs de teste.
pub fn equipment() -> [Option<Slot>; 5] {
    let v = env("EQUIP").unwrap_or_default();
    let mut out: [Option<Slot>; 5] = Default::default();
    for (i, s) in v.split(',').take(5).enumerate() {
        out[i] = parse_slot(s);
    }
    out
}

fn parse_slot(v: &str) -> Option<Slot> {
    use mc_protocol::nbt::Nbt;
    let (v, tag) = v.split_once('@').map_or((v, ""), |(a, b)| (a, b));
    let mut it = v.split(':');
    let id = it.next()?.trim().parse().ok()?;
    let damage = it.next().and_then(|d| d.parse().ok()).unwrap_or(0);
    let compound = |pairs: Vec<(&str, Nbt)>| Nbt::Compound(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect());
    let nbt = match tag {
        "banner" => {
            let pat = |id: &str, color: i32| compound(vec![("Pattern", Nbt::String(id.into())), ("Color", Nbt::Int(color))]);
            let entity = compound(vec![("Base", Nbt::Int(4)), ("Patterns", Nbt::List(vec![pat("ms", 15), pat("cbo", 1), pat("flo", 14)]))]);
            Some(compound(vec![("BlockEntityTag", entity)]))
        }
        _ => None,
    };
    Some(Slot { id, count: 1, damage, nbt, raw_nbt: Vec::new() })
}

/// Item de `MINE_RS_TEST_HELD`.
pub fn held() -> Option<Slot> {
    parse_slot(&env("HELD")?)
}

/// Itens de `MINE_RS_TEST_HOTBAR` (lista separada por vírgulas, mesmo formato do `HELD`), depois da mão.
pub fn hotbar() -> Vec<Slot> {
    env("HOTBAR").map(|v| v.split(',').filter_map(parse_slot).collect()).unwrap_or_default()
}

/// Mensagens de `MINE_RS_TEST_CHAT`: `demo` (exemplos embutidos) ou `@arquivo` (um JSON por linha).
/// Cada linha é o JSON que o servidor mandaria no S02.
pub fn chat_messages() -> Vec<String> {
    let Some(v) = env("CHAT") else { return Vec::new() };
    if let Some(path) = v.strip_prefix('@') {
        return std::fs::read_to_string(path).map(|t| t.lines().filter(|l| !l.trim().is_empty()).map(str::to_string).collect()).unwrap_or_default();
    }
    CHAT_DEMO.iter().map(|s| s.to_string()).collect()
}

/// Exemplos de componentes: cores e estilos, herança, link, comandos, hover (texto, item, conquista) e linhas longas.
const CHAT_DEMO: &[&str] = &[
    r#"{"translate":"chat.type.text","with":["Lohan","olá, mundo"]}"#,
    r#"{"text":"","extra":[{"text":"Cores: ","color":"white"},{"text":"vermelho ","color":"red"},{"text":"verde ","color":"green"},{"text":"azul ","color":"blue"},{"text":"ouro","color":"gold"}]}"#,
    r#"{"text":"Estilos: ","extra":[{"text":"negrito ","bold":true},{"text":"itálico ","italic":true},{"text":"sublinhado ","underlined":true},{"text":"tachado ","strikethrough":true},{"text":"ofuscado","obfuscated":true}]}"#,
    r#"{"text":"Herança: ","color":"aqua","bold":true,"extra":[{"text":"filho herda cor e negrito, "},{"text":"este troca a cor","color":"light_purple"},{"text":" e este tira o negrito","bold":false}]}"#,
    r#"{"text":"Clique para abrir ","color":"yellow","extra":[{"text":"[link]","color":"aqua","underlined":true,"clickEvent":{"action":"open_url","value":"https://example.com/pagina"},"hoverEvent":{"action":"show_text","value":{"text":"Abre example.com","color":"green"}}}]}"#,
    r#"{"text":"[Home] ","color":"green","clickEvent":{"action":"run_command","value":"/home"},"hoverEvent":{"action":"show_text","value":{"text":"Executa /home\nsegunda linha","color":"gray"}},"extra":[{"text":"[Sugerir /tp] ","color":"gold","clickEvent":{"action":"suggest_command","value":"/tp Lohan"}},{"text":"[Inserir] ","color":"light_purple","insertion":"texto inserido"}]}"#,
    r#"{"text":"Item: ","extra":[{"text":"[Espada]","color":"aqua","hoverEvent":{"action":"show_item","value":{"text":"{id:\"minecraft:diamond_sword\",Count:1b,Damage:0s,tag:{display:{Name:\"Excalibur\",Lore:[\"Forjada em testes\"]}}}"}}},{"text":" [Conquista]","color":"green","hoverEvent":{"action":"show_achievement","value":{"text":"achievement.mineWood"}}}]}"#,
    r#"{"text":"Esta é uma linha bem longa para testar a quebra por largura do chat: palavras e mais palavras seguidas, com cor que continua na linha de baixo e um link no fim ","color":"green","extra":[{"text":"https://example.com/um/caminho/bem/longo/para/quebrar","color":"aqua","underlined":true,"clickEvent":{"action":"open_url","value":"https://example.com/um/caminho/bem/longo"}}]}"#,
    r#"{"translate":"multiplayer.player.joined","with":["Lohan"],"color":"yellow"}"#,
];
