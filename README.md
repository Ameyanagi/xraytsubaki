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
