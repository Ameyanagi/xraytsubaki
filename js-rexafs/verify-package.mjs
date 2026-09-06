import { accessSync } from "node:fs";
for (const file of ["dist/web/rexafs_wasm.js", "dist/web/rexafs_wasm_bg.wasm", "dist/node/rexafs_wasm.js", "dist/node/rexafs_wasm_bg.wasm", "LICENSE-MIT", "LICENSE-APACHE"]) {
  accessSync(new URL(file, import.meta.url));
}
