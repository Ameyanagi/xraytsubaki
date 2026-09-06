//! Crystal lattice: cell parameters ↔ 3×3 matrix, fractional ↔ Cartesian.

use serde::{Deserialize, Serialize};

use super::StructureError;

/// Lattice with row vectors `a`, `b`, `c` in Å (crystallographic setting:
/// `a` along x, `b` in the xy plane).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lattice {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    /// Degrees.
    pub alpha: f64,
    pub beta: f64,
    pub gamma: f64,
    /// Row vectors: `matrix[0]` = a, `matrix[1]` = b, `matrix[2]` = c.
    pub matrix: [[f64; 3]; 3],
    inverse: [[f64; 3]; 3],
}

impl Lattice {
    /// Build from cell parameters (lengths in Å, angles in degrees).
    pub fn from_parameters(
        a: f64,
        b: f64,
        c: f64,
        alpha: f64,
        beta: f64,
        gamma: f64,
    ) -> Result<Self, StructureError> {
        for (name, v) in [("a", a), ("b", b), ("c", c)] {
            if !(v.is_finite() && v > 0.0) {
                return Err(StructureError::InvalidLattice {
                    reason: format!("{name} = {v}"),
                });
            }
        }
        for (name, v) in [("alpha", alpha), ("beta", beta), ("gamma", gamma)] {
            if !(v.is_finite() && v > 0.0 && v < 180.0) {
                return Err(StructureError::InvalidLattice {
                    reason: format!("{name} = {v}°"),
                });
            }
        }
        let (ca, cb, cg) = (
            alpha.to_radians().cos(),
            beta.to_radians().cos(),
            gamma.to_radians().cos(),
        );
        let sg = gamma.to_radians().sin();
        let cy = (ca - cb * cg) / sg;
        let cz2 = 1.0 - cb * cb - cy * cy;
        if cz2 <= 0.0 {
            return Err(StructureError::InvalidLattice {
                reason: format!("angles {alpha}/{beta}/{gamma} do not form a cell"),
            });
        }
        let matrix = [
            [a, 0.0, 0.0],
            [b * cg, b * sg, 0.0],
            [c * cb, c * cy, c * cz2.sqrt()],
        ];
        let inverse = invert3(&matrix).ok_or_else(|| StructureError::InvalidLattice {
            reason: "singular cell matrix".into(),
        })?;
        Ok(Self {
            a,
            b,
            c,
            alpha,
            beta,
            gamma,
            matrix,
            inverse,
        })
    }

