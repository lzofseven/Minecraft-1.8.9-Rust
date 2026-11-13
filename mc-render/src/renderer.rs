use std::collections::HashMap;
use std::sync::Arc;

use glam::{Mat4, Vec3, Vec4};
use mc_world::mesh::{Geometry, MeshData, Vertex};
use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::skygeo::{SkyBlend, SkyDraw, SkyTex, SkyVertex};
use crate::ui::{UiBatch, UiVertex, UI_H, UI_SIZE};
use crate::AtlasImage;

const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

const SHADER: &str = r#"
struct Globals {
    view_proj: mat4x4<f32>,
    // rgb = cor da neblina (gama), w = 0
    fog_color: vec4<f32>,
    // x = início, y = fim, z = densidade, w = modo (0 linear, 1 exp)
    fog: vec4<f32>,
    camera: vec4<f32>,
    // largura, altura, escala da GUI
    screen: vec4<f32>,
    // EntityRenderer.updateLightmap: índice = céu * 16 + bloco (rgb em gama)
    lightmap: array<vec4<f32>, 256>,
    // Terreno de longe (opção "Distância LOD"): xy = coluna do jogador.
    lod: vec4<f32>,
    // Colunas desenhadas pelo terreno de perto, numa grade 65 x 65 em volta de `lod.xy` (bit z * 65 + x).
    near_mask: array<vec4<u32>, 34>,
};
@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var tex: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) light: vec4<f32>,
};
struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) wpos: vec3<f32>,
    // Tinta de dano/piscar: rgb já vezes o mapa de luz e o alfa da mistura (`setBrightness`).
    @location(3) ov: vec4<f32>,
};

// GL_LINEAR sobre o lightmap 16x16 (u = bloco, v = céu), como a unidade de textura 1 do jogo.
fn light_at(sky: f32, blk: f32) -> vec3<f32> {
    let s = clamp(sky, 0.0, 15.0);
    let b = clamp(blk, 0.0, 15.0);
    let s0 = floor(s);
    let b0 = floor(b);
    let s1 = min(s0 + 1.0, 15.0);
    let b1 = min(b0 + 1.0, 15.0);
    let fs = s - s0;
    let fb = b - b0;
    let a = mix(g.lightmap[u32(s0) * 16u + u32(b0)].rgb, g.lightmap[u32(s0) * 16u + u32(b1)].rgb, fb);
    let c = mix(g.lightmap[u32(s1) * 16u + u32(b0)].rgb, g.lightmap[u32(s1) * 16u + u32(b1)].rgb, fb);
    return mix(a, c, fs);
}

fn fog_mix(rgb: vec3<f32>, dist: f32, fog: vec4<f32>, fog_rgb: vec3<f32>) -> vec3<f32> {
    if (fog.w < 0.0) {
        return rgb;
    }
    var f = 1.0;
    if (fog.w < 0.5) {
        f = clamp((fog.y - dist) / (fog.y - fog.x), 0.0, 1.0);
    } else {
        f = clamp(exp(-fog.z * dist), 0.0, 1.0);
    }
    return mix(fog_rgb, rgb, f);
}

@vertex
fn vs_main(v: VsIn) -> VsOut {
    var o: VsOut;
    o.clip = g.view_proj * vec4<f32>(v.pos, 1.0);
    o.uv = v.uv;
    // Tudo em espaço gama, como o OpenGL fixo: textura * cor do vértice * mapa de luz.
    o.color = vec4<f32>(v.color.rgb * light_at(v.light.x * 15.0, v.light.y * 15.0) * v.light.z, v.color.a);
    o.wpos = v.pos;
    // light.w: bit 7 = branco (senão vermelho); 7 bits baixos = alfa da mistura * 127.
    let w = round(v.light.w * 255.0);
    let white = w >= 128.0;
    let a = select(w, w - 128.0, white) / 127.0;
    let lm = light_at(v.light.x * 15.0, v.light.y * 15.0);
    o.ov = vec4<f32>(select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(1.0), white) * lm * a, a);
    return o;
}

fn shade(i: VsOut) -> vec4<f32> {
    return shade_texel(i, textureSample(tex, samp, i.uv));
}

fn shade_texel(i: VsOut, t: vec4<f32>) -> vec4<f32> {
    // lerp(textura x cor, cor da tinta, a) x luz: a luz já está em `color` e em `ov`.
    let c = vec4<f32>(t.rgb * i.color.rgb * (1.0 - i.ov.a) + i.ov.rgb, t.a * i.color.a);
    // GL_NV_fog_distance (radial): distância 3D até o olho.
    return vec4<f32>(fog_mix(c.rgb, length(i.wpos - g.camera.xyz), g.fog, g.fog_color.rgb), c.a);
}

@fragment
fn fs_cutout(i: VsOut) -> @location(0) vec4<f32> {
    let c = shade(i);
    if (c.a < 0.1) {
        discard;
    }
    return vec4<f32>(c.rgb, 1.0);
}

// SOLID: `EntityRenderer.renderWorldPass` desliga o teste de alfa. Sem `discard`, o teste de profundidade
// roda antes do sombreamento e corta o que fica escondido.
@fragment
fn fs_solid_block(i: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(shade(i).rgb, 1.0);
}

// CUTOUT: o mesmo recorte, mas `setBlurMipmap(false, false)`: só o nível 0 da textura.
@fragment
fn fs_cutout_block_nomip(i: VsOut) -> @location(0) vec4<f32> {
    let c = shade_texel(i, textureSampleLevel(tex, samp, i.uv, 0.0));
    if (c.a <= 0.5) {
        discard;
    }
    return vec4<f32>(c.rgb, 1.0);
}

// CUTOUT_MIPPED: `alphaFunc(GREATER, 0.5)` do `EntityRenderer.renderWorld`.
// As entidades e a mão mantêm o limiar 0,1 (`fs_cutout`).
@fragment
fn fs_cutout_block(i: VsOut) -> @location(0) vec4<f32> {
    let c = shade(i);
    if (c.a <= 0.5) {
        discard;
    }
    return vec4<f32>(c.rgb, 1.0);
}

// Terreno de longe: some onde o terreno de perto desenha a coluna (sem faixa dupla nem buraco na borda).
fn near_drawn(p: vec3<f32>) -> bool {
    let c = vec2<i32>(floor(p.xz / 16.0)) - vec2<i32>(g.lod.xy) + vec2<i32>(32, 32);
    if (c.x < 0 || c.y < 0 || c.x > 64 || c.y > 64) {
        return false;
    }
    let i = u32(c.y * 65 + c.x);
    let w = g.near_mask[i / 128u][(i / 32u) % 4u];
    return ((w >> (i % 32u)) & 1u) != 0u;
}

@fragment
fn fs_far(i: VsOut) -> @location(0) vec4<f32> {
    if (near_drawn(i.wpos)) {
        discard;
    }
    return vec4<f32>(shade(i).rgb, 1.0);
}

// Água de longe: como a de perto (`fs_translucent`), com o mesmo recorte.
@fragment
fn fs_far_water(i: VsOut) -> @location(0) vec4<f32> {
    if (near_drawn(i.wpos)) {
        discard;
    }
    return shade(i);
}

// Partículas: mistura com recorte em 1/255 (`alphaFunc(GREATER, 0.003921569)`).
@fragment
fn fs_particle(i: VsOut) -> @location(0) vec4<f32> {
    let c = shade(i);
    if (c.a < 0.003921569) {
        discard;
    }
    return c;
}

@fragment
fn fs_translucent(i: VsOut) -> @location(0) vec4<f32> {
    return shade(i);
}

// Geometria do céu/nuvens/chuva: posições relativas ao olho, uma matriz por desenho.
struct Obj {
    mvp: mat4x4<f32>,
    model: mat4x4<f32>,
    color: vec4<f32>,
    // parâmetros da neblina (w < 0 desliga)
    fog: vec4<f32>,
    // x = teste alfa (> 0.1)
    flags: vec4<f32>,
};
@group(1) @binding(0) var<uniform> obj: Obj;

struct SkyIn {
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};
struct SkyOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) rel: vec3<f32>,
};

@vertex
fn vs_skygeo(v: SkyIn) -> SkyOut {
    var o: SkyOut;
    o.clip = obj.mvp * vec4<f32>(v.pos, 1.0);
    o.uv = v.uv;
    o.color = v.color * obj.color;
    o.rel = (obj.model * vec4<f32>(v.pos, 1.0)).xyz;
    return o;
}

@fragment
fn fs_skygeo(i: SkyOut) -> @location(0) vec4<f32> {
    let c = textureSample(tex, samp, i.uv) * i.color;
    if (obj.flags.x > 0.5 && c.a < 0.1) {
        discard;
    }
    return vec4<f32>(fog_mix(c.rgb, length(i.rel), obj.fog, g.fog_color.rgb), c.a);
}

struct UiIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};
struct UiOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_ui(v: UiIn) -> UiOut {
    var o: UiOut;
    let p = v.pos * g.screen.z;
    o.clip = vec4<f32>(p.x / g.screen.x * 2.0 - 1.0, 1.0 - p.y / g.screen.y * 2.0, 0.0, 1.0);
    o.uv = v.uv;
    // A GUI mistura em espaço gama (textura e alvo sem sRGB), como o jogo original.
    o.color = v.color;
    return o;
}

// misc/vignette guardada com a luminância no alfa: a saída é luminância x cor, para o ZERO, ONE_MINUS_SRC_COLOR.
@fragment
fn fs_vignette(i: UiOut) -> @location(0) vec4<f32> {
    let l = textureSample(tex, samp, i.uv).a;
    return vec4<f32>(l * i.color.rgb, 1.0);
}

@fragment
fn fs_ui(i: UiOut) -> @location(0) vec4<f32> {
    let c = textureSample(tex, samp, i.uv) * i.color;
    if (c.a < 0.01) {
        discard;
    }
    return c;
}

@vertex
fn vs_overlay(@location(0) pos: vec2<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(pos, 0.0, 1.0);
}

