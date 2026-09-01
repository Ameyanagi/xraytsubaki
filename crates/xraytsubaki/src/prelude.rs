pub use crate::xafs::xasgroup::XASGroup;
pub use crate::xafs::xasspectrum::XASSpectrum;

pub use crate::xafs::background::{AUTOBKClampScalePolicy, AUTOBKSolver, BackgroundMethod, AUTOBK};
pub use crate::xafs::fitting::{
    feffit_independent, feffit_joint, feffit_joint_with_options, feffpath, ff2chi,
    parse_feff_path_file, path2chi, resolve_feff_commands, run_feff, run_feff_and_load_paths,
    DatasetResult, FeffBatchExecutionStrategy, FeffBatchOptions, FeffDat, FeffExecutionMode,
    FeffFit, FeffFitDataset, FeffFitJacobianMode, FeffFitOptions, FeffFitResult,
    FeffFitSolverMethod, FeffFitTransform, FeffFlavor, FeffModuleCommand, FeffPathModel,
    FeffResolvedCommands, FeffRunRequest, FeffRunResult, FitSpace, FitVariable, FitVariables,
    FitWarning, KweightResult, NoiseEstimate, Param, PathContribution, PathParamSpec,
};
pub use crate::xafs::io;
pub use crate::xafs::io::athena::{AthenaGroup, AthenaParams, AthenaProject, AthenaValue};
pub use crate::xafs::lmutils::LMParameters;
// pub use crate::xafs::mathutils;
pub use crate::xafs::normalization::{Normalization, NormalizationMethod};
#[cfg(feature = "ndarray-compat")]
pub use crate::xafs::nshare::{ToNalgebra, ToNdarray1};
pub use crate::xafs::xafsutils::{FTWindow, XAFSUtils};
pub use crate::xafs::xrayfft::{FFTUtils, XrayFFTF, XrayFFTR};

#[cfg(feature = "plotting")]
pub use crate::plot::{PlotError, PlotXAS, XASPlotBuilder};
pub use crate::xafs::analysis::{
    lcf, lcf_combinatorial, pca_train, AnalysisSpace, LcfComponent, LcfConfig, LcfResult, LcfSpace,
    PcaConfig, PcaFit, PcaModel,
};
pub use crate::xafs::errors::AnalysisError;
pub use crate::xafs::tools::{
    difference, merge_spectra, DiffSpace, EdgeFeature, MergeConfig, MergeGrid, MergeWeight,
    RebinConfig, RebinMethod, RebinOutput,
};
pub use crate::xafs::xafsutils::ConvolveForm;
