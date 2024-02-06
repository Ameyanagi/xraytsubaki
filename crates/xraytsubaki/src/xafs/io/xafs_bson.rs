use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::mem;

use bson::bson;
use bson::Bson;
use bson::Document;

use serde::{Deserialize, Serialize};
use version::version;

use crate::xafs::xasgroup::XASGroup;
use crate::xafs::xasspectrum::XASSpectrum;

#[derive(Serialize, Deserialize, Default, Debug)]
pub enum XASBsonDataType {
    #[default]
    XASGroup,
    // Currently the xas bson is implemented only for XASGroup. I am thinking that it should not be implemented for XASSpectrum.
    XASSpectrum,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct XASGroupFile {
    pub version: String,
    pub name: String,
    pub datatype: XASBsonDataType,
    pub data: XASGroup,
}

impl XASGroupFile {
    pub fn new() -> XASGroupFile {
        XASGroupFile {
            version: version!().to_string(),
            name: String::new(),
            datatype: XASBsonDataType::XASGroup,
            data: XASGroup::new(),
        }
    }
}

pub trait XASBson {
    fn read_bson(&mut self, filename: &str) -> Result<&mut Self, Box<dyn Error>>;

    fn write_bson(&mut self, filename: &str) -> Result<&mut Self, Box<dyn Error>>;
}

impl XASBson for XASGroupFile {
    fn read_bson(&mut self, filename: &str) -> Result<&mut Self, Box<dyn Error>> {
        let mut f_buffer = File::open(filename)?;

        let doc = Document::from_reader(&mut f_buffer)?;

        // let xas_group_file: XASGroupFile =

        _ = mem::replace(self, bson::from_document(doc)?);

        Ok(self)
    }

    fn write_bson(&mut self, filename: &str) -> Result<&mut Self, Box<dyn Error>> {
        self.version = version!().to_string();
        self.datatype = XASBsonDataType::XASGroup;

        let data_bson = bson::to_bson(&self)?;

        let mut data_file = File::create(filename)?;

        data_bson
            .as_document()
            .unwrap_or(&Document::new())
            .to_writer(&mut data_file)?;

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

        assert_eq!(xas_group_read.data, xas_group_file.data);

        Ok(())
    }
}
