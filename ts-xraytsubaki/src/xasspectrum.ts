/**
 * XASSpectrum class for X-ray Absorption Spectroscopy analysis
 */

import { lib, bufferToFloat64Array, float64ArrayToBuffer } from './ffi/bindings_mock';
import { WindowFunction } from './ffi/types';

/**
 * Represents an XAS spectrum with associated data and processing methods
 */
export class XASSpectrum {
  // Internal FFI handle
  private handle: any;
  
  // Cached arrays to avoid repeated FFI calls
  private _energy: Float64Array | null = null;
  private _mu: Float64Array | null = null;
  private _k: Float64Array | null = null;
  private _chi: Float64Array | null = null;
  private _r: Float64Array | null = null;
  private _chiRMag: Float64Array | null = null;
  private _chiRRe: Float64Array | null = null;
  private _chiRIm: Float64Array | null = null;
  private _q: Float64Array | null = null;
  private _chiQ: Float64Array | null = null;
  
  // Cached values
  private _e0: number | null = null;
  private _name: string | null = null;
  
  /**
   * Create a new XAS spectrum
   * 
   * @param name Optional name for the spectrum
   * @param energy Optional energy array (eV)
   * @param mu Optional absorption data array
   */
  constructor(name?: string, energy?: Float64Array, mu?: Float64Array) {
    this.handle = lib.xas_spectrum_new();
    
    if (name) {
      this.name = name;
    }
    
    if (energy && mu) {
      this.setData(energy, mu);
    }
  }
  
  /**
   * Clean up resources when object is garbage collected
   */
  destroy(): void {
    if (this.handle) {
      lib.xas_spectrum_free(this.handle);
      this.handle = null;
    }
  }
  
  /**
   * Set the spectrum data
   * 
   * @param energy Energy array (eV)
   * @param mu Absorption data array
   */
  setData(energy: Float64Array, mu: Float64Array): void {
    if (energy.length !== mu.length) {
      throw new Error('Energy and mu arrays must have the same length');
    }
    
    const energyBuffer = float64ArrayToBuffer(energy);
    const muBuffer = float64ArrayToBuffer(mu);
    
    lib.xas_spectrum_set_data(this.handle, energyBuffer, muBuffer, energy.length);
    
    // Update cached arrays
    this._energy = energy;
    this._mu = mu;
    
    // Reset processed data
    this._k = null;
    this._chi = null;
    this._r = null;
    this._chiRMag = null;
    this._chiRRe = null;
    this._chiRIm = null;
    this._q = null;
    this._chiQ = null;
    this._e0 = null;
  }
  
  /**
   * Find the edge energy (E0)
   */
  findE0(): number {
    const e0 = lib.xas_spectrum_find_e0(this.handle);
    this._e0 = e0;
    return e0;
  }
  
  /**
   * Normalize the spectrum using pre- and post-edge lines
   * 
   * @param preEdgeRange Range for pre-edge fitting [min, max] relative to E0 (default: [-150, -30])
   * @param postEdgeRange Range for post-edge fitting [min, max] relative to E0 (default: [100, 400])
   * @returns The edge step value
   */
  normalize(
    preEdgeRange: [number, number] = [-150, -30],
    postEdgeRange: [number, number] = [100, 400]
  ): number {
    if (this._e0 === null) {
      this.findE0();
    }
    
    const result = lib.xas_spectrum_normalize(
      this.handle,
      preEdgeRange[0],
      preEdgeRange[1],
      postEdgeRange[0],
      postEdgeRange[1]
    );
    
    return result.edge_step;
  }
  
  /**
   * Calculate the background and extract EXAFS chi(k)
   * 
   * @param rbkg Background removal parameter (default: 1.0)
   * @param kweight k-weighting for chi(k) (default: 2)
   * @param krange k-range for fitting [kmin, kmax] (default: [0, 15])
   */
  calcBackground(
    rbkg: number = 1.0,
    kweight: number = 2,
    krange: [number, number] = [0, 15]
  ): void {
    if (this._e0 === null) {
      this.findE0();
    }
    
    const result = lib.xas_spectrum_calc_background(
      this.handle,
      rbkg,
      kweight,
      krange[0],
      krange[1]
    );
    
    // Cache the k and chi arrays
    if (result.length > 0) {
      this._k = bufferToFloat64Array(result.k, result.length);
      this._chi = bufferToFloat64Array(result.chi, result.length);
    }
  }
  
