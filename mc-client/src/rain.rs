//! `EntityRenderer.addRainParticles` (1.8.9): gotas e respingos da chuva sobre o chão e a água,
//! fumaça sobre a lava e o som `ambient.weather.rain`. Roda uma vez por tick, logo depois do
//! `++rendererUpdateCount` de `updateRenderer`.

use mc_render::sky::{biome_can_rain, float_temperature};
use mc_world::blocks::BlockTable;
use mc_world::jmath::JRandom;
use mc_world::World;

/// `EnumParticleTypes.WATER_DROP` (39) e `SMOKE_NORMAL` (11).
pub const WATER_DROP: i32 = 39;
pub const SMOKE_NORMAL: i32 = 11;
/// Som tocado quando sobra uma gota (`world.playSound(..., "ambient.weather.rain", ...)`).
pub const RAIN_SOUND: &str = "ambient.weather.rain";

/// O que o tick da chuva pede ao jogo, na ordem do Java.
#[derive(Clone, Debug, PartialEq)]
pub enum RainFx {
    Particle { id: i32, pos: [f64; 3] },
    Sound { volume: f32, pitch: f32, pos: [f64; 3] },
}

/// O que `addRainParticles` pergunta ao mundo.
pub trait RainWorld {
    /// `World.getBiomeGenForCoords`: id do bioma da coluna.
    fn biome(&self, x: i32, z: i32) -> u8;
    /// `World.getPrecipitationHeight(pos).getY()`.
    fn precipitation_height(&self, x: i32, z: i32) -> i32;
    /// Estado do bloco (id << 4 | metadado).
    fn block(&self, x: i32, y: i32, z: i32) -> u16;
}

impl RainWorld for World {
    fn biome(&self, x: i32, z: i32) -> u8 {
        self.biome_at(x, z)
    }

    fn precipitation_height(&self, x: i32, z: i32) -> i32 {
        World::precipitation_height(self, x, z)
    }

    fn block(&self, x: i32, y: i32, z: i32) -> u16 {
        World::block(self, x, y, z)
    }
}

/// `Block.getBlockBoundsMinY/MaxY` depois de `setBlockBoundsBasedOnState`. Aproximado pelas caixas
/// de colisão sem vizinhos; sem caixa vale o bloco cheio, o padrão do Java.
fn y_bounds(table: &BlockTable, st: u16) -> (f64, f64) {
    let mut boxes = Vec::new();
    table.collision(st, &|_: i32, _: i32, _: i32| 0u16, &mut boxes);
    if boxes.is_empty() {
        return (0.0, 1.0);
    }
    let lo = boxes.iter().map(|b| b[1]).fold(f64::INFINITY, f64::min);
    let hi = boxes.iter().map(|b| b[4]).fold(f64::NEG_INFINITY, f64::max);
    (lo, hi)
}