@fragment
fn fs_overlay() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}
"#;

pub struct Camera {
    pub eye: Vec3,
    /// Graus, convenção do Minecraft (0 = +Z, cresce no sentido horário).
    pub yaw: f32,
    /// Graus, positivo olha para baixo.
    pub pitch: f32,
    pub fov_deg: f32,
    /// Em blocos.
    pub view_distance: f32,
    /// Matriz de visão completa (balanço, dano, terceira pessoa); `None` = olho + yaw/pitch.
    pub view: Option<Mat4>,
}

impl Camera {
    pub fn forward(&self) -> Vec3 {
        let (y, p) = (self.yaw.to_radians(), self.pitch.to_radians());
        Vec3::new(-y.sin() * p.cos(), -p.sin(), y.cos() * p.cos())
    }

    /// Direita na tela (para textos virados para a câmera).
    pub fn right(&self) -> Vec3 {
        let y = self.yaw.to_radians();
        Vec3::new(-y.cos(), 0.0, -y.sin())
    }
}

/// Céu e luz do momento (cores em gama, como o OpenGL fixo do jogo).
#[derive(Clone)]
pub struct Sky {
    /// `glClearColor` e cor da neblina (`EntityRenderer.updateFogColor`).
    pub fog_color: [f32; 3],
    /// Neblina do mundo (`setupFog(0)`): início, fim, densidade, modo (0 linear, 1 exp).
    pub fog_world: [f32; 4],
    /// Neblina do céu (`setupFog(-1)`).
    pub fog_sky: [f32; 4],
    /// `EntityRenderer.lightmapColors`: índice = céu * 16 + bloco.
    pub lightmap: [[u8; 3]; 256],
}

impl Default for Sky {
    fn default() -> Self {
        Self { fog_color: [0.0; 3], fog_world: [0.0, 1.0, 0.0, 0.0], fog_sky: [0.0, 1.0, 0.0, 0.0], lightmap: [[255; 3]; 256] }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Tex {
    Blocks,
    Ui,
    Custom(u32),
}

/// Geometria montada a cada quadro (entidades, nomes, seleção).
pub struct DrawItem {
    pub tex: Tex,
    pub geo: Geometry,
    pub translucent: bool,
}

/// Mão/item em primeira pessoa (`EntityRenderer.renderHand`): geometria já no espaço
/// da câmera, desenhada por cima do mundo com o depth limpo e projeção própria.
pub struct HandPass {
    pub proj: Mat4,
    pub models: Vec<DrawItem>,
    /// `ItemRenderer.renderOverlays` depois da mão: sem teste de profundidade (`depthFunc(519)`).
    pub overlays: Vec<DrawItem>,
}

pub struct Frame<'a> {
    pub cam: &'a Camera,
    pub hand: Option<&'a HandPass>,
    pub sky: &'a Sky,
    /// Antes do terreno: céu e, com o olho abaixo de 128, as nuvens.
    pub sky_draws: &'a [SkyDraw],
    /// Chuva e neve (depois do sólido, antes do translúcido).
    pub weather_draws: &'a [SkyDraw],
    /// Depois de tudo: nuvens quando o olho está em 128 ou mais.
    pub late_draws: &'a [SkyDraw],
    pub models: &'a [DrawItem],
    /// Entidades com mistura aditiva `blendFunc(SRC_ALPHA, ONE)` (raio, brilho do TNT): sem escrita de profundidade.
    pub additive: &'a [DrawItem],
    /// Rótulos de nome: fundo e texto fraco sem teste de profundidade (`disableDepth`), depois de tudo do mundo.
    pub overlay: &'a [DrawItem],
    /// Itens de `models` (o próprio jogador em terceira pessoa) cuja parte opaca é desenhada de novo depois de
    /// `overlay`: o corpo cobre os textos sem profundidade que estão atrás dele (hologramas e nomes).
    pub front: std::ops::Range<usize>,
    /// Partículas (`EffectRenderer`) por instância, na ordem: `translucent` = camada sem escrita de profundidade.
    pub particles: &'a crate::ParticleBatch,
    pub ui: &'a UiBatch,
    pub gui_scale: f32,
    pub crosshair: bool,
    /// Modelo 3D dentro de uma tela (jogador do inventário, livro do encantamento).
    pub gui_models: Option<&'a GuiModels>,
    /// Seções que a câmera alcança (`culling`); `None` desenha todas que passam no frustum e no alcance.
    pub visible: Option<&'a crate::SectionSet>,
    /// Opção "Distância LOD": com ela, o terreno de perto desenha as colunas da máscara e o de longe o resto.
    pub far: Option<&'a FarFrame>,
}

/// Terreno de longe no quadro.
#[derive(Clone, Copy)]
pub struct FarFrame {
    /// Coluna do jogador (centro da máscara).
    pub center: (i32, i32),
    /// Colunas que o terreno de perto desenha: bit `z * 65 + x` da grade 65 x 65 que começa em `center - 32`.
    pub mask: [u32; 136],
    /// Alcance do terreno de longe, em blocos.
    pub distance: f32,
}

impl FarFrame {
    pub fn new(center: (i32, i32), distance: f32) -> Self {
        Self { center, mask: [0; 136], distance }
    }

    pub fn set(&mut self, cx: i32, cz: i32) {
        let (x, z) = (cx - self.center.0 + 32, cz - self.center.1 + 32);
        if (0..65).contains(&x) && (0..65).contains(&z) {
            let i = (z * 65 + x) as usize;
            self.mask[i / 32] |= 1 << (i % 32);
        }
    }

    pub fn contains(&self, cx: i32, cz: i32) -> bool {
        let (x, z) = (cx - self.center.0 + 32, cz - self.center.1 + 32);
        if !(0..65).contains(&x) || !(0..65).contains(&z) {
            return false;
        }
        let i = (z * 65 + x) as usize;
        self.mask[i / 32] & (1 << (i % 32)) != 0
    }
}

/// Bloco do terreno de longe na GPU.
struct FarTile {
    /// Parte opaca e água (translúcida).
    parts: [Option<Part>; 2],
    min: Vec3,
    max: Vec3,
}

/// Geometria 3D desenhada no meio da GUI: entre os índices `0..at` e `at..` do `UiBatch`, com a própria
/// projeção (`proj`), profundidade limpa e, se houver, um `glViewport` em pixels da janela (x, y, w, h,
/// com y a partir de cima).
pub struct GuiModels {
    pub at: u32,
    pub proj: Mat4,
    pub viewport: Option<[f32; 4]>,
    pub models: Vec<DrawItem>,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    fog_color: [f32; 4],
    fog: [f32; 4],
    camera: [f32; 4],
    screen: [f32; 4],
    lightmap: [[f32; 4]; 256],
    lod: [f32; 4],
    near_mask: [[u32; 4]; 34],
}

/// Uniforme por desenho do céu (um slot de 256 bytes cada).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ObjUniform {
    mvp: [[f32; 4]; 4],
    model: [[f32; 4]; 4],
    color: [f32; 4],
    fog: [f32; 4],
    flags: [f32; 4],
    _pad: [f32; 20],
}

const OBJ_SLOTS: usize = 64;

/// Bit do id de textura personalizada que pede mistura aditiva (`mc-client::mobs::ADDITIVE`).
pub const ADDITIVE: u32 = 1 << 30;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct SkyKey {
    blend: u8,
    depth_test: bool,
    depth_write: bool,
    color_write: bool,
}

/// Uma camada de uma seção nos buffers grandes (`arena`).
#[derive(Clone, Copy)]
struct Part {
    v: crate::arena::Slice,
    i: crate::arena::Slice,
}

/// Camadas da seção: SOLID, CUTOUT_MIPPED, CUTOUT e TRANSLUCENT.
#[derive(Default)]
struct SectionGpu {
    parts: [Option<Part>; 4],
}

/// `DrawIndexedIndirectArgs`: contagem, instâncias, primeiro índice, vértice base, primeira instância.
fn draw_args(p: &Part) -> [u32; 5] {
    [p.i.len, 1, p.i.start, p.v.start, 0]
}

/// Intervalo de uma geometria do quadro dentro do lote: índices a partir de `first`, vértices somados a `base`.
#[derive(Clone, Copy)]
struct FrameGeom {
    first: u32,
    base: i32,
    count: u32,
}

/// Vértices e índices de todas as geometrias dinâmicas do quadro, enviados de uma vez.
#[derive(Default)]
struct Lote {
    verts: Vec<Vertex>,
    indices: Vec<u32>,
}

impl Lote {
    /// Acrescenta a geometria ao lote; `None` se ela estiver vazia (como antes, nada é desenhado).
    fn push(&mut self, g: &Geometry) -> Option<FrameGeom> {
        if g.is_empty() {
            return None;
        }
        let geo = FrameGeom { first: self.indices.len() as u32, base: self.verts.len() as i32, count: g.indices.len() as u32 };
        self.verts.extend_from_slice(&g.vertices);
        self.indices.extend_from_slice(&g.indices);
        Some(geo)
    }

    fn push_all(&mut self, items: &[DrawItem]) -> Vec<(Tex, bool, FrameGeom)> {
        items.iter().filter_map(|m| self.push(&m.geo).map(|g| (m.tex, m.translucent, g))).collect()
    }
}

/// Tamanho mínimo dos buffers do quadro.
const FRAME_BUF_MIN: u64 = 64 * 1024;

/// Buffer que o quadro reaproveita: só é recriado (com o dobro do tamanho) quando o conteúdo não cabe.
struct FrameBuf {
    buf: wgpu::Buffer,
    cap: u64,
    usage: wgpu::BufferUsages,
}

impl FrameBuf {
    fn new(device: &wgpu::Device, usage: wgpu::BufferUsages) -> Self {
        Self { buf: Self::alloc(device, FRAME_BUF_MIN, usage), cap: FRAME_BUF_MIN, usage }
    }

