//! `BlockFluidRenderer` e o que ele pede a `BlockLiquid`: altura por canto, direção do fluxo,
//! faces laterais e luz. Coordenadas locais da seção (`Padded`).

use crate::biome;
use crate::blocks::BlockTable;
use crate::math;
use crate::mesh::{atlas_uv, flat, max_light, tint_rgba, Geometry, Padded, Vertex};

/// Mesmo líquido (`getMaterial() == material`): 8/9 água, 10/11 lava.
#[inline]
pub fn same(state: u16, lava: bool) -> bool {
    let id = state >> 4;
    if lava {
        id == 10 || id == 11
    } else {
        id == 8 || id == 9
    }
}

/// `BlockLiquid.getLiquidHeightPercent`.
fn height_percent(level: u16) -> f32 {
    (if level >= 8 { 1 } else { level + 1 }) as f32 / 9.0
}

/// `BlockFluidRenderer.getFluidHeight`: média dos 4 blocos que dividem o canto (x, z).
pub fn fluid_height(p: &Padded, t: &BlockTable, x: i32, y: i32, z: i32, lava: bool) -> f32 {
    let (mut count, mut sum) = (0i32, 0.0f32);
    for i in 0..4 {
        let (bx, bz) = (x - (i & 1), z - (i >> 1 & 1));
        if same(p.block(bx, y + 1, bz), lava) {
            return 1.0;
        }
        let st = p.block(bx, y, bz);
        if !same(st, lava) {
            if !t.material_solid(st) {
                sum += 1.0;
                count += 1;
            }
        } else {
            let level = st & 15;
            if level >= 8 || level == 0 {
                sum += height_percent(level) * 10.0;
                count += 10;
            }
            sum += height_percent(level);
            count += 1;
        }
    }
    1.0 - sum / count as f32
}

/// `BlockLiquid.getEffectiveFlowDecay`: -1 quando o bloco não é o mesmo líquido.
fn flow_decay(p: &Padded, x: i32, y: i32, z: i32, lava: bool) -> i32 {
    let st = p.block(x, y, z);
    if !same(st, lava) {
        return -1;
    }
    let l = (st & 15) as i32;
    if l >= 8 {
        0
    } else {
        l
    }
}

/// `BlockLiquid.isBlockSolid(world, pos, side)` para lados horizontais.
fn liquid_block_solid(p: &Padded, t: &BlockTable, x: i32, y: i32, z: i32, lava: bool) -> bool {
    let st = p.block(x, y, z);
    if same(st, lava) {
        return false;
    }
    // Gelo (id 79) não conta como sólido para o líquido.
    st >> 4 != 79 && t.material_solid(st)
}

/// Vetor de `Vec3.normalize` (zero abaixo de 1e-4).
fn normalize(v: [f64; 3]) -> [f64; 3] {
    let d = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if d < 1.0e-4 {
        [0.0; 3]
    } else {
        [v[0] / d, v[1] / d, v[2] / d]
    }
}

/// `BlockLiquid.getFlowVector`.
pub fn flow_vector(p: &Padded, t: &BlockTable, x: i32, y: i32, z: i32, lava: bool) -> [f64; 3] {
    let mut v = [0.0f64; 3];
    let here = flow_decay(p, x, y, z, lava);
    // EnumFacing.Plane.HORIZONTAL: norte, leste, sul, oeste.
    let sides = [(0, -1), (1, 0), (0, 1), (-1, 0)];
    for (dx, dz) in sides {
        let (nx, nz) = (x + dx, z + dz);
        let mut n = flow_decay(p, nx, y, nz, lava);
        if n < 0 {
            if !t.material_blocks_movement(p.block(nx, y, nz)) {
                n = flow_decay(p, nx, y - 1, nz, lava);
                if n >= 0 {
                    let w = (n - (here - 8)) as f64;
                    v[0] += dx as f64 * w;
                    v[2] += dz as f64 * w;
                }
            }
        } else {
            let w = (n - here) as f64;
            v[0] += dx as f64 * w;
            v[2] += dz as f64 * w;
        }
    }
    if p.block(x, y, z) & 15 >= 8 {
        for (dx, dz) in sides {
            let (nx, nz) = (x + dx, z + dz);
            // `isBlockSolid(world, up, side)` com o lado horizontal: o bloco de cima do vizinho.
            if liquid_block_solid(p, t, nx, y, nz, lava) || liquid_block_solid(p, t, nx, y + 1, nz, lava) {
                let n = normalize(v);
                v = [n[0], n[1] - 6.0, n[2]];
                break;
            }
        }
    }
    normalize(v)
}

