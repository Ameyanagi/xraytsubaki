/**
 * EXAFS fitting functionality
 */

import { lib, bufferToFloat64Array, float64ArrayToBuffer } from './ffi/bindings_mock';

/**
 * Represents a parameter for fitting
 */
export class FittingParameter {
  // Internal FFI handle
  private handle: any;
  
  /**
   * Create a new fitting parameter
   * 
   * @param name Parameter name
   * @param value Initial value
   * @param min Minimum value (optional)
   * @param max Maximum value (optional)
   * @param vary Whether to vary this parameter during fitting (default: true)
   */
  constructor(
    private _name: string,
    private _value: number,
    private _min: number | null = null,
    private _max: number | null = null,
    private _vary: boolean = true
  ) {
    this.handle = lib.fitting_parameter_new(name, value);
    
    if (min !== null) {
      this.setMin(min);
    }
    
    if (max !== null) {
      this.setMax(max);
    }
    
    if (!vary) {
      this.setVary(false);
    }
  }
  
  /**
   * Clean up resources when object is garbage collected
   */
  destroy(): void {
    if (this.handle) {
      lib.fitting_parameter_free(this.handle);
      this.handle = null;
    }
  }
  
  /**
   * Get the parameter name
   */
  get name(): string {
    return this._name;
  }
  
  /**
   * Get the parameter value
   */
  get value(): number {
    return this._value;
  }
  
  /**
   * Set the parameter value
   */
  setValue(value: number): void {
    lib.fitting_parameter_set_value(this.handle, value);
    this._value = value;
  }
  
  /**
   * Get the minimum allowed value
   */
  get min(): number | null {
    return this._min;
  }
  
  /**
   * Set the minimum allowed value
   */
  setMin(min: number): void {
    lib.fitting_parameter_set_min(this.handle, min);
    this._min = min;
  }
  
  /**
   * Get the maximum allowed value
   */
  get max(): number | null {
    return this._max;
  }
  
  /**
   * Set the maximum allowed value
   */
  setMax(max: number): void {
    lib.fitting_parameter_set_max(this.handle, max);
    this._max = max;
  }
  
  /**
   * Get whether this parameter is varied during fitting
   */
  get vary(): boolean {
    return this._vary;
  }
  
  /**
   * Set whether this parameter is varied during fitting
   */
  setVary(vary: boolean): void {
    lib.fitting_parameter_set_vary(this.handle, vary);
    this._vary = vary;
  }
}

/**
 * Collection of parameters for fitting
 */
export class FittingParameters {
  // Internal FFI handle
  private handle: any;
  
  // Track parameters for easier access
  private parameters: Map<string, FittingParameter> = new Map();
  
  /**
   * Create a new set of fitting parameters
   */
  constructor() {
    this.handle = lib.fitting_parameters_new();
  }
  
  /**
   * Clean up resources when object is garbage collected
   */
  destroy(): void {
    if (this.handle) {
      lib.fitting_parameters_free(this.handle);
      this.handle = null;
    }
  }
  
  /**
   * Add a parameter to the set
   * 
   * @param name Parameter name
   * @param value Initial value
   * @param min Minimum value (optional)
   * @param max Maximum value (optional)
   * @param vary Whether to vary this parameter during fitting (default: true)
   * @returns The created FittingParameter object
   */
  add(
    name: string,
    value: number,
    min: number | null = null,
    max: number | null = null,
    vary: boolean = true
  ): FittingParameter {
    lib.fitting_parameters_add(
      this.handle,
      name,
      value,
      min !== null ? min : Number.MIN_SAFE_INTEGER,
      max !== null ? max : Number.MAX_SAFE_INTEGER,
      vary
    );
    
    const param = new FittingParameter(name, value, min, max, vary);
    this.parameters.set(name, param);
    
    return param;
  }
  
  /**
   * Get a parameter by name
   * 
   * @param name Parameter name
   * @returns The parameter, or null if not found
   */
  get(name: string): FittingParameter | null {
    return this.parameters.get(name) || null;
  }
  
  /**
   * Set the value of a parameter
   * 
   * @param name Parameter name
   * @param value New value
   */
  set(name: string, value: number): void {
    const param = this.get(name);
    
    if (param) {
      param.setValue(value);
    }
  }
  
  /**
   * Get the number of parameters in the set
   */
  size(): number {
    return lib.fitting_parameters_size(this.handle);
  }
}

/**
 * Simple path model for EXAFS fitting
 */
export class SimplePath {
  // Internal FFI handle
  private handle: any;
  
  // Parameter names
  private _s02Param: string | null = null;
  private _e0Param: string | null = null;
  private _sigma2Param: string | null = null;
  private _delrParam: string | null = null;
  
  /**
   * Create a new simple path model
   * 
   * @param label Path label
   * @param n Coordination number
   * @param r Path distance (Å)
   */
  constructor(
    private _label: string,
    private _n: number,
    private _r: number
  ) {
    this.handle = lib.simple_path_new(label, n, r);
  }
  
  /**
   * Clean up resources when object is garbage collected
   */
  destroy(): void {
    if (this.handle) {
      lib.simple_path_free(this.handle);
      this.handle = null;
    }
  }
  
  /**
   * Get the path label
   */
  getLabel(): string {
    return this._label;
  }
  
  /**
   * Get the coordination number
   */
  getN(): number {
    return this._n;
  }
  
  /**
   * Get the path distance
   */
  getR(): number {
    return this._r;
  }
  
