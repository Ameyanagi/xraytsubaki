/**
 * Multi-spectrum EXAFS fitting functionality
 */

import { lib, bufferToFloat64Array } from './ffi/bindings_mock';
import { FittingDataset, SimplePath } from './fitting';

/**
 * Types of parameter constraints
 */
export enum ParameterConstraintType {
  Reference = 'reference',
  Scale = 'scale',
  Offset = 'offset',
  Formula = 'formula'
}

/**
 * Constraint information for a parameter
 */
export interface ParameterConstraint {
  /** Type of constraint */
  type: keyof typeof ParameterConstraintType;
  /** Reference parameter name */
  reference: string;
  /** Scale factor (for 'scale' type) */
  factor?: number;
  /** Offset value (for 'offset' type) */
  offset?: number;
  /** Formula string (for 'formula' type) */
  formula?: string;
}

/**
 * Parameter with constraint support for multi-spectrum fitting
 */
export class ConstrainedParameter {
  // Internal FFI handle
  private handle: any;
  
  // Parameter properties
  private _name: string;
  private _value: number;
  private _min: number | null = null;
  private _max: number | null = null;
  private _vary: boolean = true;
  private _constraint: ParameterConstraint | null = null;
  
  /**
   * Create a new constrained parameter
   * 
   * @param name Parameter name
   * @param value Initial value
   * @param min Minimum value (optional)
   * @param max Maximum value (optional)
   * @param vary Whether to vary this parameter during fitting (default: true)
   */
  constructor(
    name: string,
    value: number,
    min: number | null = null,
    max: number | null = null,
    vary: boolean = true
  ) {
    this._name = name;
    this._value = value;
    this._min = min;
    this._max = max;
    this._vary = vary;
    
    this.handle = lib.constrained_parameter_new(name, value);
    
    if (min !== null) {
      lib.constrained_parameter_set_min(this.handle, min);
    }
    
    if (max !== null) {
      lib.constrained_parameter_set_max(this.handle, max);
    }
    
    if (!vary) {
      lib.constrained_parameter_set_vary(this.handle, false);
    }
  }
  
  /**
   * Clean up resources when object is garbage collected
   */
  destroy(): void {
    if (this.handle) {
      lib.constrained_parameter_free(this.handle);
      this.handle = null;
    }
  }
  
  /**
   * Get the parameter name
   */
  getName(): string {
    return this._name;
  }
  
  /**
   * Get the parameter value
   */
  getValue(): number {
    return this._value;
  }
  
  /**
   * Set the parameter value
   * 
   * @param value New value
   */
  setValue(value: number): void {
    lib.constrained_parameter_set_value(this.handle, value);
    this._value = value;
  }
  
  /**
   * Get the minimum allowed value
   */
  getMin(): number | null {
    return this._min;
  }
  
  /**
   * Set the minimum allowed value
   * 
   * @param min Minimum value
   */
  setMin(min: number): void {
    lib.constrained_parameter_set_min(this.handle, min);
    this._min = min;
  }
  
  /**
   * Get the maximum allowed value
   */
  getMax(): number | null {
    return this._max;
  }
  
  /**
   * Set the maximum allowed value
   * 
   * @param max Maximum value
   */
  setMax(max: number): void {
    lib.constrained_parameter_set_max(this.handle, max);
    this._max = max;
  }
  
  /**
   * Get whether this parameter is varied during fitting
   */
  getVary(): boolean {
    return this._vary;
  }
  
  /**
   * Set whether this parameter is varied during fitting
   * 
   * @param vary Whether to vary the parameter
   */
  setVary(vary: boolean): void {
    lib.constrained_parameter_set_vary(this.handle, vary);
    this._vary = vary;
  }
  
  /**
   * Get the parameter constraint
   */
  getConstraint(): ParameterConstraint | null {
    return this._constraint;
  }
  
  /**
   * Set this parameter to directly reference another parameter
   * 
   * @param reference Name of the reference parameter
   */
  referTo(reference: string): void {
    lib.constrained_parameter_refer_to(this.handle, reference);
    this._constraint = {
      type: 'reference',
      reference
    };
  }
  
  /**
   * Set this parameter to be scaled relative to another parameter
   * 
   * @param reference Name of the reference parameter
   * @param factor Scale factor
   */
  scaleFrom(reference: string, factor: number): void {
    lib.constrained_parameter_scale_from(this.handle, reference, factor);
    this._constraint = {
      type: 'scale',
      reference,
      factor
    };
  }
  
  /**
   * Set this parameter to be offset from another parameter
   * 
   * @param reference Name of the reference parameter
   * @param offset Offset value
   */
  offsetFrom(reference: string, offset: number): void {
    lib.constrained_parameter_offset_from(this.handle, reference, offset);
    this._constraint = {
      type: 'offset',
      reference,
      offset
    };
  }
  
  /**
   * Remove any constraint on this parameter
   */
  resetConstraint(): void {
    lib.constrained_parameter_reset_constraint(this.handle);
    this._constraint = null;
  }
}

/**
 * Collection of constrained parameters for multi-spectrum fitting
 */
export class ConstrainedParameters {
  // Internal FFI handle
  private handle: any;
  
  // Track parameters for easier access
  private parameters: Map<string, ConstrainedParameter> = new Map();
  