/// `EntityRenderer.addRainParticles`. `pos` são os pés do jogador (`entity.posX/Y/Z`, sem
/// interpolar); `strength` é `getRainStrength(1.0)`; `setting` é `particleSetting` (0 todas,
/// 1 diminuídas, 2 mínimas); `counter` é `rainSoundCounter`.
pub fn add_rain_particles(
    w: &dyn RainWorld,
    table: &BlockTable,
    tick: i32,
    pos: [f64; 3],
    strength: f32,
    fancy: bool,
    setting: i32,
    counter: &mut i32,
) -> Vec<RainFx> {
    let mut out = Vec::new();
    let mut f = strength;
    if !fancy {
        f /= 2.0;
    }
    if f == 0.0 {
        return out;
    }
    // `random.setSeed((long)rendererUpdateCount * 312987231L)`.
    let mut rng = JRandom::new(tick as i64 * 312_987_231);
    let (bx, by, bz) = (pos[0].floor() as i32, pos[1].floor() as i32, pos[2].floor() as i32);
    let range = 10;
    // `(int)(100.0F * f * f)`, depois a opção de partículas: 1 corta pela metade, 2 zera.
    let mut k = (100.0f32 * f * f) as i32;
    match setting {
        1 => k >>= 1,
        2 => k = 0,
        _ => {}
    }
    let (mut d0, mut d1, mut d2) = (0.0f64, 0.0f64, 0.0f64);
    let mut j = 0;
    for _ in 0..k {
        // Os argumentos de `BlockPos.add` são avaliados da esquerda para a direita, como no Java.
        let dx = rng.next_int(range) - rng.next_int(range);
        let dz = rng.next_int(range) - rng.next_int(range);
        let (x, z) = (bx + dx, bz + dz);
        // blockpos1 = topo da chuva; blockpos2 = o bloco logo abaixo.
        let top = w.precipitation_height(x, z);
        let biome = w.biome(x, z);
        let below = top - 1;
        let st = w.block(x, below, z);
        if top <= by + range && top >= by - range && biome_can_rain(biome) && float_temperature(biome, x, top, z) >= 0.15 {
            let d3 = rng.next_double();
            let d4 = rng.next_double();
            let mat = st >> 4;
            if mat == 10 || mat == 11 {
                // Lava: fumaça na altura do topo (blockpos1), descontando a mínima do bloco.
                let (min_y, _) = y_bounds(table, st);
                let y = (top as f32 + 0.1f32) as f64 - min_y;
                out.push(RainFx::Particle { id: SMOKE_NORMAL, pos: [x as f64 + d3, y, z as f64 + d4] });
            } else if mat != 0 {
                let (_, max_y) = y_bounds(table, st);
                j += 1;
                if rng.next_int(j) == 0 {
                    d0 = x as f64 + d3;
                    d1 = (below as f32 + 0.1f32) as f64 + max_y - 1.0;
                    d2 = z as f64 + d4;
                }
                let y = (below as f32 + 0.1f32) as f64 + max_y;
                out.push(RainFx::Particle { id: WATER_DROP, pos: [x as f64 + d3, y, z as f64 + d4] });
            }
        }
    }
    // `j > 0 && random.nextInt(3) < rainSoundCounter++`: o contador sobe mesmo quando não toca.
    if j > 0 {
        let r = rng.next_int(3);
        let old = *counter;
        *counter += 1;
        if r < old {
            *counter = 0;
            let p = [d0, d1, d2];
            if d1 > (by + 1) as f64 && w.precipitation_height(bx, bz) > by {
                out.push(RainFx::Sound { volume: 0.1, pitch: 0.5, pos: p });
            } else {
                out.push(RainFx::Sound { volume: 0.2, pitch: 1.0, pos: p });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Chuva caindo sobre a planície (bioma 1) com a chuva em `top` e `below` sob ela.
    struct Plano {
        top: i32,
        below: u16,
    }

    impl RainWorld for Plano {
        fn biome(&self, _: i32, _: i32) -> u8 {
            1
        }

        fn precipitation_height(&self, _: i32, _: i32) -> i32 {
            self.top
        }

        fn block(&self, _: i32, y: i32, _: i32) -> u16 {
            if y == self.top - 1 {
                self.below
            } else {
                0
            }
        }
    }

    const AGUA: u16 = 8 << 4;
    const LAVA: u16 = 10 << 4;

    fn rodar(w: &Plano, tick: i32, forca: f32, fancy: bool, setting: i32, c: &mut i32) -> Vec<RainFx> {
        add_rain_particles(w, &BlockTable::default(), tick, [0.5, 69.0, 0.5], forca, fancy, setting, c)
    }

    fn gotas(fx: &[RainFx]) -> usize {
        fx.iter().filter(|f| matches!(f, RainFx::Particle { .. })).count()
    }

    #[test]
    fn sem_chuva_nao_faz_nada() {
        let mut c = 0;
        assert!(rodar(&Plano { top: 70, below: AGUA }, 5, 0.0, true, 0, &mut c).is_empty());
        assert_eq!(c, 0);
    }

    #[test]
    fn agua_gera_gotas_no_topo_da_agua() {
        let mut c = 0;
        let fx = rodar(&Plano { top: 70, below: AGUA }, 3, 1.0, true, 0, &mut c);
        assert!(gotas(&fx) > 0);
        for f in &fx {
            if let RainFx::Particle { id, pos } = f {
                assert_eq!(*id, WATER_DROP);
                // blockpos2.y = 69: (69 + 0.1) + maxY (1.0, bloco cheio).
                assert!((pos[1] - 70.1).abs() < 1e-3, "{}", pos[1]);
            }
        }
    }

    #[test]
    fn lava_gera_fumaca_e_nao_gotas() {
        let mut c = 0;
        let fx = rodar(&Plano { top: 70, below: LAVA }, 3, 1.0, true, 0, &mut c);
        assert!(!fx.is_empty());
        for f in &fx {
            if let RainFx::Particle { id, pos } = f {
                assert_eq!(*id, SMOKE_NORMAL);
                // blockpos1.y = 70: (70 + 0.1) - minY (0.0, lava sem caixa).
                assert!((pos[1] - 70.1).abs() < 1e-3);
            }
        }
    }

    #[test]
    fn opcao_minima_nao_gera_particulas() {
        let mut c = 0;
        let fx = rodar(&Plano { top: 70, below: AGUA }, 3, 1.0, true, 2, &mut c);
        assert!(fx.is_empty());
        assert_eq!(c, 0);
    }

    #[test]
    fn opcao_diminuida_e_chuva_nao_fancy_geram_menos() {
        let w = Plano { top: 70, below: AGUA };
        let (mut todas, mut metade, mut sem_fancy) = (0, 0, 0);
        for t in 0..80 {
            let mut c = 0;
            todas += gotas(&rodar(&w, t, 1.0, true, 0, &mut c));
            let mut c = 0;
            metade += gotas(&rodar(&w, t, 1.0, true, 1, &mut c));
            let mut c = 0;
            sem_fancy += gotas(&rodar(&w, t, 1.0, false, 0, &mut c));
        }
        assert!(metade > 0 && metade < todas, "{metade} vs {todas}");
        assert!(sem_fancy > 0 && sem_fancy < todas, "{sem_fancy} vs {todas}");
    }

    #[test]
    fn som_da_chuva_segue_o_contador() {
        let w = Plano { top: 70, below: AGUA };
        let mut c = 0;
        let mut sons = Vec::new();
        for t in 0..200 {
            for f in rodar(&w, t, 1.0, true, 0, &mut c) {
                if let RainFx::Sound { volume, pitch, .. } = f {
                    sons.push((volume, pitch));
                }
            }
        }
        assert!(!sons.is_empty());
        for (v, p) in sons {
            assert!((v == 0.1 && p == 0.5) || (v == 0.2 && p == 1.0), "{v} {p}");
        }
    }

    #[test]
    fn mesmo_tick_mesmo_resultado() {
        let w = Plano { top: 70, below: AGUA };
        let (mut a, mut b) = (0, 0);
        assert_eq!(rodar(&w, 7, 1.0, true, 0, &mut a), rodar(&w, 7, 1.0, true, 0, &mut b));
    }
}