/// `BlockLiquid.getFlowDirection`: ângulo do fluxo, ou `None` (-1000) em água parada.
pub fn flow_direction(p: &Padded, t: &BlockTable, x: i32, y: i32, z: i32, lava: bool) -> Option<f32> {
    let v = flow_vector(p, t, x, y, z, lava);
    if v[0] == 0.0 && v[2] == 0.0 {
        None
    } else {
        Some((math::atan2(v[2], v[0]) - 1.5707963267948966) as f32)
    }
}

/// `Block.getMixedBrightnessForBlock`: o maior entre a luz do bloco e a do bloco de cima.
fn mixed(p: &Padded, x: i32, y: i32, z: i32) -> [u8; 4] {
    flat(max_light(p.light(x, y, z), p.light(x, y + 1, z)))
}

/// `BlockLiquid.shouldSideBeRendered` para a face voltada ao vizinho `(nx, ny, nz)`.
fn side_visible(p: &Padded, t: &BlockTable, nx: i32, ny: i32, nz: i32, up: bool, lava: bool) -> bool {
    let n = p.block(nx, ny, nz);
    if same(n, lava) {
        return false;
    }
    up || !t.get(n).full_opaque
}

/// `BlockLiquid.shouldRenderSides`: algum vizinho 3x3 que não é do líquido nem bloco inteiro.
fn render_sides(p: &Padded, t: &BlockTable, x: i32, y: i32, z: i32, lava: bool) -> bool {
    for dz in -1..=1 {
        for dx in -1..=1 {
            let n = p.block(x + dx, y, z + dz);
            if !same(n, lava) && !t.full_block(n) {
                return true;
            }
        }
    }
    false
}