  /**
   * Create a new set of constrained parameters
   */
  constructor() {
    this.handle = lib.constrained_parameters_new();
  }
  
  /**
   * Clean up resources when object is garbage collected
   */
  destroy(): void {
    if (this.handle) {
      lib.constrained_parameters_free(this.handle);
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
   * @returns The created ConstrainedParameter object
   */
  add(
    name: string,
    value: number,
    min: number | null = null,
    max: number | null = null,
    vary: boolean = true
  ): ConstrainedParameter {
    const paramHandle = lib.constrained_parameters_add(
      this.handle,
      name,
      value,
      min !== null ? min : Number.MIN_SAFE_INTEGER,
      max !== null ? max : Number.MAX_SAFE_INTEGER,
      vary
    );
    
    const param = new ConstrainedParameter(name, value, min, max, vary);
    param.handle = paramHandle;
    
    this.parameters.set(name, param);
    
    return param;
  }
  
  /**
   * Get a parameter by name
   * 
   * @param name Parameter name
   * @returns The parameter, or null if not found
   */
  get(name: string): ConstrainedParameter | null {
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
    return lib.constrained_parameters_size(this.handle);
  }
  
  /**
   * Update all constrained parameters
   * This should be called after changing values of reference parameters
   */
  updateConstraints(): void {
    lib.constrained_parameters_update_constraints(this.handle);
    
    // Update internal values
    for (const param of this.parameters.values()) {
      if (param.getConstraint()) {
        // Refresh the value from the FFI
        const value = lib.constrained_parameter_get_value(param.handle);
        param.setValue(value);
      }
    }
  }
}

/**
 * Dataset for multi-spectrum fitting
 */
export class MultiSpectrumDataset {
  // Internal FFI handle
  private handle: any;
  
  // Track datasets for easier access
  private datasets: Map<string, FittingDataset> = new Map();
  
  /**
   * Create a new multi-spectrum dataset
   */
  constructor() {
    this.handle = lib.multi_spectrum_dataset_new();
  }
  
  /**
   * Clean up resources when object is garbage collected
   */
  destroy(): void {
    if (this.handle) {
      lib.multi_spectrum_dataset_free(this.handle);
      this.handle = null;
    }
  }
  
  /**
   * Add a dataset for a spectrum
   * 
   * @param name Spectrum name
   * @param dataset Fitting dataset
   */
  addDataset(name: string, dataset: FittingDataset): void {
    lib.multi_spectrum_dataset_add(this.handle, name, dataset.handle);
    this.datasets.set(name, dataset);
  }
  
  /**
   * Get a dataset by name
   * 
   * @param name Spectrum name
   * @returns The dataset, or null if not found
   */
  getDataset(name: string): FittingDataset | null {
    return this.datasets.get(name) || null;
  }
  
  /**
   * Get all spectrum names
   */
  getNames(): string[] {
    return Array.from(this.datasets.keys());
  }
  
  /**
   * Get the number of datasets
   */
  size(): number {
    return lib.multi_spectrum_dataset_size(this.handle);
  }
}

/**
 * Result of a multi-spectrum fit
 */
export interface MultiSpectrumFitResult {
  /** Whether the fit was successful */
  success: boolean;
  /** Message describing the fit result */
  message: string;
  /** Number of function evaluations */
  nfev: number;
  /** Reduced chi-square (goodness of fit) */
  redchi: number;
  /** Best-fit parameters */
  params: ConstrainedParameters;
  /** Best-fit chi(k) values for each spectrum */
  best_fits: Record<string, Float64Array>;
}

/**
 * Multi-spectrum EXAFS fitter
 */
export class MultiSpectrumFitter {
  // Paths for each spectrum
  private paths: Map<string, SimplePath[]> = new Map();
  
  /**
   * Create a new multi-spectrum fitter
   */
  constructor() {
    // Initialize the fitter
    lib.multi_spectrum_fitter_new();
  }
  
  /**
   * Add a path to the fit model for a specific spectrum
   * 
   * @param spectrumName Name of the spectrum
   * @param path Path to add
   */
  addPath(spectrumName: string, path: SimplePath): void {
    lib.multi_spectrum_fitter_add_path(spectrumName, path.handle);
    
    // Track the path
    if (!this.paths.has(spectrumName)) {
      this.paths.set(spectrumName, []);
    }
    
    this.paths.get(spectrumName)!.push(path);
  }
  
  /**
   * Perform the multi-spectrum fit
   * 
   * @param multiDataset Multi-spectrum dataset
   * @param params Initial parameters
   * @returns Fit result
   */
  fit(multiDataset: MultiSpectrumDataset, params: ConstrainedParameters): MultiSpectrumFitResult {
    const result = lib.multi_spectrum_fitter_fit(multiDataset.handle, params.handle);
    
    // Convert best_fits for each spectrum
    const best_fits: Record<string, Float64Array> = {};
    const spectrumNames = multiDataset.getNames();
    
    for (const name of spectrumNames) {
      const buffer = lib.multi_spectrum_get_best_fit(result.handle, name);
      const length = lib.multi_spectrum_get_best_fit_length(result.handle, name);
      
      if (buffer && length > 0) {
        best_fits[name] = bufferToFloat64Array(buffer, length);
      }
    }
    
    return {
      success: result.success,
      message: result.message,
      nfev: result.nfev,
      redchi: result.redchi,
      params,
      best_fits
    };
  }
}