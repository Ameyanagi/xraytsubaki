// This is a mock test that doesn't rely on the actual FFI bindings

class MockXASSpectrum {
  name: string | null = null;
  energy: Float64Array | null = null;
  mu: Float64Array | null = null;
  e0: number | null = null;
  k: Float64Array | null = null;
  chi: Float64Array | null = null;
  
  constructor(name?: string, energy?: Float64Array, mu?: Float64Array) {
    if (name) this.name = name;
    if (energy) this.energy = energy;
    if (mu) this.mu = mu;
  }
  
  findE0(): number {
    // Mock implementation that just takes the midpoint energy value
    if (!this.energy || !this.mu) {
      throw new Error('Energy and mu must be set');
    }
    
    const index = Math.floor(this.energy.length / 2);
    this.e0 = this.energy[index];
    return this.e0;
  }
  
  normalize(): number {
    // Mock implementation
    console.log('Normalizing spectrum');
    return 1.0;
  }
  
  calcBackground(): void {
    // Mock implementation
    console.log('Calculating background');
    
    // Create mock k and chi arrays
    if (this.energy) {
      const length = Math.max(0, this.energy.length - 5);
      this.k = new Float64Array(length);
      this.chi = new Float64Array(length);
      
      for (let i = 0; i < length; i++) {
        this.k[i] = 0.1 * (i + 1);
        this.chi[i] = Math.sin(this.k[i]) * Math.exp(-0.5 * this.k[i]);
      }
    }
  }
}

class MockXASGroup {
  private spectra: MockXASSpectrum[] = [];
  
  constructor() {}
  
  addSpectrum(spectrum: MockXASSpectrum): void {
    this.spectra.push(spectrum);
  }
  
  length(): number {
    return this.spectra.length;
  }
  
  getSpectrum(index: number): MockXASSpectrum | null {
    if (index < 0 || index >= this.spectra.length) {
      return null;
    }
    return this.spectra[index];
  }
  
  findE0(): void {
    for (const spectrum of this.spectra) {
      spectrum.findE0();
    }
  }
}

function mockFindE0(energy: Float64Array, mu: Float64Array): number {
  // Mock implementation that just takes the midpoint energy value
  const index = Math.floor(energy.length / 2);
  return energy[index];
}

// Mock test
console.log('Running mock test of TypeScript bindings');

// Create test data
const energy = new Float64Array([21900, 21950, 22000, 22050, 22100, 22150, 22200, 22250, 22300]);
const mu = new Float64Array([0.1, 0.2, 0.3, 0.5, 0.8, 0.9, 1.0, 1.0, 1.0]);

// Test XASSpectrum
const spectrum = new MockXASSpectrum('test-spectrum', energy, mu);
console.log(`Created spectrum with name: ${spectrum.name}`);
console.log(`Energy array length: ${spectrum.energy?.length}`);

// Test findE0 function
const e0 = mockFindE0(energy, mu);
console.log(`Found E0: ${e0}`);

// Test spectrum methods
spectrum.findE0();
console.log(`Spectrum E0: ${spectrum.e0}`);
spectrum.normalize();
spectrum.calcBackground();
console.log(`k array length: ${spectrum.k?.length}`);
console.log(`chi array length: ${spectrum.chi?.length}`);

// Test XASGroup
const group = new MockXASGroup();
group.addSpectrum(spectrum);

const spectrum2 = new MockXASSpectrum('test-spectrum-2', energy, mu);
group.addSpectrum(spectrum2);

console.log(`Group contains ${group.length()} spectra`);
console.log(`First spectrum name: ${group.getSpectrum(0)?.name}`);

group.findE0();
console.log(`Second spectrum E0: ${group.getSpectrum(1)?.e0}`);

console.log('Mock test completed successfully!');