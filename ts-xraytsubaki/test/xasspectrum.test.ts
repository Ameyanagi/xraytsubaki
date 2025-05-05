import { XASSpectrum } from '../src';
import * as path from 'path';
import * as fs from 'fs';

describe('XASSpectrum', () => {
  // Test constants
  const TEST_DATA_DIR = path.join(__dirname, '../../crates/xraytsubaki/tests/testfiles');
  const SAMPLE_FILE = path.join(TEST_DATA_DIR, 'Ru_QAS.dat');
  
  it('should create an empty spectrum', () => {
    const spectrum = new XASSpectrum();
    expect(spectrum).toBeDefined();
    expect(spectrum.name).toBeNull();
    expect(spectrum.energy).toBeNull();
    expect(spectrum.mu).toBeNull();
    expect(spectrum.e0).toBeNull();
  });

  it('should create a spectrum with name', () => {
    const spectrum = new XASSpectrum('test-spectrum');
    expect(spectrum.name).toBe('test-spectrum');
  });

  it('should create a spectrum with data', () => {
    const energy = new Float64Array([1.0, 2.0, 3.0]);
    const mu = new Float64Array([4.0, 5.0, 6.0]);
    
    const spectrum = new XASSpectrum('test-spectrum', energy, mu);
    
    expect(spectrum.name).toBe('test-spectrum');
    expect(spectrum.energy).toEqual(energy);
    expect(spectrum.mu).toEqual(mu);
  });

  it('should load from QAS file', () => {
    // Skip test if file doesn't exist
    if (!fs.existsSync(SAMPLE_FILE)) {
      console.warn(`Test file not found: ${SAMPLE_FILE}`);
      return;
    }
    
    const spectrum = XASSpectrum.fromFile(SAMPLE_FILE);
    
    expect(spectrum).toBeDefined();
    expect(spectrum.energy).not.toBeNull();
    expect(spectrum.mu).not.toBeNull();
    expect(spectrum.energy!.length).toBeGreaterThan(0);
    expect(spectrum.mu!.length).toBeGreaterThan(0);
    expect(spectrum.energy!.length).toBe(spectrum.mu!.length);
  });

  it('should find e0', () => {
    // Skip test if file doesn't exist
    if (!fs.existsSync(SAMPLE_FILE)) {
      console.warn(`Test file not found: ${SAMPLE_FILE}`);
      return;
    }
    
    const spectrum = XASSpectrum.fromFile(SAMPLE_FILE);
    spectrum.findE0();
    
    expect(spectrum.e0).not.toBeNull();
    // The expected e0 value for the Ru_QAS sample is around 22117
    expect(spectrum.e0).toBeGreaterThan(22000);
    expect(spectrum.e0).toBeLessThan(22200);
  });

  it('should normalize spectrum', () => {
    // Skip test if file doesn't exist
    if (!fs.existsSync(SAMPLE_FILE)) {
      console.warn(`Test file not found: ${SAMPLE_FILE}`);
      return;
    }
    
    const spectrum = XASSpectrum.fromFile(SAMPLE_FILE);
    spectrum.findE0();
    spectrum.normalize();
    
    const normalization = spectrum.getNormalization();
    expect(normalization).not.toBeNull();
    
    const norm = normalization!.getNorm();
    expect(norm).not.toBeNull();
    expect(norm!.length).toBe(spectrum.energy!.length);
    
    // Check that normalized values are generally between 0 and ~1.2
    // (normalized XANES can go a bit above 1.0)
    const maxNorm = Math.max(...norm!);
    expect(maxNorm).toBeGreaterThan(0.9);
    expect(maxNorm).toBeLessThan(1.5);
  });

  it('should calculate background and extract chi(k)', () => {
    // Skip test if file doesn't exist
    if (!fs.existsSync(SAMPLE_FILE)) {
      console.warn(`Test file not found: ${SAMPLE_FILE}`);
      return;
    }
    
    const spectrum = XASSpectrum.fromFile(SAMPLE_FILE);
    spectrum.findE0();
    spectrum.normalize();
    spectrum.calcBackground();
    
    expect(spectrum.k).not.toBeNull();
    expect(spectrum.chi).not.toBeNull();
    expect(spectrum.k!.length).toBeGreaterThan(0);
    expect(spectrum.chi!.length).toBe(spectrum.k!.length);
  });

  it('should perform forward FT to get chi(R)', () => {
    // Skip test if file doesn't exist
    if (!fs.existsSync(SAMPLE_FILE)) {
      console.warn(`Test file not found: ${SAMPLE_FILE}`);
      return;
    }
    
    const spectrum = XASSpectrum.fromFile(SAMPLE_FILE);
    spectrum.findE0();
    spectrum.normalize();
    spectrum.calcBackground();
    spectrum.fft();
    
    expect(spectrum.r).not.toBeNull();
    expect(spectrum.chiR).not.toBeNull();
    expect(spectrum.chiRMag).not.toBeNull();
    expect(spectrum.r!.length).toBeGreaterThan(0);
    expect(spectrum.chiRMag!.length).toBe(spectrum.r!.length);
  });

  it('should perform reverse FT to get chi(q)', () => {
    // Skip test if file doesn't exist
    if (!fs.existsSync(SAMPLE_FILE)) {
      console.warn(`Test file not found: ${SAMPLE_FILE}`);
      return;
    }
    
    const spectrum = XASSpectrum.fromFile(SAMPLE_FILE);
    spectrum.findE0();
    spectrum.normalize();
    spectrum.calcBackground();
    spectrum.fft();
    spectrum.ifft();
    
    expect(spectrum.q).not.toBeNull();
    expect(spectrum.chiQ).not.toBeNull();
    expect(spectrum.q!.length).toBeGreaterThan(0);
    expect(spectrum.chiQ!.length).toBe(spectrum.q!.length);
  });
});