  /**
   * Set the parameter name for amplitude factor (S0²)
   * 
   * @param paramName Parameter name
   */
  setS02(paramName: string): void {
    lib.simple_path_set_s02(this.handle, paramName);
    this._s02Param = paramName;
  }
  
  /**
   * Get the parameter name for amplitude factor (S0²)
   */
  getS02Param(): string | null {
    return this._s02Param;
  }
  
  /**
   * Set the parameter name for energy shift (E0)
   * 
   * @param paramName Parameter name
   */
  setE0(paramName: string): void {
    lib.simple_path_set_e0(this.handle, paramName);
    this._e0Param = paramName;
  }
  
  /**
   * Get the parameter name for energy shift (E0)
   */
  getE0Param(): string | null {
    return this._e0Param;
  }
  
  /**
   * Set the parameter name for Debye-Waller factor (σ²)
   * 
   * @param paramName Parameter name
   */
  setSigma2(paramName: string): void {
    lib.simple_path_set_sigma2(this.handle, paramName);
    this._sigma2Param = paramName;
  }
  
  /**
   * Get the parameter name for Debye-Waller factor (σ²)
   */
  getSigma2Param(): string | null {
    return this._sigma2Param;
  }
  
  /**
   * Set the parameter name for path length variation (ΔR)
   * 
   * @param paramName Parameter name
   */
  setDelr(paramName: string): void {
    lib.simple_path_set_delr(this.handle, paramName);
    this._delrParam = paramName;
  }
  
  /**
   * Get the parameter name for path length variation (ΔR)
   */
  getDelrParam(): string | null {
    return this._delrParam;
  }
}

/**
 * Dataset for EXAFS fitting
 */
export class FittingDataset {
  // Internal FFI handle
  private handle: any;
  
  // Dataset properties
  private _k: Float64Array;
  private _chi: Float64Array;
  private _kMin: number = 2.0;
  private _kMax: number = 12.0;
  private _kWeight: number = 2;
  
  /**
   * Create a new fitting dataset
   * 
   * @param k Wave number array (Å⁻¹)
   * @param chi EXAFS chi(k) array
   */
  constructor(k: Float64Array, chi: Float64Array) {
    if (k.length !== chi.length) {
      throw new Error('k and chi arrays must have the same length');
    }
    
    this._k = k;
    this._chi = chi;
    
    const kBuffer = float64ArrayToBuffer(k);
    const chiBuffer = float64ArrayToBuffer(chi);
    
    this.handle = lib.fitting_dataset_new(kBuffer, chiBuffer, k.length);
  }
  
  /**
   * Clean up resources when object is garbage collected
   */
  destroy(): void {
    if (this.handle) {
      lib.fitting_dataset_free(this.handle);
      this.handle = null;
    }
  }
  
  /**
   * Get the k array
   */
  getK(): Float64Array {
    return this._k;
  }
  
  /**
   * Get the chi array
   */
  getChi(): Float64Array {
    return this._chi;
  }
  
  /**
   * Set the k range for fitting
   * 
   * @param kMin Minimum k value
   * @param kMax Maximum k value
   */
  setKRange(kMin: number, kMax: number): void {
    lib.fitting_dataset_set_k_range(this.handle, kMin, kMax);
    this._kMin = kMin;
    this._kMax = kMax;
  }
  
  /**
   * Get the minimum k value
   */
  getKMin(): number {
    return this._kMin;
  }
  
  /**
   * Get the maximum k value
   */
  getKMax(): number {
    return this._kMax;
  }
  
  /**
   * Set the k-weighting for fitting
   * 
   * @param kWeight k-weighting (typically 1, 2, or 3)
   */
  setKWeight(kWeight: number): void {
    lib.fitting_dataset_set_k_weight(this.handle, kWeight);
    this._kWeight = kWeight;
  }
  
  /**
   * Get the k-weighting
   */
  getKWeight(): number {
    return this._kWeight;
  }
}

/**
 * Result of an EXAFS fit
 */
export interface FitResult {
  /** Whether the fit was successful */
  success: boolean;
  /** Message describing the fit result */
  message: string;
  /** Number of function evaluations */
  nfev: number;
  /** Reduced chi-square (goodness of fit) */
  redchi: number;
  /** Best-fit parameters */
  params: FittingParameters;
  /** Best-fit chi(k) values */
  best_fit: Float64Array;
}

/**
 * EXAFS fitter for analysis
 */
export class ExafsFitter {
  // Paths to fit
  private paths: SimplePath[] = [];
  
  /**
   * Create a new EXAFS fitter
   */
  constructor() {
    // Initialize the fitter
    lib.exafs_fitter_new();
  }
  
  /**
   * Add a path to the fit model
   * 
   * @param path Path to add
   */
  addPath(path: SimplePath): void {
    lib.exafs_fitter_add_path(path.handle);
    this.paths.push(path);
  }
  
  /**
   * Perform the fit
   * 
   * @param dataset Dataset to fit
   * @param params Initial parameters
   * @returns Fit result
   */
  fit(dataset: FittingDataset, params: FittingParameters): FitResult {
    const result = lib.exafs_fitter_fit(dataset.handle, params.handle);
    
    // Convert best_fit to Float64Array
    const best_fit = bufferToFloat64Array(result.best_fit, result.best_fit_length);
    
    return {
      success: result.success,
      message: result.message,
      nfev: result.nfev,
      redchi: result.redchi,
      params,
      best_fit
    };
  }
}