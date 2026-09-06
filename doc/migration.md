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
under the new crate. The simple processing facade is additive. No old-name crate,
Python package or npm shim is published as part of this migration.

Python now has an importable `rexafs._core` extension and a typed
`rexafs.process` result. Compatibility function names remain under `rexafs`.
`run_batch_qas_trans` counts **successful complete pipelines**, continues after
failed inputs, and reports original input indices. It no longer counts failed
spectra as processed. The new processing facade rejects unsorted/duplicate energy;
legacy Rust setters retain their prior behavior.

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

The GitHub repository is still `ameyanagi/xraytsubaki`. Rename that existing
repository at the end, then update remote URLs, metadata and publisher bindings
as described in the [release runbook](releasing.md). The local checkout folder can
remain named `xraytsubaki`; it is not a package identifier. `rexafs.com` is the
project domain; publication and hosting status are tracked separately.

Historical profiler images can contain codename labels; fixture provenance and
scientific citations remain unchanged. They are records of earlier runs.
