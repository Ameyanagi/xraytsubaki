//! XANES plotting functionality
//!
//! This module provides specialized plotting functions for XANES data.

use plotters::prelude::*;
use crate::xafs::xasspectrum::XASSpectrum;
use super::{PlotResult, PlotError, traits::Plottable, builders::PlotBuilder};

/// Types of XANES plots
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum XANESPlotType {
    /// Raw energy vs. mu data
    Raw,
    /// Normalized mu(E) vs. energy
    Normalized,
    /// First derivative of mu(E) vs. energy
    Derivative,
    /// Pre-edge and post-edge fits
    PreEdgeFit,
}

/// Plot XANES data with the specified plot type
pub fn plot_xanes(
    spectrum: &XASSpectrum,
    path: &str,
    plot_type: XANESPlotType,
) -> PlotResult<()> {
    match plot_type {
        XANESPlotType::Raw => plot_raw_xanes(spectrum, path),
        XANESPlotType::Normalized => plot_normalized_xanes(spectrum, path),
        XANESPlotType::Derivative => plot_derivative_xanes(spectrum, path),
        XANESPlotType::PreEdgeFit => plot_preedge_fit(spectrum, path),
    }
}

/// Plot raw XANES data
fn plot_raw_xanes(spectrum: &XASSpectrum, path: &str) -> PlotResult<()> {
    // Check if required data is available
    if spectrum.energy.is_none() || spectrum.mu.is_none() {
        return Err(PlotError::MissingData("Energy or mu data missing for XANES plot".into()));
    }
    
    // Get plot builder with appropriate defaults
    let builder = spectrum.get_plot_builder()
        .title("Raw XANES Spectrum")
        .x_label("Energy (eV)")
        .y_label("Absorption");
    
    // Save the plot
    builder.save(path)
}

/// Plot normalized XANES data
fn plot_normalized_xanes(spectrum: &XASSpectrum, path: &str) -> PlotResult<()> {
    // Check if required data is available
    if spectrum.energy.is_none() || spectrum.normalization.is_none() {
        return Err(PlotError::MissingData("Energy or normalization data missing for normalized XANES plot".into()));
    }
    
    let energy = spectrum.energy.as_ref().unwrap();
    
    // Extract normalized data
    let normalization = spectrum.normalization.as_ref().unwrap();
    let norm_data = match normalization.get_norm() {
        Some(norm) => norm,
        None => return Err(PlotError::MissingData("Normalized data missing".into())),
    };
    
    // Create root area and fill with white
    let root = BitMapBackend::new(path, (800, 600)).into_drawing_area();
    root.fill(&WHITE)?;
    
    // Calculate plot ranges
    let x_min = energy.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let x_max = energy.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    
    let y_min = norm_data.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let y_max = norm_data.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    
    // Add margin to ranges
    let x_margin = (x_max - x_min) * 0.05;
    let y_margin = (y_max - y_min) * 0.05;
    
    // Build the chart
    let mut chart = ChartBuilder::on(&root)
        .caption(
            format!("Normalized XANES Spectrum {}", spectrum.name.clone().unwrap_or_default()),
            ("sans-serif", 30).into_font(),
        )
        .margin(40)
        .x_label_area_size(40)
        .y_label_area_size(60)
        .build_cartesian_2d(
            (x_min - x_margin)..(x_max + x_margin),
            (y_min - y_margin)..(y_max + y_margin),
        )?;
    
    // Configure the mesh
    chart
        .configure_mesh()
        .x_desc("Energy (eV)")
        .y_desc("Normalized Absorption")
        .axis_desc_style(("sans-serif", 20))
        .draw()?;
    
    // Draw the normalized data
    chart.draw_series(LineSeries::new(
        energy.iter().zip(norm_data.iter()).map(|(&x, &y)| (x, y)),
        &BLUE,
    ))?;
    
    // If e0 is set, draw a vertical line at e0
    if let Some(e0) = spectrum.e0 {
        chart.draw_series(LineSeries::new(
            vec![(e0, y_min - y_margin), (e0, y_max + y_margin)],
            &RED.mix(0.5),
        ))?;
    }
    
    // Draw a horizontal line at y=0 and y=1
    chart.draw_series(LineSeries::new(
        vec![(x_min - x_margin, 0.0), (x_max + x_margin, 0.0)],
        &BLACK.mix(0.3),
    ))?;
    
    chart.draw_series(LineSeries::new(
        vec![(x_min - x_margin, 1.0), (x_max + x_margin, 1.0)],
        &BLACK.mix(0.3),
    ))?;
    
    Ok(())
}

