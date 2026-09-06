# rexafs spectrum API

Rust, Python and TypeScript use the same mutable `Spectrum` workflow. Construct a
spectrum, then call the stage you need. `fft()` computes missing normalization
and background results using the selected methods and their defaults.

```rust,ignore
let mut spectrum = rexafs::Spectrum::from_arrays(&energy, &mu)?;
spectrum.fft()?;
let chi = spectrum.chi();
```

```python
from rexafs import Spectrum
spectrum = Spectrum.from_arrays(energy, mu).fft()
chi = spectrum.chi()
```

```typescript
import init, { Spectrum } from "rexafs";
await init();
const spectrum = Spectrum.from_arrays(energy, mu).fft();
const chi = spectrum.chi();
spectrum.free();
```

## Stages and methods

| Call | Behavior |
|---|---|
| `find_e0()` | Find E0 from the working spectrum; invalidate dependent results |
| `normalize()` | Run the selected normalization method; resolve E0 if needed |
| `calc_background()` | Run the selected background method; normalize if needed |
| `fft()` | Forward Fourier transform; calculate missing background results first |
| `ifft()` | Reverse transform; calculate the forward transform first if needed |

Each stage executes immediately. Rust returns `Result<&mut Self, Error>`; Python
and TypeScript return the same spectrum and raise/throw on failure. Explicit
chains such as `spectrum.normalize().calc_background().fft()` are also supported
(with `?` between stages in Rust). There is no public `process()` facade or
separate `ProcessedSpectrum` result class.

Generic stage names are independent of the chosen algorithm. The defaults are
pre/post-edge normalization and AUTOBK background removal. Setting another
method never silently falls back to a default: the existing MBack normalization
and ILPBkg background placeholders return not-implemented errors. A future
background algorithm belongs in `BackgroundMethod`; the spectrum workflow and
binding stage calls stay the same. Runtime custom algorithm callbacks are not
part of this API.

## Configuration

The bindings expose the Rust configuration names and fields: `PrePostEdge`,
`AUTOBK`, `XrayFFTF`, `NormalizationMethod`, and `BackgroundMethod`. Configure a
stage separately, then let `fft()` run it when needed.

```rust,ignore
use rexafs::{AUTOBK, BackgroundMethod, XrayFFTF};
let mut background = AUTOBK::new();
background.rbkg = Some(1.2);
spectrum.set_background_method(Some(BackgroundMethod::AUTOBK(background)))?;
let mut transform = XrayFFTF::new();
transform.kweight = Some(3.0);
spectrum.set_fft(transform).fft()?;
```

```python
from rexafs import AUTOBK, BackgroundMethod, XrayFFTF
background = AUTOBK()
background.rbkg = 1.2
spectrum.set_background_method(BackgroundMethod.AUTOBK(background))
transform = XrayFFTF()
transform.kweight = 3
spectrum.set_fft(transform).fft()
```

```typescript
import { AUTOBK, BackgroundMethod, XrayFFTF } from "rexafs";
const background = new AUTOBK();
background.rbkg = 1.2;
const method = BackgroundMethod.AUTOBK(background);
spectrum.set_background_method(method);
const transform = new XrayFFTF();
transform.kweight = 3;
spectrum.set_fft(transform).fft();
background.free(); method.free(); transform.free();
```

Normalization uses `set_normalization_method` with
`NormalizationMethod.PrePostEdge(parameters)` in the bindings, or
`Some(NormalizationMethod::PrePostEdge(parameters))` in Rust. Use `set_e0(value)`
for an explicit edge energy. Unset scalar parameters use Rust defaults. Window
and solver fields in the bindings use Rust variant names such as `Hanning`,
`KaiserBessel`, and `LinearDirect`. The bindings expose scalar configuration;
advanced AUTOBK standard-spectrum arrays remain available in Rust.

Setters copy configuration objects. Editing a configuration afterward takes
effect when it is passed to the setter again. Changing normalization invalidates
background and both transforms; changing background invalidates both transforms;
changing forward-transform parameters invalidates forward/reverse results.
Unchanged prerequisite results are reused. Calling a stage explicitly recomputes
that stage. Replacing spectrum data clears the old E0 and derived results.

Rust's legacy public fields remain accessible. After modifying input data or
stage parameters directly, call `invalidate_derived()` before requesting another
stage; prefer setters to invalidate automatically.

## Inputs, results and errors

Energy is in eV. `from_arrays` requires finite, equal-length, one-dimensional
arrays with strictly increasing energy. Python accepts NumPy arrays, lists and
strided views. JavaScript requires `Float64Array`. Rust accepts `&[f64]`.
At least two points are needed to construct a spectrum; calculation stages also
require sufficient pre-edge, post-edge and k-space coverage.

| Getter | Result |
|---|---|
| `e0()` | Resolved edge energy, eV |
| `norm()`, `flat()` | Normalized and flattened absorption |
| `pre_edge()`, `post_edge()` | Fitted normalization curves |
| `k()`, `chi()` | Wave number (Å⁻¹) and unweighted χ(k) |
| `r()` | Fourier distance (Å, not phase corrected) |
| `chir_mag()`, `chir_real()`, `chir_imag()` | Components on the R grid |
| `q()`, `chiq()` | Reverse-transform grid and result |

Before its stage succeeds, a result is `None` in Rust/Python and `undefined` in
JavaScript. Python getters return independent NumPy float64 arrays; JavaScript
getters return independent Float64Arrays. Rust's `k()` and `chi()` borrow buffers;
use `.to_vec()` when an owned copy is needed. Its other array getters in the table
return owned values. The bindings' `k()` and `chi()` return copies too.

Default forward-transform settings remain k-weight 2, Kaiser–Bessel window,
nominal k range 2–15 Å⁻¹, dk 1 Å⁻¹ and length 2048, subject to data-dependent range
handling. Algorithm defaults and numerical implementations are shared with Rust.

Rust returns typed errors. Python raises `ValueError` for input/normalization
errors and `RuntimeError` for other stage failures. JavaScript throws errors.
Python owns its inputs and releases the GIL during stages. Browser consumers
must `await init()` before constructing spectra; Node loads its packaged Wasm
asset automatically. Processing is synchronous after initialization. Use a Web
Worker for long browser calculations and `free()` to release Wasm objects.

Python's `rexafs.io.read_qas_transmission(path)` returns a spectrum, matching
Rust's reader. For multiple files, load spectra and call their stage methods.
Fitting, structures, plotting and other advanced modules remain Rust/desktop APIs.
