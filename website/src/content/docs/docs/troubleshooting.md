---
title: "Troubleshooting"
description: "Resolve common installation, data and processing problems."
audience: user
---

For scope and compatibility, see the [FAQ](/docs/faq/). For unfamiliar terms,
see the [glossary](/docs/concepts/).

| Symptom | Check |
|---|---|
| Import remains pending | Select the main signal, assign columns, confirm axis units and revalidate. |
| The imported point count is smaller than the file | Inspect parser diagnostics for skipped rows and excluded non-finite detector calculations; review the source-line examples before accepting the import. |
| Normalization reports an inverse-transform error | The desktop computes the full pipeline, including back FT. In **Process › Transform**, inspect the collapsed Back FT settings or reset that section from its **…** menu (reset the Transform stage in 0.2.14 and earlier). |
| Apply to marked affects fewer groups than expected | The current group and processing-locked groups are excluded. Hidden marks can still be selected; review the marked count. |
| A Series trend misses a brief feature | The overview is sampled to at most 192 frames. Inspect individual frames or calculate the batch/LCF trend for All frames. |
| The edge is at the wrong energy | Check eV versus keV and calibration; an automatic edge estimate is not a reference calibration. |
| Input arrays are rejected | Match lengths, remove non-finite values deliberately, and provide a strictly increasing energy axis. |
| `chi()`, `r()` or a window is unavailable | Run the required stage and handle `None`/`undefined` before using the result. |
| Python import/completion uses the wrong package | Run scripts with `uv run` from your analysis project and select that project's `.venv` in the editor and Jupyter kernel. |
| The Fourier spacing does not change after changing the background k step | Inferred settings are retained after calculation. Assign a fresh `XrayFFTF` configuration and rerun `fft()` to infer spacing from the new grid; see the language guide. |
| Constructor arguments are rejected | Keyword/options constructors require 0.2.5 or later. In 0.2.4, construct settings with no arguments, then assign their fields. |
| Browser objects fail during construction | Await Wasm initialization and make sure the bundler serves the `.wasm` asset. |
| A TypeScript object fails after cleanup | `free()` ends its lifetime. Create a new object before using the API again. |
| A linked project cannot find data | Restore the relative folder layout, or use an embedded project when sharing. |
| A fit converges with implausible parameters | Inspect ranges, model paths, constraints, residuals, bounds and parameter correlations. |
| Linux does not open a window | Use a graphical session and the required GTK/font/Vulkan runtime and driver. |

## Recover a project

Copy the previous save, `.rxs.bak`, to a new `.rxs` filename and open it.
Keep independent backups of important experiments.

## Ask for help

[Open a GitHub issue](https://github.com/Ameyanagi/rexafs/issues) with the rexafs
version, OS, interface, exact error and a small reproducible example. Share only
data you are able to distribute. Include processing settings and units when
reporting a numerical difference; a screenshot alone may not identify its cause.
