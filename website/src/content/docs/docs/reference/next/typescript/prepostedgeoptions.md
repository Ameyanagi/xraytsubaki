---
title: "TypeScript · PrePostEdgeOptions"
description: "PrePostEdgeOptions declarations, defaults and API explanations."
audience: user
pagefind: false
---

**Next API · unreleased.** This reference describes the source checkout. Compare with stable npm rexafs@0.2.14 before using it with an installed package.

[Installation and version guide](/docs/reference/) · [TypeScript tutorial](/docs/libraries/typescript/)

[Declaration source](https://github.com/Ameyanagi/rexafs/blob/main/js-rexafs/types.d.ts) · [JSDoc source](https://github.com/Ameyanagi/rexafs/blob/main/js-rexafs/types.d.ts)

Named field overrides for new PrePostEdge(options). Omitted fields retain constructor
defaults; explicitly assigning undefined uses the resolution described for each field. An
unknown key or a non-object options argument throws TypeError. Numerical range and data-
dependent checks generally run when the configured processing stage executes.

## pre_edge_start

```typescript
pre_edge_start?: number | undefined;
```

Lower pre-edge fit bound, as an energy offset from E0 in eV. Default: undefined selects a
rounded estimate near the beginning of the measured range. Choose a region below the
absorption edge without other edges or glitches; this region determines the subtracted
baseline. Since 0.2.15 a start before the first measured energy is raised to it during
normalization (the setting is kept); a start at or above pre_edge_end is an error.

## pre_edge_end

```typescript
pre_edge_end?: number | undefined;
```

Upper pre-edge fit bound relative to E0, in eV. Default: undefined derives a rounded value
from pre_edge_start. The pre-edge region is normally below E0 (negative offsets); moving it
toward the edge can include near-edge structure in the baseline fit.

## norm_start

```typescript
norm_start?: number | undefined;
```

Lower post-edge fit bound relative to E0, in eV. Default: undefined derives it from
norm_end, normally capped at 25 eV and kept at least 10 eV below norm_end. This region
estimates the absorption edge step and the trend removed by flat().

## norm_end

```typescript
norm_end?: number | undefined;
```

Upper post-edge fit bound relative to E0, in eV. Default: undefined rounds the available
post-edge extent to a nearby 5 eV boundary, without extending beyond the measured range.
Choose the range before a second absorption edge if one is present. Since 0.2.15 an
explicit value beyond the data is lowered to the last measured energy during
normalization, and a norm_start beyond it becomes min(25, 5 * round(norm_end / 15)) eV.

## norm_polyorder

```typescript
norm_polyorder?: number | undefined;
```

Integer degree of the polynomial fitted to pre-edge-subtracted absorption above E0.
Default: undefined selects 0, 1 or 2 for post-edge fit spans below 50, below 350, or at
least 350 eV. Values are clamped to 0 through 5. Higher order follows more curvature but
can absorb spectral structure.

## n_victoreen

```typescript
n_victoreen?: number | undefined;
```

Integer energy exponent used to model the pre-edge baseline. Default: undefined resolves to
0, a straight-line fit to mu(E). For exponent n, the code fits a line to mu(E) * E^n and
divides the fitted line by E^n; E is energy in eV. Use 0 unless this additional energy
dependence is justified.

## e0

```typescript
e0?: number | undefined;
```

Absorption-edge energy in eV. Default: undefined uses the spectrum E0 if assigned,
otherwise derivative-based edge detection. E0 sets the origin of the fit offsets and the
subsequent energy-to-k conversion. A supplied value must be finite and strictly between
the measured energy endpoints; spectrum processing throws an Error otherwise.

## edge_step

```typescript
edge_step?: number | undefined;
```

Absorption step used as the normalization divisor, in the same units as mu. Default:
undefined estimates post_edge - pre_edge at the input sample nearest E0. Use a positive
physical step; the implementation floors finite values below 1e-12 to 1e-12 and rejects
nonfinite steps. Changing it rescales norm() and chi().
