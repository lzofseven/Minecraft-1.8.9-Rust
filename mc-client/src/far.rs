//! Opção "Distância LOD" (não existe no jogo original): as colunas que o cliente recebe viram resumos
//! (`FarColumn`), guardados em disco por servidor e dimensão, e o terreno simplificado feito deles é desenhado
//! além da distância de renderização. Em servidor só aparece o que já foi recebido.
//!
//! Disco: `<dados>/lod/<servidor>/<dimensão>/r.<x>.<z>.bin`, uma região de 32 x 32 colunas comprimida.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use mc_render::Renderer;
use mc_world::blocks::BlockTable;
use mc_world::lod::{mesh_far_tile, FarColumn, FAR_COLUMN_BYTES, FAR_TILE};
use mc_world::mesh::Geometry;
use mc_world::World;

type Col = (i32, i32);

const REGION: i32 = 32;
const MAGIC: &[u8; 8] = b"LZLOD\x001\x00";
/// Colunas resumidas e blocos de longe pedidos por quadro.
const SUMMARIZE_BUDGET: usize = 96;
const MESH_BUDGET: usize = 6;
const SAVE_EVERY: Duration = Duration::from_secs(30);

enum Done {
    Region { world: u32, key: Col, cols: Vec<(Col, FarColumn)> },
    Tile { world: u32, key: Col, want: (i32, u32), geo: Geometry },
}

pub struct Far {
    /// Pasta do mundo atual (servidor + dimensão); `None` sem mundo.
    dir: Option<PathBuf>,
    /// Muda a cada troca de mundo: resultados de trabalhos antigos são descartados.
    world: u32,
    cols: HashMap<Col, Arc<FarColumn>>,
    /// Colunas do mundo que mudaram e ainda não foram resumidas.
    pending: HashSet<Col>,
    /// Regiões pedidas ao disco e as que já voltaram.
    requested: HashSet<Col>,
    loaded: HashSet<Col>,
    /// Regiões com colunas novas para gravar.
    dirty: HashSet<Col>,
    /// Blocos de longe na GPU: (passo, revisão dos dados).
    tiles: HashMap<Col, (i32, u32)>,
    rev: HashMap<Col, u32>,
    in_flight: HashSet<Col>,
    tx: mpsc::Sender<Done>,
    rx: mpsc::Receiver<Done>,
    last_save: Instant,
    /// Última varredura dos blocos (coluna do jogador, alcance, distância de renderização) e se algo mudou desde ela.
    scanned: Option<(Col, i32, i32)>,
    changed: bool,
    last_scan: Instant,
    active: bool,
}

impl Default for Far {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            dir: None,
            world: 0,
            cols: HashMap::new(),
            pending: HashSet::new(),
            requested: HashSet::new(),
            loaded: HashSet::new(),
            dirty: HashSet::new(),
            tiles: HashMap::new(),
            rev: HashMap::new(),
            in_flight: HashSet::new(),
            tx,
            rx,
            last_save: Instant::now(),
            scanned: None,
            changed: false,
            last_scan: Instant::now(),
            active: false,
        }
    }
}

fn region_of(c: Col) -> Col {
    (c.0.div_euclid(REGION), c.1.div_euclid(REGION))
}

fn tile_of(c: Col) -> Col {
    (c.0.div_euclid(FAR_TILE), c.1.div_euclid(FAR_TILE))
}

/// Pasta do mundo: endereço do servidor (nome seguro) e `<dimensão>_<x>_<z>` do ponto de spawn.
pub fn world_dir(base: &Path, server: &str, dimension: i32, spawn: (i32, i32)) -> PathBuf {
    let name: String = server.trim().to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' }).collect();
    let name = if name.is_empty() { "_".to_string() } else { name };
    base.join("lod").join(name).join(format!("{dimension}_{}_{}", spawn.0, spawn.1))
}

fn region_path(dir: &Path, r: Col) -> PathBuf {
    dir.join(format!("r.{}.{}.bin", r.0, r.1))
}

