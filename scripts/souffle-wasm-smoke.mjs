// The smoke run of scripts/build-souffle-wasm.sh: `node souffle-wasm-smoke.mjs <dir>` runs the
// module `<dir>/souffle.js` with `<dir>/souffle.wasm` as the browser workspace's Soufflé worker
// does (DES-RULE-EXECUTION in docs/rules/evaluation.mara.md): compiled once, a fresh instance on
// an in-memory filesystem, Mara's arguments, JSON facts and outputs, and the exit status of
// `main`. It fails unless the program's rows are those that the native smoke run expects.

import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

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
let status = null;
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
    },
  ],
};
try {
  await SOUFFLE(module);
} catch (error) {
  if (error?.name !== "ExitStatus") {
    throw error;
  }
}
if (status !== 0) {
  throw new Error(`exit status ${status}: ${printed}`);
}
const rows = new TextDecoder()
  .decode(module.FS.readFile("/run/out/path.json"))
  .replace(/[ \n]/g, "");
if (rows !== '[["a","b\\tc"],["a","d"],["b\\tc","d"]]') {
  throw new Error(`unexpected output: ${rows}`);
}
