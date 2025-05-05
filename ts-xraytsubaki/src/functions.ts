/**
 * Standalone XAFS analysis functions
 */

import { lib, bufferToFloat64Array, float64ArrayToBuffer } from './ffi/bindings_mock';
import { WindowFunction } from './ffi/types';

/**
 * Options for pre-edge normalization
 */
export interface PreEdgeOptions {
  /** Edge energy (E0) in eV */
  e0: number;
  /** Range for pre-edge fitting [min, max] relative to E0 (default: [-150, -30]) */
  preEdgeRange?: [number, number];
  /** Range for post-edge fitting [min, max] relative to E0 (default: [100, 400]) */
  postEdgeRange?: [number, number];
  /** Whether to normalize the spectrum (default: true) */
  normalize?: boolean;
}

/**
 * Options for autobk background subtraction
 */
export interface AutobkOptions {
  /** Edge energy (E0) in eV */
  e0: number;
  /** Background removal parameter (default: 1.0) */
  rbkg?: number;
  /** k-weighting for spline clamps (default: 2) */
  kweight?: number;
  /** Minimum k value (default: 0) */
  kmin?: number;
  /** Maximum k value (default: 15) */
  kmax?: number;
}

/**
 * Options for forward Fourier transform
 */
export interface XftfOptions {
  /** Minimum k value (default: 2) */
  kmin?: number;
  /** Maximum k value (default: 12) */
  kmax?: number;
  /** Delta k for window tapering (default: 2) */
  dk?: number;
  /** Window function type (default: 'Hanning') */
  window?: keyof typeof WindowFunction;
  /** k-weighting for transform (default: 2) */
  kweight?: number;
}

/**
 * Options for inverse Fourier transform
 */
export interface XftrOptions {
  /** Minimum R value (default: 1) */
  rmin?: number;
  /** Maximum R value (default: 3) */
  rmax?: number;
  /** Delta R for window tapering (default: 0.1) */
  dr?: number;
  /** Window function type (default: 'Hanning') */
  window?: keyof typeof WindowFunction;
}

/**
 * Find the edge energy (E0) in an XAS spectrum
 * 
 * @param energy Energy array (eV)
 * @param mu Absorption data array
 * @returns The found E0 value (eV)
 */
export function findE0(energy: Float64Array, mu: Float64Array): number {
  if (energy.length !== mu.length) {
    throw new Error('Energy and mu arrays must have the same length');
  }
  
  const energyBuffer = float64ArrayToBuffer(energy);
  const muBuffer = float64ArrayToBuffer(mu);
  
  return lib.xas_find_e0(energyBuffer, muBuffer, energy.length);
}

/**
 * Perform pre-edge normalization of an XAS spectrum
 * 
 * @param energy Energy array (eV)
 * @param mu Absorption data array
 * @param options Options for normalization
 * @returns Object containing the normalization results
 */
export function preEdge(
  energy: Float64Array,
  mu: Float64Array,
  options: PreEdgeOptions
): {
  pre: Float64Array;
  post: Float64Array;
  norm: Float64Array;
  edge_step: number;
  pre_slope: number;
  pre_intercept: number;
  post_slope: number;
  post_intercept: number;
} {
  if (energy.length !== mu.length) {
    throw new Error('Energy and mu arrays must have the same length');
  }
  
  const preEdgeRange = options.preEdgeRange || [-150, -30];
  const postEdgeRange = options.postEdgeRange || [100, 400];
  const normalize = options.normalize !== undefined ? options.normalize : true;
  
  const energyBuffer = float64ArrayToBuffer(energy);
  const muBuffer = float64ArrayToBuffer(mu);
  
  const result = lib.xas_pre_edge(
    energyBuffer,
    muBuffer,
    energy.length,
    options.e0,
    preEdgeRange[0],
    preEdgeRange[1],
    postEdgeRange[0],
    postEdgeRange[1],
    normalize
  );
  
  return {
    pre: bufferToFloat64Array(result.pre, result.length),
    post: bufferToFloat64Array(result.post, result.length),
    norm: bufferToFloat64Array(result.norm, result.length),
    edge_step: result.edge_step,
    pre_slope: result.pre_slope,
    pre_intercept: result.pre_intercept,
    post_slope: result.post_slope,
    post_intercept: result.post_intercept
  };
}

/**
 * Perform autobk background subtraction to extract EXAFS
 * 
 * @param energy Energy array (eV)
 * @param mu Normalized absorption data array
 * @param options Options for background subtraction
 * @returns Object containing the EXAFS results
 */
