# xraytsubaki: Fast XAS Data Analysis Tool

xraytsubaki is a Rust-based program that implements the core functionalities of [xraylarch](https://xraypy.github.io/xraylarch/). The primary aim of this project is to expedite the processing of extensive XAS data sets. The project's name, xraytsubaki, draws inspiration from [tsubaki](https://en.wikipedia.org/wiki/Camellia_japonica).

Currently the main source code is placed under `./crates/xraytsubaki/`.

## Project Genesis and Objectives

The inception of this project was triggered when I needed to process over 1000 spectra from in-situ measurements. The data loading and processing in xraylarch were too time-consuming, not to mention also for demeter. The goal was to develop a tool capable of processing data within a reasonable timeframe. While this project does not seek to replace xraylarch, it does aim to provide a phenomenally fast core API for xraylarch's backend to augment its capacity.

Additionally, this project seeks to leverage Rust's ecosystem to create a generalized library compatible with other languages such as Python and Javascript. This will facilitate a shift away from exclusive Python-based analysis. Essentially, this library can be integrated into native GUI applications using modern frameworks like [tauri](https://tauri.studio/en/).

## Key Features

- [x] Standard EXAFS analysis (find_e0, preedge postedge normalization, AUTOBK, FFT, IFFT)
- [x] Parallel processing using Rayon. (For example, M1 Macbook Pro with 10 cores can process 10000 spectra in 7.5 seconds, which is ~x10 enhancement without parallelization. Numpy + xraylarch takes 145 seconds.)
- [x] Optimization on AUTOBK. The AUTOBK process were optimized with providing an analytical Jacobian to speed up the minimization process by Leverberg-Marquardt algorithm.

## Future Developments

- [ ] EXAFS helper funtions (rebinning and more)
- [ ] Develop a Python wrapper for the library. (TODO: py-xraytsubaki)
- [ ] Create a GUI application using Dioxus. (TODO: xraytsubaki-gui)
- [ ] Develop a web assembly version of the library for web application usage.

## Licensing

Licensed under either of

- Apache License, Version 2.0
  ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license
  ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Plotting Support

`xraytsubaki` now supports feature-gated core plotting via `ruviz`.

```bash
cargo run -p xraytsubaki --features plotting --example plot_demo
```

Plotting APIs use a mutable entrypoint (`plot(&mut self)`) so missing intermediates can be auto-computed and cached (`normalize`, `calc_background`, `fft`) when panel selection requires them.

Current defaults:
- `mu()` renders flattened `mu(E)` (auto-normalized).
- `k()` applies symmetric y-limits and k-weight-aware units in the y-axis label.
- `FeffFitResult::plot().k()` uses the fit/dataset `kweight` by default.
- `r()` uses `xlim(0.0, 6.0)` by default.
- Plot text is rendered with `typst(true)` for scientific notation-friendly output.
- `r()` plots magnitude by default; use `.real()` / `.imag()` for components, and `.mag().real().imag()` for all traces.
- Window overlays are off by default.
- `window(true)` on `k()` is an alias that enables both `window_fn(true)` and `window_box(true)`.
- `window_fn(...)` is k-space only; `window_box(...)` supports k-space and fit r-space and draws two range lines (min/max).

For this phase, multi-panel export is PNG-focused.
