pub use crate::xafs::xasgroup::XASGroup;
pub use crate::xafs::xasspectrum::XASSpectrum;

pub use crate::xafs::background::{BackgroundMethod, AUTOBK};
pub use crate::xafs::io;
pub use crate::xafs::lmutils::LMParameters;
pub use crate::xafs::mathutils;
pub use crate::xafs::normalization::{Normalization, NormalizationMethod};
pub use crate::xafs::nshare::{ToNalgebra, ToNdarray1};
pub use crate::xafs::xafsutils::{FTWindow, XAFSUtils};
pub use crate::xafs::xrayfft::{FFTUtils, XrayFFTF, XrayFFTR};

// Plot functionality
pub use crate::plot::{Plottable, PlotResult, PlotError, PlotBuilder};
pub use crate::plot::xanes::{XANESPlotType, plot_xanes};
pub use crate::plot::exafs::{EXAFSPlotType, plot_exafs_k, plot_exafs_r};
pub use crate::plot::fitting::{plot_fit, plot_parameter_correlation, plot_fit_components};
