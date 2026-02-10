# Plan: Simplified Builder API + Multi-Dataset Fitting + Extended Expressions

## Context

The current fitting API requires ~10 types, redundant variable declarations, and a limited expression parser. This plan delivers:
1. Consuming-Self builder with clone-and-reuse
2. Auto-discovery of variables from expressions
3. Multi-dataset fitting with shared parameters
4. Full math expression engine (larch parity)
5. Unified parameter specification via `Param` struct + simple tuple shortcuts

---

## Target API

### Paths (builder methods on existing `FeffPathModel`)
```rust
// Build and clone for variations
let base_path = feffpath("feffcu01.dat", FeffFlavor::Feff85L)?
    .set_s02("amp").set_e0("de0").set_deltar("dr");

let path1 = base_path.clone().set_sigma2("sig2");
let path2 = base_path.clone().set_sigma2("sig2_2");

// Expressions work: .set_sigma2("sig2 * sqrt(reff)")
```

### Parameters — two methods
```rust
// Quick: set_inits() for simple (name, value) tuples
fit.set_inits([("amp", 0.95), ("sig2", 0.003), ("de0", 0.0)])

// Full: params() for Param structs with bounds/fixed/expr
fit.params([
    Param::new("amp", 0.95),
    Param::new("sig2", 0.003).bounds(0.0, 0.02),
    Param::fixed("de0", 1.4),
    Param::expr("scale", "amp * 2"),
])
```

### Single-dataset fit
```rust
let result = FeffFit::new()
    .data(&k, &chi)
    .add_path(feffpath("feff01.dat", FeffFlavor::Feff85L)?
        .set_s02("amp").set_e0("de0").set_sigma2("sig2").set_deltar("dr"))
    .set_inits([("amp", 0.95), ("sig2", 0.003)])
    .set_bounds("sig2", 0.0, 0.02)
    .krange(2.0, 14.0).rrange(1.0, 3.0)
    .fit()?;
```

### Clone-and-reuse template
```rust
let base = FeffFit::new()
    .params([Param::new("amp", 0.95), Param::new("sig2", 0.003).bounds(0.0, 0.02)])
    .krange(2.0, 14.0).rrange(1.0, 3.0);

let r1 = base.clone().data(&k1, &chi1).add_path(path1).fit()?;
let r2 = base.clone().data(&k2, &chi2).add_path(path2).fit()?;
```

### Multi-dataset global fit
```rust
let ds1 = FeffFitDataset::new()
    .data(&k1, &chi1)
    .add_path(feffpath("feff01.dat", FeffFlavor::Feff85L)?
        .set_s02("amp").set_e0("de0").set_sigma2("sig2"))
    .krange(2.0, 14.0).rrange(1.0, 3.0);

let ds2 = FeffFitDataset::new()
    .data(&k2, &chi2)
    .add_path(feffpath("feff02.dat", FeffFlavor::Feff85L)?
        .set_s02("amp").set_e0("de0_2").set_sigma2("sig2"))
    .krange(2.0, 12.0).rrange(1.0, 4.0);

let result = FeffFit::new()
    .add_dataset(ds1).add_dataset(ds2)
    .params([Param::new("amp", 0.9), Param::new("sig2", 0.003).bounds(0.0, 0.02)])
    .fit()?;
```

---

## Step 1: Extend expression parser — `variables.rs`

Extend `ExprParser::parse_primary()`: after parsing an identifier, check for `(` → parse as function call.

**Single-arg functions:** `abs`, `exp`, `log`, `log10`, `sqrt`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `sinh`, `cosh`, `tanh`, `ceil`, `floor`, `round`

**Two-arg functions:** `min`, `max`, `atan2`

**Constants:** `pi` → `π`, `e` → Euler's number

**Also add:** `pub fn extract_symbols(expr: &str) -> Vec<String>` — extracts user variable names, excluding function names, constants, and `"reff"`.

**File:** `crates/xraytsubaki/src/xafs/fitting/variables.rs`

---

## Step 2: Add `Param` struct — `types.rs`

```rust
/// Lightweight parameter specification for the builder API.
#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub value: f64,
    pub vary: bool,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub expr: Option<String>,
}

impl Param {
    /// Varying parameter with initial value.
    pub fn new(name: impl Into<String>, value: f64) -> Self { ... }
    /// Fixed parameter (vary=false).
    pub fn fixed(name: impl Into<String>, value: f64) -> Self { ... }
    /// Expression-derived parameter (vary=false).
    pub fn expr(name: impl Into<String>, expr: impl Into<String>) -> Self { ... }
    /// Set bounds (consuming Self).
    pub fn bounds(mut self, min: f64, max: f64) -> Self { ... }

    /// Convert to FitVariable.
    pub fn to_fit_variable(&self) -> FitVariable { ... }
}
```

**File:** `crates/xraytsubaki/src/xafs/fitting/types.rs`

---

## Step 3: Add builder methods to `FeffPathModel` — `types.rs`

Consuming-Self setters accepting `Into<PathParamSpec>`:
`set_s02`, `set_e0`, `set_ei`, `set_deltar`, `set_sigma2`, `set_third`, `set_fourth`, `set_degen`, `set_label`, `set_use_path`

**File:** `crates/xraytsubaki/src/xafs/fitting/types.rs`

---

## Step 4: Add builder methods to `FeffFitDataset` — `types.rs`

Consuming-Self methods:
`new()`, `data(k, chi)`, `epsilon_k(f64)`, `add_path(FeffPathModel)`, `krange(kmin, kmax)`, `rrange(rmin, rmax)`, `kweight(f64)`, `dk(f64)`, `window(FTWindow)`, `rwindow(FTWindow)`, `dr(f64)`

