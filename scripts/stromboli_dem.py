#!/usr/bin/env python3
"""Prepare the Basalt ground's measured Stromboli topography.

Source: Bisson et al. (2024), Stromboli DSM 2023, INGV, CC BY 4.0.
https://doi.org/10.13127/stromboli/airborne-lidar-2023
Download/extract Stromboli_DSM_2023.tif from:
https://www.pi.ingv.it/wp-content/pdf/BancheDati/StromboliLidar/Stromboli_DSM_2023.zip

Requires numpy and rasterio (asset preparation only, never at runtime).
Usage: python scripts/stromboli_dem.py SOURCE.tif assets/terrain/stromboli-2023.u16

Output: little-endian u16 decimetres, first a 1281-square whole-island grid
at 5 m, then a 1025-square summit grid at 1 m. The whole island is centred at
the summit lookout, EPSG:32633 (518488.25 E, 4293326.25 N); the detailed grid
is centred 250 m north to include the active vents. Rows increase northward.
The 50 cm source is area-averaged, not interpolated to invent finer detail.
No cone, synthetic crater or vertical exaggeration is applied.
"""

import argparse
from pathlib import Path

import numpy as np
import rasterio
from affine import Affine
from rasterio.warp import Resampling, reproject

LOOKOUT = (518488.25, 4293326.25)
LAYERS = [(1281, 5.0, 0.0), (1025, 1.0, 250.0)]


def build(source, destination):
    result = bytearray()
    with rasterio.open(source) as dataset:
        if dataset.crs.to_epsg() != 32633 or dataset.res != (0.5, 0.5):
            raise ValueError("Expected the original 50 cm Stromboli DSM 2023 in EPSG:32633")
        for width, spacing, north in LAYERS:
            reach = (width - 1) * spacing / 2
            transform = Affine(spacing, 0, LOOKOUT[0] - reach - spacing / 2,
                               0, -spacing, LOOKOUT[1] + north + reach + spacing / 2)
            heights = np.zeros((width, width), dtype=np.float32)
            reproject(rasterio.band(dataset, 1), heights,
                      src_transform=dataset.transform, src_crs=dataset.crs,
                      src_nodata=dataset.nodata, dst_transform=transform,
                      dst_crs=dataset.crs, dst_nodata=0, resampling=Resampling.average)
            if not np.isfinite(heights).all():
                raise ValueError("Non-finite source elevation")
            heights = np.clip(heights, 0, 6553.5)
            result.extend(np.rint(heights[::-1] * 10).astype("<u2").tobytes())
            print(f"{width} x {width}, {spacing:g} m: max {heights.max():.2f} m, "
                  f"centre {heights[width // 2, width // 2]:.2f} m")
    Path(destination).write_bytes(result)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    build(args.source, args.destination)
