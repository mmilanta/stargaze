# Continuous terrain

The lookout uses a triangulated heightfield, replacing the separate ellipsoid
hills. One surface forms the valley floor, slopes and connected ridgelines;
rocks, trees and the cottage remain separate grounded props.

## Generation

A fixed seed generates gradient noise with quintic interpolation. Rotated fBm
octaves warp the coordinates of a seven-octave ridged multifractal. A broad,
meandering valley keeps an open observing direction. The mountain component
uses a 900 m amplitude, reduced 25% from the initial terrain. Smaller noise bands form
the valley floor, with a flat clearing immediately around the observer.
Eighteen thermal-erosion iterations relax steep differences by transferring
material to neighbours. Transfers account for unequal cell areas and conserve
volume. This is talus relaxation, not a hydraulic river or sediment simulation.

The surface has 512×512 cells (524,288 triangles). Quadratically spaced
coordinates concentrate sub-metre cells around the observer and gradually
increase spacing toward the edge of the 32 km patch. Adjacent cells share
vertices; there are no independently generated hill meshes or LOD seams.
The last two kilometres feather down to the host sphere. Planetary curvature
is included in every vertex and conservative intersection bound. The patch
scales down on worlds smaller than a 200 km radius.

Generation runs once per preset per process. Heights, derivatives and a min/max quadtree
occupy about 3.68 MiB in the landscape GPU buffer. A compute pass caches
host-curved bounds, vertices and normals in a further 8.02 MiB; it reruns only
when the host radius or terrain preset changes. Camera turns, animation and accumulation resets
reuse that geometry. The shader traverses bounds near to far with a 256-byte
per-ray stack and intersects exact triangles only in surviving leaves;
it does not evaluate fractal noise or erosion for each ray. Surface normals
interpolate cached vertex normals. Shadow queries stop at the first opaque
blocker before the light and skip normal interpolation. Slopes blend soil into exposed rock; fine material
grain fades with distance. Props sample the same triangulation for placement.

The local terrain rotates with the host and stays fixed when the camera turns.
This is a seeded illustrative landscape shared by the hosts, not a geological
model for each planet. It cannot represent caves or overhangs. Direct lighting still uses the existing
stellar transport. Local surfaces now also receive atmospheric sky lighting as
described below.

`camera.ground: desert` selects a broad gravel basin with sparse rocks, barren
ridges and a narrow, meandering off-road trail. Two dusty tyre tracks blend
into the surrounding gravel, with no paving or painted markings. Its colours
are procedural diffuse materials. Its seeded rocks mix slabs, elongated fragments
and chunky boulders with different sizes and earth tones. Their three-axis
rotations are stored as packed angles in the existing prop record; rotated
bounds and normals use the same orientation. The preset omits vegetation and buildings;
vegetation density therefore has no effect on it. The default `forest` preset
preserves the original landscape. Switching themes uploads the selected source
heightfield and invalidates the curved geometry cache, even at the same host
radius. Both presets use the same GPU buffer. See [the desert level](atacama.md).

## Volcanic field

`camera.ground: basalt` (Level 1) stands on the summit crater of Stromboli,
using real 25 m elevation data, with eight real Aeolian islands turned into
cones on the horizon. A procedural volcanic field fills the lava plain
between them: three size classes on jittered grids (`VOLCANO_CLASSES` in
`terrain.rs`). Large stratovolcanoes (360-1050 m) merge with the real ones,
while scoria cones (80-300 m) and spatter cones (12-60 m) are added to
whatever lies beneath them, so some rise from the flanks of larger volcanoes as
parasitic vents. Each cone has a concave flank, a lobed footprint, a summit
crater (about a third are breached into a horseshoe), radial gullies of
irregular depth and loose scoria roughness. Grid-resolution weights fade
detail the distant cells cannot resolve. Stromboli's smooth slopes gain the
same gullies below its summit. The lava plain has overlapping flows with
pressure ridges. This ground uses the 1024-cell grid.

`basalt_floor` in `ground.wgsl` repeats the cone placement (same hash and
arithmetic; a unit test checks the class constants match). Every surface point
therefore knows which cones it lies on. Stratovolcanoes have grey ash upper
slopes cut by dark lava and red scoria streaks running downhill. Scoria cones
vary from iron-black through brown to oxidised red, both between cones and in
patches over each. Craters have pale fumarole crusts on the rim and ochre
altered ground in the bowl. Steep walls show layered beds. The plain mixes
fresh glassy flows, older brown aa lava and rusty crusts. A four-octave
normal perturbation (0.35 m to 75 m features) adds rough lava relief. Each band
fades once a pixel covers it.

## Dune sea

