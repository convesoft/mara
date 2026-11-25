// Soufflé's worker of a browser workspace (DES-RULE-EXECUTION): the workspace's worker starts it
// and terminates it at an evaluation's time limit, so that it holds nothing of the workspace. It
// compiles Soufflé's WebAssembly module once, then posts `{ ready: true }`, or `{ failed }` with
// the reason when the module cannot be loaded or compiled. For each run posted to it, it creates
// a fresh instance on an in-memory filesystem, writes the run's files into its private directory
// `/run`, runs `main` with the run's arguments and standard input, and posts what the run left.

import { Stopped, limitWrites } from "./output-limit.js";

const glueUrl = new URL("./souffle.js", import.meta.url);
const moduleUrl = new URL("./souffle.wasm", import.meta.url);

const loaded = (async () => {
  let factory;
  try {
    factory = (await import(glueUrl.href)).default;
  } catch (error) {
    throw new Error(`cannot load Soufflé's module ${glueUrl}: ${error.message}`);
  }
  let response;
  try {
    response = await fetch(moduleUrl);
  } catch (error) {
    throw new Error(`cannot load Soufflé's WebAssembly module ${moduleUrl}: ${error.message}`);
  }
  if (!response.ok) {
    throw new Error(
      `cannot load Soufflé's WebAssembly module ${moduleUrl}: HTTP status ${response.status}`,
    );
  }
  try {
    const compiled = await WebAssembly.compile(await response.arrayBuffer());
    return { factory, compiled };
  } catch (error) {
    throw new Error(`cannot compile Soufflé's WebAssembly module ${moduleUrl}: ${error.message}`);
  }
})();
loaded.then(
  () => self.postMessage({ ready: true }),
  (error) => self.postMessage({ failed: error.message }),
);

// What a stream printed, at most `bound` bytes of it, and how much it printed in all.
class Printed {
  #buffer;
  #bound;
  #size = 0;

  constructor(bound) {
    this.#buffer = new Uint8Array(Math.min(bound, 1 << 16));
    this.#bound = bound;
    this.length = 0;
  }

  push(byte) {
    this.length += 1;
    if (this.#size >= this.#bound) {
      return;
    }
    if (this.#size === this.#buffer.length) {
      const grown = new Uint8Array(Math.min(this.#bound, this.#buffer.length * 2));
      grown.set(this.#buffer);
      this.#buffer = grown;
    }
    this.#buffer[this.#size++] = byte;
  }

  text(text) {
    for (const byte of new TextEncoder().encode(text)) {
      this.push(byte);
    }
  }

  // The bytes kept.
  bytes() {
    return this.#buffer.slice(0, this.#size);
  }
}

// Runs `run` in a fresh instance of the compiled module.
async function evaluate({ factory, compiled }, run) {
  const stdout = new Printed(run.stdout_bytes);
  const stderr = new Printed(run.stderr_bytes);
  let stderrAtStdout = null;
  let status = null;
  let read = 0;
  let limit = null;
  // Natively each stream is a file within the output-size limit, past which the system ends
  // Soufflé (`SIGXFSZ`); in provenance mode its standard output is a pipe that the module
  // bounds instead.
  const bounded = (printed, file) => {
    if (file && printed.length > run.output_bytes) {
      limit = "output";
      throw new Stopped();
    }
  };
  const instance = {
    // A copy: the runtime puts the program's name before them.
    arguments: [...run.arguments],
    // A device, not a terminal, as a pipe or `/dev/null` is: Soufflé prints no prompt.
    stdin: () => (run.stdin !== null && read < run.stdin.length ? run.stdin[read++] : null),
    stdout: (byte) => {
      if (byte !== null) {
        stderrAtStdout ??= stderr.length;
        stdout.push(byte);
        bounded(stdout, run.stdin === null);
      }
    },
    stderr: (byte) => {
      if (byte !== null) {
        stderr.push(byte);
        bounded(stderr, true);
      }
    },
    // The runtime's own messages, such as an abort's.
    print: (text) => stdout.text(`${text}\n`),
    printErr: (text) => stderr.text(`${text}\n`),
    onExit: (code) => {
      status = code;
    },
    instantiateWasm: (imports, done) => {
      WebAssembly.instantiate(compiled, imports).then((created) => done(created, compiled));
      return {};
    },
    preRun: [
      (module) => {
        const { FS } = module;
        FS.mkdir("/run");
        FS.mkdir("/run/out");
        for (const { path, contents } of run.files) {
          FS.writeFile(`/run/${path}`, contents);
        }
        FS.chdir("/run");
        // A write that would take a file of the private directory past the output-size limit
        // ends the run, as the system ends Soufflé's process natively.
        limitWrites(FS, "/run/", run.output_bytes, () => {
          limit = "output";
        });
      },
    ],
  };
  try {
    await factory(instance);
  } catch (error) {
    // `main` returned or `exit` was called: `onExit` has the status. Anything else stopped the
    // module: an abort, whose message the runtime printed, or a trap.
    if (error?.name !== "ExitStatus") {
      status = null;
      const message = String(error?.message ?? error);
      if (!(error instanceof Stopped) && !message.startsWith("Aborted(")) {
        stderr.text(`Soufflé's module trapped: ${message}\n`);
      }
    }
  }
  const outputs = [];
  try {
    for (const name of instance.FS.readdir("/run/out")) {
      if (name === "." || name === "..") {
        continue;
      }
      const path = `/run/out/${name}`;
      // Also checked after the run, should a write have passed `limitWrites`.
      if (instance.FS.stat(path).size > run.output_bytes) {
        limit = "output";
        outputs.length = 0;
        break;
      }
      outputs.push({ path: `out/${name}`, contents: instance.FS.readFile(path) });
    }
  } catch {
    // An instance that stopped before its filesystem existed wrote no output.
  }
  // A module that could not grow its memory past the memory limit aborts with "OOM".
  if (status === null && new TextDecoder().decode(stderr.bytes()).includes("Aborted(OOM)")) {
    limit = "memory";
  }
  return {
    status,
    limit,
    stdout: stdout.bytes(),
    stdout_length: stdout.length,
    stderr: stderr.bytes(),
    stderr_at_stdout: stderrAtStdout ?? stderr.length,
    outputs,
  };
}

// Posts `{ ran }`, or `{ fault }` when the worker itself failed outside the module.
self.onmessage = async ({ data }) => {
  let ran;
  try {
    ran = await evaluate(await loaded, data);
  } catch (error) {
    self.postMessage({ fault: `Soufflé's worker failed: ${error?.message ?? error}` });
    return;
  }
  const transfer = [
    ran.stdout.buffer,
    ran.stderr.buffer,
    ...ran.outputs.map((output) => output.contents.buffer),
  ];
  self.postMessage({ ran }, transfer);
};
