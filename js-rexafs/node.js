import core from "./dist/node/rexafs_wasm.js";
import { validate } from "./validate.js";
export default async function init() {}
export function process(energy, mu, options = {}) {
  validate(energy, mu, options);
  return core.process(energy, mu, options.e0);
}
