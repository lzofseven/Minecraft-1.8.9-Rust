//! Conjunto de seções numa grade em volta da câmera, sem hash: a lista do `setupTerrain`
//! (as seções que a BFS alcançou) e as marcas da própria BFS.

type Key = (i32, i32, i32);

/// Grade de colunas `(2r + 1)²` centrada em `(ox, oz)`, 16 seções de altura cada.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SectionSet {
    ox: i32,
    oz: i32,
    r: i32,
    w: i32,
    bits: Vec<bool>,
    len: usize,
}

impl SectionSet {
    /// Vazio, cobrindo `r` colunas para cada lado de `(cx, cz)`.
    pub fn new(cx: i32, cz: i32, r: i32) -> Self {
        let r = r.max(0);
        let w = 2 * r + 1;
        Self { ox: cx, oz: cz, r, w, bits: vec![false; (w * w * 16) as usize], len: 0 }
    }

    /// Índice da coluna (sem a altura) dentro da grade.
    pub fn column(&self, cx: i32, cz: i32) -> Option<usize> {
        let (dx, dz) = (cx - self.ox + self.r, cz - self.oz + self.r);
        (dx >= 0 && dz >= 0 && dx < self.w && dz < self.w).then(|| (dx * self.w + dz) as usize)
    }

    /// Lado da grade em colunas e a coluna do canto (para percorrer as colunas).
    pub fn bounds(&self) -> (i32, i32, i32) {
        (self.ox - self.r, self.oz - self.r, self.w)
    }

    fn index(&self, k: &Key) -> Option<usize> {
        if !(0..16).contains(&k.1) {
            return None;
        }
        self.column(k.0, k.2).map(|c| c * 16 + k.1 as usize)
    }

    pub fn contains(&self, k: &Key) -> bool {
        self.index(k).is_some_and(|i| self.bits[i])
    }

    /// Marca a seção; `true` quando ela ainda não estava (fora da grade não entra).
    pub fn insert(&mut self, k: Key) -> bool {
        match self.index(&k) {
            Some(i) if !self.bits[i] => {
                self.bits[i] = true;
                self.len += 1;
                true
            }
            _ => false,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insere_uma_vez_e_ignora_fora_da_grade() {
        let mut s = SectionSet::new(10, -3, 2);
        assert!(s.insert((11, 4, -5)));
        assert!(!s.insert((11, 4, -5)));
        assert!(s.contains(&(11, 4, -5)) && !s.contains(&(11, 5, -5)));
        assert!(!s.insert((13, 0, -3)) && !s.insert((10, 16, -3)) && !s.insert((10, -1, -3)));
        assert_eq!(s.len(), 1);
    }
}
