/** Configuration names and scalar fields mirror Rust. Setters copy the configuration. */
export type FTWindow = "Hanning" | "Parzen" | "Welch" | "Gaussian" | "Sine" | "KaiserBessel" | "FHanning";
export type AUTOBKSolver = "TrustRegionDogLeg" | "LegacyLm" | "LinearDirect";
export type AUTOBKClampScalePolicy = "FixedPenalty" | "Fixed" | "TwoPass";

export class PrePostEdge {
  constructor();
  free(): void;
  pre_edge_start: number | undefined;
  pre_edge_end: number | undefined;
  norm_start: number | undefined;
  norm_end: number | undefined;
  norm_polyorder: number | undefined;
  n_victoreen: number | undefined;
  e0: number | undefined;
  edge_step: number | undefined;
}

export class AUTOBK {
  constructor();
  free(): void;
  ek0: number | undefined;
  rbkg: number | undefined;
  nknots: number | undefined;
  kmin: number | undefined;
  kmax: number | undefined;
  kstep: number | undefined;
  nclamp: number | undefined;
  clamp_lo: number | undefined;
  /** FixedPenalty strength; default 0.001, zero disables the endpoint penalty. */
  clamp_lambda: number | undefined;
  clamp_hi: number | undefined;
  nfft: number | undefined;
  kweight: number | undefined;
  dk: number | undefined;
  linear_regularization: number | undefined;
  linear_condition_limit: number | undefined;
  linear_residual_ratio_limit: number | undefined;
  linear_fallback_to_lm: boolean | undefined;
  linear_workspace_cache: boolean | undefined;
  window: FTWindow | undefined;
  solver: AUTOBKSolver | undefined;
  linear_fallback_solver: AUTOBKSolver | undefined;
  clamp_scale_policy: AUTOBKClampScalePolicy | undefined;
}

export class XrayFFTF {
  constructor();
  free(): void;
  rmax_out: number | undefined;
  dk: number | undefined;
  dk2: number | undefined;
  kmin: number | undefined;
  kmax: number | undefined;
  kweight: number | undefined;
  nfft: number | undefined;
  kstep: number | undefined;
  window: FTWindow | undefined;
}

export class NormalizationMethod {
  private constructor();
  free(): void;
  static PrePostEdge(parameters: PrePostEdge): NormalizationMethod;
  static new_prepostedge(): NormalizationMethod;
  static new_mback(): NormalizationMethod;
}
export class BackgroundMethod {
  private constructor();
  free(): void;
  static AUTOBK(parameters: AUTOBK): BackgroundMethod;
  static new_autobk(): BackgroundMethod;
  static new_ilpbkg(): BackgroundMethod;
}

/** Mutable Rust spectrum. Stages run synchronously and return the same object.
 * Missing prerequisites use the selected algorithms and their defaults.
 * Array getters return independent copies, or undefined before their stage runs.
 */
export class Spectrum {
  constructor(energy: Float64Array, mu: Float64Array);
  static from_arrays(energy: Float64Array, mu: Float64Array): Spectrum;
  free(): void;
  set_spectrum(energy: Float64Array, mu: Float64Array): this;
  set_e0(e0: number): this;
  set_normalization_method(method?: NormalizationMethod | null): this;
  set_background_method(method?: BackgroundMethod | null): this;
  set_fft(parameters: XrayFFTF): this;
  e0(): number | undefined;
  find_e0(): this;
  normalize(): this;
  calc_background(): this;
  fft(): this;
  ifft(): this;
  invalidate_derived(): this;
  k(): Float64Array | undefined;
  chi(): Float64Array | undefined;
  norm(): Float64Array | undefined;
  flat(): Float64Array | undefined;
  pre_edge(): Float64Array | undefined;
  post_edge(): Float64Array | undefined;
  r(): Float64Array | undefined;
  chir_mag(): Float64Array | undefined;
  chir_real(): Float64Array | undefined;
  chir_imag(): Float64Array | undefined;
  q(): Float64Array | undefined;
  chiq(): Float64Array | undefined;
}
