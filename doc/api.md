# rexafs API guide

The public processing entry point is `process(energy, mu)` in Rust, Python and
JavaScript. Each binding calls the Rust core. Advanced processing, fitting,
structure, plotting and analysis APIs remain available in Rust and the desktop;
binding coverage is intentionally listed in the package READMEs.

## Inputs and outputs

Energy is in eV. Energy and absorption must have the same length, contain finite
numbers, and have strictly increasing energy. The checked constructor rejects
duplicates and unsorted input; it never sorts or averages silently. At least two
points are required to construct a spectrum; a full EXAFS calculation requires
sufficient pre-edge, post-edge and k-space coverage and can return a stage error.

`process` finds E0, normalizes, removes the AUTOBK background and Fourier transforms.
An E0 override must be finite and strictly inside the measured energy range.
The default forward transform uses k-weight 2, a Kaiser–Bessel window, nominal
k range 2–15 Å⁻¹, dk 1 Å⁻¹ and FFT length 2048, subject to the core's data-dependent
range handling. Advanced configurations use the existing staged Rust methods.

| Result | Units / relationship |
|---|---|
| `e0` | Edge energy, eV |
| `k`, `chi` | Same length; k in Å⁻¹ and unweighted χ(k) |
| `r` | Fourier distance, Å; not phase corrected |
| `chir_mag`, `chir_re`, `chir_im` | Same length as `r`; magnitude, real and imaginary components |

Result arrays own their memory. Python uses NumPy float64 arrays, JavaScript uses
Float64Array, and Rust uses Vec<f64>. Output never mixes partial success with absent
fields. R-space magnitudes are transform amplitudes, not phase-corrected distances.

## Rust

```rust,ignore
use rexafs::{process, process_with_options, ProcessOptions, Spectrum};

let result = process(&energy, &mu)?;
let explicit = process_with_options(&energy, &mu, ProcessOptions { e0: Some(7112.0) })?;

let mut spectrum = Spectrum::from_arrays(&energy, &mu)?;
spectrum.find_e0()?.normalize()?.calc_background()?.fft()?;
let chi = spectrum.chi(); // borrowed slice, without cloning
```

| Preferred path | Existing path retained |
|---|---|
| `rexafs::Spectrum` | `rexafs::prelude::XASSpectrum` |
| `rexafs::Group` | `rexafs::prelude::XASGroup` |
| `rexafs::Error`, `rexafs::Result` | `rexafs::xafs::XAFSError`, `rexafs::xafs::Result` |
| `rexafs::io::read_qas_transmission` | `rexafs::xafs::io::load_spectrum_QAS_trans` |
| `rexafs::fitting`, `structure`, `analysis`, `tools` | Corresponding modules under `rexafs::xafs` |

`Spectrum::new()` and `set_spectrum` remain legacy construction methods. Prefer
`from_arrays` for caller-supplied data: the legacy setter has historical sorting
behavior and is not the checked input boundary. `k()` and `chi()` borrow buffers;
the old `get_k()` and `get_chi()` continue returning owned clones. Public spectrum
fields and existing solver/configuration types remain accessible for advanced use.

## Python and JavaScript

```python
import rexafs
result = rexafs.process(energy, mu, e0=7112.0)
```

```javascript
import init, { process } from "rexafs";
await init();
const result = process(energy, mu, { e0: 7112.0 });
```

Python accepts 1-D array-like inputs, including strided views. JavaScript requires
Float64Array inputs and rejects unknown options. Browser initialization is explicit;
Node loads its own Wasm asset. JavaScript processing is synchronous; put long jobs
in a worker. Python copies inputs and releases the GIL during the calculation.

Rust returns typed errors. Python raises ValueError for input/normalization errors
and RuntimeError for other processing failures. JavaScript throws Error/TypeError.
The Python QAS batch API returns successful count and indexed failures and continues
past failures; it is a file adapter, not the universal array-processing API.

## Further cleanup

The first release avoids removing established advanced APIs. Subsequent work should
add structured processing options shared across bindings, typed fit builders in
Python/TypeScript and private cache state with explicit invalidation. These are
future extensions, not functions advertised as available today. Preserve numerical
reference tests as the API grows; do not silently change scientific defaults to
make a cross-language interface look uniform.
