//! Partes do `GuiEnchantment` que não são desenho: `java.util.Random`, `EnchantmentNameParts` (nomes em
//! runas), animação do livro (`updateScreen`) e o nome traduzido do encantamento da dica.

use crate::gen_containers::ENCHANT_NAME_PARTS;
use crate::gen_container_items::ENCHANTS;

/// `java.util.Random` (LCG de 48 bits), igual ao do jogo.
#[derive(Clone, Debug)]
pub struct JavaRandom {
    seed: i64,
}

const MULT: i64 = 0x5DEECE66D;
const MASK: i64 = (1 << 48) - 1;

impl JavaRandom {
    pub fn new(seed: i64) -> Self {
        Self { seed: (seed ^ MULT) & MASK }
    }

    pub fn set_seed(&mut self, seed: i64) {
        self.seed = (seed ^ MULT) & MASK;
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.seed = (self.seed.wrapping_mul(MULT).wrapping_add(0xB)) & MASK;
        (self.seed >> (48 - bits)) as i32
    }

    pub fn next_int(&mut self, bound: i32) -> i32 {
        if bound & (-bound) == bound {
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

/// `EnchantmentNameParts.generateNewRandomName` com o gerador já semeado.
pub fn random_name(rng: &mut JavaRandom) -> String {
    let n = rng.next_int(2) + 3;
    let mut out = String::new();
    for i in 0..n {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(ENCHANT_NAME_PARTS[rng.next_int(ENCHANT_NAME_PARTS.len() as i32) as usize]);
    }
    out
}

/// Os três nomes mostrados para a semente `xp_seed` (`reseedRandomGenerator` + 3x `generateNewRandomName`).
pub fn names_for_seed(xp_seed: i32) -> [String; 3] {
    let mut rng = JavaRandom::new(xp_seed as i64);
    [random_name(&mut rng), random_name(&mut rng), random_name(&mut rng)]
}

/// `Enchantment.getTranslatedName(level)`.
pub fn translated_name(id: u8, level: i32) -> Option<String> {
    let key = ENCHANTS.iter().find(|(i, _)| *i == id)?.1;
    Some(format!("{} {}", crate::lang::tr(key), crate::lang::tr(&format!("enchantment.level.{level}"))))
}

/// `ModelBook.render(null, tempo, páginaDireita, páginaEsquerda, abertura, 0, 1/16)` com o
/// `setRotationAngles` (capas, páginas e as duas folhas virando).
pub fn emit_book(sink: &mut crate::models::Sink, m: &glam::Mat4, time: f32, right: f32, left: f32, open: f32) {
    use crate::mth::sin;
    let mut rig = crate::mob_rig::Rig::new(&crate::gen_mob_models::BOOK);
    let f = (sin(time * 0.02) * 0.1 + 1.25) * open;
    let [cr, cl, spine, pr, pl, fr, fl] = ["coverRight", "coverLeft", "bookSpine", "pagesRight", "pagesLeft", "flippingPageRight", "flippingPageLeft"].map(|n| rig.i(n));
    rig.rot[cr].y = std::f32::consts::PI + f;
    rig.rot[cl].y = -f;
    rig.rot[pr].y = f;
    rig.rot[pl].y = -f;
    rig.rot[fr].y = f - f * 2.0 * right;
    rig.rot[fl].y = f - f * 2.0 * left;
    for p in [pr, pl, fr, fl] {
        rig.pivot[p].x = sin(f);
    }
    rig.emit(sink, m, &[cr, cl, spine, pr, pl, fr, fl]);
}

/// Estado animado do livro (`field_147073_u` ... `field_147076_A`) e seu `updateScreen`.
#[derive(Clone, Debug)]
pub struct BookAnim {
    pub tick_count: i32,
    pub flip: f32,
    pub o_flip: f32,
    pub flip_t: f32,
    pub flip_a: f32,
    pub open: f32,
    pub o_open: f32,
    pub last_item: Option<mc_protocol::nbt::Slot>,
    rng: JavaRandom,
}

impl Default for BookAnim {
    fn default() -> Self {
        Self { tick_count: 0, flip: 0.0, o_flip: 0.0, flip_t: 0.0, flip_a: 0.0, open: 0.0, o_open: 0.0, last_item: None, rng: JavaRandom::new(crate::inventory::now_ms() as i64) }
    }
}

impl BookAnim {
    /// `GuiEnchantment.func_147068_g`.
    pub fn tick(&mut self, item: Option<&mc_protocol::nbt::Slot>, any_level: bool) {
        let same = match (&self.last_item, item) {
            (None, None) => true,
            (Some(a), Some(b)) => a.id == b.id && a.damage == b.damage && a.count == b.count && a.raw_nbt == b.raw_nbt,
            _ => false,
        };
        if !same {
            self.last_item = item.cloned();
            loop {
                self.flip_t += (self.rng.next_int(4) - self.rng.next_int(4)) as f32;
                if self.flip > self.flip_t + 1.0 || self.flip < self.flip_t - 1.0 {
                    break;
                }
            }
        }
        self.tick_count += 1;
        self.o_flip = self.flip;
        self.o_open = self.open;
        if any_level {
            self.open += 0.2;
        } else {
            self.open -= 0.2;
        }
        self.open = self.open.clamp(0.0, 1.0);
        let d = ((self.flip_t - self.flip) * 0.4).clamp(-0.2, 0.2);
        self.flip_a += (d - self.flip_a) * 0.9;
        self.flip += self.flip_a;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_random_matches_known_sequence() {
        // new Random(42): nextInt(10) -> 0, 3, 8 e nextInt(100) -> 30, 63, 48 (valores conhecidos do JDK).
        let mut r = JavaRandom::new(42);
        assert_eq!((r.next_int(100), r.next_int(100), r.next_int(100)), (30, 63, 48));
        let mut r = JavaRandom::new(42);
        assert_eq!((r.next_int(10), r.next_int(10), r.next_int(10)), (0, 3, 8));
        // new Random(0).nextInt(10) -> 0, 8, 9, 7, 5
        let mut r = JavaRandom::new(0);
        let v: Vec<i32> = (0..5).map(|_| r.next_int(10)).collect();
        assert_eq!(v, vec![0, 8, 9, 7, 5]);
    }

    #[test]
    fn names_have_three_or_four_words() {
        for seed in [0, 1, 123456, -77] {
            for n in names_for_seed(seed) {
                let words = n.split(' ').count();
                assert!((3..=4).contains(&words), "{n}");
                assert!(n.split(' ').all(|w| ENCHANT_NAME_PARTS.contains(&w)));
            }
        }
    }

    #[test]
    fn book_closes_without_levels() {
        let mut b = BookAnim::default();
        for _ in 0..10 {
            b.tick(None, true);
        }
        assert_eq!(b.open, 1.0);
        for _ in 0..10 {
            b.tick(None, false);
        }
        assert_eq!(b.open, 0.0);
    }
}
