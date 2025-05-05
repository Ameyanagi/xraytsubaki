//! Plotting module for XRayTsubaki
//!
//! This module provides plotting capabilities for XAS data using the Plotters library.
//! It includes traits and implementations for various types of XAS plots, including
//! XANES and EXAFS visualizations.

pub mod traits;
pub mod builders;
pub mod xanes;
pub mod exafs;
pub mod fitting;
pub mod utils;

#[cfg(test)]
pub mod tests;

// Re-export common types and functions
pub use traits::Plottable;
pub use builders::PlotBuilder;
pub use xanes::{XANESPlotType, plot_xanes};
pub use exafs::{EXAFSPlotType, plot_exafs_k, plot_exafs_r};
pub use fitting::plot_fit;

// Common error type for plotting operations
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlotError {
    #[error("Failed to create plot: {0}")]
    Creation(String),
    #[error("Failed to draw plot: {0}")]
    Drawing(String),
    #[error("Failed to save plot: {0}")]
    Saving(String),
    #[error("Invalid plot parameters: {0}")]
    Parameters(String),
    #[error("Missing data for plot: {0}")]
    MissingData(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Other error: {0}")]
    Other(String),
}

// Implement From for Plotters error types
impl<E: std::fmt::Debug + std::error::Error + Send + Sync> From<plotters::drawing::DrawingAreaErrorKind<E>> for PlotError {
    fn from(err: plotters::drawing::DrawingAreaErrorKind<E>) -> Self {
        PlotError::Drawing(format!("{:?}", err))
    }
}

/// Result type for plotting operations
pub type PlotResult<T> = Result<T, PlotError>;