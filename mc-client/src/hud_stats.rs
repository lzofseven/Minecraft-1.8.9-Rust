//! Barras de status do HUD: porte de `GuiIngame.renderPlayerStats`, `renderHorseJumpBar`,
//! `renderExpBar` e `renderBossHealth`. A função `layout` só calcula as posições e os recortes de
//! `icons.png` (testável sem GPU); `Hud::draw` desenha o resultado.

/// Gerador congruencial de `java.util.Random` (o HUD sorteia tremores com `rand.setSeed`).
pub struct JavaRandom(i64);

const MASK: i64 = (1 << 48) - 1;

impl JavaRandom {
    pub fn new(seed: i64) -> Self {
        Self((seed ^ 0x5DEECE66D) & MASK)
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.0 = (self.0.wrapping_mul(0x5DEECE66D).wrapping_add(0xB)) & MASK;
        (self.0 >> (48 - bits)) as i32
    }

    pub fn next_int(&mut self, bound: i32) -> i32 {
        if bound & -bound == bound {
            return ((bound as i64 * self.next(31) as i64) >> 31) as i32;
        }
        loop {
            let bits = self.next(31);
            let val = bits % bound;
            if bits.wrapping_sub(val).wrapping_add(bound - 1) >= 0 {
                return val;
            }
        }
    }
}

/// Recorte 9x9 de `icons.png` em (`x`, `y`) da tela.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Icon {
    pub x: i32,
    pub y: i32,
    pub u: i32,
    pub v: i32,
}

/// Montaria que é `EntityLivingBase`: vida atual e máxima.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MountHealth {
    pub health: f32,
    pub max_health: f32,
}

/// Estado lido por `renderPlayerStats`.
#[derive(Clone, Debug)]
pub struct Stats {
    pub health: f32,
    pub max_health: f32,
    pub absorption: f32,
    pub armor: i32,
    pub food: i32,
    pub prev_food: i32,
    pub saturation: f32,
    pub poison: bool,
    pub wither: bool,
    pub regeneration: bool,
    pub hunger: bool,
    pub hardcore: bool,
    /// Olhos dentro d'água (`isInsideOfMaterial(Material.water)`) e o ar atual.
    pub in_water: bool,
    pub air: i32,
    /// `ridingEntity != null`; `Some` quando a montaria tem vida.
    pub riding: bool,
    pub mount: Option<MountHealth>,
    /// Contador de ticks do GuiIngame (`updateCounter`).
    pub update_counter: i32,
    /// mod_vanillahud: a fome não aparece (e o ar desce para a linha dela, como o right_height do Forge).
    pub hide_food: bool,
    /// Coração piscando (`healthUpdateCounter`) e o valor guardado em `lastPlayerHealth`.
    pub blink: bool,
    pub last_health: i32,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            health: 20.0,
            max_health: 20.0,
            absorption: 0.0,
            armor: 0,
            food: 20,
            prev_food: 20,
            saturation: 5.0,
            poison: false,
            wither: false,
            regeneration: false,
            hunger: false,
            hardcore: false,
            in_water: false,
            air: 300,
            riding: false,
            mount: None,
            update_counter: 0,
            hide_food: false,
            blink: false,
            last_health: 20,
        }
    }
}

/// Estado do pisca-pisca dos corações (`healthUpdateCounter`, `playerHealth`, `lastPlayerHealth`).
#[derive(Default, Debug)]
pub struct HealthBlink {
    player_health: i32,
    last_player_health: i32,
    update_counter_at: i64,
    last_system_time: u64,
}

impl HealthBlink {
    /// Passo do começo de `renderPlayerStats`. Devolve (piscando, `lastPlayerHealth`).
    pub fn step(&mut self, health: f32, hurt_resistant_time: i32, update_counter: i32, now_ms: u64) -> (bool, i32) {
        let h = health.ceil() as i32;
        let uc = update_counter as i64;
        let blink = self.update_counter_at > uc && (self.update_counter_at - uc) / 3 % 2 == 1;
        if h < self.player_health && hurt_resistant_time > 0 {
            self.last_system_time = now_ms;
            self.update_counter_at = uc + 20;
        } else if h > self.player_health && hurt_resistant_time > 0 {
            self.last_system_time = now_ms;
            self.update_counter_at = uc + 10;
        }
        if now_ms.wrapping_sub(self.last_system_time) > 1000 {
            self.player_health = h;
            self.last_player_health = h;
            self.last_system_time = now_ms;
        }
        self.player_health = h;
        (blink, self.last_player_health)
    }
}

