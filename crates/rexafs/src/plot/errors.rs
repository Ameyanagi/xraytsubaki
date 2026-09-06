use crate::xafs::XAFSError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlotError {
    #[error("missing data: {field}")]
    MissingData { field: &'static str },

    #[error("index out of range: index {index}, len {len}")]
    IndexOutOfRange { index: usize, len: usize },

    #[error("invalid plotting option: {reason}")]
    InvalidOption { reason: String },

    #[error("spectrum compute failed at index {index}: {source}")]
    SpectrumCompute {
        index: usize,
        #[source]
        source: XAFSError,
    },

    #[error("plot backend error: {0}")]
    Ruviz(#[from] ruviz::core::PlottingError),

    #[error("single-plot output requested for multi-panel selection")]
    MultiPanelRenderUnsupported,

    #[error("no data selected for plotting")]
    EmptySelection,

    #[error("xas computation failed: {0}")]
    Xafs(#[from] XAFSError),
}

impl PlotError {
    pub fn invalid_option(reason: impl Into<String>) -> Self {
        Self::InvalidOption {
            reason: reason.into(),
        }
    }
}