/// Plot derivative of XANES data
fn plot_derivative_xanes(spectrum: &XASSpectrum, path: &str) -> PlotResult<()> {
    // Check if required data is available
    if spectrum.energy.is_none() || spectrum.mu.is_none() {
        return Err(PlotError::MissingData("Energy or mu data missing for derivative XANES plot".into()));
    }
    
    let energy = spectrum.energy.as_ref().unwrap();
    let mu = spectrum.mu.as_ref().unwrap();
    
    // Calculate derivative using utility function
    let dmude = super::utils::derivative(energy, mu)?;
    
    // Create root area and fill with white
    let root = BitMapBackend::new(path, (800, 600)).into_drawing_area();
    root.fill(&WHITE)?;
    
    // Calculate plot ranges
    let x_min = energy.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let x_max = energy.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    
    let y_min = dmude.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let y_max = dmude.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    
    // Add margin to ranges
    let x_margin = (x_max - x_min) * 0.05;
    let y_margin = (y_max - y_min) * 0.05;
    
    // Build the chart
    let mut chart = ChartBuilder::on(&root)
        .caption(
            format!("XANES Derivative {}", spectrum.name.clone().unwrap_or_default()),
            ("sans-serif", 30).into_font(),
        )
        .margin(40)
        .x_label_area_size(40)
        .y_label_area_size(60)
        .build_cartesian_2d(
            (x_min - x_margin)..(x_max + x_margin),
            (y_min - y_margin)..(y_max + y_margin),
        )?;
    
    // Configure the mesh
    chart
        .configure_mesh()
        .x_desc("Energy (eV)")
        .y_desc("dµ/dE")
        .axis_desc_style(("sans-serif", 20))
        .draw()?;
    
    // Draw the derivative data
    chart.draw_series(LineSeries::new(
        energy.iter().zip(dmude.iter()).map(|(&x, &y)| (x, y)),
        &BLUE,
    ))?;
    
    // If e0 is set, draw a vertical line at e0
    if let Some(e0) = spectrum.e0 {
        chart.draw_series(LineSeries::new(
            vec![(e0, y_min - y_margin), (e0, y_max + y_margin)],
            &RED.mix(0.5),
        ))?;
    }
    
    // Draw a horizontal line at y=0
    chart.draw_series(LineSeries::new(
        vec![(x_min - x_margin, 0.0), (x_max + x_margin, 0.0)],
        &BLACK.mix(0.5),
    ))?;
    
    Ok(())
}

