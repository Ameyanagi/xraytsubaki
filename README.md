# rexafs

![rexafs — Rust-powered X-ray absorption analysis](assets/brand/rexafs-release.png)

**Rust-powered X-ray absorption analysis.**

rexafs processes measured XAS spectra, removes EXAFS backgrounds, computes Fourier
transforms and fits scattering-path models. Use the Rust library, Python bindings,
JavaScript/Wasm package or desktop application. [rexafs.com](https://rexafs.com) is
the project's domain; website deployment is part of the release plan.

Developed under the codename **xraytsubaki**, inspired by the camellia. The `r`
evokes Rust and the connection to **ReFEFF**, the optional Rust scattering engine.
The project began with the need to process large in-situ measurement series.

## Release status

The source tree uses `rexafs`. Registry publication and GitHub downloads are being
prepared; the commands below build from this checkout. The repository is still
[`ameyanagi/xraytsubaki`](https://github.com/ameyanagi/xraytsubaki) and will be renamed
at the end of the migration. See the [release plan](doc/rebranding-plan.md),
[migration guide](doc/migration.md) and [release runbook](doc/releasing.md).

## What is available

| Surface | Implemented scope |
|---|---|
| Rust core | Normalization, AUTOBK, FFT/IFFT, parallel groups, alignment/rebinning/merging, LCF/PCA, structures, path fitting and joint/independent fits |
| Python | Spectrum stages and configuration with NumPy results; QAS file reader |
| JavaScript / TypeScript | Spectrum stages and configuration through Wasm in Node and browsers |
| Desktop | Import, processing, structures and path selection, fitting, project persistence and publication exports |

Optional Rust integrations include [ReFEFF](https://crates.io/crates/refeff), FEFF10,
structure databases and plotting. The Python and JavaScript packages expose the
small processing API; they do not yet expose all Rust fitting and structure APIs.
The desktop uses the published [`xraydb`](https://crates.io/crates/xraydb) crate
from [`xraydb-rs`](https://github.com/Ameyanagi/xraydb-rs) for absorption-edge
identification. The desktop's experimental assistant is optional.

## Build from source

The release work is tested with Rust 1.98.1. The desktop uses edition 2024 and a
pinned GPUI dependency; see the runbook for platform qualification.

```bash
cargo test -p rexafs
cargo run --release -p rexafs-gui
```

The desktop executable is `target/release/rexafs`. To build only the ReFEFF backend:

```bash
cargo build --release -p rexafs-gui --no-default-features --features refeff-runner
```

Python development (CPython 3.10–3.14):

```bash
uv venv --python 3.12
uv pip install maturin numpy
uv run --no-project maturin develop --release
```

JavaScript build (Node 22+, Rust Wasm target and `wasm-pack` on PATH):

```bash
rustup target add wasm32-unknown-unknown
npm --prefix js-rexafs run build
npm --prefix js-rexafs test
```

## Spectrum API

Each language uses the same normalization → AUTOBK → Fourier pipeline. Inputs are
finite, equal-length arrays with strictly increasing energy in eV.

```rust,ignore
let mut spectrum = rexafs::Spectrum::from_arrays(&energy, &mu)?;
spectrum.fft()?;
```

```python
import rexafs
spectrum = rexafs.Spectrum.from_arrays(energy, mu).fft()
print(spectrum.e0(), spectrum.k(), spectrum.chi())
```

```javascript
import init, { Spectrum } from "rexafs";
await init();
const spectrum = Spectrum.from_arrays(energy, mu).fft(); // Float64Array inputs
```

See the [API guide](doc/api.md) for units, errors and advanced Rust entry points,
[Python guide](py-rexafs/README.md) and [JavaScript guide](js-rexafs/README.md).

## Documentation and scientific context

### Saving projects and compatibility

Use **Save project** / **Open project** with **`.rxs`** files. This is the first
release format; unreleased codename formats are not supported. **Raw: paths** is
the default: source paths are relative to the project file's directory. Move the
project and data folders together. Choose **Raw: embedded** to include losslessly
compressed original spectra and referenced FEFF files for portability.

Saved projects use compact JSON and omit redundant defaults while retaining
numeric precision, arrays, expressions and metadata. The file begins with a header
containing format/software versions, timestamps, source paths, checksums and
original comment headers. Every save checks the reconstructed state, then uses
atomic replacement and keeps the previous `.rxs.bak`.

Projects store processing, fit history/models, joint assignments, derived spectra
and publication settings. Embedded inputs retain their original bytes; derived
spectra retain full arrays in either mode. See the
[compatibility and recovery policy](doc/project-compatibility.md).

Every release must add small retained linked and embedded project fixtures. GitHub
checks their manifest and runs load/save/reopen, relocation, byte-recovery, backup and failure tests alongside
the Rust numerical and Python/JavaScript API regressions. Historical fixtures stay
in the suite; a release version without its fixture fails the release gate.

### Publication figures and tables

**Publish** lets you set plot size, DPI, labels, limits and visible curves, then
save PNG or vector SVG. Defaults come from ruviz and previews preserve aspect
ratio. Figure/table captions are editable and saved with the project. Analysis
exports include a report with numbered captions, units, uncertainty notes and
source records. See the [publication guide](doc/publication.md).

The [documentation index](doc/README.md) links current workflows, plotting,
validation records and design history. Historical benchmark results retain their
hardware and workload context; no single speedup is promised for all inputs.
XrayLarch provides algorithm and regression-reference context. ReFEFF, FEFF and
imported structure/data sources retain their own names and attribution.

## License

The project's own source is dual-licensed under [MIT](LICENSE-MIT) or
[Apache-2.0](LICENSE-APACHE), at your option. See [COPYRIGHT.md](COPYRIGHT.md).
Dependencies and reference fixtures retain their own terms. The release license
gate requires a non-GPL license choice for every Rust dependency; see the
[distribution notices](doc/distribution-notices.md). Identify the actual calculation
backend when reporting scientific results.
