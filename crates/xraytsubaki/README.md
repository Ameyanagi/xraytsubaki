# xraytsubaki: Fast XAS Data Analysis Tool

xraytsubaki is a Rust-based program that implements the core functionalities of [xraylarch](https://xraypy.github.io/xraylarch/). The primary aim of this project is to expedite the processing of extensive XAS data sets. The project's name, xraytsubaki, draws inspiration from [tsubaki](https://en.wikipedia.org/wiki/Camellia_japonica).

## Project Genesis and Objectives

The inception of this project was triggered when I needed to process over 1000 spectra from in-situ measurements. The data loading and processing in xraylarch were too time-consuming, not to mention also for demeter. The goal was to develop a tool capable of processing data within a reasonable timeframe. While this project does not seek to replace xraylarch, it does aim to provide a phenomenally fast core API for xraylarch's backend to augment its capacity.

Additionally, this project seeks to leverage Rust's ecosystem to create a generalized library compatible with other languages such as Python and Javascript. This will facilitate a shift away from exclusive Python-based analysis. Essentially, this library can be integrated into native GUI applications using modern frameworks like [tauri](https://tauri.studio/en/).

## Key Features

- [x] Standard EXAFS analysis (find_e0, preedge postedge normalization, AUTOBK, FFT, IFFT)
- [x] Parallel processing using Rayon. (For example, M1 Macbook Pro with 10 cores can process 10000 spectra in 20 seconds, which is ~x10 enhancement without parallelization. Numpy + xraylarch takes 145 seconds.)
- [x] Optimization on AUTOBK. The AUTOBK process were optimized with providing an analytical Jacobian to speed up the minimization process by Leverberg-Marquardt algorithm.
- [x] FEFF85L path-based EXAFS fitting in Rust core (`xfeffdat` parsing, `path2chi`, `ff2chi`, single-dataset R-space fit with shared expression variables).
- [x] FEFF85L pure-Rust module execution workflow (`resolve_feff_commands`, `run_feff`, `run_feff_and_load_paths`) starting from a provided FEFF executable path.

## FEFF Fitting MVP Boundaries

- Rust core only in this release (`crates/xraytsubaki`).
- FEFF85L execution path supports deterministic module resolution (`feff8l_rdinp`, `feff8l_pot`, `feff8l_xsph`, `feff8l_pathfinder`, `feff8l_genfmt`, `feff8l_ff2x`) and output discovery.
- Existing parse-only workflows with pre-generated `feffNNNN.dat` files remain supported.
- Single-dataset R-space fitting only.
- FEFF10 parsing/execution are reserved and return typed unsupported errors in this MVP.

See `crates/xraytsubaki/doc/feff-fitting-mvp.md` for details and FEFF10 follow-up compatibility notes.

## Future Developments

- [ ] EXAFS helper funtions (rebinning and more)
- [ ] Develop a Python wrapper for the library.
- [ ] Create a GUI application using Dioxus.
- [ ] Develop a web assembly version of the library for web application usage.

## Licensing

(To be completed...)
