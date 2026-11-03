# mine-rs

Cliente do Minecraft Java 1.8.9 escrito em Rust: rede, login online, mundo, física, renderização (wgpu/Vulkan), HUD, inventários e menus, portados do jogo original para jogar em servidores 1.8.

O objetivo é o jogo vanilla com desempenho alto. Não há mods de interface ou de jogabilidade. As únicas opções extras são as gráficas do OptiFine (veja abaixo).

## Compilar

Precisa do Rust estável e de uma placa com Vulkan.

```bash
cargo build --release -p mc-client
```

O binário fica em `target/release/mc-client`.

## Jogar

```bash
target/release/mc-client
```

Na primeira vez, o jogo usa o Minecraft 1.8.9 de um launcher instalado (`~/.minecraft` ou Prism Launcher). Sem nenhum, ele baixa o jar e os sons dos servidores oficiais da Mojang para `~/.local/share/mine-rs`, conferindo o SHA-1 de cada arquivo.

Para entrar direto num servidor:

```bash
target/release/mc-client mc.exemplo.com
```

### Conta

Sem conta salva, o jogo abre a tela **Conta** (no menu principal ela fica no botão "Conta"):

- **Entrar com Microsoft:** o jogo mostra um link e um código. Abra o link, digite o código e aprove. A conta fica salva em `~/.config/mine-rs/conta.json` (só o seu usuário lê) e o token é renovado sozinho.
- **Jogar offline:** escolha um nick (3 a 16 letras, números ou `_`).

O login Microsoft precisa do Client ID de um app seu no Azure com acesso à API do Minecraft. Para informar, use a variável `MINE_RS_MS_CLIENT_ID` ou o arquivo `~/.config/mine-rs/microsoft-client-id`.

Pela linha de comando:

| Opção | O que faz |
| --- | --- |
| `--offline --name NICK` | joga offline com o nick |
| `--session auth.json` | usa a sessão de um launcher (JSON com `name`, `uuid` e `access_token`) |
| `--jar minecraft-1.8.9.jar` | usa outro jar do jogo |

## Opções gráficas do OptiFine

Se existir `~/.config/mine-rs/optionsof.txt` (o mesmo formato do OptiFine), o jogo lê:

| Opção | Valor | Efeito |
| --- | --- | --- |
| `ofFogType` | `3` | sem névoa de distância |
| `ofSky`, `ofStars`, `ofSunMoon` | `false` | sem céu, estrelas ou sol e lua |
| `ofClouds` | `3` | sem nuvens |
| `ofRain` | `3` | sem chuva |
| `ofWeather` | `false` | sem chuva nem trovão |
| `ofDynamicFov` | `false` | o FOV não muda com corrida e arco |
| `ofAnimatedWater`, `ofAnimatedLava` | `2` | texturas paradas |
| `ofAnimatedFire` | `false` | fogo parado |
| `ofVignette` | `1` ou `2` | vinheta desligada ou ligada |

As outras opções (`options.txt`, `servers.dat`) ficam em `~/.config/mine-rs`. `MINE_RS_CONFIG` troca essa pasta.

## Estrutura

| Pasta | Conteúdo |
| --- | --- |
| `mc-protocol` | protocolo 47 (1.8): pacotes, NBT, slots |
| `mc-net` | conexão e criptografia |
| `mc-world` | chunks, blocos, malhas e luz |
| `mc-render` | renderizador wgpu, atlas, modelos, UI |
| `mc-client` | o jogo: física, entidades, HUD, inventários, menus |
| `scripts` | geradores das tabelas a partir do código do jogo descompilado (`.local/src`, que não vai no repositório) |
