/**
 * XRayTsubaki TypeScript Bindings
 * 
 * This module provides TypeScript bindings for the XRayTsubaki Rust library,
 * which implements X-ray absorption spectroscopy (XAS) analysis tools.
 */

// Export XAS core classes
export { XASSpectrum } from './xasspectrum';
export { XASGroup } from './xasgroup';

// Export XAFS functions
export {
  findE0,
  preEdge,
  autobk,
  xftf,
  xftr,
  // Export option interfaces
  PreEdgeOptions,
  AutobkOptions,
  XftfOptions,
  XftrOptions
} from './functions';

// Export fitting functionality
export {
  FittingParameter,
  FittingParameters,
  SimplePath,
  FittingDataset,
  ExafsFitter,
  // Export interfaces
  FitResult
} from './fitting';

// Export multi-spectrum fitting functionality
export {
  ParameterConstraintType,
  ConstrainedParameter,
  ConstrainedParameters,
  MultiSpectrumDataset,
  MultiSpectrumFitter,
  // Export interfaces
  ParameterConstraint,
  MultiSpectrumFitResult
} from './multispectrum';

// Export types
export { WindowFunction } from './ffi/types';