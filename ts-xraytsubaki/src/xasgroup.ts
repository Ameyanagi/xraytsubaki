/**
 * XASGroup class for managing multiple XAS spectra
 */

import { lib } from './ffi/bindings_mock';
import { XASSpectrum } from './xasspectrum';

/**
 * Represents a collection of XAS spectra
 */
export class XASGroup {
  // Internal FFI handle
  private handle: any;
  
  // Cached spectra
  private spectra: XASSpectrum[] = [];
  
  /**
   * Create a new XAS group
   */
  constructor() {
    this.handle = lib.xas_group_new();
  }
  
  /**
   * Clean up resources when object is garbage collected
   */
  destroy(): void {
    if (this.handle) {
      lib.xas_group_free(this.handle);
      this.handle = null;
    }
  }
  
  /**
   * Add a spectrum to the group
   * 
   * @param spectrum The XASSpectrum to add
   */
  addSpectrum(spectrum: XASSpectrum): void {
    lib.xas_group_add_spectrum(this.handle, spectrum['handle']);
    this.spectra.push(spectrum);
  }
  
  /**
   * Add multiple spectra to the group
   * 
   * @param spectra Array of XASSpectrum objects to add
   */
  addSpectra(spectra: XASSpectrum[]): void {
    for (const spectrum of spectra) {
      this.addSpectrum(spectrum);
    }
  }
  
  /**
   * Add all spectra from another group to this group
   * 
   * @param group The XASGroup to add
   */
  addGroup(group: XASGroup): void {
    lib.xas_group_add_group(this.handle, group.handle);
    
    // Update cached spectra
    for (const spectrum of group.getAllSpectra()) {
      this.spectra.push(spectrum);
    }
  }
  
  /**
   * Get the number of spectra in the group
   * 
   * @returns The number of spectra
   */
  length(): number {
    return lib.xas_group_length(this.handle);
  }
  
  /**
   * Check if the group is empty
   * 
   * @returns True if the group is empty, false otherwise
   */
  isEmpty(): boolean {
    return this.length() === 0;
  }
  
  /**
   * Get a spectrum from the group by index
   * 
   * @param index The index of the spectrum to get
   * @returns The XASSpectrum at the given index, or null if not found
   */
  getSpectrum(index: number): XASSpectrum | null {
    if (index < 0 || index >= this.length()) {
      return null;
    }
    
    // Use cached spectrum if available
    if (index < this.spectra.length) {
      return this.spectra[index];
    }
    
    // Otherwise get from FFI
    const handle = lib.xas_group_get_spectrum(this.handle, index);
    
    if (!handle) {
      return null;
    }
    
    const spectrum = new XASSpectrum();
    spectrum['handle'] = handle;
    
    // Cache for future use
    this.spectra[index] = spectrum;
    
    return spectrum;
  }
  
  /**
   * Get all spectra in the group
   * 
   * @returns Array of all XASSpectrum objects in the group
   */
  getAllSpectra(): XASSpectrum[] {
    const count = this.length();
    const result: XASSpectrum[] = [];
    
    for (let i = 0; i < count; i++) {
      const spectrum = this.getSpectrum(i);
      if (spectrum) {
        result.push(spectrum);
      }
    }
    
    return result;
  }
  
  /**
   * Remove a spectrum from the group by index
   * 
   * @param index The index of the spectrum to remove
   * @returns True if successful, false otherwise
   */
  removeSpectrum(index: number): boolean {
    const result = lib.xas_group_remove_spectrum(this.handle, index);
    
    if (result) {
      // Update cached spectra
      this.spectra.splice(index, 1);
    }
    
    return result;
  }
  
  /**
   * Remove multiple spectra from the group by indices
   * 
   * @param indices Array of indices to remove
   * @returns True if successful, false otherwise
   */
  removeSpectra(indices: number[]): boolean {
    // Sort indices in descending order to avoid index shifting problems
    const sortedIndices = [...indices].sort((a, b) => b - a);
    
    let success = true;
    for (const index of sortedIndices) {
      if (!this.removeSpectrum(index)) {
        success = false;
      }
    }
    
    return success;
  }
  
  /**
   * Find the edge energy (E0) for all spectra in the group
   */
  findE0(): void {
    lib.xas_group_find_e0(this.handle);
    
    // Update cached spectra
    for (const spectrum of this.spectra) {
      // Force refresh of e0
      spectrum['_e0'] = null;
    }
  }
  
  /**
   * Normalize all spectra in the group
   */
  normalize(): void {
    lib.xas_group_normalize(this.handle);
  }
  
  /**
   * Calculate background and extract EXAFS chi(k) for all spectra in the group
   */
  calcBackground(): void {
    lib.xas_group_calc_background(this.handle);
    
    // Update cached spectra
    for (const spectrum of this.spectra) {
      // Force refresh of k and chi arrays
      spectrum['_k'] = null;
      spectrum['_chi'] = null;
    }
  }
  
  /**
   * Perform forward Fourier transform for all spectra in the group
   */
  fft(): void {
    lib.xas_group_fft(this.handle);
    
    // Update cached spectra
    for (const spectrum of this.spectra) {
      // Force refresh of r and chi(r) arrays
      spectrum['_r'] = null;
      spectrum['_chiRMag'] = null;
      spectrum['_chiRRe'] = null;
      spectrum['_chiRIm'] = null;
    }
  }
  
  /**
   * Perform inverse Fourier transform for all spectra in the group
   */
  ifft(): void {
    lib.xas_group_ifft(this.handle);
    
    // Update cached spectra
    for (const spectrum of this.spectra) {
      // Force refresh of q and chi(q) arrays
      spectrum['_q'] = null;
      spectrum['_chiQ'] = null;
    }
  }
  
  /**
   * Save the group to a JSON file
   * 
   * @param filePath Path to save the file
   * @returns True if successful, false otherwise
   */
  saveJSON(filePath: string): boolean {
    return lib.xas_group_save_json(this.handle, filePath);
  }
  
  /**
   * Load a group from a JSON file
   * 
   * @param filePath Path to the file
   * @returns A new XASGroup instance
   * @static
   */
  static fromJSON(filePath: string): XASGroup {
    const handle = lib.xas_group_load_json(filePath);
    
    if (!handle) {
      throw new Error(`Failed to load group from file: ${filePath}`);
    }
    
    const group = new XASGroup();
    group.handle = handle;
    
    // Refresh cached spectra count
    const count = lib.xas_group_length(handle);
    for (let i = 0; i < count; i++) {
      group.getSpectrum(i);
    }
    
    return group;
  }
}