    /// Build from row vectors.
    pub fn from_matrix(matrix: [[f64; 3]; 3]) -> Result<Self, StructureError> {
        let len = |v: &[f64; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        let angle = |u: &[f64; 3], v: &[f64; 3]| {
            let d = u[0] * v[0] + u[1] * v[1] + u[2] * v[2];
            (d / (len(u) * len(v))).clamp(-1.0, 1.0).acos().to_degrees()
        };
        let (a, b, c) = (len(&matrix[0]), len(&matrix[1]), len(&matrix[2]));
        let inverse = invert3(&matrix).ok_or_else(|| StructureError::InvalidLattice {
            reason: "singular cell matrix".into(),
        })?;
        Ok(Self {
            a,
            b,
            c,
            alpha: angle(&matrix[1], &matrix[2]),
            beta: angle(&matrix[0], &matrix[2]),
            gamma: angle(&matrix[0], &matrix[1]),
            matrix,
            inverse,
        })
    }

    pub fn cubic(a: f64) -> Result<Self, StructureError> {
        Self::from_parameters(a, a, a, 90.0, 90.0, 90.0)
    }

    pub fn volume(&self) -> f64 {
        det3(&self.matrix).abs()
    }

    /// Fractional → Cartesian (Å).
    pub fn to_cart(&self, frac: [f64; 3]) -> [f64; 3] {
        let m = &self.matrix;
        [
            frac[0] * m[0][0] + frac[1] * m[1][0] + frac[2] * m[2][0],
            frac[0] * m[0][1] + frac[1] * m[1][1] + frac[2] * m[2][1],
            frac[0] * m[0][2] + frac[1] * m[1][2] + frac[2] * m[2][2],
        ]
    }

    /// Cartesian (Å) → fractional.
    pub fn to_frac(&self, cart: [f64; 3]) -> [f64; 3] {
        let m = &self.inverse;
        [
            cart[0] * m[0][0] + cart[1] * m[1][0] + cart[2] * m[2][0],
            cart[0] * m[0][1] + cart[1] * m[1][1] + cart[2] * m[2][1],
            cart[0] * m[0][2] + cart[1] * m[1][2] + cart[2] * m[2][2],
        ]
    }

    /// Spacing between lattice planes perpendicular to reciprocal axis `i`
    /// (= volume / |a_j × a_k|); a sphere of radius R needs
    /// `ceil(R / spacing)` images along that axis.
    pub fn interplanar_spacing(&self, i: usize) -> f64 {
        let j = (i + 1) % 3;
        let k = (i + 2) % 3;
        let cross = cross3(&self.matrix[j], &self.matrix[k]);
        let n = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
        if n <= 0.0 {
            return f64::INFINITY;
        }
        self.volume() / n
    }

    /// Cartesian distance between two fractional positions (no wrapping).
    pub fn distance(&self, f1: [f64; 3], f2: [f64; 3]) -> f64 {
        let c1 = self.to_cart(f1);
        let c2 = self.to_cart(f2);
        ((c1[0] - c2[0]).powi(2) + (c1[1] - c2[1]).powi(2) + (c1[2] - c2[2]).powi(2)).sqrt()
    }
}

pub(super) fn det3(m: &[[f64; 3]; 3]) -> f64 {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}

pub(super) fn cross3(u: &[f64; 3], v: &[f64; 3]) -> [f64; 3] {
    [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ]
}

pub(super) fn invert3(m: &[[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    let d = det3(m);
    if !d.is_finite() || d.abs() < 1e-12 {
        return None;
    }
    let inv = [
        [
            (m[1][1] * m[2][2] - m[1][2] * m[2][1]) / d,
            (m[0][2] * m[2][1] - m[0][1] * m[2][2]) / d,
            (m[0][1] * m[1][2] - m[0][2] * m[1][1]) / d,
        ],
        [
            (m[1][2] * m[2][0] - m[1][0] * m[2][2]) / d,
            (m[0][0] * m[2][2] - m[0][2] * m[2][0]) / d,
            (m[0][2] * m[1][0] - m[0][0] * m[1][2]) / d,
        ],
        [
            (m[1][0] * m[2][1] - m[1][1] * m[2][0]) / d,
            (m[0][1] * m[2][0] - m[0][0] * m[2][1]) / d,
            (m[0][0] * m[1][1] - m[0][1] * m[1][0]) / d,
        ],
    ];
    Some(inv)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hexagonal_cell_round_trips() {
        let lat = Lattice::from_parameters(2.706, 2.706, 4.282, 90.0, 90.0, 120.0).unwrap();
        assert!((lat.volume() - 27.15394).abs() < 1e-4);
        let frac = [1.0 / 3.0, 2.0 / 3.0, 0.25];
        let cart = lat.to_cart(frac);
        let back = lat.to_frac(cart);
        for i in 0..3 {
            assert!((back[i] - frac[i]).abs() < 1e-12);
        }
        let again = Lattice::from_matrix(lat.matrix).unwrap();
        assert!((again.gamma - 120.0).abs() < 1e-9);
        assert!((lat.interplanar_spacing(2) - 4.282).abs() < 1e-9);
    }

    #[test]
    fn rejects_bad_cells() {
        assert!(Lattice::from_parameters(0.0, 1.0, 1.0, 90.0, 90.0, 90.0).is_err());
        assert!(Lattice::from_parameters(1.0, 1.0, 1.0, 90.0, 90.0, 180.0).is_err());
    }
}
