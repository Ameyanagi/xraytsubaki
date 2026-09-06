# Processing parity fixture

`ru-reference.json` records the default native rexafs pipeline on `Ru_QAS.dat`
(E0, grid lengths, and samples at indices 0, 20, 50 and 100). It was captured before
the September 2026 dependency upgrade to detect drift in the Wasm bindings and
upgraded numerical stack. The Rust facade is separately compared with the staged
core pipeline. These are software regression checks, not experimental validation.
