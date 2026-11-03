#!/usr/bin/env bash
# Harness de paridade visual: mine-rs (display :125) contra o cliente Java 1.8.9 vanilla (:124).
# Os dois entram no mesmo servidor 1.8.9 local (127.0.0.1, offline). Em cada cena, cada cliente
# é levado para a mesma pose, tira um print do próprio display e os dois são unidos lado a lado
# (Java à esquerda, mine-rs à direita). Ver README.md ao lado deste arquivo.
#
# Uso: paridade.sh {tudo|servidor|clientes|preparar|spawn|cenas [nome...]|parar}
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$DIR/../.." && pwd)"
# shellcheck source=cenas.sh
. "$DIR/cenas.sh"

SERVIDOR="${PARIDADE_SERVIDOR:-$HOME/Documentos/Projetos/mine-rs-servidor-teste}"
JAVA_DISPLAY="${PARIDADE_JAVA_DISPLAY:-:124}"
RS_DISPLAY="${PARIDADE_RS_DISPLAY:-:125}"
JAVA_JOGO="${PARIDADE_JAVA_JOGO:-/tmp/paridade-java}"
RS_CONFIG="${PARIDADE_RS_CONFIG:-/tmp/paridade-rs}"
RS_BIN="${PARIDADE_RS_BIN:-$REPO/target/release/mc-client}"
LOGS="${PARIDADE_LOGS:-/tmp/paridade-logs}"
SAIDA="${PARIDADE_SAIDA:-$HOME/Documentos/Projetos/mine-rs-prints/paridade}"
ESPERA="${PARIDADE_ESPERA:-10}"

PORTA=25577
JOGADOR_JAVA=Teste
JOGADOR_RS=Teste2
# Onde fica o jogador que não está em cena: espectador, longe e alto, fora da visão e do rastreio.
PARQUE="-300 250 -300"

MC="$HOME/.local/share/PrismLauncher"
JAVA_BIN="$MC/java/jre-legacy/bin/java"
LIBS="$MC/libraries"
NATIVOS="$MC/instances/1.8.9/natives"
ASSETS="$MC/assets"
MC_JAR="$LIBS/com/mojang/minecraft/1.8.9/minecraft-1.8.9-client.jar"
# Bibliotecas do 1.8.9 vanilla (as do launcher, sem Forge nem launchwrapper).
BIBLIOTECAS=(
  org/ow2/asm/asm-all/5.0.3/asm-all-5.0.3.jar
  org/lwjgl/lwjgl/lwjgl/2.9.4-nightly-20150209/lwjgl-2.9.4-nightly-20150209.jar
  org/lwjgl/lwjgl/lwjgl_util/2.9.4-nightly-20150209/lwjgl_util-2.9.4-nightly-20150209.jar
  org/lwjgl/lwjgl/lwjgl-input/2.9.4-nightly-20150209/lwjgl-input-2.9.4-nightly-20150209.jar
  org/lwjgl/lwjgl/lwjgl-opengl/2.9.4-nightly-20150209/lwjgl-opengl-2.9.4-nightly-20150209.jar
  org/lwjgl/lwjgl/lwjgl-openal/2.9.4-nightly-20150209/lwjgl-openal-2.9.4-nightly-20150209.jar
  org/lwjgl/lwjgl/lwjgl-stb/2.9.4-nightly-20150209/lwjgl-stb-2.9.4-nightly-20150209.jar
  org/lwjgl/lwjgl/lwjgl-tinyfd/2.9.4-nightly-20150209/lwjgl-tinyfd-2.9.4-nightly-20150209.jar
  com/google/guava/guava/17.0/guava-17.0.jar
  com/mojang/authlib/1.5.21/authlib-1.5.21.jar
  com/mojang/realms/1.7.59/realms-1.7.59.jar
  io/netty/netty-all/4.0.23.Final/netty-all-4.0.23.Final.jar
  net/sf/jopt-simple/jopt-simple/4.6/jopt-simple-4.6.jar
  org/apache/commons/commons-lang3/3.3.2/commons-lang3-3.3.2.jar
  commons-io/commons-io/2.4/commons-io-2.4.jar
  commons-codec/commons-codec/1.9/commons-codec-1.9.jar
  com/google/code/gson/gson/2.2.4/gson-2.2.4.jar
  com/paulscode/soundsystem/20120107/soundsystem-20120107.jar
  com/paulscode/libraryjavasound/20101123/libraryjavasound-20101123.jar
  com/paulscode/librarylwjglopenal/20100824/librarylwjglopenal-20100824.jar
  com/paulscode/codecjorbis/20101023/codecjorbis-20101023.jar
  com/paulscode/codecwav/20101023/codecwav-20101023.jar
  jline/jline/2.13/jline-2.13.jar
  org/apache/logging/log4j/log4j-api/2.0-beta9-fixed/log4j-api-2.0-beta9-fixed.jar
  org/apache/logging/log4j/log4j-core/2.0-beta9-fixed/log4j-core-2.0-beta9-fixed.jar
  org/apache/commons/commons-compress/1.8.1/commons-compress-1.8.1.jar
  org/apache/httpcomponents/httpclient/4.3.3/httpclient-4.3.3.jar
  commons-logging/commons-logging/1.1.3/commons-logging-1.1.3.jar
  org/apache/httpcomponents/httpcore/4.3.2/httpcore-4.3.2.jar
  tv/twitch/twitch/6.5/twitch-6.5.jar
  com/mojang/netty/1.8.8/netty-1.8.8.jar
  oshi-project/oshi-core/1.1/oshi-core-1.1.jar
  net/java/dev/jna/jna/3.4.0/jna-3.4.0.jar
  net/java/dev/jna/platform/3.4.0/platform-3.4.0.jar
  com/ibm/icu/icu4j-core-mojang/51.2/icu4j-core-mojang-51.2.jar
  lzma/lzma/0.0.1/lzma-0.0.1.jar
  java3d/vecmath/1.5.2/vecmath-1.5.2.jar
  net/sf/trove4j/trove4j/3.0.3/trove4j-3.0.3.jar
)

