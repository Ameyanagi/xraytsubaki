# rexafs for Python

Rust-powered X-ray absorption analysis with the same spectrum methods as Rust.
The private native extension is `rexafs._core`.

```python
import numpy as np
from rexafs import Spectrum, AUTOBK, BackgroundMethod

data = np.loadtxt("spectrum.dat")  # energy (eV), absorption mu
spectrum = Spectrum.from_arrays(data[:, 0], data[:, 1]).fft()
print(spectrum.e0(), spectrum.chi(), spectrum.chir_mag())

background = AUTOBK()
background.rbkg = 1.2
spectrum.set_background_method(BackgroundMethod.AUTOBK(background)).fft()
```

`normalize()`, `calc_background()`, `fft()` and `ifft()` execute the selected
stage and return the same spectrum. Missing prerequisite stages run with their
configured parameters or defaults. `set_e0(value)` overrides the edge energy.
Use `PrePostEdge`/`NormalizationMethod` to configure normalization and `XrayFFTF`
with `set_fft` to configure the forward transform. Parameter names match Rust;
window and solver fields accept Rust variant names as strings.

Input lists, NumPy arrays and strided views are converted to float64 and checked
for finite values, equal lengths and strictly increasing energy. Result getters
return owned NumPy arrays, or `None` before their stage runs. Inputs and returned
arrays do not share mutable storage with the spectrum. Input/normalization
errors raise `ValueError`; other stage failures raise `RuntimeError`. Stages
release the GIL while Rust computes.

```python
from rexafs import io
spectra = [io.read_qas_transmission(path).fft() for path in ["scan01.dat", "scan02.dat"]]
```

The old `process`, `ProcessedSpectrum`, and free batch/pipeline wrappers have
been removed. See the [shared API guide](../doc/api.md) for configuration,
invalidation, algorithm selection and migration details. MBack and ILPBkg are
still unimplemented placeholders; selecting them raises an error.

## Build

Registry publication is pending. Build from the repository root with CPython
3.10–3.14:

```bash
uv venv --python 3.14
uv pip install maturin numpy
uv run --no-project maturin develop --release
```

Fitting, structures, plotting and direct ReFEFF calculation remain Rust/desktop
APIs. Licensed under MIT OR Apache-2.0.

New AUTOBK objects use a single linear solve with `clamp_lambda = 0.001`.
Set `clamp_lambda` to `0` to disable the endpoint penalty. The low-end weight
is `clamp_lo = 0`, the high-end weight is `clamp_hi = 1`, and `nclamp = 3`.
The `FixedPenalty` model does not add a separate ridge penalty on coefficients.
See the [full definition](../doc/autobk-fixed-penalty.md).