/// Plot pre-edge and post-edge fits
fn plot_preedge_fit(spectrum: &XASSpectrum, path: &str) -> PlotResult<()> {
    // Check if required data is available
    if spectrum.energy.is_none() || spectrum.mu.is_none() || spectrum.normalization.is_none() {
        return Err(PlotError::MissingData("Energy, mu, or normalization data missing for pre-edge fit plot".into()));
    }
    
    let energy = spectrum.energy.as_ref().unwrap();
    let mu = spectrum.mu.as_ref().unwrap();
    
    // Get pre-edge and post-edge line values from normalization
    let normalization = spectrum.normalization.as_ref().unwrap();
    
    // Create root area and fill with white
    let root = BitMapBackend::new(path, (800, 600)).into_drawing_area();
    root.fill(&WHITE)?;
    
    // Calculate plot ranges
    let x_min = energy.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let x_max = energy.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    
    let y_min = mu.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let y_max = mu.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    
    // Add margin to ranges
    let x_margin = (x_max - x_min) * 0.05;
    let y_margin = (y_max - y_min) * 0.05;
    
    // Build the chart
    let mut chart = ChartBuilder::on(&root)
        .caption(
            format!("Pre-edge and Post-edge Fits {}", spectrum.name.clone().unwrap_or_default()),
            ("sans-serif", 30).into_font(),
        )
        .margin(40)
        .x_label_area_size(40)
        .y_label_area_size(60)
        .build_cartesian_2d(
            (x_min - x_margin)..(x_max + x_margin),
            (y_min - y_margin)..(y_max + y_margin),
        )?;
    
    // Configure the mesh
    chart
        .configure_mesh()
        .x_desc("Energy (eV)")
        .y_desc("Absorption")
        .axis_desc_style(("sans-serif", 20))
        .draw()?;
    
    // Draw the raw data
    chart.draw_series(LineSeries::new(
        energy.iter().zip(mu.iter()).map(|(&x, &y)| (x, y)),
        &BLUE,
    ))?.label("Data");
    
    // Extract pre-edge and post-edge data
    match normalization {
        crate::xafs::normalization::NormalizationMethod::PrePostEdge(pre_post) => {
            let e0 = pre_post.e0.unwrap_or(0.0);
            
            // Get pre-edge and post-edge values
            let pre_edge = match pre_post.get_pre_edge() {
                Some(pre) => pre,
                None => {
                    // Just show data without pre/post edge lines
                    return Ok(());
                }
            };
            
            let post_edge = match pre_post.get_post_edge() {
                Some(post) => post,
                None => {
                    // Just show data without pre/post edge lines
                    return Ok(());
                }
            };
            
            // Draw pre-edge line - use a simplified approach
            // Since we have the pre_edge array directly, we can just draw it
            chart.draw_series(LineSeries::new(
                energy.iter().zip(pre_edge.iter()).map(|(&x, &y)| (x, y)),
                &RED,
            ))?.label("Pre-edge fit");
            
            // Draw post-edge line
            chart.draw_series(LineSeries::new(
                energy.iter().zip(post_edge.iter()).map(|(&x, &y)| (x, y)),
                &GREEN,
            ))?.label("Post-edge fit");
            
            // If e0 is set, draw a vertical line at e0
            if e0 > 0.0 {
                chart.draw_series(LineSeries::new(
                    vec![(e0, y_min - y_margin), (e0, y_max + y_margin)],
                    &BLACK.mix(0.5),
                ))?.label("E0");
            }
            
            // Draw markers for pre-edge range
            if let (Some(pre_start), Some(pre_end)) = (pre_post.get_pre_edge_start(), pre_post.get_pre_edge_end()) {
                let pre_start_abs = e0 + pre_start;
                let pre_end_abs = e0 + pre_end;
                
                // Find closest indices to pre_start and pre_end
                let idx_start = energy.iter()
                    .position(|&x| x >= pre_start_abs)
                    .unwrap_or(0);
                
                let idx_end = energy.iter()
                    .position(|&x| x >= pre_end_abs)
                    .unwrap_or(energy.len() - 1);
                
                // Draw markers
                if idx_start < energy.len() && idx_start < pre_edge.len() {
                    chart.draw_series(std::iter::once(Circle::new(
                        (energy[idx_start], pre_edge[idx_start]),
                        5, RED.filled()
                    )))?;
                }
                
                if idx_end < energy.len() && idx_end < pre_edge.len() {
                    chart.draw_series(std::iter::once(Circle::new(
                        (energy[idx_end], pre_edge[idx_end]),
                        5, RED.filled()
                    )))?;
                }
            }
            
            // Draw markers for post-edge range
            if let (Some(post_start), Some(post_end)) = (pre_post.get_norm_start(), pre_post.get_norm_end()) {
                let post_start_abs = e0 + post_start;
                let post_end_abs = e0 + post_end;
                
                // Find closest indices to post_start and post_end
                let idx_start = energy.iter()
                    .position(|&x| x >= post_start_abs)
                    .unwrap_or(0);
                
                let idx_end = energy.iter()
                    .position(|&x| x >= post_end_abs)
                    .unwrap_or(energy.len() - 1);
                
                // Draw markers
                if idx_start < energy.len() && idx_start < post_edge.len() {
                    chart.draw_series(std::iter::once(Circle::new(
                        (energy[idx_start], post_edge[idx_start]),
                        5, GREEN.filled()
                    )))?;
                }
                
                if idx_end < energy.len() && idx_end < post_edge.len() {
                    chart.draw_series(std::iter::once(Circle::new(
                        (energy[idx_end], post_edge[idx_end]),
                        5, GREEN.filled()
                    )))?;
                }
            }
        },
        _ => {
            // For other normalization methods, skip the pre/post edge lines
            // Just show the data
        }
    }
    
    // Draw legend
    chart.configure_series_labels()
        .background_style(&WHITE.mix(0.8))
        .border_style(&BLACK)
        .draw()?;
    
    Ok(())
}