log() { printf '[paridade] %s\n' "$*" >&2; }
die() { log "erro: $*"; exit 1; }

# --- utilidades -----------------------------------------------------------

processo_ativo() { # nome (servidor, java, rs)
  [[ -f "$LOGS/$1.pid" ]] && kill -0 "$(cat "$LOGS/$1.pid")" 2>/dev/null
}

esperar_linha() { # arquivo texto segundos
  local i
  for ((i = 0; i < $3; i++)); do
    grep -qF -- "$2" "$1" 2>/dev/null && return 0
    sleep 1
  done
  return 1
}

esperar_fim() { # nome segundos
  local i
  for ((i = 0; i < $2; i++)); do
    processo_ativo "$1" || return 0
    sleep 1
  done
  return 1
}

garantir_display() { # :N (nunca :0)
  local n=${1#:}
  [[ "$1" == ":0" ]] && die "não uso o display :0"
  [[ -S /tmp/.X11-unix/X$n ]] && return 0
  setsid nohup Xvfb "$1" -screen 0 1280x720x24 -nolisten tcp >/dev/null 2>&1 </dev/null &
  sleep 2
  [[ -S /tmp/.X11-unix/X$n ]] || die "não consegui subir o display $1"
}

# Console do servidor: escreve no FIFO que o próprio servidor lê (ver _servidor).
servidor_cmd() {
  timeout 5 bash -c 'printf "%s\n" "$1" > "$2"' _ "$1" "$SERVIDOR/server.stdin" \
    || log "comando não entregue ao servidor: $1"
}

# --- servidor -------------------------------------------------------------

servidor_escrever_config() {
  mkdir -p "$SERVIDOR"
  printf 'eula=true\n' > "$SERVIDOR/eula.txt"
  cat > "$SERVIDOR/server.properties" <<'EOF'
online-mode=false
server-ip=127.0.0.1
server-port=25577
level-name=world
level-seed=1
level-type=DEFAULT
generate-structures=true
gamemode=1
force-gamemode=false
spawn-protection=0
spawn-monsters=false
spawn-animals=false
spawn-npcs=false
difficulty=1
view-distance=8
allow-flight=true
allow-nether=false
max-players=4
pvp=false
white-list=false
enable-command-block=true
enable-query=false
enable-rcon=false
motd=Paridade mine-rs
EOF
  # PARIDADE_VD: view-distance do servidor (o benchmark usa 10).
  sed -i "s/^view-distance=.*/view-distance=${PARIDADE_VD:-8}/" "$SERVIDOR/server.properties"
  sed -i "s/^pvp=.*/pvp=${PARIDADE_PVP:-false}/" "$SERVIDOR/server.properties"
  # ops.json com o UUID offline (MD5 de "OfflinePlayer:<nome>"), como o servidor offline calcula.
  python3 -I - "$SERVIDOR/ops.json" "$JOGADOR_JAVA" "$JOGADOR_RS" <<'PY'
import hashlib, json, sys, uuid

def offline(nome):
    h = bytearray(hashlib.md5(("OfflinePlayer:" + nome).encode()).digest())
    h[6] = (h[6] & 0x0F) | 0x30
    h[8] = (h[8] & 0x3F) | 0x80
    return str(uuid.UUID(bytes=bytes(h)))

saida, nomes = sys.argv[1], sys.argv[2:]
with open(saida, "w") as f:
    json.dump([{"uuid": offline(n), "name": n, "level": 4} for n in nomes], f, indent=2)
PY
}

servidor_iniciar() {
  [[ -f "$SERVIDOR/minecraft_server.1.8.9.jar" ]] || die "falta $SERVIDOR/minecraft_server.1.8.9.jar"
  if processo_ativo servidor; then
    log "servidor já está rodando"
    return 0
  fi
  servidor_escrever_config
  if [[ ! -p "$SERVIDOR/server.stdin" ]]; then
    rm -f "$SERVIDOR/server.stdin"
    mkfifo "$SERVIDOR/server.stdin"
  fi
  : > "$LOGS/servidor.log"
  rm -f "$LOGS/spawn.txt"
  (cd "$SERVIDOR" && setsid nohup "$DIR/paridade.sh" _servidor </dev/null >/dev/null 2>&1 &)
  esperar_linha "$LOGS/servidor.log" 'Done (' 240 || die "servidor não ficou pronto; veja $LOGS/servidor.log"
  log "servidor pronto em 127.0.0.1:$PORTA"
}

servidor_parar() {
  processo_ativo servidor || return 0
  servidor_cmd stop
  esperar_fim servidor 60 || kill "$(cat "$LOGS/servidor.pid")" 2>/dev/null || true
  log "servidor parado"
}

mundo_preparar() {
  log "montando o mundo de teste"
  servidor_cmd "gamerule doDaylightCycle false"
  servidor_cmd "gamerule doMobSpawning false"
  servidor_cmd "gamerule doWeatherCycle false"
  local cmd
  while IFS= read -r cmd; do
    servidor_cmd "$cmd"
  done < <(mundo_comandos)
  sleep 3
}

# --- clientes -------------------------------------------------------------

opcoes_escrever() { # pasta: mesmo options.txt para os dois clientes
  mkdir -p "$1"
  cat > "$1/options.txt" <<'EOF'
version:1343
invertYMouse:false
mouseSensitivity:0.5
fov:0.0
gamma:0.0
saturation:0.0
renderDistance:8
guiScale:2
particles:0
viewBobbing:true
anaglyph:false
maxFps:120
fboEnable:true
difficulty:1
fancyGraphics:true
ambientOcclusion:2
clouds:2
resourcePacks:[]
incompatibleResourcePacks:[]
lang:en_US
language:en_US
chatVisibility:0
chatScale:1.0
chatOpacity:1.0
enableVsync:false
hideGUI:false
EOF
}

java_cp() {
  local cp="$MC_JAR" lib
  for lib in "${BIBLIOTECAS[@]}"; do
    [[ -f "$LIBS/$lib" ]] && cp="$cp:$LIBS/$lib"
  done
  printf '%s' "$cp"
}

clientes_parar() {
  local p
  for p in java rs; do
    if processo_ativo "$p"; then
      kill "$(cat "$LOGS/$p.pid")" 2>/dev/null || true
      esperar_fim "$p" 20 || kill -9 "$(cat "$LOGS/$p.pid")" 2>/dev/null || true
    fi
  done
}

# Espera uma entrada NOVA no log do servidor (contagem maior que a anterior).
esperar_entrada() { # jogador contagem-anterior
  local i n
  for ((i = 0; i < 180; i++)); do
    n=$(grep -c "$1 joined the game" "$LOGS/servidor.log" || true)
    (( n > $2 )) && return 0
    sleep 1
  done
  return 1
}

clientes_iniciar() {
  [[ -x "$JAVA_BIN" ]] || die "Java 8 do launcher não encontrado: $JAVA_BIN"
  [[ -f "$MC_JAR" ]] || die "cliente vanilla não encontrado: $MC_JAR"
  [[ -x "$RS_BIN" ]] || die "mine-rs não compilado: $RS_BIN (cargo build --release -p mc-client)"
  garantir_display "$JAVA_DISPLAY"
  garantir_display "$RS_DISPLAY"
  clientes_parar
  opcoes_escrever "$JAVA_JOGO"
  opcoes_escrever "$RS_CONFIG"
  local java0 rs0
  java0=$(grep -c "$JOGADOR_JAVA joined the game" "$LOGS/servidor.log" || true)
  rs0=$(grep -c "$JOGADOR_RS joined the game" "$LOGS/servidor.log" || true)
  (setsid nohup "$DIR/paridade.sh" _java </dev/null >/dev/null 2>&1 &)
  (setsid nohup "$DIR/paridade.sh" _rs </dev/null >/dev/null 2>&1 &)
  esperar_entrada "$JOGADOR_JAVA" "$java0" || die "cliente Java não entrou; veja $LOGS/java.log"
  esperar_entrada "$JOGADOR_RS" "$rs0" || die "mine-rs não entrou; veja $LOGS/rs.log"
  sleep 5
  log "os dois clientes estão no servidor"
}

# Ponto de entrada (spawn): o servidor loga a posição de entrada do primeiro jogador.
ponto_de_entrada() {
  [[ -s "$LOGS/spawn.txt" ]] && return 0
  grep -m 1 'logged in with entity id' "$LOGS/servidor.log" \
    | sed -n 's/.* at (\([^)]*\)).*/\1/p' | tr -d ',' > "$LOGS/spawn.txt" || true
  [[ -s "$LOGS/spawn.txt" ]] || die "não achei o ponto de entrada no log do servidor"
  log "ponto de entrada: $(cat "$LOGS/spawn.txt")"
}

# --- cenas ----------------------------------------------------------------

janela() { # display padrão-do-nome
  DISPLAY="$1" xdotool search --onlyvisible --name "$2" 2>/dev/null | tail -n 1 || true
}

tecla() { # display janela teclas...
  local display=$1 w=$2 t
  shift 2
  DISPLAY="$display" timeout 5 xdotool windowfocus "$w" 2>/dev/null || true
  DISPLAY="$display" xdotool mousemove --window "$w" 640 360 2>/dev/null || true
  sleep 1
  for t in "$@"; do
    if [[ $t == pausa:* ]]; then
      sleep "${t#pausa:}"
    elif [[ $t == texto:* ]]; then
      DISPLAY="$display" xdotool type --delay 80 -- "${t#texto:}"
    else
      DISPLAY="$display" xdotool key --delay 100 -- "$t"
    fi
  done
}

captura() { # display arquivo.png
  DISPLAY="$1" xwd -root -silent | ffmpeg -loglevel error -y -f image2pipe -c:v xwd -i - -frames:v 1 "$2"
}

ativar() { # jogador "x y z yaw pitch"
  servidor_cmd "gamemode 1 $1"
  servidor_cmd "tp $1 $2"
}

# Parque: espectador 40 blocos acima da pose (PARQUE_CENA), fora da visão e com os chunks
# já carregados. Assim, ao entrar em cena o jogador só desce, sem chunks novos (ver README).
estacionar() { # jogador
  servidor_cmd "gamemode 3 $1"
  servidor_cmd "tp $1 $PARQUE_CENA"
}

capturar_lado() { # jogador display padrão-da-janela pose arquivo
  local jogador=$1 display=$2 padrao=$3 pose=$4 arq=$5 w
  servidor_cmd "gamemode 1 $jogador"
  servidor_cmd "tp $jogador $pose"
  sleep 3
  w="$(janela "$display" "$padrao")"
  [[ -n "$w" ]] || die "janela não encontrada em $display"
  tecla "$display" "$w" "${CENA_TECLAS[@]}"
  sleep 2
  captura "$display" "$arq"
  tecla "$display" "$w" "${CENA_DESFAZER[@]}"
  sleep 1
  estacionar "$jogador"
  sleep 2
}

cena_executar() { # nome spawn
  local nome=$1 spawn=$2 pose brutas="$LOGS/brutas" c px py pz yaw pitch
  CENA_POSE=""
  CENA_PREP=()
  CENA_TECLAS=()
  CENA_DESFAZER=()
  "cena_$nome"
  pose="${CENA_POSE//SPAWN/$spawn}"
  read -r px py pz yaw pitch <<< "$pose"
  PARQUE_CENA="$px $(LC_ALL=C awk -v y="$py" 'BEGIN { printf "%.1f", y + 40 }') $pz"
  mkdir -p "$brutas" "$SAIDA"
  log "cena $nome em [$pose]"
  for c in "${CENA_PREP[@]}"; do
    servidor_cmd "$c"
  done
  estacionar "$JOGADOR_JAVA"
  estacionar "$JOGADOR_RS"
  sleep "$ESPERA"
  capturar_lado "$JOGADOR_JAVA" "$JAVA_DISPLAY" "Minecraft" "$pose" "$brutas/java_$nome.png"
  capturar_lado "$JOGADOR_RS" "$RS_DISPLAY" "mine-rs" "$pose" "$brutas/rs_$nome.png"
  ffmpeg -loglevel error -y -i "$brutas/java_$nome.png" -i "$brutas/rs_$nome.png" \
    -filter_complex hstack "$SAIDA/$nome.png"
  log "pronto: $SAIDA/$nome.png"
}

cenas_rodar() {
  local lista=("$@") nome spawn tentativa
  [[ ${#lista[@]} -gt 0 ]] || lista=("${CENAS[@]}")
  ponto_de_entrada
  spawn="$(cat "$LOGS/spawn.txt")"
  for nome in "${lista[@]}"; do
    # O mine-rs ainda pode abortar (bug do fs_overlay, ver README): reinicia os clientes e tenta de novo.
    for tentativa in 1 2 3; do
      if (cena_executar "$nome" "$spawn"); then
        break
      fi
      log "cena $nome falhou (tentativa $tentativa); reiniciando os clientes"
      clientes_iniciar
    done
  done
}

# --- despacho -------------------------------------------------------------

main() {
  mkdir -p "$LOGS" "$SAIDA"
  local cmd=${1:-}
  [[ $# -gt 0 ]] && shift
  case "$cmd" in
    tudo)
      servidor_iniciar
      mundo_preparar
      clientes_iniciar
      cenas_rodar "$@"
      ;;
    servidor) servidor_iniciar ;;
    preparar) mundo_preparar ;;
    clientes) clientes_iniciar ;;
    spawn) ponto_de_entrada; cat "$LOGS/spawn.txt" ;;
    cenas) clientes_iniciar_se_preciso; cenas_rodar "$@" ;;
    parar)
      clientes_parar
      servidor_parar
      ;;
    _servidor)
      echo $$ > "$LOGS/servidor.pid"
      exec 3<>"$SERVIDOR/server.stdin"
      exec "$JAVA_BIN" -Xms1G -Xmx1G -jar minecraft_server.1.8.9.jar nogui <&3 >>"$LOGS/servidor.log" 2>&1
      ;;
    _java)
      echo $$ > "$LOGS/java.pid"
      cd "$JAVA_JOGO"
      export DISPLAY="$JAVA_DISPLAY"
      exec "$JAVA_BIN" -Xms512M -Xmx1536M -Djava.library.path="$NATIVOS" -cp "$(java_cp)" \
        net.minecraft.client.main.Main --username "$JOGADOR_JAVA" --accessToken 0 --version 1.8.9 \
        --assetIndex 1.8 --assetsDir "$ASSETS" --gameDir "$JAVA_JOGO" \
        --server 127.0.0.1 --port "$PORTA" --width 1280 --height 720 >>"$LOGS/java.log" 2>&1
      ;;
    _rs)
      echo $$ > "$LOGS/rs.pid"
      export DISPLAY="$RS_DISPLAY" MINE_RS_CONFIG="$RS_CONFIG"
      exec "$RS_BIN" "127.0.0.1:$PORTA" --offline --name "$JOGADOR_RS" >>"$LOGS/rs.log" 2>&1
      ;;
    *)
      sed -n '2,7p' "$0" >&2
      exit 2
      ;;
  esac
}

clientes_iniciar_se_preciso() {
  if processo_ativo java && processo_ativo rs; then return 0; fi
  clientes_iniciar
}

main "$@"
