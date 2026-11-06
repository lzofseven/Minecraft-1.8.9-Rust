#!/usr/bin/env python3
"""Gera, a partir da logo do projeto (docs/logo.png, arte original), o ícone do Windows
(windows/icone.ico com 16 a 256 px e windows/icone.png) e a lateral das telas do instalador
(windows/lateral.bmp, 164x314). Nenhuma textura do jogo é usada.

Uso: python3 scripts/gen_icone.py   (precisa do Pillow)
"""
from pathlib import Path
from PIL import Image

RAIZ = Path(__file__).resolve().parent.parent
WIN = RAIZ / "windows"

logo = Image.open(RAIZ / "docs" / "logo.png").convert("RGBA")

logo.resize((256, 256), Image.LANCZOS).save(WIN / "icone.png")
logo.save(WIN / "icone.ico", sizes=[(s, s) for s in (16, 24, 32, 48, 64, 128, 256)])

# Lateral do instalador (MUI_WELCOMEFINISHPAGE_BITMAP): fundo escuro e a logo centralizada.
lateral = Image.new("RGB", (164, 314), (24, 26, 32))
m = logo.resize((140, 140), Image.LANCZOS)
lateral.paste(m, (12, 87), m)
lateral.save(WIN / "lateral.bmp")
print("windows/icone.ico, windows/icone.png, windows/lateral.bmp")
