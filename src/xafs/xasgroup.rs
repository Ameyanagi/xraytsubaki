#![allow(dead_code)]
#![allow(unused_imports)]

#[cfg_attr(debug_assertions, allow(dead_code, unused_imports))]

// Standard library dependencies
use std::error::Error;

// External dependencies


// load dependencies
use super::xasspectrum
use super::XAFSError;

// Load local traits
use xasspectrum::XASSpectrum;


#[derive(Debug, Clone)]
pub struct XASGroup{
    pub spectra: Vec<XASSpectrum>,
}

impl Default for XASGroup{
    fn default() -> Self{
        Self::new()
    }
}


impl XASGroup{
    pub fn new() -> Self{
        Self{
            spectra: Vec::new(),
        }
    }

    pub fn add_spectrum(&mut self, spectrum: XASSpectrum) -> &mut Self{
        self.spectra.push(spectrum);
        self
    }

    pub fn remove_spectrum(&mut self, index: usize) -> Result<&mut Self, Box<dyn Error>>{
        if index >= self.spectra.len(){
            return Err(Box::new(XAFSError::GroupIndexOutOfRange));
        }
        
        self.spectra.remove(index);
        Ok(self)
    }

    pub fn move_spectrum(&mut self, from: usize, to: usize)-> Result<&mut Self, Box<dyn Error>>{
        // TODO: check if it is fast enough

        if from >= self.spectra.len() || to >= self.spectra.len(){
            return Err(Box::new(XAFSError::GroupIndexOutOfRange));
        }

        let spectrum = self.spectra.remove(from);
        self.spectra.insert(to, spectrum);
        Ok(self)
    }

    pub fn move_spectra(&mut self, from: &[usize], to: usize) -> Result<&mut Self, Box<dyn Error>>{

        if to >= self.spectra.len(){
            return Err(Box::new(XAFSError::GroupIndexOutOfRange));
        }

        todo!("move_spectra");


        // let mut spectra = Vec::from(&self.spectra[from[0]..from[from.len()-1]]);
        // for index in from.iter(){
            // if *index >= self.spectra.len(){
                // return Err(Box::new(XAFSError::GroupIndexOutOfRange));
            // }
            // spectra.push(self.spectra.remove(*index));
        }

        // self.spectra.splice(to..to, spectra);
        Ok(self)
    }

    pub fn get_spectrum(&self, index: usize) -> Result<&XASSpectrum, Box<dyn Error>>{
        if index >= self.spectra.len(){
            return Err(Box::new(XAFSError::GroupIndexOutOfRange));
        }

        Ok(&self.spectra[index])
    }

    pub fn get_spectrum_mut(&mut self, index: usize) -> Result<&mut XASSpectrum, Box<dyn Error>>{
        if index >= self.spectra.len(){
            return Err(Box::new(XAFSError::GroupIndexOutOfRange));
        }

        Ok(&mut self.spectra[index])
    }

    pub fn merge(&mut self, master: usize, slave: &[usize]) -> Result<&mut Self, Box<dyn Error>>{
        todo!("merge")
        
        // self.spectra.extend(other.spectra.clone());
        Ok(self)
    }

    


}



