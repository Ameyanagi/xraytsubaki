# Migrating from xraytsubaki to rexafs

The public name is **rexafs**. xraytsubaki remains the historical working codename.
ReFEFF is an upstream calculation engine and keeps its name.

| Before | Now |
|---|---|
| `crates/xraytsubaki` | `crates/rexafs` |
| `crates/xraytsubaki-gui` | `crates/rexafs-gui` |
| `py-xraytsubaki` | `py-rexafs` |
| `cargo test -p xraytsubaki` | `cargo test -p rexafs` |
| `cargo run -p xraytsubaki-gui` | `cargo run -p rexafs-gui` |
| `target/release/xraytsubaki-gui` | `target/release/rexafs` |
| Rust `use xraytsubaki::…` | `use rexafs::…` |
| Python codename imports | `import rexafs` |

`Spectrum`/`Group` are concise aliases; `XASSpectrum`/`XASGroup` remain accessible
under the new crate. No old-name crate, Python package or npm shim is published.

## Spectrum processing

The standalone `process`, `process_with_options`, `ProcessOptions` and
`ProcessedSpectrum` APIs have been removed. Python's legacy free pipeline/batch
wrappers are removed too. Use the Rust spectrum workflow in every language:

- Rust: `Spectrum::from_arrays(&energy, &mu)?.fft()?` (keep an owned spectrum to inspect results).
- Python/TypeScript: `Spectrum.from_arrays(energy, mu).fft()`.
- Optional explicit stages: `normalize()`, `calc_background()`, `fft()`.
- Edge override: `set_e0(value)` before requesting a stage.
- Outputs: `chi()`, `r()`, `chir_mag()` and other spectrum getters.
- Python QAS files: `rexafs.io.read_qas_transmission(path).fft()`.

The terminal stage computes missing prerequisites with configured algorithms and
defaults. Configure stages through Rust-named configuration types and spectrum
setters; see the [API guide](api.md). Checked constructors reject unsorted or
duplicate energy values; the legacy Rust setter retains sorting behavior.

Spectrum result accessors now omit `get_`: for example, `get_e0()` becomes `e0()`
and `get_chir_mag()` becomes `chir_mag()`. Use `k()` and `chi()` in place of
`get_k()` and `get_chi()`; in Rust these borrow slices, so use `.to_vec()` or
`DVector::from_column_slice(...)` when an owned result is needed. Python and
TypeScript continue to return independent arrays. `set_xftf(parameters)` is now
`set_fft(parameters)`. `calc_background()` and the other setters keep their names.

## Existing desktop data

- `.rxs` is the first release project format and the only supported suffix.
  Unreleased formats have no compatibility loader. Source links default to paths
  relative to the project directory; optional embedded originals are portable.
  See the [project policy](project-compatibility.md) for future-release compatibility.
- New settings and FEFF workspaces use `~/.rexafs`. If the new settings file does
  not exist, settings are read from `~/.xraytsubaki/settings.json`; the next save
  writes the new location. The original file is retained.
- A downloaded AMCSD database in the old directory remains usable. Stored source,
  CIF, database and FEFF paths are preserved, including old workspace paths.
- `REXAFS_SETTINGS`, `REXAFS_FEFF_BACKEND`, `REXAFS_THEME` and other `REXAFS_*`
  launch hooks take precedence over their old `XTS_*` aliases. Explicit settings
  overrides are authoritative and do not fall back to unrelated files.
- Catalog caches use the new product name and can be rebuilt from source files.
  Cache data is not the original measurement data.
- Assistant protocol tool identifiers such as `xray_get_state` remain internal
  compatibility identifiers. Product labels and generated reports say rexafs.

## Repository and domain

The GitHub repository has been renamed from `ameyanagi/xraytsubaki` to
[`Ameyanagi/rexafs`](https://github.com/Ameyanagi/rexafs). Update existing clones with
`git remote set-url origin https://github.com/Ameyanagi/rexafs.git`. Package metadata
uses the new URL. Initial registry authentication is configured; crates.io/npm
trusted publishers follow their first uploads as described in the
[release runbook](releasing.md). The local checkout folder name is not a
package identifier. `rexafs.com` is the project domain; publication and hosting
status are tracked separately.

Historical profiler images can contain codename labels; fixture provenance and
scientific citations remain unchanged. They are records of earlier runs.
