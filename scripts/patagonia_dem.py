#!/usr/bin/env python3
"""Build assets/terrain/patagonia-ice-field.u16 for the explore "Alpine" ground.

Decodes Terrarium elevation tiles (pure stdlib) and resamples a metric grid
around a viewpoint on the Southern Patagonian Ice Field (49.2833 S, 73.25 W).

Fetch the 7x7 zoom-12 tiles first, into an empty directory of their own:
  for x in $(seq 1211 1217); do for y in $(seq 2691 2697); do
    curl -sf -o "12_${x}_${y}.png" \
      "https://s3.amazonaws.com/elevation-tiles-prod/terrarium/12/${x}/${y}.png"
  done; done

Usage: python3 -I scripts/patagonia_dem.py <tile-dir> assets/terrain/patagonia-ice-field.u16 [--stats]
Grid: 641 x 641 samples, 50 m apart, x east / z north, centred on the viewpoint.
Values: absolute elevation in decimetres, little-endian u16.
"""
import math, struct, sys, zlib
from pathlib import Path

LAT0, LON0, Z = -49.28333332, -73.25000004, 12
N = 641; SPACING = 50.0

def decode_png(path):
    data = Path(path).read_bytes()
    assert data[:8] == b'\x89PNG\r\n\x1a\n'
    pos, idat, width = 8, b'', None
    while pos < len(data):
        length, kind = struct.unpack('>I4s', data[pos:pos+8]); chunk = data[pos+8:pos+8+length]
        if kind == b'IHDR':
            width, height, depth, color, _, _, interlace = struct.unpack('>IIBBBBB', chunk)
            assert depth == 8 and color == 2 and interlace == 0, (depth, color, interlace)
        elif kind == b'IDAT':
            idat += chunk
        pos += 12 + length
    raw = zlib.decompress(idat); bpp = 3; stride = width * bpp
    out = bytearray(); prev = bytearray(stride); i = 0
    for _ in range(height):
        f = raw[i]; line = bytearray(raw[i+1:i+1+stride]); i += 1 + stride
        for x in range(stride):
            a = line[x-bpp] if x >= bpp else 0; b = prev[x]; c = prev[x-bpp] if x >= bpp else 0
            if f == 1: line[x] = (line[x] + a) & 255
            elif f == 2: line[x] = (line[x] + b) & 255
            elif f == 3: line[x] = (line[x] + (a + b) // 2) & 255
            elif f == 4:
                p = a + b - c; pa, pb, pc = abs(p-a), abs(p-b), abs(p-c)
                line[x] = (line[x] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        out += line; prev = line
    return width, out

tiles = {}
def elevation_at_pixel(px, py):
    tx, ty = int(px // 256), int(py // 256)
    if (tx, ty) not in tiles:
        tiles[(tx, ty)] = decode_png(Path(sys.argv[1]) / f'{Z}_{tx}_{ty}.png')[1]
    buf = tiles[(tx, ty)]; ix, iy = int(px) % 256, int(py) % 256
    r, g, b = buf[(iy*256+ix)*3:(iy*256+ix)*3+3]
    return r * 256 + g + b / 256 - 32768

def elevation(lat, lon):
    n = 2 ** Z * 256
    px = (lon + 180) / 360 * n - 0.5
    py = (1 - math.log(math.tan(math.radians(lat)) + 1 / math.cos(math.radians(lat))) / math.pi) / 2 * n - 0.5
    x0, y0 = math.floor(px), math.floor(py); fx, fy = px - x0, py - y0
    e = lambda x, y: elevation_at_pixel(x, y)
    return (e(x0, y0) * (1-fx) + e(x0+1, y0) * fx) * (1-fy) + (e(x0, y0+1) * (1-fx) + e(x0+1, y0+1) * fx) * fy

def latlon(x, z):
    return LAT0 + z / 111_320.0, LON0 + x / (111_320.0 * math.cos(math.radians(LAT0)))

if '--stats' in sys.argv:
    for name, lat, lon in [('viewpoint', LAT0, LON0), ('Cerro Torre', -49.2925, -73.0983), ('Fitz Roy', -49.2714, -73.0431)]:
        print(f'{name:12} {elevation(lat, lon):7.0f} m')
grid = []
for j in range(N):
    z = -16000 + j * SPACING
    for i in range(N):
        grid.append(elevation(*latlon(-16000 + i * SPACING, z)))
heights = [max(0, min(65535, round(h * 10))) for h in grid]
Path(sys.argv[2]).write_bytes(struct.pack(f'<{len(heights)}H', *heights))
c = grid[(N//2)*N + N//2]; top = max(grid); k = grid.index(top)
print(f'centre {c:.0f} m, min {min(grid):.0f} m, max {top:.0f} m at x={-16000 + (k%N)*SPACING:.0f} z={-16000 + (k//N)*SPACING:.0f}')
