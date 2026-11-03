#!/usr/bin/env python3
"""Gera mc-client/src/gen_banner.rs a partir do Java decompilado (.local/src):
padrões de estandarte (TileEntityBanner.EnumBannerPattern) e cores (EnumDyeColor + MapColor)."""
import re, sys, pathlib
src = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ".local/src") / "net/minecraft"
banner = (src / "tileentity/TileEntityBanner.java").read_text()
dye = (src / "item/EnumDyeColor.java").read_text()
mapc = (src / "block/material/MapColor.java").read_text()

pats = re.findall(r'^\s+([A-Z_]+)\("([a-z_]+)", "([a-z]+)"', banner, re.M)
colors = {m.group(1): int(m.group(2)) for m in re.finditer(r'public static final MapColor (\w+) = new MapColor\(\d+, (\d+)\)', mapc)}
dyes = re.findall(r'^\s+([A-Z_]+)\((\d+), (\d+), "(\w+)", "\w+", MapColor\.(\w+),', dye, re.M)
by_damage = {int(d[2]): (d[3], colors[d[4]]) for d in dyes}

out = ["// Gerado por scripts/gen_banner.py a partir do Java decompilado. Não editar.", ""]
out.append(f"/// `EnumBannerPattern`: (nome do arquivo em `textures/entity/banner/`, id curto).")
out.append(f"pub const PATTERNS: [(&str, &str); {len(pats)}] = [")
for _, name, pid in pats:
    out.append(f'    ("{name}", "{pid}"),')
out.append("];\n")
out.append("/// `EnumDyeColor.byDyeDamage(i)`: cor do `MapColor` (0xRRGGBB), na ordem do dano de tinta.")
out.append("pub const DYE_COLORS: [u32; 16] = [")
for i in range(16):
    n, c = by_damage[i]
    out.append(f"    0x{c:06X}, // {n}")
out.append("];")
pathlib.Path("mc-client/src/gen_banner.rs").write_text("\n".join(out) + "\n")
print(len(pats), "padrões,", len(by_damage), "cores")
