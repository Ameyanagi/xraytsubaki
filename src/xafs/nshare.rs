use nalgebra::{DMatrix, DVector, Dyn};
use ndarray::{Array1, ArrayBase, Ix1, Ix2, OwnedRepr};

/// Trait for converting from ndarray to nalgebra
///
/// It is specific to f64 and 1D or 2D arrays.
/// For more general conversions, you should consider using nshare crate.
pub trait ToNalgebra {
    type Out;
    fn into_nalgebra(self) -> Self::Out;
}

impl ToNalgebra for ArrayBase<OwnedRepr<f64>, Ix1> {
    type Out = DVector<f64>;
    fn into_nalgebra(self) -> DVector<f64> {
        DVector::from_vec(self.to_vec())
    }
}

impl ToNalgebra for ArrayBase<OwnedRepr<f64>, Ix2> {
    type Out = DMatrix<f64>;
    fn into_nalgebra(self) -> Self::Out {
        let nrows = Dyn(self.nrows());
        let ncols = Dyn(self.ncols());
        DMatrix::from_vec_generic(ncols, nrows, self.into_raw_vec()).transpose()
    }
}

/// Trait for converting from nalgebra to ndarray
///
/// It is specific to f64 and 1D arrays.
/// For more general conversions, you should consider using nshare crate.
pub trait ToNdarray1 {
    type Out;
    fn into_ndarray1(self) -> Self::Out;
}

impl ToNdarray1 for DVector<f64> {
    type Out = ArrayBase<OwnedRepr<f64>, Ix1>;
    fn into_ndarray1(self) -> Self::Out {
        Array1::from_vec(self.data.as_vec().clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::{Matrix3x2, Vector3};
    use ndarray::{Array1, Array2};

    #[test]
    fn test_to_nalgebra() {
        let a = Array1::from_vec(vec![1.0, 2.0, 3.0]);
        let b = Array2::from(vec![[1.0, 2.0], [3.0, 4.0], [5.0, 6.0]]);

        let a_ref = DVector::from_vec(vec![1.0, 2.0, 3.0]);
        let b_ref = Matrix3x2::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
        let c = a.clone().into_nalgebra();
        let d = b.clone().into_nalgebra();

        assert_eq!(c, a_ref);
        assert_eq!(d, b_ref);

        let a_rev = c.into_ndarray1();

        assert_eq!(a, a_rev);
    }
}
