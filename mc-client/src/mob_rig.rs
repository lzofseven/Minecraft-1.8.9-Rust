//! Tipos das tabelas geradas por `scripts/gen_mob_models.py` (`gen_mob_models.rs`) e o `Rig`,
//! o estado animável de um modelo (`ModelRenderer` com pivô e rotação mutáveis por nome).

use glam::{Mat4, Vec3};

use crate::models::{Cuboid, Part, Sink};

#[derive(Clone, Copy, Debug)]
pub struct GenCube {
    pub uv: (u32, u32),
    pub from: [f32; 3],
    pub size: [u32; 3],
    pub inflate: f32,
    pub mirror: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct GenPart {
    pub name: &'static str,
    pub pivot: [f32; 3],
    pub rot: [f32; 3],
    /// `ModelRenderer.setTextureSize`; as UVs geradas já vêm nesse tamanho.
    pub tex: (u32, u32),
    pub cubes: &'static [GenCube],
    pub children: &'static [usize],
}

#[derive(Clone, Copy, Debug)]
pub struct GenModel {
    pub tex: (u32, u32),
    pub parts: &'static [GenPart],
}

/// Estado mutável de um modelo: pivô, rotação (rad) e visibilidade de cada parte.
#[derive(Clone, Debug)]
pub struct Rig {
    pub model: &'static GenModel,
    pub pivot: Vec<Vec3>,
    pub rot: Vec<Vec3>,
    pub show: Vec<bool>,
}

impl Rig {
    pub fn new(model: &'static GenModel) -> Self {
        Self {
            model,
            pivot: model.parts.iter().map(|p| Vec3::from(p.pivot)).collect(),
            rot: model.parts.iter().map(|p| Vec3::from(p.rot)).collect(),
            show: vec![true; model.parts.len()],
        }
    }

    /// Índice da parte pelo nome do campo no Java (`spiderLeg1`, `tentacles_3`...).
    pub fn i(&self, name: &str) -> usize {
        self.model.parts.iter().position(|p| p.name == name).unwrap_or_else(|| panic!("parte {name} inexistente"))
    }

    pub fn tex(&self) -> (f32, f32) {
        (self.model.tex.0 as f32, self.model.tex.1 as f32)
    }

    /// Tamanho da textura da parte (`ModelRenderer.setTextureSize`).
    pub fn part_tex(&self, i: usize) -> (f32, f32) {
        let t = self.model.parts[i].tex;
        (t.0 as f32, t.1 as f32)
    }

    /// A parte (e os filhos) como `Part`, com o estado atual.
    pub fn part(&self, i: usize) -> Part {
        let g = &self.model.parts[i];
        let mut p = Part::new(self.pivot[i].to_array());
        p.rot = self.rot[i];
        p.show = self.show[i];
        p.cubes = g
            .cubes
            .iter()
            .map(|c| Cuboid { from: Vec3::from(c.from), size: c.size, uv: c.uv, inflate: c.inflate, mirror: c.mirror })
            .collect();
        p.children = g.children.iter().map(|&c| self.part(c)).collect();
        p
    }

    /// `ModelRenderer.render` das partes raiz pedidas.
    pub fn emit(&self, sink: &mut Sink, m: &Mat4, roots: &[usize]) {
        let lights = sink.lights.map(Vec3::normalize);
        for &i in roots {
            // Cada `ModelRenderer` tem o próprio `textureWidth/Height` (golem 128x128, boneco de neve 64x64...).
            sink.tex = self.part_tex(i);
            self.emit_part(sink, &lights, i, m);
        }
    }

