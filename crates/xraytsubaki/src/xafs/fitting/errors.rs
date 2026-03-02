use thiserror::Error;

use super::types::{FeffExecutionMode, FeffFlavor};

#[derive(Error, Debug, Clone)]
pub enum FittingError {
    #[error("unsupported FEFF flavor for this build: {flavor:?}")]
    UnsupportedFeffFlavor { flavor: FeffFlavor },

    #[error("failed to parse FEFF path file '{path}': {reason}")]
    ParseFailed { path: String, reason: String },

    #[error("invalid FEFF path data: {reason}")]
    InvalidFeffData { reason: String },

    #[error("expression evaluation failed for '{expr}': {reason}")]
    ExpressionFailed { expr: String, reason: String },

    #[error("undefined symbol in expression: {symbol}")]
    UndefinedSymbol { symbol: String },

    #[error("cyclic variable expression dependency detected at: {symbol}")]
    CyclicExpression { symbol: String },

    #[error("invalid fit transform configuration: {reason}")]
    InvalidTransform { reason: String },

    #[error("invalid fit dataset: {reason}")]
    InvalidDataset { reason: String },

    #[error("fitting requires at least one active FEFF path")]
    EmptyPaths,

    #[error("fitting requires at least one varying variable")]
    NoVaryingVariables,

    #[error("nonlinear solver failed: {reason}")]
    SolverFailed { reason: String },

    #[error("invalid FEFF executable path '{path}': {reason}")]
    InvalidExecutablePath { path: String, reason: String },

    #[error("FEFF workspace path does not exist or is not a directory: '{path}'")]
    WorkspaceNotFound { path: String },

    #[error("FEFF input file was not found: '{path}'")]
    FeffInputNotFound { path: String },

    #[error("required FEFF module executable could not be resolved: {module}")]
    ExecutableNotFound { module: String },

    #[error("requested FEFF execution mode is not available in this build: {mode:?} ({reason})")]
    UnsupportedExecutionMode {
        mode: FeffExecutionMode,
        reason: String,
    },

    #[error("failed to spawn FEFF module '{module}' using '{executable}': {reason}")]
    ProcessSpawnFailed {
        module: String,
        executable: String,
        reason: String,
    },

    #[error("FEFF module '{module}' exited with non-zero status {code}")]
    ProcessFailed { module: String, code: i32 },

    #[error("FEFF module '{module}' timed out after {timeout_sec}s")]
    ProcessTimedOut { module: String, timeout_sec: u64 },

    #[error("failed to read FEFF module output for '{module}': {reason}")]
    OutputReadFailed { module: String, reason: String },

    #[error("FEFF10 pipeline failed: {reason}")]
    Feff10PipelineFailed { reason: String },

    #[error("FEFF execution produced no path output files (feffNNNN.dat) in '{workspace}'")]
    NoPathOutputs { workspace: String },

    #[error("I/O failure during {action} for '{path}': {reason}")]
    IOFailed {
        action: String,
        path: String,
        reason: String,
    },
}
