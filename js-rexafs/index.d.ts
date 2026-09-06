/** Initialize Wasm. Node loads its local Wasm automatically; init is a no-op there. */
export default function init(wasm?: URL | Request | Response | BufferSource | WebAssembly.Module): Promise<void>;
export { process, type ProcessOptions, type ProcessedSpectrum } from "./types.js";
