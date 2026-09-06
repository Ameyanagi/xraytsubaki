import initialize, * as core from "./dist/web/rexafs_wasm.js";
import { bindSpectrum } from "./spectrum.js";
let ready = false;
export default async function init(wasm) {
  await initialize(wasm === undefined ? undefined : { module_or_path: wasm });
  ready = true;
}
export const Spectrum = bindSpectrum(core, () => ready);
export const { PrePostEdge, AUTOBK, XrayFFTF, NormalizationMethod, BackgroundMethod } = core;