fn read_region(path: &Path, r: Col) -> Vec<(Col, FarColumn)> {
    let mut out = Vec::new();
    let Ok(raw) = std::fs::read(path) else { return out };
    let mut data = Vec::new();
    if flate2::read::ZlibDecoder::new(&raw[..]).read_to_end(&mut data).is_err() || !data.starts_with(MAGIC) {
        return out;
    }
    let present = &data[MAGIC.len()..];
    if present.len() < 1024 {
        return out;
    }
    let mut at = MAGIC.len() + 1024;
    for (i, &p) in present[..1024].iter().enumerate() {
        if p == 0 {
            continue;
        }
        let Some(c) = data.get(at..at + FAR_COLUMN_BYTES).and_then(FarColumn::from_bytes) else { break };
        at += FAR_COLUMN_BYTES;
        out.push(((r.0 * REGION + i as i32 % REGION, r.1 * REGION + i as i32 / REGION), c));
    }
    out
}

fn write_region(path: &Path, r: Col, cols: &[(Col, Arc<FarColumn>)]) -> std::io::Result<()> {
    let mut slots: Vec<Option<&FarColumn>> = vec![None; 1024];
    for (c, col) in cols {
        let (x, z) = (c.0 - r.0 * REGION, c.1 - r.1 * REGION);
        slots[(z * REGION + x) as usize] = Some(col);
    }
    let mut data = MAGIC.to_vec();
    data.extend(slots.iter().map(|s| s.is_some() as u8));
    for c in slots.iter().flatten() {
        c.to_bytes(&mut data);
    }
    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    enc.write_all(&data)?;
    let bytes = enc.finish()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(tmp, path)
}

impl Far {
    /// Troca de mundo (entrar, mudar de dimensão, sair): grava o que falta e esquece o resto.
    pub fn set_world(&mut self, dir: Option<PathBuf>, renderer: Option<&mut Renderer>) {
        if self.dir == dir && dir.is_some() {
            return;
        }
        self.save(false);
        let (tx, rx) = (self.tx.clone(), std::mem::replace(&mut self.rx, mpsc::channel().1));
        *self = Self { dir, world: self.world.wrapping_add(1), tx, rx, ..Default::default() };
        if let Some(r) = renderer {
            r.clear_far();
        }
    }

    /// A coluna do mundo mudou (chegou, recebeu bloco): o resumo é refeito no próximo `update`.
    pub fn column_changed(&mut self, cx: i32, cz: i32) {
        if self.active {
            self.pending.insert((cx, cz));
        }
    }

    pub fn active(&self) -> bool {
        self.active
    }

    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    fn touch(&mut self, c: Col) {
        let mut seen = [(i32::MAX, i32::MAX); 9];
        let mut n = 0;
        // As paredes de um bloco usam as colunas vizinhas: os blocos em volta também mudam.
        for dz in -1..=1 {
            for dx in -1..=1 {
                let t = tile_of((c.0 + dx, c.1 + dz));
                if !seen[..n].contains(&t) {
                    seen[n] = t;
                    n += 1;
                    *self.rev.entry(t).or_insert(0) += 1;
                }
            }
        }
        self.changed = true;
    }

    /// Grava as regiões sujas já lidas do disco (as outras são pedidas e gravadas depois, sem perder o antigo).
    pub fn save(&mut self, wait: bool) {
        self.last_save = Instant::now();
        let Some(dir) = self.dir.clone() else { return };
        let mut jobs = Vec::new();
        let dirty: Vec<Col> = self.dirty.iter().copied().collect();
        for r in dirty {
            if !self.loaded.contains(&r) && region_path(&dir, r).exists() {
                self.request(r);
                continue;
            }
            self.dirty.remove(&r);
            let cols: Vec<(Col, Arc<FarColumn>)> = (0..REGION * REGION)
                .filter_map(|i| {
                    let c = (r.0 * REGION + i % REGION, r.1 * REGION + i / REGION);
                    self.cols.get(&c).map(|col| (c, col.clone()))
                })
                .collect();
            jobs.push((region_path(&dir, r), r, cols));
        }
        if jobs.is_empty() {
            return;
        }
        let run = move || {
            for (path, r, cols) in jobs {
                if let Err(e) = write_region(&path, r, &cols) {
                    eprintln!("LOD: não gravei {}: {e}", path.display());
                }
            }
        };
        if wait {
            run();
        } else {
            std::thread::spawn(run);
        }
    }

