# rexafs for JavaScript and TypeScript

Rust-powered X-ray absorption processing through WebAssembly, with Rust's
`Spectrum` methods. npm publication is pending; build and install a local tarball.

```typescript
import init, { Spectrum, AUTOBK, BackgroundMethod } from "rexafs";
await init();
const spectrum = Spectrum.from_arrays(energy, mu).fft(); // Float64Array inputs
console.log(spectrum.e0(), spectrum.chi(), spectrum.chir_mag());

const background = new AUTOBK();
background.rbkg = 1.2;
const method = BackgroundMethod.AUTOBK(background);
spectrum.set_background_method(method).fft();
background.free(); method.free(); spectrum.free();
```

`normalize()`, `calc_background()`, `fft()` and `ifft()` return the same spectrum.
Missing prerequisites run in Rust using the selected methods and their defaults.
`set_e0(value)` overrides E0. Configure normalization with `PrePostEdge` and
`NormalizationMethod`; configure Fourier parameters with `XrayFFTF` and
`set_fft`. Scalar field names match Rust. Window and solver fields use Rust
variant names such as `Hanning`, `KaiserBessel` and `LinearDirect`.

Inputs must be finite, equal-length Float64Arrays with strictly increasing energy
in eV. Array getters return independent Float64Arrays, or `undefined` before a
stage has produced the result. Errors are thrown for invalid inputs or failed
stages. There is no public `process()` function. See the [shared API guide](../doc/api.md)
for getters, parameter invalidation and units. MBack and ILPBkg remain
unimplemented and return errors rather than falling back to defaults.

Node 22+ loads its packaged Wasm automatically; `init()` is a no-op there.
Browsers must await `init()` before constructing spectra. `rexafs/browser` and
`rexafs/node` select a runtime explicitly. `init(wasmBytes)` or `init(wasmUrl)`
supplies an explicit browser asset; otherwise the relative packaged asset is
loaded. Processing is synchronous after initialization; use a Web Worker for
long calculations. Call `free()` when done with spectra and configuration objects.

## Build and verify

```bash
rustup target add wasm32-unknown-unknown
npm --prefix js-rexafs run build  # requires wasm-pack
npm --prefix js-rexafs test
cd js-rexafs
npm pack
```

The package includes Node and browser Wasm/glue and TypeScript declarations.
There is no native filesystem, FEFF subprocess, structure-download or fitting API
in this binding. Licensed under MIT OR Apache-2.0.
