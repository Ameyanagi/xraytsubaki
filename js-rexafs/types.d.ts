/**
 * Forward-transform sampling convention. Input preserves the background k grid; Larch
 * constructs a zero-origin uniform grid with linear resampling and an extended window domain.
 * The default is Input. This choice affects Fourier preparation only, not the background
 * k()/chi() arrays.
 * Names are case-sensitive. Assigning an unsupported grid throws a string exception and
 * leaves the existing selection unchanged.
 */
export type FFTGrid = "Input" | "Larch";
/**
 * Supported Fourier-window families. Hanning and FHanning use cosine tapers; Parzen, Welch,
 * Gaussian, Sine and KaiserBessel use their named shapes. Forward and inverse settings default
 * to KaiserBessel; AUTOBK defaults to Hanning. Parameters dk/dr have shape-dependent meaning,
 * and KaiserBessel also uses them as shape parameters. See the [window
 * implementation](https://github.com/Ameyanagi/rexafs/blob/main/crates/rexafs/src/xafs/xafsutils.rs)
 * for the exact formulas.
 * Names are case-sensitive. An unsupported name throws an Error immediately during property
 * assignment, leaving the existing window selection unchanged.
 */
export type FTWindow = "Hanning" | "Parzen" | "Welch" | "Gaussian" | "Sine" | "KaiserBessel" | "FHanning";
/**
 * Background spline solver. LinearDirect is the recommended default and is required for
 * FixedPenalty. LegacyLm is the iterative Levenberg-Marquardt solver for legacy objectives.
 * TrustRegionDogLeg requires a native Rust feature and is unavailable in the distributed Wasm
 * build; selecting it throws during processing.
 * An unrecognized name throws an Error immediately during property assignment and leaves the
 * existing selection unchanged.
 */
export type AUTOBKSolver = "TrustRegionDogLeg" | "LegacyLm" | "LinearDirect";
/**
 * Background endpoint model. FixedPenalty is the recommended fixed mean-square endpoint
 * penalty, controlled by clamp_lambda and solved by LinearDirect. Fixed and TwoPass retain
 * historical residual-dependent scaling behavior. Selecting a policy changes the optimization
 * objective, not merely the numerical solver.
 * Names are case-sensitive; an unsupported name throws an Error during property assignment
 * without changing the existing selection.
 */
export type AUTOBKClampScalePolicy = "FixedPenalty" | "Fixed" | "TwoPass";

/**
 * Named field overrides for new PrePostEdge(options). Omitted fields retain constructor
 * defaults; explicitly assigning undefined uses the resolution described for each field. An
 * unknown key or a non-object options argument throws TypeError. Numerical range and data-
 * dependent checks generally run when the configured processing stage executes.
 */
export interface PrePostEdgeOptions {
  /**
   * Lower pre-edge fit bound, as an energy offset from E0 in eV. Default: undefined selects a
   * rounded estimate near the beginning of the measured range. Choose a region below the
   * absorption edge without other edges or glitches; this region determines the subtracted
   * baseline. Since 0.2.15 a start before the first measured energy is raised to it during
   * normalization (the setting is kept); a start at or above pre_edge_end is an error.
   */
  pre_edge_start?: number | undefined;
  /**
   * Upper pre-edge fit bound relative to E0, in eV. Default: undefined derives a rounded value
   * from pre_edge_start. The pre-edge region is normally below E0 (negative offsets); moving it
   * toward the edge can include near-edge structure in the baseline fit.
   */
  pre_edge_end?: number | undefined;
  /**
   * Lower post-edge fit bound relative to E0, in eV. Default: undefined derives it from
   * norm_end, normally capped at 25 eV and kept at least 10 eV below norm_end. This region
   * estimates the absorption edge step and the trend removed by flat().
   */
  norm_start?: number | undefined;
  /**
   * Upper post-edge fit bound relative to E0, in eV. Default: undefined rounds the available
   * post-edge extent to a nearby 5 eV boundary, without extending beyond the measured range.
   * Choose the range before a second absorption edge if one is present. Since 0.2.15 an
   * explicit value beyond the data is lowered to the last measured energy during
   * normalization, and a norm_start beyond it becomes min(25, 5 * round(norm_end / 15)) eV.
   */
  norm_end?: number | undefined;
  /**
   * Integer degree of the polynomial fitted to pre-edge-subtracted absorption above E0.
   * Default: undefined selects 0, 1 or 2 for post-edge fit spans below 50, below 350, or at
   * least 350 eV. Values are clamped to 0 through 5. Higher order follows more curvature but
   * can absorb spectral structure.
   */
  norm_polyorder?: number | undefined;
  /**
   * Integer energy exponent used to model the pre-edge baseline. Default: undefined resolves to
   * 0, a straight-line fit to mu(E). For exponent n, the code fits a line to mu(E) * E^n and
   * divides the fitted line by E^n; E is energy in eV. Use 0 unless this additional energy
   * dependence is justified.
   */
  n_victoreen?: number | undefined;
  /**
   * Absorption-edge energy in eV. Default: undefined uses the spectrum E0 if assigned,
   * otherwise derivative-based edge detection. E0 sets the origin of the fit offsets and the
   * subsequent energy-to-k conversion. A supplied value must be finite and strictly between
   * the measured energy endpoints; spectrum processing throws an Error otherwise.
   */
  e0?: number | undefined;
  /**
   * Absorption step used as the normalization divisor, in the same units as mu. Default:
   * undefined estimates post_edge - pre_edge at the input sample nearest E0. Use a positive
   * physical step; the implementation floors finite values below 1e-12 to 1e-12 and rejects
   * nonfinite steps. Changing it rescales norm() and chi().
   */
  edge_step?: number | undefined;
}
/**
 * Settings for absorption-edge normalization and flattening.
 *
 * The pre-edge fit estimates the smooth baseline before the edge. The post-edge fit estimates
 * the edge step and the trend used by flat(). All energy bounds are offsets from E0 in eV, not
 * absolute energies. Undefined fields select data-dependent values where described; inspect the
 * measured fit regions before interpreting a normalized spectrum.
 *
 * Settings are copied when assigned to a spectrum. Changing this object afterwards
 * requires assigning it again; free() releases only this object's Wasm allocation.
 *
 * Background and normalization conventions are explained in [Newville, Fundamentals of
 * XAFS](https://docs.xrayabsorption.org/tutorials/XAFS_Fundamentals.pdf). Automatic range and
 * polynomial rules are rexafs implementation choices in
 * [PrePostEdge](https://github.com/Ameyanagi/rexafs/blob/main/crates/rexafs/src/xafs/normalization.rs).
 */
export class PrePostEdge {
  /**
   * Create owned settings with the recommended defaults described below. Browser callers must
   * await init() first. Edit the fields, copy the settings into the appropriate spectrum stage,
   * and free() this object when finished.
   *
   * Automatic fields are resolved on the spectrum's copy during processing; resolved values
   * are not written back into the original settings object. Resolved values stay in the
   * spectrum's settings until those settings are replaced.
   *
   * Named options were added in 0.2.5. Version 0.2.4 uses this constructor without
   * arguments, followed by property assignment.
   */
  constructor(options?: PrePostEdgeOptions);
  /**
   * Release this object's Wasm allocation. Do not call methods, read fields or free it again
   * afterwards. Arrays and settings already copied elsewhere remain valid.
   */
  free(): void;
  /**
   * Lower pre-edge fit bound, as an energy offset from E0 in eV. Default: undefined selects a
   * rounded estimate near the beginning of the measured range. Choose a region below the
   * absorption edge without other edges or glitches; this region determines the subtracted
   * baseline.
   */
  pre_edge_start: number | undefined;
  /**
   * Upper pre-edge fit bound relative to E0, in eV. Default: undefined derives a rounded value
   * from pre_edge_start. The pre-edge region is normally below E0 (negative offsets); moving it
   * toward the edge can include near-edge structure in the baseline fit.
   */
  pre_edge_end: number | undefined;
  /**
   * Lower post-edge fit bound relative to E0, in eV. Default: undefined derives it from
   * norm_end, normally capped at 25 eV and kept at least 10 eV below norm_end. This region
   * estimates the absorption edge step and the trend removed by flat().
   */
  norm_start: number | undefined;
  /**
   * Upper post-edge fit bound relative to E0, in eV. Default: undefined rounds the available
   * post-edge extent to a nearby 5 eV boundary, without extending beyond the measured range.
   * Choose the range before a second absorption edge if one is present.
   */
  norm_end: number | undefined;
  /**
   * Integer degree of the polynomial fitted to pre-edge-subtracted absorption above E0.
   * Default: undefined selects 0, 1 or 2 for post-edge fit spans below 50, below 350, or at
   * least 350 eV. Values are clamped to 0 through 5. Higher order follows more curvature but
   * can absorb spectral structure.
   */
  norm_polyorder: number | undefined;
  /**
   * Integer energy exponent used to model the pre-edge baseline. Default: undefined resolves to
   * 0, a straight-line fit to mu(E). For exponent n, the code fits a line to mu(E) * E^n and
   * divides the fitted line by E^n; E is energy in eV. Use 0 unless this additional energy
   * dependence is justified.
   */
  n_victoreen: number | undefined;
  /**
   * Absorption-edge energy in eV. Default: undefined uses the spectrum E0 if assigned,
   * otherwise derivative-based edge detection. E0 sets the origin of the fit offsets and the
   * subsequent energy-to-k conversion. A supplied value must be finite and strictly between
   * the measured energy endpoints; spectrum processing throws an Error otherwise.
   */
  e0: number | undefined;
  /**
   * Absorption step used as the normalization divisor, in the same units as mu. Default:
   * undefined estimates post_edge - pre_edge at the input sample nearest E0. Use a positive
   * physical step; the implementation floors finite values below 1e-12 to 1e-12 and rejects
   * nonfinite steps. Changing it rescales norm() and chi().
   */
  edge_step: number | undefined;
}

/**
 * Named field overrides for new AUTOBK(options). Omitted fields retain constructor defaults;
 * explicitly assigning undefined uses the resolution described for each field. An unknown key
 * or a non-object options argument throws TypeError. Numerical range and data-dependent checks
 * generally run when the configured processing stage executes.
 */
