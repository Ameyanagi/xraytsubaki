//! Crystal structures for EXAFS modelling: CIF import, space-group
//! expansion, cluster generation around an absorber, `feff.inp` export,
//! structure databases (local CIF library, AMCSD, Materials Project) and
//! FEFF path geometry for visualisation.
//!
//! Pipeline: `Structure` (from a CIF, a database, or built by hand) →
//! [`cluster::build_cluster`] → [`feffinp::write_feff_inp`] → the existing
//! FEFF runners in [`crate::xafs::fitting::runner`] → path files whose leg
//! geometry [`paths::PathGeometry`] maps back onto the cluster atoms.

pub mod builtin;
pub mod cif;
pub mod cluster;
pub mod db;
pub mod element;
mod element_table;
pub mod feffinp;
pub mod lattice;
pub mod model;
pub mod pathrank;
pub mod paths;
pub mod symmetry;
pub mod xyz;

pub use cif::{parse_cif, read_cif, structure_from_cif, structure_to_cif, CifBlock, CifLoop};
pub use cluster::{
    absorber_sites, build_cluster, AbsorberSelection, Cluster, ClusterAtom, ClusterOptions,
    OccupancyPolicy, Potential, Shell,
};
pub use db::{LocalCifLibrary, StructureHit, StructureQuery, StructureSource};
pub use element::Element;
pub use feffinp::{write_feff_inp, Edge, FeffInputOptions, FeffInputStyle};
pub use lattice::Lattice;
pub use model::{Site, SpaceGroupInfo, Species, Structure};
pub use builtin::{BuiltinEntry, BuiltinLibrary};
pub use pathrank::{path_label, rank_paths, select_by, select_default, shells_of, PathInfo, ShellInfo};
pub use paths::{PathGeometry, PathLeg};
pub use xyz::{parse_xyz, read_xyz, Xyz, XyzAbsorber, XyzAtom};
pub use symmetry::{expand_sites, find_space_group, SpaceGroupEntry, SymOp};

use thiserror::Error;

/// Errors raised by the structure module.
#[derive(Debug, Error)]
pub enum StructureError {
    #[error("CIF parse error at line {line}: {message}")]
    CifParse { line: usize, message: String },
    #[error("CIF has no crystal structure data: {reason}")]
    CifNoStructure { reason: String },
    #[error("unknown element or site label '{label}'")]
    UnknownElement { label: String },
    #[error("unknown space group ({reason})")]
    UnknownSpaceGroup { reason: String },
    #[error("invalid symmetry operation '{op}': {reason}")]
    InvalidSymOp { op: String, reason: String },
    #[error("invalid lattice: {reason}")]
    InvalidLattice { reason: String },
    #[error("absorber not found: {reason}")]
    AbsorberNotFound { reason: String },
    #[error("invalid cluster request: {reason}")]
    InvalidCluster { reason: String },
    #[error("structure database error: {reason}")]
    Database { reason: String },
    #[error("network error: {reason}")]
    Network { reason: String },
    #[error("I/O error for {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("path geometry error: {reason}")]
    PathGeometry { reason: String },
}

impl From<serde_json::Error> for StructureError {
    fn from(err: serde_json::Error) -> Self {
        StructureError::Database {
            reason: format!("JSON: {err}"),
        }
    }
}
