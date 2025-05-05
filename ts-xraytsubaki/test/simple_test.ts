import { XASSpectrum, findE0 } from '../src';

// Create a simple spectrum with test data
const energy = new Float64Array([21900, 22000, 22100, 22200, 22300]);
const mu = new Float64Array([0.1, 0.2, 0.8, 1.0, 1.0]);

// Test creating a spectrum
const spectrum = new XASSpectrum('test-spectrum', energy, mu);
console.log(`Created spectrum with name: ${spectrum.name}`);
console.log(`Energy array length: ${spectrum.energy?.length}`);

// Test findE0 function
const e0 = findE0(energy, mu);
console.log(`Found E0: ${e0}`);

// Set E0 on the spectrum
spectrum.e0 = e0;
console.log(`Spectrum E0: ${spectrum.e0}`);

console.log('Test completed successfully!');