export interface AUTOBKOptions {
  /**
   * Edge energy used for the energy-to-k conversion, in eV. Default: undefined uses
   * normalization E0. An in-range override changes the background k grid without redefining the
   * pre/post-edge fitting regions. Prefer a consistent E0 across both stages; an out-of-range
   * override is discarded.
   */
  ek0?: number | undefined;
  /**
   * Background cutoff in angstroms. Default: 1.0; undefined restores this default. AUTOBK
   * minimizes low-R Fourier components of the residual below a cutoff derived from this value.
   * Increasing rbkg allows a more flexible background and can remove real first-shell signal;
   * start below the first structural peak.
   */
  rbkg?: number | undefined;
  /**
   * Number of spline control points used to represent the smooth background. Default: undefined
   * derives the count from rbkg and the fitted k interval; the resolved count is clamped to 5
   * through 128. More points add flexibility and can overfit structure. This is not the length
   * of the repeated endpoint knot vector.
   */
  nknots?: number | undefined;
  /**
   * Lower background fit/window bound in inverse angstroms. Default: 0.0; undefined restores
   * this default. Require kmin < the usable kmax. Raising this bound excludes the near-edge
   * region from the Fourier objective; the returned k() grid still begins at zero.
   */
  kmin?: number | undefined;
  /**
   * Upper background fit/window bound in inverse angstroms. Default: undefined uses the
   * available data limit and explicit values are capped at that limit. Reducing it can exclude
   * noisy high-k data and shortens the returned k()/chi() arrays.
   */
  kmax?: number | undefined;
  /**
   * Spacing of the uniform output k() grid in inverse angstroms. Default: 0.05; undefined
   * restores this default. Must be finite and positive. Smaller spacing interpolates the same
   * measured data more densely and changes the internal R sampling; it does not add
   * experimental resolution.
   */
  kstep?: number | undefined;
  /**
   * Number of samples at each enabled endpoint used to discourage large residual chi values.
   * Default: 3; undefined restores this default. FixedPenalty requires a nonnegative integer,
   * caps the count at the available samples, and includes the last high-k sample. Use 0 to
   * disable endpoint clamping.
   */
  nclamp?: number | undefined;
  /**
   * Integer multiplier for the low-k endpoint residuals. Default: 0, which disables that
   * endpoint. In FixedPenalty the absolute value multiplies each residual, so its square
   * weights the objective. The active low-k samples begin at k=0 on the output grid.
   */
  clamp_lo?: number | undefined;
  /**
   * Integer multiplier for the high-k endpoint residuals. Default: 1. In FixedPenalty the
   * absolute value multiplies each residual, so doubling it quadruples that endpoint
   * contribution before averaging. Use 0 to disable the high-k endpoint penalty.
   */
  clamp_hi?: number | undefined;
  /**
   * Numerical strength of the FixedPenalty endpoint term. Recommended default: 0.001; undefined
   * restores this default. The objective adds lambda times the mean squared active, weighted
   * endpoint chi residual to the mean squared low-R residual. Require a finite nonnegative
   * value; 0 disables the endpoint term.
   *
   * This empirical balance is tied to the implemented residual convention: the FixedPenalty
   * Fourier residual uses the fixed numerical factor 0.05/sqrt(pi), while the endpoint residual
   * uses unweighted, edge-step-normalized chi. Changing kweight or the window changes the
   * balance at a fixed lambda. The parameter is unused by legacy endpoint policies; it is not a
   * universal physical constant. See [the fixed-penalty
   * objective](https://rexafs.com/docs/science/autobk/).
   */
  clamp_lambda?: number | undefined;
  /**
   * FFT length used inside background removal. Default: 2048; undefined restores this default.
   * Use a positive integer large enough to contain the prepared k grid. It sets the internal R
   * spacing together with kstep; increasing zero-padding refines that grid without adding
   * measured information.
   */
  nfft?: number | undefined;
  /**
   * Integer power of k applied in the background Fourier objective. Default: 1; undefined
   * restores this default. Larger powers emphasize high-k residuals and their noise. The
   * returned chi() remains unweighted; the independent XrayFFTF.kweight controls the later
   * displayed transform.
   */
  kweight?: number | undefined;
  /**
   * Background window parameter. Default: 0.1; undefined restores this default. For the default
   * Hanning window it controls endpoint taper widths in inverse angstroms. KaiserBessel also
   * uses this numeric value as a dimensionless shape parameter; equal dk does not make
   * different window families equivalent.
   */
  dk?: number | undefined;
  /**
   * Ridge strength for the legacy LinearDirect objective. Default: 0.0001; undefined restores
   * this default. Unused by the recommended FixedPenalty model, which solves the specified
   * endpoint-penalized least-squares problem without this additional ridge term. Change only
   * when reproducing a legacy calculation.
   */
  linear_regularization?: number | undefined;
  /**
   * Largest accepted condition number of the linear system. Default: 1e8; undefined restores
   * this default. FixedPenalty applies this limit to the column-scaled design matrix and
   * requires a finite value of at least 1. An ill-conditioned or rank-deficient solve throws
   * instead of silently changing the objective.
   */
  linear_condition_limit?: number | undefined;
  /**
   * Acceptance threshold for comparing the legacy direct solution residual against its
   * reference residual. Default: 1.05; undefined restores this default. Unused by FixedPenalty.
   * This is a numerical fallback criterion, not a statistical uncertainty or goodness-of-fit
   * probability.
   */
  linear_residual_ratio_limit?: number | undefined;
  /**
   * Allow the legacy direct solver to retry with linear_fallback_solver if its checks fail.
   * Default: true; undefined restores this default. Despite the historical name, the chosen
   * fallback need not be Levenberg-Marquardt. FixedPenalty never falls back and ignores this
   * flag.
   */
  linear_fallback_to_lm?: boolean | undefined;
  /**
   * Reuse compatible spline geometry, Fourier operators and matrix factorization. Default:
   * true; undefined restores this default. Each spectrum still supplies new data and receives a
   * new solution, with condition checks repeated. Disabling this changes reuse and runtime, not
   * the specified objective.
   */
  linear_workspace_cache?: boolean | undefined;
  /**
   * Window family used inside the background Fourier objective. Default: Hanning; undefined
   * restores Hanning. The taper reduces ringing at the selected k boundaries. This setting is
   * independent of the later XrayFFTF window; see FTWindow for the accepted names.
   */
  window?: FTWindow | undefined;
  /**
   * Algorithm used to determine background spline coefficients. Recommended default:
   * LinearDirect, required by FixedPenalty; undefined restores it. LegacyLm solves the legacy
   * objective iteratively. TrustRegionDogLeg requires a native Rust feature and throws when
   * processing in the published Wasm package.
   */
  solver?: AUTOBKSolver | undefined;
  /**
   * Solver used only when an enabled legacy LinearDirect fallback is needed. Default in Wasm:
   * LegacyLm; undefined also resolves to LegacyLm with the default fallback flag. FixedPenalty
   * ignores this option. TrustRegionDogLeg is unavailable in Wasm, and LinearDirect cannot be
   * its own fallback.
   */
  linear_fallback_solver?: AUTOBKSolver | undefined;
  /**
   * Endpoint penalty model. Recommended default: FixedPenalty; undefined restores it.
   * FixedPenalty uses a fixed mean-square endpoint term and requires LinearDirect. Fixed and
   * TwoPass preserve older residual-dependent scaling choices for reproducing historical
   * results; they are different objectives.
   */
  clamp_scale_policy?: AUTOBKClampScalePolicy | undefined;
}
/**
 * Settings for extracting extended X-ray absorption fine structure, chi(k), with a cubic
 * spline background in photoelectron wavenumber k.
 *
 * AUTOBK separates slowly varying atomic absorption from oscillations associated with
 * neighboring atoms by suppressing low-R Fourier residuals. Recommended starting values are
 * rbkg=1 angstrom, kstep=0.05 inverse angstroms, kweight=1, Hanning, and LinearDirect with
 * FixedPenalty and clamp_lambda=0.001. Choose rbkg below structural signal; excessive
 * background flexibility can remove that signal.
 *
 * The original method is [Newville et al. (1993)](https://doi.org/10.1103/PhysRevB.47.14126).
 * The fixed endpoint penalty and its default strength are rexafs-specific choices, implemented
 * in
 * [background/fixed.rs](https://github.com/Ameyanagi/rexafs/blob/main/crates/rexafs/src/xafs/background/fixed.rs);
 * they are not part of that paper's objective.
 *
 * The spectrum copies assigned background settings. Reassign after editing, and release your
 * settings with free() when finished. A processing failure, including an ill-conditioned
 * fixed-penalty fit, throws an Error without silently switching objectives.
 */
export class AUTOBK {
  /**
   * Create owned settings with the recommended defaults described below. Browser callers must
   * await init() first. Edit the fields, copy the settings into the appropriate spectrum stage,
   * and free() this object when finished.
   *
   * Automatic fields are resolved on the spectrum's copy during processing; resolved values
   * are not written back into the original settings object. Resolved scalar defaults and ek0
   * are retained in the spectrum. Automatic kmax and nknots remain unset in stored settings
   * and are calculated locally for each input.
   *
   * Named options were added in 0.2.5. Version 0.2.4 uses this constructor without
   * arguments, followed by property assignment.
   */
  constructor(options?: AUTOBKOptions);
  /**
   * Release this object's Wasm allocation. Do not call methods, read fields or free it again
   * afterwards. Arrays and settings already copied elsewhere remain valid.
   */
  free(): void;
  /**
   * Edge energy used for the energy-to-k conversion, in eV. Default: undefined uses
   * normalization E0. An in-range override changes the background k grid without redefining the
   * pre/post-edge fitting regions. Prefer a consistent E0 across both stages; an out-of-range
   * override is discarded.
   */
  ek0: number | undefined;
  /**
   * Background cutoff in angstroms. Default: 1.0; undefined restores this default. AUTOBK
   * minimizes low-R Fourier components of the residual below a cutoff derived from this value.
   * Increasing rbkg allows a more flexible background and can remove real first-shell signal;
   * start below the first structural peak.
   */
  rbkg: number | undefined;
  /**
   * Number of spline control points used to represent the smooth background. Default: undefined
   * derives the count from rbkg and the fitted k interval; the resolved count is clamped to 5
   * through 128. More points add flexibility and can overfit structure. This is not the length
   * of the repeated endpoint knot vector.
   */
  nknots: number | undefined;
  /**
   * Lower background fit/window bound in inverse angstroms. Default: 0.0; undefined restores
   * this default. Require kmin < the usable kmax. Raising this bound excludes the near-edge
   * region from the Fourier objective; the returned k() grid still begins at zero.
   */
  kmin: number | undefined;
  /**
   * Upper background fit/window bound in inverse angstroms. Default: undefined uses the
   * available data limit and explicit values are capped at that limit. Reducing it can exclude
   * noisy high-k data and shortens the returned k()/chi() arrays.
   */
  kmax: number | undefined;
  /**
   * Spacing of the uniform output k() grid in inverse angstroms. Default: 0.05; undefined
   * restores this default. Must be finite and positive. Smaller spacing interpolates the same
   * measured data more densely and changes the internal R sampling; it does not add
   * experimental resolution.
   */
  kstep: number | undefined;
  /**
   * Number of samples at each enabled endpoint used to discourage large residual chi values.
   * Default: 3; undefined restores this default. FixedPenalty requires a nonnegative integer,
   * caps the count at the available samples, and includes the last high-k sample. Use 0 to
   * disable endpoint clamping.
   */
  nclamp: number | undefined;
  /**
   * Integer multiplier for the low-k endpoint residuals. Default: 0, which disables that
   * endpoint. In FixedPenalty the absolute value multiplies each residual, so its square
   * weights the objective. The active low-k samples begin at k=0 on the output grid.
   */
  clamp_lo: number | undefined;
  /**
   * Integer multiplier for the high-k endpoint residuals. Default: 1. In FixedPenalty the
   * absolute value multiplies each residual, so doubling it quadruples that endpoint
   * contribution before averaging. Use 0 to disable the high-k endpoint penalty.
   */
  clamp_hi: number | undefined;
  /**
   * Numerical strength of the FixedPenalty endpoint term. Recommended default: 0.001; undefined
   * restores this default. The objective adds lambda times the mean squared active, weighted
   * endpoint chi residual to the mean squared low-R residual. Require a finite nonnegative
   * value; 0 disables the endpoint term.
   *
   * This empirical balance is tied to the implemented residual convention: the FixedPenalty
   * Fourier residual uses the fixed numerical factor 0.05/sqrt(pi), while the endpoint residual
   * uses unweighted, edge-step-normalized chi. Changing kweight or the window changes the
   * balance at a fixed lambda. The parameter is unused by legacy endpoint policies; it is not a
   * universal physical constant. See [the fixed-penalty
   * objective](https://rexafs.com/docs/science/autobk/).
   */
  clamp_lambda: number | undefined;
  /**
   * FFT length used inside background removal. Default: 2048; undefined restores this default.
   * Use a positive integer large enough to contain the prepared k grid. It sets the internal R
   * spacing together with kstep; increasing zero-padding refines that grid without adding
   * measured information.
   */
  nfft: number | undefined;
  /**
   * Integer power of k applied in the background Fourier objective. Default: 1; undefined
   * restores this default. Larger powers emphasize high-k residuals and their noise. The
   * returned chi() remains unweighted; the independent XrayFFTF.kweight controls the later
   * displayed transform.
   */
  kweight: number | undefined;
  /**
   * Background window parameter. Default: 0.1; undefined restores this default. For the default
   * Hanning window it controls endpoint taper widths in inverse angstroms. KaiserBessel also
   * uses this numeric value as a dimensionless shape parameter; equal dk does not make
   * different window families equivalent.
   */
  dk: number | undefined;
  /**
   * Ridge strength for the legacy LinearDirect objective. Default: 0.0001; undefined restores
   * this default. Unused by the recommended FixedPenalty model, which solves the specified
   * endpoint-penalized least-squares problem without this additional ridge term. Change only
   * when reproducing a legacy calculation.
   */
  linear_regularization: number | undefined;
  /**
   * Largest accepted condition number of the linear system. Default: 1e8; undefined restores
   * this default. FixedPenalty applies this limit to the column-scaled design matrix and
   * requires a finite value of at least 1. An ill-conditioned or rank-deficient solve throws
   * instead of silently changing the objective.
   */
  linear_condition_limit: number | undefined;
  /**
   * Acceptance threshold for comparing the legacy direct solution residual against its
   * reference residual. Default: 1.05; undefined restores this default. Unused by FixedPenalty.
   * This is a numerical fallback criterion, not a statistical uncertainty or goodness-of-fit
   * probability.
   */
  linear_residual_ratio_limit: number | undefined;
  /**
   * Allow the legacy direct solver to retry with linear_fallback_solver if its checks fail.
   * Default: true; undefined restores this default. Despite the historical name, the chosen
   * fallback need not be Levenberg-Marquardt. FixedPenalty never falls back and ignores this
   * flag.
   */
  linear_fallback_to_lm: boolean | undefined;
  /**
   * Reuse compatible spline geometry, Fourier operators and matrix factorization. Default:
   * true; undefined restores this default. Each spectrum still supplies new data and receives a
   * new solution, with condition checks repeated. Disabling this changes reuse and runtime, not
   * the specified objective.
   */
  linear_workspace_cache: boolean | undefined;
  /**
   * Window family used inside the background Fourier objective. Default: Hanning; undefined
   * restores Hanning. The taper reduces ringing at the selected k boundaries. This setting is
   * independent of the later XrayFFTF window; see FTWindow for the accepted names.
   */
  window: FTWindow | undefined;
  /**
   * Algorithm used to determine background spline coefficients. Recommended default:
   * LinearDirect, required by FixedPenalty; undefined restores it. LegacyLm solves the legacy
   * objective iteratively. TrustRegionDogLeg requires a native Rust feature and throws when
   * processing in the published Wasm package.
   */
  solver: AUTOBKSolver | undefined;
  /**
   * Solver used only when an enabled legacy LinearDirect fallback is needed. Default in Wasm:
   * LegacyLm; undefined also resolves to LegacyLm with the default fallback flag. FixedPenalty
   * ignores this option. TrustRegionDogLeg is unavailable in Wasm, and LinearDirect cannot be
   * its own fallback.
   */
  linear_fallback_solver: AUTOBKSolver | undefined;
  /**
   * Endpoint penalty model. Recommended default: FixedPenalty; undefined restores it.
   * FixedPenalty uses a fixed mean-square endpoint term and requires LinearDirect. Fixed and
   * TwoPass preserve older residual-dependent scaling choices for reproducing historical
   * results; they are different objectives.
   */
  clamp_scale_policy: AUTOBKClampScalePolicy | undefined;
}

