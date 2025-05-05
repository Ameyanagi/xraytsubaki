import { findE0, preEdge, autobk, xftf, xftr } from '../src';
import * as path from 'path';
import * as fs from 'fs';

describe('XAFS Functions', () => {
  // Test constants
  const TEST_DATA_DIR = path.join(__dirname, '../../crates/xraytsubaki/tests/testfiles');
  const SAMPLE_FILE = path.join(TEST_DATA_DIR, 'Ru_QAS.dat');
  
  // Helper function to load test data
  function loadTestData(): { energy: Float64Array, mu: Float64Array } {
    // This is a simplified version for the test
    // In a real implementation, we would read the file directly
    
    // Generate dummy data if the test file doesn't exist
    if (!fs.existsSync(SAMPLE_FILE)) {
      console.warn(`Test file not found: ${SAMPLE_FILE}`);
      
      const energy = new Float64Array(1000);
      const mu = new Float64Array(1000);
      
      for (let i = 0; i < 1000; i++) {
        energy[i] = 21900 + i * 0.5;
        mu[i] = Math.exp(-(energy[i] - 22117) * (energy[i] - 22117) / 200) + 0.1 * Math.random();
      }
      
      return { energy, mu };
    }
    
    // Read the data from a file
    // This would be implemented based on the actual file format
    // For this test, we're just going to create a simple mock
    
    const data = fs.readFileSync(SAMPLE_FILE, 'utf-8');
    const lines = data.split('\n');
    
    const energy: number[] = [];
    const mu: number[] = [];
    
    for (const line of lines) {
      const trimmed = line.trim();
      if (trimmed === '' || trimmed.startsWith('#')) continue;
      
      const parts = trimmed.split(/\s+/);
      if (parts.length >= 2) {
        energy.push(parseFloat(parts[0]));
        mu.push(parseFloat(parts[1]));
      }
    }
    
    return {
      energy: new Float64Array(energy),
      mu: new Float64Array(mu)
    };
  }
  
  it('should find e0', () => {
    const { energy, mu } = loadTestData();
    
    const e0 = findE0(energy, mu);
    
    // The expected e0 value for the Ru_QAS sample is around 22117
    expect(e0).toBeGreaterThan(22000);
    expect(e0).toBeLessThan(22200);
  });

  it('should perform pre-edge normalization', () => {
    const { energy, mu } = loadTestData();
    const e0 = findE0(energy, mu);
    
    const result = preEdge(energy, mu, {
      e0,
      preEdgeRange: [-150, -30],
      postEdgeRange: [100, 400],
      normalize: true
    });
    
    expect(result).toBeDefined();
    expect(result.pre).toBeDefined();
    expect(result.post).toBeDefined();
    expect(result.norm).toBeDefined();
    expect(result.edge_step).toBeGreaterThan(0);
    expect(result.pre.length).toBe(energy.length);
    expect(result.post.length).toBe(energy.length);
    expect(result.norm.length).toBe(energy.length);
    
    // Check that normalized values are generally between 0 and ~1.2
    const maxNorm = Math.max(...result.norm);
    expect(maxNorm).toBeGreaterThan(0.9);
    expect(maxNorm).toBeLessThan(1.5);
  });

  it('should perform autobk background subtraction', () => {
    const { energy, mu } = loadTestData();
    const e0 = findE0(energy, mu);
    
    const preEdgeResult = preEdge(energy, mu, {
      e0,
      preEdgeRange: [-150, -30],
      postEdgeRange: [100, 400],
      normalize: true
    });
    
    const result = autobk(energy, preEdgeResult.norm, {
      e0,
      rbkg: 1.0,
      kweight: 2,
      kmin: 0,
      kmax: 15
    });
    
    expect(result).toBeDefined();
    expect(result.k).toBeDefined();
    expect(result.chi).toBeDefined();
    expect(result.kmin).toBeCloseTo(0, 1);
    expect(result.kmax).toBeCloseTo(15, 1);
    expect(result.k.length).toBeGreaterThan(0);
    expect(result.chi.length).toBe(result.k.length);
  });

  it('should perform forward Fourier transform', () => {
    const { energy, mu } = loadTestData();
    const e0 = findE0(energy, mu);
    
    const preEdgeResult = preEdge(energy, mu, {
      e0,
      preEdgeRange: [-150, -30],
      postEdgeRange: [100, 400],
      normalize: true
    });
    
    const autobkResult = autobk(energy, preEdgeResult.norm, {
      e0,
      rbkg: 1.0,
      kweight: 2,
      kmin: 0,
      kmax: 15
    });
    
    const result = xftf(autobkResult.k, autobkResult.chi, {
      kmin: 2,
      kmax: 12,
      dk: 2,
      window: 'Hanning',
      kweight: 2
    });
    
    expect(result).toBeDefined();
    expect(result.r).toBeDefined();
    expect(result.chir).toBeDefined();
    expect(result.chir_mag).toBeDefined();
    expect(result.chir_re).toBeDefined();
    expect(result.chir_im).toBeDefined();
    expect(result.r.length).toBeGreaterThan(0);
    expect(result.chir_mag.length).toBe(result.r.length);
    expect(result.chir_re.length).toBe(result.r.length);
    expect(result.chir_im.length).toBe(result.r.length);
  });

  it('should perform reverse Fourier transform', () => {
    const { energy, mu } = loadTestData();
    const e0 = findE0(energy, mu);
    
    const preEdgeResult = preEdge(energy, mu, {
      e0,
      preEdgeRange: [-150, -30],
      postEdgeRange: [100, 400],
      normalize: true
    });
    
    const autobkResult = autobk(energy, preEdgeResult.norm, {
      e0,
      rbkg: 1.0,
      kweight: 2,
      kmin: 0,
      kmax: 15
    });
    
    const xftfResult = xftf(autobkResult.k, autobkResult.chi, {
      kmin: 2,
      kmax: 12,
      dk: 2,
      window: 'Hanning',
      kweight: 2
    });
    
    const result = xftr(xftfResult.r, xftfResult.chir, {
      rmin: 1.0,
      rmax: 3.0,
      dr: 0.1,
      window: 'Hanning'
    });
    
    expect(result).toBeDefined();
    expect(result.q).toBeDefined();
    expect(result.chiq).toBeDefined();
    expect(result.q.length).toBeGreaterThan(0);
    expect(result.chiq.length).toBe(result.q.length);
  });
});