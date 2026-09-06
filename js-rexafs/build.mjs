import { spawnSync } from "node:child_process";
import { copyFileSync, rmSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
const root = fileURLToPath(new URL("../", import.meta.url));
const pkg = fileURLToPath(new URL("./", import.meta.url));
for (const [target, directory] of [["web", "web"], ["nodejs", "node"]]) {
  const result = spawnSync(process.env.REXAFS_WASM_PACK ?? "wasm-pack", ["build", "crates/rexafs-wasm", "--target", target,
    "--out-dir", `${pkg}dist/${directory}`, "--release", "--", "--locked"], { cwd: root, stdio: "inherit" });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
  // Old wasm-pack output may contain `*`, which also excludes assets from npm pack.
  rmSync(`${pkg}dist/${directory}/.gitignore`, { force: true });
}
writeFileSync(`${pkg}dist/node/package.json`, '{"type":"commonjs"}\n');
writeFileSync(`${pkg}dist/web/package.json`, '{"type":"module"}\n');
for (const license of ["LICENSE-MIT", "LICENSE-APACHE"]) copyFileSync(`${root}${license}`, `${pkg}${license}`);
