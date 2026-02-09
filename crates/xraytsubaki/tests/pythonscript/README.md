# Larch Fixture Generation (uv)

This directory uses `uv` for generating xraylarch-based reference fixtures.

## Setup dependencies

```bash
uv lock --project crates/xraytsubaki/tests/pythonscript
```

## Regenerate FEFF fitting fixtures

```bash
uv run --project crates/xraytsubaki/tests/pythonscript \
  python crates/xraytsubaki/tests/pythonscript/generate_test.py --target feff
```

## Regenerate all fixtures

```bash
uv run --project crates/xraytsubaki/tests/pythonscript \
  python crates/xraytsubaki/tests/pythonscript/generate_test.py
```

This generates/updates files under `crates/xraytsubaki/tests/testfiles/`, including FEFF fitting references.
