#!/usr/bin/env bash
# bench.sh BINARIO [rótulo]: FPS do mine-rs na GPU de verdade, sem aparecer na tela.
#
# Sobe o servidor local da paridade (view-distance 10, como os servidores de minigame), abre o cliente
# num Xvfb só para a janela e os eventos, com MINE_RS_FORA_DA_TELA=1 (desenha na GPU de alto desempenho
# numa textura fora da tela, sem apresentar) e MINE_RS_STATS=1. Cada cena é uma posição fixa, voando no
# criativo; o cliente entra de novo já nela, espera os chunks e tira a média dos quadros/s.
# Saída: uma linha por cena e o log em $LOGS/bench-<rótulo>.log. Nunca usa o display :0.
set -euo pipefail
export LC_ALL=C  # awk com ponto decimal
DIR=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$DIR/../.." && pwd)
P="$REPO/scripts/paridade/paridade.sh"
BIN=$(readlink -f "$1"); ROT=${2:-$(basename "$BIN")}
DISPLAY_BENCH=${BENCH_DISPLAY:-:141}
ESPERA=${BENCH_ESPERA:-40}   # s para carregar e montar os chunks de cada cena
MEDE=${BENCH_MEDE:-30}       # s de medição por cena
LOGS=/tmp/paridade-logs; mkdir -p "$LOGS"
SERVIDOR=${PARIDADE_SERVIDOR:-$HOME/Documentos/Projetos/mine-rs-servidor-teste}
NOME=Bench
# Cenas: x y z yaw pitch (mundo de semente 1). Horizonte aberto, olhando para baixo no terreno, e uma vista alta.
CENAS=("horizonte 200 90 200 45 0" "terreno 200 80 200 135 30" "alto 0 140 0 -45 15")

[[ "$DISPLAY_BENCH" == ":0" ]] && { echo "não uso o display :0"; exit 1; }
PARIDADE_VD=10 "$P" servidor >/dev/null 2>&1
cmd() { timeout 5 bash -c 'printf "%s\n" "$1" > "$2"' _ "$1" "$SERVIDOR/server.stdin"; }
conta() { grep -c "$NOME $1 the game" "$LOGS/servidor.log" || true; }
n=${DISPLAY_BENCH#:}
# Fora da árvore do script (subshell): um Xvfb filho seguraria o fim do script.
[[ -S /tmp/.X11-unix/X$n ]] || (setsid nohup Xvfb "$DISPLAY_BENCH" -screen 0 1600x900x24 -nolisten tcp -noreset >/dev/null 2>&1 </dev/null &)
# Espera o Xvfb aceitar conexões; com o PC carregado ele demora mais que alguns segundos.
for _ in $(seq 30); do DISPLAY=$DISPLAY_BENCH xdpyinfo >/dev/null 2>&1 && break; sleep 1; done
DISPLAY=$DISPLAY_BENCH xdpyinfo >/dev/null 2>&1 || { echo "Xvfb $DISPLAY_BENCH não respondeu"; exit 1; }
CFG=$(mktemp -d /tmp/bench-cfg.XXXX); cp -r /tmp/mrs-cfg/. "$CFG/"
LOG="$LOGS/bench-$ROT.log"; : > "$LOG"
CP=
trap 'kill $CP 2>/dev/null; rm -rf "$CFG"' EXIT

entrar() {
  local j0; j0=$(conta joined)
  DISPLAY=$DISPLAY_BENCH MINE_RS_CONFIG=$CFG MINE_RS_FORA_DA_TELA=1 MINE_RS_STATS=1 \
    "$BIN" 127.0.0.1:25577 --offline --name $NOME >>"$LOG" 2>&1 &
  CP=$!
  for _ in $(seq 60); do (( $(conta joined) > j0 )) && break; kill -0 $CP 2>/dev/null || break; sleep 1; done
  kill -0 $CP 2>/dev/null && (( $(conta joined) > j0 )) || { echo "o cliente não entrou:"; tail -3 "$LOG"; exit 1; }
}
sair() {
  local l0; l0=$(conta left)
  kill $CP 2>/dev/null
  for _ in $(seq 50); do kill -0 $CP 2>/dev/null || break; sleep 0.2; done
  for _ in $(seq 20); do (( $(conta left) > l0 )) && break; sleep 0.5; done
}

entrar
grep -m1 "GPU:" "$LOG" || true
for c in "${CENAS[@]}"; do
  read -r nome x y z yaw pitch <<<"$c"
  [[ -n ${BENCH_SO:-} && $nome != "$BENCH_SO" ]] && continue  # BENCH_SO=cena mede só uma
  # O servidor vanilla 1.8 não manda os chunks do destino depois de um /tp longo (o cliente Java fica
  # no vazio do mesmo jeito): o /tp só guarda a posição, e o cliente entra de novo já nela.
  # Espectador e volta ao criativo deixa o voo ligado, e o voo fica salvo com o jogador.
  cmd "gamemode 3 $NOME"; sleep 1; cmd "gamemode 1 $NOME"; sleep 1
  cmd "tp $NOME $x $y $z $yaw $pitch"; sleep 2
  sair
  entrar
  sleep "$ESPERA"
  kill -0 $CP 2>/dev/null || { echo "o cliente morreu:"; tail -3 "$LOG"; exit 1; }
  ini=$(wc -l <"$LOG")
  sleep "$MEDE"
  # Só as linhas [stats] (as [prof] também falam em quadros/s). mawk não tem asort: mín/máx na mão.
  tail -n +"$((ini + 1))" "$LOG" | grep '^\[stats\]' | grep -o '[0-9.]* quadros/s' | awk -v n="$nome" -v r="$ROT" \
    'NR==1 {lo=$1; hi=$1} {s+=$1; if ($1<lo) lo=$1; if ($1>hi) hi=$1} END {if (NR==0) {print r, n, "sem amostras"; exit}; printf "%s %-10s média %.0f  mín %.0f  máx %.0f  (%d amostras)\n", r, n, s/NR, lo, hi, NR}' || true
  # Cena válida só com o mundo carregado: colunas e seções desenhadas, e o perfil médio da medição.
  grep '^\[stats\]' "$LOG" | tail -1 | grep -o 'colunas: [0-9]*\|desenhadas: [0-9]*\|jogador: .*' | paste -sd' ' | sed 's/^/    /' || true
  tail -n +"$((ini + 1))" "$LOG" | grep '^\[prof\]' | sed 's/.*ms\/quadro: //' | awk \
    '{for (i=1; i<NF; i+=2) {s[$i]+=$(i+1); if (NR==1) o[++k]=$i}} END {if (NR) {printf "   "; for (j=1; j<=k; j++) printf " %s %.2f", o[j], s[o[j]]/NR; print ""}}' || true
done
