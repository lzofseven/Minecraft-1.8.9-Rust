# Paridade visual mine-rs x Minecraft 1.8.9 vanilla

Harness que tira prints iguais do cliente Java 1.8.9 vanilla e do mine-rs, na mesma pose,
no mesmo servidor local, e une os dois lado a lado. Serve para achar diferenças visuais.

## O que sobe

- **Servidor:** `minecraft_server.1.8.9.jar` oficial em `~/mine-rs-servidor-teste` (ou `PARIDADE_SERVIDOR`)
  (offline, 127.0.0.1:25577, `level-seed=1`, sem mobs naturais, `doDaylightCycle` desligado).
  O console recebe comandos por um FIFO (`server.stdin`).
- **Cliente Java:** `minecraft-1.8.9-client.jar` vanilla do PrismLauncher, classe
  `net.minecraft.client.main.Main`, sem mods, no display `:124`. Jogo em `/tmp/paridade-java`.
- **mine-rs:** `target/release/mc-client`, no display `:125`, `--offline`, nome `Teste2`.
  Configuração em `/tmp/paridade-rs` (`MINE_RS_CONFIG`).
- Os dois usam o mesmo `options.txt`: render distance 8, GUI scale 2, gráficos fancy,
  sem resource packs, idioma en_US, FOV 70.
- Jogadores: Java = `Teste`, mine-rs = `Teste2`.

Só 127.0.0.1 e só os displays `:124` e `:125`. Nunca `:0`. Nenhuma conta Microsoft é usada.

## Pré-requisitos

- Java 8 do PrismLauncher (`~/.local/share/PrismLauncher/java/jre-legacy`), jar do 1.8.9 e
  bibliotecas em `~/.local/share/PrismLauncher/libraries`.
- `Xvfb`, `xdotool`, `xwd`, `ffmpeg`, `python3`, `timeout`.
- Build do mine-rs: `cargo build --release -p mc-client`.

## Como rodar

```sh
scripts/paridade/paridade.sh tudo                    # servidor + mundo + clientes + todas as cenas
scripts/paridade/paridade.sh cenas 05_blocos 08_f3   # só algumas cenas
scripts/paridade/paridade.sh parar                   # derruba clientes e servidor
```

Outros comandos: `servidor`, `preparar` (refaz o mundo de teste), `clientes`, `spawn`.
Variáveis úteis: `PARIDADE_SERVIDOR`, `PARIDADE_RS_BIN`, `PARIDADE_SAIDA`, `PARIDADE_ESPERA`.

Saídas:
- `~/mine-rs-prints/paridade/NN_nome.png` (ou `PARIDADE_SAIDA`): Java à esquerda, mine-rs à direita.
- `/tmp/paridade-logs/brutas/`: os dois prints separados.
- `/tmp/paridade-logs/`: logs do servidor e dos clientes.

## Como cada cena funciona

Cada cena fica em `cenas.sh` como `cena_NN_nome`, com a pose `x y z yaw pitch` (igual para os dois),
comandos de console (hora, clima, entidades), teclas a apertar e teclas para desfazer.
`SPAWN` vira o ponto de entrada lido do log do servidor.

Para não ter os dois jogadores na tela um do outro, e sem mexer nos chunks:
1. os dois vão para espectador, 40 blocos acima da pose (`PARQUE_CENA`);
2. espera `PARIDADE_ESPERA` segundos para os chunks carregarem;
3. cada cliente desce para a pose, espera 3 s, aperta as teclas, tira o print e volta ao parque.

Para o Java, ao focar a janela, o harness move o mouse para o centro antes do print.
Sem isso o 1.8.9 pode abrir o menu de pausa (perda de foco).

## Limitações conhecidas

- **Bug do mine-rs:** ao teleportar para chunks ainda não carregados, o cliente pode abortar
  com `fs_overlay ... expects a BindGroup` (mira desenhada sem bind group 0). O harness evita
  isso com o parque acima da pose. A correção está sendo feita no master.
- Itens giram e flutuam pela idade do item; mobs e fogo animam. Diferenças de frame nesses
  elementos não são diferença de render.
- O F3 mostra FPS, memória e tempo de frame: sempre diferente.