  /**
   * Perform forward Fourier transform to get chi(R)
   * 
   * @param krange k-range for transform [kmin, kmax] (default: [2, 12])
   * @param dk Delta k for window tapering (default: 2)
   * @param window Window function type (default: 'Hanning')
   * @param kweight k-weighting for transform (default: 2)
   */
  fft(
    krange: [number, number] = [2, 12],
    dk: number = 2,
    window: keyof typeof WindowFunction = 'Hanning',
    kweight: number = 2
  ): void {
    if (!this._k || !this._chi) {
      throw new Error('Need to calculate k and chi first. Call calcBackground() before fft().');
    }
    
    const result = lib.xas_spectrum_fft(
      this.handle,
      krange[0],
      krange[1],
      dk,
      window,
      kweight
    );
    
    // Cache the arrays
    if (result.length > 0) {
      this._r = bufferToFloat64Array(result.r, result.length);
      this._chiRMag = bufferToFloat64Array(result.chir_mag, result.length);
      this._chiRRe = bufferToFloat64Array(result.chir_re, result.length);
      this._chiRIm = bufferToFloat64Array(result.chir_im, result.length);
    }
  }
  
  /**
   * Perform inverse Fourier transform to get back-transformed chi(q)
   * 
   * @param rrange R-range for transform [rmin, rmax] (default: [1, 3])
   * @param dr Delta r for window tapering (default: 0.1)
   * @param window Window function type (default: 'Hanning')
   */
  ifft(
    rrange: [number, number] = [1, 3],
    dr: number = 0.1,
    window: keyof typeof WindowFunction = 'Hanning'
  ): void {
    if (!this._r || !this._chiRMag || !this._chiRRe || !this._chiRIm) {
      throw new Error('Need to calculate r and chi(r) first. Call fft() before ifft().');
    }
    
    const result = lib.xas_spectrum_ifft(
      this.handle,
      rrange[0],
      rrange[1],
      dr,
      window
    );
    
    // Cache the arrays
    if (result.length > 0) {
      this._q = bufferToFloat64Array(result.q, result.length);
      this._chiQ = bufferToFloat64Array(result.chiq, result.length);
    }
  }
  
  /**
   * Get the normalization result
   */
  getNormalization(): {
    pre: Float64Array;
    post: Float64Array;
    norm: Float64Array;
    edgeStep: number;
  } | null {
    const result = lib.xas_spectrum_get_normalization(this.handle);
    
    if (!result || result.length === 0) {
      return null;
    }
    
    return {
      pre: bufferToFloat64Array(result.pre, result.length),
      post: bufferToFloat64Array(result.post, result.length),
      norm: bufferToFloat64Array(result.norm, result.length),
      edgeStep: result.edge_step
    };
  }
  
  /**
   * Save the spectrum to a file
   * 
   * @param filePath Path to save the file
   * @returns True if successful, false otherwise
   */
  saveToFile(filePath: string): boolean {
    return lib.xas_spectrum_save_file(this.handle, filePath);
  }
  
  /**
   * Load a spectrum from a file
   * 
   * @param filePath Path to the file
   * @returns A new XASSpectrum instance
   * @static
   */
  static fromFile(filePath: string): XASSpectrum {
    const handle = lib.xas_spectrum_load_file(filePath);
    
    if (!handle) {
      throw new Error(`Failed to load spectrum from file: ${filePath}`);
    }
    
    const spectrum = new XASSpectrum();
    spectrum.handle = handle;
    
    // Refresh cached data
    spectrum._energy = spectrum.getEnergyFromFFI();
    spectrum._mu = spectrum.getMuFromFFI();
    
    return spectrum;
  }
  
