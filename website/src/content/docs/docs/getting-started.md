---
title: "Start with a spectrum"
description: "Open-source X-ray absorption analysis across formats and platforms."
audience: user
---

rexafs is free and open source under [MIT or Apache-2.0](/licenses/), built for
fast X-ray absorption spectroscopy (XAS) analysis in a small package. It supports
text/XDI spectra, CIF/XYZ structures and CSV/SVG/PNG exports, with desktop tools
for scattering calculations, joint fits and measurement series.

This manual covers **stable 0.2.14** on the desktop and in Python, TypeScript and
Rust. The browser previews offer [spectrum processing](/app/) and
[ReFEFF scattering](/app/scattering/), separately from the published npm API.

## Who this manual is for

- **Desktop users:** follow the analysis and fitting guides below.
- **Programmers:** choose a library and its versioned API reference.
- **New to XAS:** start with [concepts and glossary](/docs/concepts/).

## Choose your path

| Desktop | Libraries |
|---|---|
| [Install the application](/docs/getting-started/install/) | [Choose Python, TypeScript or Rust](/docs/libraries/) |
| [Follow your first analysis](/docs/getting-started/first-analysis/) | [Run the spectrum pipeline](/docs/libraries/spectrum-api/) |
| [Build a structural fit](/docs/desktop/fitting/) | [Read the generated API reference](/docs/reference/) |

## A typical analysis

1. **Import** a spectrum and check its columns and energy unit.
2. **Process**: normalize after checking the edge energy and baseline windows.
3. **Process**: extract and transform EXAFS with AUTOBK and a Fourier transform.
4. **Fit** scattering paths from a structure; inspect residuals and correlations.
5. **Save and export** a `.rxs` project, figures and data.

Use the bundled Cu example for [your first
analysis](/docs/getting-started/first-analysis/), then [fit a coordination
shell](/docs/desktop/fitting/).

## Features by interface

Python and TypeScript expose spectrum processing and selected analysis APIs. Rust includes the additional
capabilities below, with optional features where indicated.

| Feature | Desktop | Python | TypeScript / JS | Rust |
|---|:---:|:---:|:---:|:---:|
| Energy/absorption arrays, normalization, AUTOBK, FFT and IFFT | Yes | Yes | Yes | Yes |
| Normalization/background/forward-FFT settings | Yes | Yes | Yes | Yes |
| Custom inverse-transform settings | Yes | Yes | Yes | Yes |
| Text mapping, XDI and multiple imported channels | Yes | Shared reader / NumPy | Shared reader / arrays | Shared reader |
| MBACK atomic-reference normalization | Yes | Yes | Yes | Yes |
| Cauchy wavelet maps and region measurements | Yes | Yes | Yes | Yes |
| XANES peak, step and baseline fitting | Yes | Yes | Yes | Yes |
| Fluorescence correction (XANES only) | Yes | Yes | Yes | Yes |
| Scalar measurements | Yes | Yes | Yes | Yes |
| Reverse Monte Carlo refinement | One spectrum/structure | — | — | Yes, `refeff-runner` |
| Experimental Live acquisition | Yes | — | — | — |
| Groups, marks, processing overrides and comparisons | Yes | Loop over spectra | Loop over spectra | Group API |
| Alignment, calibration, deglitching, truncation, rebinning, smoothing, merging and differences | Yes | — | — | Yes |
| Linear combination fits and principal components | Yes | — | — | Yes |
| CIF/XYZ structures, built-in structures and online structure sources | Yes | — | — | Yes |
| FEFF path calculation and structural fitting | Yes | — | — | Optional engines |
| Simultaneous fits, shared/local parameters and independent batches | Yes | — | — | Yes |
| Portable projects, backups and fit history | Yes | — | — | Desktop-specific format |
| Publication figures, captions, data and methods exports | Yes | Use plotting tools | Use plotting tools | Optional plotting |
| Optional analysis assistant | Yes | — | — | — |

Use the [API version guide](/docs/reference/) to select documentation matching
your installed package. **Next** documents the source checkout.

## Know your data

Photon energy is in **eV**, wave number $k$ and back-transform coordinate $q$ are
in **Å⁻¹**, and Fourier coordinate $R$ is in **Å**. Supply finite arrays with a
strictly increasing energy axis. Construct absorption from your detector signals
before passing arrays to a library.

The [processing guide](/docs/science/processing/) explains the equations and units.
A Fourier peak is not automatically a phase-corrected bond distance. A converged
fit still needs inspection of its residuals, parameter correlations and model.

## Support rexafs

If rexafs is useful to you, [star the repository on GitHub](https://github.com/Ameyanagi/rexafs)
or [sponsor development through GitHub Sponsors](https://github.com/sponsors/Ameyanagi).
Sponsorship supports ongoing development and maintenance; stars help others discover the project.
