/**
 * FFI bindings to the XRayTsubaki Rust library
 */

import * as ffi from 'ffi-napi';
import * as path from 'path';
import * as os from 'os';
import {
  float64Type,
  float64PtrType,
  size_tType,
  boolType,
  voidType,
  stringType,
  XASSpectrumType,
  XASGroupType,
  NormalizationResultType,
  BackgroundResultType,
  FourierResultType,
  InverseFourierResultType,
  FittingParameterType,
  FittingParametersType,
  SimplePathType,
  FittingDatasetType,
  FitResultType,
  ConstrainedParameterType,
  ConstrainedParametersType,
  MultiSpectrumDatasetType,
  MultiSpectrumFitResultType
} from './types';

// Determine the library extension based on the platform
function getLibraryPath(): string {
  const platform = os.platform();
  let libPath: string;
  
  switch (platform) {
    case 'darwin':
      libPath = path.resolve(__dirname, '../../../target/release/libxraytsubaki.dylib');
      break;
    case 'linux':
      libPath = path.resolve(__dirname, '../../../target/release/libxraytsubaki.so');
      break;
    case 'win32':
      libPath = path.resolve(__dirname, '../../../target/release/xraytsubaki.dll');
      break;
    default:
      throw new Error(`Unsupported platform: ${platform}`);
  }
  
  return libPath;
}

