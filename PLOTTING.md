# XRayTsubaki Plotting Functionality

This document outlines the approach to implementing plotting capabilities in the XRayTsubaki library.

## Overview

The XRayTsubaki library will implement direct plotting capabilities for visualizing X-ray Absorption Spectroscopy (XAS) data at various stages of analysis. The implementation will use the Plotters library, which is already used in the GUI portion of the project, to provide consistent visualization across the library and GUI application.

## Design Goals

1. **Independence from GUI**: The plotting functionality should work independently from the GUI application.
2. **Multiple Output Formats**: Support for multiple output formats (PNG, SVG, etc.)
3. **Customizable Appearance**: Users should be able to customize plot appearance.
4. **Integration with Existing Structures**: Tight integration with existing XRayTsubaki data structures.
5. **Simple API**: Provide a simple, intuitive API for generating common XAS plots.
6. **Advanced Options**: Allow advanced customization for specific needs.

## Library Selection: Plotters

We have selected the [Plotters](https://github.com/plotters-rs/plotters) library for the following reasons:

1. **Pure Rust Implementation**: No external dependencies or bindings to other languages.
2. **Multiple Backends**: Supports PNG, SVG, and other output formats.
3. **Already Used in GUI**: Consistency with the GUI portion of the application.
4. **Active Development**: Under active maintenance and development.
5. **Good Documentation**: Well-documented with examples.
6. **Flexible API**: Provides both high-level and low-level APIs for customization.

## Architecture

The plotting functionality will be implemented as a module within the XRayTsubaki library. The module will provide:

1. **Trait Implementation**: XASSpectrum and XASGroup will implement a `Plottable` trait.
2. **Plot Builder Pattern**: A builder pattern for configuring plot options.
3. **Common Plot Types**: Pre-configured functions for common XAS plot types.
4. **Customization Options**: Methods for customizing the appearance of plots.

### Module Structure

```
xraytsubaki::plot/
├── mod.rs         # Module definition and exports
├── traits.rs      # Plottable trait and associated types
├── builders.rs    # Builder patterns for plot configuration
├── xanes.rs       # XANES-specific plotting functions
├── exafs.rs       # EXAFS-specific plotting functions
├── fitting.rs     # Fitting visualization functions
└── utils.rs       # Utility functions for plotting
```

## API Design

### Plottable Trait

```rust
pub trait Plottable {
    fn plot<B: DrawingBackend>(&self, backend: B) -> Result<(), PlotError>;
    fn plot_to_file(&self, path: &str) -> Result<(), PlotError>;
    fn get_plot_builder(&self) -> PlotBuilder;
}
```

### Plot Builder

```rust
pub struct PlotBuilder<'a, T: Plottable> {
    plottable: &'a T,
    title: Option<String>,
    x_label: Option<String>,
    y_label: Option<String>,
    x_range: Option<(f64, f64)>,
    y_range: Option<(f64, f64)>,
    width: usize,
    height: usize,
    // ... other options
}

impl<'a, T: Plottable> PlotBuilder<'a, T> {
    pub fn new(plottable: &'a T) -> Self { /* ... */ }
    pub fn title(mut self, title: &str) -> Self { /* ... */ }
    pub fn x_label(mut self, label: &str) -> Self { /* ... */ }
    pub fn y_label(mut self, label: &str) -> Self { /* ... */ }
    pub fn x_range(mut self, min: f64, max: f64) -> Self { /* ... */ }
    pub fn y_range(mut self, min: f64, max: f64) -> Self { /* ... */ }
    pub fn dimensions(mut self, width: usize, height: usize) -> Self { /* ... */ }
    
    pub fn build<B: DrawingBackend>(self, backend: B) -> Result<(), PlotError> { /* ... */ }
    pub fn save(self, path: &str) -> Result<(), PlotError> { /* ... */ }
}
```

### Implementation for XASSpectrum

```rust
impl Plottable for XASSpectrum {
    fn plot<B: DrawingBackend>(&self, backend: B) -> Result<(), PlotError> {
        self.get_plot_builder().build(backend)
    }
    
    fn plot_to_file(&self, path: &str) -> Result<(), PlotError> {
        self.get_plot_builder().save(path)
    }
    
    fn get_plot_builder(&self) -> PlotBuilder {
        PlotBuilder::new(self)
            .title("XAS Spectrum")
            .x_label("Energy (eV)")
            .y_label("Absorption")
            // ... other default settings
    }
}

// Extension methods for XASSpectrum
impl XASSpectrum {
    pub fn plot_normalized(&self, path: &str) -> Result<(), PlotError> { /* ... */ }
    pub fn plot_k_space(&self, path: &str) -> Result<(), PlotError> { /* ... */ }
    pub fn plot_r_space(&self, path: &str) -> Result<(), PlotError> { /* ... */ }
    // ... other specialized plotting methods
}
```

### Implementation for XASGroup

```rust
impl Plottable for XASGroup {
    fn plot<B: DrawingBackend>(&self, backend: B) -> Result<(), PlotError> {
        self.get_plot_builder().build(backend)
    }
    
    fn plot_to_file(&self, path: &str) -> Result<(), PlotError> {
        self.get_plot_builder().save(path)
    }
    
    fn get_plot_builder(&self) -> PlotBuilder {
        PlotBuilder::new(self)
            .title("XAS Group")
            .x_label("Energy (eV)")
            .y_label("Absorption")
            // ... other default settings
    }
}

// Extension methods for XASGroup
impl XASGroup {
    pub fn plot_normalized(&self, path: &str) -> Result<(), PlotError> { /* ... */ }
    pub fn plot_k_space(&self, path: &str) -> Result<(), PlotError> { /* ... */ }
    pub fn plot_r_space(&self, path: &str) -> Result<(), PlotError> { /* ... */ }
    // ... other specialized plotting methods
}
```

## Common Plot Types

### XANES Plots

1. **Raw Data Plot**: Plot of raw energy vs. absorption
2. **Normalized Plot**: Plot of normalized XANES data
3. **Derivative Plot**: First derivative of XANES data
4. **Pre-edge Plot**: Plot showing pre-edge and post-edge fits

### EXAFS Plots

1. **k-space Plot**: Chi(k) vs. k
2. **k-weighted Plot**: k^n * Chi(k) vs. k
3. **R-space Magnitude Plot**: |Chi(R)| vs. R
4. **R-space Real and Imaginary Plot**: Re[Chi(R)] and Im[Chi(R)] vs. R

### Fitting Visualization

1. **Fit Results Plot**: Data with fit overlay
2. **Residuals Plot**: Difference between data and fit
3. **Component Plot**: Individual components of a fit
4. **Parameter Correlation Plot**: Visual representation of parameter correlations

## Output Formats

The plotting functionality will support the following output formats:

1. **PNG**: Raster image format
2. **SVG**: Vector image format
3. **PDF**: Document format (via SVG conversion)
4. **HTML**: Interactive web plots (optional, via Plotly integration)

## Implementation Plan

1. **Core Implementation**: Implement the Plottable trait and basic plot builder.
2. **XANES Plotting**: Implement plotting functions for XANES data.
3. **EXAFS k-space Plotting**: Implement plotting functions for EXAFS k-space data.
4. **EXAFS R-space Plotting**: Implement plotting functions for EXAFS R-space data.
5. **Fitting Visualization**: Implement plotting functions for fitting results.
6. **Plot Customization**: Implement options for customizing plot appearance.
7. **Examples**: Create example scripts demonstrating plotting capabilities.

## Examples

### Basic Usage

```rust
use xraytsubaki::prelude::*;

fn main() -> Result<(), Box<dyn Error>> {
    // Load spectrum
    let spectrum = io::load_spectrum_QAS_trans("path/to/data.dat")?;
    
    // Plot raw data
    spectrum.plot_to_file("raw_data.png")?;
    
    // Process data
    spectrum.find_e0()?;
    spectrum.normalize()?;
    
    // Plot normalized data with customization
    spectrum.get_plot_builder()
        .title("Normalized XAS Spectrum")
        .x_label("Energy (eV)")
        .y_label("Normalized Absorption")
        .x_range(spectrum.e0.unwrap() - 50.0, spectrum.e0.unwrap() + 200.0)
        .save("normalized.png")?;
    
    // Extract EXAFS
    spectrum.calc_background()?;
    
    // Plot k-space data
    spectrum.plot_k_space("k_space.png")?;
    
    // Plot R-space data
    spectrum.fft()?;
    spectrum.plot_r_space("r_space.png")?;
    
    Ok(())
}
```

### Group Plotting

```rust
use xraytsubaki::prelude::*;

fn main() -> Result<(), Box<dyn Error>> {
    // Create a group
    let mut group = XASGroup::new();
    
    // Add spectra
    group.add_spectrum(io::load_spectrum_QAS_trans("path/to/data1.dat")?);
    group.add_spectrum(io::load_spectrum_QAS_trans("path/to/data2.dat")?);
    group.add_spectrum(io::load_spectrum_QAS_trans("path/to/data3.dat")?);
    
    // Process all spectra
    group.find_e0()?;
    group.normalize()?;
    
    // Plot all normalized spectra
    group.plot_normalized("normalized_group.png")?;
    
    // Extract EXAFS for all spectra
    group.calc_background()?;
    
    // Plot k-space data for all spectra
    group.plot_k_space("k_space_group.png")?;
    
    // Plot R-space data for all spectra
    group.fft()?;
    group.plot_r_space("r_space_group.png")?;
    
    Ok(())
}
```

## Testing Strategy

1. **Unit Tests**: Test individual plotting functions.
2. **Integration Tests**: Test plotting with real XAS data.
3. **Visual Tests**: Generate reference plots and compare outputs.
4. **API Tests**: Test the usability and correctness of the API.

## Constraints and Considerations

1. **Performance**: Minimal impact on performance for core XAS functionality.
2. **Memory Usage**: Efficient memory usage, especially for large datasets.
3. **Error Handling**: Robust error handling and informative error messages.
4. **Dependencies**: Minimize external dependencies beyond Plotters.
5. **API Design**: Balance between simplicity and flexibility in the API.

## Future Extensions

1. **Interactive Plots**: Integration with web-based interactive plotting tools.
2. **3D Plots**: Support for 3D plotting for specialized analyses.
3. **Custom Plot Types**: User-defined plot types and templates.
4. **Animation**: Support for creating animations of time-dependent data.
5. **Plot Saving**: Save plot configurations for later use.