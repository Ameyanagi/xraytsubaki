# rexafs for JavaScript and TypeScript

Rust-powered X-ray absorption processing through WebAssembly. Developed under the
codename xraytsubaki. npm publication is pending; the package can be built and
installed from a local tarball now.

## API

```javascript
import init, { process } from "rexafs";
await init();
const result = process(energy, mu); // both Float64Array, energy in eV
// const result = process(energy, mu, { e0: 7112 });
console.log(result.e0, result.k, result.chi);
```

`process` runs the same default normalization → AUTOBK → Fourier pipeline as the
Rust and Python APIs. Inputs must be finite, equal-length arrays with strictly
increasing energy. Results contain `e0`, `k`, `chi`, `r`, `chir_mag`, `chir_re` and
`chir_im`. `k` is in Å⁻¹; `r` is in Å and is not phase corrected. χ(k) is unweighted;
the Fourier transform uses k-weight 2. Result arrays are owned Float64Arrays,
independent of Wasm memory. Invalid arguments and calculation failures throw errors.

Node 22+ automatically loads the local Wasm file; `init()` provides the same
calling pattern as the browser. Browser consumers must await `init()` first.
Export conditions select the appropriate entry point; `rexafs/browser` and
`rexafs/node` select one explicitly. `init(wasmBytes)` or `init(wasmUrl)` supplies
an explicit browser Wasm source when the bundler cannot serve the relative asset.
Serve Wasm with `application/wasm`; the generated glue handles loading.

Processing is synchronous after initialization. Use a Web Worker for long browser
calculations. v0.1 uses serial Wasm and exposes the array pipeline only: no native
filesystem access, FEFF subprocesses, structure downloads or fitting API.

## Build and verify

Install `wasm-pack` and ensure it is on PATH, then run from the repository root:

```bash
rustup target add wasm32-unknown-unknown
npm --prefix js-rexafs run build
npm --prefix js-rexafs test
cd js-rexafs
npm pack
```

The build creates separate browser and Node glue and copies both license notices.
The tarball has no install-time compiler requirement. `npm install rexafs` becomes
the public install command after release.

Licensed under MIT OR Apache-2.0.
