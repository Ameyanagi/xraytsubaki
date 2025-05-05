//! EXAFS plotting functionality
//!
//! This module provides specialized plotting functions for EXAFS data in k-space and R-space.

use plotters::prelude::*;
use crate::xafs::xasspectrum::XASSpectrum;
use crate::xafs::xasgroup::XASGroup;
use super::{PlotResult, PlotError, traits::Plottable, builders::PlotBuilder};

/// Types of EXAFS plots
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EXAFSPlotType {
    /// Chi(k) vs. k
    KSpace,
    /// k^n * Chi(k) vs. k
    KWeighted,
    /// |Chi(R)| vs. R
    RMagnitude,
    /// Re[Chi(R)] and Im[Chi(R)] vs. R
    RComponents,
    /// Chi(q) vs. q
    QSpace,
}

/// Plot EXAFS k-space data
pub fn plot_exafs_k(
    spectrum: &XASSpectrum,
    path: &str,
    k_weight: Option<f64>,
) -> PlotResult<()> {
    // Check if required data is available
    if spectrum.background.is_none() {
        return Err(PlotError::MissingData("Background data missing for EXAFS k-space plot".into()));
    }
    
    let k = match spectrum.background.as_ref().unwrap().get_k() {
        Some(k) => k,
        None => return Err(PlotError::MissingData("k data missing for EXAFS k-space plot".into())),
    };
    
    let chi = match spectrum.background.as_ref().unwrap().get_chi() {
        Some(chi) => chi,
        None => return Err(PlotError::MissingData("chi data missing for EXAFS k-space plot".into())),
    };
    
    // Determine k-weighting
    let k_weight = k_weight.unwrap_or(0.0);
    
    // Apply k-weighting if requested
    let y_data = if k_weight > 0.0 {
        let weighted_chi = chi.iter().zip(k.iter()).map(|(&c, &k)| c * k.powf(k_weight)).collect::<Vec<_>>();
        weighted_chi
    } else {
        chi.to_vec()
    };
    
    // Create root area and fill with white
    let root = BitMapBackend::new(path, (800, 600)).into_drawing_area();
    root.fill(&WHITE)?;
    
    // Calculate ranges
    let x_min = k.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let x_max = k.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    
    let y_min = y_data.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let y_max = y_data.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    
    // Add margin to ranges
    let x_margin = (x_max - x_min) * 0.05;
    let y_margin = (y_max - y_min) * 0.05;
    
    // Create chart
    let mut chart = ChartBuilder::on(&root)
        .caption(
            format!(
                "{}{}",
                if k_weight > 0.0 {
                    format!("k^{} * ", k_weight)
                } else {
                    "".to_string()
                },
                "EXAFS χ(k)"
            ),
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
        .x_desc("k (Å⁻¹)")
        .y_desc(
            if k_weight > 0.0 {
                format!("k^{} * χ(k) (Å⁻{})", k_weight, k_weight as usize)
            } else {
                "χ(k)".to_string()
            }
        )
        .axis_desc_style(("sans-serif", 20))
        .draw()?;
    
    // Draw the data
    chart.draw_series(LineSeries::new(
        k.iter().zip(y_data.iter()).map(|(&x, &y)| (x, y)),
        &BLUE,
    ))?;
    
    Ok(())
}

/// Plot EXAFS R-space data
pub fn plot_exafs_r(
    spectrum: &XASSpectrum,
    path: &str,
    plot_type: EXAFSPlotType,
) -> PlotResult<()> {
    // Check if required data is available
    if spectrum.xftf.is_none() {
        return Err(PlotError::MissingData("XFTF data missing for EXAFS R-space plot".into()));
    }
    
    let r = match spectrum.xftf.as_ref().unwrap().get_r() {
        Some(r) => r,
        None => return Err(PlotError::MissingData("R data missing for EXAFS R-space plot".into())),
    };
    
    // Create root area and fill with white
    let root = BitMapBackend::new(path, (800, 600)).into_drawing_area();
    root.fill(&WHITE)?;
    
    // Draw different plot types
    match plot_type {
        EXAFSPlotType::RMagnitude => {
            let mag = match spectrum.xftf.as_ref().unwrap().get_chir_mag() {
                Some(mag) => mag,
                None => return Err(PlotError::MissingData("Chi(R) magnitude data missing".into())),
            };
            
            // Calculate ranges
            let x_min = r.iter().fold(f64::INFINITY, |a, &b| a.min(b));
            let x_max = r.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
            
            let y_min = 0.0; // Magnitude is usually plotted from zero
            let y_max = mag.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
            
            // Add margin to ranges
            let x_margin = (x_max - x_min) * 0.05;
            let y_margin = y_max * 0.05;
            
            // Create chart
            let mut chart = ChartBuilder::on(&root)
                .caption("EXAFS |χ(R)|", ("sans-serif", 30).into_font())
                .margin(40)
                .x_label_area_size(40)
                .y_label_area_size(60)
                .build_cartesian_2d(
                    (x_min - x_margin)..(x_max + x_margin),
                    (y_min)..(y_max + y_margin),
                )?;
            
            // Configure mesh
            chart
                .configure_mesh()
                .x_desc("R (Å)")
                .y_desc("|χ(R)| (Å⁻³)")
                .axis_desc_style(("sans-serif", 20))
                .draw()?;
            
            // Draw the data
            chart.draw_series(LineSeries::new(
                r.iter().zip(mag.iter()).map(|(&x, &y)| (x, y)),
                &BLUE,
            ))?;
        },
        EXAFSPlotType::RComponents => {
            let re = match spectrum.xftf.as_ref().unwrap().get_chir_real() {
                Some(re) => re,
                None => return Err(PlotError::MissingData("Chi(R) real component data missing".into())),
            };
            
            let im = match spectrum.xftf.as_ref().unwrap().get_chir_imag() {
                Some(im) => im,
                None => return Err(PlotError::MissingData("Chi(R) imaginary component data missing".into())),
            };
            
            // Calculate ranges
            let x_min = r.iter().fold(f64::INFINITY, |a, &b| a.min(b));
            let x_max = r.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
            
            let y_min = re.iter().chain(im.iter()).fold(f64::INFINITY, |a, &b| a.min(b));
            let y_max = re.iter().chain(im.iter()).fold(f64::NEG_INFINITY, |a, &b| a.max(b));
            
            // Add margin to ranges
            let x_margin = (x_max - x_min) * 0.05;
            let y_margin = (y_max - y_min) * 0.05;
            
            // Create chart
            let mut chart = ChartBuilder::on(&root)
                .caption("EXAFS χ(R) Components", ("sans-serif", 30).into_font())
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
                .x_desc("R (Å)")
                .y_desc("χ(R) (Å⁻³)")
                .axis_desc_style(("sans-serif", 20))
                .draw()?;
            
            // Draw the real component
            chart.draw_series(LineSeries::new(
                r.iter().zip(re.iter()).map(|(&x, &y)| (x, y)),
                &BLUE,
            ))?.label("Re[χ(R)]");
            
            // Draw the imaginary component
            chart.draw_series(LineSeries::new(
                r.iter().zip(im.iter()).map(|(&x, &y)| (x, y)),
                &RED,
            ))?.label("Im[χ(R)]");
            
            // Draw legend
            chart.configure_series_labels()
                .background_style(&WHITE.mix(0.8))
                .border_style(&BLACK)
                .draw()?;
        },
        _ => return Err(PlotError::Parameters(format!("Unsupported plot type: {:?}", plot_type))),
    }
    
    Ok(())
}

// Add specialized EXAFS plotting methods to XASSpectrum
impl XASSpectrum {
    /// Plot EXAFS k-space data
    pub fn plot_k_space(&self, path: &str, k_weight: Option<f64>) -> PlotResult<()> {
        plot_exafs_k(self, path, k_weight)
    }
    
    /// Plot EXAFS R-space magnitude
    pub fn plot_r_mag(&self, path: &str) -> PlotResult<()> {
        plot_exafs_r(self, path, EXAFSPlotType::RMagnitude)
    }
    
    /// Plot EXAFS R-space components
    pub fn plot_r_components(&self, path: &str) -> PlotResult<()> {
        plot_exafs_r(self, path, EXAFSPlotType::RComponents)
    }
}

// Add specialized EXAFS plotting methods to XASGroup
impl XASGroup {
    /// Plot k-space data for all spectra in the group
    pub fn plot_k_space(&self, path: &str, k_weight: Option<f64>) -> PlotResult<()> {
        // Check if there are spectra in the group
        if self.spectra.is_empty() {
            return Err(PlotError::MissingData("No spectra in the group".into()));
        }
        
        // Create root area and fill with white
        let root = BitMapBackend::new(path, (800, 600)).into_drawing_area();
        root.fill(&WHITE)?;
        
        // Determine k-weighting (default to 2 if not specified)
        let k_weight = k_weight.unwrap_or(2.0);
        
        // Find the overall min and max for k and chi
        let mut x_min = f64::INFINITY;
        let mut x_max = f64::NEG_INFINITY;
        let mut y_min = f64::INFINITY;
        let mut y_max = f64::NEG_INFINITY;
        
        // Collect data for all valid spectra first
        let mut plot_data = Vec::new();
        let mut labels = Vec::new();
        
        for (i, spectrum) in self.spectra.iter().enumerate() {
            // Skip if no background data
            if spectrum.background.is_none() {
                continue;
            }
            
            let k = match spectrum.background.as_ref().unwrap().get_k() {
                Some(k) => k,
                None => continue, // Skip if no k data
            };
            
            let chi = match spectrum.background.as_ref().unwrap().get_chi() {
                Some(chi) => chi,
                None => continue, // Skip if no chi data
            };
            
            // Apply k-weighting
            let weighted_chi = chi.iter().zip(k.iter())
                .map(|(&c, &k)| c * k.powf(k_weight))
                .collect::<Vec<_>>();
            
            // Update min/max values for axes scaling
            x_min = x_min.min(k.iter().fold(f64::INFINITY, |a, &b| a.min(b)));
            x_max = x_max.max(k.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)));
            
            y_min = y_min.min(weighted_chi.iter().fold(f64::INFINITY, |a, &b| a.min(b)));
            y_max = y_max.max(weighted_chi.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)));
            
            // Add data for this spectrum
            plot_data.push((k.to_vec(), weighted_chi));
            
            // Add label for this spectrum
            let label = spectrum.name.clone().unwrap_or_else(|| format!("Spectrum {}", i + 1));
            labels.push(label);
        }
        
        // If no valid data was found
        if plot_data.is_empty() {
            return Err(PlotError::MissingData("No valid EXAFS data found in the group".into()));
        }
        
        // Add margin to ranges
        let x_margin = (x_max - x_min) * 0.05;
        let y_margin = (y_max - y_min) * 0.05;
        
        // Create chart
        let mut chart = ChartBuilder::on(&root)
            .caption(
                format!(
                    "Group EXAFS k^{} * χ(k)",
                    k_weight
                ),
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
            .x_desc("k (Å⁻¹)")
            .y_desc(
                if k_weight > 0.0 {
                    format!("k^{} * χ(k) (Å⁻{})", k_weight, k_weight as usize)
                } else {
                    "χ(k)".to_string()
                }
            )
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
    
    /// Plot R-space magnitude for all spectra in the group
    pub fn plot_r_mag(&self, path: &str) -> PlotResult<()> {
        // Check if there are spectra in the group
        if self.spectra.is_empty() {
            return Err(PlotError::MissingData("No spectra in the group".into()));
        }
        
        // Create root area and fill with white
        let root = BitMapBackend::new(path, (800, 600)).into_drawing_area();
        root.fill(&WHITE)?;
        
        // Find the overall min and max for r and chi_r_mag
        let mut x_min = f64::INFINITY;
        let mut x_max = f64::NEG_INFINITY;
        let mut y_max = f64::NEG_INFINITY;
        
        // Collect data for all valid spectra first
        let mut plot_data = Vec::new();
        let mut labels = Vec::new();
        
        for (i, spectrum) in self.spectra.iter().enumerate() {
            // Skip if no XFTF data
            if spectrum.xftf.is_none() {
                continue;
            }
            
            let r = match spectrum.xftf.as_ref().unwrap().get_r() {
                Some(r) => r,
                None => continue, // Skip if no r data
            };
            
            let mag = match spectrum.xftf.as_ref().unwrap().get_chir_mag() {
                Some(mag) => mag,
                None => continue, // Skip if no magnitude data
            };
            
            // Update min/max values for axes scaling
            x_min = x_min.min(r.iter().fold(f64::INFINITY, |a, &b| a.min(b)));
            x_max = x_max.max(r.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)));
            
            y_max = y_max.max(mag.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)));
            
            // Add data for this spectrum
            plot_data.push((r.to_vec(), mag.to_vec()));
            
            // Add label for this spectrum
            let label = spectrum.name.clone().unwrap_or_else(|| format!("Spectrum {}", i + 1));
            labels.push(label);
        }
        
        // If no valid data was found
        if plot_data.is_empty() {
            return Err(PlotError::MissingData("No valid R-space data found in the group".into()));
        }
        
        // Add margin to ranges
        let x_margin = (x_max - x_min) * 0.05;
        let y_margin = y_max * 0.05;
        
        // Create chart
        let mut chart = ChartBuilder::on(&root)
            .caption(
                "Group EXAFS |χ(R)|",
                ("sans-serif", 30).into_font(),
            )
            .margin(40)
            .x_label_area_size(40)
            .y_label_area_size(60)
            .build_cartesian_2d(
                (x_min - x_margin)..(x_max + x_margin),
                (0.0)..(y_max + y_margin),
            )?;
        
        // Configure mesh
        chart
            .configure_mesh()
            .x_desc("R (Å)")
            .y_desc("|χ(R)| (Å⁻³)")
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
    
    /// Plot R-space components for all spectra in the group
    pub fn plot_r_components(&self, path: &str) -> PlotResult<()> {
        // Check if there are spectra in the group
        if self.spectra.is_empty() {
            return Err(PlotError::MissingData("No spectra in the group".into()));
        }
        
        // Create root area and fill with white
        let root = BitMapBackend::new(path, (800, 600)).into_drawing_area();
        root.fill(&WHITE)?;
        
        // Find the overall min and max for r and chi_r components
        let mut x_min = f64::INFINITY;
        let mut x_max = f64::NEG_INFINITY;
        let mut y_min = f64::INFINITY;
        let mut y_max = f64::NEG_INFINITY;
        
        // Collect data for all valid spectra first
        let mut plot_data_re = Vec::new();
        let mut plot_data_im = Vec::new();
        let mut labels = Vec::new();
        
        for (i, spectrum) in self.spectra.iter().enumerate() {
            // Skip if no XFTF data
            if spectrum.xftf.is_none() {
                continue;
            }
            
            let r = match spectrum.xftf.as_ref().unwrap().get_r() {
                Some(r) => r,
                None => continue, // Skip if no r data
            };
            
            let re = match spectrum.xftf.as_ref().unwrap().get_chir_real() {
                Some(re) => re,
                None => continue, // Skip if no real component
            };
            
            let im = match spectrum.xftf.as_ref().unwrap().get_chir_imag() {
                Some(im) => im,
                None => continue, // Skip if no imaginary component
            };
            
            // Update min/max values for axes scaling
            x_min = x_min.min(r.iter().fold(f64::INFINITY, |a, &b| a.min(b)));
            x_max = x_max.max(r.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)));
            
            let re_min = re.iter().fold(f64::INFINITY, |a, &b| a.min(b));
            let re_max = re.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
            
            let im_min = im.iter().fold(f64::INFINITY, |a, &b| a.min(b));
            let im_max = im.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
            
            y_min = y_min.min(re_min).min(im_min);
            y_max = y_max.max(re_max).max(im_max);
            
            // Add data for this spectrum
            plot_data_re.push((r.to_vec(), re.to_vec()));
            plot_data_im.push((r.to_vec(), im.to_vec()));
            
            // Add label for this spectrum
            let label = spectrum.name.clone().unwrap_or_else(|| format!("Spectrum {}", i + 1));
            labels.push(label);
        }
        
        // If no valid data was found
        if plot_data_re.is_empty() {
            return Err(PlotError::MissingData("No valid R-space data found in the group".into()));
        }
        
        // Add margin to ranges
        let x_margin = (x_max - x_min) * 0.05;
        let y_margin = (y_max - y_min) * 0.05;
        
        // Create chart
        let mut chart = ChartBuilder::on(&root)
            .caption(
                "Group EXAFS χ(R) Components",
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
            .x_desc("R (Å)")
            .y_desc("χ(R) (Å⁻³)")
            .axis_desc_style(("sans-serif", 20))
            .draw()?;
        
        // Get colors for each spectrum
        let colors = super::utils::generate_colors(plot_data_re.len());
        
        // Draw each spectrum
        for (_i, (re_data, im_data, label, color)) in 
            plot_data_re.iter()
                .zip(plot_data_im.iter())
                .zip(labels.iter())
                .zip(colors.iter())
                .map(|(((a, b), c), d)| (a, b, c, d))
                .enumerate() {
                    
            let (x_re, y_re) = re_data;
            let (_, y_im) = im_data;
            
            // Draw real part with solid line
            chart.draw_series(LineSeries::new(
                x_re.iter().zip(y_re.iter()).map(|(&x, &y)| (x, y)),
                color.clone(),
            ))?.label(format!("{} (Re)", label));
            
            // Draw imaginary part with different color
            let im_color = RGBColor(
                color.0.saturating_sub(30), 
                color.1.saturating_sub(30), 
                color.2.saturating_sub(30)
            );
            chart.draw_series(LineSeries::new(
                x_re.iter().zip(y_im.iter()).map(|(&x, &y)| (x, y)),
                im_color,
            ))?.label(format!("{} (Im)", label));
        }
        
        // Draw legend
        chart.configure_series_labels()
            .background_style(&WHITE.mix(0.8))
            .border_style(&BLACK)
            .draw()?;
        
        Ok(())
    }
}