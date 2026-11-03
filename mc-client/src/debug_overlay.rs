//! Tela F3 (porte de `GuiOverlayDebug`, `Minecraft.debug` e `FrameTimer`).
//!
//! Os textos são os literais do Java (a tela não é traduzida). Onde o Java mostra a JVM e o OpenGL,
//! aqui vão os equivalentes do processo e do wgpu: memória do `/proc`, adaptador e driver.

use mc_render::ui::UiBatch;

use crate::gen_debug::{NAMES, PROPS};

/// Tudo que a tela lê do jogo num quadro.
#[derive(Clone, Debug, Default)]
pub struct DebugInfo {
    pub fps: u32,
    /// `RenderChunk.renderChunksUpdated` do último segundo.
    pub chunk_updates: u32,
    pub max_fps: i32,
    pub vsync: bool,
    pub fancy: bool,
    pub clouds: i32,
    pub vbo: bool,
    /// Seções desenhadas com malha e o total do `ViewFrustum` ((2d+1)² x 16).
    pub sections_drawn: usize,
    pub render_distance: i32,
    /// Seções esperando malha (`queueChunkUpdates`) e trabalhadores livres (`queueFreeRenderBuilders`).
    pub pending_compile: usize,
    pub free_builders: usize,
    pub entities_rendered: usize,
    pub entities_total: usize,
    pub particles: usize,
    pub columns: usize,
    pub pos: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
    /// `isBlockLoaded` nos pés; então bioma, luz (céu, bloco), dificuldade e tempo do mundo.
    pub loaded: bool,
    pub biome: String,
    pub sky: u8,
    pub block: u8,
    pub has_sky: bool,
    pub difficulty: u8,
    pub world_time: i64,
    /// Bloco mirado, o estado (`id << 4 | meta`) e a variante do `getActualState`.
    pub looking_at: Option<((i32, i32, i32), u16, String)>,
    pub reduced: bool,
    pub display: (u32, u32),
    /// Como `GL_VENDOR`, `GL_RENDERER` e `GL_VERSION`.
    pub gl: [String; 3],
}

/// `MathHelper.wrapAngleTo180_float`.
fn wrap180(a: f32) -> f32 {
    let mut a = a % 360.0;
    if a >= 180.0 {
        a -= 360.0;
    }
    if a < -180.0 {
        a += 360.0;
    }
    a
}

/// `DifficultyInstance.calculateAdditionalDifficulty` (o cliente não recebe o `inhabitedTime`: 0).
pub fn additional_difficulty(difficulty: u8, world_time: i64, inhabited: i64, moon: f32) -> f32 {
    if difficulty == 0 {
        return 0.0;
    }
    let hard = difficulty == 3;
    let mut f = 0.75;
    let g = ((world_time as f32 - 72000.0) / 1_440_000.0).clamp(0.0, 1.0) * 0.25;
    f += g;
    let mut h = 0.0;
    h += (inhabited as f32 / 3_600_000.0).clamp(0.0, 1.0) * if hard { 1.0 } else { 0.75 };
    h += (moon * 0.25).clamp(0.0, g);
    if difficulty == 1 {
        h *= 0.5;
    }
    f += h;
    difficulty as f32 * f
}

/// `WorldProvider.moonPhaseFactors[getMoonPhase(worldTime)]`.
pub fn moon_factor(world_time: i64) -> f32 {
    const F: [f32; 8] = [1.0, 0.75, 0.5, 0.25, 0.0, 0.25, 0.5, 0.75];
    F[((world_time / 24000 % 8 + 8) % 8) as usize]
}

/// `Minecraft.debug`.
fn debug_line(d: &DebugInfo) -> String {
    let limit = if d.max_fps == 260 { "inf".to_string() } else { d.max_fps.to_string() };
    format!(
        "{} fps ({} chunk update{}) T: {}{}{}{}{}",
        d.fps,
        d.chunk_updates,
        if d.chunk_updates != 1 { "s" } else { "" },
        limit,
        if d.vsync { " vsync" } else { "" },
        if d.fancy { "" } else { " fast" },
        match d.clouds {
            0 => "",
            1 => " fast-clouds",
            _ => " fancy-clouds",
        },
        if d.vbo { " vbo" } else { "" },
    )
}

