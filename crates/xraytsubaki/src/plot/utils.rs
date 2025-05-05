//! Utility functions for plotting
//!
//! This module provides utility functions for plotting XAS data.

use ndarray::{ArrayBase, Ix1, OwnedRepr};
use plotters::style::RGBColor;
use super::PlotResult;

/// Generate a series of distinct colors for plotting multiple datasets
pub fn generate_colors(n: usize) -> Vec<RGBColor> {
    let colors = vec![
        RGBColor(0, 0, 255),     // Blue
        RGBColor(255, 0, 0),     // Red
        RGBColor(0, 128, 0),     // Green
        RGBColor(128, 0, 128),   // Purple
        RGBColor(255, 165, 0),   // Orange
        RGBColor(0, 128, 128),   // Teal
        RGBColor(128, 0, 0),     // Maroon
        RGBColor(0, 0, 128),     // Navy
        RGBColor(128, 128, 0),   // Olive
        RGBColor(255, 0, 255),   // Magenta
    ];
    
    if n <= colors.len() {
        colors[0..n].to_vec()
    } else {
        // If more colors are needed, cycle through the available colors
        (0..n).map(|i| colors[i % colors.len()]).collect()
    }
}

/// Calculate the derivative of a dataset
pub fn derivative(
    x: &ArrayBase<OwnedRepr<f64>, Ix1>,
    y: &ArrayBase<OwnedRepr<f64>, Ix1>,
) -> PlotResult<ArrayBase<OwnedRepr<f64>, Ix1>> {
    if x.len() != y.len() {
        return Err(super::PlotError::Parameters("x and y arrays must have the same length for derivative calculation".into()));
    }
    
    if x.len() < 3 {
        return Err(super::PlotError::Parameters("Need at least 3 points for derivative calculation".into()));
    }
    
    let mut result = ArrayBase::<OwnedRepr<f64>, Ix1>::zeros(x.len());
    
    // Calculate derivative using central difference
    for i in 1..x.len() - 1 {
        let dx = x[i + 1] - x[i - 1];
        let dy = y[i + 1] - y[i - 1];
        result[i] = dy / dx;
    }
    
    // Use forward difference for the first point
    result[0] = (y[1] - y[0]) / (x[1] - x[0]);
    
    // Use backward difference for the last point
    let last = x.len() - 1;
    result[last] = (y[last] - y[last - 1]) / (x[last] - x[last - 1]);
    
    Ok(result)
}

/// Format axis tick labels
pub fn format_tick_labels(value: f64, precision: Option<usize>) -> String {
    let precision = precision.unwrap_or(1);
    
    // Format the value based on its magnitude
    if value.abs() < 0.01 {
        format!("{:.1e}", value)
    } else {
        format!("{:.*}", precision, value)
    }
}

/// Calculate appropriate tick intervals for an axis
pub fn calculate_tick_interval(min: f64, max: f64) -> f64 {
    let range = max - min;
    
    // Aim for approximately 5-10 tick marks
    let rough_interval = range / 8.0;
    
    // Find the order of magnitude of the rough interval
    let order = rough_interval.abs().log10().floor();
    let magnitude = 10.0_f64.powf(order);
    
    // Try intervals of 1, 2, and 5 times the magnitude
    let intervals = [1.0 * magnitude, 2.0 * magnitude, 5.0 * magnitude, 10.0 * magnitude];
    
    // Find the interval that gives closest to the desired number of ticks
    intervals
        .iter()
        .min_by(|&&a, &&b| {
            let a_ticks = (range / a).round();
            let b_ticks = (range / b).round();
            let a_diff = (a_ticks - 8.0).abs();
            let b_diff = (b_ticks - 8.0).abs();
            a_diff.partial_cmp(&b_diff).unwrap()
        })
        .cloned()
        .unwrap_or(magnitude)
}