/**
 * Named field overrides for new XrayFFTF(options). Omitted fields retain constructor defaults;
 * explicitly assigning undefined uses the resolution described for each field. An unknown key
 * or a non-object options argument throws TypeError. Numerical range and data-dependent checks
 * generally run when the configured processing stage executes.
 */
export interface XrayFFTFOptions {
  /**
   * Sampling and window-construction convention. Default: Input preserves the prepared
   * background k grid. Larch linearly resamples onto a zero-origin grid and extends the window
   * domain as needed. Neither changes the returned background k()/chi(); always pair kwin()
   * with kwin_k().
   */
  grid?: FFTGrid;
  /**
   * Maximum reported R in angstroms. Default: 10.0; undefined restores this default. This
   * limits the r() and chir_*() output arrays, not the internally retained Fourier bins used by
   * ifft(). It does not change the transform amplitude or frequency resolution. ifft()
   * nevertheless needs at least two reported R samples to infer/validate their spacing, so
   * rmax_out=0 is insufficient for a back-transform.
   */
  rmax_out?: number | undefined;
  /**
   * Low-k window parameter. Default: 1.0; undefined restores this default. For taper windows it
   * controls transition geometry in inverse angstroms. KaiserBessel also uses the same numeric
   * value as a dimensionless shape parameter, so it cannot be compared as a universal taper
   * width across all windows.
   */
  dk?: number | undefined;
  /**
   * High-k window parameter. Default: undefined uses dk. It controls the upper transition
   * geometry in inverse angstroms; interpretation depends on the window family. Use the same
   * value as dk for symmetric endpoint settings.
   */
  dk2?: number | undefined;
  /**
   * Lower Fourier window bound in inverse angstroms. Default: 2.0; explicitly assigning
   * undefined uses the first prepared k sample. Both bounds must be finite with kmin < kmax;
   * negative bounds are accepted. Select this above the region where the EXAFS approximation or
   * background subtraction is unreliable.
   */
  kmin?: number | undefined;
  /**
   * Upper Fourier window bound in inverse angstroms. Default: 15.0; explicitly assigning
   * undefined uses the last prepared k sample. Require kmax > kmin. Select this within the
   * useful measured range; high-k noise can dominate after k weighting.
   */
  kmax?: number | undefined;
  /**
   * Power of k applied before the forward transform. Default: 2.0; undefined restores this
   * default. Finite nonnegative values are floored to an integer w. Larger w emphasizes high-k
   * oscillations and noise; for dimensionless chi, the transformed amplitude has units
   * angstrom^(-(w + 1)).
   */
  kweight?: number | undefined;
  /**
   * Forward FFT length N. Default: 2048; undefined restores this default. Require an integer of
   * at least 2. The R spacing is pi / (N * kstep), in angstroms. Use N at least as large as the
   * prepared data: Input truncates excess samples, whereas Larch rejects a window grid that
   * does not fit. Larger zero-padding does not improve experimental resolution.
   */
  nfft?: number | undefined;
  /**
   * k spacing used to scale the transform and label R, in inverse angstroms. Default: undefined
   * infers the first spacing of the prepared k grid (normally 0.05 from AUTOBK defaults). Larch
   * also uses this spacing to resample chi. Input does not resample, so keep it consistent with
   * the input grid. Require a finite positive value. The inferred value is retained in the
   * spectrum's copied FFT settings. After changing the background grid, reassign FFT settings
   * with kstep undefined to infer the new spacing.
   */
  kstep?: number | undefined;
  /**
   * Fourier window family. Constructor default: KaiserBessel; explicitly assigning undefined
   * selects the window routine's Hanning fallback. The window reduces truncation ringing and
   * changes amplitude. No correction for window area or coherent gain is applied; see FTWindow
   * for valid names.
   */
  window?: FTWindow | undefined;
}
/**
 * Settings for converting weighted, windowed chi(k) into complex chi(R).
 *
 * Recommended starting values: kmin=2, kmax=15 inverse angstroms, kweight=2, KaiserBessel,
 * dk=1 and nfft=2048. Adapt the k interval to the useful measured data. The default Input grid
 * preserves the background grid; automatic kstep normally resolves to AUTOBK's 0.05 inverse
 * angstroms.
 *
 * For a uniform zero-origin grid `k[j]=j*kstep`, prepare
 * `g[j] = chi(k[j]) * k[j]^w * window[j]`. The code computes `chiR[m] = (kstep /
 * sqrt(pi)) * sum_j g[j] * exp(-2*pi*i*j*m/N)`, with N=nfft, w=kweight, i^2=-1 and
 * `R[m]=pi*m/(N*kstep)`. Here j indexes prepared k samples and m indexes nonnegative
 * Fourier bins. The sum uses the first N prepared samples and zeros for missing
 * samples. There is no 1/N forward normalization or window-area correction. For dimensionless
 * chi, chi(R) has units angstrom^(-(w+1)).
 *
 * This is the [NumPy unnormalized forward DFT
 * convention](https://numpy.org/doc/stable/reference/routines.fft.html#implementation-details)
 * with the explicit factor in
 * [xftf_fast_nalgebra](https://github.com/Ameyanagi/rexafs/blob/main/crates/rexafs/src/xafs/xrayfft.rs).
 * Scattering phases shift the peaks, so R is not automatically a phase-corrected bond
 * distance; see [Rehr and Albers (2000)](https://doi.org/10.1103/RevModPhys.72.621).
 *
 * set_fft() copies settings. Reassign after editing, and free() this object when finished.
 */
export class XrayFFTF {
  /**
   * Create owned settings with the recommended defaults described below. Browser callers must
   * await init() first. Edit the fields, copy the settings into the appropriate spectrum stage,
   * and free() this object when finished.
   *
   * Automatic fields are resolved on the spectrum's copy during processing; resolved values
   * are not written back into the original settings object. Resolved values stay in the
   * spectrum's settings until those settings are replaced.
   *
   * Named options were added in 0.2.5. Version 0.2.4 uses this constructor without
   * arguments, followed by property assignment.
   */
  constructor(options?: XrayFFTFOptions);
  /**
   * Release this object's Wasm allocation. Do not call methods, read fields or free it again
   * afterwards. Arrays and settings already copied elsewhere remain valid.
   */
  free(): void;
  /**
   * Sampling and window-construction convention. Default: Input preserves the prepared
   * background k grid. Larch linearly resamples onto a zero-origin grid and extends the window
   * domain as needed. Neither changes the returned background k()/chi(); always pair kwin()
   * with kwin_k().
   */
  grid: FFTGrid;
  /**
   * Maximum reported R in angstroms. Default: 10.0; undefined restores this default. This
   * limits the r() and chir_*() output arrays, not the internally retained Fourier bins used by
   * ifft(). It does not change the transform amplitude or frequency resolution. ifft()
   * nevertheless needs at least two reported R samples to infer/validate their spacing, so
   * rmax_out=0 is insufficient for a back-transform.
   */
  rmax_out: number | undefined;
  /**
   * Low-k window parameter. Default: 1.0; undefined restores this default. For taper windows it
   * controls transition geometry in inverse angstroms. KaiserBessel also uses the same numeric
   * value as a dimensionless shape parameter, so it cannot be compared as a universal taper
   * width across all windows.
   */
  dk: number | undefined;
  /**
   * High-k window parameter. Default: undefined uses dk. It controls the upper transition
   * geometry in inverse angstroms; interpretation depends on the window family. Use the same
   * value as dk for symmetric endpoint settings.
   */
  dk2: number | undefined;
  /**
   * Lower Fourier window bound in inverse angstroms. Default: 2.0; explicitly assigning
   * undefined uses the first prepared k sample. Both bounds must be finite with kmin < kmax;
   * negative bounds are accepted. Select this above the region where the EXAFS approximation or
   * background subtraction is unreliable.
   */
  kmin: number | undefined;
  /**
   * Upper Fourier window bound in inverse angstroms. Default: 15.0; explicitly assigning
   * undefined uses the last prepared k sample. Require kmax > kmin. Select this within the
   * useful measured range; high-k noise can dominate after k weighting.
   */
  kmax: number | undefined;
  /**
   * Power of k applied before the forward transform. Default: 2.0; undefined restores this
   * default. Finite nonnegative values are floored to an integer w. Larger w emphasizes high-k
   * oscillations and noise; for dimensionless chi, the transformed amplitude has units
   * angstrom^(-(w + 1)).
   */
  kweight: number | undefined;
  /**
   * Forward FFT length N. Default: 2048; undefined restores this default. Require an integer of
   * at least 2. The R spacing is pi / (N * kstep), in angstroms. Use N at least as large as the
   * prepared data: Input truncates excess samples, whereas Larch rejects a window grid that
   * does not fit. Larger zero-padding does not improve experimental resolution.
   */
  nfft: number | undefined;
  /**
   * k spacing used to scale the transform and label R, in inverse angstroms. Default: undefined
   * infers the first spacing of the prepared k grid (normally 0.05 from AUTOBK defaults). Larch
   * also uses this spacing to resample chi. Input does not resample, so keep it consistent with
   * the input grid. Require a finite positive value. The inferred value is retained in the
   * spectrum's copied FFT settings. After changing the background grid, reassign FFT settings
   * with kstep undefined to infer the new spacing.
   */
  kstep: number | undefined;
  /**
   * Fourier window family. Constructor default: KaiserBessel; explicitly assigning undefined
   * selects the window routine's Hanning fallback. The window reduces truncation ringing and
   * changes amplitude. No correction for window area or coherent gain is applied; see FTWindow
   * for valid names.
   */
  window: FTWindow | undefined;
}

/**
 * Named field overrides for new XrayFFTR(options). Omitted fields retain constructor defaults;
 * explicitly assigning undefined uses the resolution described for each field. An unknown key
 * or a non-object options argument throws TypeError. Numerical range and data-dependent checks
 * generally run when the configured processing stage executes.
 */
export interface XrayFFTROptions {
  /**
   * Largest reported back-transform q in inverse angstroms. Default: 10.0; undefined restores
   * this default. This crops q()/chiq() to the available output range without changing the
   * inverse calculation. Require a finite nonnegative value.
   */
  qmax_out?: number | undefined;
  /**
   * Low-R window parameter. Default: 1.0; undefined restores this default. For taper windows it
   * controls transition geometry in angstroms. KaiserBessel also uses this numeric value as a
   * dimensionless shape parameter; different windows do not have equivalent shape for the same
   * dr.
   */
  dr?: number | undefined;
  /**
   * High-R window parameter. Default: undefined uses dr. It controls the upper transition
   * geometry in angstroms, with interpretation depending on the selected window family. Use
   * matching dr and dr2 for symmetric endpoint settings.
   */
  dr2?: number | undefined;
  /**
   * Lower inverse-transform window bound in angstroms. Default: 0.0; explicitly assigning
   * undefined uses the first input R sample. Require 0 <= rmin < rmax. A nonzero lower bound
   * can exclude low-R background contributions.
   */
  rmin?: number | undefined;
  /**
   * Upper inverse-transform window bound in angstroms. Default: 20.0; explicitly assigning
   * undefined uses the last reported input R sample. Require rmax > rmin. Choose the R interval
   * around the shell contribution of interest; uncorrected Fourier peaks are not directly bond
   * lengths.
   */
  rmax?: number | undefined;
  /**
   * Power of R applied before the inverse transform. Default: 0.0; undefined restores this
   * default. Finite nonnegative values are floored to an integer. Leave at 0 for ordinary shell
   * filtering; a positive value additionally emphasizes larger-R contributions and changes the
   * signal units.
   */
  rweight?: number | undefined;
  /**
   * Inverse FFT length N. Default: 2048; undefined restores this default. Require an integer of
   * at least 2. Keeping kstep automatic resolves output spacing from the existing R grid and N;
   * reducing N discards high-R bins and increasing it pads them with zeros.
   */
  nfft?: number | undefined;
  /**
   * Output q spacing in inverse angstroms. Default: undefined computes pi / (nfft * delta_R),
   * where delta_R is the input R spacing in angstroms. An explicit value must agree with that
   * relationship or processing throws. Leave automatic when changing nfft. The resolved value
   * is retained in the spectrum's copy. After changing the forward R grid, reassign inverse
   * settings whose kstep is undefined to resolve it again.
   */
  kstep?: number | undefined;
  /**
   * R-space window family. Constructor default: KaiserBessel; explicitly assigning undefined
   * selects Hanning. The window selects and tapers the R contributions retained in chiq(). The
   * forward k weight and window are not divided out by the inverse transform.
   */
  window?: FTWindow | undefined;
}
/**
 * Settings for filtering selected R-space contributions and returning a real signal chi(q).
 *
 * Choose rmin/rmax in angstroms to enclose the contributions of interest. Defaults are rmin=0,
 * rmax=20, dr=1, KaiserBessel, rweight=0, nfft=2048 and qmax_out=10 inverse angstroms.
 * Automatic kstep preserves consistency with the input R spacing.
 *
 * rexafs windows and optionally R-weights the complex forward bins, supplies their conjugate
 * negative-frequency partners, and performs a real inverse DFT with scale
 * sqrt(pi)/(kstep*nfft). This implements a real filtered back-transform; forward k-weighting
 * and windowing remain in the result. It is not a general recovery of the original unweighted
 * chi(k), and is not Larch's complex, one-sided inverse representation. See
 * [inverse_fft.rs](https://github.com/Ameyanagi/rexafs/blob/main/crates/rexafs/src/xafs/inverse_fft.rs)
 * and the [DFT normalization
 * convention](https://numpy.org/doc/stable/reference/routines.fft.html#normalization).
 *
 * set_ifft() copies settings. Reassign after editing, and free() this object when finished.
 */
