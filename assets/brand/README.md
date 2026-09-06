# rexafs brand assets

Created with the built-in imagegen tool on 2026-09-06. The palette follows the
structure viewer's copper atoms and cyan absorber. These are brand illustrations,
not representations of a measured spectrum or a specific crystal.

- `rexafs-icon.png`: original generated app emblem, with alpha preserved.
- `rexafs-release.png`: release/README artwork with the lowercase rexafs name.
- `rexafs.icns`: macOS icon used by the desktop packaging script.
- `rexafs.ico`: Windows icon resource included with the desktop download.

Regenerate icon containers from the selected PNG with:

```bash
uv run --no-project --python 3.12 --with pillow python scripts/build-brand-icons.py
```

The PNG originals are kept intact. The icon script only resizes and packages the
same artwork. No external image API key or CLI fallback was used for generation.

## Generation prompts

### App icon

```text
Use case: logo-brand
Asset type: rexafs desktop application icon, square 1024 by 1024 pixels.
Primary request: Create a polished, distinctive raster app icon for rexafs, a Rust-powered X-ray absorption spectroscopy application.
Subject: a compact abstract cluster of five copper-orange spherical atoms arranged around one small cyan atom, with a single restrained cyan scattering-wave arc integrated into the silhouette. Scientific software identity, geometric and readable at small dock sizes.
Style/medium: premium restrained 3D icon, softly shaded copper and cyan material, crisp edges, minimal detail.
Composition: centered emblem filling about 70 percent of a solid deep graphite square canvas. Full-bleed square background; do not draw rounded outer corners or a mockup frame. Balanced silhouette with generous safe margins.
Lighting: soft upper-left studio light, subtle depth, no lens flare.
Constraints: no lettering, no text, no watermark, no orbiting-electron cartoon, no chart axes, no desktop screenshot, no extra objects. This is a brand illustration, not a scientific structure diagram.
```

### Release artwork

```text
Use case: ads-marketing
Asset type: rexafs release banner, wide landscape 1792 by 1024 pixels.
Input image: reference for the copper and cyan material palette only, not an edit target.
Primary request: Create matching release artwork for rexafs, a Rust-powered X-ray absorption spectroscopy application. A restrained scientific software identity.
Composition: deep graphite background across the full canvas. On the right, a compact tasteful 3D copper atom cluster with a small cyan central atom visible through real gaps; foreground atoms correctly occlude the central atom. On the left, generous negative space with the exact lowercase word "rexafs" in a clean large off-white sans-serif, spelled r e x a f s. A thin cyan damped oscillation curve subtly bridges the composition. No other text.
Style: precise, calm, contemporary scientific software artwork; softly shaded matte copper spheres, subtle cyan accent, soft studio lighting. Less glossy than the reference. Balanced and legible at README and release-page sizes.
Constraints: no fake UI, no chart axes or numerical claims, no electron-orbit cartoon, no Wi-Fi symbol, no watermarks, no slogans. Illustrative brand art, not a rendered experimental dataset.
```
