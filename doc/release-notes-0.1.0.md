# rexafs 0.1.0 — release notes

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

## Distribution and validation

[`rexafs 0.1.0`](https://github.com/Ameyanagi/rexafs/releases/tag/v0.1.0) is public
with signed macOS downloads. The library is available on crates.io and npm, and all
20 Python wheels plus the source distribution are available on PyPI. The source
archive's omitted root licenses were restored by
[repair run 34028288522](https://github.com/Ameyanagi/rexafs/actions/runs/34028288522),
which passed installation and API tests before upload. The source and binaries
come from [tagged build 34025866097](https://github.com/Ameyanagi/rexafs/actions/runs/34025866097),
which passed all 29 jobs at `ee365067ba97a762888caf65593af213dca5b7e4`.

The preceding pull-request matrix passed Rust package checks, CPython 3.10–3.14
wheel tests on Linux, Apple Silicon macOS, Intel macOS and Windows, Python sdist
installation, npm/TypeScript/Chromium checks and four desktop archive self-checks.
That run is validation evidence, not the source of final release uploads.

[Signing run 34032364680](https://github.com/Ameyanagi/rexafs/actions/runs/34032364680)
signed and notarized both macOS architectures, stapled their tickets, and passed
Gatekeeper and extracted-app checks. The signed Apple Silicon application was
launched locally: its Cu normalization/FFT plots rendered, and a project was saved
and reopened. The Intel build also launched and reopened that project under
Rosetta on Apple Silicon; native Intel graphical hardware was not tested locally.

Windows and Linux desktop archives remain in the tagged CI build pending graphical
launch qualification, and are excluded from the public desktop release. Their
Python wheels are published. The 30 public release assets have verified hashes;
`SHA256SUMS` and `RELEASE-PROVENANCE.json` record their final contents and origin.

The project's source is licensed under MIT OR Apache-2.0. Dependencies and
scientific reference data retain their own notices. Identify the calculation
backend when reporting scientific results.
