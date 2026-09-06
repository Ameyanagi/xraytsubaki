# rexafs brand assets

Regenerated with the built-in imagegen tool on 2026-09-06 in a geometric direction.
Six copper facets surround a cyan hexagon, combining a crystalline motif with a
subtle reference to the camellia codename. The copper/cyan palette connects the
identity to the structure viewer. These are abstract brand illustrations, not
representations of a measured spectrum or a specific crystal.

- `rexafs-icon.png`: generated geometric app emblem, with alpha preserved.
- `rexafs-release.png`: matching release/README banner with the lowercase rexafs name.
- `rexafs.icns`: macOS icon used by the desktop packaging script.
- `rexafs.ico`: Windows icon resource included with the desktop download.

Regenerate icon containers from the selected PNG with:

```bash
uv run --no-project --python 3.12 --with pillow python scripts/build-brand-icons.py
```

The generated PNG masters are kept intact. The icon script only resizes and
packages the same artwork. No external image API key or CLI fallback was used.

## Generation prompts

### App icon

```text
Use case: logo-brand
Asset type: rexafs desktop app icon; one square 1024 × 1024 raster master.
Primary request: Regenerate the rexafs identity in a much more geometric, minimal style (幾何学的). Create one distinctive flat emblem based on a crystalline hexagonal rosette: six precisely constructed copper rhombus facets around one small cyan central hexagon, separated by clean even negative-space channels. A subtle geometric camellia reference is welcome, but the mark must read primarily as a precise scientific lattice.
Style: Swiss modernist graphic design, exact 60-degree angles, a disciplined triangular grid, flat solid fills, crisp antialiased edges, bold simple silhouette legible at 24 pixels. Two closely related copper tones may distinguish facets; one cyan accent.
Palette: warm copper #CF8748 / #AD6534 and clear cyan #68D6E7. Genuine transparent background.
Composition: a single centered symmetrical emblem occupying about 72% of the canvas, ample equal safe margins. No surrounding tile or frame.
Constraints: no text, no letters, no wordmark, no gradients, no 3D, no spheres, no glossy reflections, no shadows, no texture, no orbit rings, no Wi-Fi arcs, no decorative linework, no watermark, no presentation board. This is abstract brand art, not a claim about a specific crystal.
```

### Release artwork

The newly generated app icon was supplied as the reference image.

```text
Use case: logo-brand
Asset type: rexafs release and README banner, wide landscape approximately 1792 × 1024.
Input image: reference for the exact new geometric rexafs emblem, its six copper facets, cyan central hexagon, and palette. This is a matching new composition.
Primary request: Create a minimal geometric scientific-software identity banner for rexafs. Use the same sixfold crystalline rosette emblem from the reference on the right, reproduced faithfully, with generous space around it. On the left set the exact lowercase word "rexafs" in large, precise, clean off-white geometric sans-serif typography, spelled r e x a f s. Align the wordmark and emblem to the same visual center.
Backdrop: a uniform deep graphite #171C22 background across the entire rectangular canvas.
Style: restrained Swiss modernist graphic design, mathematically crisp polygon edges, flat copper and cyan fills, clean negative space, confident and quiet. The emblem should occupy about one third of the canvas width; wordmark and emblem have balanced visual weight.
Text (verbatim): "rexafs". No other text.
Constraints: preserve the emblem geometry and palette from the reference; no 3D, no glossy surfaces, no spheres, no shadows, no glow, no texture, no gradients, no wave curves, no chart axes, no UI screenshot, no mockup, no slogans, no watermark. This is abstract brand artwork rather than a scientific dataset.
```