export class XrayFFTR {
  /**
   * Create owned settings with the recommended defaults described below. Browser callers must
   * await init() first. Edit the fields, copy the settings into the appropriate spectrum stage,
   * and free() this object when finished.
   *
   * Automatic fields are resolved on the spectrum's copy during processing; resolved values
   * are not written back into the original settings object. Resolved values stay in the
   * spectrum's settings until those settings are replaced.
   *
   * This class was added in 0.2.5 and is not exported by npm 0.2.4.
   */
  constructor(options?: XrayFFTROptions);
  /**
   * Release this object's Wasm allocation. Do not call methods, read fields or free it again
   * afterwards. Arrays and settings already copied elsewhere remain valid.
   */
  free(): void;
  /**
   * Largest reported back-transform q in inverse angstroms. Default: 10.0; undefined restores
   * this default. This crops q()/chiq() to the available output range without changing the
   * inverse calculation. Require a finite nonnegative value.
   */
  qmax_out: number | undefined;
  /**
   * Low-R window parameter. Default: 1.0; undefined restores this default. For taper windows it
   * controls transition geometry in angstroms. KaiserBessel also uses this numeric value as a
   * dimensionless shape parameter; different windows do not have equivalent shape for the same
   * dr.
   */
  dr: number | undefined;
  /**
   * High-R window parameter. Default: undefined uses dr. It controls the upper transition
   * geometry in angstroms, with interpretation depending on the selected window family. Use
   * matching dr and dr2 for symmetric endpoint settings.
   */
  dr2: number | undefined;
  /**
   * Lower inverse-transform window bound in angstroms. Default: 0.0; explicitly assigning
   * undefined uses the first input R sample. Require 0 <= rmin < rmax. A nonzero lower bound
   * can exclude low-R background contributions.
   */
  rmin: number | undefined;
  /**
   * Upper inverse-transform window bound in angstroms. Default: 20.0; explicitly assigning
   * undefined uses the last reported input R sample. Require rmax > rmin. Choose the R interval
   * around the shell contribution of interest; uncorrected Fourier peaks are not directly bond
   * lengths.
   */
  rmax: number | undefined;
  /**
   * Power of R applied before the inverse transform. Default: 0.0; undefined restores this
   * default. Finite nonnegative values are floored to an integer. Leave at 0 for ordinary shell
   * filtering; a positive value additionally emphasizes larger-R contributions and changes the
   * signal units.
   */
  rweight: number | undefined;
  /**
   * Inverse FFT length N. Default: 2048; undefined restores this default. Require an integer of
   * at least 2. Keeping kstep automatic resolves output spacing from the existing R grid and N;
   * reducing N discards high-R bins and increasing it pads them with zeros.
   */
  nfft: number | undefined;
  /**
   * Output q spacing in inverse angstroms. Default: undefined computes pi / (nfft * delta_R),
   * where delta_R is the input R spacing in angstroms. An explicit value must agree with that
   * relationship or processing throws. Leave automatic when changing nfft. The resolved value
   * is retained in the spectrum's copy. After changing the forward R grid, reassign inverse
   * settings whose kstep is undefined to resolve it again.
   */
  kstep: number | undefined;
  /**
   * R-space window family. Constructor default: KaiserBessel; explicitly assigning undefined
   * selects Hanning. The window selects and tapers the R contributions retained in chiq(). The
   * forward k weight and window are not divided out by the inverse transform.
   */
  window: FTWindow | undefined;
}

/**
 * Select the normalization algorithm and hold an owned copy of its settings.
 *
 * Use PrePostEdge(settings) for customized pre/post-edge fits or new_prepostedge() for
 * automatic defaults. The no-argument MBack factory is a historical empty selector and cannot
 * normalize data. MBACK was unimplemented through version 0.2.9. Copy this method into
 * Spectrum.set_normalization_method(), then free() the wrapper when no longer needed.
 */
export class NormalizationMethod {
  private constructor();
  /**
   * Release this object's Wasm allocation. Do not call methods, read fields or free it again
   * afterwards. Arrays and settings already copied elsewhere remain valid.
   */
  free(): void;
  /**
   * Copy the supplied pre/post-edge configuration into a new owned algorithm wrapper. The input
   * settings are not consumed. Assign the wrapper to Spectrum.set_normalization_method(), then
   * free() it when no longer needed; the spectrum retains its own copy.
   */
  static PrePostEdge(parameters: PrePostEdge): NormalizationMethod;
  /**
   * Create a new owned pre/post-edge normalization wrapper with automatic E0, fit ranges,
   * polynomial order and edge step. It does not process any spectrum. Assign it to
   * Spectrum.set_normalization_method() and free() the wrapper when finished.
   */
  static new_prepostedge(): NormalizationMethod;
  /**
   * Create the historical empty MBack selector. Selecting it makes normalize() throw.
   * Use new_prepostedge() for automatic polynomial normalization. MBACK was unimplemented
   * through version 0.2.9; this no-argument selector remains unusable for normalization.
   * Free this wrapper when finished.
   */
  static new_mback(): NormalizationMethod;
}
/**
 * Select the background-removal algorithm and hold an owned copy of its settings.
 *
 * Use AUTOBK(settings) for a configured spline background or new_autobk() for the recommended
 * defaults. The ILPBkg factory is only a placeholder; it does not implement that algorithm.
 * Copy this method into Spectrum.set_background_method(), then free() the wrapper when no
 * longer needed.
 */
export class BackgroundMethod {
  private constructor();
  /**
   * Release this object's Wasm allocation. Do not call methods, read fields or free it again
   * afterwards. Arrays and settings already copied elsewhere remain valid.
   */
  free(): void;
  /**
   * Copy the supplied AUTOBK configuration into a new owned algorithm wrapper. The input
   * settings are not consumed. Assign the wrapper to Spectrum.set_background_method(), then
   * free() it when no longer needed; the spectrum retains its own copy.
   */
  static AUTOBK(parameters: AUTOBK): BackgroundMethod;
  /**
   * Create a new owned AUTOBK wrapper with the recommended LinearDirect/FixedPenalty defaults,
   * rbkg=1 angstrom and clamp_lambda=0.001. It does not process any spectrum. Assign it to
   * Spectrum.set_background_method() and free() the wrapper when finished.
   */
  static new_autobk(): BackgroundMethod;
  /**
   * Create an owned ILPBkg placeholder for API compatibility. This background method is not
   * implemented and calc_background() throws if selected. Use new_autobk() for supported
   * background removal, and free() any placeholder you create.
   */
  static new_ilpbkg(): BackgroundMethod;
}

/**
 * Mutable absorption spectrum processed by the Rust engine in WebAssembly.
 *
 * Construct from finite, equal-length Float64Arrays containing strictly increasing photon
 * energy in eV and absorption mu. The supplied absorption scale is retained until edge-step
 * normalization. Arrays and stage settings are copied, so callers keep ownership of their
 * inputs.
 *
 * Stages run synchronously and return this same object for chaining. fft() computes missing
 * normalization and AUTOBK stages using the selected settings; ifft() additionally computes the
 * forward transform if absent. Changing or rerunning a stage clears dependent results. A stage
 * error throws; inspect or correct the inputs/settings before retrying.
 *
 * Result getters never run processing. They return independent Float64Array copies, or
 * undefined before their stage runs and after invalidation. Copies remain valid after free().
 * Browser callers must await init() before constructing objects, and should use a Web Worker
 * for long calculations. Release every spectrum and settings/wrapper object with free() when
 * finished.
 *
 * See [processing theory](https://rexafs.com/docs/science/processing/) for equations,
 * assumptions and interpretation.
 */
