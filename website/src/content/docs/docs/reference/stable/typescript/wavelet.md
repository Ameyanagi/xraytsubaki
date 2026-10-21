---
title: "TypeScript · Wavelet"
description: "Wavelet declarations, defaults and API explanations."
audience: user
pagefind: true
---

**Stable 0.2.14.** These signatures match the released npm package. Explanations are maintained in source JSDoc and reviewed against this release.

[Installation and version guide](/docs/reference/) · [TypeScript tutorial](/docs/libraries/typescript/)

[Declaration source](https://github.com/Ameyanagi/rexafs/blob/v0.2.14/js-rexafs/types.d.ts) · [JSDoc source](https://github.com/Ameyanagi/rexafs/blob/main/js-rexafs/types.d.ts)

Cauchy settings, available since 0.2.10. Use spectrum.wavelet(new Wavelet([2, 12])).
k_range is measured support in Å⁻¹; since 0.2.15 endpoints beyond the measured k range
are limited to it; the map keeps the requested settings and its warnings report the
range used.
Defaults: weight 2, order 100,
k step 0.05 Å⁻¹, R up to 6 Å, no taper and automatic FFT/R sampling.
cauchy_v1 fixes order independently of R extent; larger order narrows frequency
response and broadens localization in k. R is not phase-corrected.
Browser init() is required. Calculations are synchronous native Wasm operations;
use a Worker for large interactive jobs. Invalid coverage/grids/budgets throw.

## constructor

```typescript
constructor(k_range: [number, number], options?: WaveletOptions);
```

Copy an inclusive measured k interval and optional named settings.

## calculate

```typescript
calculate(k: Float64Array, chi: Float64Array): WaveletMap;
```

Transform original unweighted dimensionless χ(k). Copies finite matching arrays
with increasing, nonnegative k; linearly resamples without extrapolation.

## estimate

```typescript
estimate(k: Float64Array): WaveletSize;
```

Validate dimensions and estimate buffer storage before transforming.

## to_json

```typescript
to_json(): string;
```

Native settings JSON with automatic choices preserved; no input arrays.

## from_json

```typescript
static from_json(json: string): Wavelet;
```

Restore settings. Calculation validates scientific values and resource limits.

## free

```typescript
free(): void;
```

Release this model. Independent maps and copied spectrum settings remain valid.
