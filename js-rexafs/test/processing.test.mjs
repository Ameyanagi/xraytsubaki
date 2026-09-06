import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import init, * as api from "../node.js";
import browserInit, { Spectrum as BrowserSpectrum } from "../browser.js";
const { Spectrum, PrePostEdge, AUTOBK, XrayFFTF, NormalizationMethod, BackgroundMethod } = api;
const text = await readFile(new URL("../../crates/rexafs/tests/testfiles/Ru_QAS.dat", import.meta.url), "utf8");
const rows = text.split(/\r?\n/).filter(line => line.trim() && !line.trim().startsWith("#")).map(line => line.trim().split(/\s+/).map(Number));
const energy = Float64Array.from(rows, row => row[0]);
const mu = Float64Array.from(rows, row => Math.log(row[1] / row[2]));
const reference = JSON.parse(await readFile(new URL("ru-reference.json", import.meta.url), "utf8"));
const getters = { k: "k", chi: "chi", r: "r", chir_mag: "chir_mag", chir_re: "chir_real", chir_im: "chir_imag" };
function verify(spectrum) {
  assert.equal(spectrum.e0(), reference.e0);
  assert.equal(spectrum.k().length, reference.k_length);
  assert.equal(spectrum.r().length, reference.r_length);
  for (const [key, values] of Object.entries(reference.samples)) {
    const output = spectrum[getters[key]]();
    assert.ok(output instanceof Float64Array);
    [0, 20, 50, 100].forEach((index, i) => assert.ok(Math.abs(output[index] - values[i]) <= 1e-7 * Math.max(1, Math.abs(values[i])), `${key}[${index}] differs from native pipeline`));
  }
}

test("terminal stage matches native fixture and explicit chain", async () => {
  await init();
  const saved = energy.slice();
  const spectrum = Spectrum.from_arrays(energy, mu);
  assert.equal(spectrum.chi(), undefined);
  assert.equal(spectrum.fft(), spectrum);
  verify(spectrum);
  const explicit = new Spectrum(energy, mu).find_e0().normalize().calc_background().fft();
  verify(explicit);
  assert.deepEqual(energy, saved);
  const copy = spectrum.chi();
  copy.fill(0);
  verify(spectrum);
  spectrum.free(); explicit.free();
  assert.equal("process" in api, false);
});

test("parameters, stage invalidation, and selected methods", () => {
  const norm = new PrePostEdge(); norm.pre_edge_start = -200; norm.pre_edge_end = -65;
  const bkg = new AUTOBK(); bkg.rbkg = 1.2;
  const n = NormalizationMethod.PrePostEdge(norm), b = BackgroundMethod.AUTOBK(bkg);
  const ft = new XrayFFTF(); ft.kweight = 1;
  const s = new Spectrum(energy, mu).set_normalization_method(n).set_background_method(b).set_fft(ft).fft();
  const chi = s.chi(), r = s.chir_mag();
  ft.kweight = 3;
  s.set_fft(ft);
  assert.equal(s.r(), undefined);
  s.fft();
  assert.deepEqual(s.chi(), chi);
  assert.notDeepEqual(s.chir_mag(), r);
  s.set_e0(reference.e0 + 0.25);
  assert.equal(s.chi(), undefined); assert.equal(s.norm(), undefined);
  assert.equal(s.fft().e0(), reference.e0 + 0.25);
  const unavailable = NormalizationMethod.new_mback();
  assert.throws(() => s.set_normalization_method(unavailable).fft(), /MBack/);
  const unsupported = BackgroundMethod.new_ilpbkg();
  assert.throws(() => s.set_normalization_method().set_background_method(unsupported).fft(), /ILPBkg/);
  assert.equal(s.r(), undefined);
  s.set_background_method().fft();
  for (const value of [s,n,b,norm,bkg,ft,unavailable,unsupported]) value.free();
});