export class Spectrum {
  /** Correct into an independent unnormalized Spectrum (since 0.2.10). Internal
   * conventional normalization runs automatically; the source stays unchanged.
   * Unknown provenance is explicitly interpreted as fluorescence. Known transmission,
   * prepared norm/flat and repeated correction throw. Supply line and measured
   * surface angles in the model. Call normalize() for separate final polynomial/MBACK
   * normalization. History survives edits; this XANES-only branch rejects background,
   * FFT and wavelets. Array uncertainties are unavailable. Free the new Spectrum. */
  correct_fluorescence(model: FluorescenceCorrection): Spectrum;
  /** Independent historical correction record, or undefined. Edits/normalization
   * never rewrite original correction inputs or remove the XANES-only restriction. */
  fluorescence_correction(): FluorescenceCorrectionResult | undefined;
  /** Acquisition interpretation; unknown means missing evidence. */
  absorption_mode(): AbsorptionMode;
  /** Explicitly revise interpretation without changing arrays/caches. Correction
   * history and restrictions survive. Returns this Spectrum; invalid names throw. */
  set_absorption_mode(mode: AbsorptionMode): this;
  /** Since 0.2.10: spectrum.wavelet(new Wavelet([2, 12])) prepares missing
   * normalization/AUTOBK on a private copy, reusing existing χ. The inclusive
   * interval uses Å⁻¹. Source arrays/settings/caches stay unchanged. Returns an
   * owned native map; invalid coverage/grids and corrected XANES-only input throw.
   * R is not phase-corrected and colors do not imply concentration. */
  wavelet(model: Wavelet): WaveletMap;
  /** Fit a composite XANES model, preparing missing normalization on a private copy (since 0.2.10).
   * Recommended: spectrum.fit_peaks(new PeakFit([-20, 40]).gaussian("p1", { center: 5, area: 2, fwhm: 3 })).
   * Model defaults are Norm, E0-relative eV and 200 iterations. Source arrays, settings,
   * caches and model remain unchanged. Results use retained native points; no smoothing
   * or interpolation occurs. Invalid models, coverage or preparation throw Error.
   * Inspect termination and warnings: a numerical result can be nonconverged.
   * Optional errors are positive independent standard deviations of the SELECTED signal
   * on the original native grid, including excluded points. Raw errors are not propagated.
   * Without errors, covariance uses residual-based variance. Active bounds, deficient rank
   * and nonconvergence withhold local errors. These are conditional, not model confidence.
   * Synchronous; use a Web Worker for large browser fits.
   */
  fit_peaks(model: PeakFit, options?: { errors?: Float64Array }): PeakFitResult;
  /**
   * Measure a point or region without changing this spectrum (since 0.2.10).
   * Recommended: `spectrum.measure("mean", [-20, 30])`. Defaults to normalized
   * mu and E0-relative energy offsets in eV; choose `space: "flat"` explicitly.
   * k is in inverse angstroms; R is in angstroms without phase correction.
   * Missing prerequisite stages run on a private copy using this spectrum's
   * settings; caller arrays, settings and cached results remain unchanged.
   * No extrapolation or display sampling occurs. Mean is the piecewise-linear
   * integral divided by interval width, not the arithmetic sample mean.
   * Returns an owned scalar result with units and the resolved absolute range.
   * Throws on invalid input, preparation failure, or missing range coverage.
   * Optional independent point errors apply to point/mean/integral only; see
   * SpectrumMeasurementOptions.errors. No uncertainty is inferred by default.
   */
  measure(operation: "point", coordinates: number, options?: SpectrumMeasurementOptions): MeasurementResult;
  /**
   * Measure a point or region without changing this spectrum (since 0.2.10).
   * Recommended: `spectrum.measure("mean", [-20, 30])`. Defaults to normalized
   * mu and E0-relative energy offsets in eV; choose `space: "flat"` explicitly.
   * k is in inverse angstroms; R is in angstroms without phase correction.
   * Missing prerequisite stages run on a private copy using this spectrum's
   * settings; caller arrays, settings and cached results remain unchanged.
   * No extrapolation or display sampling occurs. Mean is the piecewise-linear
   * integral divided by interval width, not the arithmetic sample mean.
   * Returns an owned scalar result with units and the resolved absolute range.
   * Throws on invalid input, preparation failure, or missing range coverage.
   * Optional independent point errors apply to point/mean/integral only; see
   * SpectrumMeasurementOptions.errors. No uncertainty is inferred by default.
   */
  measure(operation: "mean" | "integral" | "maximum", coordinates: readonly [number, number], options?: SpectrumMeasurementOptions): MeasurementResult;
  /**
   * Copy measured photon energy in eV and absorption mu into a new, initially unprocessed
   * spectrum. Both inputs must be Float64Arrays of equal length, with at least two finite
   * samples and strictly increasing energy. Duplicate or decreasing energies are rejected.
   * Throws TypeError for other array types, and Error for invalid data or uninitialized browser
   * Wasm. Processing may need more samples than construction; free() the spectrum when
   * finished.
   */
  constructor(energy: Float64Array, mu: Float64Array);
  /**
   * Create a new spectrum by copying measured energy in eV and absorption mu. Equivalent to new
   * Spectrum(energy, mu), including Float64Array type checks and the requirement for at least
   * two finite, strictly increasing energy samples of matching length. No processing runs
   * automatically at construction; free() the result when finished.
   */
  static from_arrays(energy: Float64Array, mu: Float64Array): Spectrum;
  /**
   * Release this object's Wasm allocation. Do not call methods, read fields or free it again
   * afterwards. Arrays and settings already copied elsewhere remain valid.
   */
  free(): void;
  /**
   * Replace measured energy (eV) and absorption mu with independent copies. Uses the same
   * validation as the constructor and preserves the previous spectrum if input validation
   * fails. Clears E0 and all calculated results while retaining stage settings. Returns this
   * spectrum; recompute the desired stages after replacement.
   */
  set_spectrum(energy: Float64Array, mu: Float64Array): this;
  /**
   * Assign a finite edge energy in eV. Clears normalization, background, forward and inverse
   * results, and updates the selected normalization/background energy origins. The value is not
   * checked against the measured range until processing. Throws TypeError for a non-number or
   * RangeError for a nonfinite value. Returns this spectrum.
   */
  set_e0(e0: number): this;
  /**
   * Copy the selected normalization method and clear normalization, background, forward and
   * inverse results. A specified method E0 overrides the spectrum E0; otherwise the existing E0
   * is retained. The caller keeps ownership of the settings and wrapper and may free them after
   * assignment. Later edits require reassignment. Returns this spectrum.
   *
   * Omitting the argument, undefined or null restores automatic pre/post-edge settings while
   * retaining the selected E0. These reset forms work in 0.2.4 and later. For custom
   * settings, 0.2.4 accepts a NormalizationMethod wrapper; direct PrePostEdge settings
   * were added in 0.2.5.
   */
  set_normalization_method(method?: PrePostEdge | MBack | NormalizationMethod | null): this;
  /** Copy the latest full MBACK result, or undefined when absent/invalidated. */
  mback_result(): MbackResult | undefined;
  /**
   * Copy the selected background method and clear background, forward and inverse results while
   * retaining normalization. The caller keeps ownership of the settings and wrapper and may
   * free them after assignment. Later edits require reassignment. Returns this spectrum.
   *
   * Omitting the argument, undefined or null restores default AUTOBK settings. These reset
   * forms work in 0.2.4 and later. For custom settings, 0.2.4 accepts a BackgroundMethod
   * wrapper; direct AUTOBK settings were added in 0.2.5. This
   * does not reset forward or inverse configuration values that were already resolved
   * automatically.
   */
  set_background_method(method?: AUTOBK | BackgroundMethod | null): this;
  /**
   * Copy inverse-transform settings and clear q()/chiq() while preserving normalization,
   * background and forward results. Settings can be freed after assignment; later edits require
   * reassignment. Invalid R ranges or inconsistent kstep/nfft are reported when ifft() runs.
   * Returns this spectrum. Assign settings with kstep undefined to resolve spacing again after
   * changing the forward grid. This method and XrayFFTR were added in 0.2.5.
   */
  set_ifft(parameters: XrayFFTR): this;
  /**
   * Copy forward-transform settings and clear r(), chir_*(), kwin(), kwin_k(), q() and chiq().
   * Normalization and background k()/chi() are preserved. Settings can be freed after
   * assignment; later edits require reassignment. Invalid numerical settings are reported when
   * fft() runs. Returns this spectrum. Assign settings with kstep undefined to request fresh
   * spacing inference, for example after changing AUTOBK.kstep. Inverse settings are retained;
   * if their spacing was already resolved, they may also need replacement before ifft().
   */
  set_fft(parameters: XrayFFTF): this;
  /**
   * Return the selected or detected absorption-edge energy in eV, or undefined before
   * assignment/detection. Reading this value does not detect an edge or run any processing.
   */
  e0(): number | undefined;
  /**
   * Estimate the absorption-edge energy from the derivative of measured mu(E), including local
   * smoothing/refinement. Clears normalization and all downstream results, then returns this
   * spectrum. Throws if the input cannot be used for edge detection. Inspect the result for
   * noisy, multiple-edge or unusual spectra and use set_e0() for an explicit choice.
   */
  find_e0(): this;
  /**
   * Fit the selected pre/post-edge model, find E0 if needed, and calculate dimensionless
   * norm()/flat() plus pre_edge()/post_edge() baselines on the original energy grid. Clears
   * background and all Fourier results even when normalization was already present. Uses
   * automatic PrePostEdge defaults if no method was selected. Throws on unsupported methods or
   * failed baseline fits. Returns this spectrum.
   */
  normalize(): this;
  /**
   * Fit the selected smooth background and calculate unweighted, dimensionless chi(k) on k().
   * Runs missing normalization first and uses default AUTOBK if no background method was
   * selected. Clears forward and inverse results on every call. Throws on invalid parameters,
   * unsupported methods, insufficient data or a failed spline solve. Returns this spectrum.
   */
  calc_background(): this;
  /**
   * Calculate complex chi(R) from chi(k), computing missing normalization and background first.
   * Defaults: k=2..15 inverse angstroms, kweight=2, KaiserBessel, nfft=2048 and kstep inferred
   * from the background grid. The unnormalized forward DFT is multiplied by kstep/sqrt(pi),
   * with no additional 1/N; XrayFFTF explains the formula and units. Clears inverse results and
   * throws on invalid settings or failed prerequisite stages. Returns this spectrum.
   */
  fft(): this;
  /**
   * Calculate real chi(q) by windowing the retained complex Fourier bins in R and performing a
   * conjugate-symmetric inverse transform. Runs missing forward and prerequisite stages first.
   * Forward k-weighting and windowing remain in the output, so this is not generally unweighted
   * chi(k). Throws on inconsistent transform settings or failed prerequisite stages. Returns
   * this spectrum. At least two reported R samples are required even though filtering uses the
   * full internal Fourier bins; rmax_out=0 therefore fails. When reusing a spectrum with a
   * different forward grid, reset previously resolved inverse settings with set_ifft()
   * (added in 0.2.5), or create a fresh spectrum in 0.2.4.
   */
  ifft(): this;
  /**
   * Clear normalization, background, forward and inverse calculated arrays without discarding
   * the measured inputs, selected E0 or stage settings. User-specified edge-step overrides are
   * retained; a previously estimated step is recomputed by the next normalization. Returns this
   * spectrum. Subsequent getters return undefined until their stages run again. Resolved
   * automatic settings, such as FFT kstep, are retained. Reassign the affected stage settings
   * to request fresh automatic values for changed input grids.
   */
  invalidate_derived(): this;
  /**
   * Return an independent copy of the uniform background k axis in inverse angstroms, beginning
   * at zero and paired with chi(). Its spacing is AUTOBK.kstep (default 0.05). Returns
   * undefined before background removal or after invalidation; this getter never runs a stage.
   */
  k(): Float64Array | undefined;
  /**
   * Return an independent copy of unweighted EXAFS chi(k) = (mu - smooth background) /
   * edge_step, dimensionless and paired with k(). The measured absorption and smooth background
   * are resampled according to the selected background method. Returns undefined before
   * background removal or after invalidation. Forward kweight and window settings do not change
   * this array.
   */
  chi(): Float64Array | undefined;
  /**
   * Return an independent copy of dimensionless normalized absorption, (mu - pre_edge) /
   * edge_step, on the original input energy grid. Here pre_edge is the fitted baseline and
   * edge_step is the selected or fitted absorption jump. Returns undefined before normalization
   * or after invalidation; does not calculate missing results.
   */
  norm(): Float64Array | undefined;
  /**
   * Return an independent copy of dimensionless flattened absorption on the input energy grid.
   * Above E0 this subtracts the fitted post-edge trend from norm(), with an offset that
   * preserves the value at the edge; below E0 it equals norm(). This is a
   * presentation/near-edge quantity, not the background chi(k). Returns undefined before
   * normalization or after invalidation.
   */
  flat(): Float64Array | undefined;
  /**
   * Return an independent copy of the fitted pre-edge baseline, in the same units as input mu
   * and evaluated across the entire original energy grid. The fit uses the selected pre-edge
   * interval, and its extrapolation is subtracted during normalization. Returns undefined
   * before normalization or after invalidation.
   */
  pre_edge(): Float64Array | undefined;
  /**
   * Return an independent copy of the fitted post-edge baseline in input mu units, evaluated on
   * the original energy grid. It includes the pre-edge baseline plus the fitted polynomial for
   * pre-edge-subtracted absorption. It determines the edge step and flattening trend; it is not
   * the AUTOBK background. Returns undefined before normalization or after invalidation.
   */
  post_edge(): Float64Array | undefined;
  /**
   * Return an independent copy of the reported Fourier R axis in angstroms, paired with
   * chir_mag(), chir_real() and chir_imag(). Its spacing is pi/(nfft*kstep), and its extent is
   * limited by rmax_out and the available positive-frequency bins. Peaks are not
   * phase-corrected bond lengths. Returns undefined before fft() or after invalidation.
   */
  r(): Float64Array | undefined;
  /**
   * Return an independent copy of the dimensionless forward window values, paired with
   * kwin_k(). These values exclude the k^kweight factor. The array can have a different
   * length/grid from background k()/chi() with grid=Larch. Returns undefined before fft() or
   * after invalidation.
   */
  kwin(): Float64Array | undefined;
  /**
   * Return an independent copy of the forward window axis in inverse angstroms, paired with
   * kwin(). Input uses the prepared background grid; Larch returns its resampled and possibly
   * extended window grid. This getter does not alter the background k()/chi() arrays. Returns
   * undefined before fft() or after invalidation.
   */
  kwin_k(): Float64Array | undefined;
  /**
   * Return an independent copy of the magnitude sqrt(real^2 + imag^2) of complex chi(R), paired
   * with r(). For dimensionless chi and forward kweight w, units are angstrom^(-(w+1)); at w=2
   * they are inverse cubic angstroms. No peak-height or window-area normalization is applied.
   * Returns undefined before fft() or after invalidation.
   */
  chir_mag(): Float64Array | undefined;
  /**
   * Return an independent copy of the real component of chi(R), paired with r(). Units are
   * angstrom^(-(w+1)), where w is the forward kweight and chi is dimensionless. Uses the
   * negative-exponent forward DFT with amplitude factor kstep/sqrt(pi). Returns undefined
   * before fft() or after invalidation.
   */
  chir_real(): Float64Array | undefined;
  /**
   * Return an independent copy of the imaginary component of chi(R), paired with r(). Units are
   * angstrom^(-(w+1)), where w is the forward kweight and chi is dimensionless. Its sign
   * follows exp(-2*pi*i*j*m/nfft); reversing the Fourier convention changes that sign. Returns
   * undefined before fft() or after invalidation.
   */
  chir_imag(): Float64Array | undefined;
  /**
   * Return an independent copy of the real back-transform axis in inverse angstroms, beginning
   * at zero and paired with chiq(). Its spacing follows the inverse FFT settings and the
   * forward R grid; q distinguishes this possibly filtered/resized grid from background k().
   * Returns undefined before ifft() or after invalidation.
   */
  q(): Float64Array | undefined;
  /**
   * Return an independent copy of the real R-filtered signal, paired with q(). Forward
   * k-weighting and windowing remain, so this is not generally unweighted chi(k). With
   * dimensionless chi, forward kweight w and inverse rweight v, units are angstrom^(v-w);
   * ordinary v=0 retains the units of k^w*chi. Returns undefined before ifft() or after
   * invalidation.
   */
  chiq(): Float64Array | undefined;
}

/** Source axis conversion to eV; no magnitude-based unit guessing (since 0.2.6). */
export type EnergyConversion =
  | { kind: 'ev' }
  | { kind: 'kev' }
  | {
      kind: 'offset_ev';
      /** Finite origin in eV: absolute energy = source energy + offset_ev.
       * FDMNES detection uses its declared E_edge. The source axis stays unchanged. */
      offset_ev: number;
    }
  | {
      kind: 'bragg';
      /** Positive lattice-plane spacing in angstroms. Uses E = hc/(2 d sin(theta)). */
      d_spacing: number;
      /** Degrees per axis unit: 1 for degrees, 180/pi for radians. */
      degrees_per_unit: number;
    };
/** Exact, case-sensitive column name or zero-based index. Duplicate names require indices. */
export type ColumnSelector = string | number;

/**
 * Exact column names or zero-based detector roles. Transmission is ln(incident/transmitted), with
 * nonzero matching polarity and matching units. Ratio is sum(detectors)/incident
 * with a nonzero monitor. No dark-current, gain or dead-time correction is inferred.
 * Direct copies the original signal and scale. See
 * [Newville (2014)](https://doi.org/10.2138/rmg.2014.78.2) for transmission assumptions.
 */
