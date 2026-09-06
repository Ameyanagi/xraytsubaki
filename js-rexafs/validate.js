export function validate(energy, mu, options) {
  if (!(energy instanceof Float64Array) || !(mu instanceof Float64Array)) {
    throw new TypeError("energy and mu must be Float64Array instances");
  }
  if (options == null || typeof options !== "object" || Array.isArray(options)) {
    throw new TypeError("options must be an object");
  }
  for (const key of Object.keys(options)) {
    if (key !== "e0") throw new TypeError(`unknown processing option: ${key}`);
  }
  if (options.e0 !== undefined && (typeof options.e0 !== "number" || !Number.isFinite(options.e0))) {
    throw new TypeError("e0 must be a finite number in eV");
  }
}
