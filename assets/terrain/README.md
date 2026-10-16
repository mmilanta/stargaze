# Terrain data

`patagonia-ice-field.u16` — real elevation for the explore "Alpine" ground:
641 × 641 samples, 50 m apart (x east, z north, ±16 km), centred on the
Southern Patagonian Ice Field at 49.2833° S, 73.2500° W (~1,370 m on the ice).
Values are absolute elevation in decimetres, little-endian `u16`.

Built by `scripts/patagonia_dem.py` from the AWS Open Data
[Terrain Tiles](https://registry.opendata.aws/terrain-tiles/) (Terrarium
encoding, zoom 12), which combine public sources including SRTM. See the
dataset's [attribution](https://github.com/tilezen/joerd/blob/master/docs/attribution.md).

`aeolian-islands.u16` — real elevation of six Aeolian volcanoes for the
explore "Basalt" ground, in order Stromboli, Alicudi, Filicudi, Salina,
Panarea, Vulcano: 320 × 320 samples each, 25 m apart (x east, z north),
land above sea level only and only land connected to each island's summit.
Values are decimetres, little-endian `u16`. Built by `scripts/aeolian_dem.py`
from the same Terrain Tiles (Terrarium, zoom 12).
