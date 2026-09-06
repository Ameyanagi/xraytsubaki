export function validate(energy, mu) {
  if (!(energy instanceof Float64Array) || !(mu instanceof Float64Array)) {
    throw new TypeError("energy and mu must be Float64Array instances");
  }
}
