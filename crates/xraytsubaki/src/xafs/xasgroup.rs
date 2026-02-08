#![allow(dead_code)]
#![allow(unused_imports)]

#[cfg_attr(debug_assertions, allow(dead_code, unused_imports))]
// Standard library dependencies
use std::error::Error;
use std::fmt;
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

#[derive(Debug, Clone)]
pub struct BatchSpectrumError {
    pub index: usize,
    pub source: XAFSError,
}

#[derive(Debug, Clone)]
pub struct BatchProcessError {
    pub errors: Vec<BatchSpectrumError>,
}

impl fmt::Display for BatchProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "batch processing failed for {} spectrum(s)",
            self.errors.len()
        )?;
        for err in &self.errors {
            write!(f, "; index {}: {}", err.index, err.source)?;
        }
        Ok(())
    }
}

impl Error for BatchProcessError {}

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
            }
            .into());
        }

        self.spectra.remove(index);
        Ok(self)
    }

    pub fn remove_spectra(&mut self, indices: &[usize]) -> Result<&mut Self, XAFSError> {
        if self.spectra.is_empty() || indices.is_empty() {
            return Ok(self);
        }

        let mut remove_mask = vec![false; self.len()];
        for &index in indices {
            if index < self.spectra.len() {
                remove_mask[index] = true;
            }
        }

        let mut current_index = 0usize;
        self.spectra.retain(|_| {
            let keep = !remove_mask[current_index];
            current_index += 1;
            keep
        });
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
            .filter(|&index| *index < self.spectra.len())
            .copied()
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
        let mut remove_mask = vec![false; self.len()];
        for index in from_index.iter().copied() {
            remove_mask[index] = true;
        }

        // Calculate the shift of the insert index
        let insert_index_shift = from_index.iter().filter(|&index| *index < to_index).count();

        let insert_index = to_index - insert_index_shift;

        let mut current_index = 0usize;
        self.spectra.retain(|_| {
            let keep = !remove_mask[current_index];
            current_index += 1;
            keep
        });

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
            return self
                .spectra
                .last()
                .ok_or_else(|| DataError::EmptyGroup.into());
        }

        Ok(&self.spectra[index])
    }

    pub fn get_spectrum_mut(&mut self, index: usize) -> Result<&mut XASSpectrum, XAFSError> {
        if self.spectra.is_empty() {
            return Err(DataError::EmptyGroup.into());
        }

        if index >= self.spectra.len() {
            return self
                .spectra
                .last_mut()
                .ok_or_else(|| DataError::EmptyGroup.into());
        }

        Ok(&mut self.spectra[index])
    }

    pub fn merge(&mut self, _master: usize, _slave: &[usize]) -> Result<&mut Self, XAFSError> {
        // This feature is not implemented yet
        Err(DataError::NotImplemented {
            feature: "spectrum merge".to_string(),
        }
        .into())
    }

    fn collect_seq_errors<F>(&mut self, mut op: F) -> Result<&mut Self, BatchProcessError>
    where
        F: FnMut(&mut XASSpectrum) -> Result<&mut XASSpectrum, XAFSError>,
    {
        let errors = self
            .spectra
            .iter_mut()
            .enumerate()
            .filter_map(|(index, spectrum)| {
                op(spectrum)
                    .err()
                    .map(|source| BatchSpectrumError { index, source })
            })
            .collect::<Vec<_>>();

        if errors.is_empty() {
            Ok(self)
        } else {
            Err(BatchProcessError { errors })
        }
    }

    fn collect_par_errors<F>(&mut self, op: F) -> Result<&mut Self, BatchProcessError>
    where
        F: Fn(&mut XASSpectrum) -> Result<&mut XASSpectrum, XAFSError> + Sync + Send,
    {
        let mut errors = self
            .spectra
            .par_iter_mut()
            .enumerate()
            .filter_map(|(index, spectrum)| {
                op(spectrum)
                    .err()
                    .map(|source| BatchSpectrumError { index, source })
            })
            .collect::<Vec<_>>();
        errors.sort_by_key(|err| err.index);

        if errors.is_empty() {
            Ok(self)
        } else {
            Err(BatchProcessError { errors })
        }
    }

    pub fn find_e0(&mut self) -> Result<&mut Self, BatchProcessError> {
        self.find_e0_par()
    }

    pub fn find_e0_seq(&mut self) -> Result<&mut Self, BatchProcessError> {
        self.collect_seq_errors(|spectrum| spectrum.find_e0())
    }

    pub fn find_e0_par(&mut self) -> Result<&mut Self, BatchProcessError> {
        self.collect_par_errors(|spectrum| spectrum.find_e0())
    }

    pub fn normalize(&mut self) -> Result<&mut Self, BatchProcessError> {
        self.normalize_par()
    }

    pub fn normalize_seq(&mut self) -> Result<&mut Self, BatchProcessError> {
        self.collect_seq_errors(|spectrum| spectrum.normalize())
    }

    pub fn normalize_par(&mut self) -> Result<&mut Self, BatchProcessError> {
        self.collect_par_errors(|spectrum| spectrum.normalize())
    }

    pub fn calc_background(&mut self) -> Result<&mut Self, BatchProcessError> {
        self.calc_background_par()
    }

    pub fn calc_background_seq(&mut self) -> Result<&mut Self, BatchProcessError> {
        self.collect_seq_errors(|spectrum| spectrum.calc_background())
    }

    pub fn calc_background_par(&mut self) -> Result<&mut Self, BatchProcessError> {
        self.collect_par_errors(|spectrum| spectrum.calc_background())
    }

    pub fn fft(&mut self) -> Result<&mut Self, BatchProcessError> {
        self.fft_par()
    }

    pub fn fft_seq(&mut self) -> Result<&mut Self, BatchProcessError> {
        self.collect_seq_errors(|spectrum| spectrum.fft())
    }

    pub fn fft_par(&mut self) -> Result<&mut Self, BatchProcessError> {
        self.collect_par_errors(|spectrum| spectrum.fft())
    }

    pub fn ifft(&mut self) -> Result<&mut Self, BatchProcessError> {
        self.ifft_par()
    }

    pub fn ifft_seq(&mut self) -> Result<&mut Self, BatchProcessError> {
        self.collect_seq_errors(|spectrum| spectrum.ifft())
    }

    pub fn ifft_par(&mut self) -> Result<&mut Self, BatchProcessError> {
        self.collect_par_errors(|spectrum| spectrum.ifft())
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

    fn assert_slice_close(left: &[f64], right: &[f64], epsilon: f64) {
        assert_eq!(left.len(), right.len());
        for (l, r) in left.iter().zip(right.iter()) {
            assert_abs_diff_eq!(l, r, epsilon = epsilon);
        }
    }

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

    #[test]
    fn test_batch_find_e0_returns_structured_error_seq_and_par() {
        let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let valid = io::load_spectrum_QAS_trans(&path).unwrap();
        let invalid = XASSpectrum::new();

        let mut seq_group = XASGroup::new();
        seq_group.add_spectrum(valid.clone());
        seq_group.add_spectrum(invalid.clone());
        let seq_err = seq_group.find_e0_seq().unwrap_err();
        assert_eq!(seq_err.errors.len(), 1);
        assert_eq!(seq_err.errors[0].index, 1);

        let mut par_group = XASGroup::new();
        par_group.add_spectrum(valid);
        par_group.add_spectrum(invalid);
        let par_err = par_group.find_e0_par().unwrap_err();
        assert_eq!(par_err.errors.len(), 1);
        assert_eq!(par_err.errors[0].index, 1);
    }

    #[test]
    fn test_default_and_par_error_semantics_match() {
        let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let valid = io::load_spectrum_QAS_trans(&path).unwrap();
        let invalid = XASSpectrum::new();

        let mut par_group = XASGroup::new();
        par_group
            .add_spectrum(valid.clone())
            .add_spectrum(invalid.clone());
        let mut default_group = XASGroup::new();
        default_group.add_spectrum(valid).add_spectrum(invalid);

        let par_err = par_group.find_e0_par().unwrap_err();
        let default_err = default_group.find_e0().unwrap_err();

        assert_eq!(par_err.errors.len(), default_err.errors.len());
        for (par, default_) in par_err.errors.iter().zip(default_err.errors.iter()) {
            assert_eq!(par.index, default_.index);
            assert_eq!(par.source.to_string(), default_.source.to_string());
        }
    }

    #[test]
    fn test_seq_par_default_numerical_equivalence() {
        let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let base = io::load_spectrum_QAS_trans(&path).unwrap();

        let mut group_seq = XASGroup::new();
        group_seq
            .add_spectrum(base.clone())
            .add_spectrum(base.clone());

        let mut group_par = group_seq.clone();
        let mut group_default = group_seq.clone();

        group_seq.find_e0_seq().unwrap();
        group_seq.normalize_seq().unwrap();
        group_seq.calc_background_seq().unwrap();
        group_seq.fft_seq().unwrap();

        group_par.find_e0_par().unwrap();
        group_par.normalize_par().unwrap();
        group_par.calc_background_par().unwrap();
        group_par.fft_par().unwrap();

        group_default.find_e0().unwrap();
        group_default.normalize().unwrap();
        group_default.calc_background().unwrap();
        group_default.fft().unwrap();

        for index in 0..group_seq.len() {
            let seq = &group_seq.spectra[index];
            let par = &group_par.spectra[index];
            let default = &group_default.spectra[index];

            assert_abs_diff_eq!(
                seq.get_e0().unwrap(),
                par.get_e0().unwrap(),
                epsilon = 1.0e-8
            );
            assert_abs_diff_eq!(
                par.get_e0().unwrap(),
                default.get_e0().unwrap(),
                epsilon = 1.0e-8
            );

            let seq_norm = seq
                .normalization
                .as_ref()
                .and_then(|method| method.get_norm())
                .unwrap();
            let par_norm = par
                .normalization
                .as_ref()
                .and_then(|method| method.get_norm())
                .unwrap();
            let default_norm = default
                .normalization
                .as_ref()
                .and_then(|method| method.get_norm())
                .unwrap();
            assert_slice_close(
                seq_norm.as_slice().unwrap(),
                par_norm.as_slice().unwrap(),
                1.0e-6,
            );
            assert_slice_close(
                par_norm.as_slice().unwrap(),
                default_norm.as_slice().unwrap(),
                1.0e-6,
            );

            let seq_k = seq.get_k().unwrap();
            let par_k = par.get_k().unwrap();
            let default_k = default.get_k().unwrap();
            assert_slice_close(seq_k.as_slice(), par_k.as_slice(), 1.0e-8);
            assert_slice_close(par_k.as_slice(), default_k.as_slice(), 1.0e-8);

            let seq_chi = seq.get_chi().unwrap();
            let par_chi = par.get_chi().unwrap();
            let default_chi = default.get_chi().unwrap();
            assert_slice_close(seq_chi.as_slice(), par_chi.as_slice(), 1.0e-6);
            assert_slice_close(par_chi.as_slice(), default_chi.as_slice(), 1.0e-6);

            let seq_chir_imag = seq.get_chir_imag().unwrap();
            let par_chir_imag = par.get_chir_imag().unwrap();
            let default_chir_imag = default.get_chir_imag().unwrap();
            assert_slice_close(
                seq_chir_imag.as_slice(),
                par_chir_imag.as_slice(),
                1.0e-6,
            );
            assert_slice_close(
                par_chir_imag.as_slice(),
                default_chir_imag.as_slice(),
                1.0e-6,
            );
        }
    }
}