/// `BlockFluidRenderer.renderFluid` para o bloco em `(x, y, z)`; `wpos` é a posição no mundo.
/// Devolve se algo foi desenhado.
pub fn render_fluid(geo: &mut Geometry, p: &Padded, t: &BlockTable, [x, y, z]: [i32; 3], wpos: [i32; 3], lava: bool) -> bool {
    let sprites = t.fluid_tex[lava as usize];
    let color = if lava {
        0xFFFFFF
    } else {
        let origin = [wpos[0] - x, wpos[2] - z];
        biome::average_3x3(&|bx, bz| p.biome(bx - origin[0], bz - origin[1]), wpos[0], wpos[2], &|b, _, _| b.water_color)
    };
    let up = side_visible(p, t, x, y + 1, z, true, lava);
    let down = side_visible(p, t, x, y - 1, z, false, lava);
    // Norte (-z), sul (+z), oeste (-x), leste (+x).
    let dirs = [(0, -1), (0, 1), (-1, 0), (1, 0)];
    let sides: Vec<bool> = dirs.iter().map(|&(dx, dz)| side_visible(p, t, x + dx, y, z + dz, false, lava)).collect();
    if !up && !down && !sides.iter().any(|&s| s) {
        return false;
    }
    let base = [wpos[0] as f32, wpos[1] as f32, wpos[2] as f32];
    let mut h = [
        fluid_height(p, t, x, y, z, lava),
        fluid_height(p, t, x, y, z + 1, lava),
        fluid_height(p, t, x + 1, y, z + 1, lava),
        fluid_height(p, t, x + 1, y, z, lava),
    ];
    let v = |pos: [f32; 3], u: f32, vv: f32, rgba: [u8; 4], light: [u8; 4], tex: u16| Vertex {
        pos: [base[0] + pos[0], base[1] + pos[1], base[2] + pos[2]],
        uv: atlas_uv(tex, u, vv),
        color: rgba,
        light,
    };
    const E: f32 = 0.001;
    if up {
        for k in &mut h {
            *k -= E;
        }
        let dir = flow_direction(p, t, x, y, z, lava);
        let tex = if dir.is_some() { sprites[1] } else { sprites[0] };
        let (u, vv): ([f32; 4], [f32; 4]) = match dir {
            None => ([0.0, 0.0, 16.0, 16.0], [0.0, 16.0, 16.0, 0.0]),
            Some(a) => {
                let (s, c) = (math::sin(a) * 0.25, math::cos(a) * 0.25);
                (
                    [8.0 + (-c - s) * 16.0, 8.0 + (-c + s) * 16.0, 8.0 + (c + s) * 16.0, 8.0 + (c - s) * 16.0],
                    [8.0 + (-c + s) * 16.0, 8.0 + (c + s) * 16.0, 8.0 + (c - s) * 16.0, 8.0 + (-c - s) * 16.0],
                )
            }
        };
        let rgba = tint_rgba(color, 1.0);
        let l = mixed(p, x, y, z);
        let top = [
            v([0.0, h[0], 0.0], u[0], vv[0], rgba, l, tex),
            v([0.0, h[1], 1.0], u[1], vv[1], rgba, l, tex),
            v([1.0, h[2], 1.0], u[2], vv[2], rgba, l, tex),
            v([1.0, h[3], 0.0], u[3], vv[3], rgba, l, tex),
        ];
        geo.quad_fixed(top);
        if render_sides(p, t, x, y + 1, z, lava) {
            geo.quad_fixed([top[0], top[3], top[2], top[1]]);
        }
    }
    if down {
        let rgba = tint_rgba(0xFFFFFF, 0.5);
        let l = mixed(p, x, y - 1, z);
        let tex = sprites[0];
        geo.quad_fixed([
            v([0.0, 0.0, 1.0], 0.0, 16.0, rgba, l, tex),
            v([0.0, 0.0, 0.0], 0.0, 0.0, rgba, l, tex),
            v([1.0, 0.0, 0.0], 16.0, 0.0, rgba, l, tex),
            v([1.0, 0.0, 1.0], 16.0, 16.0, rgba, l, tex),
        ]);
    }
    for (i, &(dx, dz)) in dirs.iter().enumerate() {
        if !sides[i] {
            continue;
        }
        // Alturas e extremos de cada lado, na ordem do original.
        let (h0, h1, a, b) = match i {
            0 => (h[0], h[3], [0.0, E], [1.0, E]),
            1 => (h[2], h[1], [1.0, 1.0 - E], [0.0, 1.0 - E]),
            2 => (h[1], h[0], [E, 1.0], [E, 0.0]),
            _ => (h[3], h[2], [1.0 - E, 0.0], [1.0 - E, 1.0]),
        };
        let tex = sprites[1];
        let v0 = (1.0 - h0) * 16.0 * 0.5;
        let v1 = (1.0 - h1) * 16.0 * 0.5;
        let shade = if i < 2 { 0.8 } else { 0.6 };
        let rgba = tint_rgba(color, shade);
        let l = mixed(p, x + dx, y, z + dz);
        let q = [
            v([a[0], h0, a[1]], 0.0, v0, rgba, l, tex),
            v([b[0], h1, b[1]], 8.0, v1, rgba, l, tex),
            v([b[0], 0.0, b[1]], 8.0, 8.0, rgba, l, tex),
            v([a[0], 0.0, a[1]], 0.0, 8.0, rgba, l, tex),
        ];
        geo.quad_fixed(q);
        geo.quad_fixed([q[3], q[2], q[1], q[0]]);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world(f: impl Fn(i32, i32, i32) -> u16) -> Padded {
        let mut p = Padded::new(0xF0);
        for y in -3..19 {
            for z in -3..19 {
                for x in -3..19 {
                    p.blocks[Padded::idx(x, y, z)] = f(x, y, z);
                }
            }
        }
        p
    }

    const WATER: u16 = 9 << 4;
    const STONE: u16 = 1 << 4;

    #[test]
    fn source_block_under_air_is_8_9_tall_at_every_corner() {
        let t = BlockTable::new();
        let p = world(|_, y, _| if y == 5 { WATER } else if y < 5 { STONE } else { 0 });
        // Fonte (nível 0): (1 - (10 + 1/9*10... a média ponderada dá 1 - 1/9.
        for (x, z) in [(5, 5), (6, 5), (5, 6), (6, 6)] {
            let h = fluid_height(&p, &t, x, 5, z, false);
            assert!((h - 8.0 / 9.0).abs() < 1e-6, "{h}");
        }
    }

    #[test]
    fn water_below_water_is_full_height() {
        let t = BlockTable::new();
        let p = world(|_, y, _| if y <= 6 { WATER } else { 0 });
        assert_eq!(fluid_height(&p, &t, 5, 5, 5, false), 1.0);
    }

    #[test]
    fn corner_heights_follow_the_weighted_average() {
        let t = BlockTable::new();
        // Fonte em x = 5 e fluxo nível 3 em x = 6 (todas as linhas z iguais); o resto do nível é ar.
        let p = world(|x, y, _| match (x, y) {
            (5, 5) => WATER,
            (6, 5) => (9 << 4) | 3,
            (_, 5) => 0,
            (_, y) if y < 5 => STONE,
            _ => 0,
        });
        // Cada fonte pesa 10 x (1/9) + 1/9 em 11 amostras; fluxo nível 3 pesa 4/9 em 1; ar pesa 1 em 1.
        let (src, flow) = (11.0f32 / 9.0, 4.0f32 / 9.0);
        let cases = [(5, 1.0 - (2.0 * src + 2.0) / 24.0), (6, 1.0 - (2.0 * flow + 2.0 * src) / 24.0), (7, 1.0 - (2.0 + 2.0 * flow) / 4.0)];
        for (x, want) in cases {
            let got = fluid_height(&p, &t, x, 5, 5, false);
            assert!((got - want).abs() < 1e-5, "x={x}: {got} != {want}");
        }
    }

    #[test]
    fn flow_points_downhill_and_still_water_has_no_direction() {
        let t = BlockTable::new();
        let still = world(|_, y, _| if y == 5 { WATER } else if y < 5 { STONE } else { 0 });
        assert_eq!(flow_direction(&still, &t, 5, 5, 5, false), None);
        // Nível 0 em x = 5 e nível 4 em x = 6: o fluxo vai para +x.
        let p = world(|x, y, _| match (x, y) {
            (5, 5) => WATER,
            (6, 5) => (9 << 4) | 4,
            (_, 5) => STONE,
            (_, y) if y < 5 => STONE,
            _ => 0,
        });
        let v = flow_vector(&p, &t, 5, 5, 5, false);
        assert!(v[0] > 0.99 && v[1] == 0.0 && v[2].abs() < 1e-9, "{v:?}");
        // atan2(0, 1) - pi/2.
        let a = flow_direction(&p, &t, 5, 5, 5, false).unwrap();
        assert!((a + std::f32::consts::FRAC_PI_2).abs() < 1e-4, "{a}");
    }

    #[test]
    fn buried_water_draws_nothing_and_exposed_top_draws_one_quad() {
        let t = BlockTable::new();
        let mut geo = Geometry::default();
        let buried = world(|_, _, _| WATER);
        assert!(!render_fluid(&mut geo, &buried, &t, [5, 5, 5], [5, 5, 5], false));
        assert!(geo.is_empty());
        let top = world(|_, y, _| if y == 5 { WATER } else if y < 5 { STONE } else { 0 });
        assert!(render_fluid(&mut geo, &top, &t, [5, 5, 5], [5, 5, 5], false));
        // Topo (2 se houver lado a renderizar: vizinhos de água não contam) + 4 lados invisíveis + fundo oculto.
        let tris = geo.indices.len() / 3;
        assert!(tris >= 2, "{tris}");
    }
}
