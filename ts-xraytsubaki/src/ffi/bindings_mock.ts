/**
 * Mock FFI bindings for testing
 */

// Mock lib object with empty functions
export const lib = {
  // XASSpectrum functions
  xas_spectrum_new: () => ({}),
  xas_spectrum_free: () => {},
  xas_spectrum_set_name: () => {},
  xas_spectrum_get_name: () => "mock-name",
  xas_spectrum_set_data: () => {},
  xas_spectrum_get_energy: () => Buffer.alloc(0),
  xas_spectrum_get_mu: () => Buffer.alloc(0),
  xas_spectrum_get_length: () => 0,
  xas_spectrum_find_e0: () => 0,
  xas_spectrum_get_e0: () => 0,
  xas_spectrum_normalize: () => ({ edge_step: 1.0, length: 0, pre: Buffer.alloc(0), post: Buffer.alloc(0), norm: Buffer.alloc(0) }),
  xas_spectrum_calc_background: () => ({ length: 0, k: Buffer.alloc(0), chi: Buffer.alloc(0), kmin: 0, kmax: 15 }),
  xas_spectrum_get_k: () => Buffer.alloc(0),
  xas_spectrum_get_chi: () => Buffer.alloc(0),
  xas_spectrum_fft: () => ({ length: 0, r: Buffer.alloc(0), chir_mag: Buffer.alloc(0), chir_re: Buffer.alloc(0), chir_im: Buffer.alloc(0) }),
  xas_spectrum_get_r: () => Buffer.alloc(0),
  xas_spectrum_get_chir_mag: () => Buffer.alloc(0),
  xas_spectrum_get_chir_re: () => Buffer.alloc(0),
  xas_spectrum_get_chir_im: () => Buffer.alloc(0),
  xas_spectrum_ifft: () => ({ length: 0, q: Buffer.alloc(0), chiq: Buffer.alloc(0) }),
  xas_spectrum_get_q: () => Buffer.alloc(0),
  xas_spectrum_get_chiq: () => Buffer.alloc(0),
  xas_spectrum_load_file: () => ({}),
  xas_spectrum_save_file: () => true,
  
  // XASGroup functions
  xas_group_new: () => ({}),
  xas_group_free: () => {},
  xas_group_add_spectrum: () => {},
  xas_group_length: () => 0,
  xas_group_get_spectrum: () => ({}),
  xas_group_remove_spectrum: () => true,
  xas_group_remove_spectra: () => true,
  xas_group_find_e0: () => {},
  xas_group_normalize: () => {},
  xas_group_calc_background: () => {},
  xas_group_fft: () => {},
  xas_group_ifft: () => {},
  xas_group_add_group: () => {},
  xas_group_save_json: () => true,
  xas_group_load_json: () => ({}),
  
  // XAFS functions
  xas_find_e0: () => 0,
  xas_pre_edge: () => ({ length: 0, pre: Buffer.alloc(0), post: Buffer.alloc(0), norm: Buffer.alloc(0), edge_step: 1.0 }),
  xas_autobk: () => ({ length: 0, k: Buffer.alloc(0), chi: Buffer.alloc(0), kmin: 0, kmax: 15 }),
  xas_xftf: () => ({ length: 0, r: Buffer.alloc(0), chir_mag: Buffer.alloc(0), chir_re: Buffer.alloc(0), chir_im: Buffer.alloc(0) }),
  xas_xftr: () => ({ length: 0, q: Buffer.alloc(0), chiq: Buffer.alloc(0) }),
  
  // Fitting functions
  fitting_parameter_new: () => ({}),
  fitting_parameter_free: () => {},
  fitting_parameter_get_name: () => "mock-param",
  fitting_parameter_get_value: () => 0,
  fitting_parameter_set_value: () => {},
  fitting_parameter_set_min: () => {},
  fitting_parameter_set_max: () => {},
  fitting_parameter_set_vary: () => {},
  fitting_parameters_new: () => ({}),
  fitting_parameters_free: () => {},
  fitting_parameters_add: () => {},
  fitting_parameters_get: () => ({}),
  fitting_parameters_size: () => 0,
  simple_path_new: () => ({}),
  simple_path_free: () => {},
  simple_path_set_s02: () => {},
  simple_path_set_e0: () => {},
  simple_path_set_sigma2: () => {},
  simple_path_set_delr: () => {},
  fitting_dataset_new: () => ({}),
  fitting_dataset_free: () => {},
  fitting_dataset_set_k_range: () => {},
  fitting_dataset_set_k_weight: () => {},
  exafs_fitter_new: () => {},
  exafs_fitter_add_path: () => {},
  exafs_fitter_fit: () => ({ success: true, message: "Mock fit", nfev: 10, redchi: 1.0, best_fit: Buffer.alloc(0), best_fit_length: 0 }),
  
  // MultiSpectrum functions
  constrained_parameter_new: () => ({}),
  constrained_parameter_free: () => {},
  constrained_parameter_refer_to: () => {},
  constrained_parameter_scale_from: () => {},
  constrained_parameter_offset_from: () => {},
  constrained_parameter_reset_constraint: () => {},
  constrained_parameters_new: () => ({}),
  constrained_parameters_free: () => {},
  constrained_parameters_add: () => ({}),
  constrained_parameters_get: () => ({}),
  constrained_parameters_size: () => 0,
  constrained_parameters_update_constraints: () => {},
  multi_spectrum_dataset_new: () => ({}),
  multi_spectrum_dataset_free: () => {},
  multi_spectrum_dataset_add: () => {},
  multi_spectrum_dataset_size: () => 0,
  multi_spectrum_fitter_new: () => {},
  multi_spectrum_fitter_add_path: () => {},
  multi_spectrum_fitter_fit: () => ({ success: true, message: "Mock fit", nfev: 10, redchi: 1.0 }),
  multi_spectrum_get_best_fit: () => Buffer.alloc(0),
  multi_spectrum_get_best_fit_length: () => 0
};

// Helper function to convert a Float64Array to a buffer for FFI
export function float64ArrayToBuffer(array: Float64Array): Buffer {
  const buffer = Buffer.alloc(array.length * 8);
  for (let i = 0; i < array.length; i++) {
    buffer.writeDoubleLE(array[i], i * 8);
  }
  return buffer;
}

// Helper function to convert a buffer to a Float64Array
export function bufferToFloat64Array(buffer: Buffer, length: number): Float64Array {
  const array = new Float64Array(length);
  for (let i = 0; i < length; i++) {
    array[i] = buffer.readDoubleLE(i * 8);
  }
  return array;
}