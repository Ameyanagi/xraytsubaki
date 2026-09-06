# Larch Fixture Generation (uv)

This directory uses `uv` for generating xraylarch-based reference fixtures.

## Setup dependencies

```bash
uv lock --project crates/rexafs/tests/pythonscript
```

## Regenerate FEFF fitting fixtures

```bash
uv run --project crates/rexafs/tests/pythonscript \
  python crates/rexafs/tests/pythonscript/generate_test.py --target feff
```

## Regenerate all fixtures

```bash
uv run --project crates/rexafs/tests/pythonscript \
  python crates/rexafs/tests/pythonscript/generate_test.py
```

This generates/updates files under `crates/rexafs/tests/testfiles/`, including FEFF fitting references.

## Independent spline reference

```bash
uv run --python 3.12 --with scipy \
  python crates/rexafs/tests/pythonscript/generate_spline_reference.py
```

This writes `spline_scipy_reference.json`, recording the SciPy version, nonuniform
knots, coefficients, clamped/extrapolated evaluations and coefficient derivatives.
Rust tests compare the local direct spline solver to these independent values.
