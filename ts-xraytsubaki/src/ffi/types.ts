/**
 * Type definitions for FFI bindings to the XRayTsubaki Rust library
 */

import * as ref from 'ref-napi';
import * as StructType from 'ref-struct-napi';
import * as ArrayType from 'ref-array-napi';

// Basic types for FFI
export const float64Type = ref.types.double;
export const float64PtrType = ref.refType(float64Type);
export const size_tType = ref.types.size_t;
export const boolType = ref.types.bool;
export const voidType = ref.types.void;
export const stringType = ref.types.CString;
export const stringPtrType = ref.refType(stringType);

// Array type for float64
export const Float64ArrayType = ArrayType(float64Type);

// FFI struct types
export const XASSpectrumType = StructType({
  // Handle to the underlying Rust XASSpectrum
  handle: ref.refType(voidType)
});

export const XASGroupType = StructType({
  // Handle to the underlying Rust XASGroup
  handle: ref.refType(voidType)
});

export const NormalizationResultType = StructType({
  pre: float64PtrType,
  post: float64PtrType,
  norm: float64PtrType,
  edge_step: float64Type,
  pre_slope: float64Type,
  pre_intercept: float64Type,
  post_slope: float64Type,
  post_intercept: float64Type,
  length: size_tType
});

export const BackgroundResultType = StructType({
  k: float64PtrType,
  chi: float64PtrType,
  kmin: float64Type,
  kmax: float64Type,
  length: size_tType
});

export const FourierResultType = StructType({
  r: float64PtrType,
  chir_mag: float64PtrType,
  chir_re: float64PtrType,
  chir_im: float64PtrType,
  length: size_tType
});

export const InverseFourierResultType = StructType({
  q: float64PtrType,
  chiq: float64PtrType,
  length: size_tType
});

export const FittingParameterType = StructType({
  // Handle to the underlying Rust FittingParameter
  handle: ref.refType(voidType)
});

export const FittingParametersType = StructType({
  // Handle to the underlying Rust FittingParameters
  handle: ref.refType(voidType)
});

export const SimplePathType = StructType({
  // Handle to the underlying Rust SimplePath
  handle: ref.refType(voidType)
});

export const FittingDatasetType = StructType({
  // Handle to the underlying Rust FittingDataset
  handle: ref.refType(voidType)
});

export const FitResultType = StructType({
  // Handle to the underlying Rust FitResult
  handle: ref.refType(voidType),
  success: boolType,
  message: stringType,
  nfev: size_tType,
  redchi: float64Type,
  best_fit: float64PtrType,
  best_fit_length: size_tType
});

export const ConstrainedParameterType = StructType({
  // Handle to the underlying Rust ConstrainedParameter
  handle: ref.refType(voidType)
});

export const ConstrainedParametersType = StructType({
  // Handle to the underlying Rust ConstrainedParameters
  handle: ref.refType(voidType)
});

export const MultiSpectrumDatasetType = StructType({
  // Handle to the underlying Rust MultiSpectrumDataset
  handle: ref.refType(voidType)
});

export const MultiSpectrumFitResultType = StructType({
  // Handle to the underlying Rust MultiSpectrumFitResult
  handle: ref.refType(voidType),
  success: boolType,
  message: stringType,
  nfev: size_tType,
  redchi: float64Type
});

// Window function types for FFT
export enum WindowFunction {
  Gaussian = 'Gaussian',
  Hanning = 'Hanning',
  KaiserBessel = 'Kaiser-Bessel',
  Parzen = 'Parzen',
  Sine = 'Sine',
  Welch = 'Welch'
}

// Parameter constraint types
export enum ConstraintType {
  None = 'none',
  Reference = 'reference',
  Scale = 'scale',
  Offset = 'offset',
  Formula = 'formula'
}