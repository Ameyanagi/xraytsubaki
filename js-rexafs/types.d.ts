export interface ProcessOptions { /** Edge energy in eV; omitted means automatic. */ e0?: number; }
/** Owned arrays; k pairs with chi, r pairs with all Fourier components. */
export interface ProcessedSpectrum {
  e0: number;
  /** Wave number in inverse angstroms. */ k: Float64Array;
  /** Unweighted chi(k). */ chi: Float64Array;
  /** Distance in angstroms, not phase corrected. */ r: Float64Array;
  chir_mag: Float64Array;
  chir_re: Float64Array;
  chir_im: Float64Array;
}
/** Finite, equal-length arrays; energy must strictly increase, in eV. Throws Error on failure. */
export function process(energy: Float64Array, mu: Float64Array, options?: ProcessOptions): ProcessedSpectrum;
