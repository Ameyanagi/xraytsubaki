# pyxraytsubaki

Python bindings for XRayTsubaki, a Rust library for X-ray Absorption Spectroscopy analysis.

## Features

- XAS data processing including normalization, background removal, and Fourier transforms
- EXAFS fitting with Levenberg-Marquardt optimization
- Multi-spectrum fitting with parameter constraints
- Fast performance thanks to Rust implementation
- Compatible interface with xraylarch

## Installation

```bash
uv add pyxraytsubaki
```

## Example Usage

```python
import numpy as np
import pyxraytsubaki as xt

# Load data
energy = np.linspace(17000, 18000, 1000)
mu = np.sin(energy / 1000) + np.random.normal(0, 0.01, 1000)

# Create a spectrum
spectrum = xt.XASSpectrum(energy=energy, mu=mu)

# Normalize
spectrum.normalize(pre1=-200, pre2=-30, norm1=100, norm2=600)

# Apply background removal
spectrum.autobk(rbkg=1.0, kmin=0, kmax=15)

# Perform Fourier transform
spectrum.xftf(kmin=2, kmax=12, dk=1, window='hanning', kweight=2)

# Or use the fluent API for method chaining
spectrum = (
    xt.XASSpectrum(energy=energy, mu=mu)
    .normalize()
    .pre_range(-200, -30)
    .norm_range(100, 600)
    .autobk()
    .rbkg(1.0)
    .k_range(0, 15)
    .xftf()
    .k_range(2, 12)
    .kweight(2)
    .window('hanning')
    .run()
)

# Save to file
spectrum.save("spectrum.json")
```

## License

This project is licensed under either of:

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.