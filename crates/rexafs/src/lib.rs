//! Rust-powered X-ray absorption analysis.
//!
//! Start with [`process`] for the default EXAFS pipeline, or [`Spectrum`] for
//! individual stages. Advanced APIs live in [`fitting`], [`structure`] and [`io`].
//! Developed under the codename xraytsubaki.

pub mod parser;
#[cfg(feature = "plotting")]
pub mod plot;
pub mod prelude;
pub mod xafs;

mod processing;
pub use processing::{process, process_with_options, ProcessOptions, ProcessedSpectrum};
pub use xafs::xasgroup::XASGroup as Group;
pub use xafs::xasspectrum::XASSpectrum as Spectrum;
pub use xafs::{analysis, fitting, io, structure, tools, Result, XAFSError as Error};