/// `GuiOverlayDebug.call`.
pub fn left(d: &DebugInfo) -> Vec<String> {
    let (bx, by, bz) = (d.pos[0].floor() as i32, d.pos[1].floor() as i32, d.pos[2].floor() as i32);
    let side = 2 * d.render_distance + 1;
    let mut l = vec![
        "Minecraft 1.8.9 (1.8.9/vanilla)".to_string(),
        debug_line(d),
        // RenderGlobal.getDebugInfoRenders com o ChunkRenderDispatcher.getDebugInfo (sem fila de envio: pU 0).
        format!("C: {}/{} (s) D: {}, pC: {:03}, pU: {}, aB: {}", d.sections_drawn, side * side * 16, d.render_distance, d.pending_compile, 0, d.free_builders),
        format!("E: {}/{}, B: 0, I: {}", d.entities_rendered, d.entities_total, d.entities_total - d.entities_rendered.min(d.entities_total)),
        format!("P: {}. T: All: {}", d.particles, d.entities_total),
        format!("MultiplayerChunkCache: {}, {}", d.columns, d.columns),
        String::new(),
    ];
    if d.reduced {
        l.push(format!("Chunk-relative: {} {} {}", bx & 15, by & 15, bz & 15));
        return l;
    }
    // Entity.getHorizontalFacing: floor(yaw * 4 / 360 + 0,5) & 3 em EnumFacing.getHorizontal.
    let (facing, towards) = match ((d.yaw * 4.0 / 360.0) as f64 + 0.5).floor() as i32 & 3 {
        0 => ("south", "Towards positive Z"),
        1 => ("west", "Towards negative X"),
        2 => ("north", "Towards negative Z"),
        _ => ("east", "Towards positive X"),
    };
    l.push(format!("XYZ: {:.3} / {:.5} / {:.3}", d.pos[0], d.pos[1], d.pos[2]));
    l.push(format!("Block: {bx} {by} {bz}"));
    l.push(format!("Chunk: {} {} {} in {} {} {}", bx & 15, by & 15, bz & 15, bx >> 4, by >> 4, bz >> 4));
    l.push(format!("Facing: {facing} ({towards}) ({:.1} / {:.1})", wrap180(d.yaw), wrap180(d.pitch)));
    if d.loaded {
        l.push(format!("Biome: {}", d.biome));
        let sky = if d.has_sky { d.sky } else { 0 };
        l.push(format!("Light: {} ({} sky, {} block)", sky.max(d.block), d.sky, d.block));
        let time = d.world_time;
        l.push(format!("Local Difficulty: {:.2} (Day {})", additional_difficulty(d.difficulty, time, 0, moon_factor(time)), time / 24000));
    }
    if let Some(((x, y, z), _, _)) = &d.looking_at {
        l.push(format!("Looking at: {x} {y} {z}"));
    }
    l
}

/// Memória do processo e total da máquina em bytes (`VmRSS`, `VmHWM`, `MemTotal`).
pub fn memory() -> (u64, u64, u64) {
    let field = |text: &str, key: &str| -> u64 {
        text.lines().find(|l| l.starts_with(key)).and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0) * 1024
    };
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let info = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    (field(&status, "VmRSS:"), field(&status, "VmHWM:"), field(&info, "MemTotal:").max(1))
}

/// `OpenGlHelper.getCpu`: "Nx nome" com os espaços juntados.
fn cpu() -> String {
    let n = std::thread::available_parallelism().map_or(1, |n| n.get());
    let name = std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|t| t.lines().find(|l| l.starts_with("model name")).and_then(|l| l.split_once(':')).map(|(_, v)| v.trim().to_string()))
        .unwrap_or_else(|| "<unknown>".to_string());
    format!("{n}x {name}").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Propriedades do estado mirado: as de `getStateFromMeta` com os valores do `getActualState`
