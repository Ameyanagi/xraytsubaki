//! EXAFS fitting visualization functionality
//!
//! This module provides plotting functions for visualizing EXAFS fitting results.

use plotters::prelude::*;
use crate::xafs::fitting::{FitResult, FittingDataset};
use super::{PlotResult, PlotError};

/// Plot fitting results
pub fn plot_fit(
    dataset: &FittingDataset,
    fit_result: &FitResult,
    path: &str,
) -> PlotResult<()> {
    // Check if dataset has required data
    let x_data = match dataset.get_k() {
        Some(k) => k,
        None => return Err(PlotError::MissingData("k data missing from dataset".into())),
    };
    
    let y_data = match dataset.get_data() {
        Some(data) => data,
        None => return Err(PlotError::MissingData("Data missing from dataset".into())),
    };
    
    // Check if fit result has best fit data
    let y_fit = match fit_result.get_best_fit() {
        Some(fit) => fit,
        None => return Err(PlotError::MissingData("Best fit data missing from fit result".into())),
    };
    
    // Create the residuals
    let residuals: Vec<f64> = y_data.iter()
        .zip(y_fit.iter())
        .map(|(&data, &fit)| data - fit)
        .collect();
    
    // Calculate plot ranges
    let x_min = x_data.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let x_max = x_data.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    
    let y_min = y_data.iter()
        .chain(y_fit.iter())
        .fold(f64::INFINITY, |a, &b| a.min(b));
    let y_max = y_data.iter()
        .chain(y_fit.iter())
        .fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    
    // Add margin to ranges
    let x_margin = (x_max - x_min) * 0.05;
    let y_margin = (y_max - y_min) * 0.05;
    
    // Calculate range for residuals
    let r_min = residuals.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let r_max = residuals.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    let r_margin = (r_max - r_min) * 0.1;
    
    // Create root area and fill with white
    let root = BitMapBackend::new(path, (800, 800)).into_drawing_area();
    root.fill(&WHITE)?;
    
    // Split the drawing area for data and residuals
    let (upper, lower) = root.split_vertically(600);
    
    // Create chart for data and fit
    let mut chart = ChartBuilder::on(&upper)
        .caption("EXAFS Fit", ("sans-serif", 30).into_font())
        .margin(40)
        .x_label_area_size(0) // No x-axis label for upper plot
        .y_label_area_size(60)
        .build_cartesian_2d(
            (x_min - x_margin)..(x_max + x_margin),
            (y_min - y_margin)..(y_max + y_margin),
        )?;
    
    // Configure mesh for data and fit
    chart
        .configure_mesh()
        .y_desc("χ(k)")
        .axis_desc_style(("sans-serif", 20))
        .draw()?;
    
    // Draw the data
    chart.draw_series(LineSeries::new(
        x_data.iter().zip(y_data.iter()).map(|(&x, &y)| (x, y)),
        &BLUE,
    ))?.label("Data");
    
    // Draw the fit
    chart.draw_series(LineSeries::new(
        x_data.iter().zip(y_fit.iter()).map(|(&x, &y)| (x, y)),
        &RED,
    ))?.label("Fit");
    
    // Draw legend
    chart.configure_series_labels()
        .background_style(&WHITE.mix(0.8))
        .border_style(&BLACK)
        .draw()?;
    
    // Create chart for residuals
    let mut residual_chart = ChartBuilder::on(&lower)
        .margin(40)
        .x_label_area_size(40)
        .y_label_area_size(60)
        .build_cartesian_2d(
            (x_min - x_margin)..(x_max + x_margin),
            (r_min - r_margin)..(r_max + r_margin),
        )?;
    
    // Configure mesh for residuals
    residual_chart
        .configure_mesh()
        .x_desc("k (Å⁻¹)")
        .y_desc("Residuals")
        .axis_desc_style(("sans-serif", 20))
        .draw()?;
    
    // Draw the residuals
    residual_chart.draw_series(LineSeries::new(
        x_data.iter().zip(residuals.iter()).map(|(&x, &y)| (x, y)),
        &GREEN,
    ))?;
    
    // Draw a zero line for reference
    residual_chart.draw_series(LineSeries::new(
        vec![(x_min - x_margin, 0.0), (x_max + x_margin, 0.0)],
        &BLACK.mix(0.5),
    ))?;
    
    Ok(())
}