fn ceil_div2(a: f32) -> i32 {
    (a / 2.0).ceil() as i32
}

/// Todos os ícones 9x9 de `renderPlayerStats` na ordem em que o Java os desenha.
pub fn layout(s: &Stats, w: i32, h: i32) -> Vec<Icon> {
    let mut out = Vec::new();
    let mut put = |x: i32, y: i32, u: i32, v: i32| out.push(Icon { x, y, u, v });
    let health = s.health.ceil() as i32;
    let mut rand = JavaRandom::new((s.update_counter.wrapping_mul(312871)) as i64);
    let left = w / 2 - 91;
    let right = w / 2 + 91;
    let top = h - 39;
    let max = s.max_health;
    let abs = s.absorption;
    let rows = ((max + abs) / 2.0 / 10.0).ceil() as i32;
    let row_gap = (10 - (rows - 2)).max(3);
    let armor_y = top - (rows - 1) * row_gap - 10;
    let mut abs_left = abs;
    let regen_at = if s.regeneration { s.update_counter % (max + 5.0).ceil() as i32 } else { -1 };

    // Armadura.
    for i in 0..10 {
        if s.armor > 0 {
            let x = left + i * 8;
            if i * 2 + 1 < s.armor {
                put(x, armor_y, 34, 9);
            }
            if i * 2 + 1 == s.armor {
                put(x, armor_y, 25, 9);
            }
            if i * 2 + 1 > s.armor {
                put(x, armor_y, 16, 9);
            }
        }
    }

    // Vida (de trás para frente, como o Java).
    for i in (0..ceil_div2(max + abs)).rev() {
        let mut tint = 16;
        if s.poison {
            tint += 36;
        } else if s.wither {
            tint += 72;
        }
        let blink = s.blink as i32;
        let row = ((i + 1) as f32 / 10.0).ceil() as i32 - 1;
        let x = left + i % 10 * 8;
        let mut y = top - row * row_gap;
        if health <= 4 {
            y += rand.next_int(2);
        }
        if i == regen_at {
            y -= 2;
        }
        let hc = if s.hardcore { 5 } else { 0 };
        put(x, y, 16 + blink * 9, 9 * hc);
        if s.blink {
            if i * 2 + 1 < s.last_health {
                put(x, y, tint + 54, 9 * hc);
            }
            if i * 2 + 1 == s.last_health {
                put(x, y, tint + 63, 9 * hc);
            }
        }
        if abs_left > 0.0 {
            if abs_left == abs && abs % 2.0 == 1.0 {
                put(x, y, tint + 153, 9 * hc);
            } else {
                put(x, y, tint + 144, 9 * hc);
            }
            abs_left -= 2.0;
        } else {
            if i * 2 + 1 < health {
                put(x, y, tint + 36, 9 * hc);
            }
            if i * 2 + 1 == health {
                put(x, y, tint + 45, 9 * hc);
            }
        }
    }

    if !s.riding && !s.hide_food {
        // Fome.
        for i in 0..10 {
            let mut y = top;
            let mut tint = 16;
            let mut bg = 0;
            if s.hunger {
                tint += 36;
                bg = 13;
            }
            if s.saturation <= 0.0 && s.update_counter % (s.food * 3 + 1) == 0 {
                y = top + (rand.next_int(3) - 1);
            }
            let x = right - i * 8 - 9;
            put(x, y, 16 + bg * 9, 27);
            if i * 2 + 1 < s.food {
                put(x, y, tint + 36, 27);
            }
            if i * 2 + 1 == s.food {
                put(x, y, tint + 45, 27);
            }
        }
    } else if let Some(m) = s.mount.filter(|_| s.riding) {
        // Vida da montaria no lugar da fome.
        let hp = m.health.ceil() as i32;
        let mut n = ((m.max_health + 0.5) as i32) / 2;
        if n > 30 {
            n = 30;
        }
        let mut y = top;
        let mut off = 0;
        while n > 0 {
            let in_row = n.min(10);
            n -= in_row;
            for i in 0..in_row {
                let x = right - i * 8 - 9;
                put(x, y, 52, 9);
                if i * 2 + 1 + off < hp {
                    put(x, y, 52 + 36, 9);
                }
                if i * 2 + 1 + off == hp {
                    put(x, y, 52 + 45, 9);
                }
            }
            y -= 10;
            off += 20;
        }
    }

    // Ar.
    if s.in_water {
        let full = (((s.air - 2) as f64) * 10.0 / 300.0).ceil() as i32;
        let part = ((s.air as f64) * 10.0 / 300.0).ceil() as i32 - full;
        for i in 0..full + part {
            let x = right - i * 8 - 9;
            let y = if s.hide_food && !s.riding { top } else { armor_y };
            if i < full {
                put(x, y, 16, 18);
            } else {
                put(x, y, 25, 18);
            }
        }
    }
    out
}