export function autobk(
  energy: Float64Array,
  mu: Float64Array,
  options: AutobkOptions
): {
  k: Float64Array;
  chi: Float64Array;
  kmin: number;
  kmax: number;
} {
  if (energy.length !== mu.length) {
    throw new Error('Energy and mu arrays must have the same length');
  }
  
  const rbkg = options.rbkg !== undefined ? options.rbkg : 1.0;
  const kweight = options.kweight !== undefined ? options.kweight : 2;
  const kmin = options.kmin !== undefined ? options.kmin : 0;
  const kmax = options.kmax !== undefined ? options.kmax : 15;
  
  const energyBuffer = float64ArrayToBuffer(energy);
  const muBuffer = float64ArrayToBuffer(mu);
  
  const result = lib.xas_autobk(
    energyBuffer,
    muBuffer,
    energy.length,
    options.e0,
    rbkg,
    kweight,
    kmin,
    kmax
  );
  
  return {
    k: bufferToFloat64Array(result.k, result.length),
    chi: bufferToFloat64Array(result.chi, result.length),
    kmin: result.kmin,
    kmax: result.kmax
  };
}

/**
 * Perform forward Fourier transform of EXAFS data
 * 
 * @param k Wave number array (Å⁻¹)
 * @param chi EXAFS chi(k) array
 * @param options Options for Fourier transform
 * @returns Object containing the Fourier transform results
 */
export function xftf(
  k: Float64Array,
  chi: Float64Array,
  options: XftfOptions = {}
): {
  r: Float64Array;
  chir: {
    re: Float64Array;
    im: Float64Array;
  };
  chir_mag: Float64Array;
  chir_re: Float64Array;
  chir_im: Float64Array;
} {
  if (k.length !== chi.length) {
    throw new Error('k and chi arrays must have the same length');
  }
  
  const kmin = options.kmin !== undefined ? options.kmin : 2;
  const kmax = options.kmax !== undefined ? options.kmax : 12;
  const dk = options.dk !== undefined ? options.dk : 2;
  const window = options.window || 'Hanning';
  const kweight = options.kweight !== undefined ? options.kweight : 2;
  
  const kBuffer = float64ArrayToBuffer(k);
  const chiBuffer = float64ArrayToBuffer(chi);
  
  const result = lib.xas_xftf(
    kBuffer,
    chiBuffer,
    k.length,
    kmin,
    kmax,
    dk,
    window,
    kweight
  );
  
  const r = bufferToFloat64Array(result.r, result.length);
  const chir_mag = bufferToFloat64Array(result.chir_mag, result.length);
  const chir_re = bufferToFloat64Array(result.chir_re, result.length);
  const chir_im = bufferToFloat64Array(result.chir_im, result.length);
  
  return {
    r,
    chir: {
      re: chir_re,
      im: chir_im
    },
    chir_mag,
    chir_re,
    chir_im
  };
}

/**
 * Perform inverse Fourier transform of EXAFS data
 * 
 * @param r Distance array (Å)
 * @param chir_re Real part of chi(R)
 * @param chir_im Imaginary part of chi(R)
 * @param options Options for inverse Fourier transform
 * @returns Object containing the inverse Fourier transform results
 */
export function xftr(
  r: Float64Array,
  chir: { re: Float64Array; im: Float64Array },
  options: XftrOptions = {}
): {
  q: Float64Array;
  chiq: Float64Array;
} {
  if (r.length !== chir.re.length || r.length !== chir.im.length) {
    throw new Error('r, chir.re, and chir.im arrays must have the same length');
  }
  
  const rmin = options.rmin !== undefined ? options.rmin : 1;
  const rmax = options.rmax !== undefined ? options.rmax : 3;
  const dr = options.dr !== undefined ? options.dr : 0.1;
  const window = options.window || 'Hanning';
  
  const rBuffer = float64ArrayToBuffer(r);
  const chirReBuffer = float64ArrayToBuffer(chir.re);
  const chirImBuffer = float64ArrayToBuffer(chir.im);
  
  const result = lib.xas_xftr(
    rBuffer,
    chirReBuffer,
    chirImBuffer,
    r.length,
    rmin,
    rmax,
    dr,
    window
  );
  
  return {
    q: bufferToFloat64Array(result.q, result.length),
    chiq: bufferToFloat64Array(result.chiq, result.length)
  };
}