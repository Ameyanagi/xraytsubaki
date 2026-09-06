import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import init, { process } from "../node.js";
import browserInit, { process as browserProcess } from "../browser.js";

const text = await readFile(new URL("../../crates/rexafs/tests/testfiles/Ru_QAS.dat", import.meta.url), "utf8");
const rows = text.split(/\r?\n/).filter(line => line.trim() && !line.trim().startsWith("#")).map(line => line.trim().split(/\s+/).map(Number));
const energy = Float64Array.from(rows, row => row[0]);
const mu = Float64Array.from(rows, row => Math.log(row[1] / row[2]));
const reference = JSON.parse(await readFile(new URL("ru-reference.json", import.meta.url), "utf8"));

function verify(output) {
  assert.equal(output.e0, reference.e0);
  assert.equal(output.k.length, reference.k_length);
  assert.equal(output.chi.length, reference.k_length);
  assert.equal(output.r.length, reference.r_length);
  for (const [key, values] of Object.entries(reference.samples)) {
    assert.ok(output[key] instanceof Float64Array);
    [0, 20, 50, 100].forEach((index, i) => {
      assert.ok(Math.abs(output[key][index] - values[i]) <= 1e-7 * Math.max(1, Math.abs(values[i])), `${key}[${index}] differs from native pipeline`);
    });
  }
  for (let i = 0; i < output.r.length; i++) {
    assert.ok(Number.isFinite(output.chir_mag[i]));
    assert.ok(Math.abs(output.chir_mag[i] - Math.hypot(output.chir_re[i], output.chir_im[i])) < 1e-10);
  }
}

test("Node pipeline agrees with native Ru fixture", async () => {
  await init();
  const saved = energy.slice();
  const output = process(energy, mu);
  verify(output);
  assert.deepEqual(energy, saved);
  assert.equal(process(energy, mu, { e0: reference.e0 + 0.25 }).e0, reference.e0 + 0.25);
  // A second calculation must not invalidate earlier result memory.
  verify(output);
});

test("invalid inputs throw JS errors rather than trapping Wasm", () => {
  assert.throws(() => process([1, 2], [1, 2]), TypeError);
  for (const [e, m] of [[[], []], [[2, 1, 3], [1]], [[1, 2, 2], [1, 2, 3]], [[1, NaN, 3], [1, 2, 3]]]) {
    assert.throws(() => process(Float64Array.from(e), Float64Array.from(m)), Error);
  }
  assert.throws(() => process(energy, mu, { e0: NaN }), TypeError);
  assert.throws(() => process(energy, mu, { typo: 2 }), TypeError);
  verify(process(energy, mu));
});

test("browser glue initializes from bytes and matches Node", async () => {
  assert.throws(() => browserProcess(energy, mu), /init/);
  const bytes = await readFile(new URL("../dist/web/rexafs_wasm_bg.wasm", import.meta.url));
  await browserInit(bytes);
  verify(browserProcess(energy, mu));
});
