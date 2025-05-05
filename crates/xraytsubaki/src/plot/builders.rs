//! Plot builder pattern implementation
//!
//! This module provides a builder pattern for configuring and creating plots.

use plotters::prelude::*;
use super::{PlotResult, PlotError, traits::Plottable};

/// Builder for configuring plots
pub struct PlotBuilder<'a, T: Plottable> {
    /// The object to be plotted
    plottable: &'a T,
    /// Plot title
    title: Option<String>,
    /// X-axis label
    x_label: Option<String>,
    /// Y-axis label
    y_label: Option<String>,
    /// X-axis range
    x_range: Option<(f64, f64)>,
    /// Y-axis range
    y_range: Option<(f64, f64)>,
    /// Plot width in pixels
    width: usize,
    /// Plot height in pixels
    height: usize,
    /// Whether to show a legend
    show_legend: bool,
    /// Plot margin (top, right, bottom, left)
    margin: (u32, u32, u32, u32),
    /// Plot background color
    background_color: RGBColor,
    /// Plot mesh color
    mesh_color: RGBColor,
    /// Whether to show x-axis tick marks
    show_x_ticks: bool,
    /// Whether to show y-axis tick marks
    show_y_ticks: bool,
    /// Custom line colors
    line_colors: Vec<RGBColor>,
    /// Line width in pixels
    line_width: u32,
}

impl<'a, T: Plottable> PlotBuilder<'a, T> {
    /// Create a new plot builder for the given plottable object
    pub fn new(plottable: &'a T) -> Self {
        Self {
            plottable,
            title: None,
            x_label: None,
            y_label: None,
            x_range: None,
            y_range: None,
            width: 800,
            height: 600,
            show_legend: true,
            margin: (40, 40, 40, 60),
            background_color: WHITE,
            mesh_color: RGBColor(0, 0, 0),
            show_x_ticks: true,
            show_y_ticks: true,
            line_colors: vec![],
            line_width: 2,
        }
    }
    
    /// Set the plot title
    pub fn title(mut self, title: &str) -> Self {
        self.title = Some(title.to_string());
        self
    }
    
    /// Set the x-axis label
    pub fn x_label(mut self, label: &str) -> Self {
        self.x_label = Some(label.to_string());
        self
    }
    
    /// Set the y-axis label
    pub fn y_label(mut self, label: &str) -> Self {
        self.y_label = Some(label.to_string());
        self
    }
    
    /// Set the x-axis range
    pub fn x_range(mut self, min: f64, max: f64) -> Self {
        self.x_range = Some((min, max));
        self
    }
    
    /// Set the y-axis range
    pub fn y_range(mut self, min: f64, max: f64) -> Self {
        self.y_range = Some((min, max));
        self
    }
    
    /// Set the plot dimensions
    pub fn dimensions(mut self, width: usize, height: usize) -> Self {
        self.width = width;
        self.height = height;
        self
    }
    
    /// Set whether to show a legend
    pub fn show_legend(mut self, show: bool) -> Self {
        self.show_legend = show;
        self
    }
    
    /// Set the plot margin
    pub fn margin(mut self, top: u32, right: u32, bottom: u32, left: u32) -> Self {
        self.margin = (top, right, bottom, left);
        self
    }
    
    /// Set the background color
    pub fn background_color(mut self, color: RGBColor) -> Self {
        self.background_color = color;
        self
    }
    
    /// Set the mesh color
    pub fn mesh_color(mut self, color: RGBColor) -> Self {
        self.mesh_color = color;
        self
    }
    
    /// Set whether to show x-axis tick marks
    pub fn show_x_ticks(mut self, show: bool) -> Self {
        self.show_x_ticks = show;
        self
    }
    
    /// Set whether to show y-axis tick marks
    pub fn show_y_ticks(mut self, show: bool) -> Self {
        self.show_y_ticks = show;
        self
    }
    
    /// Set custom line colors
    pub fn line_colors(mut self, colors: Vec<RGBColor>) -> Self {
        self.line_colors = colors;
        self
    }
    
    /// Set the line width
    pub fn line_width(mut self, width: u32) -> Self {
        self.line_width = width;
        self
    }
    
    /// Build the plot using the specified backend
    pub fn build<B: DrawingBackend>(self, backend: B) -> PlotResult<()> {
        // Delegate to the plottable object
        self.plottable.plot(backend)
    }
    
    /// Save the plot to a file
    pub fn save(self, path: &str) -> PlotResult<()> {
        // Determine the file extension
        let path_str = path.to_lowercase();
        
        // Create the appropriate backend based on file extension
        if path_str.ends_with(".svg") {
            let backend = SVGBackend::new(path, (self.width as u32, self.height as u32));
            self.build(backend)
        } else if path_str.ends_with(".png") {
            let backend = BitMapBackend::new(path, (self.width as u32, self.height as u32));
            self.build(backend)
        } else {
            // Default to PNG if extension not recognized
            let path_with_png = format!("{}.png", path);
            let backend = BitMapBackend::new(&path_with_png, (self.width as u32, self.height as u32));
            self.build(backend)
        }
    }
}