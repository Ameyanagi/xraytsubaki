# py-xraytsubaki

Python bindings for the xraytsubaki library, a Rust implementation of X-ray absorption spectroscopy (XAS) analysis tools. 

## Features

- Full XAS processing pipeline: normalization, background removal, Fourier transforms
- EXAFS fitting with single and multiple path models
- Multi-spectrum fitting with parameter constraints
- Same interface as xraylarch but with better performance and cleaner API
- Both functional and object-oriented interfaces
- Fluent API for method chaining

## Quick Start

```python
import numpy as np
import py_xraytsubaki as xt

# Load or create data
energy = np.linspace(17000, 18000, 1000)
mu = ... # your absorption data

# Functional API
e0 = xt.find_e0(energy, mu)
result = xt.pre_edge(energy, mu, e0=e0, pre1=-200, pre2=-30, norm1=100, norm2=600)

# Get outputs
norm = result['norm']  # normalized spectrum
edge_step = result['edge_step']

# Object-oriented API
spectrum = xt.XASSpectrum(energy=energy, mu=mu)
spectrum.normalize(pre1=-200, pre2=-30, norm1=100, norm2=600)
spectrum.autobk(rbkg=1.0, kmin=0, kmax=15)
spectrum.xftf(kmin=2, kmax=12, dk=1, window='hanning', kweight=2)

# Access properties
print(f"E0 = {spectrum.e0:.1f} eV")
print(f"Edge step = {spectrum.edge_step:.4f}")

# Fluent API
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

# EXAFS Fitting
# Create parameters
params = xt.FittingParameters()
params.add("amp", 0.8, vary=True, min=0, max=2.0)
params.add("r", 2.5, vary=True, min=1.0, max=5.0)
params.add("phase", 0.0, vary=True, min=-np.pi, max=np.pi)
params.add("sigma2", 0.004, vary=True, min=0.0, max=0.05)

# Create path
path = xt.SimplePath(
    amp_param="amp",
    r_param="r",
    phase_param="phase",
    sigma2_param="sigma2"
)

# Create dataset
dataset = xt.FittingDataset(k=spectrum.k, chi=spectrum.chi)
dataset.add_path(path)
dataset.kweight(2.0)
dataset.k_range(2.0, 12.0)
dataset.window("hanning")

# Fit
fitter = xt.ExafsFitter(dataset=dataset, params=params)
result = fitter.fit()

print(result)
print(f"Amplitude = {result.params['amp'].value:.4f} ± {result.params['amp'].stderr:.4f}")
print(f"Distance = {result.params['r'].value:.4f} ± {result.params['r'].stderr:.4f} Å")

# Multi-spectrum fitting with constraints
# Create parameters with constraints
params = xt.ConstrainedParameters()
params.add("amp_1", 0.8, vary=True, min=0.0, max=2.0)
params.add("sigma2_1", 0.004, vary=True, min=0.0, max=0.05)
params.add("r", 2.5, vary=True, min=1.0, max=5.0)
params.add("phase", 0.0, vary=True, min=-np.pi, max=np.pi)

# Add constraint parameters
params.add("amp_scale_2", 0.94, vary=True, min=0.5, max=1.5)
params.add("amp_scale_3", 0.88, vary=True, min=0.5, max=1.5)
params.add("delta_sigma2_2", 0.0005, vary=True, min=0.0, max=0.01)
params.add("delta_sigma2_3", 0.001, vary=True, min=0.0, max=0.01)

# Add constrained parameters
params.add("amp_2", 0.0, constraint=xt.ParameterConstraint.scale("amp_1", "amp_scale_2"))
params.add("amp_3", 0.0, constraint=xt.ParameterConstraint.scale("amp_1", "amp_scale_3"))
params.add("sigma2_2", 0.0, constraint=xt.ParameterConstraint.offset("sigma2_1", "delta_sigma2_2"))
params.add("sigma2_3", 0.0, constraint=xt.ParameterConstraint.offset("sigma2_1", "delta_sigma2_3"))

# Apply constraints
params.apply_constraints()

# Create multi-spectrum dataset and fit
multi_dataset = xt.MultiSpectrumDataset()
multi_dataset.add_dataset(dataset1)
multi_dataset.add_dataset(dataset2)
multi_dataset.add_dataset(dataset3)
multi_dataset.params(params)

fitter = xt.MultiSpectrumFitter(dataset=multi_dataset)
result = fitter.fit()

print(result)
```

## Installation

```bash
uv add xraytsubaki
```

## Requirements

- Python 3.7+
- NumPy
- Matplotlib (optional, for plotting)

## Documentation

For detailed documentation, see [XRayTsubaki Documentation](https://github.com/fujunustc/xraytsubaki).