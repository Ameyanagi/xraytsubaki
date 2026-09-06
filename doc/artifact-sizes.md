# Existing artifact sizes

Measured from local v0.1.0 artifacts before the spectrum API changes. Sizes use
MiB (1,048,576 bytes). These are package contents, excluding Python/NumPy,
system libraries, and compiler/build caches.

| Artifact | Compressed download | Expanded contents |
|---|---:|---:|
| macOS Apple Silicon desktop distribution | 24.18 MiB | 50.43 MiB |
| CPython 3.14 Apple Silicon wheel | 1.23 MiB | 3.44 MiB |
| JavaScript/TypeScript npm package | 0.46 MiB | 1.39 MiB |
| Rust source `.crate` archive | 2.70 MiB | 15.47 MiB |

The desktop `.app` alone is **44.34 MiB** (46,494,026 bytes); its executable is
**41.49 MiB** (43,507,824 bytes). The complete extracted distribution is
50.43 MiB including notices and metadata. The ZIP's raw expanded total is slightly
larger because it contains macOS resource metadata.

This archive includes **ReFEFF 0.2.2 only** (`refeff-runner`), not FEFFRS/FEFF10.
Its build manifest records source revision
`40f3a941e4709e9e6ae661230385ef82b2500ba7` with a dirty working tree. Current desktop
Cargo defaults enable both `refeff-runner` and `feff10-runner`; a new default build
is not represented by these archive measurements.

The Rust crate is a source archive, not an installed binary. Its contribution to
a consuming executable depends on features and linker optimization.

## Saved project files

`.rxs` files are project data, not executable binaries. Current fixtures illustrate
how source links and embedded raw data affect size; they are not universal limits.
The gzip column is a measured optional transport compression, not the on-disk format.

| Fixture | Saved `.rxs` | Gzip compressed |
|---|---:|---:|
| Minimal | 238 B | 164 B |
| Linked sources | 5,240 B | 1,908 B |
| Embedded sources | 15,738 B | 9,921 B |

The format is JSON, with compressed/deduplicated raw payloads when embedding is
selected. Actual project size depends on spectrum count, fit history, and embedded
source data; there is no separate installed size for a project document.
