#!/usr/bin/env python3
"""Gera o ícone do instalador Windows (windows/icone.ico e windows/icone.png) e a lateral das telas do
instalador (windows/lateral.bmp): uma mesa de trabalho em pixel art desenhada do zero aqui (paleta e desenho
próprios, nenhuma textura do jogo)."""
from pathlib import Path
from PIL import Image

N = 16
OUT = Path(__file__).resolve().parent.parent / "windows"

WOOD = [(196, 140, 82), (181, 126, 70), (168, 114, 62), (205, 152, 94)]
DARK = (92, 56, 30)
SEAM = (128, 80, 42)
CELL = (222, 176, 116)
IRON = [(200, 206, 214), (150, 156, 166), (96, 100, 110)]
HANDLE = (110, 66, 34)


def plank(x, y, rows):
    if y % rows == rows - 1:
        return SEAM
    return WOOD[(x * 7 + (y // rows) * 3 + (x // 5)) % 3]


def top(x, y):
    if x in (0, 15) or y in (0, 15):
        return DARK
    if x in (5, 10) or y in (5, 10):
        return SEAM
    return CELL if (x + y) % 5 else WOOD[3]


def left(x, y):  # serrote
    if x in (0, 15) or y in (0, 15):
        return DARK
    if 3 <= x <= 12 and 5 <= y <= 9 and y - 5 >= (x - 3) // 3:
        return IRON[0] if y == 5 + (x - 3) // 3 else (IRON[2] if y == 9 and x % 2 else IRON[1])
    if 12 <= x <= 13 and 4 <= y <= 9:
        return HANDLE
    return plank(x, y, 4)


def right(x, y):  # martelo
    if x in (0, 15) or y in (0, 15):
        return DARK
    if 4 <= x <= 11 and 3 <= y <= 5:
        return IRON[0] if y == 3 else IRON[1] if y == 4 else IRON[2]
    if 7 <= x <= 8 and 6 <= y <= 12:
        return HANDLE if x == 7 else DARK
    return plank(x, y, 4)


def shade(c, k):
    return tuple(min(255, int(v * k)) for v in c) + (255,)


def render(size=64):
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    px = img.load()
    s = size / 32
    for py in range(size):
        for qx in range(size):
            x, y = (qx + 0.5) / s, (py + 0.5) / s
            # Topo: O=(16,1), A=(15,7.5), B=(-15,7.5)
            dx, dy = x - 16, y - 1
            u = (dx / 15 + dy / 7.5) / 2
            v = (dy / 7.5 - dx / 15) / 2
            if 0 <= u < 1 and 0 <= v < 1:
                px[qx, py] = shade(top(int(u * N), int(v * N)), 1.0)
                continue
            # Esquerda: O=(1,8.5), U=(15,7.5), V=(0,15)
            u = (x - 1) / 15
            v = (y - 8.5 - u * 7.5) / 15
            if 0 <= u < 1 and 0 <= v < 1:
                px[qx, py] = shade(left(int(u * N), int(v * N)), 0.82)
                continue
            # Direita: O=(16,16), U=(15,-7.5), V=(0,15)
            u = (x - 16) / 15
            v = (y - 16 + u * 7.5) / 15
            if 0 <= u < 1 and 0 <= v < 1:
                px[qx, py] = shade(right(int(u * N), int(v * N)), 0.64)
    return img


def main():
    OUT.mkdir(exist_ok=True)
    big = render(256)
    big.save(OUT / "icone.png")
    sizes = [16, 24, 32, 48, 64, 128, 256]
    big.save(OUT / "icone.ico", sizes=[(n, n) for n in sizes], append_images=[render(n) for n in sizes])
    # Lateral do instalador (164x314, BMP 24 bits): degradê de céu a grama e a mesa no meio.
    side = Image.new("RGB", (164, 314))
    sp = side.load()
    for y in range(314):
        for x in range(164):
            if y < 236:
                t = y / 236
                sp[x, y] = (int(96 + 60 * t), int(150 + 50 * t), int(220 - 10 * t))
            else:
                sp[x, y] = (88, 150, 64) if y < 244 else ((121, 85, 58) if (x // 8 + y // 8) % 2 else (110, 77, 52))
    side.paste(render(128), (18, 112), render(128))
    side.save(OUT / "lateral.bmp")


if __name__ == "__main__":
    main()
