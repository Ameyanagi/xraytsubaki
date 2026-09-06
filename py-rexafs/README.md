# rexafs for Python

Rust-powered X-ray absorption analysis with NumPy inputs and results. Developed
under the codename xraytsubaki. This directory builds the PyPI distribution
`rexafs`; its private native extension is `rexafs._core`. Python build configuration
lives in the repository-root `pyproject.toml` so source archives include the
workspace's local dependency patches.

Registry publication is pending. Build from the repository root with CPython 3.10–3.14:

```bash
uv venv --python 3.12
uv pip install maturin numpy
uv run --no-project maturin develop --release
```

After publication the install command will be `pip install rexafs`.

## Process one spectrum

```python
import numpy as np
import rexafs

data = np.loadtxt("spectrum.dat") # columns: energy in eV, absorption mu
result = rexafs.process(data[:, 0], data[:, 1])
# Optional edge override: rexafs.process(energy, mu, e0=7112.0)
print(result.e0)
```

`process(energy, mu, *, e0=None) -> ProcessedSpectrum` runs normalization, AUTOBK
and the forward transform. Inputs must be one-dimensional, finite and equal in
length, with strictly increasing energy in eV. Lists, NumPy arrays and strided
views are accepted and converted to float64. Inputs are copied before the native
calculation releases the GIL; results own their memory.

| Attribute | Meaning |
|---|---|
| `e0` | Resolved edge energy, eV |
| `k`, `chi` | Wave number in Å⁻¹ and unweighted χ(k) |
| `r` | Transform distance in Å, not phase corrected |
| `chir_mag`, `chir_re`, `chir_im` | Magnitude, real and imaginary components on `r` |

The transform uses the core defaults, including k-weight 2. `ProcessedSpectrum`
is a dataclass; array attributes are NumPy float64 arrays. Invalid inputs and
normalization failures raise `ValueError`; other calculation failures raise
`RuntimeError`. Failed calculations never return a partial result.

## Process QAS transmission files

```python
batch = rexafs.process_qas_batch(["scan01.dat", "scan02.dat"])
print(batch.processed_count)
for error in batch.errors:
    print(error.index, error.category, error.message)
```

Each file is processed independently. `processed_count` counts successful full
pipelines; failures refer to original input indices. Categories are `io`, `data`,
`normalization`, `background`, `fft`, `math`, `fitting` and `group`.

The old `run_pipeline_arrays` dictionary and `run_batch_qas_trans` tuple entry
points remain in the new module. The batch count now means successful processing,
not the number of loaded files. `import xraytsubaki` is not installed as an alias.

Fitting, structures, plotting and direct ReFEFF calculation remain Rust/desktop
APIs; this binding does not claim full parity. See the repository's
[API guide](https://github.com/ameyanagi/xraytsubaki/blob/main/doc/api.md).

Licensed under MIT OR Apache-2.0. See the included license files.