export type SignalConversion<Column extends ColumnSelector = ColumnSelector> =
  | { kind: 'direct'; column: Column }
  | { kind: 'transmission'; incident: Column; transmitted: Column }
  | { kind: 'ratio'; detectors: Column[]; incident: Column };
/** Explicit, copied selection for converting one original scan.
 * Column defaults to ColumnSelector (string | number). Detected candidates use
 * SpectrumMapping<number>, since the reader has already resolved their indices.
 */
export interface SpectrumMapping<Column extends ColumnSelector = ColumnSelector> {
  /** Exact name or zero-based index of the energy or calibrated angle column. */
  energy_column: Column;
  /** Source axis conversion to electronvolts. */
  energy: EnergyConversion;
  /** Selected detector arithmetic, without normalization or background removal. */
  signal: SignalConversion<Column>;
}
/** Header-supported signal choice; several choices require explicit selection. */
export interface SignalCandidate {
  /** Display label for this signal. */
  name: string;
  /** Fully specified axis and signal conversion. */
  mapping: SpectrumMapping<number>;
}
/** Original owned numeric channel in acquisition order. */
export interface MeasurementColumn {
  /** Source label, or column_N when absent (N is one-based). */
  name: string;
  /** Declared source units; null means absent. */
  units: string | null;
  /** Raw samples. JSON snapshots encode nonfinite source values as null. */
  values: (number | null)[];
}
/** One scan or project group; reading alone does not convert detector values. */
export interface MeasurementScan {
  /** Original scan identifier or dataset-group path. */
  id: string;
  /** Display label from the source. */
  label: string;
  /** Channels in source order, retaining units and repeated energies. */
  columns: MeasurementColumn[];
  /** Original header, or complete XTUNES record text, retained for interpretation. */
  header: string;
  /** Extracted metadata. XTUNES ordered_parameters holds ordered JSON section/key/value triples. */
  metadata: Record<string, string>;
  /** Detected choices; empty means that explicit mapping is required. */
  signals: SignalCandidate[];
  /** Unit assumptions, conflicting metadata and historical-format observations. */
  warnings: string[];
}
/** Numeric dataset or archived Larix/XTUNES result; its quantity may not be absorption. */
export interface MeasurementDataset {
  /** HDF5 path, XTUNES table path, or Larix /symbol/attribute path (JSON Pointer escaping: ~0 for ~ and ~1 for /). */
  path: string;
  /** Dimensions in HDF5 order; empty means scalar. */
  shape: number[];
  /** Row-major values; nonfinite values appear as null in snapshots. */
  values: (number | null)[];
  /** Imaginary components matching values and shape for a complex Larix array;
   * null for real data. Values contains the real components. */
  imaginary: (number | null)[] | null;
  /** Source attributes. Larix retains exact numeric bytes in larix.bytes_base64
   * with NumPy dtype in larix.dtype, including integers rounded by the f64 view. */
  attributes: Record<string, string>;
}
/** Independent snapshot of a universal import; editing it does not change Rust data. */
export interface MeasurementDocument {
  /** Content-detected format family. */
  format: string;
  /** All recovered scans, including those requiring manual mapping. */
  scans: MeasurementScan[];
  /** HDF5 datasets and saved Larix/XTUNES arrays, retaining shapes and independent grids. */
  datasets: MeasurementDataset[];
  /** Container provenance. Larix uses larix.session_text, larix.command_history
   * and larix.symbol_order; commands and saved Python objects remain inert text. */
  metadata: Record<string, string>;
  /** Encoding, container and unreadable HDF5 alias-group diagnostics. */
  warnings: string[];
}
/** Convenient scan and column selection (since 0.2.6). Names must be exact and unique.
 * Supply energy and exactly one of mu, it or iff; it/iff also require i0.
 * Conflicting/incomplete options throw. Omit column options to use automatic detection.
 */
export interface MeasurementOptions {
  /** Zero-based scan index; defaults to 0. */
  scan?: number;
  /** Axis name or zero-based index; required with explicit signal roles. */
  energy?: ColumnSelector;
  /** Override detected axis calibration; omitted retains detected/declared units. Unknown units require a choice. */
  energy_unit?: 'eV' | 'keV';
  /** Stored absorption column; cannot be combined with i0, it or iff. */
  mu?: ColumnSelector;
  /** Incident monitor, required for it or iff. */
  i0?: ColumnSelector;
  /** Transmitted intensity; produces ln(i0 / it), using the shared conversion checks. */
  it?: ColumnSelector;
  /** Fluorescence/yield detector or explicit list; summed then divided by i0, without corrections. */
  iff?: ColumnSelector | ColumnSelector[];
}

/**
 * Owned universal reader (since 0.2.6). Supports text/CSV, beamline layouts,
 * historical binary, Athena Perl/JSON, Larix 1.0 sessions, XTUNES, gzip and HDF5. Browser callers first await
 * init(). No filesystem/network access, processing or input mutation occurs.
 * Input and expanded gzip text are each limited to 256 MiB; HDF5 numeric values
 * have a 256 MiB decoded budget. Gzip requires one complete member with no
 * trailing data. Malformed input throws Error. Inspect warnings
 * and choose a scan and signal; detector images require reduction/calibration.
 */
export class Measurement {
  /** Parse UTF-8 text or binary bytes into independent native storage. */
  constructor(data: string | Uint8Array);
  /** Copy metadata and raw arrays. Nonfinite source cells are represented by null. */
  readonly document: MeasurementDocument;
  /**
   * Copy converted energy in eV and signal to Float64Arrays in acquisition order.
   * scan defaults to 0. Omit mapping only when there is exactly one detected
   * signal. Select a candidate's mapping or supply exact names or zero-based roles.
   * Rejects invalid indices, conflicting roles, nonfinite selected cells,
   * nonpositive energy, invalid Bragg calibration and invalid intensity ratios.
   * Duplicates and source order remain; Spectrum construction requires strictly
   * increasing unique energy. No processing or cached results are changed.
   */
  arrays(scan?: number, mapping?: SpectrumMapping): { energy: Float64Array; mu: Float64Array };
  /** Select columns with exact names or indices, for example arrays({energy:"energy", i0:"I0", it:"It"}).
   * Missing/duplicate names fail. Retains the same owned arrays, ordering, conversion checks and
   * no-processing behavior as the positional overload. Cannot combine options with a mapping.
   */
  arrays(options: MeasurementOptions): { energy: Float64Array; mu: Float64Array };
  /**
   * Append copied real dataset vectors in path order; return the new scan index.
   * Requires at least two distinct nonempty vectors of equal length. Invalid
   * paths/shapes and complex arrays throw. Multidimensional detector arrays require reduction.
   * No processing occurs; original scans and datasets remain unchanged.
   */
  select_datasets(paths: string[]): number;
  /** Release native data. Copied arrays remain valid; repeated free() is harmless. */
  free(): void;
}
/**
 * Parse measurement content with the shared Rust reader (since 0.2.6).
 * Accepts UTF-8 text or Uint8Array, including Node Buffers and browser file bytes.
 * Returns owned Measurement storage; free it after copying the arrays you need.
 * Same limits/errors as Measurement. No filesystem/network access or processing
 * occurs. Example: read_measurement(new Uint8Array(await file.arrayBuffer())).
 */
export function read_measurement(data: string | Uint8Array): Measurement;

/** Scalar spectrum measurement options (since 0.2.10); defaults prepare Norm on a copy. */
export interface SpectrumMeasurementOptions {
  /** Selected signal. Default: norm. mu retains original units; flat is dimensionless. */
  space?: "mu" | "norm" | "flat" | "chi" | "fourier";
  /** Default: e0 for energy, absolute for k/R. E0 means offsets in eV. k/R require absolute. */
  origin?: "e0" | "absolute";
  /** Nonnegative integer exponent on k, 0–255. Default: 0. Applies only to chi. */
  kweight?: number;
  /** Independent standard deviations on the selected signal's native grid, in its units.
   * Raw-count errors are NOT propagated through normalization or Fourier transforms.
   * Requires finite nonnegative values matching that grid. Supports point, mean and
   * integral; maximum rejects this model. Axis, E0 and settings are treated as exact;
   * no correlations or confidence intervals are inferred. Omit for unknown errors. */
  errors?: Float64Array;
}
/** Owned native scalar result. JSON serialization preserves its complete definition. */
export interface MeasurementResult {
  /** Finite scalar in unit. */
  value: number;
  /** Signal unit for mean/maximum/point; signal times axis unit for integral. */
  unit: string;
  /** Resolved absolute native-axis bounds: eV, inverse angstroms, or angstroms. */
  range: [number, number];
  /** Absolute point/maximum position, otherwise null. */
  position: number | null;
  /** Propagated independent standard error, or null. Not a confidence interval. */
  standard_error: number | null;
  /** Resolved absorption edge in eV, or null when unnecessary. */
  e0_ev: number | null;
  /** Core definition for reproducible storage; coordinates retain their requested origin. */
  measurement: {
    metric: { Point: { x: number } } | { Mean: { start: number; end: number } }
      | { Integral: { start: number; end: number } } | { Maximum: { start: number; end: number } };
    space: "Mu" | "Norm" | "Flat" | "Fourier" | { Chi: { kweight: number } };
    origin: "E0" | "Absolute";
  };
}

/** Immutable composite XANES peak definition (since 0.2.10).
 * Start with new PeakFit([-20, 40]).gaussian("p1", { center: 5, area: 2, fwhm: 3 }).linear_baseline().
 * Defaults are Norm and E0-relative eV. Builders return NEW definitions; inputs
 * remain unchanged. Missing normalization runs on a copy. No smoothing or
 * chemical/component-count assignment is performed. Default bounds keep centers
 * in the interval, areas nonnegative and widths positive. Inspect termination
 * and warnings: local covariance is conditional on the selected model.
 */
export class PeakFit {
  /** Create an empty Norm model over inclusive E0-relative eV; add components before fitting. */
  constructor(range: readonly [number, number]);
  /** Release this definition's native memory. Do not use it afterwards. Other copies remain valid. */
  free(): void;
  /** Use dimensionless flattened mu; prerequisites run on a copy. Returns a new model. */
  flat(): PeakFit;
  /** Use the original mapped absorption signal and its units. Returns a new model. */
  raw_mu(): PeakFit;
  /** Interpret ranges, centers and baseline references as absolute eV. Returns a new model. */
  absolute(): PeakFit;
  /** Use offsets from this fixed reference energy in eV. Returns a new model. */
  reference(energy_ev: number): PeakFit;
  /** Add a Gaussian: center/FWHM in eV, whole-axis area in signal units times eV. Returns a new model. */
  gaussian(name: string, options: PeakOptions): PeakFit;
  /** Add a Lorentzian with whole-axis area and FWHM in eV. Returns a new model. */
  lorentzian(name: string, options: PeakOptions): PeakFit;
  /** Add a common-FWHM mixture; fraction is the Lorentzian share from zero to one. Returns a new model. */
  pseudo_voigt(name: string, options: PseudoVoigtOptions): PeakFit;
  /** Add a true Voigt with independent Gaussian/Lorentzian FWHM in eV. Returns a new model. */
  voigt(name: string, options: VoigtOptions): PeakFit;
  /** Add height*(1+erf((E-center)/scale))/2; scale is positive eV. Returns a new model. */
  erf_step(name: string, options: StepOptions): PeakFit;
  /** Add height*(1/2+atan((E-center)/scale)/pi); scale is positive eV. Returns a new model. */
  arctan_step(name: string, options: StepOptions): PeakFit;
  /** Add a fitted constant named baseline, in signal units. Returns a new model. */
  constant_baseline(offset?: number): PeakFit;
  /** Add baseline = offset+slope*E_offset; slope is signal units/eV. Returns a new model. */
  linear_baseline(options?: { offset?: number; slope?: number }): PeakFit;
  /** Exclude an inclusive interval in model coordinates; masked gaps are not integrated. */
  exclude(range: readonly [number, number]): PeakFit;
  /** Replace an EXISTING parameter (e.g. p1_center or p1_width); returns a new model.
   * Bounds default to unbounded: supply them explicitly to retain restrictions.
   * An expression is a restricted tie, not executable code, and overrides vary.
   * Unknown names fail immediately; domains/dependencies are checked when fitting.
   */
  parameter(name: string, value: number, options?: PeakParameterOptions): PeakFit;
  /** Make a named peak part of the baseline, excluding it from the weighted center; steps cannot change role. */
  as_baseline(name: string): PeakFit;
  /** Positive optimizer limits, default 200 iterations and 1e-10 tolerance; returns a new model. */
  solver(options?: { max_iterations?: number; tolerance?: number }): PeakFit;
  /** Evaluate at absolute energy in eV without fitting/masking. Relative models require e0; returns an owned array. */
  evaluate(energy: Float64Array, options?: { e0?: number }): Float64Array;
  /** Initialize baseline-role variables outside peak intervals in model coordinates.
   * Returns a new starting model. Source, final masks and this definition stay unchanged.
   */
  initialize_baseline(spectrum: Spectrum, peak_intervals: ReadonlyArray<readonly [number, number]>): PeakFit;
  /** Independent unweighted fits from this starting model, one outcome per input.
   * A bad frame keeps an error row and does not stop later frames. Inputs are unchanged.
   * Runs synchronously; use a Web Worker for large browser batches.
   */
  fit_batch(spectra: readonly Spectrum[]): PeakFitOutcome[];
  /** Serialize the complete initial definition, including constraints and masks. */
  to_json(): string;
  /** Restore and validate a complete definition. Browser init must have completed. */
  static from_json(json: string): PeakFit;
}
/** Optional fixed values, bounds and restricted ties for an existing model parameter. */
export interface PeakParameterOptions {
  /** Independently vary this parameter, default true; a tie overrides this. */
  vary?: boolean;
  /** Inclusive minimum/maximum in parameter units; null means unbounded, the default. */
  bounds?: readonly [number | null, number | null];
  /** Restricted expression in other parameter names; never external code. */
  expression?: string;
}
/** One batch outcome, including failures. A nonconverged numerical result remains a result. */
export type PeakFitOutcome =
  | { index: number; result: PeakFitResult; error: null }
  | { index: number; result: null; error: string };
