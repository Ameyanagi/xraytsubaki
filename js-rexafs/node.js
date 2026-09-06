import core from "./dist/node/rexafs_wasm.js";
import { bindSpectrum } from "./spectrum.js";
export default async function init() {}
export const Spectrum = bindSpectrum(core);
export const { PrePostEdge, AUTOBK, XrayFFTF, NormalizationMethod, BackgroundMethod } = core;