test("invalid inputs and FFT settings throw errors and allow recovery", () => {
  assert.throws(() => new Spectrum([1, 2], [1, 2]), TypeError);
  for (const [e,m] of [[[],[]],[[2,1,3],[1]],[[1,2,2],[1,2,3]],[[1,NaN,3],[1,2,3]]]) {
    assert.throws(() => new Spectrum(Float64Array.from(e), Float64Array.from(m)), Error);
  }
  const s = new Spectrum(energy, mu);
  s.set_e0(reference.e0).fft();
  const savedMagnitude = s.chir_mag();
  for (const e0 of [NaN, Infinity, -Infinity]) {
    assert.throws(() => s.set_e0(e0), RangeError);
    assert.equal(s.e0(), reference.e0);
    assert.deepEqual(s.chir_mag(), savedMagnitude);
  }
  const ft = new XrayFFTF(); ft.nfft = 0;
  assert.throws(() => s.set_fft(ft).fft(), /nfft/);
  assert.equal(s.r(), undefined);
  ft.nfft = 2048;
  verify(s.set_fft(ft).fft());
  s.set_spectrum(energy, mu);
  assert.equal(s.e0(), undefined); assert.equal(s.chi(), undefined);
  verify(s.fft());
  s.free(); ft.free();
});

test("browser glue initializes from bytes and matches Node", async () => {
  assert.throws(() => new BrowserSpectrum(energy, mu), /init/);
  await browserInit(await readFile(new URL("../dist/web/rexafs_wasm_bg.wasm", import.meta.url)));
  const spectrum = BrowserSpectrum.from_arrays(energy, mu).fft();
  verify(spectrum); spectrum.free();
});

test("fixed lambda matches the independent Larch-model reference", async () => {
  const cases = JSON.parse(await readFile(new URL("../../crates/rexafs/tests/testfiles/autobk_fixed_reference.json", import.meta.url), "utf8"));
  for (const data of cases) {
    const norm = new PrePostEdge(); norm.e0 = data.settings.ek0; norm.edge_step = data.edge_step;
    const bkg = new AUTOBK();
    assert.equal(bkg.clamp_scale_policy, "FixedPenalty");
    assert.equal(bkg.clamp_lambda, 0.001);
    for (const [key, value] of Object.entries(data.settings)) bkg[key] = value;
    const n = NormalizationMethod.PrePostEdge(norm);
    for (const [index, lambda] of [0, 0.001, 1].entries()) {
      bkg.clamp_lambda = lambda;
      const b = BackgroundMethod.AUTOBK(bkg);
      const s = new Spectrum(Float64Array.from(data.energy), Float64Array.from(data.mu))
        .set_e0(data.settings.ek0).set_normalization_method(n).set_background_method(b).calc_background();
      const actual = s.chi(), expected = data.chi[index];
      assert.equal(actual.length, expected.length);
      // Same fixed objective; absolute floor protects comparisons at zero crossings.
      actual.forEach((value, i) => assert.ok(Math.abs(value - expected[i]) <= 1e-12 + 1e-11 * Math.abs(expected[i]), `${data.name} lambda=${lambda} chi[${i}]`));
      s.free(); b.free();
    }
    norm.free(); n.free(); bkg.free();
  }
});

test("legacy clamp mode retains the earlier native reference", async () => {
  const previous = JSON.parse(await readFile(new URL("ru-reference-0.1.0.json", import.meta.url), "utf8"));
  const bkg = new AUTOBK(); bkg.clamp_scale_policy = "Fixed";
  const b = BackgroundMethod.AUTOBK(bkg);
  const s = new Spectrum(energy, mu).set_background_method(b).fft();
  for (const [key, values] of Object.entries(previous.samples)) {
    const actual = s[getters[key]]();
    [0, 20, 50, 100].forEach((j, i) => assert.ok(Math.abs(actual[j] - values[i]) < 1e-7, `${key}[${j}]`));
  }
  s.free(); b.free(); bkg.free();
});