/// Plot parameter correlation matrix
pub fn plot_parameter_correlation(
    fit_result: &FitResult,
    path: &str,
) -> PlotResult<()> {
    // Check if fit result has correlation data
    let correlation = match fit_result.get_correlation_matrix() {
        Some(corr) => corr,
        None => return Err(PlotError::MissingData("Correlation matrix missing from fit result".into())),
    };
    
    // Get parameter names
    let param_names = match fit_result.get_param_names() {
        Some(names) => names.clone(),
        None => {
            // If no names are available, use generic names
            let n = correlation.shape().0;
            (0..n).map(|i| format!("p{}", i)).collect::<Vec<_>>()
        },
    };
    
    // Determine the number of parameters
    let n_params = param_names.len();
    
    // Create root area and fill with white
    // Use a square canvas based on number of parameters
    let size = 600.max(100 + 50 * n_params) as u32;
    let root = BitMapBackend::new(path, (size, size)).into_drawing_area();
    root.fill(&WHITE)?;
    
    // Define correlation color scale
    // We'll use a blue-white-red color scale for negative/positive correlations
    let get_color = |val: f64| -> RGBColor {
        if val >= 0.0 {
            // White to red (positive correlation)
            let intensity = (val * 255.0) as u8;
            RGBColor(255, 255 - intensity, 255 - intensity)
        } else {
            // White to blue (negative correlation)
            let intensity = (-val * 255.0) as u8;
            RGBColor(255 - intensity, 255 - intensity, 255)
        }
    };
    
    // Create the chart
    let mut chart = ChartBuilder::on(&root)
        .caption(
            "Parameter Correlation Matrix", 
            ("sans-serif", 25).into_font()
        )
        .margin(40)
        .x_label_area_size(60)
        .y_label_area_size(60)
        .build_cartesian_2d(0f64..n_params as f64, 0f64..n_params as f64)?;
    
    // Configure mesh
    chart
        .configure_mesh()
        .disable_mesh()
        .x_labels(n_params)
        .y_labels(n_params)
        .x_label_formatter(&|x| {
            let idx = (*x).floor() as usize;
            if idx < param_names.len() {
                param_names[idx].clone()
            } else {
                String::new()
            }
        })
        .y_label_formatter(&|y| {
            let idx = (*y).floor() as usize;
            if idx < param_names.len() {
                param_names[idx].clone()
            } else {
                String::new()
            }
        })
        .x_label_style(("sans-serif", 12).into_font().transform(FontTransform::Rotate90))
        .y_label_style(("sans-serif", 12))
        .draw()?;
    
    // Draw correlation cells
    for i in 0..n_params {
        for j in 0..n_params {
            let val = correlation[(i, j)];
            
            // Draw a rectangle for each cell
            chart.draw_series(std::iter::once(Rectangle::new(
                [(j as f64, i as f64), ((j+1) as f64, (i+1) as f64)],
                get_color(val).filled(),
            )))?;
            
            // Draw correlation value in the cell
            if i != j {  // Skip diagonal (always 1.0)
                chart.draw_series(std::iter::once(Text::new(
                    format!("{:.2}", val),
                    ((j as f64) + 0.5, (i as f64) + 0.5),
                    ("sans-serif", 10).into_font(),
                )))?;
            }
        }
    }
    
    // Add a color scale legend
    let legend_root = root.split_vertically(size as i32 - 40).0;
    let legend_area = legend_root.margin(0, 0, 0, 80);
    
    // Draw the color scale for -1 to 1
    let mut legend_chart = ChartBuilder::on(&legend_area)
        .margin(5)
        .caption("Correlation", ("sans-serif", 15))
        .x_label_area_size(20)
        .y_label_area_size(0)
        .build_cartesian_2d(-1.0..1.0, 0..1)?;
    
    // Draw rectangles for the color scale
    let steps = 40;
    for i in 0..steps {
        let val = -1.0 + (2.0 * i as f64) / steps as f64;
        let next_val = -1.0 + (2.0 * (i + 1) as f64) / steps as f64;
        
        legend_chart.draw_series(std::iter::once(Rectangle::new(
            [(val, 0), (next_val, 1)],
            get_color(val).filled(),
        )))?;
    }
    
    // Configure the legend axis
    legend_chart.configure_mesh()
        .disable_mesh()
        .x_labels(5)
        .x_label_formatter(&|x| format!("{:.1}", x))
        .draw()?;
    
    Ok(())
}