    fn request(&mut self, r: Col) {
        let Some(dir) = &self.dir else { return };
        if !self.requested.insert(r) {
            return;
        }
        let (path, tx, world) = (region_path(dir, r), self.tx.clone(), self.world);
        rayon::spawn(move || {
            let cols = read_region(&path, r);
            let _ = tx.send(Done::Region { world, key: r, cols });
        });
    }

    /// Um passo por quadro: resume colunas novas, lê regiões, monta e envia blocos de longe.
    /// `distance` e `render` em chunks; `distance` 0 desliga (e apaga os blocos da GPU).
    pub fn update(&mut self, world: &World, table: &Arc<BlockTable>, pc: Col, distance: i32, render: i32, mut renderer: Option<&mut Renderer>) {
        if distance <= 0 || self.dir.is_none() {
            if self.active {
                self.save(false);
                self.active = false;
                self.tiles.clear();
                self.scanned = None;
                if let Some(r) = renderer.as_deref_mut() {
                    r.clear_far();
                }
            }
            return;
        }
        if !self.active {
            // Ligou agora: tudo que já está carregado entra.
            self.active = true;
            self.pending.extend(world.columns.keys().copied());
        }
        // 1. Resumos das colunas do mundo.
        let take: Vec<Col> = self.pending.iter().copied().take(SUMMARIZE_BUDGET).collect();
        for c in take {
            self.pending.remove(&c);
            let Some(col) = world.columns.get(&c) else { continue };
            let s = FarColumn::summarize(col, table);
            if self.cols.get(&c).is_some_and(|old| **old == s) {
                continue;
            }
            self.cols.insert(c, Arc::new(s));
            self.dirty.insert(region_of(c));
            self.touch(c);
        }
        // 2. Regiões do disco dentro do alcance.
        let rr = distance / REGION + 1;
        let pr = region_of(pc);
        for dz in -rr..=rr {
            for dx in -rr..=rr {
                self.request((pr.0 + dx, pr.1 + dz));
            }
        }
        // 3. Resultados.
        while let Ok(d) = self.rx.try_recv() {
            match d {
                Done::Region { world: w, key, cols } if w == self.world => {
                    self.loaded.insert(key);
                    for (c, col) in cols {
                        // O que veio do servidor nesta sessão vale mais que o disco.
                        if !self.cols.contains_key(&c) {
                            self.cols.insert(c, Arc::new(col));
                            self.touch(c);
                        }
                    }
                }
                Done::Tile { world: w, key, want, geo } if w == self.world => {
                    self.in_flight.remove(&key);
                    if let Some(r) = renderer.as_deref_mut() {
                        r.upload_far(key, &geo);
                    }
                    self.tiles.insert(key, want);
                    self.changed = true;
                }
                _ => {}
            }
        }
        // 4. Blocos que faltam ou mudaram (varredura quando o jogador muda de coluna ou a cada 1/4 s com mudanças).
        let scan_key = (pc, distance, render);
        if self.scanned == Some(scan_key) && !(self.changed && self.last_scan.elapsed() >= Duration::from_millis(250)) {
            self.maybe_save();
            return;
        }
        self.scanned = Some(scan_key);
        self.changed = false;
        self.last_scan = Instant::now();
        let pt = tile_of(pc);
        let tr = distance / FAR_TILE + 2;
        let mut wanted: HashSet<Col> = HashSet::new();
        let mut jobs: Vec<(i64, Col, (i32, u32))> = Vec::new();
        for tz in pt.1 - tr..=pt.1 + tr {
            for tx in pt.0 - tr..=pt.0 + tr {
                let key = (tx, tz);
                let (c0x, c0z) = (tx * FAR_TILE, tz * FAR_TILE);
                // Distância (em chunks) da coluna mais perto do bloco até o jogador.
                let nx = pc.0.clamp(c0x, c0x + FAR_TILE - 1) - pc.0;
                let nz = pc.1.clamp(c0z, c0z + FAR_TILE - 1) - pc.1;
                if nx * nx + nz * nz > distance * distance {
                    continue;
                }
                // Bloco todo dentro do terreno de perto (carregado e no círculo), e também as colunas em volta (as
                // paredes da borda saem dos blocos de dentro): nada a desenhar.
                let mut all_near = true;
                let mut any = false;
                for z in c0z - 1..=c0z + FAR_TILE {
                    for x in c0x - 1..=c0x + FAR_TILE {
                        let inside = (c0x..c0x + FAR_TILE).contains(&x) && (c0z..c0z + FAR_TILE).contains(&z);
                        any |= inside && self.cols.contains_key(&(x, z));
                        let (dx, dz) = (x - pc.0, z - pc.1);
                        all_near &= dx * dx + dz * dz <= render * render && world.columns.contains_key(&(x, z));
                    }
                }
                if !any || all_near {
                    continue;
                }
                wanted.insert(key);
                let fx = (pc.0 - (c0x + FAR_TILE / 2)) as i64;
                let fz = (pc.1 - (c0z + FAR_TILE / 2)) as i64;
                let d2 = fx * fx + fz * fz;
                let step = if d2 <= 32 * 32 { 4 } else { 8 };
                let want = (step, self.rev.get(&key).copied().unwrap_or(0));
                if self.tiles.get(&key) != Some(&want) && !self.in_flight.contains(&key) {
                    jobs.push((d2, key, want));
                }
            }
        }
        let gone: Vec<Col> = self.tiles.keys().filter(|k| !wanted.contains(k)).copied().collect();
        for k in gone {
            self.tiles.remove(&k);
            if let Some(r) = renderer.as_deref_mut() {
                r.remove_far(k);
            }
        }
        jobs.sort_by_key(|j| j.0);
        if jobs.len() > MESH_BUDGET {
            // Sobrou trabalho: a próxima varredura não espera o jogador andar.
            self.changed = true;
            self.last_scan = Instant::now() - Duration::from_millis(250);
        }
        for (_, key, want) in jobs.into_iter().take(MESH_BUDGET) {
            let (c0x, c0z) = (key.0 * FAR_TILE, key.1 * FAR_TILE);
            let mut part: HashMap<Col, Arc<FarColumn>> = HashMap::new();
            for z in c0z - 1..=c0z + FAR_TILE {
                for x in c0x - 1..=c0x + FAR_TILE {
                    if let Some(c) = self.cols.get(&(x, z)) {
                        part.insert((x, z), c.clone());
                    }
                }
            }
            self.in_flight.insert(key);
            let (tx, table, w) = (self.tx.clone(), table.clone(), self.world);
            rayon::spawn(move || {
                let get = |x: i32, z: i32| part.get(&(x, z)).map(|c| &**c);
                let geo = mesh_far_tile(c0x, c0z, want.0, &table, &get);
                let _ = tx.send(Done::Tile { world: w, key, want, geo });
            });
        }
        self.maybe_save();
    }

    fn maybe_save(&mut self) {
        if !self.dirty.is_empty() && self.last_save.elapsed() >= SAVE_EVERY {
            self.save(false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regiao_ida_e_volta_no_disco() {
        let dir = std::env::temp_dir().join(format!("mine-rs-lod-teste-{}", std::process::id()));
        let mut a = FarColumn::default();
        a.height[3] = 77;
        a.state[3] = 1 << 4;
        let mut b = FarColumn::default();
        b.height[255] = 12;
        let r = (-1, 2);
        let cols = vec![((-32, 64), Arc::new(a.clone())), ((-1, 95), Arc::new(b.clone()))];
        let path = region_path(&dir, r);
        write_region(&path, r, &cols).unwrap();
        let mut back = read_region(&path, r);
        back.sort_by_key(|(c, _)| *c);
        assert_eq!(back, vec![((-32, 64), a), ((-1, 95), b)]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pasta_por_servidor_e_dimensao() {
        let p = world_dir(Path::new("/x"), "Jogar.AuratoonsMC.com:25565", -1, (12, -340));
        assert_eq!(p, PathBuf::from("/x/lod/jogar.auratoonsmc.com_25565/-1_12_-340"));
    }
}
