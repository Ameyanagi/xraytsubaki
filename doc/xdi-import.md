# XDI import

Implemented against the XraySpectroscopy working group's [XDI 1.0 draft
specification](https://github.com/XraySpectroscopy/XAS-Data-Interchange/blob/master/specification/spec.md)
and [metadata dictionary](https://github.com/XraySpectroscopy/XAS-Data-Interchange/blob/master/specification/dictionary.md),
reviewed on 2026-09-05. This is XAS Data Interchange, not the unrelated OASIS XDI format.

The desktop app discovers `.xdi` and `.XDI` files in folder scans. A file with
the `# XDI/1.x` signature is also recognized when its extension is `.dat` or
another supported text extension. Files named `.xdi` without the signature
produce an import error. The normal import, normalization, background removal,
fit, project reopen, and batch workflows share this reader.

The Import panel shows the XDI version, sample, absorber, edge and axis units.
Expanding it shows measurement details, user comments, original numeric values
and the assigned columns. The preview keeps the file's original units; the
processing pipeline always receives energy in eV. Scan.edge_energy remains
metadata and does not overwrite the user's E0 processing setting.

## Interpretation

- `Column.N` declarations determine column names and units. The optional label
  line is not required; conflicting labels and inconsistent row widths are errors.
- Field names are case insensitive, duplicate fields use their last value, and
  unknown extension fields are retained. User comments are kept separately from
  metadata, including blank lines and interior whitespace. Application/version
  tokens keep their order. LF, CRLF and CR line endings and a UTF-8 BOM are accepted.
- Energy in eV or keV is converted to eV. Angle in degrees or radians uses
  first-order Bragg diffraction with `Mono.d_spacing` in Å. Unsupported axes or
  missing conversion information cause an error rather than an assumed scale.
- Auto import prefers precomputed sample μ (`mutrans`, `mufluor`, `normtrans`,
  `normfluor`), then sample intensities, then reference signals. Transmission uses
  ln(I0/itrans); fluorescence uses ifluor/I0. Reference mode accepts `murefer` /
  `normrefer`, or computes ln(itrans/irefer). Explicit GUI column/mode assignments
  remain available. Precomputed μ is never logarithmically transformed again.
- Invalid or non-finite numeric tokens and damaged rows report errors; they are
  not silently skipped. Missing absorber/edge descriptions and malformed metadata
  fields produce warnings. The reader is not a full metadata-dictionary validator.

Pixel and motor-step energy calibrations, χ(k)/χ(R) import into the μ(E)
processing pipeline, and XDI export are not implemented. The low-level reader can
retain those tables, but conversion to an energy spectrum rejects unsupported axes.

## Rust API

```rust,no_run
use xraytsubaki::prelude::{XdiFile, XdiSignal};

let file = XdiFile::read("ni_metal_rt.xdi")?;
let spectrum = file.to_spectrum(XdiSignal::Auto)?;
let reference = file.header.get("sample.prep");
// file.header retains metadata, comments, column units and import warnings.
// file.data retains the numeric table in its original units and order.
# Ok::<(), xraytsubaki::prelude::XdiError>(())
```

`XdiFile::parse` accepts text directly. `XdiFile::energy_ev` exposes the converted
axis. `XdiSignal::{Transmission, Fluorescence, Reference}` explicitly select the
signal for the core convenience conversion. `to_spectrum` sorts energy and μ
together using the existing spectrum API; it does not modify the original XdiFile.
Metadata remains on XdiFile rather than being embedded in XASSpectrum.

Tests include the unmodified [Ni metal foil XDI from XrayLarch's pinned
fixture revision](https://github.com/xraypy/xraylarch/blob/d8678dd666fd95839fe9dc71b4dbe8bedec278ff/examples/xafsdata/ni_metal_rt.xdi),
plus declarations without labels, signal math, unit conversion, angle conversion,
comment preservation, duplicate fields, malformed rows and folder discovery.
