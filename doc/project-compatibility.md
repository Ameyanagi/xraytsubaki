# Projects, portability and recovery

**`.rxs` is the only project suffix.** Format 1 is the first release format.
Unreleased codename formats have no compatibility loader or file association.
Use **Save project** and **Open project** in the desktop app.

Projects store processing settings, overrides, derived spectra, fit models/history,
joint assignments and publication settings, including figure sizes and captions.
Materials Project credentials remain in machine-local settings.

## Raw data: paths or embedded originals

Choose **Raw: paths / Raw: embedded** beside **Save project**. This choice is
remembered in the project and also applies to **Export analysis folder**.

| Mode | Contents | Moving or sharing |
|---|---|---|
| **Paths (default)** | Source references and metadata; no original file payloads | Move the project and source folders together, preserving their relative layout |
| **Embedded** | Losslessly compressed original spectra and referenced FEFF inputs | Reopen the `.rxs` file without its original input files |

Relative paths resolve against the **directory containing the project**, never the
process working directory. For example, `run.rxs` beside a `data` folder stores
`data/cu.xmu`; a project inside `projects` stores `../data/cu.xmu`. Save As
recalculates references to the same files. Different Windows drives require
absolute paths. The save dialog starts in the current project's directory, or
the source folder for a new project.

Embedded mode includes the imported spectrum folder, standalone sources, override
and joint-fit inputs, referenced current/historical FEFF paths, and available
`feff.inp`, `crystal.json` and `engine.txt` workspace metadata. It waits for
catalog scanning and fails explicitly if a required source is missing or unreadable.
Original bytes, including comments, line endings and non-text bytes, survive
unchanged. Identical files share one compressed payload. Extraction checks byte
counts and SHA-256 hashes and writes only inside the private rexafs cache.

Paths mode permits recording unavailable sources so settings can still be saved;
unavailable entries lack size/checksum metadata. Linked files can change independently.
Their recorded hashes identify the bytes present at save time. Use embedded mode
for a portable input snapshot. Switching an opened embedded project to paths
references its currently extracted cache files; keep embedded mode for sharing it.

Processed spectra are recomputed when reopening. Derived spectra created inside
the app retain full energy and μ arrays in both modes. Historical fit statistics
and models remain saved; historical curve arrays are available through analysis
exports when present in the session. Embedded mode does not include unrelated
workspace files or a saved executable/backend. **Export analysis folder** adds
processed arrays, available fit arrays and captioned figures/tables beside `project.rxs`.

## Metadata header and compact storage

The file is compact UTF-8 JSON beginning with a `header` object. It records:

- `format: "rxs"`, `format_version: 1`, writer name and software version;
- UTC creation/save timestamps, storage mode and `path_base: "project_directory"`;
- source paths, byte counts, SHA-256 hashes and available modification times;
- original leading comment lines, including acquisition metadata already present.

Comment previews are bounded to the first 32 KiB of each source, with
`header_truncated` when a longer comment header is cut off. Embedded payloads
preserve the entire file. Linked projects record metadata for active, overridden,
joint and historical inputs; the folder reference represents the remaining catalog
without duplicating a potentially huge filename list.

The `embedded` object maps SHA-256 hashes to base64-encoded gzip payloads. Header
entries map original references to safe internal archive paths. Limits are
512 MiB per project file and 1 GiB of expanded inputs. Unsafe archive paths,
missing payloads and integrity failures are errors.

Redundant defaults and formatting whitespace are omitted. Numbers are never rounded
or quantized; double-precision values round-trip exactly. Captions, expressions,
arrays and top-level extension metadata remain intact. Before writing, rexafs
reconstructs the compact document and compares its full serialized state against
the original. A mismatch prevents replacement. Savings depend on content.

## Compatibility from the first release onward

- Application and project-format versions are independent. Optional additions can
  retain format 1 only if their interpretation stays compatible. Existing defaults
  are part of the format contract.
- Loading never rewrites the source. Saved values, bounds, expressions, derived
  arrays and assignments must survive load/save/reopen. Unknown top-level metadata
  is retained; undocumented nested extension fields are outside this contract.
- Future formats are rejected with an actionable error; failed loading leaves the
  session intact. Saving over a future-format file is refused. Missing versions,
  missing headers and malformed input are errors.
- Incompatible changes require a new format number, explicit migration from every
  previously released format, retained fixtures and tests. Never silently change
  the meaning of a field, enum value or default.

Backward compatibility means **new releases read projects from earlier released
versions**. An older executable cannot be guaranteed to retain later features
when saving a newer file. Keep the original/backup when switching versions.

## Safe saving and recovery

The complete project is prepared and validated before the destination changes.
A temporary file in the same directory is written, flushed and renamed into place.
Failed writes, syncs and renames leave the previous project intact. Replacing a
project first retains its exact previous bytes in `project.rxs.bak`; only the
immediately preceding save is retained.

To recover, copy `project.rxs.bak` to `recovered.rxs` and open the copy. This
backup is on the same disk; keep separate archival copies of valuable experiments.

## Every-release regression record

The [fixture collection](../crates/rexafs-gui/tests/fixtures/projects/README.md)
starts with release 0.1.0. Each release must add small representative **linked and
embedded** projects, checksums and release entries. Keep every previously released
sample. Tests discover the full manifest.

```bash
python scripts/check-compatibility-fixtures.py
cargo test --locked --release -p rexafs-gui --no-default-features --features refeff-runner
```

Tests cover fixture load/save/reopen, relocation, Save As, original-byte recovery
without source files, repeat embedded saves, FEFF preservation, identical processing
results, metadata, backups and injected failures. Negative tests cover future
formats, corrupt payloads and unsafe paths. Exact-float tests include signed zero,
subnormals, extremes and deterministic finite double samples.
A retained defaults snapshot also catches changes to the meaning of omitted
format-1 fields; a load/save round trip alone would not detect those changes.

GitHub `release-build.yml` requires a fixture pair for the coordinated version,
verifies retained hashes and runs desktop tests on each release platform. Publication,
credential and stale FEFF job tests remain in that suite. Rust numerical references
and installed Python/JavaScript API tests are additional gates. Add a small reproducer
when format or scientific behavior changes; do not replace old references simply
to accept changed results.