/// `renderHorseJumpBar`: devolve (y, largura preenchida).
pub fn jump_bar(h: i32, power: f32) -> (i32, i32) {
    (h - 32 + 3, (power * 183.0) as i32)
}

/// `renderExpBar`: (y, largura preenchida) quando `xpBarCap() > 0`.
pub fn exp_bar(h: i32, experience: f32) -> (i32, i32) {
    (h - 32 + 3, (experience * 183.0) as i32)
}

/// `EntityPlayer.xpBarCap()`: pontos para subir de nível.
pub fn xp_bar_cap(level: i32) -> i32 {
    if level >= 30 {
        112 + (level - 30) * 9
    } else if level >= 15 {
        37 + (level - 15) * 5
    } else {
        7 + level * 2
    }
}

/// Barra de chefe (`BossStatus`).
#[derive(Default, Clone, Debug)]
pub struct BossStatus {
    pub name: Option<String>,
    pub health_scale: f32,
    pub status_bar_time: i32,
    pub has_color_modifier: bool,
}

impl BossStatus {
    /// `BossStatus.setBossStatus`, chamado pelo render do dragão e do wither.
    pub fn set(&mut self, name: String, health: f32, max_health: f32, color_modifier: bool) {
        self.health_scale = health / max_health;
        self.status_bar_time = 100;
        self.name = Some(name);
        self.has_color_modifier = color_modifier;
    }

    /// Posições de `renderBossHealth` (largura 182): (x, y, largura preenchida) se visível.
    /// Gasta um tick, como o `--statusBarTime` do Java a cada quadro desenhado.
    pub fn frame(&mut self, w: i32) -> Option<(i32, i32, i32)> {
        if self.name.is_some() && self.status_bar_time > 0 {
            self.status_bar_time -= 1;
            let x = w / 2 - 182 / 2;
            Some((x, 12, (self.health_scale * 183.0) as i32))
        } else {
            None
        }
    }
}

/// Contagem do pulo do cavalo (`EntityPlayerSP.horseJumpPowerCounter` e `horseJumpPower`).
#[derive(Default, Debug)]
pub struct HorseJump {
    counter: i32,
    pub power: f32,
    prev_jump: bool,
}

impl HorseJump {
    /// Um tick de `onLivingUpdate`. `jump` = tecla de pulo agora. Devolve a potência (0..100)
    /// do pacote C0B `RIDING_JUMP` quando o jogador solta a tecla.
    pub fn tick(&mut self, riding_horse: bool, jump: bool) -> Option<i32> {
        let flag = self.prev_jump;
        let mut send = None;
        if riding_horse {
            if self.counter < 0 {
                self.counter += 1;
                if self.counter == 0 {
                    self.power = 0.0;
                }
            }
            if flag && !jump {
                self.counter = -10;
                send = Some((self.power * 100.0) as i32);
            } else if !flag && jump {
                self.counter = 0;
                self.power = 0.0;
            } else if flag {
                self.counter += 1;
                self.power = if self.counter < 10 { self.counter as f32 * 0.1 } else { 0.8 + 2.0 / (self.counter - 9) as f32 * 0.1 };
            }
        } else {
            self.power = 0.0;
        }
        self.prev_jump = jump;
        send
    }
}

