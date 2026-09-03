//! Scattering-path geometry from FEFF path files, mapped onto a cluster so
//! a visualiser can draw each path through the atoms it scatters from.

use serde::{Deserialize, Serialize};

use super::cluster::Cluster;
use super::element::Element;
use super::StructureError;
use crate::xafs::fitting::types::{FeffDat, PathAtom};

/// One leg endpoint of a scattering path (Cartesian, absorber at origin).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathLeg {
    pub cart: [f64; 3],
    pub ipot: u16,
    pub z: u8,
    pub symbol: String,
    /// Nearest [`Cluster`] atom index, when mapped.
    pub cluster_atom: Option<usize>,
}

/// Geometry of one path: the absorber, each scattering atom in order, and
/// (implicitly) the return to the absorber.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathGeometry {
    pub reff: f64,
    pub degen: f64,
    pub nleg: usize,
    /// `legs[0]` is the absorber at the origin; `legs[1..]` the scatterers.
    pub legs: Vec<PathLeg>,
}

impl PathGeometry {
    /// Extract from a parsed path file. Files that carry only labels (older
    /// parses) yield an error.
    pub fn from_feffdat(dat: &FeffDat) -> Result<Self, StructureError> {
        if dat.geometry_atoms.is_empty() {
            return Err(StructureError::PathGeometry {
                reason: format!("{} has no leg coordinates", dat.filename),
            });
        }
        let legs = dat
            .geometry_atoms
            .iter()
            .map(|a: &PathAtom| PathLeg {
                cart: [a.x, a.y, a.z],
                ipot: a.ipot,
                z: a.atomic_number,
                symbol: Element::from_z(a.atomic_number)
                    .map(|e| e.symbol.to_string())
                    .unwrap_or_else(|| a.label.clone()),
                cluster_atom: None,
            })
            .collect();
        Ok(Self {
            reff: dat.reff,
            degen: dat.degen,
            nleg: dat.nleg,
            legs,
        })
    }

    /// Closed polyline through the legs (absorber → scatterers → absorber).
    pub fn polyline(&self) -> Vec<[f64; 3]> {
        let mut pts: Vec<[f64; 3]> = self.legs.iter().map(|l| l.cart).collect();
        if let Some(first) = pts.first().copied() {
            pts.push(first);
        }
        pts
    }

    /// Half path length from the leg coordinates (should equal `reff`).
    pub fn half_length(&self) -> f64 {
        let pts = self.polyline();
        let mut total = 0.0;
        for w in pts.windows(2) {
            total += ((w[1][0] - w[0][0]).powi(2)
                + (w[1][1] - w[0][1]).powi(2)
                + (w[1][2] - w[0][2]).powi(2))
            .sqrt();
        }
        total / 2.0
    }

    /// Map every leg onto the nearest cluster atom within `tol` Å (the FEFF
    /// output keeps the input coordinates to ~1e-4 Å). Unmatched legs keep
    /// `cluster_atom = None`.
    pub fn map_to_cluster(&mut self, cluster: &Cluster, tol: f64) -> usize {
        let mut matched = 0;
        for leg in &mut self.legs {
            leg.cluster_atom = cluster
                .nearest(leg.cart)
                .filter(|(_, d)| *d <= tol)
                .map(|(i, _)| i);
            if leg.cluster_atom.is_some() {
                matched += 1;
            }
        }
        matched
    }

    /// Cluster atom indices touched by this path (deduplicated, absorber
    /// excluded).
    pub fn scatterers(&self) -> Vec<usize> {
        let mut out = Vec::new();
        for leg in self.legs.iter().skip(1) {
            if let Some(i) = leg.cluster_atom {
                if i != 0 && !out.contains(&i) {
                    out.push(i);
                }
            }
        }
        out
    }
}

/// Parse the leg coordinate lines of a FEFF path file body: each line is
/// `x y z ipot z label…`.
pub(crate) fn parse_geometry_line(line: &str) -> Option<PathAtom> {
    let words: Vec<&str> = line.split_whitespace().collect();
    if words.len() < 5 {
        return None;
    }
    let x = words[0].parse::<f64>().ok()?;
    let y = words[1].parse::<f64>().ok()?;
    let z = words[2].parse::<f64>().ok()?;
    let ipot = words[3].parse::<u16>().ok()?;
    let znum = words[4].parse::<u16>().ok()?;
    let label = words.get(5).map(|s| s.to_string()).unwrap_or_else(|| {
        Element::from_z(znum as u8)
            .map(|e| e.symbol.to_string())
            .unwrap_or_default()
    });
    Some(PathAtom {
        x,
        y,
        z,
        ipot,
        atomic_number: znum.min(255) as u8,
        label,
    })
}
