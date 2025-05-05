import { XASSpectrum, XASGroup } from '../src';
import * as path from 'path';
import * as fs from 'fs';

describe('XASGroup', () => {
  // Test constants
  const TEST_DATA_DIR = path.join(__dirname, '../../crates/xraytsubaki/tests/testfiles');
  const SAMPLE_FILE = path.join(TEST_DATA_DIR, 'Ru_QAS.dat');
  
  it('should create an empty group', () => {
    const group = new XASGroup();
    expect(group).toBeDefined();
    expect(group.length()).toBe(0);
    expect(group.isEmpty()).toBe(true);
  });

  it('should add a spectrum to the group', () => {
    const group = new XASGroup();
    const spectrum = new XASSpectrum('test-spectrum');
    
    group.addSpectrum(spectrum);
    
    expect(group.length()).toBe(1);
    expect(group.isEmpty()).toBe(false);
    
    const retrieved = group.getSpectrum(0);
    expect(retrieved).toBeDefined();
    expect(retrieved?.name).toBe('test-spectrum');
  });

  it('should add multiple spectra to the group', () => {
    const group = new XASGroup();
    
    const spectrum1 = new XASSpectrum('test-spectrum-1');
    const spectrum2 = new XASSpectrum('test-spectrum-2');
    const spectrum3 = new XASSpectrum('test-spectrum-3');
    
    group.addSpectra([spectrum1, spectrum2, spectrum3]);
    
    expect(group.length()).toBe(3);
    expect(group.getSpectrum(0)?.name).toBe('test-spectrum-1');
    expect(group.getSpectrum(1)?.name).toBe('test-spectrum-2');
    expect(group.getSpectrum(2)?.name).toBe('test-spectrum-3');
  });

  it('should remove a spectrum from the group', () => {
    const group = new XASGroup();
    
    const spectrum1 = new XASSpectrum('test-spectrum-1');
    const spectrum2 = new XASSpectrum('test-spectrum-2');
    const spectrum3 = new XASSpectrum('test-spectrum-3');
    
    group.addSpectra([spectrum1, spectrum2, spectrum3]);
    
    // Remove spectrum at index 1
    group.removeSpectrum(1);
    
    expect(group.length()).toBe(2);
    expect(group.getSpectrum(0)?.name).toBe('test-spectrum-1');
    expect(group.getSpectrum(1)?.name).toBe('test-spectrum-3');
  });

  it('should remove multiple spectra from the group', () => {
    const group = new XASGroup();
    
    const spectrum1 = new XASSpectrum('test-spectrum-1');
    const spectrum2 = new XASSpectrum('test-spectrum-2');
    const spectrum3 = new XASSpectrum('test-spectrum-3');
    const spectrum4 = new XASSpectrum('test-spectrum-4');
    const spectrum5 = new XASSpectrum('test-spectrum-5');
    
    group.addSpectra([spectrum1, spectrum2, spectrum3, spectrum4, spectrum5]);
    
    // Remove spectra at indices 1 and 3
    group.removeSpectra([1, 3]);
    
    expect(group.length()).toBe(3);
    expect(group.getSpectrum(0)?.name).toBe('test-spectrum-1');
    expect(group.getSpectrum(1)?.name).toBe('test-spectrum-3');
    expect(group.getSpectrum(2)?.name).toBe('test-spectrum-5');
  });

  it('should process all spectra in the group', () => {
    // Skip test if file doesn't exist
    if (!fs.existsSync(SAMPLE_FILE)) {
      console.warn(`Test file not found: ${SAMPLE_FILE}`);
      return;
    }
    
    const group = new XASGroup();
    
    // Create three copies of the same spectrum
    const spectrum1 = XASSpectrum.fromFile(SAMPLE_FILE);
    spectrum1.name = 'sample-1';
    
    const spectrum2 = XASSpectrum.fromFile(SAMPLE_FILE);
    spectrum2.name = 'sample-2';
    
    const spectrum3 = XASSpectrum.fromFile(SAMPLE_FILE);
    spectrum3.name = 'sample-3';
    
    group.addSpectra([spectrum1, spectrum2, spectrum3]);
    
    // Process all spectra in the group
    group.findE0();
    group.normalize();
    group.calcBackground();
    group.fft();
    
    // Check that all spectra have been processed
    for (let i = 0; i < group.length(); i++) {
      const spectrum = group.getSpectrum(i);
      expect(spectrum?.e0).not.toBeNull();
      expect(spectrum?.k).not.toBeNull();
      expect(spectrum?.chi).not.toBeNull();
      expect(spectrum?.r).not.toBeNull();
      expect(spectrum?.chiRMag).not.toBeNull();
    }
  });

  it('should merge two groups', () => {
    const group1 = new XASGroup();
    const group2 = new XASGroup();
    
    const spectrum1 = new XASSpectrum('group1-spectrum-1');
    const spectrum2 = new XASSpectrum('group1-spectrum-2');
    
    const spectrum3 = new XASSpectrum('group2-spectrum-1');
    const spectrum4 = new XASSpectrum('group2-spectrum-2');
    
    group1.addSpectra([spectrum1, spectrum2]);
    group2.addSpectra([spectrum3, spectrum4]);
    
    group1.addGroup(group2);
    
    expect(group1.length()).toBe(4);
    expect(group1.getSpectrum(0)?.name).toBe('group1-spectrum-1');
    expect(group1.getSpectrum(1)?.name).toBe('group1-spectrum-2');
    expect(group1.getSpectrum(2)?.name).toBe('group2-spectrum-1');
    expect(group1.getSpectrum(3)?.name).toBe('group2-spectrum-2');
  });

  it('should save and load group data', () => {
    // Skip test if file doesn't exist
    if (!fs.existsSync(SAMPLE_FILE)) {
      console.warn(`Test file not found: ${SAMPLE_FILE}`);
      return;
    }
    
    const group = new XASGroup();
    
    // Create a spectrum and process it
    const spectrum = XASSpectrum.fromFile(SAMPLE_FILE);
    spectrum.name = 'test-export';
    spectrum.findE0();
    spectrum.normalize();
    spectrum.calcBackground();
    spectrum.fft();
    
    group.addSpectrum(spectrum);
    
    // Save to a temporary file
    const tempFile = path.join(__dirname, 'temp-group.json');
    group.saveJSON(tempFile);
    
    // Load from the temporary file
    const loadedGroup = XASGroup.fromJSON(tempFile);
    
    expect(loadedGroup.length()).toBe(1);
    
    const loadedSpectrum = loadedGroup.getSpectrum(0);
    expect(loadedSpectrum?.name).toBe('test-export');
    expect(loadedSpectrum?.e0).toBeCloseTo(spectrum.e0!, 6);
    expect(loadedSpectrum?.k).not.toBeNull();
    expect(loadedSpectrum?.chi).not.toBeNull();
    
    // Clean up
    if (fs.existsSync(tempFile)) {
      fs.unlinkSync(tempFile);
    }
  });
});