/// `timeInPortal` e `prevTimeInPortal` de `EntityPlayerSP.onLivingUpdate`.
#[derive(Default, Debug, Clone, Copy)]
pub struct Portal {
    pub time: f32,
    pub prev: f32,
}

impl Portal {
    /// `in_portal`: o corpo toca um bloco de portal; `confusion_ticks`: duração restante de náusea (0 = sem).
    /// Devolve true no primeiro tick dentro do portal (toca `portal.trigger`).
    pub fn tick(&mut self, in_portal: bool, confusion_ticks: i32) -> bool {
        self.prev = self.time;
        let mut trigger = false;
        if in_portal {
            trigger = self.time == 0.0;
            self.time += 0.0125;
            if self.time >= 1.0 {
                self.time = 1.0;
            }
        } else if confusion_ticks > 60 {
            self.time += 0.006_666_667;
            if self.time > 1.0 {
                self.time = 1.0;
            }
        } else {
            if self.time > 0.0 {
                self.time -= 0.05;
            }
            if self.time < 0.0 {
                self.time = 0.0;
            }
        }
        trigger
    }

    /// Alfa de `renderPortal` (com a curva do jogo) para o quadro interpolado.
    pub fn alpha(&self, partial: f32) -> f32 {
        let mut a = self.prev + (self.time - self.prev) * partial;
        if a > 0.0 && a < 1.0 {
            a *= a;
            a *= a;
            a = a * 0.8 + 0.2;
        }
        a
    }
}

/// `WorldProvider.generateLightBrightnessTable` do Overworld (ambiente 0).
pub fn light_brightness(level: i32) -> f32 {
    let f1 = 1.0 - level.clamp(0, 15) as f32 / 15.0;
    (1.0 - f1) / (f1 * 3.0 + 1.0)
}

/// `WorldProvider.calculateCelestialAngle` (Overworld).
pub fn celestial_angle(world_time: i64, partial: f32) -> f32 {
    let i = (world_time % 24000) as i32;
    let mut f = (i as f32 + partial) / 24000.0 - 0.25;
    if f < 0.0 {
        f += 1.0;
    }
    if f > 1.0 {
        f -= 1.0;
    }
    let f1 = f;
    let f = 1.0 - (((f * std::f32::consts::PI) as f64).cos() + 1.0) as f32 / 2.0;
    f1 + (f - f1) / 3.0
}

/// `World.calculateSkylightSubtracted` sem chuva nem trovoada.
pub fn skylight_subtracted(world_time: i64) -> i32 {
    let f = celestial_angle(world_time, 1.0);
    let mut f1 = 1.0 - (((f * std::f32::consts::PI * 2.0) as f64).cos() * 2.0 + 0.5) as f32;
    f1 = f1.clamp(0.0, 1.0);
    f1 = 1.0 - f1;
    ((1.0 - f1) * 11.0) as i32
}

