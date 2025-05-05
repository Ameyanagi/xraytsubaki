//! Traits for plotting functionality
//!
//! This module defines the traits that are used to implement plotting
//! capabilities for XAS data types.

use plotters::prelude::*;
use super::{PlotResult, PlotError, builders::PlotBuilder};

/// Trait for objects that can be plotted
pub trait Plottable: Sized {
    /// Plot to a specific backend
    fn plot<B: DrawingBackend>(&self, backend: B) -> PlotResult<()>;
    
    /// Plot to a file at the specified path
    fn plot_to_file(&self, path: &str) -> PlotResult<()> {
        // Determine the file extension
        let path_str = path.to_lowercase();
        
        // Create the appropriate backend based on file extension
        if path_str.ends_with(".svg") {
            let backend = SVGBackend::new(path, (800, 600));
            self.plot(backend)
        } else if path_str.ends_with(".png") {
            let backend = BitMapBackend::new(path, (800, 600));
            self.plot(backend)
        } else {
            // Default to PNG if extension not recognized
            let path_with_png = format!("{}.png", path);
            let backend = BitMapBackend::new(&path_with_png, (800, 600));
            self.plot(backend)
        }
    }
    
    /// Get a plot builder for customizing plots
    fn get_plot_builder(&self) -> PlotBuilder<'_, Self>;
}

/// Trait for types that can provide plot data
pub trait PlotData {
    /// Get the x-axis data for plotting
    fn get_x_data(&self) -> PlotResult<Vec<f64>>;
    
    /// Get the y-axis data for plotting
    fn get_y_data(&self) -> PlotResult<Vec<f64>>;
    
    /// Get the labels for the data (optional)
    fn get_labels(&self) -> Option<Vec<String>> {
        None
    }
    
    /// Get the suggested x-axis range for plotting
    fn get_x_range(&self) -> PlotResult<(f64, f64)> {
        let x_data = self.get_x_data()?;
        if x_data.is_empty() {
            return Err(PlotError::MissingData("No x data available for plotting".into()));
        }
        
        let min_x = x_data.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_x = x_data.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        
        Ok((min_x, max_x))
    }
    
    /// Get the suggested y-axis range for plotting
    fn get_y_range(&self) -> PlotResult<(f64, f64)> {
        let y_data = self.get_y_data()?;
        if y_data.is_empty() {
            return Err(PlotError::MissingData("No y data available for plotting".into()));
        }
        
        let min_y = y_data.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_y = y_data.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        
        // Add some margin to the range
        let range = max_y - min_y;
        let margin = range * 0.05;
        
        Ok((min_y - margin, max_y + margin))
    }
    
    /// Get the suggested title for the plot
    fn get_plot_title(&self) -> String {
        "XAS Data".to_string()
    }
    
    /// Get the suggested x-axis label for the plot
    fn get_x_label(&self) -> String {
        "X".to_string()
    }
    
    /// Get the suggested y-axis label for the plot
    fn get_y_label(&self) -> String {
        "Y".to_string()
    }
}