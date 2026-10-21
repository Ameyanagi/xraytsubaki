---
title: "Python · Wavelet"
description: "Wavelet signatures, defaults and API explanations."
audience: user
pagefind: true
---

**Stable 0.2.14.** These signatures match the released Python package. Explanations are maintained in the source docstrings and reviewed against this release.

[Installation and version guide](/docs/reference/) · [Python tutorial](/docs/libraries/python/)

[Declaration source](https://github.com/Ameyanagi/rexafs/blob/v0.2.14/py-rexafs/python/rexafs/__init__.pyi) · [Docstring source](https://github.com/Ameyanagi/rexafs/blob/main/py-rexafs/python/rexafs/__init__.pyi)

Cauchy settings, available since 0.2.10. Use spectrum.wavelet(Wavelet((2, 12))).

k_range is measured support in inverse angstroms. Defaults: weight 2, order
100, kstep 0.05, R maximum 6 angstroms, no taper and automatic FFT/R sampling.
Missing normalization/AUTOBK run on a copy; existing chi is reused. Larger
order narrows frequency response and broadens localization in k. R is not
phase-corrected. Fixed order is independent of R extent (cauchy_v1).
Calculations release the GIL and return owned results; invalid definitions,
intervals wholly outside the measured k range and excessive allocations
raise ValueError. Since 0.2.15 endpoints beyond the measured k range are
limited to it; the map keeps the requested settings and its warnings
report the range used.

## Wavelet

```python
Wavelet(k_range: tuple[float, float], *, kweight: int=2, order: int=100, kstep: float=0.05, rmax: float=6.0, rstep: float | None=None, taper: float=0.0, nfft: int | None=None, radii: Sequence[float] | None=None)
```

Copy settings. taper is half-cosine width inside support (inverse angstroms);
zero means none. radii replaces generated positive increasing R coordinates.
nfft must be a power of two at least twice the prepared grid length.

## calculate

```python
calculate(self, k: NDArray[np.float64] | Sequence[float], chi: NDArray[np.float64] | Sequence[float]) -> WaveletMap
```

Copy original unweighted chi(k), release the GIL and calculate a native map.
k is finite, nonnegative, increasing and in inverse angstroms; chi is finite
and dimensionless. Linear resampling never extrapolates measured support.
Original inputs are retained; no display sampling alters the result.

## estimate

```python
estimate(self, k: NDArray[np.float64] | Sequence[float]) -> WaveletSize
```

Validate dimensions and estimate scientific buffer bytes before calculation.

## to_json

```python
to_json(self) -> str
```

Native settings JSON, with automatic choices preserved and no input arrays.

## from_json

```python
from_json(json: str) -> Wavelet
```

Restore native settings; calculation validates scientific values and budgets.
