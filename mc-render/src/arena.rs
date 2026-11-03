//! Malhas das seções num buffer grande de vértices e noutro de índices: cada camada do terreno sai num
//! desenho indireto só (`multi_draw_indexed_indirect`), em vez de trocar os buffers e desenhar seção por seção.

use std::collections::BTreeMap;

/// Pedaço de um buffer grande, em elementos (vértices ou índices).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slice {
    pub start: u32,
    pub len: u32,
}

/// Espaço livre por primeiro-que-cabe, com vizinhos livres fundidos; o fim usado (`top`) recua quando o último
/// pedaço volta. Só contas, sem GPU.
#[derive(Default, Debug)]
pub struct Alloc {
    /// início -> tamanho dos buracos livres abaixo de `top`.
    free: BTreeMap<u32, u32>,
    top: u32,
}

impl Alloc {
    pub fn alloc(&mut self, len: u32) -> Slice {
        if let Some((&start, &size)) = self.free.iter().find(|(_, &size)| size >= len) {
            self.free.remove(&start);
            if size > len {
                self.free.insert(start + len, size - len);
            }
            return Slice { start, len };
        }
        let start = self.top;
        self.top += len;
        Slice { start, len }
    }

    pub fn release(&mut self, s: Slice) {
        let (mut start, mut len) = (s.start, s.len);
        if let Some((&ps, &pl)) = self.free.range(..start).next_back() {
            if ps + pl == start {
                self.free.remove(&ps);
                start = ps;
                len += pl;
            }
        }
        if let Some(nl) = self.free.remove(&(start + len)) {
            len += nl;
        }
        if start + len == self.top {
            self.top = start;
        } else {
            self.free.insert(start, len);
        }
    }

    pub fn top(&self) -> u32 {
        self.top
    }
}

/// Buffer da GPU que cresce (cópia na própria GPU) com um `Alloc` por cima.
pub struct Pool {
    pub buf: wgpu::Buffer,
    cap: u32,
    elem: u64,
    usage: wgpu::BufferUsages,
    label: &'static str,
    max: u32,
    alloc: Alloc,
}

impl Pool {
    pub fn new(device: &wgpu::Device, label: &'static str, elem: u64, usage: wgpu::BufferUsages, cap: u32, max_bytes: u64) -> Self {
        let usage = usage | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC;
        let max = (max_bytes / elem).min(u32::MAX as u64) as u32;
        let cap = cap.min(max);
        let buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: cap as u64 * elem, usage, mapped_at_creation: false });
        Self { buf, cap, elem, usage, label, max, alloc: Alloc::default() }
    }

    /// Copia `data` para um pedaço novo; `None` se nem o buffer no tamanho máximo comporta.
    pub fn put(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[u8]) -> Option<Slice> {
        let len = (data.len() as u64 / self.elem) as u32;
        let s = self.alloc.alloc(len);
        if self.alloc.top() > self.max {
            self.alloc.release(s);
            return None;
        }
        if self.alloc.top() > self.cap {
            self.grow(device, queue, self.alloc.top());
        }
        queue.write_buffer(&self.buf, s.start as u64 * self.elem, data);
        Some(s)
    }

    pub fn release(&mut self, s: Slice) {
        self.alloc.release(s);
    }

    /// Dobra o buffer e copia o conteúdo na GPU. As escritas pendentes no buffer velho entram antes da cópia
    /// (a fila aplica os `write_buffer` no começo do próximo envio).
    fn grow(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, need: u32) {
        let cap = need.max(self.cap.saturating_mul(2)).min(self.max);
        let buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some(self.label), size: cap as u64 * self.elem, usage: self.usage, mapped_at_creation: false });
        let mut enc = device.create_command_encoder(&Default::default());
        enc.copy_buffer_to_buffer(&self.buf, 0, &buf, 0, self.cap as u64 * self.elem);
        queue.submit([enc.finish()]);
        self.buf = buf;
        self.cap = cap;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reaproveita_buracos_e_funde_vizinhos() {
        let mut a = Alloc::default();
        let x = a.alloc(10);
        let y = a.alloc(5);
        let z = a.alloc(7);
        assert_eq!((x.start, y.start, z.start, a.top()), (0, 10, 15, 22));
        a.release(x);
        // Cabe no buraco do começo.
        let w = a.alloc(4);
        assert_eq!(w.start, 0);
        a.release(y);
        // 4..10 e 10..15 viram um buraco só de 11.
        let v = a.alloc(11);
        assert_eq!(v.start, 4);
        // O último volta e o fim recua.
        a.release(z);
        assert_eq!(a.top(), 15);
        a.release(v);
        a.release(w);
        assert_eq!(a.top(), 0);
    }
}
