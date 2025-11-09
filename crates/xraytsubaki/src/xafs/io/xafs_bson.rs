use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::mem;

use bson::bson;
use bson::Bson;
use bson::Document;

use serde::{Deserialize, Serialize};
use version::version;

use crate::xafs::errors::IOError;
use crate::xafs::io::xasdatatype::{XASDataType, XASGroupFile};
use crate::xafs::xasgroup::XASGroup;
use crate::xafs::xasspectrum::XASSpectrum;

pub trait XASBson {
    fn read_bson(&mut self, filename: &str) -> Result<&mut Self, IOError>;

    fn write_bson(&mut self, filename: &str) -> Result<&mut Self, IOError>;
}

impl XASBson for XASGroupFile {
    fn read_bson(&mut self, filename: &str) -> Result<&mut Self, IOError> {
        let mut f_buffer = File::open(filename).map_err(|e| IOError::ReadFailed {
            path: filename.to_string(),
            source: e.kind(),
        })?;

        let doc = Document::from_reader(&mut f_buffer).map_err(|e| IOError::BsonError {
            message: e.to_string(),
        })?;

        let deserialized = bson::from_document(doc).map_err(|e| IOError::BsonError {
            message: e.to_string(),
        })?;

        _ = mem::replace(self, deserialized);

        Ok(self)
    }

    fn write_bson(&mut self, filename: &str) -> Result<&mut Self, IOError> {
        self.version = version!().to_string();
        self.datatype = XASDataType::XASGroup;

        let data_bson = bson::to_bson(&self).map_err(|e| IOError::BsonError {
            message: e.to_string(),
        })?;

        let mut data_file = File::create(filename).map_err(|e| IOError::ReadFailed {
            path: filename.to_string(),
            source: e.kind(),
        })?;

        data_bson
            .as_document()
            .unwrap_or(&Document::new())
            .to_writer(&mut data_file)
            .map_err(|e| IOError::BsonError {
                message: e.to_string(),
            })?;

        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use ndarray::Array1;

    use super::*;
    use crate::prelude::*;
    use approx::{assert_abs_diff_eq, assert_relative_eq};

    use data_reader::reader::{load_txt_f64, Delimiter, ReaderParams};

    use crate::xafs::tests::PARAM_LOADTXT;
    use crate::xafs::tests::TEST_TOL;
    use crate::xafs::tests::TOP_DIR;

    const ACCEPTABLE_MU_DIFF: f64 = 1e-6;
    const CHI_MSE_TOL: f64 = 1e-2;
    const CHI_Q_TOL: f64 = 1e-1;

    #[test]
    #[allow(non_snake_case)]
    fn test_xas_bson_write() -> Result<(), Box<dyn std::error::Error>> {
        let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let save_path = String::from(TOP_DIR) + "/tests/testfiles/test.bson";
        let mut xafs_test_group = io::load_spectrum_QAS_trans(&path).unwrap();

        xafs_test_group.set_background_method(Some(BackgroundMethod::AUTOBK(AUTOBK {
            rbkg: Some(1.4),
            ..Default::default()
        })))?;
        xafs_test_group.calc_background()?;

        xafs_test_group.xftf = Some(XrayFFTF {
            window: Some(FTWindow::Hanning),
            dk: Some(std::f64::EPSILON),
            kmin: Some(0.0),
            kmax: Some(15.0),
            kweight: Some(2.0),
            ..Default::default()
        });
        xafs_test_group.fft()?;

        xafs_test_group.xftr = Some(XrayFFTR {
            window: Some(FTWindow::Hanning),
            rweight: Some(0.0),
            dr: Some(std::f64::EPSILON),
            rmin: Some(0.0),
            rmax: Some(10.0),
            ..Default::default()
        });
        xafs_test_group.ifft()?;

        let mut xas_group = XASGroup::new();
        xas_group.add_spectrum(xafs_test_group);

        let mut xas_group_file = XASGroupFile::new();

        xas_group_file.name = "test.bson".into();
        xas_group_file.data = xas_group;

        xas_group_file.write_bson(&save_path)?;

        Ok(())
    }

    #[test]
    #[allow(non_snake_case)]
    fn test_xas_bson_read() -> Result<(), Box<dyn std::error::Error>> {
        let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let save_path = String::from(TOP_DIR) + "/tests/testfiles/test.bson";
        let mut xafs_test_group = io::load_spectrum_QAS_trans(&path).unwrap();

        xafs_test_group.set_background_method(Some(BackgroundMethod::AUTOBK(AUTOBK {
            rbkg: Some(1.4),
            ..Default::default()
        })))?;
        xafs_test_group.calc_background()?;

        xafs_test_group.xftf = Some(XrayFFTF {
            window: Some(FTWindow::Hanning),
            dk: Some(std::f64::EPSILON),
            kmin: Some(0.0),
            kmax: Some(15.0),
            kweight: Some(2.0),
            ..Default::default()
        });
        xafs_test_group.fft()?;

        xafs_test_group.xftr = Some(XrayFFTR {
            window: Some(FTWindow::Hanning),
            rweight: Some(0.0),
            dr: Some(std::f64::EPSILON),
            rmin: Some(0.0),
            rmax: Some(10.0),
            ..Default::default()
        });
        xafs_test_group.ifft()?;

        let mut xas_group = XASGroup::new();
        xas_group.add_spectrum(xafs_test_group);

        let mut xas_group_file = XASGroupFile::new();

        xas_group_file.name = "test.bson".into();
        xas_group_file.data = xas_group;

        let mut xas_group_read = XASGroupFile::new();
        xas_group_read.read_bson(&save_path)?;

        // TODO:: Assertion of the struct is not working at this momment. Float has to be handled
        // properly.
        // assert_eq!(xas_group_read.data, xas_group_file.data);

        Ok(())
    }
}