/// `Entity.getBrightness`: luz (0..15) -> brilho (0..1) usando bloco, céu e o escurecimento do dia.
pub fn brightness_at(block: i32, sky: i32, world_time: i64) -> f32 {
    light_brightness(block.max(sky - skylight_subtracted(world_time)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hearts(v: &[Icon]) -> Vec<Icon> {
        v.iter().copied().filter(|i| i.v % 9 == 0 && i.u < 200 && i.y < 1000 && i.x < 1000).collect()
    }

    #[test]
    fn java_random_bate_com_o_java() {
        // new Random(0).nextInt(10) == 0, nextInt(10) == 8 (valores do JDK).
        let mut r = JavaRandom::new(0);
        assert_eq!(r.next_int(10), 0);
        assert_eq!(r.next_int(10), 8);
        let mut r = JavaRandom::new(42);
        assert_eq!(r.next_int(100), 30);
        assert_eq!(r.next_int(100), 63);
    }

    #[test]
    fn vida_cheia_posicoes() {
        let s = Stats::default();
        let l = layout(&s, 427, 240);
        // 10 contêineres + 10 cheios + 10 de fome (2 camadas) = 40; sem armadura nem ar.
        assert_eq!(l.len(), 10 + 10 + 20);
        // Primeiro desenhado é o último coração (x = left + 9*8), contêiner u=16.
        assert_eq!(l[0], Icon { x: 427 / 2 - 91 + 72, y: 240 - 39, u: 16, v: 0 });
        assert_eq!(l[1], Icon { x: 427 / 2 - 91 + 72, y: 240 - 39, u: 16 + 36, v: 0 });
        // Fome: x vai da direita para a esquerda.
        assert_eq!(l[20], Icon { x: 427 / 2 + 91 - 9, y: 240 - 39, u: 16, v: 27 });
        assert_eq!(l[21], Icon { x: 427 / 2 + 91 - 9, y: 240 - 39, u: 16 + 36, v: 27 });
    }

    #[test]
    fn meio_coracao_e_hardcore() {
        let s = Stats { health: 5.0, hardcore: true, ..Stats::default() };
        let l = layout(&s, 400, 200);
        // Coração 2 (índice 2): 2*2+1 == 5 -> meio coração (u = 16+45) na linha hardcore (v = 45).
        assert!(l.iter().any(|i| i.u == 16 + 45 && i.v == 45 && i.x == 400 / 2 - 91 + 16));
        assert!(l.iter().any(|i| i.u == 16 && i.v == 45));
    }

    #[test]
    fn envenenado_e_wither_trocam_a_cor() {
        let p = layout(&Stats { poison: true, ..Stats::default() }, 400, 200);
        assert!(p.iter().any(|i| i.u == 16 + 36 + 36 && i.v == 0));
        let w = layout(&Stats { wither: true, ..Stats::default() }, 400, 200);
        assert!(w.iter().any(|i| i.u == 16 + 72 + 36 && i.v == 0));
        // Veneno vence o wither (else if do Java).
        let pw = layout(&Stats { poison: true, wither: true, ..Stats::default() }, 400, 200);
        assert!(pw.iter().all(|i| i.u != 16 + 72 + 36));
    }

    #[test]
    fn absorcao_empilha_uma_segunda_linha() {
        // 20 + 4 de absorção = 12 corações -> 2 linhas, espaçamento max(10-(2-2),3) = 10.
        let s = Stats { absorption: 4.0, ..Stats::default() };
        let l = layout(&s, 400, 200);
        let top = 200 - 39;
        assert!(l.iter().any(|i| i.y == top - 10 && i.u == 16));
        // Absorção usa u = 16+144 (sem piscar).
        assert_eq!(l.iter().filter(|i| i.u == 16 + 144).count(), 2);
        // Absorção ímpar: o primeiro (o de trás) é meio coração dourado (+153).
        let s = Stats { absorption: 3.0, ..Stats::default() };
        let l = layout(&s, 400, 200);
        assert_eq!(l.iter().filter(|i| i.u == 16 + 153).count(), 1);
    }

    #[test]
    fn armadura_cheia_meia_e_vazia() {
        let s = Stats { armor: 5, ..Stats::default() };
        let l = layout(&s, 400, 200);
        let armor: Vec<Icon> = l.iter().copied().filter(|i| i.v == 9 && i.y == 200 - 39 - 10).collect();
        assert_eq!(armor.len(), 10);
        assert_eq!(armor.iter().filter(|i| i.u == 34).count(), 2);
        assert_eq!(armor.iter().filter(|i| i.u == 25).count(), 1);
        assert_eq!(armor.iter().filter(|i| i.u == 16).count(), 7);
        // x: left + i*8.
        assert_eq!(armor[3].x, 400 / 2 - 91 + 24);
    }

    #[test]
    fn armadura_sobe_com_duas_linhas_de_vida() {
        let s = Stats { armor: 20, absorption: 4.0, ..Stats::default() };
        let l = layout(&s, 400, 200);
        // rows = 2 -> gap 10 -> armadura em top - 10 - 10.
        assert!(l.iter().any(|i| i.v == 9 && i.u == 34 && i.y == 200 - 39 - 10 - 10));
    }

    #[test]
    fn ar_bolhas() {
        let s = Stats { in_water: true, air: 300, ..Stats::default() };
        let l = layout(&s, 400, 200);
        let bubbles: Vec<_> = l.iter().filter(|i| i.v == 18).collect();
        assert_eq!(bubbles.len(), 10);
        assert!(bubbles.iter().all(|i| i.u == 16));
        // Com 31 de ar: uma bolha cheia e uma "estourando" (u=25).
        let s = Stats { in_water: true, air: 31, ..Stats::default() };
        let l = layout(&s, 400, 200);
        let b: Vec<_> = l.iter().filter(|i| i.v == 18).collect();
        assert_eq!(b.len(), 2);
        assert_eq!(b.iter().filter(|i| i.u == 25).count(), 1);
        assert_eq!(b[0].x, 400 / 2 + 91 - 9);
        assert_eq!(b[1].x, 400 / 2 + 91 - 9 - 8);
        assert_eq!(b[0].y, 200 - 39 - 10);
    }

    #[test]
    fn fome_com_efeito_e_montaria() {
        let s = Stats { hunger: true, ..Stats::default() };
        let l = layout(&s, 400, 200);
        assert!(l.iter().any(|i| i.v == 27 && i.u == 16 + 13 * 9));
        assert!(l.iter().any(|i| i.v == 27 && i.u == 16 + 36 + 36));
        // Montado: some a fome e aparece a vida da montaria (cavalo 30/30).
        let s = Stats { riding: true, mount: Some(MountHealth { health: 15.0, max_health: 30.0 }), ..Stats::default() };
        let l = layout(&s, 400, 200);
        assert!(l.iter().all(|i| i.v != 27));
        let m: Vec<_> = l.iter().filter(|i| i.v == 9 && i.u >= 52).collect();
        // 15 corações de container em 2 linhas (10 + 5), cheios os com 2i+1 < 15.
        assert_eq!(m.iter().filter(|i| i.u == 52).count(), 15);
        assert_eq!(m.iter().filter(|i| i.u == 52 + 36).count(), 7);
        assert_eq!(m.iter().filter(|i| i.u == 52 + 45).count(), 1);
        assert!(m.iter().any(|i| i.y == 200 - 39 - 10));
    }

    #[test]
    fn regeneracao_levanta_um_coracao() {
        let s = Stats { regeneration: true, update_counter: 3, ..Stats::default() };
        let l = layout(&s, 400, 200);
        let up = l.iter().filter(|i| i.y == 200 - 39 - 2 && i.v == 0).count();
        assert!(up >= 2, "contêiner e cheio do coração 3 sobem 2 px");
        assert!(l.iter().any(|i| i.x == 400 / 2 - 91 + 3 * 8 && i.y == 200 - 41 && i.u == 16));
    }

    #[test]
    fn vida_baixa_treme_com_o_sorteio_do_java() {
        let s = Stats { health: 4.0, update_counter: 7, ..Stats::default() };
        let l = layout(&s, 400, 200);
        let ys: std::collections::HashSet<i32> = hearts(&l).iter().filter(|i| i.u == 16 && i.y > 150).map(|i| i.y).collect();
        assert!(ys.contains(&(200 - 39)) || ys.contains(&(200 - 38)));
        // Reprodutível: mesma semente, mesma saída.
        assert_eq!(l, layout(&s, 400, 200));
    }

    #[test]
    fn pisca_ao_levar_dano() {
        let mut b = HealthBlink::default();
        // Estável há mais de 1 s.
        assert_eq!(b.step(20.0, 0, 100, 5000), (false, 20));
        // Dano com hurtResistantTime > 0: marca 20 ticks de pisca-pisca.
        let (blink, last) = b.step(16.0, 10, 101, 5050);
        assert!(!blink);
        assert_eq!(last, 20, "lastPlayerHealth continua com o valor antigo");
        // Dois ticks depois: (counter - uc)/3 % 2 -> (121-103)/3 = 6 -> par, não pisca; mais tarde ímpar.
        assert!(!b.step(16.0, 8, 103, 5100).0);
        assert!(b.step(16.0, 8, 106, 5150).0, "(121-106)/3 = 5 é ímpar");
        // Passado 1 s o valor guardado alcança a vida atual.
        assert_eq!(b.step(16.0, 0, 130, 7000), (false, 16));
    }

    #[test]
    fn barras_de_experiencia_e_pulo() {
        assert_eq!(xp_bar_cap(0), 7);
        assert_eq!(xp_bar_cap(15), 37);
        assert_eq!(xp_bar_cap(30), 112);
        assert_eq!(exp_bar(240, 0.5), (211, 91));
        assert_eq!(jump_bar(240, 1.0), (211, 183));
    }

    #[test]
    fn barra_de_chefe() {
        let mut b = BossStatus::default();
        assert_eq!(b.frame(427), None);
        b.set("Wither".into(), 150.0, 300.0, false);
        assert_eq!(b.frame(427), Some((427 / 2 - 91, 12, 91)));
        assert_eq!(b.status_bar_time, 99);
        b.status_bar_time = 1;
        assert!(b.frame(427).is_some());
        assert_eq!(b.frame(427), None, "some quando o tempo acaba");
    }

    #[test]
    fn pulo_do_cavalo_carrega_e_envia() {
        let mut j = HorseJump::default();
        assert_eq!(j.tick(true, true), None);
        for _ in 0..3 {
            j.tick(true, true);
        }
        assert!((j.power - 0.3).abs() < 1e-6, "{}", j.power);
        // Solta a tecla: envia a potência *100 e entra em recarga (-10).
        assert_eq!(j.tick(true, false), Some(30));
        for _ in 0..10 {
            j.tick(true, false);
        }
        assert_eq!(j.power, 0.0);
        // Depois de 10 ticks a potência cresce devagar até 1.0.
        let mut j = HorseJump::default();
        j.tick(true, true);
        for _ in 0..30 {
            j.tick(true, true);
        }
        assert!(j.power > 0.8 && j.power < 1.0, "{}", j.power);
        // Sem montaria a potência zera.
        assert_eq!(j.tick(false, true), None);
        assert_eq!(j.power, 0.0);
    }

    #[test]
    fn portal_sobe_e_desce() {
        let mut p = Portal::default();
        assert!(p.tick(true, 0), "primeiro tick toca o som");
        assert!(!p.tick(true, 0));
        assert!((p.time - 0.025).abs() < 1e-6);
        for _ in 0..100 {
            p.tick(true, 0);
        }
        assert_eq!(p.time, 1.0);
        p.tick(false, 0);
        assert!((p.time - 0.95).abs() < 1e-6);
        // Náusea longa também enche; curta não.
        let mut n = Portal::default();
        n.tick(false, 100);
        assert!((n.time - 0.006_666_667).abs() < 1e-7);
        // Curva: alfa 1.0 fica 1.0; 0.5 vira 0.5^4*0.8+0.2.
        let p = Portal { time: 0.5, prev: 0.5 };
        assert!((p.alpha(0.0) - (0.0625 * 0.8 + 0.2)).abs() < 1e-6);
        assert_eq!(Portal { time: 1.0, prev: 1.0 }.alpha(0.5), 1.0);
    }

    #[test]
    fn brilho_e_luz_do_dia() {
        assert!((light_brightness(15) - 1.0).abs() < 1e-6);
        assert_eq!(light_brightness(0), 0.0);
        // Meio-dia: nada subtraído; meia-noite: 11 níveis.
        assert_eq!(skylight_subtracted(6000), 0);
        assert_eq!(skylight_subtracted(18000), 11);
        assert!(brightness_at(0, 15, 18000) < brightness_at(0, 15, 6000));
        assert_eq!(brightness_at(15, 0, 18000), 1.0);
    }
}