    /// `Sink::part` sobre a parte `i` e os filhos, lendo o modelo direto: as mesmas contas de `part` e
    /// `Part::matrix`, sem montar a árvore de `Part` (que copia os cubos a cada quadro e a cada entidade).
    fn emit_part(&self, sink: &mut Sink, lights: &[Vec3; 2], i: usize, parent: &Mat4) {
        if !self.show[i] {
            return;
        }
        let g = &self.model.parts[i];
        let (p, r) = (self.pivot[i], self.rot[i]);
        let m = *parent * Mat4::from_translation(p / 16.0) * Mat4::from_rotation_z(r.z) * Mat4::from_rotation_y(r.y) * Mat4::from_rotation_x(r.x);
        for c in g.cubes {
            sink.cube(&Cuboid { from: Vec3::from(c.from), size: c.size, uv: c.uv, inflate: c.inflate, mirror: c.mirror }, &m, lights);
        }
        for &c in g.children {
            self.emit_part(sink, lights, c, &m);
        }
    }

    /// `ModelRenderer.renderWithRotation`: ponto de giro e depois rotação Y, X, Z (não Z, Y, X como
    /// `render`), sem desenhar os filhos.
    pub fn emit_rot(&self, sink: &mut Sink, m: &Mat4, roots: &[usize]) {
        for &i in roots {
            sink.tex = self.part_tex(i);
            let g = &self.model.parts[i];
            let mut z = Part::new([0.0; 3]);
            z.rot.z = self.rot[i].z;
            z.cubes = g.cubes.iter().map(|c| Cuboid { from: Vec3::from(c.from), size: c.size, uv: c.uv, inflate: c.inflate, mirror: c.mirror }).collect();
            z.show = self.show[i];
            let mut x = Part::new([0.0; 3]);
            x.rot.x = self.rot[i].x;
            x.children = vec![z];
            let mut y = Part::new(self.pivot[i].to_array());
            y.rot.y = self.rot[i].y;
            y.children = vec![x];
            sink.part(&y, m);
        }
    }

    /// Todas as partes que não são filhas de outra, na ordem de criação.
    pub fn roots(&self) -> Vec<usize> {
        let mut child = vec![false; self.model.parts.len()];
        for p in self.model.parts {
            for &c in p.children {
                child[c] = true;
            }
        }
        (0..child.len()).filter(|&i| !child[i]).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen_mob_models as g;
    use crate::models::{FULL_LIGHT, WORLD_LIGHTS};
    use mc_world::mesh::Geometry;

    fn sink_teste(geo: &mut Geometry) -> Sink<'_> {
        Sink { geo, tex: (64.0, 32.0), tint: [0.9, 0.8, 0.7], alpha: 255, light: FULL_LIGHT, lights: WORLD_LIGHTS }
    }

    /// `emit` tem de dar o mesmo que `Sink::part` sobre a árvore montada por `part`, com pose e parte escondida.
    #[test]
    fn emit_igual_a_montar_a_arvore() {
        for model in [&g::CREEPER, &g::PIG, &g::COW, &g::SPIDER, &g::ARMOR_STAND, &g::HORSE] {
            let mut rig = Rig::new(model);
            for (i, r) in rig.rot.iter_mut().enumerate() {
                *r = Vec3::new(0.1 * i as f32, -0.2, 0.05 * i as f32);
            }
            if rig.show.len() > 2 {
                rig.show[2] = false;
            }
            let m = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0)) * Mat4::from_rotation_y(0.7);
            let roots = rig.roots();
            let (mut a, mut b) = (Geometry::default(), Geometry::default());
            let mut s = sink_teste(&mut a);
            rig.emit(&mut s, &m, &roots);
            let mut s = sink_teste(&mut b);
            for &i in &roots {
                s.tex = rig.part_tex(i);
                s.part(&rig.part(i), &m);
            }
            assert_eq!(a.indices, b.indices);
            assert_eq!(a.vertices.len(), b.vertices.len());
            for (x, y) in a.vertices.iter().zip(&b.vertices) {
                assert_eq!((x.pos, x.uv, x.color, x.light), (y.pos, y.uv, y.color, y.light));
            }
        }
    }
}
