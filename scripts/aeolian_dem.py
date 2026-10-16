#!/usr/bin/env python3
"""Build assets/terrain/aeolian-islands.u16 for the explore "Basalt" volcano ground.

Real elevation of six Aeolian volcanoes (Stromboli, Alicudi, Filicudi, Salina,
Panarea, Vulcano), from the AWS Open Data Terrain Tiles (Terrarium, zoom 12).
Each island is a 320 x 320 grid, 25 m apart (x east, z north), centred on the
listed point; land above sea level only (the sea becomes 0), and only land
connected to the island's own summit. Values are
decimetres, little-endian u16, islands stored one after another in this order.

Usage: python3 -I scripts/aeolian_dem.py <tile-dir> assets/terrain/aeolian-islands.u16
Missing tiles are downloaded into <tile-dir>, which should be its own directory.
"""
import math, struct, sys, urllib.request, zlib
from pathlib import Path

Z = 12
N = 320
SPACING = 25.0
ISLANDS = [
    # name, centre latitude, centre longitude
    ("Stromboli", 38.7930, 15.2110),
    ("Alicudi", 38.5420, 14.3520),
    ("Filicudi", 38.5700, 14.5600),
    ("Salina", 38.5560, 14.8420),
    ("Panarea", 38.6360, 15.0660),
    ("Vulcano", 38.3830, 14.9720),
]

def decode_png(data):
    assert data[:8] == b'\x89PNG\r\n\x1a\n'
    pos, idat = 8, b''
    while pos < len(data):
        length, kind = struct.unpack('>I4s', data[pos:pos + 8]); chunk = data[pos + 8:pos + 8 + length]
        if kind == b'IHDR':
            width, height, depth, color, _, _, interlace = struct.unpack('>IIBBBBB', chunk)
            assert (depth, color, interlace) == (8, 2, 0)
        elif kind == b'IDAT':
            idat += chunk
        pos += 12 + length
    raw = zlib.decompress(idat); bpp = 3; stride = width * bpp
    out = bytearray(); prev = bytearray(stride); i = 0
    for _ in range(height):
        f = raw[i]; line = bytearray(raw[i + 1:i + 1 + stride]); i += 1 + stride
        for x in range(stride):
            a = line[x - bpp] if x >= bpp else 0; b = prev[x]; c = prev[x - bpp] if x >= bpp else 0
            if f == 1: line[x] = (line[x] + a) & 255
            elif f == 2: line[x] = (line[x] + b) & 255
            elif f == 3: line[x] = (line[x] + (a + b) // 2) & 255
            elif f == 4:
                p = a + b - c; pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[x] = (line[x] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        out += line; prev = line
    return out

tiles = {}
def pixel(px, py):
    tx, ty = px // 256, py // 256
    if (tx, ty) not in tiles:
        path = Path(sys.argv[1]) / f'{Z}_{tx}_{ty}.png'
        if not path.exists():
            url = f'https://s3.amazonaws.com/elevation-tiles-prod/terrarium/{Z}/{tx}/{ty}.png'
            path.write_bytes(urllib.request.urlopen(url, timeout=30).read())
        tiles[(tx, ty)] = decode_png(path.read_bytes())
    buf = tiles[(tx, ty)]; k = ((py % 256) * 256 + px % 256) * 3
    r, g, b = buf[k:k + 3]
    return r * 256 + g + b / 256 - 32768

def elevation(lat, lon):
    n = 2 ** Z * 256
    px = (lon + 180) / 360 * n - 0.5
    py = (1 - math.log(math.tan(math.radians(lat)) + 1 / math.cos(math.radians(lat))) / math.pi) / 2 * n - 0.5
    x0, y0 = math.floor(px), math.floor(py); fx, fy = px - x0, py - y0
    return ((pixel(x0, y0) * (1 - fx) + pixel(x0 + 1, y0) * fx) * (1 - fy)
            + (pixel(x0, y0 + 1) * (1 - fx) + pixel(x0 + 1, y0 + 1) * fx) * fy)

out = bytearray()
for name, lat0, lon0 in ISLANDS:
    grid = []
    for j in range(N):
        z = (j - (N - 1) / 2) * SPACING
        for i in range(N):
            x = (i - (N - 1) / 2) * SPACING
            lat = lat0 + z / 111_320.0
            lon = lon0 + x / (111_320.0 * math.cos(math.radians(lat0)))
            grid.append(max(0.0, elevation(lat, lon)))
    # Keep only land connected to this island's summit: neighbouring
    # islands (Lipari beside Salina and Vulcano) would be cut by the grid edge.
    top = max(grid); k = grid.index(top)
    keep = bytearray(N * N); stack = [k]; keep[k] = 1
    while stack:
        c = stack.pop(); cx, cz = c % N, c // N
        for nx, nz in ((cx + 1, cz), (cx - 1, cz), (cx, cz + 1), (cx, cz - 1)):
            if 0 <= nx < N and 0 <= nz < N and not keep[nz * N + nx] and grid[nz * N + nx] > 0:
                keep[nz * N + nx] = 1; stack.append(nz * N + nx)
    grid = [h if keep[i] else 0.0 for i, h in enumerate(grid)]
    land = sum(h > 0 for h in grid) / len(grid)
    edge = max(max(grid[:N]), max(grid[-N:]), max(grid[::N]), max(grid[N - 1::N]))
    print(f'{name:10} max {top:5.0f} m at x={(k % N - (N - 1) / 2) * SPACING:+6.0f} '
          f'z={(k // N - (N - 1) / 2) * SPACING:+6.0f}, land {100 * land:4.1f}%, edge max {edge:4.0f} m')
    out += struct.pack(f'<{N * N}H', *(min(65535, round(h * 10)) for h in grid))
Path(sys.argv[2]).write_bytes(bytes(out))