// Implement Plottable trait for XASSpectrum for XANES plotting
impl Plottable for XASSpectrum {
    fn plot<B: DrawingBackend>(&self, backend: B) -> PlotResult<()> {
        // Create a chart area with specified dimensions
        let root = backend.into_drawing_area();
        root.fill(&WHITE)?;
        
        // Get energy and mu data
        let energy = match &self.energy {
            Some(e) => e,
            None => return Err(PlotError::MissingData("Energy data missing".into())),
        };
        
        let mu = match &self.mu {
            Some(m) => m,
            None => return Err(PlotError::MissingData("Mu data missing".into())),
        };
        
        // Calculate plot ranges
        let x_min = energy.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let x_max = energy.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        
        let y_min = mu.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let y_max = mu.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        
        // Add some margin to the ranges
        let x_margin = (x_max - x_min) * 0.05;
        let y_margin = (y_max - y_min) * 0.05;
        
        // Build the chart
        let mut chart = ChartBuilder::on(&root)
            .caption(
                self.name.clone().unwrap_or_else(|| "XAS Spectrum".to_string()),
                ("sans-serif", 30).into_font(),
            )
            .margin(40)
            .x_label_area_size(40)
            .y_label_area_size(60)
            .build_cartesian_2d(
                (x_min - x_margin)..(x_max + x_margin),
                (y_min - y_margin)..(y_max + y_margin),
            )?;
        
        // Configure the mesh
        chart
            .configure_mesh()
            .x_desc("Energy (eV)")
            .y_desc("Absorption")
            .axis_desc_style(("sans-serif", 20))
            .draw()?;
        
        // Draw the XAS data
        chart.draw_series(LineSeries::new(
            energy.iter().zip(mu.iter()).map(|(&x, &y)| (x, y)),
            &BLUE,
        ))?;
        
        // If e0 is set, draw a vertical line at e0
        if let Some(e0) = self.e0 {
            chart.draw_series(LineSeries::new(
                vec![(e0, y_min - y_margin), (e0, y_max + y_margin)],
                &RED.mix(0.5),
            ))?;
        }
        
        Ok(())
    }
    
    fn get_plot_builder(&self) -> PlotBuilder<'_, Self> {
        PlotBuilder::new(self)
    }
}

// Add specialized XANES plotting methods to XASSpectrum
impl XASSpectrum {
    /// Plot raw XANES data
    pub fn plot_raw(&self, path: &str) -> PlotResult<()> {
        plot_raw_xanes(self, path)
    }
    
    /// Plot normalized XANES data
    pub fn plot_normalized(&self, path: &str) -> PlotResult<()> {
        plot_normalized_xanes(self, path)
    }
    