/// Plot fit components (individual paths)
pub fn plot_fit_components(
    dataset: &FittingDataset,
    fit_result: &FitResult,
    path: &str,
) -> PlotResult<()> {
    // Check if dataset has required data
    let x_data = match dataset.get_k() {
        Some(k) => k,
        None => return Err(PlotError::MissingData("k data missing from dataset".into())),
    };
    
    let y_data = match dataset.get_data() {
        Some(data) => data,
        None => return Err(PlotError::MissingData("Data missing from dataset".into())),
    };
    
    // Check if fit result has component data
    let components = match fit_result.get_components() {
        Some(comp) => comp,
        None => return Err(PlotError::MissingData("Component data missing from fit result".into())),
    };
    
    // Check if fit result has best fit data
    let y_fit = match fit_result.get_best_fit() {
        Some(fit) => fit,
        None => return Err(PlotError::MissingData("Best fit data missing from fit result".into())),
    };
    
    // Calculate plot ranges
    let x_min = x_data.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let x_max = x_data.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    
    // Find y range including all components
    let mut y_min = y_data.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let mut y_max = y_data.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    
    // Check each component for min/max values
    for component in components.iter() {
        y_min = y_min.min(component.iter().fold(f64::INFINITY, |a, &b| a.min(b)));
        y_max = y_max.max(component.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)));
    }
    
    // Add margin to ranges
    let x_margin = (x_max - x_min) * 0.05;
    let y_margin = (y_max - y_min) * 0.05;
    
    // Create root area and fill with white
    let root = BitMapBackend::new(path, (800, 600)).into_drawing_area();
    root.fill(&WHITE)?;
    
    // Create chart
    let mut chart = ChartBuilder::on(&root)
        .caption("EXAFS Fit Components", ("sans-serif", 30).into_font())
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
        .y_desc("χ(k)")
        .axis_desc_style(("sans-serif", 20))
        .draw()?;
    
    // Draw the data
    chart.draw_series(LineSeries::new(
        x_data.iter().zip(y_data.iter()).map(|(&x, &y)| (x, y)),
        &BLUE,
    ))?.label("Data");
    
    // Draw the fit
    chart.draw_series(LineSeries::new(
        x_data.iter().zip(y_fit.iter()).map(|(&x, &y)| (x, y)),
        &RED,
    ))?.label("Fit");
    
    // Draw each component with a different color
    let component_colors = super::utils::generate_colors(components.len() + 2)[2..].to_vec();
    
    for (i, component) in components.iter().enumerate() {
        let color = if i < component_colors.len() {
            component_colors[i] // Use color directly
        } else {
            // Fallback color if we run out of colors
            RGBColor(100, 100, 100)
        };
        
        chart.draw_series(LineSeries::new(
            x_data.iter().zip(component.iter()).map(|(&x, &y)| (x, y)),
            color,
        ))?.label(format!("Component {}", i + 1));
    }
    
    // Draw legend
    chart.configure_series_labels()
        .background_style(&WHITE.mix(0.8))
        .border_style(&BLACK)
        .position(SeriesLabelPosition::UpperRight)
        .draw()?;
    
    Ok(())
}