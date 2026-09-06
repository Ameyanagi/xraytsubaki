// Verify the public package import from an isolated consumer, after npm pack.
import { mkdtempSync, writeFileSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
const root = fileURLToPath(new URL("../", import.meta.url));
const archive = resolve(process.argv[2]);
const consumer = mkdtempSync(join(tmpdir(), "rexafs-consumer-"));
function run(command, args) {
  const child = spawnSync(command, args, { cwd: consumer, stdio: "inherit", shell: process.platform === "win32" });
  if (child.error) throw child.error;
  if (child.status !== 0) throw Error(`${command} failed (${child.status})`);
}
try {
  writeFileSync(join(consumer, "package.json"), '{"private":true,"type":"module"}');
  run("npm", ["install", "--ignore-scripts", "--no-audit", "--no-fund", archive]);
  const rows = readFileSync(join(root, "crates/rexafs/tests/testfiles/Ru_QAS.dat"), "utf8")
    .split(/\r?\n/).filter(line => line.trim() && !line.trim().startsWith("#")).map(line => line.trim().split(/\s+/).map(Number));
  writeFileSync(join(consumer, "input.json"), JSON.stringify(rows));
  writeFileSync(join(consumer, "test.mjs"), `import init, { Spectrum } from "rexafs";
import { readFileSync } from "node:fs";
import assert from "node:assert/strict";
const rows=JSON.parse(readFileSync("input.json","utf8"));
await init();
const result=Spectrum.from_arrays(Float64Array.from(rows,r=>r[0]),Float64Array.from(rows,r=>Math.log(r[1]/r[2]))).fft();
assert.equal(result.e0(),22118.8);
assert.equal(result.k().length,315);
assert.equal(result.r().length,326);
console.log("Installed npm tarball passed");\n`);
  run("node", ["test.mjs"]);
  writeFileSync(join(consumer, "test.ts"), 'import init, { Spectrum, XrayFFTF, AUTOBK, BackgroundMethod } from "rexafs";\nawait init();\nconst spectrum: Spectrum = Spectrum.from_arrays(new Float64Array(), new Float64Array());\nconst bkg = new AUTOBK(); bkg.rbkg = 1.2;\nconst ft = new XrayFFTF(); ft.kweight = 2;\nspectrum.set_background_method(BackgroundMethod.AUTOBK(bkg)).set_fft(ft).fft().chi();\n');
  run("npx", ["--yes", "--package", "typescript", "tsc", "--noEmit", "--strict", "--module", "NodeNext", "--target", "ES2022", "--lib", "ES2022", "test.ts"]);
} finally {
  rmSync(consumer, { recursive: true, force: true });
}