/// que a variante do modelo carrega; booleanos em verde ou vermelho.
pub fn block_lines(state: u16, variant: &str) -> Vec<String> {
    let actual: Vec<(&str, &str)> = variant.split(',').filter_map(|kv| kv.split_once('=')).collect();
    let mut out = vec![NAMES[(state >> 4) as usize & 255].to_string()];
    for &(k, v, boolean) in PROPS[state as usize & 4095] {
        let v = actual.iter().find(|(a, _)| *a == k).map_or(v, |(_, b)| *b);
        let shown = match (boolean, v) {
            (true, "true") => format!("§a{v}"),
            (true, "false") => format!("§c{v}"),
            _ => v.to_string(),
        };
        out.push(format!("{k}: {shown}"));
    }
    out
}

/// `getDebugInfoRight`.
pub fn right(d: &DebugInfo) -> Vec<String> {
    let mb = |b: u64| b / 1024 / 1024;
    let (used, peak, total) = memory();
    let mut l = vec![
        format!("Rust: mine-rs {} {}bit", env!("CARGO_PKG_VERSION"), usize::BITS),
        // `% 2d` do Java: espaço na frente do número positivo.
        format!("Mem:  {}% {:03}/{:03}MB", used * 100 / total, mb(used), mb(total)),
        format!("Allocated:  {}% {:03}MB", peak * 100 / total, mb(peak)),
        String::new(),
        format!("CPU: {}", cpu()),
        String::new(),
        format!("Display: {}x{} ({})", d.display.0, d.display.1, d.gl[0]),
        d.gl[1].clone(),
        d.gl[2].clone(),
    ];
    if d.reduced {
        return l;
    }
    if let Some((_, state, variant)) = &d.looking_at {
        l.push(String::new());
        l.extend(block_lines(*state, variant));
    }
    l
}

/// `renderDebugInfoLeft` e `renderDebugInfoRight`.
pub fn draw(ui: &mut UiBatch, w: i32, d: &DebugInfo) {
    for (i, s) in left(d).iter().enumerate() {
        if s.is_empty() {
            continue;
        }
        let k = ui.text_width(s) as i32;
        let y = 2 + 9 * i as i32;
        ui.rect(1.0, (y - 1) as f32, (k + 2) as f32, 9.0, 0x90505050);
        ui.text(2.0, y as f32, s, 0xE0E0E0, false);
    }
    for (i, s) in right(d).iter().enumerate() {
        if s.is_empty() {
            continue;
        }
        let k = ui.text_width(s) as i32;
        let x = w - 2 - k;
        let y = 2 + 9 * i as i32;
        ui.rect((x - 1) as f32, (y - 1) as f32, (k + 2) as f32, 9.0, 0x90505050);
        ui.text(x as f32, y as f32, s, 0xE0E0E0, false);
    }
}

/// `FrameTimer`: duração dos últimos 240 quadros em nanossegundos.
pub struct FrameTimer {
    frames: [u64; 240],
    last_index: usize,
    counts: usize,
    index: usize,
}

impl Default for FrameTimer {
    fn default() -> Self {
        Self { frames: [0; 240], last_index: 0, counts: 0, index: 0 }
    }
}

impl FrameTimer {
    pub fn add(&mut self, nanos: u64) {
        self.frames[self.index] = nanos;
        self.index += 1;
        if self.index == 240 {
            self.index = 0;
        }
        if self.counts < 240 {
            self.last_index = 0;
            self.counts += 1;
        } else {
            self.last_index = (self.index + 1) % 240;
        }
    }
}

/// `GuiOverlayDebug.blendColors`.
fn blend(a: u32, b: u32, t: f32) -> u32 {
    let ch = |s: u32| -> i32 {
        let (x, y) = ((a >> s & 255) as f32, (b >> s & 255) as f32);
        ((x + (y - x) * t) as i32).clamp(0, 255)
    };
    (ch(24) as u32) << 24 | (ch(16) as u32) << 16 | (ch(8) as u32) << 8 | ch(0) as u32
}

/// `getFrameColor(v, 0, 30, 60)`.
fn frame_color(v: i32) -> u32 {
    if v < 30 {
        blend(0xFF00FF00, 0xFFFFFF00, v as f32 / 30.0)
    } else {
        blend(0xFFFFFF00, 0xFFFF0000, (v - 30) as f32 / 30.0)
    }
}

