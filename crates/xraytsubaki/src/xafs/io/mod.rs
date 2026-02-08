#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

pub mod xafs_bson;
pub mod xafs_json;
pub mod xasdatatype;

use crate::xafs::errors::IOError;
use crate::xafs::xasspectrum::XASSpectrum;
use data_reader::reader::{load_txt_f64, Delimiter, ReaderParams};
use std::error::Error;
use std::path::Path;

#[allow(non_snake_case)]
pub fn load_spectrum_QAS_trans<P: AsRef<Path>>(path: P) -> Result<XASSpectrum, IOError> {
    let path_ref = path.as_ref();
    let path_string = path_ref.to_string_lossy().to_string();
    let params = ReaderParams {
        comments: Some(b'#'),
        delimiter: Delimiter::WhiteSpace,
        ..Default::default()
    };

    let data = load_txt_f64(&path_string, &params).map_err(|_e| IOError::ReadFailed {
        path: path_ref.display().to_string(),
        kind: std::io::ErrorKind::Other,
    })?;
    let energy = data.get_col(0);
    let i0 = data.get_col(1);
    let it = data.get_col(2);
    let ir = data.get_col(3);
    let iff = data.get_col(4);

    let mut xafs_group = XASSpectrum::new();
    xafs_group.set_spectrum(
        energy,
        i0.iter()
            .zip(it)
            .map(|(i0, it)| (i0 / it).ln())
            .collect::<Vec<_>>(),
    );

    Ok(xafs_group)
}

mod tests {
    use super::*;

    const TOP_DIR: &str = env!("CARGO_MANIFEST_DIR");

    #[test]
    fn test_load_spectrum() {
        let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let result = load_spectrum_QAS_trans(path).unwrap();
        println!("{:?}", result);
    }
}