`camera.ground: dunes` (Level 2) stands on Big Daddy above Deadvlei, with
star dunes on a jittered grid around it. The ground uses the 1024-cell grid,
and crests round over 2.4 cells rather than 1.5. That keeps them knife-sharp
but stops the shading terminator stepping from triangle to triangle on
crests that run diagonally across the grid. After the large dunes are
generated, a second pass adds compound dunes where they leave gentle ground
(the steepest one-sided slope stays under ~0.3, so slip faces and crests stay
clean). These are sinuous transverse ridges 260 m apart and up to 22 m high.
Near the lookout, 47 m ripples of 2 m sit on top. Both have a long windward
slope and a short slip face facing the prevailing wind (`DUNE_WIND`,
north-east, like Big Daddy's) and break into crescent segments. They stay off
the pans and the summit.

`dune_floor` paints darker, redder plinths and apricot upper dunes. Slip faces
have paler, freshly avalanched sand in grain-flow tongues running downhill.
Gentle low ground carries dark heavy-mineral lag, and corridors are pale buff
gravel with dark pebbles. The pans are white clay with darker damp shores.
Desiccation cracks there widen and fade with the pixel footprint, so the
network still greys the clay where single cracks vanish. Wind ripples are a
normal perturbation: sinuous, asymmetric crests 12 cm apart with forks and
ends, plus coarser 0.8 m ripples on the corridors. There are none on slip
faces or pans, and each band fades once a pixel covers it.

## Ice field

`camera.ground: alpine` (Level 3) uses the real Patagonian elevation data on
the 1024-cell grid. The lookout now stands on a 180 m nunatak with concave
flanks, so the eye looks straight down onto the ice instead of across it from
2 m. Ridged crags and broken rock restore the jagged detail the 50 m samples
round off the granite. Ribs and gullies cut the nunatak without rising above
its summit. The ice has broad swells and drifts elongated along the flow
(`ICE_FLOW`, west-south-west).

Crevasses are drawn by `alpine_floor`, not the heightfield: cracks a few
metres wide alias into dashed blocks on distant cells, while the shader
antialiases them by pixel footprint at any distance. They are lens-shaped,
staggered rows across the flow, open where the ice is under tension (in
patches and round the nunatak's foot), with blue walls fading to darkness and
bright snow lips. The rest of the ice is wind-packed crust and powder, bare
blue glacier ice with white fractures, and dirty, rock-strewn ice under the
mountains. Granite has joints and lichen, with snow on every ledge that holds
it and wind-scoured outcrops on the nunatak. Sastrugi and drift ridges are a
three-band normal perturbation stretched along the wind. `ground.wgsl`
mirrors the wind, pan, flow and nunatak constants; a unit test checks them.

## References

- Ken Perlin, [Improving Noise (2002)](https://mrl.cs.nyu.edu/~perlin/paper445.pdf):
  smooth gradient noise and quintic interpolation.
- F. Kenton Musgrave, Craig E. Kolb and Robert S. Mace,
  [The Synthesis and Rendering of Eroded Fractal Terrains (1989)](https://dl.acm.org/doi/10.1145/74333.74337):
  foundational work on fractal terrain and erosion.
- Art Tevs, Ivo Ihrke and Hans-Peter Seidel,
  [Maximum Mipmaps for Fast, Accurate, and Scalable Dynamic Height Field Rendering (2008)](https://pure.mpg.de/pubman/item/item_1325622_5/component/file_3590464/i3d08.pdf):
  hierarchical height bounds for accelerating heightfield intersections.
- Xing Mei, Philippe Decaudin and Bao-Gang Hu,
  [Fast Hydraulic Erosion Simulation and Visualization on GPU (2007)](https://evasion.imag.fr/Publications/2007/MDH07/):
  a possible future direction for drainage channels and sediment transport.

These papers inform the design; the code is an independent implementation.
In particular, its min/max quadtree and exact leaf triangles are not a reproduction
of the maximum-mipmap traversal, and hydraulic erosion is not implemented.

## Validation

CPU checks cover finite heights, clearing and edge continuity, all hierarchy
bounds, conservative erosion and grounded prop bounds. A GPU oracle compares
20 rays against an independent exhaustive f64 intersection of all triangles on
Earth-sized and 5 km-radius hosts. It includes near-horizontal rays, misses,
axis-aligned rays, and patch boundaries. Additional GPU tests check prop bounds,
shadows, darkness without light sources and accumulation after quality changes.

See [performance](performance.md) for measurements and [the README](../README.md#rocky-lookout)
for a reproducible native preview.

## Forest clearing and materials

Eight irregular groves surround an open observing direction. Each conifer uses
one bounded record containing a trunk and six analytic branch tiers. A stable
procedural mask breaks up the tier edges and creates gaps. Sparse grass clumps
use nine triangular blades each; capped cylinders form fallen logs. Nested
bounds skip entire groves when rays miss them. The full scene uses 198 of 256
prop records; this is an economical procedural representation, not a scanned
botanical model.

World-space moss, earth, needles and gravel blend across the ground, with exposed
stone on steep slopes. Material detail and a small normal perturbation fade with
distance to limit aliasing. Bark has longitudinal variation and stones have
patchy lichen. These are procedural materials, without downloaded texture assets.

The Settings vegetation slider selects 0–100% density. Lower settings remove a
deterministic subset of trees and grass without moving the survivors. Terrain,
rocks, the cottage and logs remain. Presets choose 35%, 70% and 100%; the separate
slider can override them. Applying density changes resets scene accumulation.

## Sky lighting

A 16×8 hemisphere map caches the existing atmospheric scattering model at the
observer. It is rebuilt when scene history resets, not for every sample. Each
local diffuse vertex takes one cosine-weighted sky sample, testing the actual
terrain, props, celestial bodies and ring transmission for occlusion. The
Lambertian cosine and sampling density cancel. Continuation rays do not add the
cache again, so sky illumination is not double-counted. The cache excludes
stellar discs and the decorative Milky Way; stars retain their existing direct
lighting estimator. No atmosphere or no illuminating sources produces a black
cache, not a constant ambient term.

This is a local lighting approximation: the observer's sky is reused across the
32 km patch, with coarse angular resolution and no multiple atmospheric
scattering. It is not a full volumetric path tracer. The cache occupies 2 KiB in
the landscape buffer and uses a separate compute dispatch before tracing, with
no additional storage-buffer binding. Airless scenes skip sky sampling.

A GPU regression checks finite lit cache values, cache reuse, canopy occlusion,
and invalidation to black for airless and unlit scenes. CPU checks cover density
subsets, the observing gap, settings draft/cancel behavior and all prop bounds.