    /// Plot derivative of XANES data
    pub fn plot_derivative(&self, path: &str) -> PlotResult<()> {
        plot_derivative_xanes(self, path)
    }
    
    /// Plot pre-edge and post-edge fits
    pub fn plot_preedge_fit(&self, path: &str) -> PlotResult<()> {
        plot_preedge_fit(self, path)
    }
}

// Implementation for XASGroup plotting methods
use crate::xafs::xasgroup::XASGroup;

impl XASGroup {
    /// Plot raw XANES data for all spectra in the group
    pub fn plot_raw(&self, path: &str) -> PlotResult<()> {
        // Check if there are spectra in the group
        if self.spectra.is_empty() {
            return Err(PlotError::MissingData("No spectra in the group".into()));
        }
        
        // Create root area and fill with white
        let root = BitMapBackend::new(path, (800, 600)).into_drawing_area();
        root.fill(&WHITE)?;
        
        // Find the overall min and max for energy and mu
        let mut x_min = f64::INFINITY;
        let mut x_max = f64::NEG_INFINITY;
        let mut y_min = f64::INFINITY;
        let mut y_max = f64::NEG_INFINITY;
        
        // Collect data for all valid spectra first
        let mut plot_data = Vec::new();
        let mut labels = Vec::new();
        
        for (i, spectrum) in self.spectra.iter().enumerate() {
            // Skip if no energy or mu data
            if spectrum.energy.is_none() || spectrum.mu.is_none() {
                continue;
            }
            
            let energy = spectrum.energy.as_ref().unwrap();
            let mu = spectrum.mu.as_ref().unwrap();
            
            // Update min/max values for axes scaling
            x_min = x_min.min(energy.iter().fold(f64::INFINITY, |a, &b| a.min(b)));
            x_max = x_max.max(energy.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)));
            
