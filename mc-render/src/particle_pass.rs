//! Partículas por instância: o cliente manda uma instância por `EntityFX` (centro interpolado, tamanho,
//! UV, cor, luz) e o vertex shader monta o quadrado com os vetores da câmera do `ActiveRenderInfo`
//! (`rotationX/XZ/Z/YZ/XY`), com as mesmas contas e a mesma ordem de vértices do `renderParticle`.

use std::ops::Range;

use crate::renderer::Tex;

/// Uma partícula do quadro.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ParticleInstance {
    /// `interpPos` já somado: centro do quadrado no mundo.
    pub pos: [f32; 3],
    /// Meio lado (`0.1 * particleScale`).
    pub size: f32,
    /// u0, u1, v0, v1.
    pub uv: [f32; 4],
    pub color: [u8; 4],
    /// Céu, bloco (0..255 = 0..15), 255, 0: como `Vertex::light`.
    pub light: [u8; 4],
    /// Bit 0: UV espelhada (partículas de ícone: `EntityDiggingFX`, `EntityBreakingFX`).
    pub flags: u32,
}

pub const MIRRORED: u32 = 1;

/// Trecho de instâncias com a mesma textura, na ordem do `EffectRenderer.renderParticles`.
#[derive(Clone, Debug)]
pub struct ParticleDraw {
    pub tex: Tex,
    /// Camada com alfa (`translucent`): sem escrita de profundidade.
    pub translucent: bool,
    pub range: Range<u32>,
}

/// As partículas do quadro.
#[derive(Default)]
pub struct ParticleBatch {
    /// `rotationX`, `rotationZ`, `rotationYZ`, `rotationXY`, `rotationXZ`.
    pub rot: [f32; 5],
    pub instances: Vec<ParticleInstance>,
    pub draws: Vec<ParticleDraw>,
}

/// Vertex shader, acrescentado ao shader do mundo (usa `VsOut`, `light_at` e o `fs_particle` de lá).
pub const WGSL: &str = r#"
struct ParticleCam {
    // rotationX, rotationZ, rotationYZ, rotationXY
    a: vec4<f32>,
    // rotationXZ
    b: vec4<f32>,
};
@group(1) @binding(1) var<uniform> pcam: ParticleCam;

struct ParticleIn {
    @location(0) pos: vec3<f32>,
    @location(1) size: f32,
    @location(2) uv: vec4<f32>,
    @location(3) color: vec4<f32>,
    @location(4) light: vec4<f32>,
    @location(5) flags: u32,
};

@vertex
fn vs_particle(@builtin(vertex_index) vi: u32, p: ParticleIn) -> VsOut {
    // Os dois triângulos do `quad_fixed`: 0 1 2, 0 2 3.
    var corners = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);
    let k = corners[vi];
    let s = p.size;
    let rx = pcam.a.x * s;
    let rz = pcam.a.y * s;
    let ryz = pcam.a.z * s;
    let rxy = pcam.a.w * s;
    let rxz = pcam.b.x * s;
    // (-rx - ryz, -rxz, -rz - rxy), (-rx + ryz, rxz, -rz + rxy), (rx + ryz, rxz, rz + rxy), (rx - ryz, -rxz, rz - rxy)
    var off: vec3<f32>;
    if (k == 0u) {
        off = vec3<f32>(-rx - ryz, -rxz, -rz - rxy);
    } else if (k == 1u) {
        off = vec3<f32>(-rx + ryz, rxz, -rz + rxy);
    } else if (k == 2u) {
        off = vec3<f32>(rx + ryz, rxz, rz + rxy);
    } else {
        off = vec3<f32>(rx - ryz, -rxz, rz - rxy);
    }
    let pos = p.pos + off;
    let mirrored = (p.flags & 1u) != 0u;
    let left = k < 2u;
    let u = select(select(p.uv.x, p.uv.y, mirrored), select(p.uv.y, p.uv.x, mirrored), left);
    let v = select(p.uv.z, p.uv.w, k == 0u || k == 3u);

    var o: VsOut;
    o.clip = g.view_proj * vec4<f32>(pos, 1.0);
    o.uv = vec2<f32>(u, v);
    o.color = vec4<f32>(p.color.rgb * light_at(p.light.x * 15.0, p.light.y * 15.0) * p.light.z, p.color.a);
    o.wpos = pos;
    o.ov = vec4<f32>(0.0);
    return o;
}
"#;

/// Buffer de instâncias, câmera e os dois pipelines (com e sem escrita de profundidade).
pub struct ParticlePass {
    with_depth: wgpu::RenderPipeline,
    no_depth: wgpu::RenderPipeline,
    cam: wgpu::Buffer,
    cam_bg: wgpu::BindGroup,
    instances: wgpu::Buffer,
    cap: u64,
}

impl ParticlePass {
    /// `shader`: o shader do mundo com `WGSL` no fim; `world` = layout do grupo 0.
    pub fn new(device: &wgpu::Device, shader: &str, world: &wgpu::BindGroupLayout, target: wgpu::TextureFormat, depth: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("partículas"), source: wgpu::ShaderSource::Wgsl(shader.into()) });
        let cam_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("câmera das partículas"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let cam = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("câmera das partículas"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let cam_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &cam_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 1, resource: cam.as_entire_binding() }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("partículas"), bind_group_layouts: &[world, &cam_bgl], push_constant_ranges: &[] });
        let attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32, 2 => Float32x4, 3 => Unorm8x4, 4 => Unorm8x4, 5 => Uint32];
        let pipeline = |write: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("partículas"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_particle"),
                    compilation_options: Default::default(),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<ParticleInstance>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &attrs,
                    }],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs_particle"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format: target, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })],
                }),
                primitive: Default::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: depth,
                    depth_write_enabled: write,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview: None,
                cache: None,
            })
        };
        let cap = 64 * 1024;
        Self { with_depth: pipeline(true), no_depth: pipeline(false), cam, cam_bg, instances: Self::alloc(device, cap), cap }
    }

    fn alloc(device: &wgpu::Device, size: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("instâncias das partículas"),
            size,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Sobe as instâncias e a câmera do quadro (antes do passe).
    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, b: &ParticleBatch) {
        if b.instances.is_empty() {
            return;
        }
        let bytes: &[u8] = bytemuck::cast_slice(&b.instances);
        if bytes.len() as u64 > self.cap {
            self.cap = (bytes.len() as u64).next_power_of_two();
            self.instances = Self::alloc(device, self.cap);
        }
        queue.write_buffer(&self.instances, 0, bytes);
        let r = b.rot;
        let cam: [f32; 8] = [r[0], r[1], r[2], r[3], r[4], 0.0, 0.0, 0.0];
        queue.write_buffer(&self.cam, 0, bytemuck::cast_slice(&cam));
    }

    /// Um trecho; `bg` é o grupo 0 da textura dele.
    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, d: &ParticleDraw, bg: &'a wgpu::BindGroup) {
        if d.range.is_empty() {
            return;
        }
        pass.set_pipeline(if d.translucent { &self.no_depth } else { &self.with_depth });
        pass.set_bind_group(0, bg, &[]);
        pass.set_bind_group(1, &self.cam_bg, &[]);
        pass.set_vertex_buffer(0, self.instances.slice(..));
        pass.draw(0..6, d.range.clone());
    }
}