// Load the XRayTsubaki library
export const lib = ffi.Library(getLibraryPath(), {
  // ===== XASSpectrum Functions =====
  
  // Create a new XASSpectrum
  'xas_spectrum_new': [XASSpectrumType, []],
  
  // Free an XASSpectrum
  'xas_spectrum_free': [voidType, [XASSpectrumType]],
  
  // Set the name of an XASSpectrum
  'xas_spectrum_set_name': [voidType, [XASSpectrumType, stringType]],
  
  // Get the name of an XASSpectrum
  'xas_spectrum_get_name': [stringType, [XASSpectrumType]],
  
  // Set the spectrum data
  'xas_spectrum_set_data': [voidType, [
    XASSpectrumType,
    float64PtrType,
    float64PtrType,
    size_tType
  ]],
  
  // Get the energy array
  'xas_spectrum_get_energy': [float64PtrType, [XASSpectrumType, size_tType]],
  
  // Get the mu array
  'xas_spectrum_get_mu': [float64PtrType, [XASSpectrumType, size_tType]],
  
  // Get the array length
  'xas_spectrum_get_length': [size_tType, [XASSpectrumType]],
  
  // Find the edge energy (E0)
  'xas_spectrum_find_e0': [float64Type, [XASSpectrumType]],
  
  // Get the edge energy (E0)
  'xas_spectrum_get_e0': [float64Type, [XASSpectrumType]],
  
  // Perform normalization
  'xas_spectrum_normalize': [
    NormalizationResultType,
    [
      XASSpectrumType,
      float64Type,
      float64Type,
      float64Type,
      float64Type
    ]
  ],
  
  // Calculate background and extract EXAFS
  'xas_spectrum_calc_background': [
    BackgroundResultType,
    [
      XASSpectrumType,
      float64Type,
      float64Type,
      float64Type,
      float64Type
    ]
  ],
  
  // Get the k array
  'xas_spectrum_get_k': [float64PtrType, [XASSpectrumType, size_tType]],
  
  // Get the chi array
  'xas_spectrum_get_chi': [float64PtrType, [XASSpectrumType, size_tType]],
  
  // Perform Fourier transform
  'xas_spectrum_fft': [
    FourierResultType,
    [
      XASSpectrumType,
      float64Type,
      float64Type,
      float64Type,
      stringType,
      float64Type
    ]
  ],
  
  // Get the R array
  'xas_spectrum_get_r': [float64PtrType, [XASSpectrumType, size_tType]],
  
  // Get the chi(R) magnitude array
  'xas_spectrum_get_chir_mag': [float64PtrType, [XASSpectrumType, size_tType]],
  
  // Get the chi(R) real part array
  'xas_spectrum_get_chir_re': [float64PtrType, [XASSpectrumType, size_tType]],
  
  // Get the chi(R) imaginary part array
  'xas_spectrum_get_chir_im': [float64PtrType, [XASSpectrumType, size_tType]],
  
  // Perform inverse Fourier transform
  'xas_spectrum_ifft': [
    InverseFourierResultType,
    [
      XASSpectrumType,
      float64Type,
      float64Type,
      float64Type,
      stringType
    ]
  ],
  
  // Get the q array
  'xas_spectrum_get_q': [float64PtrType, [XASSpectrumType, size_tType]],
  
  // Get the chi(q) array
  'xas_spectrum_get_chiq': [float64PtrType, [XASSpectrumType, size_tType]],
  
  // Load an XAS spectrum from a file
  'xas_spectrum_load_file': [XASSpectrumType, [stringType]],
  
  // Save an XAS spectrum to a file
  'xas_spectrum_save_file': [boolType, [XASSpectrumType, stringType]],
  
  // ===== XASGroup Functions =====
  
  // Create a new XASGroup
  'xas_group_new': [XASGroupType, []],
  
  // Free an XASGroup
  'xas_group_free': [voidType, [XASGroupType]],
  
  // Add a spectrum to the group
  'xas_group_add_spectrum': [voidType, [XASGroupType, XASSpectrumType]],
  
  // Get the number of spectra in the group
  'xas_group_length': [size_tType, [XASGroupType]],
  
  // Get a spectrum from the group
  'xas_group_get_spectrum': [XASSpectrumType, [XASGroupType, size_tType]],
  
  // Remove a spectrum from the group
  'xas_group_remove_spectrum': [boolType, [XASGroupType, size_tType]],
  
  // Remove multiple spectra from the group
  'xas_group_remove_spectra': [boolType, [XASGroupType, size_tType, size_tType]],
  
  // Find E0 for all spectra in the group
  'xas_group_find_e0': [voidType, [XASGroupType]],
  
  // Normalize all spectra in the group
  'xas_group_normalize': [voidType, [XASGroupType]],
  
  // Calculate background for all spectra in the group
  'xas_group_calc_background': [voidType, [XASGroupType]],
  
  // Perform FFT for all spectra in the group
  'xas_group_fft': [voidType, [XASGroupType]],
  
  // Perform IFFT for all spectra in the group
  'xas_group_ifft': [voidType, [XASGroupType]],
  
  // Add another group to this group
  'xas_group_add_group': [voidType, [XASGroupType, XASGroupType]],
  
  // Save the group to a JSON file
  'xas_group_save_json': [boolType, [XASGroupType, stringType]],
  
  // Load a group from a JSON file
  'xas_group_load_json': [XASGroupType, [stringType]],
  
  // ===== XAFS Functions =====
  
  // Find the edge energy (E0)
  'xas_find_e0': [float64Type, [float64PtrType, float64PtrType, size_tType]],
  
  // Perform pre-edge normalization
  'xas_pre_edge': [
    NormalizationResultType,
    [
      float64PtrType,
      float64PtrType,
      size_tType,
      float64Type,
      float64Type,
      float64Type,
      float64Type,
      float64Type,
      boolType
    ]
  ],
  
  // Perform autobk background subtraction
  'xas_autobk': [
    BackgroundResultType,
    [
      float64PtrType,
      float64PtrType,
      size_tType,
      float64Type,
      float64Type,
      float64Type,
      float64Type,
      float64Type
    ]
  ],
  
  // Perform forward Fourier transform
  'xas_xftf': [
    FourierResultType,
    [
      float64PtrType,
      float64PtrType,
      size_tType,
      float64Type,
      float64Type,
      float64Type,
      stringType,
      float64Type
    ]
  ],
  
  // Perform inverse Fourier transform
  'xas_xftr': [
    InverseFourierResultType,
    [
      float64PtrType,
      float64PtrType,
      float64PtrType,
      float64PtrType,
      size_tType,
      float64Type,
      float64Type,
      float64Type,
      stringType
    ]
  ],
  
  // ===== Fitting Functions =====
  
  // Create a new FittingParameter
  'fitting_parameter_new': [FittingParameterType, [stringType, float64Type]],
  
  // Free a FittingParameter
  'fitting_parameter_free': [voidType, [FittingParameterType]],
  
  // Get the name of a FittingParameter
  'fitting_parameter_get_name': [stringType, [FittingParameterType]],
  
  // Get the value of a FittingParameter
  'fitting_parameter_get_value': [float64Type, [FittingParameterType]],
  
  // Set the value of a FittingParameter
  'fitting_parameter_set_value': [voidType, [FittingParameterType, float64Type]],
  
  // Set the min value of a FittingParameter
  'fitting_parameter_set_min': [voidType, [FittingParameterType, float64Type]],
  
  // Set the max value of a FittingParameter
  'fitting_parameter_set_max': [voidType, [FittingParameterType, float64Type]],
  
  // Set whether a FittingParameter should vary during fitting
  'fitting_parameter_set_vary': [voidType, [FittingParameterType, boolType]],
  
  // Create a new FittingParameters set
  'fitting_parameters_new': [FittingParametersType, []],
  
  // Free a FittingParameters set
  'fitting_parameters_free': [voidType, [FittingParametersType]],
  
  // Add a parameter to a FittingParameters set
  'fitting_parameters_add': [voidType, [
    FittingParametersType,
    stringType,
    float64Type,
    float64Type,
    float64Type,
    boolType
  ]],
  
  // Get a parameter from a FittingParameters set
  'fitting_parameters_get': [FittingParameterType, [FittingParametersType, stringType]],
  
  // Get the number of parameters in a FittingParameters set
  'fitting_parameters_size': [size_tType, [FittingParametersType]],
  
  // Create a new SimplePath
  'simple_path_new': [SimplePathType, [stringType, float64Type, float64Type]],
  
  // Free a SimplePath
  'simple_path_free': [voidType, [SimplePathType]],
  
  // Set the S02 parameter name for a SimplePath
  'simple_path_set_s02': [voidType, [SimplePathType, stringType]],
  
  // Set the E0 parameter name for a SimplePath
  'simple_path_set_e0': [voidType, [SimplePathType, stringType]],
  
  // Set the sigma² parameter name for a SimplePath
  'simple_path_set_sigma2': [voidType, [SimplePathType, stringType]],
  
  // Set the delR parameter name for a SimplePath
  'simple_path_set_delr': [voidType, [SimplePathType, stringType]],
  
  // Create a new FittingDataset
  'fitting_dataset_new': [FittingDatasetType, [
    float64PtrType,
    float64PtrType,
    size_tType
  ]],
  
  // Free a FittingDataset
  'fitting_dataset_free': [voidType, [FittingDatasetType]],
  
  // Set the k range for a FittingDataset
  'fitting_dataset_set_k_range': [voidType, [
    FittingDatasetType,
    float64Type,
    float64Type
  ]],
  
  // Set the k weight for a FittingDataset
  'fitting_dataset_set_k_weight': [voidType, [FittingDatasetType, float64Type]],
  
  // Create a new ExafsFitter
  'exafs_fitter_new': [voidType, []],
  
  // Add a path to the ExafsFitter
  'exafs_fitter_add_path': [voidType, [SimplePathType]],
  
  // Perform the EXAFS fit
  'exafs_fitter_fit': [FitResultType, [
    FittingDatasetType,
    FittingParametersType
  ]],
  
  // ===== MultiSpectrum Fitting Functions =====
  
  // Create a new ConstrainedParameter
  'constrained_parameter_new': [ConstrainedParameterType, [stringType, float64Type]],
  
  // Free a ConstrainedParameter
  'constrained_parameter_free': [voidType, [ConstrainedParameterType]],
  
  // Set a ConstrainedParameter to refer to another parameter
  'constrained_parameter_refer_to': [voidType, [
    ConstrainedParameterType,
    stringType
  ]],
  
  // Set a ConstrainedParameter to scale from another parameter
  'constrained_parameter_scale_from': [voidType, [
    ConstrainedParameterType,
    stringType,
    float64Type
  ]],
  
  // Set a ConstrainedParameter to offset from another parameter
  'constrained_parameter_offset_from': [voidType, [
    ConstrainedParameterType,
    stringType,
    float64Type
  ]],
  
  // Reset any constraint on a ConstrainedParameter
  'constrained_parameter_reset_constraint': [voidType, [ConstrainedParameterType]],
  
  // Create a new ConstrainedParameters set
  'constrained_parameters_new': [ConstrainedParametersType, []],
  
  // Free a ConstrainedParameters set
  'constrained_parameters_free': [voidType, [ConstrainedParametersType]],
  
  // Add a parameter to a ConstrainedParameters set
  'constrained_parameters_add': [ConstrainedParameterType, [
    ConstrainedParametersType,
    stringType,
    float64Type,
    float64Type,
    float64Type,
    boolType
  ]],
  
  // Get a parameter from a ConstrainedParameters set
  'constrained_parameters_get': [ConstrainedParameterType, [
    ConstrainedParametersType,
    stringType
  ]],
  
  // Get the number of parameters in a ConstrainedParameters set
  'constrained_parameters_size': [size_tType, [ConstrainedParametersType]],
  
  // Update all constraints in a ConstrainedParameters set
  'constrained_parameters_update_constraints': [voidType, [ConstrainedParametersType]],
  
  // Create a new MultiSpectrumDataset
  'multi_spectrum_dataset_new': [MultiSpectrumDatasetType, []],
  
  // Free a MultiSpectrumDataset
  'multi_spectrum_dataset_free': [voidType, [MultiSpectrumDatasetType]],
  
  // Add a dataset to a MultiSpectrumDataset
  'multi_spectrum_dataset_add': [voidType, [
    MultiSpectrumDatasetType,
    stringType,
    FittingDatasetType
  ]],
  
  // Get the number of datasets in a MultiSpectrumDataset
  'multi_spectrum_dataset_size': [size_tType, [MultiSpectrumDatasetType]],
  
  // Create a new MultiSpectrumFitter
  'multi_spectrum_fitter_new': [voidType, []],
  
  // Add a path to the MultiSpectrumFitter for a specific spectrum
  'multi_spectrum_fitter_add_path': [voidType, [
    stringType,
    SimplePathType
  ]],
  
  // Perform the multi-spectrum fit
  'multi_spectrum_fitter_fit': [MultiSpectrumFitResultType, [
    MultiSpectrumDatasetType,
    ConstrainedParametersType
  ]]
});

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