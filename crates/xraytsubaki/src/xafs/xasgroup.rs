#![allow(dead_code)]
#![allow(unused_imports)]

#[cfg_attr(debug_assertions, allow(dead_code, unused_imports))]
// Standard library dependencies
use std::error::Error;
use std::mem;

// External dependencies
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

// load dependencies
use super::errors::DataError;
use super::xasspectrum;
use super::XAFSError;

use itertools::Itertools;

// Load local traits
use crate::xafs::io::xasdatatype::XASGroupFile;
use crate::xafs::io::{xafs_bson::XASBson, xafs_json::XASJson};
use crate::xafs::xasspectrum::XASSpectrum;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct XASGroup {
    pub spectra: Vec<XASSpectrum>,
}

impl Default for XASGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl XASGroup {
    pub fn new() -> Self {
        Self {
            spectra: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.spectra.len()
    }

    pub fn is_empty(&self) -> bool {
        self.spectra.is_empty()
    }

    pub fn add_spectrum(&mut self, spectrum: XASSpectrum) -> &mut Self {
        self.spectra.push(spectrum);
        self
    }

    pub fn add_spectra(&mut self, spectra: Vec<XASSpectrum>) -> &mut Self {
        self.spectra.extend(spectra);
        self
    }

    pub fn add_group(&mut self, group: XASGroup) -> &mut Self {
        self.spectra.extend(group.spectra);
        self
    }

    pub fn remove_spectrum(&mut self, index: usize) -> Result<&mut Self, XAFSError> {
        if index >= self.spectra.len() {
            return Err(DataError::IndexOutOfRange {
                index,
                length: self.spectra.len(),
            }.into());
        }

        self.spectra.remove(index);
        Ok(self)
    }

    pub fn remove_spectra(&mut self, indices: &[usize]) -> Result<&mut Self, XAFSError> {
        let mut indices = indices.to_vec();
        indices.sort();
        indices.dedup();
        let mut remove_index_iter = (0..self.len()).map(|index| !indices.contains(&index));
        self.spectra.retain(|_| remove_index_iter.next().unwrap());
        Ok(self)
    }

    pub fn move_spectrum(&mut self, from: usize, to: usize) -> &mut Self {
        // TODO: check if it is fast enough

        let from_index = if from < self.spectra.len() {
            from
        } else {
            self.spectra.len() - 1
        };

        let to_index = if to <= self.spectra.len() {
            to
        } else {
            self.spectra.len()
        };

        if from_index + 1 == to_index {
            return self;
        }

        let tmp_spectrum = mem::take(&mut self.spectra[from_index]);
        self.spectra.insert(to_index, tmp_spectrum);

        if from_index > to_index {
            self.spectra.remove(from_index + 1);
        } else {
            self.spectra.remove(from_index);
        }

        self
    }

    pub fn move_spectra(&mut self, from: &[usize], to: usize) -> &mut Self {
        let to_index = if to <= self.spectra.len() {
            to
        } else {
            self.spectra.len()
        };

        // Remove the duplicate index from the from list
        let mut from_index: Vec<usize> = from
            .as_ref()
            .iter()
            .filter(|&index| *index < self.spectra.len()).copied()
            .collect::<Vec<usize>>();

        from_index.sort();
        from_index.dedup();

        // Create a temporary vector to store the spectra to be moved
        // It is moved by mem::take() to avoid cloning
        let mut tmp_spectra = Vec::with_capacity(from_index.len());

        for index in from_index.iter() {
            tmp_spectra.push(mem::take(&mut self.spectra[*index]));
        }

        // Create a iterator to remove the spectra from the group
        let mut remove_index_iter = (0..self.len()).map(|index| !from_index.contains(&index));

        // Calculate the shift of the insert index
        let insert_index_shift = from_index.iter().filter(|&index| *index < to_index).count();

        let insert_index = to_index - insert_index_shift;

        self.spectra.retain(|_| remove_index_iter.next().unwrap());

        let (left_spectra, right_spectra) = self.spectra.split_at_mut(insert_index);

        // I think this part is not very efficient
        // TODO: check if it is fast enough
        self.spectra = left_spectra
            .iter_mut()
            .chain(tmp_spectra.iter_mut())
            .chain(right_spectra.iter_mut())
            .map(mem::take)
            .collect::<Vec<XASSpectrum>>();
        self
    }

    pub fn get_spectrum(&self, index: usize) -> Result<&XASSpectrum, XAFSError> {
        if self.spectra.is_empty() {
            return Err(DataError::EmptyGroup.into());
        }

        if index >= self.spectra.len() {
            return Ok(self.spectra.last().unwrap());
        }

        Ok(&self.spectra[index])
    }

    pub fn get_spectrum_mut(&mut self, index: usize) -> Result<&mut XASSpectrum, XAFSError> {
        if self.spectra.is_empty() {
            return Err(DataError::EmptyGroup.into());
        }

        if index >= self.spectra.len() {
            return Ok(self.spectra.last_mut().unwrap());
        }

        Ok(&mut self.spectra[index])
    }

    pub fn merge(&mut self, _master: usize, _slave: &[usize]) -> Result<&mut Self, XAFSError> {
        // This feature is not implemented yet
        Err(DataError::NotImplemented {
            feature: "spectrum merge".to_string(),
        }.into())
    }

    pub fn find_e0(&mut self) -> Result<&mut Self, XAFSError> {
        self.spectra.par_iter_mut().for_each(|spectrum| {
            spectrum.find_e0().unwrap();
        });

        Ok(self)
    }

    pub fn find_e0_seq(&mut self) -> Result<&mut Self, XAFSError> {
        self.spectra.iter_mut().for_each(|spectrum| {
            spectrum.find_e0().unwrap();
        });

        Ok(self)
    }

    pub fn find_e0_par(&mut self) -> Result<&mut Self, XAFSError> {
        self.spectra.par_iter_mut().for_each(|spectrum| {
            spectrum.find_e0().unwrap();
        });

        Ok(self)
    }

    pub fn normalize(&mut self) -> Result<&mut Self, XAFSError> {
        self.spectra.par_iter_mut().for_each(|spectrum| {
            spectrum.normalize().unwrap();
        });

        Ok(self)
    }

    pub fn normalize_seq(&mut self) -> Result<&mut Self, XAFSError> {
        self.spectra.iter_mut().for_each(|spectrum| {
            spectrum.normalize().unwrap();
        });

        Ok(self)
    }

    pub fn normalize_par(&mut self) -> Result<&mut Self, XAFSError> {
        self.spectra.par_iter_mut().for_each(|spectrum| {
            spectrum.normalize().unwrap();
        });

        Ok(self)
    }

    pub fn calc_background(&mut self) -> Result<&mut Self, XAFSError> {
        self.spectra.par_iter_mut().for_each(|spectrum| {
            spectrum.calc_background().unwrap();
        });

        Ok(self)
    }

    pub fn calc_background_seq(&mut self) -> Result<&mut Self, XAFSError> {
        self.spectra.iter_mut().for_each(|spectrum| {
            spectrum.calc_background().unwrap();
        });

        Ok(self)
    }

    pub fn calc_background_par(&mut self) -> Result<&mut Self, XAFSError> {
        self.spectra.par_iter_mut().for_each(|spectrum| {
            spectrum.calc_background().unwrap();
        });

        Ok(self)
    }

    pub fn fft(&mut self) -> Result<&mut Self, XAFSError> {
        self.spectra.par_iter_mut().for_each(|spectrum| {
            spectrum.fft().unwrap();
        });

        Ok(self)
    }

    pub fn fft_seq(&mut self) -> Result<&mut Self, XAFSError> {
        self.spectra.iter_mut().for_each(|spectrum| {
            spectrum.fft().unwrap();
        });

        Ok(self)
    }

    pub fn fft_par(&mut self) -> Result<&mut Self, XAFSError> {
        self.spectra.par_iter_mut().for_each(|spectrum| {
            spectrum.fft().unwrap();
        });

        Ok(self)
    }

    pub fn ifft(&mut self) -> Result<&mut Self, XAFSError> {
        self.spectra.par_iter_mut().for_each(|spectrum| {
            spectrum.ifft().unwrap();
        });

        Ok(self)
    }

    pub fn ifft_seq(&mut self) -> Result<&mut Self, XAFSError> {
        self.spectra.iter_mut().for_each(|spectrum| {
            spectrum.ifft().unwrap();
        });

        Ok(self)
    }

    pub fn ifft_par(&mut self) -> Result<&mut Self, XAFSError> {
        self.spectra.par_iter_mut().for_each(|spectrum| {
            spectrum.ifft().unwrap();
        });

        Ok(self)
    }

    pub fn read_bson(&mut self, filename: &str) -> Result<&mut Self, XAFSError> {
        let mut xas_group_file = XASGroupFile::new();

        xas_group_file.read_bson(filename)?;

        _ = mem::replace(self, xas_group_file.data);

        Ok(self)
    }

    pub fn write_bson(&self, filename: &str) -> Result<&Self, XAFSError> {
        let mut xas_group_file = XASGroupFile::new();

        xas_group_file.name = filename.to_string();
        xas_group_file.data = self.clone();
        xas_group_file.write_bson(filename)?;

        Ok(self)
    }

    pub fn add_spectrum_from_bson(&mut self, filename: &str) -> Result<&mut Self, XAFSError> {
        let mut xas_group_file = XASGroupFile::new();
        xas_group_file.read_bson(filename)?;
        self.add_group(xas_group_file.data);

        Ok(self)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::xafs::io;
    use crate::xafs::nshare::ToNalgebra;
    use approx::{assert_abs_diff_eq, assert_relative_eq};

    use data_reader::reader::{load_txt_f64, Delimiter, ReaderParams};

    use crate::xafs::tests::PARAM_LOADTXT;
    use crate::xafs::tests::TEST_TOL;
    use crate::xafs::tests::TOP_DIR;

    #[test]
    fn test_xasgroup() {
        let mut group = XASGroup::new();
        let spectrum = XASSpectrum::new();

        assert_eq!(group.len(), 0);
    }

    #[test]
    fn test_add_spectrum() {
        let mut group = XASGroup::new();
        let spectrum = XASSpectrum::new();
        group.add_spectrum(spectrum.clone());
        assert_eq!(group.len(), 1);
    }

    #[test]
    fn test_remove_spectrum() {
        let mut group = XASGroup::new();
        let spectrum = XASSpectrum::new();
        group.add_spectrum(spectrum.clone());
        group.remove_spectrum(0);
        assert_eq!(group.len(), 0);
    }

    #[test]
    fn test_move_spectrum() {
        let mut group = XASGroup::new();
        let spectrum = XASSpectrum::new();
        group.add_spectrum(spectrum.clone().set_name("spectrum1").to_owned());
        group.add_spectrum(spectrum.clone().set_name("spectrum2").to_owned());
        group.add_spectrum(spectrum.clone().set_name("spectrum3").to_owned());
        group.move_spectrum(1, 0);
        assert_eq!(group.spectra[0].name.as_ref().unwrap(), "spectrum2");

        group.move_spectrum(0, group.len());
        assert_eq!(group.spectra[2].name.as_ref().unwrap(), "spectrum2");

        group.move_spectrum(10, group.len());
        println!("{:?}", group);
        assert_eq!(group.spectra[2].name.as_ref().unwrap(), "spectrum2");

        group.move_spectrum(10, 0);
        assert_eq!(group.spectra[0].name.as_ref().unwrap(), "spectrum2");

        group.move_spectrum(0, 10);
        assert_eq!(group.spectra[2].name.as_ref().unwrap(), "spectrum2");
    }

    #[test]
    fn test_move_spectra() {
        let mut group = XASGroup::new();
        let spectrum = XASSpectrum::new();
        group.add_spectrum(spectrum.clone().set_name("spectrum1").to_owned());
        group
            .add_spectrum(spectrum.clone().set_name("spectrum2").to_owned())
            .to_owned();
        group
            .add_spectrum(spectrum.clone().set_name("spectrum3").to_owned())
            .to_owned();
        group.move_spectra(&[0, 1], 3);
        assert_eq!(group.spectra[2].name.as_ref().unwrap(), "spectrum2");
    }
}