    fn alloc(device: &wgpu::Device, size: u64, usage: wgpu::BufferUsages) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor { label: Some("quadro"), size, usage: usage | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false })
    }

    /// Escreve `bytes` no começo do buffer, crescendo antes se não couber.
    fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, bytes: &[u8]) {
        let need = bytes.len() as u64;
        if need == 0 {
            return;
        }
        if need > self.cap {
            self.cap = need.max(self.cap * 2);
            self.buf = Self::alloc(device, self.cap, self.usage);
        }
        queue.write_buffer(&self.buf, 0, bytes);
    }
}

struct CustomTex {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    hand_bind_group: wgpu::BindGroup,
    gui_bind_group: wgpu::BindGroup,
    size: (u32, u32),
}

/// Tamanho (px) do atlas da GUI a partir do RGBA: `UI_SIZE` x `UI_H` vezes o fator de escala HD.
fn ui_dims(rgba: &[u8]) -> (u32, u32) {
    let k = ((rgba.len() / 4) as f64 / (UI_SIZE as f64 * UI_H as f64)).sqrt().round().max(1.0) as u32;
    (UI_SIZE * k, UI_H * k)
}

fn make_texture(device: &wgpu::Device, queue: &wgpu::Queue, label: &str, w: u32, h: u32, levels: &[Vec<u8>], tex_format: wgpu::TextureFormat) -> wgpu::Texture {
    let t = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: levels.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: tex_format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (level, data) in levels.iter().enumerate() {
        let (lw, lh) = ((w >> level).max(1), (h >> level).max(1));
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &t, mip_level: level as u32, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            data,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * lw), rows_per_image: Some(lh) },
            wgpu::Extent3d { width: lw, height: lh, depth_or_array_layers: 1 },
        );
    }
    t
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    /// `MINE_RS_FORA_DA_TELA`: alvo fora da tela na GPU de alto desempenho, sem apresentar (benchmark no
    /// display escondido com a GPU de verdade). `None` = desenha no surface da janela.
    offscreen: Option<wgpu::Texture>,
    /// Envios ainda na GPU no modo fora da tela: no máximo 2, como o `desired_maximum_frame_latency`.
    inflight: std::collections::VecDeque<wgpu::SubmissionIndex>,
    /// Teste no modo fora da tela: o próximo quadro é lido da GPU e salvo neste PNG.
    pub pedido_print: Option<std::path::PathBuf>,
    /// Tempo do último `render` parado esperando a GPU (textura do surface ou envio antigo), para o perfil.
    pub espera_gpu: std::time::Duration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    depth: wgpu::TextureView,
    cutout: wgpu::RenderPipeline,
    solid_blocks: wgpu::RenderPipeline,
    cutout_blocks: wgpu::RenderPipeline,
    cutout_nomip_blocks: wgpu::RenderPipeline,
    translucent_blocks: wgpu::RenderPipeline,
    translucent: wgpu::RenderPipeline,
    /// `blendFunc(1, 1)` das camadas de brilho (olhos, carga do creeper); id de textura com o bit `ADDITIVE`.
    additive: wgpu::RenderPipeline,
    particles: crate::particle_pass::ParticlePass,
    particle_nodepth: wgpu::RenderPipeline,
    label_nodepth: wgpu::RenderPipeline,
    /// `blendFunc(770, 1)` com `doPolygonOffset(-3, -3)` (TNT, raio): `Frame.additive`.
    additive_alpha: wgpu::RenderPipeline,
    ui_pipeline: wgpu::RenderPipeline,
    ui_vignette: wgpu::RenderPipeline,
    overlay: wgpu::RenderPipeline,
    globals: wgpu::Buffer,
    hand_globals: wgpu::Buffer,
    gui_globals: wgpu::Buffer,
    bgl: wgpu::BindGroupLayout,
    nearest: wgpu::Sampler,
    nearest_clamp: wgpu::Sampler,
    mipmapped: wgpu::Sampler,
    /// Lado de cada tile do atlas de blocos (16 no jogo padrão).
    atlas_tile: u32,
    /// Fator de escala do atlas da GUI (1 = 2048 px; 2 e 4 para pacotes HD).
    ui_scale: u32,
    /// Sobe a cada troca do atlas da GUI (recarga de recursos): o que foi gravado depois precisa voltar.
    ui_generation: u32,
    blocks_bg: wgpu::BindGroup,
    blocks_hand_bg: wgpu::BindGroup,
    blocks_gui_bg: wgpu::BindGroup,
    blocks_tex: wgpu::Texture,
    ui_tex: wgpu::Texture,
    ui_format: wgpu::TextureFormat,
    ui_bg: wgpu::BindGroup,
    custom: HashMap<u32, CustomTex>,
    next_custom: u32,
    crosshair: wgpu::Buffer,
    shader: wgpu::ShaderModule,
    sky_layout: wgpu::PipelineLayout,
    sky_pipes: HashMap<SkyKey, wgpu::RenderPipeline>,
    sky_bgs: HashMap<SkyTex, wgpu::BindGroup>,
    repeat: wgpu::Sampler,
    obj_buf: wgpu::Buffer,
    obj_bg: wgpu::BindGroup,
    /// Geometria dinâmica do quadro (entidades, partículas, mão, GUI): buffers persistentes e o lote da vez.
    mesh_vb: FrameBuf,
    mesh_ib: FrameBuf,
    mesh_lote: Lote,
    ui_vb: FrameBuf,
    ui_ib: FrameBuf,
    sky_vb: FrameBuf,
    sky_lote: Vec<SkyVertex>,
    sections: HashMap<(i32, i32, i32), SectionGpu>,
    far_tiles: HashMap<(i32, i32), FarTile>,
    far_blocks: wgpu::RenderPipeline,
    far_water_blocks: wgpu::RenderPipeline,
    /// Vértices e índices de todas as seções, e os argumentos dos desenhos indiretos do quadro.
    sec_vb: crate::arena::Pool,
    sec_ib: crate::arena::Pool,
    indirect: FrameBuf,
    /// `Features::MULTI_DRAW_INDIRECT`: uma camada inteira num comando; sem ele, um `draw_indexed` por seção.
    multi_draw: bool,
    pub adapter_name: String,
    /// O que o F3 mostra no lugar de `GL_VENDOR`/`GL_RENDERER`/`GL_VERSION`.
    pub adapter_info: wgpu::AdapterInfo,
    /// Seções com malha desenhadas no último quadro (o `C:` do F3).
    pub drawn_sections: std::cell::Cell<usize>,
    present_modes: Vec<wgpu::PresentMode>,
}

fn create_depth(device: &wgpu::Device, w: u32, h: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

fn crosshair_vertices(w: u32, h: u32) -> [[f32; 2]; 12] {
    let (sx, sy) = (2.0 / w.max(1) as f32, 2.0 / h.max(1) as f32);
    let rect = |hw: f32, hh: f32| {
        let (x, y) = (hw * sx, hh * sy);
        [[-x, -y], [x, -y], [x, y], [-x, -y], [x, y], [-x, y]]
    };
    let a = rect(9.0, 1.0);
    let b = rect(1.0, 9.0);
    [a[0], a[1], a[2], a[3], a[4], a[5], b[0], b[1], b[2], b[3], b[4], b[5]]
}

/// Planos do frustum (clip z de 0 a w).
fn frustum_planes(m: &Mat4) -> [Vec4; 5] {
    let r = [m.row(0), m.row(1), m.row(2), m.row(3)];
    [r[3] + r[0], r[3] - r[0], r[3] + r[1], r[3] - r[1], r[2]]
}

/// Caixa dentro do frustum (ou tocando as faces dos planos).
pub fn aabb_visible(planes: &[Vec4; 5], min: Vec3, max: Vec3) -> bool {
    planes.iter().all(|p| {
        let v = Vec3::new(
            if p.x >= 0.0 { max.x } else { min.x },
            if p.y >= 0.0 { max.y } else { min.y },
            if p.z >= 0.0 { max.z } else { min.z },
        );
        p.truncate().dot(v) + p.w >= 0.0
    })
}

/// Alvo do modo fora da tela: mesmo formato e tamanho do surface.
fn create_offscreen(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("fora_da_tela"),
        size: wgpu::Extent3d { width: config.width.max(1), height: config.height.max(1), depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: config.format,
        // COPY_SRC: a captura dos testes (`pedido_print`) lê a imagem de volta da GPU.
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &config.view_formats,
    })
}

