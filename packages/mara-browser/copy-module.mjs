// `nx run mara-browser:build`: copies the WebAssembly module that `mara-wasm:build` wrote into
// the package, with Soufflé's module that `cargo-workspace:souffle-wasm` built
// (ADR-NX-RUST-TASKS); mara-865a2e packs Soufflé's. Nx stores that build's module at `target/` of the workspace,
// where nx-rust names its outputs. When Cargo writes elsewhere, through `CARGO_TARGET_DIR` or
// `build.target-dir`, a build that Nx restored leaves the module only at `target/` and the one in
// Cargo's directory may be older, so the copy refuses instead of taking a module that this build
// did not write (mara-6z7bjs).

import { execFileSync } from "node:child_process";
import { copyFileSync, realpathSync } from "node:fs";
import { join } from "node:path";

const root = process.cwd();
const real = (path) => {
  try {
    return realpathSync(path);
  } catch {
    return path;
  }
};
const metadata = JSON.parse(
  execFileSync("cargo", ["metadata", "--format-version", "1", "--no-deps", "--locked"], {
    cwd: root,
    encoding: "utf8",
    maxBuffer: 64 << 20,
  }),
);
const expected = join(root, "target");
if (real(metadata.target_directory) !== real(expected)) {
  console.error(
    `mara-browser:build: Cargo writes to ${metadata.target_directory}, not ${expected}, where ` +
      "mara-wasm:build leaves the module that Nx stores, so the package could get a module that " +
      "this build did not write. Build the browser package without CARGO_TARGET_DIR or " +
      "build.target-dir (mara-6z7bjs).",
  );
  process.exit(1);
}
const packageDir = join(root, "packages", "mara-browser");
copyFileSync(
  join(expected, "wasm32-unknown-unknown", "release", "mara_wasm.wasm"),
  join(packageDir, "mara.wasm"),
);
for (const name of ["souffle.js", "souffle.wasm"]) {
  copyFileSync(join(expected, "souffle-2.5-wasm", name), join(packageDir, name));
}