  /**
   * Get the energy array from FFI
   * @private
   */
  private getEnergyFromFFI(): Float64Array | null {
    const length = lib.xas_spectrum_get_length(this.handle);
    
    if (length === 0) {
      return null;
    }
    
    const buffer = lib.xas_spectrum_get_energy(this.handle, length);
    return bufferToFloat64Array(buffer, length);
  }
  
  /**
   * Get the mu array from FFI
   * @private
   */
  private getMuFromFFI(): Float64Array | null {
    const length = lib.xas_spectrum_get_length(this.handle);
    
    if (length === 0) {
      return null;
    }
    
    const buffer = lib.xas_spectrum_get_mu(this.handle, length);
    return bufferToFloat64Array(buffer, length);
  }
  
  /**
   * Get the k array from FFI
   * @private
   */
  private getKFromFFI(): Float64Array | null {
    const length = lib.xas_spectrum_get_k_length(this.handle);
    
    if (length === 0) {
      return null;
    }
    
    const buffer = lib.xas_spectrum_get_k(this.handle, length);
    return bufferToFloat64Array(buffer, length);
  }
  
  /**
   * Get the chi array from FFI
   * @private
   */
  private getChiFromFFI(): Float64Array | null {
    const length = lib.xas_spectrum_get_k_length(this.handle);
    
    if (length === 0) {
      return null;
    }
    
    const buffer = lib.xas_spectrum_get_chi(this.handle, length);
    return bufferToFloat64Array(buffer, length);
  }
  
  // === Getters and setters ===
  
  /**
   * Get the spectrum name
   */
  get name(): string | null {
    if (this._name === null) {
      this._name = lib.xas_spectrum_get_name(this.handle);
    }
    return this._name;
  }
  
  /**
   * Set the spectrum name
   */
  set name(value: string | null) {
    if (value !== null) {
      lib.xas_spectrum_set_name(this.handle, value);
      this._name = value;
    }
  }
  
  /**
   * Get the energy array
   */
  get energy(): Float64Array | null {
    if (this._energy === null) {
      this._energy = this.getEnergyFromFFI();
    }
    return this._energy;
  }
  
  /**
   * Get the absorption data array
   */
  get mu(): Float64Array | null {
    if (this._mu === null) {
      this._mu = this.getMuFromFFI();
    }
    return this._mu;
  }
  
  /**
   * Get the edge energy (E0)
   */
  get e0(): number | null {
    if (this._e0 === null) {
      this._e0 = lib.xas_spectrum_get_e0(this.handle);
      if (this._e0 === 0 && !this._energy) {
        this._e0 = null;
      }
    }
    return this._e0;
  }
  
  /**
   * Set the edge energy (E0)
   */
  set e0(value: number | null) {
    if (value !== null) {
      lib.xas_spectrum_set_e0(this.handle, value);
      this._e0 = value;
    }
  }
  
  /**
   * Get the k array (wave number)
   */
  get k(): Float64Array | null {
    if (this._k === null) {
      this._k = this.getKFromFFI();
    }
    return this._k;
  }
  
  /**
   * Get the EXAFS chi(k) array
   */
  get chi(): Float64Array | null {
    if (this._chi === null) {
      this._chi = this.getChiFromFFI();
    }
    return this._chi;
  }
  
  /**
   * Get the R array (Fourier transform distances)
   */
  get r(): Float64Array | null {
    return this._r;
  }
  
  /**
   * Get the chi(R) magnitude array
   */
  get chiRMag(): Float64Array | null {
    return this._chiRMag;
  }
  
  /**
   * Get the chi(R) real part array
   */
  get chiRRe(): Float64Array | null {
    return this._chiRRe;
  }
  
  /**
   * Get the chi(R) imaginary part array
   */
  get chiRIm(): Float64Array | null {
    return this._chiRIm;
  }
  
  /**
   * Get the q array (inverse transform wave number)
   */
  get q(): Float64Array | null {
    return this._q;
  }
  
  /**
   * Get the chi(q) array (back-transformed EXAFS)
   */
  get chiQ(): Float64Array | null {
    return this._chiQ;
  }
}