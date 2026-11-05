#!/usr/bin/env python3
"""Gera os gráficos SVG do README (docs/*.svg) a partir das medições abaixo.

Medido com `MINE_RS_STATS=1` (média das últimas 4 linhas do [prof]) e o VmRSS de /proc/<pid>/status
depois de 20 s, no mundo de teste (`--test-world --offline`), 1280x720, Intel Iris Xe (Vulkan).
Para refazer com a sua máquina, troque os números e rode: python3 scripts/gen_docs_graficos.py
"""
from pathlib import Path

GPU = "Intel Iris Xe (integrada) · 1280x720 · release"
CENARIOS = [
    # (rótulo, FPS médio, RAM em MB)
    ("Mundo pequeno (raio 3)", 1225, 141),
    ("Mundo grande (raio 12)", 466, 169),
    ("200 entidades", 418, 143),
    ("Partículas em massa", 306, 170),
    ("Tudo junto", 162, 169),
]

OUT = Path(__file__).resolve().parent.parent / "docs"
FONT = "font-family='-apple-system,Segoe UI,Helvetica,Arial,sans-serif'"
MUTED = "#8b949e"


def barras(nome, titulo, unidade, valores, cor, maximo):
    w, linha, esq, topo = 760, 46, 230, 64
    h = topo + linha * len(valores) + 40
    largura = w - esq - 90
    p = [
        f"<svg xmlns='http://www.w3.org/2000/svg' width='{w}' height='{h}' viewBox='0 0 {w} {h}' {FONT}>",
        f"<rect width='{w}' height='{h}' rx='14' fill='#0d1117'/>",
        f"<text x='24' y='36' font-size='20' font-weight='700' fill='#e6edf3'>{titulo}</text>",
        f"<text x='{w - 24}' y='36' font-size='12' fill='{MUTED}' text-anchor='end'>{GPU}</text>",
    ]
    for i, (rotulo, v) in enumerate(valores):
        y = topo + i * linha
        bw = max(4, largura * v / maximo)
        p.append(f"<text x='{esq - 14}' y='{y + 23}' font-size='14' fill='#c9d1d9' text-anchor='end'>{rotulo}</text>")
        p.append(f"<rect x='{esq}' y='{y + 6}' width='{largura}' height='26' rx='6' fill='#161b22'/>")
        p.append(f"<rect x='{esq}' y='{y + 6}' width='{bw:.1f}' height='26' rx='6' fill='{cor}'/>")
        p.append(f"<text x='{esq + bw + 10:.1f}' y='{y + 24}' font-size='14' font-weight='700' fill='#e6edf3'>{v} {unidade}</text>")
    p.append(f"<text x='24' y='{h - 16}' font-size='12' fill='{MUTED}'>Mundo de teste do próprio cliente (--test-world). Números da sua máquina vão variar.</text>")
    p.append("</svg>")
    (OUT / nome).write_text("\n".join(p) + "\n")


def arquitetura():
    w, h = 760, 320
    caixas = [
        ("mc-protocol", "protocolo 47, NBT, slots", 24, 196),
        ("mc-net", "conexão e criptografia", 24, 100),
        ("mc-world", "chunks, blocos, luz, malhas", 272, 196),
        ("mc-render", "wgpu/Vulkan, atlas, UI", 272, 100),
        ("mc-client", "física, entidades, HUD", 520, 148),
    ]
    p = [
        f"<svg xmlns='http://www.w3.org/2000/svg' width='{w}' height='{h}' viewBox='0 0 {w} {h}' {FONT}>",
        f"<rect width='{w}' height='{h}' rx='14' fill='#0d1117'/>",
        "<defs><marker id='s' viewBox='0 0 10 10' refX='9' refY='5' markerWidth='7' markerHeight='7' orient='auto'>"
        f"<path d='M0 0L10 5L0 10z' fill='{MUTED}'/></marker></defs>",
        "<text x='24' y='36' font-size='20' font-weight='700' fill='#e6edf3'>Arquitetura</text>",
        f"<text x='24' y='58' font-size='13' fill='{MUTED}'>Cinco crates; cada seta aponta de quem usa para quem é usado.</text>",
    ]
    retas = [(520, 168, 478, 130), (520, 188, 478, 226), (375, 160, 375, 196), (272, 226, 230, 226), (127, 160, 127, 196)]
    for x1, y1, x2, y2 in retas:
        p.append(f"<line x1='{x1}' y1='{y1}' x2='{x2}' y2='{y2}' stroke='{MUTED}' stroke-width='1.5' marker-end='url(#s)'/>")
    # mc-client também usa mc-net (por cima) e mc-protocol (por baixo).
    for d in ["M623 148 C623 74 127 70 127 98", "M623 208 C623 300 127 304 127 258"]:
        p.append(f"<path d='{d}' fill='none' stroke='{MUTED}' stroke-width='1.5' marker-end='url(#s)'/>")
    for nome, desc, x, y in caixas:
        cor = "#f97316" if nome == "mc-client" else "#30363d"
        p.append(f"<rect x='{x}' y='{y}' width='206' height='60' rx='10' fill='#161b22' stroke='{cor}' stroke-width='1.5'/>")
        p.append(f"<text x='{x + 14}' y='{y + 25}' font-size='15' font-weight='700' fill='#e6edf3'>{nome}</text>")
        p.append(f"<text x='{x + 14}' y='{y + 45}' font-size='11.5' fill='{MUTED}'>{desc}</text>")
    p.append("</svg>")
    (OUT / "arquitetura.svg").write_text("\n".join(p) + "\n")


if __name__ == "__main__":
    OUT.mkdir(exist_ok=True)
    barras("fps.svg", "Quadros por segundo", "FPS", [(c[0], c[1]) for c in CENARIOS], "#f97316", 1300)
    barras("ram.svg", "Memória usada (RAM)", "MB", [(c[0], c[2]) for c in CENARIOS], "#3fb950", 400)
    arquitetura()
    print("docs/fps.svg, docs/ram.svg, docs/arquitetura.svg")