            y_min = y_min.min(mu.iter().fold(f64::INFINITY, |a, &b| a.min(b)));
            y_max = y_max.max(mu.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)));
            
            // Add data for this spectrum
            plot_data.push((energy.to_vec(), mu.to_vec()));
            
            // Add label for this spectrum
            let label = spectrum.name.clone().unwrap_or_else(|| format!("Spectrum {}", i + 1));
            labels.push(label);
        }
        
        // If no valid data was found
        if plot_data.is_empty() {
            return Err(PlotError::MissingData("No valid XANES data found in the group".into()));
        }
        
        // Add margin to ranges
        let x_margin = (x_max - x_min) * 0.05;
        let y_margin = (y_max - y_min) * 0.05;
        
        // Create chart
        let mut chart = ChartBuilder::on(&root)
            .caption(
                "Group XANES Spectra",
                ("sans-serif", 30).into_font(),
            )
            .margin(40)
            .x_label_area_size(40)
            .y_label_area_size(60)
            .build_cartesian_2d(
                (x_min - x_margin)..(x_max + x_margin),
                (y_min - y_margin)..(y_max + y_margin),
            )?;
        
        // Configure mesh
        chart
            .configure_mesh()
            .x_desc("Energy (eV)")
            .y_desc("Absorption")
            .axis_desc_style(("sans-serif", 20))
            .draw()?;
        
        // Get colors for each spectrum
        let colors = super::utils::generate_colors(plot_data.len());
        
        // Draw each spectrum
        for ((x, y), label, color) in plot_data.iter().zip(labels.iter()).zip(colors.iter()).map(|((a, b), c)| (a, b, c)) {
            chart.draw_series(LineSeries::new(
                x.iter().zip(y.iter()).map(|(&x, &y)| (x, y)),
                color.clone(),
            ))?.label(label);
        }
        
        // Draw legend
        chart.configure_series_labels()
            .background_style(&WHITE.mix(0.8))
            .border_style(&BLACK)
            .draw()?;
        
        Ok(())
    }
    
    /// Plot normalized XANES data for all spectra in the group
    pub fn plot_normalized(&self, path: &str) -> PlotResult<()> {
        // Check if there are spectra in the group
        if self.spectra.is_empty() {
            return Err(PlotError::MissingData("No spectra in the group".into()));
        }
        
        // Create root area and fill with white
        let root = BitMapBackend::new(path, (800, 600)).into_drawing_area();
        root.fill(&WHITE)?;
        
        // Find the overall min and max for energy and normalized mu
        let mut x_min = f64::INFINITY;
        let mut x_max = f64::NEG_INFINITY;
        let mut y_min = f64::INFINITY;
        let mut y_max = f64::NEG_INFINITY;
        
        // Collect data for all valid spectra first
        let mut plot_data = Vec::new();
        let mut labels = Vec::new();
        let mut e0_values = Vec::new();
        
        for (i, spectrum) in self.spectra.iter().enumerate() {
            // Skip if no energy or normalization data
            if spectrum.energy.is_none() || spectrum.normalization.is_none() {
                continue;
            }
            
            let energy = spectrum.energy.as_ref().unwrap();
            
            // Extract normalized data
            let normalization = spectrum.normalization.as_ref().unwrap();
            let norm_data = match normalization.get_norm() {
                Some(norm) => norm,
                None => continue, // Skip if no normalized data
            };
            
            // Update min/max values for axes scaling
            x_min = x_min.min(energy.iter().fold(f64::INFINITY, |a, &b| a.min(b)));
            x_max = x_max.max(energy.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)));
            
            y_min = y_min.min(norm_data.iter().fold(f64::INFINITY, |a, &b| a.min(b)));
            y_max = y_max.max(norm_data.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)));
            
            // Add data for this spectrum
            plot_data.push((energy.to_vec(), norm_data.to_vec()));
            
            // Track E0 if available
            if let Some(e0) = spectrum.e0 {
                e0_values.push((i, e0));
            }
            
            // Add label for this spectrum
            let label = spectrum.name.clone().unwrap_or_else(|| format!("Spectrum {}", i + 1));
            labels.push(label);
        }
        
        // If no valid data was found
        if plot_data.is_empty() {
            return Err(PlotError::MissingData("No valid normalized XANES data found in the group".into()));
        }
        
        // Add margin to ranges
        let x_margin = (x_max - x_min) * 0.05;
        let y_margin = (y_max - y_min) * 0.05;
        
        // Create chart
        let mut chart = ChartBuilder::on(&root)
            .caption(
                "Group Normalized XANES Spectra",
                ("sans-serif", 30).into_font(),
            )
            .margin(40)
            .x_label_area_size(40)
            .y_label_area_size(60)
            .build_cartesian_2d(
                (x_min - x_margin)..(x_max + x_margin),
                (y_min - y_margin)..(y_max + y_margin),
            )?;
        
        // Configure mesh
        chart
            .configure_mesh()
            .x_desc("Energy (eV)")
            .y_desc("Normalized Absorption")
            .axis_desc_style(("sans-serif", 20))
            .draw()?;
        
        // Get colors for each spectrum
        let colors = super::utils::generate_colors(plot_data.len());
        
        // Draw each spectrum
        for ((x, y), label, color) in plot_data.iter().zip(labels.iter()).zip(colors.iter()).map(|((a, b), c)| (a, b, c)) {
            chart.draw_series(LineSeries::new(
                x.iter().zip(y.iter()).map(|(&x, &y)| (x, y)),
                color.clone(),
            ))?.label(label);
        }
        
        // Draw a horizontal line at y=0 and y=1
        chart.draw_series(LineSeries::new(
            vec![(x_min - x_margin, 0.0), (x_max + x_margin, 0.0)],
            &BLACK.mix(0.3),
        ))?;
        
        chart.draw_series(LineSeries::new(
            vec![(x_min - x_margin, 1.0), (x_max + x_margin, 1.0)],
            &BLACK.mix(0.3),
        ))?;
        
        // Draw legend
        chart.configure_series_labels()
            .background_style(&WHITE.mix(0.8))
            .border_style(&BLACK)
            .draw()?;
        
        Ok(())
    }
    
    /// Plot derivative of XANES data for all spectra in the group
    pub fn plot_derivative(&self, path: &str) -> PlotResult<()> {
        // Check if there are spectra in the group
        if self.spectra.is_empty() {
            return Err(PlotError::MissingData("No spectra in the group".into()));
        }
        
        // Create root area and fill with white
        let root = BitMapBackend::new(path, (800, 600)).into_drawing_area();
        root.fill(&WHITE)?;
        
        // Collect data for all valid spectra first
        let mut plot_data = Vec::new();
        let mut labels = Vec::new();
        let mut e0_values = Vec::new();
        let mut x_min = f64::INFINITY;
        let mut x_max = f64::NEG_INFINITY;
        let mut y_min = f64::INFINITY;
        let mut y_max = f64::NEG_INFINITY;
        
        for (i, spectrum) in self.spectra.iter().enumerate() {
            // Skip if no energy or mu data
            if spectrum.energy.is_none() || spectrum.mu.is_none() {
                continue;
            }
            
            let energy = spectrum.energy.as_ref().unwrap();
            let mu = spectrum.mu.as_ref().unwrap();
            
            // Calculate derivative
            let derivative = match super::utils::derivative(energy, mu) {
                Ok(der) => der,
                Err(_) => continue, // Skip if derivative calculation fails
            };
            
            // Update min/max values for axes scaling
            x_min = x_min.min(energy.iter().fold(f64::INFINITY, |a, &b| a.min(b)));
            x_max = x_max.max(energy.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)));
            
            y_min = y_min.min(derivative.iter().fold(f64::INFINITY, |a, &b| a.min(b)));
            y_max = y_max.max(derivative.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)));
            
            // Add data for this spectrum
            plot_data.push((energy.to_vec(), derivative.to_vec()));
            
            // Track E0 if available
            if let Some(e0) = spectrum.e0 {
                e0_values.push((i, e0));
            }
            
            // Add label for this spectrum
            let label = spectrum.name.clone().unwrap_or_else(|| format!("Spectrum {}", i + 1));
            labels.push(label);
        }
        
        // If no valid data was found
        if plot_data.is_empty() {
            return Err(PlotError::MissingData("No valid XANES data found for derivative calculation".into()));
        }
        
        // Add margin to ranges
        let x_margin = (x_max - x_min) * 0.05;
        let y_margin = (y_max - y_min) * 0.05;
        
        // Create chart
        let mut chart = ChartBuilder::on(&root)
            .caption(
                "Group XANES Derivatives",
                ("sans-serif", 30).into_font(),
            )
            .margin(40)
            .x_label_area_size(40)
            .y_label_area_size(60)
            .build_cartesian_2d(
                (x_min - x_margin)..(x_max + x_margin),
                (y_min - y_margin)..(y_max + y_margin),
            )?;
        
        // Configure mesh
        chart
            .configure_mesh()
            .x_desc("Energy (eV)")
            .y_desc("dµ/dE")
            .axis_desc_style(("sans-serif", 20))
            .draw()?;
        
        // Get colors for each spectrum
        let colors = super::utils::generate_colors(plot_data.len());
        
        // Draw each spectrum
        for (((x, y), label), color) in plot_data.iter().zip(labels.iter()).zip(colors.iter()) {
            chart.draw_series(LineSeries::new(
                x.iter().zip(y.iter()).map(|(&x, &y)| (x, y)),
                color.clone(),
            ))?.label(label);
        }
        
        // Draw a horizontal line at y=0
        chart.draw_series(LineSeries::new(
            vec![(x_min - x_margin, 0.0), (x_max + x_margin, 0.0)],
            &BLACK.mix(0.3),
        ))?;
        
        // Draw legend
        chart.configure_series_labels()
            .background_style(&WHITE.mix(0.8))
            .border_style(&BLACK)
            .draw()?;
        
        Ok(())
    }
}