import { 
  XASSpectrum,
  FittingParameter, 
  FittingParameters, 
  SimplePath, 
  FittingDataset, 
  ExafsFitter,
  findE0, 
  preEdge, 
  autobk, 
  xftf 
} from '../src';
import * as path from 'path';
import * as fs from 'fs';

describe('EXAFS Fitting', () => {
  // Test constants
  const TEST_DATA_DIR = path.join(__dirname, '../../crates/xraytsubaki/tests/testfiles');
  const SYNTHETIC_SPECTRUM = path.join(TEST_DATA_DIR, 'fit_results/synthetic_spectrum_1.dat');
  
  function loadOrCreateTestData(): { k: Float64Array, chi: Float64Array } {
    // Load synthetic spectrum if it exists
    if (fs.existsSync(SYNTHETIC_SPECTRUM)) {
      const data = fs.readFileSync(SYNTHETIC_SPECTRUM, 'utf-8');
      const lines = data.split('\n');
      
      const k: number[] = [];
      const chi: number[] = [];
      
      for (const line of lines) {
        const trimmed = line.trim();
        if (trimmed === '' || trimmed.startsWith('#')) continue;
        
        const parts = trimmed.split(/\s+/);
        if (parts.length >= 2) {
          k.push(parseFloat(parts[0]));
          chi.push(parseFloat(parts[1]));
        }
      }
      
      return {
        k: new Float64Array(k),
        chi: new Float64Array(chi)
      };
    }
    
    // Create synthetic EXAFS data
    // This is a simplified version with just one path for testing
    const k = new Float64Array(100);
    const chi = new Float64Array(100);
    
    for (let i = 0; i < 100; i++) {
      k[i] = 0.05 * (i + 1);
      
      // Simple EXAFS model with one path
      // χ(k) = S₀² * N * exp(-2σ² * k²) * exp(-2R/λ(k)) * sin(2kR + φ(k)) / (kR²)
      const s02 = 0.9;
      const n = 6;
      const sigma2 = 0.003;
      const r = 2.5;
      const e0 = 0;
      const amplitude = s02 * n * Math.exp(-2 * sigma2 * k[i] * k[i]) / (k[i] * r * r);
      const phase = 2 * k[i] * r + 0.1 * k[i];
      
      chi[i] = amplitude * Math.sin(phase);
    }
    
    return { k, chi };
  }
  
  it('should create fitting parameters', () => {
    const amp = new FittingParameter('amp', 1.0);
    expect(amp.name).toBe('amp');
    expect(amp.value).toBe(1.0);
    expect(amp.min).toBeNull();
    expect(amp.max).toBeNull();
    expect(amp.vary).toBe(true);
    
    amp.setValue(0.9);
    expect(amp.value).toBe(0.9);
    
    amp.setMin(0.5);
    expect(amp.min).toBe(0.5);
    
    amp.setMax(1.5);
    expect(amp.max).toBe(1.5);
    
    amp.setVary(false);
    expect(amp.vary).toBe(false);
  });

  it('should create a set of fitting parameters', () => {
    const params = new FittingParameters();
    
    params.add('amp', 1.0);
    params.add('delr', 0.0, -0.5, 0.5);
    params.add('sigma2', 0.003, 0.0, 0.02);
    params.add('e0', 0.0, -10.0, 10.0);
    
    expect(params.size()).toBe(4);
    expect(params.get('amp')?.value).toBe(1.0);
    expect(params.get('delr')?.value).toBe(0.0);
    expect(params.get('sigma2')?.value).toBe(0.003);
    expect(params.get('e0')?.value).toBe(0.0);
    
    expect(params.get('delr')?.min).toBe(-0.5);
    expect(params.get('delr')?.max).toBe(0.5);
    
    params.set('amp', 0.9);
    expect(params.get('amp')?.value).toBe(0.9);
  });

  it('should create a simple path model', () => {
    const path = new SimplePath('path1', 6, 2.5);
    
    expect(path.getLabel()).toBe('path1');
    expect(path.getN()).toBe(6);
    expect(path.getR()).toBe(2.5);
    
    path.setS02('amp');
    path.setE0('e0');
    path.setSigma2('sigma2');
    path.setDelr('delr');
    
    expect(path.getS02Param()).toBe('amp');
    expect(path.getE0Param()).toBe('e0');
    expect(path.getSigma2Param()).toBe('sigma2');
    expect(path.getDelrParam()).toBe('delr');
  });

  it('should create a fitting dataset', () => {
    const { k, chi } = loadOrCreateTestData();
    
    const dataset = new FittingDataset(k, chi);
    dataset.setKRange(2.0, 12.0);
    dataset.setKWeight(2);
    
    expect(dataset.getK()).toBe(k);
    expect(dataset.getChi()).toBe(chi);
    expect(dataset.getKMin()).toBe(2.0);
    expect(dataset.getKMax()).toBe(12.0);
    expect(dataset.getKWeight()).toBe(2);
  });

  it('should fit a simple EXAFS model', () => {
    const { k, chi } = loadOrCreateTestData();
    
    // Create dataset
    const dataset = new FittingDataset(k, chi);
    dataset.setKRange(2.0, 12.0);
    dataset.setKWeight(2);
    
    // Create parameters
    const params = new FittingParameters();
    params.add('amp', 0.8, 0.5, 1.5);
    params.add('delr', 0.0, -0.5, 0.5);
    params.add('sigma2', 0.004, 0.0005, 0.02);
    params.add('e0', 0.0, -10.0, 10.0);
    
    // Create path
    const path = new SimplePath('path1', 6, 2.5);
    path.setS02('amp');
    path.setE0('e0');
    path.setSigma2('sigma2');
    path.setDelr('delr');
    
    // Create fitter
    const fitter = new ExafsFitter();
    fitter.addPath(path);
    
    // Perform the fit
    const result = fitter.fit(dataset, params);
    
    // Check results
    expect(result).toBeDefined();
    expect(result.success).toBe(true);
    expect(result.nfev).toBeGreaterThan(0);
    expect(result.message).toBeDefined();
    
    // Check parameters
    expect(result.params.get('amp')?.value).toBeCloseTo(0.9, 1);  // Expected value is around 0.9
    expect(result.params.get('sigma2')?.value).toBeCloseTo(0.003, 3);  // Expected value is around 0.003
    
    // Check fit quality
    expect(result.redchi).toBeDefined();
    expect(result.redchi).toBeLessThan(10);  // A reasonable fit should have a relatively low reduced chi-squared
    
    // Check the fit result contains the calculated chi
    expect(result.best_fit).toBeDefined();
    expect(result.best_fit.length).toBe(chi.length);
  });
});