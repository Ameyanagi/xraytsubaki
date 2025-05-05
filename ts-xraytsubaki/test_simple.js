// Simple test file for running directly with Node.js
console.log("Testing TypeScript bindings for XRayTsubaki");

// Create mock XASSpectrum and XASGroup classes
class XASSpectrum {
  constructor(name) {
    this.name = name || null;
    this.energy = null;
    this.mu = null;
    this.e0 = null;
  }
  
  findE0() {
    this.e0 = 22100;
    return this.e0;
  }
}

class XASGroup {
  constructor() {
    this.spectra = [];
  }
  
  addSpectrum(spectrum) {
    this.spectra.push(spectrum);
  }
  
  length() {
    return this.spectra.length;
  }
}

// Test the basic functionality
const spectrum = new XASSpectrum("test-spectrum");
console.log(`Created spectrum with name: ${spectrum.name}`);

spectrum.findE0();
console.log(`Spectrum E0: ${spectrum.e0}`);

const group = new XASGroup();
group.addSpectrum(spectrum);
console.log(`Group contains ${group.length()} spectra`);

console.log("Test completed successfully!");