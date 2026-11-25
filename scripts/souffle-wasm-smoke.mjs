// The smoke run of scripts/build-souffle-wasm.sh: `node souffle-wasm-smoke.mjs <dir>` runs the
// module `<dir>/souffle.js` with `<dir>/souffle.wasm` as the browser workspace's Soufflé worker
// does (DES-RULE-EXECUTION in docs/rules/evaluation.mara.md): compiled once, a fresh instance on
// an in-memory filesystem, Mara's arguments, JSON facts and outputs, and the exit status of
// `main`. It fails unless the program's rows are those that the native smoke run expects, and
// unless the worker's output-size guard (packages/mara-browser/output-limit.js) stops the same
// run at a bound of 16 bytes: a module whose writes bypass the guard fails here.

import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

import { Stopped, limitWrites } from "../packages/mara-browser/output-limit.js";

const dir = process.argv[2];
const { default: SOUFFLE } = await import(pathToFileURL(`${dir}/souffle.js`).href);
const compiled = await WebAssembly.compile(readFileSync(`${dir}/souffle.wasm`));
const WRAPPER = `.decl edge(a: symbol, b: symbol)
.decl path(a: symbol, b: symbol)
path(x, y) :- edge(x, y).
path(x, z) :- path(x, y), edge(y, z).
.input edge(IO=jsonfile, format=list, filename="edge.json")
.output path(IO=jsonfile, format=list, filename="out/path.json")
`;

/** Runs the wrapper in a fresh instance, each file under `/run/` bounded to `bound` bytes if set. */
async function run(bound) {
  let status = null;
  let stopped = false;
  let printed = "";
  const module = {
    arguments: ["--no-preprocessor", "-l", "", "wrapper.dl"],
    print: (text) => {
      printed += `${text}\n`;
    },
    printErr: (text) => {
      printed += `${text}\n`;
    },
    onExit: (code) => {
      status = code;
    },
    instantiateWasm: (imports, done) => {
      WebAssembly.instantiate(compiled, imports).then((instance) => done(instance, compiled));
      return {};
    },
    preRun: [
      (m) => {
        m.FS.mkdir("/run");
        m.FS.mkdir("/run/out");
        m.FS.chdir("/run");
        m.FS.writeFile("wrapper.dl", WRAPPER);
        m.FS.writeFile("edge.json", '[["a","b\\tc"],["b\\tc","d"]]');
        if (bound !== undefined) {
          limitWrites(m.FS, "/run/", bound, () => {
            stopped = true;
          });
        }
      },
    ],
  };
  try {
    await SOUFFLE(module);
  } catch (error) {
    if (error?.name !== "ExitStatus" && !(error instanceof Stopped)) {
      throw error;
    }
  }
  return { module, status, stopped, printed };
}

const unbounded = await run();
if (unbounded.status !== 0) {
  throw new Error(`exit status ${unbounded.status}: ${unbounded.printed}`);
}
const rows = new TextDecoder()
  .decode(unbounded.module.FS.readFile("/run/out/path.json"))
  .replace(/[ \n]/g, "");
if (rows !== '[["a","b\\tc"],["a","d"],["b\\tc","d"]]') {
  throw new Error(`unexpected output: ${rows}`);
}
const bounded = await run(16);
if (!bounded.stopped || bounded.status === 0) {
  throw new Error(
    `a write past 16 bytes was not stopped (exit status ${bounded.status}): the module's ` +
      "writes bypass FS.write, so the output-size guard of Soufflé's worker does not apply",
  );
}
// Emscripten's runtime under Node sets `process.exitCode` to the stopped run's status.
process.exitCode = 0;
