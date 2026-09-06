import initialize, { process as coreProcess } from "./dist/web/rexafs_wasm.js";
import { validate } from "./validate.js";
let ready = false;
export default async function init(wasm) {
  await initialize(wasm === undefined ? undefined : { module_or_path: wasm });
  ready = true;
}
export function process(energy, mu, options = {}) {
  if (!ready) throw new Error("Call await init() before processing a spectrum");
  validate(energy, mu, options);
  return coreProcess(energy, mu, options.e0);
}
