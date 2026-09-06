# rexafs 0.1.0 — draft release notes

Rust-powered X-ray absorption analysis. The `r` stands for Rust and reinventing
the wheel for EXAFS analysis. Developed under the codename xraytsubaki.

## Included in this release

- A Rust library for normalization, AUTOBK background removal, Fourier transforms,
  group processing, alignment, rebinning, merging, LCF/PCA and EXAFS path fitting.
- A shared `Spectrum` workflow in Rust, Python and JavaScript/TypeScript, with
  checked array inputs, configurable processing stages and result getters.
  Requesting a stage computes any missing prerequisites.
- A desktop application for importing spectra, processing and fitting, structure
  viewing and path selection, with the ReFEFF scattering engine in the packaged
  build.
- `.rxs` projects with relative input paths or embedded original data, saved
  processing and fit state, checked atomic saves and previous-save backups.
- Publication figures in PNG/SVG and reports with numbered captions, units,
  uncertainty notes and source records.
- Dependency license notices, build metadata, an example spectrum and SHA-256
  checksums accompanying the packaged desktop application.

See the [API guide](api.md), [migration guide](migration.md),
[publication guide](publication.md) and [full changelog](../CHANGELOG.md).
The Python and JavaScript bindings expose the spectrum processing API; advanced
Rust fitting and structure APIs are not all available through those bindings.

## Distribution status before publication

These notes are a draft. No release assets or registry packages have been
published. Promote only the artifacts from a successful manual GitHub build of
the final `v0.1.0` commit, and update this section with its run link and the
channels actually published.

The preceding pull-request matrix passed Rust package checks, CPython 3.10–3.14
wheel tests on Linux, Apple Silicon macOS, Intel macOS and Windows, Python sdist
installation, npm/TypeScript/Chromium checks and four desktop archive self-checks.
That run is validation evidence, not the source of final release uploads.

Desktop archives are unsigned. Interactive launch qualification on Intel macOS,
Linux and Windows remains pending; automated archive self-checks do not verify GPU
rendering. Record the final downloads' launch, import, processing, project reopen
and ReFEFF results before advertising those targets as supported.

The project's source is licensed under MIT OR Apache-2.0. Dependencies and
scientific reference data retain their own notices. Identify the calculation
backend when reporting scientific results.
