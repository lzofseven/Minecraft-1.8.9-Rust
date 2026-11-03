#!/usr/bin/env python3
"""Gera mc-client/src/gen_paintings.rs a partir de `EntityPainting.EnumArt` do Java decompilado.

Nada digitado à mão: cada constante `NOME("Título", largura, altura, offsetX, offsetY)` do enum e o
comprimento máximo do título (`"SkullAndRoses".length()`, usado pelo S10PacketSpawnPainting) vêm do
código-fonte. Uso:  python3 scripts/gen_paintings.py
"""
import os, re

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.environ.get("MC_SRC") or os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".local/src")
JAVA = os.path.join(SRC, "net/minecraft/entity/item/EntityPainting.java")
OUT = os.path.join(ROOT, "mc-client/src/gen_paintings.rs")

src = open(JAVA, encoding="utf-8").read()
enum = src[src.index("enum EnumArt"):]
arts = re.findall(r'^\s*([A-Z0-9_]+)\("(\w+)",\s*(\d+),\s*(\d+),\s*(\d+),\s*(\d+)\)', enum, re.M)
assert len(arts) == 26, f"esperava 26 pinturas, achei {len(arts)}"
m = re.search(r'=\s*"(\w+)"\.length\(\)', enum)
title_max = len(m.group(1))

lines = [
    "// GERADO por scripts/gen_paintings.py a partir de EntityPainting.EnumArt do Java 1.8.9. Não editar.",
    "",
    "/// `EntityPainting.EnumArt`: título, tamanho em pixels e origem na `paintings_kristoffer_zetterstrand.png`.",
    "#[derive(Clone, Copy, Debug, PartialEq, Eq)]",
    "pub struct Art {",
    "    pub title: &'static str,",
    "    pub size_x: u32,",
    "    pub size_y: u32,",
    "    pub offset_x: u32,",
    "    pub offset_y: u32,",
    "}",
    "",
    f"/// `EnumArt.field_180001_A`: tamanho máximo do título no S10.",
    f"pub const TITLE_MAX: usize = {title_max};",
    "",
    f"pub const ARTS: [Art; {len(arts)}] = [",
]
for _, title, sx, sy, ox, oy in arts:
    lines.append(f'    Art {{ title: "{title}", size_x: {sx}, size_y: {sy}, offset_x: {ox}, offset_y: {oy} }},')
lines.append("];")
open(OUT, "w", encoding="utf-8").write("\n".join(lines) + "\n")
print(f"{OUT}: {len(arts)} pinturas")