impl Renderer {
    pub async fn new(window: Arc<Window>, atlas: &AtlasImage, ui_rgba: &[u8]) -> Result<Self, String> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            // DX12: Windows (fora dele a flag não acha adaptador). `WGPU_BACKEND=vulkan|dx12|gl` força um.
            backends: wgpu::Backends::from_env().unwrap_or(wgpu::Backends::VULKAN | wgpu::Backends::GL | wgpu::Backends::DX12),
            ..Default::default()
        });
        let surface = instance.create_surface(window).map_err(|e| e.to_string())?;
        let offscreen = std::env::var_os("MINE_RS_FORA_DA_TELA").is_some();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: if offscreen { None } else { Some(&surface) },
                force_fallback_adapter: false,
            })
            .await
            .ok_or("nenhuma GPU compatível")?;
        let info = adapter.get_info();
        let adapter_name = format!("{} ({:?})", info.name, info.backend);
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("mc"),
                    required_features: adapter.features() & wgpu::Features::MULTI_DRAW_INDIRECT,
                    // Buffers das seções do tamanho que a GPU aceitar (o padrão é 256 MB).
                    required_limits: wgpu::Limits { max_buffer_size: adapter.limits().max_buffer_size, ..wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()) },
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .map_err(|e| e.to_string())?;

        let caps = if offscreen {
            wgpu::SurfaceCapabilities {
                formats: vec![wgpu::TextureFormat::Rgba8UnormSrgb],
                present_modes: vec![wgpu::PresentMode::Immediate],
                alpha_modes: vec![wgpu::CompositeAlphaMode::Opaque],
                usages: wgpu::TextureUsages::RENDER_ATTACHMENT,
            }
        } else {
            surface.get_capabilities(&adapter)
        };
        let format = caps.formats.iter().copied().find(|f| f.is_srgb()).unwrap_or(caps.formats[0]);
        let ui_format = format.remove_srgb_suffix();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            // Sem vsync e sem bloquear com a janela coberta.
            present_mode: [wgpu::PresentMode::Mailbox, wgpu::PresentMode::Immediate]
                .into_iter()
                .find(|m| caps.present_modes.contains(m))
                .unwrap_or(wgpu::PresentMode::Fifo),
            alpha_mode: caps.alpha_modes[0],
            view_formats: if ui_format != format { vec![ui_format] } else { vec![] },
            desired_maximum_frame_latency: 2,
        };
        let offscreen = if offscreen {
            Some(create_offscreen(&device, &config))
        } else {
            surface.configure(&device, &config);
            None
        };
        let depth = create_depth(&device, config.width, config.height);

        let blocks_tex = make_texture(&device, &queue, "atlas", atlas.size, atlas.size, &atlas.levels, wgpu::TextureFormat::Rgba8Unorm);
        let (ui_w, ui_h) = ui_dims(ui_rgba);
        let ui_tex = make_texture(&device, &queue, "ui", ui_w, ui_h, &[ui_rgba.to_vec()], wgpu::TextureFormat::Rgba8Unorm);

        let mipmapped = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("blocos"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        // GL_REPEAT é o padrão do OpenGL para as texturas de entidade (o raio do guardião passa de 0..1).
        let nearest = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("nearest"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            ..Default::default()
        });
        // `"clamp": true` no .mcmeta (`misc/shadow`): GL_CLAMP, para a sombra não se repetir fora do bloco.
        let nearest_clamp = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("nearest_clamp"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let globals_buffer = |label: &str| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: std::mem::size_of::<Globals>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let globals = globals_buffer("globals");
        let hand_globals = globals_buffer("globals da mão");
        let gui_globals = globals_buffer("globals dos modelos da GUI");
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("world"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let bind_with = |g: &wgpu::Buffer, t: &wgpu::Texture, s: &wgpu::Sampler| {
            let view = t.create_view(&Default::default());
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: g.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(s) },
                ],
            })
        };
        let bind = |t: &wgpu::Texture, s: &wgpu::Sampler| bind_with(&globals, t, s);
        let blocks_bg = bind(&blocks_tex, &mipmapped);
        let blocks_hand_bg = bind_with(&hand_globals, &blocks_tex, &nearest);
        let blocks_gui_bg = bind_with(&gui_globals, &blocks_tex, &nearest);
        let ui_bg = bind(&ui_tex, &nearest);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("mc"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("world"), bind_group_layouts: &[&bgl], push_constant_ranges: &[] });
        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Unorm8x4, 3 => Unorm8x4],
        };
        let ui_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<UiVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Unorm8x4],
        };
        let depth_state = |write: bool, compare: wgpu::CompareFunction| wgpu::DepthStencilState {
            format: DEPTH,
            depth_write_enabled: write,
            depth_compare: compare,
            stencil: Default::default(),
            bias: Default::default(),
        };
        let pipeline_cull = |target: wgpu::TextureFormat,
                             vs: &str,
                             fs: &str,
                             buffers: &[wgpu::VertexBufferLayout],
                             blend: Option<wgpu::BlendState>,
                             depth: Option<wgpu::DepthStencilState>,
                             cull_mode: Option<wgpu::Face>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(fs),
                layout: Some(&layout),
                vertex: wgpu::VertexState { module: &shader, entry_point: Some(vs), compilation_options: Default::default(), buffers },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format: target, blend, write_mask: wgpu::ColorWrites::ALL })],
                }),
                primitive: wgpu::PrimitiveState { cull_mode, ..Default::default() },
                depth_stencil: depth,
                multisample: Default::default(),
                multiview: None,
                cache: None,
            })
        };
        let pipeline_for = |target: wgpu::TextureFormat, vs: &str, fs: &str, buffers: &[wgpu::VertexBufferLayout], blend: Option<wgpu::BlendState>, depth: Option<wgpu::DepthStencilState>| {
            pipeline_cull(target, vs, fs, buffers, blend, depth, None)
        };
        // Camadas do terreno: `GlStateManager.enableCull` no renderWorldPass (faces de costas não aparecem).
        let terrain = |fs: &str, buffers: &[wgpu::VertexBufferLayout], blend: Option<wgpu::BlendState>, depth: wgpu::DepthStencilState| {
            pipeline_cull(ui_format, "vs_main", fs, buffers, blend, Some(depth), Some(wgpu::Face::Back))
        };
        // O mundo também é desenhado em espaço gama (alvo sem sRGB), como o OpenGL fixo.
        let pipeline = |vs: &str, fs: &str, buffers: &[wgpu::VertexBufferLayout], blend: Option<wgpu::BlendState>, depth: wgpu::DepthStencilState| {
            pipeline_for(ui_format, vs, fs, buffers, blend, Some(depth))
        };
        let always = wgpu::CompareFunction::Always;
        let cutout = pipeline("vs_main", "fs_cutout", &[vertex_layout.clone()], None, depth_state(true, wgpu::CompareFunction::LessEqual));
        let solid_blocks = terrain("fs_solid_block", &[vertex_layout.clone()], None, depth_state(true, wgpu::CompareFunction::LessEqual));
        let far_blocks = terrain("fs_far", &[vertex_layout.clone()], None, depth_state(true, wgpu::CompareFunction::LessEqual));
        let far_water_blocks = terrain("fs_far_water", &[vertex_layout.clone()], Some(wgpu::BlendState::ALPHA_BLENDING), depth_state(false, wgpu::CompareFunction::LessEqual));
        let cutout_blocks = terrain("fs_cutout_block", &[vertex_layout.clone()], None, depth_state(true, wgpu::CompareFunction::LessEqual));
        let cutout_nomip_blocks = terrain("fs_cutout_block_nomip", &[vertex_layout.clone()], None, depth_state(true, wgpu::CompareFunction::LessEqual));
        let translucent_blocks = terrain("fs_translucent", &[vertex_layout.clone()], Some(wgpu::BlendState::ALPHA_BLENDING), depth_state(false, wgpu::CompareFunction::LessEqual));
        let translucent = pipeline(
            "vs_main",
            "fs_translucent",
            &[vertex_layout.clone()],
            Some(wgpu::BlendState::ALPHA_BLENDING),
            depth_state(false, wgpu::CompareFunction::LessEqual),
        );
        let additive = pipeline(
            "vs_main",
            "fs_translucent",
            &[vertex_layout.clone()],
            Some(wgpu::BlendState {
                color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add },
                alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add },
            }),
            depth_state(false, wgpu::CompareFunction::LessEqual),
        );
        let particles = crate::particle_pass::ParticlePass::new(&device, &format!("{SHADER}{}", crate::particle_pass::WGSL), &bgl, ui_format, DEPTH);
        let particle_nodepth = pipeline(
            "vs_main",
            "fs_particle",
            &[vertex_layout.clone()],
            Some(wgpu::BlendState::ALPHA_BLENDING),
            depth_state(false, wgpu::CompareFunction::LessEqual),
        );
        // `renderLivingLabel` com `disableDepth`: o fundo e o texto fraco aparecem através dos blocos.
        let label_nodepth = pipeline(
            "vs_main",
            "fs_particle",
            &[vertex_layout.clone()],
            Some(wgpu::BlendState::ALPHA_BLENDING),
            depth_state(false, wgpu::CompareFunction::Always),
        );
        // `blendFunc(770, 1)`: soma a cor vezes o alfa; o desvio de profundidade do `doPolygonOffset(-3, -3)` do TNT.
        let additive_alpha = pipeline(
            "vs_main",
            "fs_particle",
            &[vertex_layout.clone()],
            Some(wgpu::BlendState {
                color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::SrcAlpha, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add },
                alpha: wgpu::BlendComponent::OVER,
            }),
            wgpu::DepthStencilState { bias: wgpu::DepthBiasState { constant: -3, slope_scale: -3.0, clamp: 0.0 }, ..depth_state(false, wgpu::CompareFunction::LessEqual) },
        );
        let ui_pipeline = pipeline_for(ui_format, "vs_ui", "fs_ui", &[ui_layout.clone()], Some(wgpu::BlendState::ALPHA_BLENDING), None);
        // tryBlendFuncSeparate(0, 769, 1, 0): o fundo vezes 1 - cor da fonte; o alfa do alvo fica.
        let multiply = wgpu::BlendState {
            color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::Zero, dst_factor: wgpu::BlendFactor::OneMinusSrc, operation: wgpu::BlendOperation::Add },
            alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::Zero, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add },
        };
        let ui_vignette = pipeline_for(ui_format, "vs_ui", "fs_vignette", &[ui_layout], Some(multiply), None);

        // Mira que inverte a cor do fundo.
        let invert = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::OneMinusDst,
            dst_factor: wgpu::BlendFactor::OneMinusSrc,
            operation: wgpu::BlendOperation::Add,
        };
        let overlay = pipeline(
            "vs_overlay",
            "fs_overlay",
            &[wgpu::VertexBufferLayout { array_stride: 8, step_mode: wgpu::VertexStepMode::Vertex, attributes: &wgpu::vertex_attr_array![0 => Float32x2] }],
            Some(wgpu::BlendState { color: invert, alpha: wgpu::BlendComponent::OVER }),
            depth_state(false, always),
        );
        let crosshair = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mira"),
            contents: bytemuck::cast_slice(&crosshair_vertices(config.width, config.height)),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let repeat = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("repeat"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            ..Default::default()
        });
        let obj_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("obj"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: true, min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<ObjUniform>() as u64) },
                count: None,
            }],
        });
        let obj_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("obj"),
            size: (OBJ_SLOTS * std::mem::size_of::<ObjUniform>()) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let obj_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("obj"),
            layout: &obj_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &obj_buf, offset: 0, size: wgpu::BufferSize::new(std::mem::size_of::<ObjUniform>() as u64) }),
            }],
        });
        let sky_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("sky"), bind_group_layouts: &[&bgl, &obj_bgl], push_constant_ranges: &[] });
        // Textura branca 1x1 para o que não tem textura.
        let white = make_texture(&device, &queue, "branco", 1, 1, &[vec![255, 255, 255, 255]], wgpu::TextureFormat::Rgba8Unorm);
        let mut sky_bgs = HashMap::new();
        sky_bgs.insert(SkyTex::White, bind(&white, &repeat));

        let sec_vb = crate::arena::Pool::new(&device, "secoes_v", std::mem::size_of::<Vertex>() as u64, wgpu::BufferUsages::VERTEX, 1 << 20, device.limits().max_buffer_size);
        let sec_ib = crate::arena::Pool::new(&device, "secoes_i", 4, wgpu::BufferUsages::INDEX, 3 << 19, device.limits().max_buffer_size);
        let indirect = FrameBuf::new(&device, wgpu::BufferUsages::INDIRECT);
        let multi_draw = device.features().contains(wgpu::Features::MULTI_DRAW_INDIRECT);
        Ok(Self {
            shader,
            sky_layout,
            sky_pipes: HashMap::new(),
            sky_bgs,
            repeat,
            obj_buf,
            obj_bg,
            mesh_vb: FrameBuf::new(&device, wgpu::BufferUsages::VERTEX),
            mesh_ib: FrameBuf::new(&device, wgpu::BufferUsages::INDEX),
            mesh_lote: Lote::default(),
            ui_vb: FrameBuf::new(&device, wgpu::BufferUsages::VERTEX),
            ui_ib: FrameBuf::new(&device, wgpu::BufferUsages::INDEX),
            sky_vb: FrameBuf::new(&device, wgpu::BufferUsages::VERTEX),
            sky_lote: Vec::new(),
            surface,
            offscreen,
            inflight: std::collections::VecDeque::new(),
            espera_gpu: std::time::Duration::ZERO,
            pedido_print: None,
            device,
            queue,
            config,
            depth,
            cutout,
            solid_blocks,
            cutout_blocks,
            cutout_nomip_blocks,
            translucent_blocks,
            translucent,
            additive,
            particles,
            particle_nodepth,
            label_nodepth,
            additive_alpha,
            ui_pipeline,
            ui_vignette,
            overlay,
            globals,
            hand_globals,
            gui_globals,
            bgl,
            nearest,
            nearest_clamp,
            mipmapped,
            atlas_tile: atlas.tile,
            ui_scale: (ui_w / UI_SIZE).max(1),
            ui_generation: 0,
            blocks_bg,
            blocks_hand_bg,
            blocks_gui_bg,
            blocks_tex,
            ui_tex,
            ui_format,
            ui_bg,
            custom: HashMap::new(),
            next_custom: 1,
            crosshair,
            sections: HashMap::new(),
            far_tiles: HashMap::new(),
            far_blocks,
            far_water_blocks,
            sec_vb,
            sec_ib,
            indirect,
            multi_draw,
            adapter_name,
            adapter_info: info.clone(),
            drawn_sections: std::cell::Cell::new(0),
            present_modes: caps.present_modes.clone(),
        })
    }

    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    /// Liga/desliga o vsync (`Display.setVSyncEnabled`): Fifo com, Mailbox/Immediate sem.
    pub fn set_vsync(&mut self, on: bool) {
        let mode = if on {
            wgpu::PresentMode::Fifo
        } else {
            [wgpu::PresentMode::Mailbox, wgpu::PresentMode::Immediate]
                .into_iter()
                .find(|m| self.present_modes.contains(m))
                .unwrap_or(wgpu::PresentMode::Fifo)
        };
        if self.config.present_mode != mode && self.offscreen.is_none() {
            self.config.present_mode = mode;
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        self.config.width = w;
        self.config.height = h;
        match &mut self.offscreen {
            Some(t) => *t = create_offscreen(&self.device, &self.config),
            None => self.surface.configure(&self.device, &self.config),
        }
        self.depth = create_depth(&self.device, w, h);
        self.queue.write_buffer(&self.crosshair, 0, bytemuck::cast_slice(&crosshair_vertices(w, h)));
    }

    /// Regrava um tile do atlas de blocos (todos os mipmaps), como `TextureUtil.uploadTextureMipmap`
    /// das texturas animadas.
    pub fn write_atlas_tile(&mut self, tile: u16, levels: &[Vec<u8>]) {
        for (level, rgba) in levels.iter().enumerate() {
            let (x, y, size) = crate::anim::tile_rect(tile, level as u32, self.atlas_tile);
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: &self.blocks_tex, mip_level: level as u32, origin: wgpu::Origin3d { x, y, z: 0 }, aspect: wgpu::TextureAspect::All },
                rgba,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * size), rows_per_image: Some(size) },
                wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
            );
        }
    }

    /// Grava um retângulo RGBA no atlas da GUI (ícones, páginas da fonte). Coordenadas lógicas
    /// (a grade de `UI_SIZE`); com atlas HD a imagem é ampliada pelo fator de escala.
    pub fn write_ui(&mut self, x: u32, y: u32, w: u32, h: u32, rgba: &[u8]) {
        let k = self.ui_scale;
        let scaled;
        let (rgba, w, h) = if k > 1 {
            let (nw, nh) = (w * k, h * k);
            let mut out = Vec::with_capacity((nw * nh * 4) as usize);
            for py in 0..nh {
                for px in 0..nw {
                    let o = (((py / k) * w + px / k) * 4) as usize;
                    out.extend_from_slice(&rgba[o..o + 4]);
                }
            }
            scaled = out;
            (scaled.as_slice(), nw, nh)
        } else {
            (rgba, w, h)
        };
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &self.ui_tex, mip_level: 0, origin: wgpu::Origin3d { x: x * k, y: y * k, z: 0 }, aspect: wgpu::TextureAspect::All },
            rgba,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * w), rows_per_image: Some(h) },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
    }

    /// Fator do atlas da GUI (px físicos por px lógico).
    pub fn ui_scale(&self) -> u32 {
        self.ui_scale
    }

    pub fn ui_generation(&self) -> u32 {
        self.ui_generation
    }

    /// Grava RGBA no atlas da GUI em px físicos (sem ampliar): `x, y` são lógicos, `w, h` físicos.
    pub fn write_ui_physical(&mut self, x: u32, y: u32, w: u32, h: u32, rgba: &[u8]) {
        let k = self.ui_scale;
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &self.ui_tex, mip_level: 0, origin: wgpu::Origin3d { x: x * k, y: y * k, z: 0 }, aspect: wgpu::TextureAspect::All },
            rgba,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * w), rows_per_image: Some(h) },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
    }

    /// Troca os atlas de blocos e da GUI (recarga de recursos): refaz texturas e bind groups.
    pub fn replace_atlases(&mut self, atlas: &AtlasImage, ui_rgba: &[u8]) {
        let (ui_w, ui_h) = ui_dims(ui_rgba);
        self.blocks_tex = make_texture(&self.device, &self.queue, "atlas", atlas.size, atlas.size, &atlas.levels, wgpu::TextureFormat::Rgba8Unorm);
        self.ui_tex = make_texture(&self.device, &self.queue, "ui", ui_w, ui_h, &[ui_rgba.to_vec()], wgpu::TextureFormat::Rgba8Unorm);
        self.atlas_tile = atlas.tile;
        self.ui_scale = (ui_w / UI_SIZE).max(1);
        self.ui_generation += 1;
        let bind = |g: &wgpu::Buffer, t: &wgpu::Texture, s: &wgpu::Sampler| {
            let view = t.create_view(&Default::default());
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: g.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(s) },
                ],
            })
        };
        self.blocks_bg = bind(&self.globals, &self.blocks_tex, &self.mipmapped);
        self.blocks_hand_bg = bind(&self.hand_globals, &self.blocks_tex, &self.nearest);
        self.blocks_gui_bg = bind(&self.gui_globals, &self.blocks_tex, &self.nearest);
        self.ui_bg = bind(&self.globals, &self.ui_tex, &self.nearest);
    }

    /// Tamanho de uma textura própria.
    pub fn custom_size(&self, id: u32) -> Option<(u32, u32)> {
        self.custom.get(&id).map(|t| t.size)
    }

    /// Regrava uma textura própria inteira (mesmo tamanho da criação).
    pub fn update_texture(&mut self, id: u32, rgba: &[u8]) {
        let Some(t) = self.custom.get(&id) else { return };
        let (w, h) = t.size;
        if rgba.len() != (w * h * 4) as usize {
            return;
        }
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &t.texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            rgba,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * w), rows_per_image: Some(h) },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
    }

    /// Textura própria (skins, entidades). Devolve o id para usar em `Tex::Custom`.
    pub fn create_texture(&mut self, w: u32, h: u32, rgba: &[u8]) -> u32 {
        self.create_texture_clamp(w, h, rgba, false)
    }

    /// Como `create_texture`; `clamp` = `"clamp": true` do .mcmeta (a amostra não repete as bordas).
    pub fn create_texture_clamp(&mut self, w: u32, h: u32, rgba: &[u8], clamp: bool) -> u32 {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("custom"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            rgba,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * w), rows_per_image: Some(h) },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        let view = texture.create_view(&Default::default());
        let sampler = if clamp { &self.nearest_clamp } else { &self.nearest };
        let bind = |g: &wgpu::Buffer| {
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: g.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
                ],
            })
        };
        let (bind_group, hand_bind_group, gui_bind_group) = (bind(&self.globals), bind(&self.hand_globals), bind(&self.gui_globals));
        let id = self.next_custom;
        self.next_custom += 1;
        self.custom.insert(id, CustomTex { texture, bind_group, hand_bind_group, gui_bind_group, size: (w, h) });
        id
    }

    pub fn texture_size(&self, id: u32) -> Option<(u32, u32)> {
        self.custom.get(&id).map(|t| t.size)
    }

    pub fn remove_texture(&mut self, id: u32) {
        if let Some(t) = self.custom.remove(&id) {
            t.texture.destroy();
        }
    }

    /// Projeção x visão da câmera.
    fn view_proj(&self, cam: &Camera) -> Mat4 {
        self.view_proj_to(cam, cam.view_distance)
    }

    fn view_proj_to(&self, cam: &Camera, distance: f32) -> Mat4 {
        let aspect = self.config.width as f32 / self.config.height as f32;
        // farPlaneDistance * sqrt(2), como o EntityRenderer.
        let far = distance * std::f32::consts::SQRT_2;
        let proj = Mat4::perspective_rh(cam.fov_deg.to_radians(), aspect, 0.05, far);
        let view = cam.view.unwrap_or_else(|| Mat4::look_to_rh(cam.eye, cam.forward(), Vec3::Y));
        proj * view
    }

    /// Planos do frustum da câmera (os de `render`), para o `culling` do cliente.
    pub fn cam_planes(&self, cam: &Camera) -> [Vec4; 5] {
        frustum_planes(&self.view_proj(cam))
    }

    /// Desenha uma geometria do lote do quadro; o `base` leva os índices até os vértices dela.
    fn draw_mesh<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, g: &FrameGeom) {
        pass.set_vertex_buffer(0, self.mesh_vb.buf.slice(..));
        pass.set_index_buffer(self.mesh_ib.buf.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(g.first..g.first + g.count, g.base, 0..1);
    }

    pub fn upload(&mut self, key: (i32, i32, i32), mesh: MeshData) {
        self.remove(key);
        let mut s = SectionGpu::default();
        for (n, g) in [&mesh.opaque, &mesh.cutout_mipped, &mesh.cutout, &mesh.translucent].into_iter().enumerate() {
            if !g.is_empty() {
                s.parts[n] = self.put_part(g);
            }
        }
        if s.parts.iter().any(Option::is_some) {
            self.sections.insert(key, s);
        }
    }

    fn put_part(&mut self, g: &Geometry) -> Option<Part> {
        let v = self.sec_vb.put(&self.device, &self.queue, bytemuck::cast_slice(&g.vertices))?;
        match self.sec_ib.put(&self.device, &self.queue, bytemuck::cast_slice(&g.indices)) {
            Some(i) => Some(Part { v, i }),
            None => {
                self.sec_vb.release(v);
                None
            }
        }
    }

    fn free_section(&mut self, s: SectionGpu) {
        for p in s.parts.into_iter().flatten() {
            self.sec_vb.release(p.v);
            self.sec_ib.release(p.i);
        }
    }

    pub fn remove(&mut self, key: (i32, i32, i32)) {
        if let Some(s) = self.sections.remove(&key) {
            self.free_section(s);
        }
    }

    pub fn remove_column(&mut self, cx: i32, cz: i32) {
        let keys: Vec<_> = self.sections.keys().filter(|k| (k.0, k.2) == (cx, cz)).copied().collect();
        for k in keys {
            self.remove(k);
        }
    }

    pub fn clear(&mut self) {
        let keys: Vec<_> = self.sections.keys().copied().collect();
        for k in keys {
            self.remove(k);
        }
    }

    pub fn section_count(&self) -> usize {
        self.sections.len()
    }

    /// Terreno de longe do bloco `key` (substitui o anterior; geometria vazia só remove).
    /// Terreno de longe do bloco `key`: parte opaca e água (substitui o anterior; tudo vazio só remove).
    pub fn upload_far(&mut self, key: (i32, i32), opaque: &Geometry, water: &Geometry) {
        self.remove_far(key);
        if opaque.is_empty() && water.is_empty() {
            return;
        }
        let (mut min, mut max) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for v in opaque.vertices.iter().chain(&water.vertices) {
            let p = Vec3::from(v.pos);
            min = min.min(p);
            max = max.max(p);
        }
        let parts = [opaque, water].map(|g| if g.is_empty() { None } else { self.put_part(g) });
        if parts.iter().any(Option::is_some) {
            self.far_tiles.insert(key, FarTile { parts, min, max });
        }
    }

    pub fn remove_far(&mut self, key: (i32, i32)) {
        if let Some(t) = self.far_tiles.remove(&key) {
            for p in t.parts.into_iter().flatten() {
                self.sec_vb.release(p.v);
                self.sec_ib.release(p.i);
            }
        }
    }

    pub fn clear_far(&mut self) {
        let keys: Vec<_> = self.far_tiles.keys().copied().collect();
        for k in keys {
            self.remove_far(k);
        }
    }

    pub fn far_count(&self) -> usize {
        self.far_tiles.len()
    }

    /// Quadriláteros do terreno de longe na GPU (opaco + água), para o `[stats]`.
    pub fn far_quads(&self) -> usize {
        self.far_tiles.values().flat_map(|t| t.parts.iter().flatten()).map(|p| p.i.len as usize / 6).sum()
    }

    fn gui_bind_group(&self, t: Tex) -> Option<&wgpu::BindGroup> {
        match t {
            Tex::Blocks => Some(&self.blocks_gui_bg),
            Tex::Ui => None,
            Tex::Custom(id) => self.custom.get(&id).map(|c| &c.gui_bind_group),
        }
    }

    fn hand_bind_group(&self, t: Tex) -> Option<&wgpu::BindGroup> {
        match t {
            Tex::Blocks => Some(&self.blocks_hand_bg),
            Tex::Ui => None,
            Tex::Custom(id) => self.custom.get(&id).map(|c| &c.hand_bind_group),
        }
    }

    /// Textura do céu (sol, lua, nuvens, céu do End, chuva e neve), com repetição e sem filtro.
    pub fn set_sky_texture(&mut self, tex: SkyTex, w: u32, h: u32, rgba: &[u8]) {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ceu"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            rgba,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * w), rows_per_image: Some(h) },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        let view = texture.create_view(&Default::default());
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.repeat) },
            ],
        });
        self.sky_bgs.insert(tex, bg);
    }

    fn sky_pipeline(&mut self, k: SkyKey) {
        if self.sky_pipes.contains_key(&k) {
            return;
        }
        let blend = match k.blend {
            0 => None,
            1 => Some(wgpu::BlendState {
                color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::SrcAlpha, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
                alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::Zero, operation: wgpu::BlendOperation::Add },
            }),
            _ => Some(wgpu::BlendState {
                color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::SrcAlpha, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add },
                alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::Zero, operation: wgpu::BlendOperation::Add },
            }),
        };
        let layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SkyVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32x4],
        };
        let p = self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ceu"),
            layout: Some(&self.sky_layout),
            vertex: wgpu::VertexState { module: &self.shader, entry_point: Some("vs_skygeo"), compilation_options: Default::default(), buffers: &[layout] },
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: Some("fs_skygeo"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: self.ui_format,
                    blend,
                    write_mask: if k.color_write { wgpu::ColorWrites::ALL } else { wgpu::ColorWrites::empty() },
                })],
            }),
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH,
                depth_write_enabled: k.depth_write,
                depth_compare: if k.depth_test { wgpu::CompareFunction::LessEqual } else { wgpu::CompareFunction::Always },
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        self.sky_pipes.insert(k, p);
    }

    fn bind_group(&self, t: Tex) -> Option<&wgpu::BindGroup> {
        match t {
            Tex::Blocks => Some(&self.blocks_bg),
            Tex::Ui => Some(&self.ui_bg),
            Tex::Custom(id) => self.custom.get(&id).map(|c| &c.bind_group),
        }
    }

    /// Desenha um quadro. Só devolve erro quando não dá para continuar.
    pub fn render(&mut self, f: &Frame) -> Result<(), String> {
        let cam = f.cam;
        let aspect = self.config.width as f32 / self.config.height as f32;
        let vp = match f.far {
            Some(ff) => self.view_proj_to(cam, cam.view_distance.max(ff.distance + 64.0)),
            None => self.view_proj(cam),
        };
        let sky = f.sky;
        let mut lightmap = [[0f32; 4]; 256];
        for (o, c) in lightmap.iter_mut().zip(sky.lightmap.iter()) {
            *o = [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, 1.0];
        }
        let g = Globals {
            view_proj: vp.to_cols_array_2d(),
            fog_color: [sky.fog_color[0], sky.fog_color[1], sky.fog_color[2], 1.0],
            fog: sky.fog_world,
            camera: [cam.eye.x, cam.eye.y, cam.eye.z, 0.0],
            screen: [self.config.width as f32, self.config.height as f32, f.gui_scale, 0.0],
            lightmap,
            lod: f.far.map_or([0.0; 4], |ff| [ff.center.0 as f32, ff.center.1 as f32, 1.0, 0.0]),
            near_mask: f.far.map_or([[0; 4]; 34], |ff| bytemuck::cast(ff.mask)),
        };
        self.queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&g));
        if let Some(h) = f.hand {
            // Sem neblina e com a câmera na origem: a geometria já vem no espaço da câmera.
            let hg = Globals {
                view_proj: h.proj.to_cols_array_2d(),
                fog: [1.0e6, 2.0e6, 0.0, 0.0],
                camera: [0.0; 4],
                ..g
            };
            self.queue.write_buffer(&self.hand_globals, 0, bytemuck::bytes_of(&hg));
        }
        if let Some(gm) = f.gui_models {
            // Como a mão: sem neblina, a geometria já vem no espaço da tela.
            let gg = Globals { view_proj: gm.proj.to_cols_array_2d(), fog: [1.0e6, 2.0e6, 0.0, 0.0], camera: [0.0; 4], ..g };
            self.queue.write_buffer(&self.gui_globals, 0, bytemuck::bytes_of(&gg));
        }

        // Desenhos do céu: um slot de uniforme e um vértice-buffer por desenho.
        let rot_view = Mat4::look_to_rh(Vec3::ZERO, cam.forward(), Vec3::Y);
        let all: Vec<(u8, &SkyDraw)> = f
            .sky_draws
            .iter()
            .map(|d| (0u8, d))
            .chain(f.weather_draws.iter().map(|d| (1u8, d)))
            .chain(f.late_draws.iter().map(|d| (2u8, d)))
            .filter(|(_, d)| !d.verts.is_empty())
            .take(OBJ_SLOTS)
            .collect();
        let mut objs: Vec<ObjUniform> = Vec::with_capacity(all.len());
        for (_, d) in &all {
            let p = Mat4::perspective_rh(cam.fov_deg.to_radians(), aspect, 0.05, cam.view_distance * d.far_mul);
            let fog = if !d.fog {
                [0.0, 1.0, 0.0, -1.0]
            } else if d.far_mul == 2.0 {
                sky.fog_sky
            } else {
                sky.fog_world
            };
            objs.push(ObjUniform {
                mvp: (p * rot_view * d.model).to_cols_array_2d(),
                model: d.model.to_cols_array_2d(),
                color: [1.0; 4],
                fog,
                flags: [d.alpha_test as u32 as f32, 0.0, 0.0, 0.0],
                _pad: [0.0; 20],
            });
        }
        if !objs.is_empty() {
            self.queue.write_buffer(&self.obj_buf, 0, bytemuck::cast_slice(&objs));
        }
        let keys: Vec<SkyKey> = all
            .iter()
            .map(|(_, d)| SkyKey {
                blend: match d.blend {
                    SkyBlend::Opaque => 0,
                    SkyBlend::Alpha => 1,
                    SkyBlend::Additive => 2,
                },
                depth_test: d.depth_test,
                depth_write: d.depth_write,
                color_write: d.color_write,
            })
            .collect();
        for k in &keys {
            self.sky_pipeline(*k);
        }
        // Vértices do céu num lote só; cada desenho guarda o primeiro vértice dele.
        self.sky_lote.clear();
        let sky_first: Vec<u32> = all
            .iter()
            .map(|(_, d)| {
                let first = self.sky_lote.len() as u32;
                self.sky_lote.extend_from_slice(&d.verts);
                first
            })
            .collect();
        self.sky_vb.upload(&self.device, &self.queue, bytemuck::cast_slice(&self.sky_lote));

        let espera = std::time::Instant::now();
        let frame = match &self.offscreen {
            Some(_) => None,
            None => match self.surface.get_current_texture() {
                Ok(fr) => Some(fr),
                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                    self.surface.configure(&self.device, &self.config);
                    return Ok(());
                }
                Err(wgpu::SurfaceError::Timeout) => return Ok(()),
                Err(e) => return Err(e.to_string()),
            },
        };
        self.espera_gpu = espera.elapsed();
        let view = wgpu::TextureViewDescriptor { format: Some(self.ui_format), ..Default::default() };
        let target = match (&frame, &self.offscreen) {
            (Some(fr), _) => fr.texture.create_view(&view),
            (None, Some(t)) => t.create_view(&view),
            (None, None) => return Ok(()),
        };

        let planes = frustum_planes(&vp);
        let max_d2 = (cam.view_distance + 24.0).powi(2);
        let center = |k: &(i32, i32, i32)| Vec3::new(k.0 as f32 * 16.0 + 8.0, k.1 as f32 * 16.0 + 8.0, k.2 as f32 * 16.0 + 8.0);
        let mut visible: Vec<(&(i32, i32, i32), &SectionGpu)> = self
            .sections
            .iter()
            .filter(|(k, _)| {
                let c = center(k);
                let d = c - cam.eye;
                let near = match f.far {
                    Some(ff) => ff.contains(k.0, k.2),
                    None => d.x * d.x + d.z * d.z <= max_d2,
                };
                near && f.visible.map_or(true, |v| v.contains(*k)) && aabb_visible(&planes, c - Vec3::splat(8.0), c + Vec3::splat(8.0))
            })
            .collect();
        self.drawn_sections.set(visible.len());
        // Argumentos dos desenhos indiretos: SOLID, CUTOUT_MIPPED e CUTOUT de frente para trás (o teste de
        // profundidade corta o que fica atrás), TRANSLUCENT de trás para a frente.
        visible.sort_by(|a, b| center(a.0).distance_squared(cam.eye).total_cmp(&center(b.0).distance_squared(cam.eye)));
        let mut args: Vec<[u32; 5]> = Vec::with_capacity(visible.len() * 2);
        let mut layers = [(0u32, 0u32); 4];
        for (n, layer) in layers.iter_mut().enumerate() {
            let first = args.len() as u32;
            if n < 3 {
                args.extend(visible.iter().filter_map(|(_, s)| s.parts[n].as_ref()).map(draw_args));
            } else {
                args.extend(visible.iter().rev().filter_map(|(_, s)| s.parts[n].as_ref()).map(draw_args));
            }
            *layer = (first, args.len() as u32 - first);
        }
        // Terreno de longe: o opaco depois das camadas de perto, de frente para trás; a água antes da de perto, de
        // trás para a frente.
        let (mut far_layer, mut far_water) = ((0u32, 0u32), (0u32, 0u32));
        {
            if let Some(ff) = f.far {
                let reach = (ff.distance + 96.0).powi(2);
                let eye = cam.eye;
                let mut tiles: Vec<(f32, &FarTile)> = self
                    .far_tiles
                    .values()
                    .filter_map(|t| {
                        let c = (t.min + t.max) * 0.5;
                        let d = Vec3::new(c.x - eye.x, 0.0, c.z - eye.z).length_squared();
                        (d <= reach && aabb_visible(&planes, t.min, t.max)).then_some((d, t))
                    })
                    .collect();
                tiles.sort_by(|a, b| a.0.total_cmp(&b.0));
                let first = args.len() as u32;
                args.extend(tiles.iter().filter_map(|(_, t)| t.parts[0].as_ref()).map(draw_args));
                far_layer = (first, args.len() as u32 - first);
                let first = args.len() as u32;
                args.extend(tiles.iter().rev().filter_map(|(_, t)| t.parts[1].as_ref()).map(draw_args));
                far_water = (first, args.len() as u32 - first);
            }
        }
        self.indirect.upload(&self.device, &self.queue, bytemuck::cast_slice(&args));

        // Geometria do quadro (entidades, textos, mão e GUI) num lote só: um upload por buffer, antes do desenho.
        let lote = &mut self.mesh_lote;
        lote.verts.clear();
        lote.indices.clear();
        let dynamic = lote.push_all(f.models);
        let glow = lote.push_all(f.additive);
        let overlay = lote.push_all(f.overlay);
        let front = lote.push_all(f.models.get(f.front.clone()).unwrap_or_default());
        let hand = f.hand.map(|h| (lote.push_all(&h.models), lote.push_all(&h.overlays)));
        let gui = f.gui_models.map(|gm| lote.push_all(&gm.models)).unwrap_or_default();
        self.mesh_vb.upload(&self.device, &self.queue, bytemuck::cast_slice(&self.mesh_lote.verts));
        self.mesh_ib.upload(&self.device, &self.queue, bytemuck::cast_slice(&self.mesh_lote.indices));
        self.particles.upload(&self.device, &self.queue, f.particles);
        let ui_count = f.ui.indices.len() as u32;
        self.ui_vb.upload(&self.device, &self.queue, bytemuck::cast_slice(&f.ui.vertices));
        self.ui_ib.upload(&self.device, &self.queue, bytemuck::cast_slice(&f.ui.indices));

        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mundo"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: sky.fog_color[0] as f64, g: sky.fog_color[1] as f64, b: sky.fog_color[2] as f64, a: 1.0 }), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            // Uma camada do terreno inteira: os buffers grandes e um desenho indireto (ou um draw_indexed por seção).
            let terrain = |pass: &mut wgpu::RenderPass, layer: usize| {
                let (first, count) = match layer {
                    0..=3 => layers[layer],
                    4 => far_layer,
                    _ => far_water,
                };
                if count == 0 {
                    return;
                }
                pass.set_vertex_buffer(0, self.sec_vb.buf.slice(..));
                pass.set_index_buffer(self.sec_ib.buf.slice(..), wgpu::IndexFormat::Uint32);
                if self.multi_draw {
                    pass.multi_draw_indexed_indirect(&self.indirect.buf, first as u64 * 20, count);
                } else {
                    for a in &args[first as usize..(first + count) as usize] {
                        pass.draw_indexed(a[2]..a[2] + a[0], a[3] as i32, 0..1);
                    }
                }
            };

            let sky_pass = |pass: &mut wgpu::RenderPass, which: u8| {
                for (n, (w, d)) in all.iter().enumerate() {
                    if *w != which {
                        continue;
                    }
                    let Some(bg) = self.sky_bgs.get(&d.tex) else { continue };
                    pass.set_pipeline(&self.sky_pipes[&keys[n]]);
                    pass.set_bind_group(0, bg, &[]);
                    pass.set_bind_group(1, &self.obj_bg, &[(n * std::mem::size_of::<ObjUniform>()) as u32]);
                    pass.set_vertex_buffer(0, self.sky_vb.buf.slice(..));
                    let first = sky_first[n];
                    pass.draw(first..first + d.verts.len() as u32, 0..1);
                }
            };
            sky_pass(&mut pass, 0);

            pass.set_bind_group(0, &self.blocks_bg, &[]);
            pass.set_pipeline(&self.solid_blocks);
            terrain(&mut pass, 0);
            pass.set_pipeline(&self.cutout_blocks);
            terrain(&mut pass, 1);
            pass.set_pipeline(&self.cutout_nomip_blocks);
            terrain(&mut pass, 2);
            if far_layer.1 > 0 {
                pass.set_pipeline(&self.far_blocks);
                terrain(&mut pass, 4);
            }
            pass.set_pipeline(&self.cutout);
            for (tex, translucent, g) in &dynamic {
                if !translucent {
                    if let Some(bg) = self.bind_group(*tex) {
                        pass.set_bind_group(0, bg, &[]);
                        self.draw_mesh(&mut pass, g);
                    }
                }
            }

            // Partículas: depois das entidades e antes da água, camada a camada (como o EffectRenderer).
            for d in &f.particles.draws {
                if let Some(bg) = self.bind_group(d.tex) {
                    self.particles.draw(&mut pass, d, bg);
                }
            }

            sky_pass(&mut pass, 1);

            // Translúcidos de trás para a frente: a água de longe primeiro.
            if far_water.1 > 0 {
                pass.set_pipeline(&self.far_water_blocks);
                pass.set_bind_group(0, &self.blocks_bg, &[]);
                terrain(&mut pass, 5);
            }
            pass.set_pipeline(&self.translucent_blocks);
            pass.set_bind_group(0, &self.blocks_bg, &[]);
            terrain(&mut pass, 3);
            for (tex, translucent, g) in &dynamic {
                if *translucent {
                    // Camadas aditivas: o bit `ADDITIVE` (1 << 30) no id da textura personalizada.
                    let (add, tex) = match *tex {
                        Tex::Custom(id) if id & ADDITIVE != 0 => (true, Tex::Custom(id & !ADDITIVE)),
                        t => (false, t),
                    };
                    if let Some(bg) = self.bind_group(tex) {
                        pass.set_pipeline(if add { &self.additive } else { &self.translucent });
                        pass.set_bind_group(0, bg, &[]);
                        self.draw_mesh(&mut pass, g);
                    }
                }
            }

            sky_pass(&mut pass, 2);

            for (list, pipe) in [(&glow, &self.additive_alpha), (&overlay, &self.label_nodepth)] {
                pass.set_pipeline(pipe);
                for (tex, _, g) in list {
                    if let Some(bg) = self.bind_group(*tex) {
                        pass.set_bind_group(0, bg, &[]);
                        self.draw_mesh(&mut pass, g);
                    }
                }
            }
            // A mesma geometria com LessEqual passa só onde o corpo é o mais próximo: os textos sem profundidade
            // atrás dele somem e o que está na frente (já com profundidade menor) fica.
            if !overlay.is_empty() {
                pass.set_pipeline(&self.cutout);
                for (tex, translucent, g) in &front {
                    if !translucent {
                        if let Some(bg) = self.bind_group(*tex) {
                            pass.set_bind_group(0, bg, &[]);
                            self.draw_mesh(&mut pass, g);
                        }
                    }
                }
            }

            if f.crosshair && f.hand.is_none() {
                // O pipeline da mira pede o grupo 0 mesmo sem textura: nada garante que outro desenho o pôs.
                pass.set_bind_group(0, &self.blocks_bg, &[]);
                pass.set_pipeline(&self.overlay);
                pass.set_vertex_buffer(0, self.crosshair.slice(..));
                pass.draw(0..12, 0..1);
            }

        }
        if let Some((geoms, overlays)) = &hand {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mão"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                // GlStateManager.clear(256): a mão não entra nos blocos.
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            for want in [false, true] {
                pass.set_pipeline(if want { &self.translucent } else { &self.cutout });
                for (tex, translucent, g) in geoms {
                    if *translucent == want {
                        if let Some(bg) = self.hand_bind_group(*tex) {
                            pass.set_bind_group(0, bg, &[]);
                            self.draw_mesh(&mut pass, g);
                        }
                    }
                }
            }
            pass.set_pipeline(&self.particle_nodepth);
            for (tex, _, g) in overlays {
                if let Some(bg) = self.hand_bind_group(*tex) {
                    pass.set_bind_group(0, bg, &[]);
                    self.draw_mesh(&mut pass, g);
                }
            }
            // A mira é da GUI: vem depois da mão.
            if f.crosshair {
                // Sem nada da mão desenhado (pedaços ainda carregando) o grupo 0 não foi posto neste passe.
                pass.set_bind_group(0, &self.blocks_hand_bg, &[]);
                pass.set_pipeline(&self.overlay);
                pass.set_vertex_buffer(0, self.crosshair.slice(..));
                pass.draw(0..12, 0..1);
            }
        }
        if ui_count > 0 {
            let gamma_view = match (&frame, &self.offscreen) {
                (Some(fr), _) => fr.texture.create_view(&view),
                (_, Some(t)) => t.create_view(&view),
                (None, None) => return Ok(()),
            };
            let gui_pass = |enc: &mut wgpu::CommandEncoder, range: std::ops::Range<u32>| {
                if range.is_empty() {
                    return;
                }
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("gui"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &gamma_view,
                        resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                pass.set_bind_group(0, &self.ui_bg, &[]);
                pass.set_vertex_buffer(0, self.ui_vb.buf.slice(..));
                pass.set_index_buffer(self.ui_ib.buf.slice(..), wgpu::IndexFormat::Uint32);
                // A vinheta no meio do lote vai com a mistura dela.
                match f.ui.vignette.clone().filter(|v| v.start >= range.start && v.end <= range.end) {
                    Some(v) => {
                        pass.set_pipeline(&self.ui_pipeline);
                        pass.draw_indexed(range.start..v.start, 0, 0..1);
                        pass.set_pipeline(&self.ui_vignette);
                        pass.draw_indexed(v.clone(), 0, 0..1);
                        pass.set_pipeline(&self.ui_pipeline);
                        pass.draw_indexed(v.end..range.end, 0, 0..1);
                    }
                    None => {
                        pass.set_pipeline(&self.ui_pipeline);
                        pass.draw_indexed(range, 0, 0..1);
                    }
                }
            };
            match f.gui_models {
                Some(gm) => {
                    let at = gm.at.min(ui_count);
                    gui_pass(&mut enc, 0..at);
                    let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("modelos da gui"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &target,
                            resolve_target: None,
                            ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                        })],
                        // GuiAchievement/GuiScreen limpam a profundidade antes; o modelo testa contra si mesmo.
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &self.depth,
                            depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    if let Some([x, y, w, h]) = gm.viewport {
                        let (sw, sh) = (self.config.width as f32, self.config.height as f32);
                        let (x0, y0) = (x.clamp(0.0, sw), y.clamp(0.0, sh));
                        let (x1, y1) = ((x + w).clamp(0.0, sw), (y + h).clamp(0.0, sh));
                        // A escala da GUI garante 320x240 de tela, então o viewport do Java cabe na janela;
                        // o recorte só evita o erro do wgpu numa janela minúscula.
                        if x1 > x0 && y1 > y0 {
                            pass.set_viewport(x0, y0, x1 - x0, y1 - y0, 0.0, 1.0);
                        }
                    }
                    for want in [false, true] {
                        pass.set_pipeline(if want { &self.translucent } else { &self.cutout });
                        for (tex, translucent, g) in &gui {
                            if *translucent == want {
                                if let Some(bg) = self.gui_bind_group(*tex) {
                                    pass.set_bind_group(0, bg, &[]);
                                    self.draw_mesh(&mut pass, g);
                                }
                            }
                        }
                    }
                    drop(pass);
                    gui_pass(&mut enc, at..ui_count);
                }
                None => gui_pass(&mut enc, 0..ui_count),
            }
        }
        let print = match (&self.offscreen, self.pedido_print.take()) {
            (Some(tex), Some(path)) => {
                let (w, h) = (self.config.width.max(1), self.config.height.max(1));
                let row = (w * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
                let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("print"),
                    size: (row * h) as u64,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                });
                enc.copy_texture_to_buffer(
                    tex.as_image_copy(),
                    wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) } },
                    wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                );
                Some((buf, path, w, h, row))
            }
            _ => None,
        };
        let idx = self.queue.submit([enc.finish()]);
        if let Some((buf, path, w, h, row)) = print {
            buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
            let _ = self.device.poll(wgpu::Maintain::Wait);
            let data = buf.slice(..).get_mapped_range();
            let mut rgba = Vec::with_capacity((w * h * 4) as usize);
            for y in 0..h as usize {
                let start = y * row as usize;
                rgba.extend_from_slice(&data[start..start + w as usize * 4]);
            }
            drop(data);
            buf.unmap();
            // Grava ao lado e renomeia: quem espera o PNG nunca vê o arquivo pela metade.
            let tmp = path.with_extension("parcial");
            let r = image::save_buffer_with_format(&tmp, &rgba, w, h, image::ColorType::Rgba8, image::ImageFormat::Png).map_err(|e| e.to_string());
            if let Err(e) = r.and_then(|_| mc_world::replace::replace_file(&tmp, &path).map_err(|e| e.to_string())) {
                eprintln!("print {}: {e}", path.display());
            }
        }
        match frame {
            Some(fr) => fr.present(),
            None => {
                // Sem apresentar, nada segura a CPU: espera o envio de dois quadros atrás, como a fila do surface.
                self.inflight.push_back(idx);
                let espera = std::time::Instant::now();
                while self.inflight.len() > 2 {
                    if let Some(old) = self.inflight.pop_front() {
                        let _ = self.device.poll(wgpu::Maintain::WaitForSubmissionIndex(old));
                    }
                }
                self.espera_gpu += espera.elapsed();
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SHADER;

    /// Corpo de uma função WGSL (do `fn nome(` até a primeira `}`).
    fn corpo(nome: &str) -> &'static str {
        let inicio = SHADER.find(&format!("fn {nome}(")).expect("função no shader");
        let resto = &SHADER[inicio..];
        &resto[..resto.find('}').unwrap() + 1]
    }

    #[test]
    fn limiares_de_recorte_como_o_alphafunc_do_java() {
        // Blocos: GREATER 0,5 (passa se alfa > 0,5). Entidades e mão: GREATER 0,1.
        assert!(corpo("fs_cutout_block").contains("c.a <= 0.5"));
        // SOLID não testa alfa.
        assert!(!corpo("fs_solid_block").contains("discard"));
        // CUTOUT: sem mipmap, mesmo limiar.
        assert!(corpo("fs_cutout_block_nomip").contains("textureSampleLevel") && corpo("fs_cutout_block_nomip").contains("c.a <= 0.5"));
        assert!(corpo("fs_cutout").contains("c.a < 0.1"));
    }
}
