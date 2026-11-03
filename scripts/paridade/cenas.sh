# Cenas e mundo de teste da paridade. Carregado por paridade.sh (não executar sozinho).
#
# Cada função cena_NN_nome define, com a mesma pose para os dois clientes:
#   CENA_POSE      "x y z yaw pitch"; SPAWN é trocado pelo ponto de entrada medido no servidor
#   CENA_PREP      comandos do console do servidor antes de posicionar (hora, clima, entidades)
#   CENA_TECLAS    teclas (nomes do xdotool) pressionadas depois de chegar na pose
#   CENA_DESFAZER  teclas para voltar ao normal antes do próximo cliente
# "texto:..." digita o texto em vez de apertar tecla.
#
# Mundo de teste (montado por mundo_comandos): plataforma de pedra em x/z 180..215,
# camadas y 68..70, superfície em y 71. Piscinas de água e lava em y 69..70.
# Fileira de blocos em z=198; mobs em z=207; itens em z 204..206.

CENAS=(
  01_spawn_meio_dia
  02_agua_lava
  03_noite
  04_chuva
  05_blocos
  06_mobs
  07_itens_no_chao
  08_f3
  09_inventario
  10_chat
  11_pausa
)

# Comandos do console que montam o mundo de teste (um por linha).
mundo_comandos() {
  cat <<'EOF'
fill 180 68 180 215 70 215 stone
fill 192 69 192 194 70 194 water
fill 204 69 192 206 70 194 lava
setblock 192 71 198 fence
setblock 193 71 198 nether_brick_fence
setblock 194 71 198 oak_stairs 0
setblock 195 71 198 stone_stairs 0
setblock 196 71 198 stone_slab 0
setblock 197 71 198 wooden_slab 0
setblock 198 71 198 glass
setblock 199 71 198 stained_glass 14
setblock 200 71 198 leaves 0
setblock 201 71 198 leaves 1
setblock 202 71 198 torch 5
setblock 203 71 198 redstone_torch 5
setblock 204 71 198 redstone_block
setblock 205 71 198 redstone_wire
setblock 206 71 198 redstone_lamp
setblock 207 71 198 wooden_door 0
setblock 207 72 198 wooden_door 8
setblock 208 71 198 chest 2
setblock 209 71 198 standing_sign 0
setblock 210 71 198 iron_bars
setblock 211 71 198 cobblestone_wall
setblock 212 71 198 hay_block
setblock 213 71 198 wool 14
setblock 214 71 198 log 0
EOF
}

# Limpeza: tira mobs e itens do mundo, mantendo os jogadores.
LIMPEZA="kill @e[type=!Player]"

# Entidades de teste. NoAI congela os mobs; motion zero e PickupDelay alto congelam os itens.
mobs_comandos() {
  cat <<'EOF'
summon Zombie 198.5 71 207.5 {NoAI:1b}
summon Skeleton 200.5 71 207.5 {NoAI:1b}
summon Creeper 202.5 71 207.5 {NoAI:1b}
summon Pig 204.5 71 207.5 {NoAI:1b}
summon Sheep 206.5 71 207.5 {NoAI:1b,Color:0b}
summon Villager 208.5 71 207.5 {NoAI:1b,Profession:0}
summon ArmorStand 210.5 71 207.5 {ArmorItems:[{},{},{},{id:"minecraft:diamond_helmet",Count:1b,Damage:0s}]}
EOF
}

itens_comandos() {
  local x z item i=0
  local itens=(diamond iron_sword apple bread stick golden_apple redstone paper)
  for z in 204.5 206.5; do
    for x in 197.5 199.5 201.5 203.5; do
      item=${itens[$i]}
      echo "summon Item $x 71.0 $z {Item:{id:\"minecraft:$item\",Count:1b,Damage:0s},PickupDelay:32767s,Motion:[0.0d,0.0d,0.0d]}"
      i=$((i + 1))
    done
  done
}

# Cenas em dia/noite/chuva: no ponto de entrada (SPAWN), sem entidades.
cena_01_spawn_meio_dia() {
  CENA_POSE="SPAWN 0 0"
  CENA_PREP=("time set 6000" "weather clear 100000")
}

cena_02_agua_lava() {
  CENA_POSE="199.5 71 189.5 0 30"
  CENA_PREP=("time set 6000" "weather clear 100000")
}

cena_03_noite() {
  CENA_POSE="SPAWN 0 0"
  CENA_PREP=("time set 18000" "weather clear 100000")
}

cena_04_chuva() {
  CENA_POSE="SPAWN 0 0"
  CENA_PREP=("time set 6000" "weather rain 100000")
}

cena_05_blocos() {
  CENA_POSE="204.5 71 190.5 0 0"
  CENA_PREP=("time set 6000" "weather clear 100000" "$LIMPEZA")
}

cena_06_mobs() {
  CENA_POSE="204.5 71 201.5 0 0"
  CENA_PREP=("time set 6000" "weather clear 100000" "$LIMPEZA")
  mapfile -t -O "${#CENA_PREP[@]}" CENA_PREP < <(mobs_comandos)
}

cena_07_itens_no_chao() {
  CENA_POSE="199.5 71 200.5 0 30"
  CENA_PREP=("time set 6000" "weather clear 100000" "$LIMPEZA")
  mapfile -t -O "${#CENA_PREP[@]}" CENA_PREP < <(itens_comandos)
}

cena_08_f3() {
  CENA_POSE="204.5 71 190.5 0 0"
  CENA_PREP=("time set 6000" "weather clear 100000" "$LIMPEZA")
  CENA_TECLAS=(F3)
  CENA_DESFAZER=(F3)
}

cena_09_inventario() {
  CENA_POSE="204.5 71 190.5 0 0"
  CENA_PREP=("time set 6000" "weather clear 100000" "$LIMPEZA")
  CENA_TECLAS=(e)
  CENA_DESFAZER=(Escape)
}

cena_10_chat() {
  CENA_POSE="204.5 71 190.5 0 0"
  CENA_PREP=("time set 6000" "weather clear 100000" "$LIMPEZA")
  CENA_TECLAS=(t "pausa:1" "texto:Ola do paridade" Return)
}

cena_11_pausa() {
  CENA_POSE="204.5 71 190.5 0 0"
  CENA_PREP=("time set 6000" "weather clear 100000" "$LIMPEZA")
  CENA_TECLAS=(Escape)
  CENA_DESFAZER=(Escape)
}