**File:** `crates/xraytsubaki/src/xafs/fitting/types.rs`

---

## Step 5: Restructure `FeffFitResult` — `types.rs`

New `DatasetResult` struct for per-dataset outputs. `FeffFitResult` gets `datasets: Vec<DatasetResult>`, `n_idp: f64`, and `dataset()` convenience method. Remove flat data/model fields.

**File:** `crates/xraytsubaki/src/xafs/fitting/types.rs`

---

## Step 6: Add `compute_n_idp()` — `transform.rs`

`pub fn compute_n_idp(transform: &FeffFitTransform) -> f64`
Formula: `1.0 + 2.0 * (rmax - rmin) * (kmax - kmin) / π`

**File:** `crates/xraytsubaki/src/xafs/fitting/transform.rs`

---

## Step 7: Multi-dataset solver — `solver.rs`

- `FeffFitMultiProblem` holding `Vec<FeffFitDataset>`, shared `FitVariables`, per-dataset `TransformOutput`
- `LeastSquaresProblem` impl: concatenated residuals, block-concatenated Jacobian
- `pub fn feffit_multi(datasets: &[FeffFitDataset], vars: &FitVariables) -> Result<FeffFitResult>`
- `feffit()` delegates to `feffit_multi(&[dataset], vars)`
- Update existing solver tests for new `FeffFitResult` shape

**File:** `crates/xraytsubaki/src/xafs/fitting/solver.rs`

---

## Step 8: `FeffFit` builder — `builder.rs` (new)

```rust
pub struct FeffFit {
    datasets: Vec<FeffFitDataset>,
    variables: FitVariables,
    flavor: FeffFlavor,
    default_dataset: FeffFitDataset,
    has_default: bool,
}
```

**All methods consume and return `Self`.**

| Method | Purpose |
|--------|---------|
| `data(k, chi)` | Set data on default dataset |
| `epsilon_k(f64)` | Set epsilon_k on default dataset |
| `add_path(FeffPathModel)` | Add path to default dataset |
| `krange(kmin, kmax)` | Set k-range on default dataset |
| `rrange(rmin, rmax)` | Set R-range on default dataset |
| `kweight(f64)`, `dk(f64)`, `window()`, `rwindow()`, `dr(f64)` | Transform config on default dataset |
| `add_dataset(FeffFitDataset)` | Add a pre-built dataset for multi-dataset fitting |
| `set_init(name, f64)` | Set single variable init value (auto-creates vary=true) |
| `set_inits(IntoIter<(Into<String>, f64)>)` | Batch set init values from tuples |
| `set_bounds(name, min, max)` | Set bounds on a variable |
| `fix(name, f64)` | Fix variable (vary=false) |
| `var_expr(name, expr)` | Expression-derived variable |
| `params(IntoIter<Param>)` | Batch configure from Param structs |
| `set_flavor(FeffFlavor)` | Set default FEFF flavor |
| `fit(&self) -> Result<FeffFitResult>` | Auto-discover variables, run fit |

**Auto-discovery in `fit()`:**
1. Merge default dataset into datasets list (if used)
2. Scan all `PathParamSpec::Expression` + `var_expr` expressions
3. `extract_symbols()` → collect all referenced variable names
4. Auto-create missing symbols as `FitVariable::new(0.0, true)`
5. Skip built-in names (function names, constants, `"reff"`)
6. Call `feffit_multi()`

**File:** `crates/xraytsubaki/src/xafs/fitting/builder.rs`

---

## Step 9: Module registration + prelude

**`fitting/mod.rs`:** Add `pub mod builder;`, re-export `FeffFit`, `Param`, `DatasetResult`, `feffit_multi`

**`prelude.rs`:** Add `FeffFit`, `Param`, `DatasetResult`, `feffit_multi`

---

## Step 10: Tests

**Expression parser** (`variables.rs`): `exp(-2.0)`, `abs(-3.5)`, `max(a,b)`, `sqrt(4)`, `pi*2`, `log(e)`, `sin(pi/2)`, nested `exp(-sigma2*k^2)`, error on unknown function

**Builder** (`builder.rs`): auto-discovery, `set_inits` tuples, `params` with Param structs, `set_bounds`, `fix`, `var_expr`, single-dataset parity, clone-reuse template, multi-dataset global fit, error cases, mixed literal/expression

**Solver** (`solver.rs`): multi-dataset with shared truth params, update existing test for new result shape

---

## Files summary

| File | Action |
|------|--------|
| `fitting/variables.rs` | MODIFY — extend parser + `extract_symbols()` |
| `fitting/types.rs` | MODIFY — `Param`, builder methods, `DatasetResult`, restructure `FeffFitResult` |
| `fitting/transform.rs` | MODIFY — `compute_n_idp()` |
| `fitting/solver.rs` | MODIFY — `feffit_multi()`, update tests |
| `fitting/builder.rs` | CREATE — `FeffFit` builder |
| `fitting/mod.rs` | MODIFY — register builder, re-exports |
| `prelude.rs` | MODIFY — add new types |

## Verification

1. `cargo check` — compiles
2. `cargo clippy` — no warnings
3. `cargo test` — all pass
4. Expression: `exp(-sigma2 * k^2)` evaluates correctly
5. Single-dataset builder matches direct `feffit()` output
6. Multi-dataset fit converges with shared parameters
7. Clone-and-reuse produces valid independent fits