/** Owned numerical result (since 0.2.10). Arrays are ordinary JavaScript copies.
 * Editing the displayed values never changes the retained JSON or the input spectrum.
 * Local errors are conditional, not model-selection confidence intervals.
 */
export interface PeakFitResult {
  /** Initial model; each access returns a new native copy. Release it with free() when finished. */
  readonly definition: PeakFit;
  /** Copy fitted values for explicit reuse; caller owns the returned model. */
  fitted_model(): PeakFit;
  /** Named final values; parameter centers retain the model's coordinate origin. */
  parameters: Record<string, number>;
  /** Conditional local errors, null when unavailable or not independently estimated. */
  parameter_errors: Record<string, number | null>;
  /** Component curves/summaries in model order. */
  components: PeakContribution[];
  /** Full native snapshot with initial/final constraints, masks and diagnostics. */
  to_json(): string;
  /** Resolved energy origin in eV, added to parameter centers/reference energies. */
  origin_ev: number;
  /** Absolute energy in eV, only the native points used by this fit. */
  energy: number[];
  /** Original zero-based point indices; preserves masks and sampling provenance. */
  source_indices: number[];
  /** Selected representation's measured values, in its signal units. */
  data: number[];
  /** Joint baseline + peaks + steps, in the same signal units. */
  model: number[];
  /** Unweighted data minus model, in signal units (also for weighted fits). */
  residual: number[];
  /** Supplied selected-space standard deviations on the fitted points, if any. */
  standard_deviation: number[] | null;
  /** Sum of squared residuals, divided by supplied standard deviations if present. */
  objective: number;
  /** Number of fitted native data points (not EXAFS independent-point estimates). */
  points: number;
  /** Number of independent varying parameters; expression ties are excluded. */
  free_parameters: number;
  /** points − free_parameters. Fits with fewer points than variables are rejected. */
  degrees_of_freedom: number;
  /** Weighted numerical Jacobian rank under a 1e-10 relative singular-value cutoff. */
  jacobian_rank: number;
  /** Sorted independent parameter names defining covariance/correlation axes. */
  covariance_names: string[];
  /** Local covariance; absolute-error scaling when standard deviations were given,
   * otherwise multiplied by objective/degrees_of_freedom. */
  covariance: number[][] | null;
  /** Dimensionless correlations corresponding to covariance_names. Absent when
   * any conditional variance is zero; the warning explains that case. */
  correlation: number[][] | null;
  /** Why covariance/standard errors were withheld, rather than replaced by zero. */
  uncertainty_unavailable: string | null;
  /** Model peak-area-weighted center in absolute eV, excluding baseline/steps. */
  peak_center_ev: number | null;
  /** Conditional error in that center, using full parameter covariance. */
  peak_center_standard_error_ev: number | null;
  /** Explicit numerical termination category. */
  termination: "FixedModel" | "Converged" | "NotConverged" | "Cancelled";
  /** Solver-specific termination detail, retained verbatim for diagnosis. */
  termination_detail: string;
  /** Number of residual-vector evaluations during optimization, including numerical derivatives. */
  evaluations: number;
  /** Active bounds and other model/uncertainty limitations. */
  warnings: string[];
}
/** One peak, step or baseline contribution on retained native points. */
export interface PeakContribution {
  /** Stable component identity from the initial definition. */
  name: string;
  /** Scientific role, independent of mathematical shape. */
  role: "Peak" | "Baseline" | "Edge";
  /** Mathematical shape used for evaluation. */
  shape: "Gaussian" | "Lorentzian" | "PseudoVoigt" | "Voigt" | "ErfStep" | "ArctanStep" | "Constant" | "Linear";
  /** Component values at the result's absolute-energy points, in signal units. */
  curve: number[];
  /** Peak/step center in absolute eV; None for polynomial baselines. */
  center_ev: number | null;
  /** Whole-axis model area in signal units × eV; None for steps/polynomials. */
  area: number | null;
  /** Peak contribution at its center, excluding all other components. */
  height: number | null;
  /** Peak FWHM in eV; true Voigt uses a numerical half-height root. */
  fwhm_ev: number | null;
  /** Conditional errors propagated with the full joint covariance; absent when
   * local uncertainty is unavailable or the quantity does not apply. */
  center_standard_error_ev: number | null;
  /** Conditional whole-axis area error, in signal units × eV. */
  area_standard_error: number | null;
  /** Conditional peak-height error, in signal units. */
  height_standard_error: number | null;
  /** Conditional FWHM error, in eV, including both true-Voigt width parameters. */
  fwhm_standard_error_ev: number | null;
  /** Trapezoidal component integral over included native-grid segments only.
   * Masked gaps are not bridged; this is not its whole-axis analytic area. */
  sampled_integral: number;
}

/** Initial Gaussian/Lorentzian values; units follow the selected representation. */
export interface PeakOptions {
  /** Initial center in eV under the chosen origin: E0 offsets by default. */
  center: number;
  /** Whole-axis analytic area, signal units times eV; nonnegative by default. */
  area: number;
  /** Full width at half maximum in eV; strictly positive. */
  fwhm: number;
}
/** Common-width Gaussian/Lorentzian mixture. */
export interface PseudoVoigtOptions extends PeakOptions {
  /** Lorentzian share from zero to one; zero is Gaussian and one Lorentzian. */
  fraction: number;
}
/** True convolution with separate Gaussian/Lorentzian widths. */
export interface VoigtOptions {
  /** Center in eV under the selected origin, E0 offsets by default. */
  center: number;
  /** Whole-axis analytic area in signal units times eV, nonnegative by default. */
  area: number;
  /** Gaussian FWHM in eV. One width may be fixed to zero, not both. */
  gaussian_fwhm: number;
  /** Lorentzian FWHM in eV. Combined FWHM is computed in the result. */
  lorentzian_fwhm: number;
}
/** Initial values for an absorption-edge step; a step has no finite peak area. */
export interface StepOptions {
  /** Center in eV under the selected origin, E0 offsets by default. */
  center: number;
  /** Change between asymptotes in the selected signal units. */
  height: number;
  /** Positive eV scale in erf((E-center)/scale) or atan((E-center)/scale); not a peak FWHM. */
  scale: number;
}

/** Exact offline provider/table identity. Retain it for reproducible processing. */
export interface AtomicReference {
  /** Provider/profile version and actual decoded-data SHA-256. */
  data: { provider: string; data_version: string; data_sha256: string };
  /** Numerical table and interpolation/contribution profile. */
  table: "ChantlerF2LogLogV1" | "ElamTotalV1" | "ElamTransitionsV1";
}
/** Explicit bounds for the optional MBACK erfc background, not a sample correction. */
export interface MbackErfcOptions {
  /** Positive increasing width bounds in eV. */
  width: [number, number];
  /** Increasing finite amplitude bounds in f2 units; signed values are allowed. */
  amplitude: [number, number];
  /** False selects an exact line such as Ka1; true selects a within-shell family such as Ka. */
  family?: boolean;
}
/** Immutable optional smooth-background term (since 0.2.10). It does not correct over-absorption. */
export class MbackErfc {
  /** Select an emission originating at the absorber edge, with explicit scientific bounds. */
  constructor(line: string, options: MbackErfcOptions);
}
/** Optional MBACK settings (since 0.2.10); energy and ranges use eV. */
export interface MbackOptions {
  /** Fixed measured edge origin in eV; omitted uses derivative detection. Does not shift the table. */
  e0?: number;
  /** Inclusive offsets from E0. Omitted suggests the outer 80% of the pre-edge span. */
  pre_edge?: [number, number];
  /** Inclusive offsets from E0. Omitted suggests the outer 80% of the post-edge span. */
  post_edge?: [number, number];
  /** Smooth-background polynomial degree 0–5, default 2. More flexibility can absorb real structure. */
  degree?: number;
  /** Optional bounded fluorescence-background term; omitted disables erfc. */
  erfc?: MbackErfc;
}
/**
 * Full Chantler MBACK normalization (since 0.2.10). Example:
 * new MBack("Cu", "K", {pre_edge: [-200,-50], post_edge: [100,800]}).
 *
 * Default degree 2 and erfc off. Automatic ranges respect neighboring edges;
 * inspect the returned intervals. Offline data are loaded automatically and
 * never shifted. fit() copies inputs and leaves them/model unchanged. Assign to
 * spectrum.set_normalization_method(model).normalize() for ordinary processing.
 * Invalid coverage, unsupported tables, unidentifiable fits and nonpositive
 * scale/step throw. Use a Worker for large browser fits; fit() is synchronous.
 * Call free() when finished. Settings copied into a spectrum remain independent.
 */
export class MBack {
  /** Select absorber/edge and optional named settings; browser init() is required first. */
  constructor(element: string, edge: string, options?: MbackOptions);
  /** Fit matching finite raw absorption arrays, with strictly increasing energy in eV. */
  fit(energy: Float64Array, mu: Float64Array): MbackResult;
  /** Versioned model JSON; no input arrays are added. Throws after free(). */
  to_json(): string;
  /** Restore a native definition. fit() checks scientific values and archived table identity. */
  static from_json(json: string): MBack;
  /** Release this native model. Spectrum settings and owned results remain valid. */
  free(): void;
}
/**
 * Owned full-MBACK result (since 0.2.10). Arrays are independent JavaScript copies.
 * norm and fpp are different quantities. Region balancing is not inverse-variance
 * weighting; objective/convergence alone do not establish experimental uncertainty.
 */
export interface MbackResult {
  /** Named profile, currently mback_chantler_v1. */
  method: string;
  /** A fresh native replay model pinned to the original table; free() it after use. */
  readonly definition: MBack;
  /** Original result snapshot including all settings/provenance. Editing copied arrays does not alter it. */
  to_json(): string;
  /** Exact offline reference and interpolation identity. */
  reference: AtomicReference;
  /** Fixed resolved edge origin in eV. */
  e0: number;
  /** Tabulated edge in eV; not shifted to measured E0. */
  tabulated_edge_ev: number;
  /** Resolved inclusive pre-edge offsets in eV. */
  pre_edge: [number, number];
  /** Resolved inclusive post-edge offsets in eV. */
  post_edge: [number, number];
  /** Positive atomic conversion scale from input mu units. */
  scale: number;
  /** Positive fitted absorption step in input mu units. */
  edge_step: number;
  /** Sum of squared balanced residuals, in squared f2 units. */
  objective: number;
  /** Column-scaled Jacobian condition number. */
  condition: number;
  /** Final Jacobian rank, including erfc width when enabled. */
  rank: number;
  /** Number of linear solves during fitting. */
  evaluations: number;
  /** Width in eV, or null when erfc is disabled. */
  erfc_width: number | null;
  /** Amplitude in f2 units; zero when disabled. */
  erfc_amplitude: number;
  /** Polynomial coordinate scale in eV. */
  energy_scale: number;
  /** Original energy grid, in eV. */
  energy: number[];
  /** Atomic scattering factor in electron units. */
  f2: number[];
  /** Matched scale*mu-background in electron units; distinct from norm. */
  fpp: number[];
  /** Dimensionless normalized absorption (scale*mu-pre_curve)/Delta. */
  norm: number[];
  /** Dimensionless flattened absorption using the auxiliary post-edge trend. */
  flat: number[];
  /** Smooth background in f2 units. */
  background: number[];
  /** Auxiliary pre-edge line on f2+background, in f2 units. */
  pre_curve: number[];
  /** Auxiliary quadratic post-edge curve, in f2 units. */
  post_curve: number[];
  /** Unweighted f2+background-scale*mu on every input point. */
  residual: number[];
  /** Increasing polynomial powers of (energy-e0)/energy_scale, in f2 units. */
  coefficients: number[];
  /** Original zero-based indices included in the objective. */
  fit_indices: number[];
  /** 1/sqrt(region count), in fit_indices order. */
  weights: number[];
  /** Nonfatal boundary/conditioning diagnostics. */
  warnings: string[];
}

