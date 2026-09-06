import { validate } from "./validate.js";

// Only ownership and fluent return values are adapted here. Rust runs every stage.
export function bindSpectrum(core, ready = () => true) {
  return class Spectrum {
    #inner;
    constructor(energy, mu) {
      if (!ready()) throw new Error("Call await init() before creating a spectrum");
      validate(energy, mu);
      this.#inner = core.Spectrum.from_arrays(energy, mu);
    }
    static from_arrays(energy, mu) { return new this(energy, mu); }
    free() { this.#inner.free(); }
    set_spectrum(energy, mu) { validate(energy, mu); this.#inner.set_spectrum(energy, mu); return this; }
    set_e0(e0) {
      if (typeof e0 !== "number") throw new TypeError("e0 must be a number in eV");
      if (!Number.isFinite(e0)) throw new RangeError("e0 must be finite in eV");
      this.#inner.set_e0(e0);
      return this;
    }
    set_normalization_method(method) {
      const selected = method ?? core.NormalizationMethod.new_prepostedge();
      try { this.#inner.set_normalization_method(selected); }
      finally { if (method == null) selected.free(); }
      return this;
    }
    set_background_method(method) {
      const selected = method ?? core.BackgroundMethod.new_autobk();
      try { this.#inner.set_background_method(selected); }
      finally { if (method == null) selected.free(); }
      return this;
    }
    set_fft(parameters) { this.#inner.set_fft(parameters); return this; }
    invalidate_derived() { this.#inner.invalidate_derived(); return this; }
    find_e0() { this.#inner.find_e0(); return this; }
    normalize() { this.#inner.normalize(); return this; }
    calc_background() { this.#inner.calc_background(); return this; }
    fft() { this.#inner.fft(); return this; }
    ifft() { this.#inner.ifft(); return this; }
    e0() { return this.#inner.e0(); }
    k() { return this.#inner.k(); }
    chi() { return this.#inner.chi(); }
    norm() { return this.#inner.norm(); }
    flat() { return this.#inner.flat(); }
    pre_edge() { return this.#inner.pre_edge(); }
    post_edge() { return this.#inner.post_edge(); }
    r() { return this.#inner.r(); }
    chir_mag() { return this.#inner.chir_mag(); }
    chir_real() { return this.#inner.chir_real(); }
    chir_imag() { return this.#inner.chir_imag(); }
    q() { return this.#inner.q(); }
    chiq() { return this.#inner.chiq(); }
  };
}
