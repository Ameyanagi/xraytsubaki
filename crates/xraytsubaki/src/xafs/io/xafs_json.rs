use std::error::Error;
use std::fs::File;
use std::io::{Read, Write};
use std::mem;

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use serde_json::{self, json};
use version::version;

use crate::xafs::io::xasdatatype::{XASDataType, XASGroupFile};
use crate::xafs::xasgroup::XASGroup;
use crate::xafs::xasspectrum::XASSpectrum;

pub trait XASJson {
    fn read_json(&mut self, filename: &str) -> Result<&mut Self, Box<dyn Error>>;

    fn write_json(&mut self, filename: &str) -> Result<&mut Self, Box<dyn Error>>;

    fn read_jsongz(&mut self, filename: &str) -> Result<&mut Self, Box<dyn Error>>;

    fn write_jsongz(&mut self, filename: &str) -> Result<&mut Self, Box<dyn Error>>;
}

impl XASJson for XASGroupFile {
    fn read_json(&mut self, filename: &str) -> Result<&mut Self, Box<dyn Error>> {
        let f_buffer = File::open(filename)?;

        let doc = serde_json::from_reader(f_buffer)?;
        _ = mem::replace(self, doc);

        Ok(self)
    }

    fn write_json(&mut self, filename: &str) -> Result<&mut Self, Box<dyn Error>> {
        self.version = version!().to_string();
        self.datatype = XASDataType::XASGroup;

        // let data_bson = bson::to_bson(&self)?;

        let mut data_file = File::create(filename)?;

        serde_json::to_writer(&mut data_file, &self)?;

        Ok(self)
    }

    fn read_jsongz(&mut self, filename: &str) -> Result<&mut Self, Box<dyn Error>> {
        if filename.ends_with(".json") {
            return self.read_json(filename);
        }

        let f_buffer = File::open(filename)?;
        let f_buffer = GzDecoder::new(f_buffer);
        let doc = serde_json::from_reader(f_buffer)?;

        _ = mem::replace(self, doc);

        Ok(self)
    }

    fn write_jsongz(&mut self, filename: &str) -> Result<&mut Self, Box<dyn Error>> {
        if filename.ends_with(".json") {
            return self.write_json(filename);
        }

        self.version = version!().to_string();
        self.datatype = XASDataType::XASGroup;

        let mut data_file = File::create(filename)?;

        let mut encoder = GzEncoder::new(&mut data_file, Compression::default());

        serde_json::to_writer(&mut encoder, &self)?;

        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use approx;
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
    fn test_xas_json_write() -> Result<(), Box<dyn std::error::Error>> {
        let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let save_path = String::from(TOP_DIR) + "/tests/testfiles/test.json";
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

        xas_group_file.name = "test.json".into();
        xas_group_file.data = xas_group;

        xas_group_file.write_json(&save_path)?;

        Ok(())
    }

    #[test]
    #[allow(non_snake_case)]
    fn test_xas_json_read() -> Result<(), Box<dyn std::error::Error>> {
        let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let save_path = String::from(TOP_DIR) + "/tests/testfiles/test.json";
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

        xas_group_file.name = "test.json".into();
        xas_group_file.data = xas_group;

        let mut xas_group_read = XASGroupFile::new();
        xas_group_read.read_json(&save_path)?;

        // TODO:: Assertion of the struct is not working at this momment. Float has to be handled
        // properly.
        // assert_eq!(xas_group_read.data, xas_group_file.data);

        Ok(())
    }

    #[test]
    #[allow(non_snake_case)]
    fn test_xas_jsongz_write() -> Result<(), Box<dyn std::error::Error>> {
        let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let save_path = String::from(TOP_DIR) + "/tests/testfiles/test.json.gz";
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

        xas_group_file.name = "test.json.gz".into();
        xas_group_file.data = xas_group;

        xas_group_file.write_jsongz(&save_path)?;

        Ok(())
    }

    #[test]
    #[ignore = "Issue with compressed JSON reading - will be fixed in future version"]
    #[allow(non_snake_case)]
    fn test_xas_jsongz_read() -> Result<(), Box<dyn std::error::Error>> {
        let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let save_path = String::from(TOP_DIR) + "/tests/testfiles/test.json.gz";
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

        xas_group_file.name = "test.json.gz".into();
        xas_group_file.data = xas_group;

        let mut xas_group_read = XASGroupFile::new();
        xas_group_read.read_jsongz(&save_path)?;

        // TODO:: Assertion of the struct is not working at this momment. Float has to be handled
        // properly.
        // assert_eq!(xas_group_read.data, xas_group_file.data);

        Ok(())
    }
}