/** Optional Cauchy settings (since 0.2.10). Construction copies these values. */
export interface WaveletOptions {
  /** Integer exponent 0–6, default 2; emphasizes high-k signal and noise. */
  kweight?: number;
  /** Cauchy order 1–4096, default 100. Larger order narrows frequency response and broadens localization in k. */
  order?: number;
  /** Uniform numerical k spacing in Å⁻¹, default 0.05; interpolation adds no experimental resolution. */
  kstep?: number;
  /** Maximum generated R in Å, default 6. R is not phase-corrected. */
  rmax?: number;
  /** Numerical R spacing in Å; default π/(FFT length*kstep). */
  rstep?: number;
  /** Half-cosine width inside the support endpoints in Å⁻¹. Default 0 means no taper. */
  taper?: number;
  /** Positive increasing R coordinates (Å), replacing the generated grid. */
  radii?: number[] | Float64Array;
  /** Power-of-two FFT length, at least twice the prepared k grid length; default automatic. No silent truncation. */
  nfft?: number;
}
/** Checked output dimensions and approximate scientific buffer storage. */
export interface WaveletSize {
  /** Prepared k columns, including padding. */ k_points: number;
  /** Number of map rows at strictly positive R coordinates. */ r_points: number;
  /** Power-of-two length of the internal fast Fourier transform. */ nfft: number;
  /** Number of complex output cells: R rows multiplied by k columns. */ cells: number;
  /** Estimated bytes, excluding input copies, FFT scratch and serialization. */ bytes: number;
}
/**
 * Cauchy settings, available since 0.2.10. Use spectrum.wavelet(new Wavelet([2, 12])).
 * k_range is measured support in Å⁻¹; since 0.2.15 endpoints beyond the measured k range
 * are limited to it; the map keeps the requested settings and its warnings report the
 * range used.
 * Defaults: weight 2, order 100,
 * k step 0.05 Å⁻¹, R up to 6 Å, no taper and automatic FFT/R sampling.
 * cauchy_v1 fixes order independently of R extent; larger order narrows frequency
 * response and broadens localization in k. R is not phase-corrected.
 * Browser init() is required. Calculations are synchronous native Wasm operations;
 * use a Worker for large interactive jobs. Invalid coverage/grids/budgets throw.
 */
export class Wavelet {
  /** Copy an inclusive measured k interval and optional named settings. */
  constructor(k_range: [number, number], options?: WaveletOptions);
  /** Transform original unweighted dimensionless χ(k). Copies finite matching arrays
   * with increasing, nonnegative k; linearly resamples without extrapolation. */
  calculate(k: Float64Array, chi: Float64Array): WaveletMap;
  /** Validate dimensions and estimate buffer storage before transforming. */
  estimate(k: Float64Array): WaveletSize;
  /** Native settings JSON with automatic choices preserved; no input arrays. */
  to_json(): string;
  /** Restore settings. Calculation validates scientific values and resource limits. */
  static from_json(json: string): Wavelet;
  /** Release this model. Independent maps and copied spectrum settings remain valid. */
  free(): void;
}
/** Spectrum preparation retained with a map; original k/χ are its direct replay inputs. */
export interface WaveletPreparation {
  /** Rexafs version that prepared the spectrum. */ software_version: string;
  /** Numerical backend name. */ backend: string;
  /** Resolved E₀ (eV), if available. */ e0: number | null;
  /** Normalization edge step in input μ units, if available. */ edge_step: number | null;
  /** Versioned native normalization settings, without large result arrays. */ normalization: unknown;
  /** Versioned native background settings, without large result arrays. */ background: unknown;
}
/**
 * Owned Cauchy map (since 0.2.10). All array getters return independent typed-array
 * copies. Complex/magnitude/phase arrays are flat, row-major: index r*shape[1]+k.
 * shape is [R rows, k columns]; W units are those of k**weight * χ, distinct from
 * ordinary Fourier scaling. Display colors and sampling do not define a metric.
 */
export class WaveletMap {
  private constructor();
  /** Matrix dimensions in [R, k] order, including explicit k padding. */ readonly shape: [number, number];
  /** Independent k coordinates, in Å⁻¹. */ readonly k: Float64Array;
  /** Independent R coordinates, in Å; not phase-corrected distances. */ readonly r: Float64Array;
  /** Original measured k before resampling. */ readonly input_k: Float64Array;
  /** Original unweighted χ, unchanged. */ readonly input_chi: Float64Array;
  /** Resampled unweighted χ; zero outside selected support. */ readonly prepared_chi: Float64Array;
  /** Support/taper multipliers applied before weighting. */ readonly window: Float64Array;
  /** One for measured support, zero for padding. */ readonly support: Uint8Array;
  /** Flat native real values. */ readonly real: Float64Array;
  /** Flat native imaginary values. */ readonly imaginary: Float64Array;
  /** Flat native magnitude, without display normalization/resampling. */ readonly magnitude: Float64Array;
  /** Radians, with NaN for zero amplitude or values below a fraction of the maximum
   * (default 1%). Fraction must lie in [0,1]. The native map remains unchanged. */
  phase(relative_floor?: number): Float64Array;
  /** Native magnitude versus k at a covered R coordinate (Å). */ slice_at_r(r: number): Float64Array;
  /** Native magnitude versus R at a covered k coordinate (Å⁻¹). */ slice_at_k(k: number): Float64Array;
  /** Integrate native bilinear magnitude over a covered k/R rectangle. Display
   * sampling never participates; invalid bounds throw. No uncertainty is inferred. */
  integral(k_range: [number, number], r_range: [number, number]): WaveletRegionValue;
  /** Area-weighted mean of native bilinear magnitude (since 0.2.10), not an average
   * of cells. k is Å⁻¹ and R is Å. Increasing, fully covered ranges are required;
   * invalid bounds throw. Returns units/method without inferred uncertainty. */
  mean(k_range: [number, number], r_range: [number, number]): WaveletRegionValue;
  /** Maximum native bilinear magnitude, including rectangle boundaries
   * (since 0.2.10). k is Å⁻¹ and R is Å; increasing, fully covered ranges are
   * required. Returns units/method without inferred uncertainty. */
  maximum(k_range: [number, number], r_range: [number, number]): WaveletRegionValue;
  /** Fresh independent settings; release them with free() after use. */ readonly definition: Wavelet;
  /** Original processing metadata, or null for a direct array calculation. */ readonly preparation: WaveletPreparation | null;
  /** Interpretation and boundary diagnostics, not confidence intervals. */ readonly warnings: string[];
  /** Full native map, original inputs and preparation provenance as JSON. */ to_json(): string;
  /** Restore checked method, dimensions, axes, finite values and budgets. This does
   * not independently prove an external producer's numerical correctness. */
  static from_json(json: string): WaveletMap;
  /** Release the native map. Previously returned array copies remain valid. */ free(): void;
}
/** Native covered-rectangle magnitude statistic; method identifies integral, mean or maximum. Units are those of k**weight * χ. */
export interface WaveletRegionValue {
  /** Native region statistic; no experimental uncertainty is supplied. */ value: number;
  /** Exact inclusive k bounds, in Å⁻¹. */ k_range: [number, number];
  /** Exact inclusive R bounds, in Å. */ r_range: [number, number];
  /** Integral units including k weight. */ unit: string;
  /** Numerical convention: bilinear_magnitude_v1 (integral), bilinear_magnitude_mean_v1 or bilinear_magnitude_maximum_v1. */ method: string;
}

/** Acquisition interpretation (since 0.2.10). Unknown means missing evidence, not
 * established fluorescence. Changing it never removes correction history. */
export type AbsorptionMode = "unknown" | "transmission" | "fluorescence";

/** Explicit sample geometry/emission plus optional internal-fit settings (since 0.2.10). */
export interface FluorescenceCorrectionOptions {
  /** Detected emission, for example Ka1; no line is inferred. */ line: string;
  /** Measured [incidence, exit] angles in degrees FROM THE SAMPLE SURFACE, each in (0,90]. */ angles: [number, number];
  /** Default false selects one line; true selects an unresolved within-shell family, e.g. Ka. */ family?: boolean;
  /** Measured E0 in eV; omitted detects the edge. Does not shift the atomic table. */ e0?: number;
  /** Internal pre-edge eV offsets from E0; omitted uses available low endpoint to -30 eV. */ pre_edge?: [number, number];
  /** Internal post-edge eV offsets from E0; omitted uses +100 eV to available high endpoint. */ post_edge?: [number, number];
  /** Internal post-edge polynomial degree 0–5, default 1. Pre-edge is linear; no Victoreen term. */ degree?: number;
}
/**
 * Optically thick, homogeneous-sample XANES correction (since 0.2.10).
 * new FluorescenceCorrection("CuO", "Cu", "K", {line:"Ka1", angles:[45,45]})
 * requires the complete sample formula, absorber, edge, detected emission and
 * measured geometry. Angles use the sample surface convention, not the normal.
 * Internal conventional normalization runs automatically; final normalization
 * of corrected mu is a separate operation. The fluo_elam_v1 model is not qualified
 * for EXAFS or finite-thickness samples. Offline atomic data load automatically.
 * See https://xraypy.github.io/xraylarch/xafs_preedge.html#over-absorption-corrections.
 */
export class FluorescenceCorrection {
  /** Copy explicit settings. Invalid types/options throw; scientific checks run
   * on calculation. Browser callers must await init() before construction. */
  constructor(formula: string, element: string, edge: string, options: FluorescenceCorrectionOptions);
  /** Correct original unnormalized fluorescence arrays on their energy grid (eV).
   * Copies matching finite Float64Arrays; energy must be positive and increasing.
   * Returns original/corrected mu in the same units. Invalid composition, geometry,
   * coverage, fitted step or singular denominator throw; nothing is clipped.
   * Uncertainty is unavailable. Prefer Spectrum.correct_fluorescence to retain
   * domain restrictions in later processing. This synchronous calculation should
   * run in a Worker for large browser workloads. Result needs no free(). */
  apply(energy: Float64Array, mu: Float64Array): FluorescenceCorrectionResult;
  /** Native settings JSON, including any pinned atomic identity. */ to_json(): string;
  /** Restore settings; calculation checks scientific values and reference availability. */
  static from_json(json: string): FluorescenceCorrection;
  /** Release this model's Wasm allocation. Do not access it again; copied results remain valid. */ free(): void;
}
/** Internal conventional fit of original mu (since 0.2.10), distinct from final
 * normalization. Lists are independent copies on the original energy grid. */
export interface FluorescenceInternalNormalization {
  /** Edge energy E0 used by the internal normalization, in electronvolts. */ e0: number;
  /** Resolved pre-edge eV offsets from E0. */ pre_edge: [number, number];
  /** Resolved post-edge eV offsets from E0. */ post_edge: [number, number];
  /** Internal post-edge polynomial degree; pre-edge is linear. */ degree: number;
  /** Positive fitted jump in original mu units, before numerical flooring. */ edge_step: number;
  /** Pre-edge line in original mu units. */ pre_curve: number[];
  /** Pre-edge line plus post-edge polynomial, in original mu units. */ post_curve: number[];
  /** Dimensionless internal n0 in alpha+1-n0. */ norm: number[];
}
/** Independent historical correction (since 0.2.10), with copied arrays/dictionaries.
 * Editing these values never alters the spectrum, to_json() record or replay
 * definition. No free() is required for this JavaScript result. Inspect warnings
 * and amplification; numerical success does not establish physical validity. */
export interface FluorescenceCorrectionResult {
  /** Original measured energy in eV, without resampling. */ readonly energy: Float64Array;
  /** Original uncorrected absorption, in supplied units. */ readonly original_mu: Float64Array;
  /** Corrected absorption, same grid/units; final normalization is separate. */ readonly corrected_mu: Float64Array;
  /** Dimensionless alpha/denominator, without clipping. */ readonly factor: Float64Array;
  /** Dimensionless alpha+1-internal_norm, without clipping. */ readonly denominator: Float64Array;
  /** Named numerical convention, fluo_elam_v1. */ readonly method: string;
  /** Original acquisition interpretation; unknown records a caller assumption. */ readonly input_mode: AbsorptionMode;
  /** Dimensionless attenuation/geometry constant. */ readonly alpha: number;
  /** sin(incidence)/sin(exit) with surface angles; dimensionless. */ readonly geometry_ratio: number;
  /** Smallest dimensionless denominator on the whole input grid. */ readonly minimum_denominator: number;
  /** Largest dimensionless factor; high values amplify noise. */ readonly maximum_amplification: number;
  /** Numerical rejection limit, 64*epsilon*max(1, alpha+1). */ readonly singularity_threshold: number;
  /** Domain, interpretation and numerical diagnostics; not confidence intervals. */ readonly warnings: string[];
  /** Fresh independent settings pinned to resolved ranges/E0 and atomic data; call free() after use. */ readonly definition: FluorescenceCorrection;
  /** Internal conventional fit of original mu, with independent lists. */ readonly internal: FluorescenceInternalNormalization;
  /** Edge, emission and compound attenuation records: eV energies, mass fractions,
   * cm²/g values, table identities and checksums. */ readonly atomic: Record<string, unknown>;
  /** Original native record with full inputs/assumptions/atomic evidence. */ to_json(): string;
}
