# XRayTsubaki TypeScript API Usage Guide

This guide provides examples and documentation for using the XRayTsubaki TypeScript bindings.

## Installation

```bash
npm install ts-xraytsubaki
```

The package requires the XRayTsubaki Rust library to be built and available on your system.

## Basic Usage

### Working with XAS Spectra

```typescript
import { XASSpectrum } from 'ts-xraytsubaki';

// Create a new spectrum
const spectrum = new XASSpectrum('my-spectrum');

// Load data from file
const loadedSpectrum = XASSpectrum.fromFile('/path/to/spectrum.dat');

// Process data
loadedSpectrum.findE0();
console.log(`Edge energy (E0): ${loadedSpectrum.e0} eV`);

// Normalize the spectrum
loadedSpectrum.normalize();

// Calculate background and extract EXAFS
loadedSpectrum.calcBackground();

// Get the k and chi arrays
const k = loadedSpectrum.k;
const chi = loadedSpectrum.chi;
console.log(`k range: ${Math.min(...k!)} to ${Math.max(...k!)} Å⁻¹`);

// Perform Fourier transform
loadedSpectrum.fft();

// Get the R and chi(R) arrays
const r = loadedSpectrum.r;
const chiRMag = loadedSpectrum.chiRMag;
console.log(`R range: ${Math.min(...r!)} to ${Math.max(...r!)} Å`);

// Perform inverse Fourier transform
loadedSpectrum.ifft();

// Get the q and chi(q) arrays
const q = loadedSpectrum.q;
const chiQ = loadedSpectrum.chiQ;
```

### Working with Multiple Spectra (XASGroup)

```typescript
import { XASSpectrum, XASGroup } from 'ts-xraytsubaki';

// Create a new group
const group = new XASGroup();

// Load multiple spectra
const spectrum1 = XASSpectrum.fromFile('/path/to/spectrum1.dat');
const spectrum2 = XASSpectrum.fromFile('/path/to/spectrum2.dat');
spectrum1.name = 'sample-1';
spectrum2.name = 'sample-2';

// Add spectra to the group
group.addSpectrum(spectrum1);
group.addSpectrum(spectrum2);

// Process all spectra in the group at once
group.findE0();
group.normalize();
group.calcBackground();
group.fft();

// Access individual spectra
console.log(`Group contains ${group.length()} spectra`);
const firstSpectrum = group.getSpectrum(0);
console.log(`First spectrum name: ${firstSpectrum?.name}`);

// Save the group to a JSON file
group.saveJSON('/path/to/output/group.json');

// Load a group from a JSON file
const loadedGroup = XASGroup.fromJSON('/path/to/group.json');
```

### Using Standalone XAFS Functions

```typescript
import { findE0, preEdge, autobk, xftf, xftr } from 'ts-xraytsubaki';

// Assuming you have energy and mu data arrays

// Find edge energy
const e0 = findE0(energy, mu);
console.log(`Found E0: ${e0} eV`);

// Perform pre-edge normalization
const preEdgeResult = preEdge(energy, mu, {
  e0,
  preEdgeRange: [-150, -30],
  postEdgeRange: [100, 400]
});

// Extract normalized spectrum and edge step
const { norm, edge_step } = preEdgeResult;
console.log(`Edge step: ${edge_step}`);

// Perform background subtraction to get chi(k)
const bkgResult = autobk(energy, norm, {
  e0,
  rbkg: 1.0,
  kweight: 2,
  kmin: 0,
  kmax: 15
});

// Extract k and chi arrays
const { k, chi } = bkgResult;

// Perform forward Fourier transform
const ftResult = xftf(k, chi, {
  kmin: 2,
  kmax: 12,
  dk: 2,
  window: 'Hanning',
  kweight: 2
});

// Extract R and chi(R) arrays
const { r, chir_mag, chir_re, chir_im } = ftResult;

// Perform inverse Fourier transform
const ifftResult = xftr(r, { re: chir_re, im: chir_im }, {
  rmin: 1,
  rmax: 3,
  dr: 0.1,
  window: 'Hanning'
});

// Extract q and chi(q) arrays
const { q, chiq } = ifftResult;
```

## EXAFS Fitting

### Simple Path Fitting

