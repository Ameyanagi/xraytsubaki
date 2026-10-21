---
title: "Python · PrePostEdge"
description: "PrePostEdge signatures, defaults and API explanations."
audience: user
pagefind: true
---

**Stable 0.2.14.** These signatures match the released Python package. Explanations are maintained in the source docstrings and reviewed against this release.

[Installation and version guide](/docs/reference/) · [Python tutorial](/docs/libraries/python/)

[Declaration source](https://github.com/Ameyanagi/rexafs/blob/v0.2.14/py-rexafs/python/rexafs/__init__.pyi) · [Docstring source](https://github.com/Ameyanagi/rexafs/blob/main/py-rexafs/python/rexafs/__init__.pyi)

Configure the baselines and edge step used to normalize absorption.

A line is fitted before the absorption edge and a polynomial after it.
The normalized signal is (mu - pre_edge) / edge_step; mu and both baselines
have the same absorption units, so the result is dimensionless. The edge
step is the fitted baseline difference near E0 unless you override it.
Fit limits are energy offsets from E0 in eV, not absolute energies.

All fields default to None and are resolved from the spectrum when processing.
These automatic choices match Rust PrePostEdge::new(); Rust's Default trait
uses fixed ranges instead. Start with automatic settings and inspect the
baselines before choosing narrower ranges or a higher polynomial degree.
Settings are copied into a method and spectrum; edit and assign them again
to apply a later change.

Example: p = PrePostEdge(); p.pre_edge_end = -30.0. Assign p to the
spectrum's normalization stage, then call normalize() or a later stage.

For the measurement and normalization convention, see
[Newville, Fundamentals of XAFS, sections 4 and 5](https://docs.xrayabsorption.org/tutorials/XAFS_Fundamentals.pdf).
Automatic range selection and the numerical safeguards are rexafs choices;
see [processing theory](https://rexafs.com/docs/science/processing/).

Resolved fit ranges, polynomial degree and Victoreen exponent are retained
inside the spectrum. An automatically estimated edge step is recalculated
after normalization results are invalidated.
Processing does not replace None fields in your original settings object.
Reassign fresh or reset settings when you want retained automatic choices
recalculated after changing the data or an earlier stage.

## PrePostEdge

```python
PrePostEdge(*, pre_edge_start: float | None=None, pre_edge_end: float | None=None, norm_start: float | None=None, norm_end: float | None=None, norm_polyorder: int | None=None, n_victoreen: int | None=None, e0: float | None=None, edge_step: float | None=None)
```

Create automatic pre/post-edge settings, with every field initially None.

Set only the fields your data require, then assign the settings to the
spectrum's normalization stage. Fit ranges and degrees are
resolved when normalization runs; creating settings does not process data.

Keyword arguments were added in 0.2.5. Published
0.2.4 settings use construction without arguments followed by field
assignment. Python type conversion can raise TypeError, and an integer
outside the native field's representable range can raise OverflowError
before any numerical processing.

Normalization fit choices are validated when a stage runs. To see the
effect of a change, inspect pre_edge(), post_edge(), norm() and flat()
after assigning the settings and calling normalize().

## pre_edge_start

```python
pre_edge_start: float | None
```

Start of the pre-edge fit, as an offset from E0 in eV.

Default None estimates a lower bound near the second energy sample, rounded
on a 2 or 5 eV grid and clipped to the measured range. Usually this offset
is negative. Choose a range below the edge rise. Since 0.2.15 an explicit
start before the first measured energy is raised to it when normalize() runs
(the stored value is kept), and a start at or above pre_edge_end is an error
naming pre_edge_start instead of being swapped.

## pre_edge_end

```python
pre_edge_end: float | None
```

End of the pre-edge fit, as an offset from E0 in eV.

Default None uses 5 * round(pre_edge_start / 15), approximately one third
of the lower offset. A more negative value excludes more of the edge rise
but leaves fewer baseline samples. Keep at least two usable pre-edge points.

## norm_start

```python
norm_start: float | None
```

Start of the post-edge polynomial fit, as an offset from E0 in eV.

Default None uses 5 * round(norm_end / 15), capped at 25 eV, then ensures
at least a 10 eV separation from norm_end. Select a region beyond the edge
rise; very short scans can make this automatic choice unsuitable. Since
0.2.15, when a start lies beyond the (data-limited) norm_end, normalize() uses
min(25, 5 * round(norm_end / 15)) eV instead of failing; the setting is kept.

## norm_end

```python
norm_end: float | None
```

End of the post-edge polynomial fit, as an offset from E0 in eV.

Default None rounds the available upper energy offset to a 5 eV grid and
clips it to the data limit. Reducing this bound excludes high-energy data
from the normalization fit; it does not trim the spectrum itself. Since
0.2.15 an explicit value beyond the last measured energy is lowered to it
when normalize() runs; the stored setting keeps the requested value.

## norm_polyorder

```python
norm_polyorder: int | None
```

Degree of the polynomial fitted to the pre-edge-subtracted post-edge data.

Default None chooses degree 0 for a fit span below 50 eV, 1 below 350 eV,
and 2 otherwise. Explicit degrees are clamped to 0 through 5. Higher degrees
can follow baseline curvature but also fit noise or oscillations; begin
with the automatic choice. The fit needs more samples than its degree.

## n_victoreen

```python
n_victoreen: int | None
```

Energy exponent used to fit the pre-edge baseline. Default None resolves to 0.

For exponent n, the code fits mu(E) * E**n with a line, then divides that
line by E**n to recover the baseline in mu units; E is in eV. With n=0
this is an ordinary straight-line baseline. A nonzero exponent changes
its curvature; it is a baseline model, not a correction for self-absorption.

## e0

```python
e0: float | None
```

Absorption edge energy E0 in eV. Default: None for automatic detection.

E0 defines the origin of the fit-range offsets and the conversion from
energy to photoelectron k. Assigning these settings can replace a spectrum's
previous E0. Spectrum processing rejects an explicit E0 that is non-finite
or not strictly inside the measured energy range with ValueError; it does
not silently replace such an invalid override with an automatic value. The
lower-level Rust baseline fitter has its own redetection safeguards near the
end of the scan, so inspect the resolved Spectrum.e0(). An automatic
derivative estimate is not an energy calibration.

## edge_step

```python
edge_step: float | None
```

Override the absorption edge step, in the same units as mu.

Default None estimates post_edge - pre_edge at the sample nearest E0.
Normalization divides by this value, so changing it rescales norm and chi.
Use a finite positive value if a separately determined step is available.
The implementation floors finite steps below 1e-12 to 1e-12; that safeguard
does not make a zero or negative experimental edge step meaningful.