/// `renderLagometer`.
pub fn draw_lagometer(ui: &mut UiBatch, h: i32, t: &FrameTimer, limit_framerate: i32) {
    let rect = |ui: &mut UiBatch, x0: i32, y0: i32, x1: i32, y1: i32, c: u32| ui.rect(x0 as f32, y0 as f32, (x1 - x0) as f32, (y1 - y0) as f32, c);
    // Gui.drawVerticalLine / drawHorizontalLine.
    let vline = |ui: &mut UiBatch, x: i32, a: i32, b: i32, c: u32| {
        let (a, b) = if b < a { (b, a) } else { (a, b) };
        rect(ui, x, a + 1, x + 1, b, c);
    };
    let hline = |ui: &mut UiBatch, a: i32, b: i32, y: i32, c: u32| {
        let (a, b) = if b < a { (b, a) } else { (a, b) };
        rect(ui, a, y, b + 1, y + 1, c);
    };
    rect(ui, 0, h - 60, 240, h, 0x90505050);
    let (mut k, mut x) = (t.last_index, 0);
    while k != t.index {
        // FrameTimer.getLagometerValue(ns, 30).
        let v = (t.frames[k] as f64 / 1.666_666_6e7 * 30.0) as i32;
        vline(ui, x, h, h - v, frame_color(v.clamp(0, 60)));
        x += 1;
        k = (k + 1) % 240;
    }
    rect(ui, 1, h - 30 + 1, 14, h - 30 + 10, 0x90505050);
    ui.text(2.0, (h - 30 + 2) as f32, "60", 0xE0E0E0, false);
    hline(ui, 0, 239, h - 30, 0xFFFFFFFF);
    rect(ui, 1, h - 60 + 1, 14, h - 60 + 10, 0x90505050);
    ui.text(2.0, (h - 60 + 2) as f32, "30", 0xE0E0E0, false);
    hline(ui, 0, 239, h - 60, 0xFFFFFFFF);
    hline(ui, 0, 239, h - 1, 0xFFFFFFFF);
    vline(ui, 0, h - 60, h, 0xFFFFFFFF);
    vline(ui, 239, h - 60, h, 0xFFFFFFFF);
    if limit_framerate <= 120 {
        hline(ui, 0, 239, h - 60 + limit_framerate / 2, 0xFF00FFFF);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dificuldade_local_como_o_java() {
        // Normal, dia 0, lua cheia: 2 * (0,75 + 0 + min(0,25, 0)) = 1,5.
        assert_eq!(additional_difficulty(2, 0, 0, 1.0), 1.5);
        // Difícil depois de 63 dias: 3 * (1,0 + 0,25) = 3,75.
        assert_eq!(additional_difficulty(3, 72000 + 1_440_000, 0, 1.0), 3.75);
        assert_eq!(additional_difficulty(0, 999_999, 0, 1.0), 0.0);
        assert_eq!(moon_factor(24000 * 4), 0.0);
    }

    #[test]
    fn propriedades_do_bloco_com_estado_atual() {
        // Escada de carvalho (53) meta 0: virada para leste; a variante traz a forma calculada.
        let l = block_lines(53 << 4, "facing=east,half=bottom,shape=outer_right");
        assert_eq!(l, ["minecraft:oak_stairs", "facing: east", "half: bottom", "shape: outer_right"]);
        // Cerca (85): conexões booleanas em verde e vermelho.
        let l = block_lines(85 << 4, "east=true,north=false,south=false,west=false");
        assert!(l.contains(&"east: §atrue".to_string()) && l.contains(&"north: §cfalse".to_string()), "{l:?}");
    }

    #[test]
    fn linha_de_quadros() {
        let d = DebugInfo { fps: 60, chunk_updates: 1, max_fps: 260, fancy: true, clouds: 2, vbo: true, ..Default::default() };
        assert_eq!(debug_line(&d), "60 fps (1 chunk update) T: inf fancy-clouds vbo");
    }
}