```typescript
import {
  FittingParameter,
  FittingParameters,
  SimplePath,
  FittingDataset,
  ExafsFitter
} from 'ts-xraytsubaki';

// Assuming you have k and chi arrays

// Create a dataset
const dataset = new FittingDataset(k, chi);
dataset.setKRange(2.0, 12.0);
dataset.setKWeight(2);

// Create parameters
const params = new FittingParameters();
params.add('amp', 0.9, 0.5, 1.5);
params.add('e0', 0.0, -10.0, 10.0);
params.add('sigma2', 0.003, 0.001, 0.01);
params.add('delr', 0.0, -0.2, 0.2);

// Create a path
const path = new SimplePath('path1', 6, 2.5);
path.setS02('amp');
path.setE0('e0');
path.setSigma2('sigma2');
path.setDelr('delr');

// Create fitter and add path
const fitter = new ExafsFitter();
fitter.addPath(path);

// Perform the fit
const result = fitter.fit(dataset, params);

// Check results
console.log(`Fit successful: ${result.success}`);
console.log(`Message: ${result.message}`);
console.log(`Reduced chi-square: ${result.redchi}`);
console.log(`Number of function evaluations: ${result.nfev}`);

// Get fitted parameters
console.log(`Fitted S0²: ${result.params.get('amp')?.value}`);
console.log(`Fitted E0: ${result.params.get('e0')?.value}`);
console.log(`Fitted σ²: ${result.params.get('sigma2')?.value}`);
console.log(`Fitted ΔR: ${result.params.get('delr')?.value}`);

// The best-fit chi(k) values
const best_fit = result.best_fit;
```

### Multi-Spectrum Fitting

```typescript
import {
  ConstrainedParameter,
  ConstrainedParameters,
  SimplePath,
  FittingDataset,
  MultiSpectrumDataset,
  MultiSpectrumFitter
} from 'ts-xraytsubaki';

// Assuming you have k and chi arrays for multiple spectra

// Create datasets
const dataset1 = new FittingDataset(k1, chi1);
dataset1.setKRange(2.0, 12.0);
dataset1.setKWeight(2);

const dataset2 = new FittingDataset(k2, chi2);
dataset2.setKRange(2.0, 12.0);
dataset2.setKWeight(2);

// Create multi-spectrum dataset
const multiDataset = new MultiSpectrumDataset();
multiDataset.addDataset('spectrum1', dataset1);
multiDataset.addDataset('spectrum2', dataset2);

// Create constrained parameters
const params = new ConstrainedParameters();

// Parameters for spectrum 1
params.add('s02_1', 0.9, 0.7, 1.1);
params.add('e0_1', 0.0, -5.0, 5.0);
params.add('sigma2_1', 0.003, 0.001, 0.01);
params.add('delr_1', 0.0, -0.1, 0.1);

// Parameters for spectrum 2 with constraints
const s02_2 = params.add('s02_2', 0.85);
s02_2.scaleFrom('s02_1', 0.95);  // s02_2 = 0.95 * s02_1

const e0_2 = params.add('e0_2', 1.0);
e0_2.offsetFrom('e0_1', 1.0);    // e0_2 = e0_1 + 1.0

params.add('sigma2_2', 0.004, 0.001, 0.01);
params.add('delr_2', 0.05, -0.1, 0.2);

// Create paths for each spectrum
const path1 = new SimplePath('path1_1', 6, 2.5);
path1.setS02('s02_1');
path1.setE0('e0_1');
path1.setSigma2('sigma2_1');
path1.setDelr('delr_1');

const path2 = new SimplePath('path1_2', 6, 2.6);
path2.setS02('s02_2');
path2.setE0('e0_2');
path2.setSigma2('sigma2_2');
path2.setDelr('delr_2');

// Create fitter and add paths
const fitter = new MultiSpectrumFitter();
fitter.addPath('spectrum1', path1);
fitter.addPath('spectrum2', path2);

// Perform the fit
const result = fitter.fit(multiDataset, params);

// Check results
console.log(`Fit successful: ${result.success}`);
console.log(`Message: ${result.message}`);
console.log(`Reduced chi-square: ${result.redchi}`);

// Get fitted parameters
console.log(`Fitted S0² (spectrum 1): ${result.params.get('s02_1')?.getValue()}`);
console.log(`Fitted S0² (spectrum 2): ${result.params.get('s02_2')?.getValue()}`);

// The best-fit chi(k) values for each spectrum
const best_fit1 = result.best_fits['spectrum1'];
const best_fit2 = result.best_fits['spectrum2'];
```

## API Documentation

For full API documentation, refer to the TypeScript type definitions or the source code documentation in the project repository.

## Building from Source

```bash
# Clone the repository
git clone https://github.com/yourusername/ts-xraytsubaki.git
cd ts-xraytsubaki

# Install dependencies
npm install

# Build the project
npm run build

# Run tests
npm test
```